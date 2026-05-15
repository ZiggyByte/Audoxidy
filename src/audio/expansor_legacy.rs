#[derive(Clone)]
pub struct MultiBandStereoExpander {
    pub enabled: bool,
    pub width: f32,
    // Crossover 1: 250Hz (Low vs MidHigh)
    low_lp1: BiquadFilter, low_lp2: BiquadFilter,
    low_hp1: BiquadFilter, low_hp2: BiquadFilter,
    // Crossover 2: 5000Hz (Mid vs High)
    mid_lp1: BiquadFilter, mid_lp2: BiquadFilter,
    mid_hp1: BiquadFilter, mid_hp2: BiquadFilter,
    // Air Boost for Side Channel
    high_shelf_side: BiquadFilter,
    // Decorrelators for Mid Side channel (Enveloping body)
    mid_ap1: AllPassFilter,
    mid_ap2: AllPassFilter,
    // Decorrelators for High Side channel (Natural texture/Air)
    side_ap1: AllPassFilter,
    side_ap2: AllPassFilter,
    side_ap3: AllPassFilter, // Tercera etapa para mayor suavidad
    sample_rate: f32,
}

impl Default for MultiBandStereoExpander {
    fn default() -> Self {
        let sr = 44100.0;
        let q = 0.7071; // Butterworth Q for LR4 stages
        Self {
            enabled: false,
            width: 1.0,
            low_lp1: BiquadFilter::new(BiquadFilterType::LowPass, 250.0, 0.0, q),
            low_lp2: BiquadFilter::new(BiquadFilterType::LowPass, 250.0, 0.0, q),
            low_hp1: BiquadFilter::new(BiquadFilterType::HighPass, 250.0, 0.0, q),
            low_hp2: BiquadFilter::new(BiquadFilterType::HighPass, 250.0, 0.0, q),
            mid_lp1: BiquadFilter::new(BiquadFilterType::LowPass, 5000.0, 0.0, q),
            mid_lp2: BiquadFilter::new(BiquadFilterType::LowPass, 5000.0, 0.0, q),
            mid_hp1: BiquadFilter::new(BiquadFilterType::HighPass, 5000.0, 0.0, q),
            mid_hp2: BiquadFilter::new(BiquadFilterType::HighPass, 5000.0, 0.0, q),
            high_shelf_side: BiquadFilter::new(BiquadFilterType::HighShelf, 8000.0, 0.0, 0.5),
            mid_ap1: AllPassFilter::new(631),
            mid_ap2: AllPassFilter::new(1031),
            side_ap1: AllPassFilter::new(227),
            side_ap2: AllPassFilter::new(337),
            side_ap3: AllPassFilter::new(557),
            sample_rate: sr,
        }
    }
}

impl MultiBandStereoExpander {
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        self.low_lp1.set_sample_rate(rate); self.low_lp2.set_sample_rate(rate);
        self.low_hp1.set_sample_rate(rate); self.low_hp2.set_sample_rate(rate);
        self.mid_lp1.set_sample_rate(rate); self.mid_lp2.set_sample_rate(rate);
        self.mid_hp1.set_sample_rate(rate); self.mid_hp2.set_sample_rate(rate);
        self.high_shelf_side.set_sample_rate(rate);
        // Los AllPass ya se ajustan internamente o tienen tamaños fijos pequeños para textura
    }

    pub fn process(&mut self, frame: &mut [f64]) {
        if frame.len() != 2 || self.width == 1.0 { return; }

        let mut low_band = [frame[0], frame[1]];
        let mut mid_high_band = [frame[0], frame[1]];

        // 1. Separar Low (< 250Hz)
        self.low_lp1.process_frame(&mut low_band);
        self.low_lp2.process_frame(&mut low_band);
        self.low_hp1.process_frame(&mut mid_high_band);
        self.low_hp2.process_frame(&mut mid_high_band);

        // El Low se fuerza a Mono para mantener el punch
        // Compensamos con un ligero boost (+1.0dB) para recuperar presencia
        let low_mono = (low_band[0] + low_band[1]) * 0.5 * 1.0_f64;
        low_band[0] = low_mono;
        low_band[1] = low_mono;

        // 2. Separar Mid (250Hz - 5kHz) y High (> 5kHz)
        let mut mid_band = [mid_high_band[0], mid_high_band[1]];
        let mut high_band = [mid_high_band[0], mid_high_band[1]];

        self.mid_lp1.process_frame(&mut mid_band);
        self.mid_lp2.process_frame(&mut mid_band);
        self.mid_hp1.process_frame(&mut high_band);
        self.mid_hp2.process_frame(&mut high_band);

        self.expand_band(&mut mid_band, self.width as f64);

        // Aplicar Inmersión a la banda Media (Side channel only)
        let m_mid = (mid_band[0] + mid_band[1]) * 0.5;
        let mut m_side = (mid_band[0] - mid_band[1]) * 0.5;

        m_side = self.mid_ap1.process(m_side);
        m_side = self.mid_ap2.process(m_side);

        mid_band[0] = m_mid + m_side;
        mid_band[1] = m_mid - m_side;

        // 4. Procesar High Band (Expansión suave + Air Boost)
        let high_width = (self.width as f64 - 1.0) * 0.8 + 1.0; // Reducida agresividad para mayor naturalidad
        self.expand_band(&mut high_band, high_width);

        // Aplicar Air Boost y Decorrelación solo al canal Side de la banda alta
        let h_mid = (high_band[0] + high_band[1]) * 0.5;
        let mut h_side_val = (high_band[0] - high_band[1]) * 0.5;

        // 1. Decorrelación de fase multinivel (Inmersión cristalina)
        h_side_val = self.side_ap1.process(h_side_val);
        h_side_val = self.side_ap2.process(h_side_val);
        h_side_val = self.side_ap3.process(h_side_val);

        let mut h_side_buf = [h_side_val, 0.0];

        let air_gain = ((self.width - 1.0) * 1.5).clamp(0.0, 2.0); // 2db de air boost (Refinado)
        if air_gain > 0.0 {
            self.high_shelf_side.gain = air_gain;
            self.high_shelf_side.update_coefficients(self.sample_rate);
            self.high_shelf_side.process_frame(&mut h_side_buf);
        }

        high_band[0] = h_mid + h_side_buf[0];
        high_band[1] = h_mid - h_side_buf[0];

        // 5. Recombinar todas las bandas con equilibrio tonal optimizado
        // Aplicamos una ligerísima atenuación en agudos para que se sientan "dentro" de la escena
        frame[0] = low_band[0] + mid_band[0] + high_band[0] * 0.90;
        frame[1] = low_band[1] + mid_band[1] + high_band[1] * 0.90;
    }

    fn expand_band(&self, band: &mut [f64], width: f64) {
        let mid = (band[0] + band[1]) * 0.5;
        let side = (band[0] - band[1]) * 0.5;
        band[0] = mid + side * width;
        band[1] = mid - side * width;
    }

    pub fn reset_state(&mut self) {
        self.low_lp1.reset_state(); self.low_lp2.reset_state();
        self.low_hp1.reset_state(); self.low_hp2.reset_state();
        self.mid_lp1.reset_state(); self.mid_lp2.reset_state();
        self.mid_hp1.reset_state(); self.mid_hp2.reset_state();
        self.high_shelf_side.reset_state();
        self.mid_ap1.reset();
        self.mid_ap2.reset();
        self.side_ap1.reset();
        self.side_ap2.reset();
        self.side_ap3.reset();
    }
}
