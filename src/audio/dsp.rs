use std::collections::VecDeque;

use wide::CmpLt;

/// Cadena de procesamiento DSP de Audoxidy.
///
/// Aplica en orden: preamplificador, ecualizador, noise gate, sub-bass, mid-bass,
/// voice boost, compresor, reverberación, expansor estéreo, balance y limitador.
pub struct DspChain {
    pub preamp_gain: f32, // Linear gain
    pub equalizer: Equalizer,
    pub reverb: Reverb,
    pub compressor: Compressor,
    pub sub_bass: SubBass,
    pub mid_bass: MidBass,
    pub voice_boost: VoiceBoost,
    pub stereo_expander: MultiBandStereoExpander,
    pub noise_gate: NoiseGate,
    pub limiter: Limiter,
    pub stereo_balance: StereoBalance,
    pub enabled: bool,
}

impl Default for DspChain {
    fn default() -> Self {
        Self {
            preamp_gain: 1.0,
            equalizer: Equalizer::new(31), // Default 31 bands
            reverb: Reverb::new(),
            compressor: Compressor::new(),
            sub_bass: SubBass::default(),
            mid_bass: MidBass::default(),
            voice_boost: VoiceBoost::default(),
            stereo_expander: MultiBandStereoExpander::default(),
            noise_gate: NoiseGate::default(),
            limiter: Limiter::default(),
            stereo_balance: StereoBalance::default(),
            enabled: true,
        }
    }
}

#[allow(dead_code)]
impl DspChain {
    /// Procesa un frame completo (todos los canales) a través de la cadena DSP.
    ///
    /// Si `enabled` es `false`, retorna sin modificar el frame.
    pub fn process_frame(&mut self, frame: &mut [f64]) {
        if !self.enabled {
            return;
        }

        // Apply Preamp (Only if Equalizer module is active/enabled)
        if self.equalizer.enabled {
            for s in frame.iter_mut() {
                *s *= self.preamp_gain as f64;
            }
            // Apply EQ — procesa frame completo con SIMD (x86_64)
            self.equalizer.process_frame(frame);
        }

        // Noise Gate
        if self.noise_gate.enabled {
            self.noise_gate.process(frame);
        }

        // Sub Bass
        if self.sub_bass.enabled {
            self.sub_bass.process(frame);
        }

        // Mid Bass
        if self.mid_bass.enabled {
            self.mid_bass.process(frame);
        }

        // Voice Boost
        if self.voice_boost.enabled {
            self.voice_boost.process(frame);
        }

        // Compressor
        if self.compressor.enabled {
            self.compressor.process(frame);
        }

        // Reverb
        if self.reverb.enabled {
            self.reverb.process(frame);
        }

        // Stereo Effects (only if frame has 2 channels)
        if frame.len() == 2 {
            if self.stereo_expander.enabled {
                self.stereo_expander.process(frame);
            }
            if self.stereo_balance.enabled {
                self.stereo_balance.process(frame);
            }
        }

        // Limiter (Always last)
        if self.limiter.enabled {
            self.limiter.process(frame);
        }
    }

    /// Force limiter on. Returns the previous enabled state for restore.
    /// Used by normalization auto-on (D-25).
    pub fn force_limiter_on(&mut self) -> bool {
        let was = self.limiter.enabled;
        self.limiter.enabled = true;
        was
    }

    /// Restore limiter to its previous user-controlled state.
    pub fn restore_limiter(&mut self, was_enabled: bool) {
        self.limiter.enabled = was_enabled;
    }

    /// Actualiza la frecuencia de muestreo en todos los módulos DSP.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.equalizer.set_sample_rate(sample_rate);
        self.sub_bass.set_sample_rate(sample_rate);
        self.mid_bass.set_sample_rate(sample_rate);
        self.voice_boost.set_sample_rate(sample_rate);
        self.compressor.set_sample_rate(sample_rate);
        self.noise_gate.set_sample_rate(sample_rate);
        self.stereo_expander.set_sample_rate(sample_rate);
        self.limiter.set_sample_rate(sample_rate);
        self.reverb.set_sample_rate(sample_rate);
    }

    /// Redimensiona el estado interno de los filtros para el número de canales dado.
    pub fn set_channel_count(&mut self, channels: usize) {
        self.equalizer.set_channel_count(channels);
        self.sub_bass.resize_channels(channels);
        self.mid_bass.resize_channels(channels);
        self.voice_boost.resize_channels(channels);
    }

    /// Devuelve la ganancia del preamplificador en dB.
    pub fn get_preamp_db(&self) -> f32 {
        if self.preamp_gain > 0.0 {
            20.0 * self.preamp_gain.log10()
        } else {
            -96.0
        }
    }

    /// Establece la ganancia del preamplificador en dB.
    ///
    /// Convierte el valor a ganancia lineal internamente.
    pub fn set_preamp_db(&mut self, db: f32) {
        self.preamp_gain = 10.0f32.powf(db / 20.0);
    }

    pub fn reset_state(&mut self) {
        self.equalizer.reset_state();
        self.reverb.reset_state();
        self.compressor.reset_state();
        self.sub_bass.reset_state();
        self.mid_bass.reset_state();
        self.voice_boost.reset_state();
        self.stereo_expander.reset_state();
        self.noise_gate.reset_state();
        self.limiter.reset_state();
    }
}

/// Ecualizador paramétrico de múltiples bandas (20 o 31 bandas).
///
/// Cada banda es un filtro biquad peak con frecuencia, ganancia y Q configurables.
/// Usa SIMD (f64x4) en x86_64 para procesamiento eficiente.
#[derive(Clone)]
pub struct Equalizer {
    pub bands: Vec<EqBand>,
    pub enabled: bool,
    // Persistence
    saved_bands_20: Vec<EqBand>,
    saved_bands_31: Vec<EqBand>,
    last_sample_rate: f32,
    last_channel_count: usize,
}

impl Equalizer {
    /// Crea un ecualizador con el número de bandas especificado (20 o 31).
    pub fn new(num_bands: usize) -> Self {
        // Create both sets initially
        let bands_20 = Self::create_bands(20);
        let bands_31 = Self::create_bands(31);

        // Default to num_bands, or fallback to 31 if invalid
        let active = if num_bands == 20 {
            bands_20.clone()
        } else {
            bands_31.clone()
        };

        Self {
            bands: active,
            enabled: false,
            saved_bands_20: bands_20,
            saved_bands_31: bands_31,
            last_sample_rate: 44100.0, // Default
            last_channel_count: 2,     // Default
        }
    }

    fn create_bands(num_bands: usize) -> Vec<EqBand> {
        let freqs = match num_bands {
            20 => vec![
                22.4, 31.5, 45.0, 63.0, 90.0, 125.0, 180.0, 250.0, 355.0, 500.0, 710.0, 1000.0,
                1400.0, 2000.0, 2800.0, 4000.0, 5600.0, 8000.0, 11200.0, 16000.0,
            ],
            31 => vec![
                20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0,
                400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0,
                5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
            ],
            _ => vec![1000.0], // Should not happen for our UI
        };

        if num_bands == 20 {
            freqs
                .into_iter()
                .map(|f| {
                    let mut b = EqBand::new(f);
                    b.set_q(2.87);
                    b
                })
                .collect()
        } else {
            freqs
                .into_iter()
                .map(|f| {
                    let mut b = EqBand::new(f);
                    b.set_q(4.4);
                    b
                })
                .collect()
        }
    }

    /// Cambia entre modo de 20 y 31 bandas preservando los estados guardados.
    pub fn set_mode(&mut self, num_bands: usize) {
        // 1. Save current state
        if self.bands.len() == 20 {
            self.saved_bands_20 = self.bands.clone();
        } else if self.bands.len() == 31 {
            self.saved_bands_31 = self.bands.clone();
        }

        // 2. Load requested state
        if num_bands == 20 {
            self.bands = self.saved_bands_20.clone();
        } else {
            self.bands = self.saved_bands_31.clone();
        }

        // 3. Update coefficients with current settings immediately
        // This fixes the "frequency shift" bug when switching
        for band in &mut self.bands {
            band.resize_channels(self.last_channel_count);
            band.update_coefficients(self.last_sample_rate);
        }
    }

    // Initialize/Update Sample Rate
    /// Actualiza los coeficientes de todas las bandas al nuevo sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.last_sample_rate = sample_rate;
        for band in &mut self.bands {
            band.update_coefficients(sample_rate);
        }
        // Also update saved states to prevent stale filters?
        // Actually no, filters need re-update on load anyway.
        // But gains are what we care about saving.
    }

    // Initialize/Update Channels
    /// Redimensiona los estados internos de los filtros por canal.
    pub fn set_channel_count(&mut self, channels: usize) {
        self.last_channel_count = channels;
        for band in &mut self.bands {
            band.resize_channels(channels);
        }
    }

    #[allow(dead_code)]
    pub fn process(&mut self, sample: &mut f64, channel_idx: usize) {
        if !self.enabled {
            return;
        }
        for band in &mut self.bands {
            band.process(sample, channel_idx);
        }
    }

    /// Procesa un frame completo (todos los canales) a través de todas las bandas.
    /// Usa EqBand::process_frame con SIMD para aceleración en x86_64.
    /// Preferir esta sobre `process()` cuando se tenga un frame completo.
    pub fn process_frame(&mut self, frame: &mut [f64]) {
        if !self.enabled {
            return;
        }
        for band in &mut self.bands {
            band.process_frame(frame);
        }
    }

    /// Pone a cero la ganancia de todas las bandas (20 y 31).
    pub fn reset_all(&mut self) {
        for band in &mut self.bands {
            band.set_gain(0.0);
        }
        for band in &mut self.saved_bands_20 {
            band.set_gain(0.0);
        }
        for band in &mut self.saved_bands_31 {
            band.set_gain(0.0);
        }
    }

    /// Apply gains from a preset to both saved band sets and the active bands.
    /// bands_20 and bands_31 are the full 20/31-element gain arrays from the preset.
    pub fn apply_preset_gains(&mut self, bands_20: &[f32], bands_31: &[f32]) {
        // Update saved_bands_20 (always — both sets get stored)
        for (i, &gain) in bands_20.iter().enumerate() {
            if i < self.saved_bands_20.len() {
                self.saved_bands_20[i].set_gain(gain);
            }
        }
        // Update saved_bands_31 (always)
        for (i, &gain) in bands_31.iter().enumerate() {
            if i < self.saved_bands_31.len() {
                self.saved_bands_31[i].set_gain(gain);
            }
        }
        // Update active bands according to current mode (D-10: don't change mode)
        let active_gains = if self.bands.len() == 20 {
            bands_20
        } else {
            bands_31
        };
        for (i, &gain) in active_gains.iter().enumerate() {
            if i < self.bands.len() {
                self.bands[i].set_gain(gain);
            }
        }
    }

    /// Resetea el estado interno de todas las bandas del ecualizador.
    pub fn reset_state(&mut self) {
        for band in &mut self.bands {
            band.reset_state();
        }
        for band in &mut self.saved_bands_20 {
            band.reset_state();
        }
        for band in &mut self.saved_bands_31 {
            band.reset_state();
        }
    }
}

#[derive(Clone, Default)]
struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}
impl BiquadState {
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

/// Banda de ecualización paramétrica (filtro biquad peak).
///
/// Cada banda tiene frecuencia, ganancia (dB) y Q configurables,
/// con estado por canal para soporte multicanal (hasta 7.1).
#[derive(Clone)]
pub struct EqBand {
    pub frequency: f32,
    pub gain: f32, // dB
    pub q: f32,
    // Biquad coefficients (shared across channels)
    #[allow(dead_code)]
    a0: f64,
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    // State per channel
    states: Vec<BiquadState>,
    last_sample_rate: f32, // Store last SR to re-calculate if needed in set_gain/q
}

impl EqBand {
    /// Crea una nueva banda de ecualización con la frecuencia central dada.
    pub fn new(freq: f32) -> Self {
        let mut band = Self {
            frequency: freq,
            gain: 0.0,
            q: 1.41,
            a0: 1.0,
            a1: 0.0,
            a2: 0.0,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            states: vec![BiquadState::default(); 8], // Pre-alloc for 7.1/8 channels default
            last_sample_rate: 44100.0,
        };
        band.update_coefficients(44100.0);
        band
    }

    /// Establece la ganancia de la banda en dB y actualiza los coeficientes.
    pub fn set_gain(&mut self, gain_db: f32) {
        self.gain = gain_db;
        self.update_coefficients(self.last_sample_rate);
    }

    // Add set_sample_rate aware setter if needed, or just update coeffs after.

    /// Establece el factor Q de la banda y actualiza los coeficientes.
    pub fn set_q(&mut self, q: f32) {
        self.q = q;
        self.update_coefficients(self.last_sample_rate);
    }

    /// Redimensiona el buffer de estados por canal.
    pub fn resize_channels(&mut self, channels: usize) {
        if self.states.len() != channels {
            self.states.resize(channels, BiquadState::default());
        }
    }

    /// Resetea el estado interno del filtro biquad (pone historial a cero).
    pub fn reset_state(&mut self) {
        for state in &mut self.states {
            state.reset();
        }
    }

    /*
     * Peaking EQ Filter Design
     */
    /// Recalcula los coeficientes del filtro biquad peak para el sample rate dado.
    pub fn update_coefficients(&mut self, sample_rate: f32) {
        self.last_sample_rate = sample_rate;
        if sample_rate <= 0.0 {
            return;
        }

        let w0 = 2.0 * std::f64::consts::PI * (self.frequency as f64) / (sample_rate as f64);
        let c = w0.cos();
        let s = w0.sin();
        let alpha = s / (2.0 * (self.q as f64));

        let a = 10.0f64.powf((self.gain as f64) / 40.0); // A = 10^(dB/40)

        // Peaking EQ coeffs
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * c;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * c;
        let a2 = 1.0 - alpha / a;

        // Normalized
        if a0.abs() > 1e-6 {
            self.b0 = b0 / a0;
            self.b1 = b1 / a0;
            self.b2 = b2 / a0;
            self.a1 = a1 / a0;
            self.a2 = a2 / a0;
        } else {
            // Fallback bypass
            self.b0 = 1.0;
            self.b1 = 0.0;
            self.b2 = 0.0;
            self.a1 = 0.0;
            self.a2 = 0.0;
        }
    }

    #[allow(dead_code)]
    pub fn process(&mut self, sample: &mut f64, channel_idx: usize) {
        if self.gain == 0.0 {
            return;
        }
        if channel_idx >= self.states.len() {
            return;
        }

        let state = &mut self.states[channel_idx];
        let x = *sample;
        let y = self.b0 * x + self.b1 * state.x1 + self.b2 * state.x2
            - self.a1 * state.y1
            - self.a2 * state.y2;
        let y = if y.abs() < 1e-20 { 0.0 } else { y };
        state.x2 = state.x1;
        state.x1 = x;
        state.y2 = state.y1;
        state.y1 = y;
        *sample = y;
    }

    /// Procesa un frame completo (todos los canales) a través de este filtro biquad.
    /// Usa SIMD (f64x4) para procesar 4 canales simultáneamente en x86_64.
    pub fn process_frame(&mut self, frame: &mut [f64]) {
        if self.gain == 0.0 {
            return;
        }
        let b0 = self.b0;
        let b1 = self.b1;
        let b2 = self.b2;
        let a1 = self.a1;
        let a2 = self.a2;

        #[cfg(target_arch = "x86_64")]
        {
            let max_ch = frame.len().min(self.states.len());
            let mut i = 0;
            while i + 4 <= max_ch {
                let s_ptr = self.states.as_mut_ptr();
                let s0 = unsafe { &mut *s_ptr.add(i) };
                let s1 = unsafe { &mut *s_ptr.add(i + 1) };
                let s2 = unsafe { &mut *s_ptr.add(i + 2) };
                let s3 = unsafe { &mut *s_ptr.add(i + 3) };

                let x = wide::f64x4::new([frame[i], frame[i + 1], frame[i + 2], frame[i + 3]]);
                let xs1 = wide::f64x4::new([s0.x1, s1.x1, s2.x1, s3.x1]);
                let xs2 = wide::f64x4::new([s0.x2, s1.x2, s2.x2, s3.x2]);
                let ys1 = wide::f64x4::new([s0.y1, s1.y1, s2.y1, s3.y1]);
                let ys2 = wide::f64x4::new([s0.y2, s1.y2, s2.y2, s3.y2]);

                let out = wide::f64x4::splat(b0) * x
                    + wide::f64x4::splat(b1) * xs1
                    + wide::f64x4::splat(b2) * xs2
                    - wide::f64x4::splat(a1) * ys1
                    - wide::f64x4::splat(a2) * ys2;
                let out = out.blend(
                    wide::f64x4::splat(0.0),
                    out.abs().cmp_lt(wide::f64x4::splat(1e-20)),
                );
                let arr = out.to_array();

                frame[i] = arr[0];
                frame[i + 1] = arr[1];
                frame[i + 2] = arr[2];
                frame[i + 3] = arr[3];

                s0.x2 = s0.x1;
                s0.x1 = frame[i];
                s0.y2 = s0.y1;
                s0.y1 = frame[i];
                s1.x2 = s1.x1;
                s1.x1 = frame[i + 1];
                s1.y2 = s1.y1;
                s1.y1 = frame[i + 1];
                s2.x2 = s2.x1;
                s2.x1 = frame[i + 2];
                s2.y2 = s2.y1;
                s2.y1 = frame[i + 2];
                s3.x2 = s3.x1;
                s3.x1 = frame[i + 3];
                s3.y2 = s3.y1;
                s3.y1 = frame[i + 3];

                i += 4;
            }
            // Canales restantes (<4) en modo escalar
            while i < max_ch {
                let s = &mut self.states[i];
                let x = frame[i];
                let y = b0 * x + b1 * s.x1 + b2 * s.x2 - a1 * s.y1 - a2 * s.y2;
                frame[i] = if y.abs() < 1e-20 { 0.0 } else { y };
                s.x2 = s.x1;
                s.x1 = x;
                s.y2 = s.y1;
                s.y1 = frame[i];
                i += 1;
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            for ch in 0..frame.len().min(self.states.len()) {
                let s = &mut self.states[ch];
                let x = frame[ch];
                let y = b0 * x + b1 * s.x1 + b2 * s.x2 - a1 * s.y1 - a2 * s.y2;
                frame[ch] = if y.abs() < 1e-20 { 0.0 } else { y };
                s.x2 = s.x1;
                s.x1 = x;
                s.y2 = s.y1;
                s.y1 = frame[ch];
            }
        }
    }
}
// --- Reverb (FDN implementation) ---

#[derive(Clone)]
struct DelayLine {
    buffer: Vec<f64>,
    index: usize,
}

impl DelayLine {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0_f64; size.max(1)],
            index: 0,
        }
    }

    fn read(&self) -> f64 {
        self.buffer[self.index]
    }

    fn write(&mut self, value: f64) {
        self.buffer[self.index] = value;
        self.index = (self.index + 1) % self.buffer.len();
    }

    fn reset(&mut self) {
        for s in &mut self.buffer {
            *s = 0.0;
        }
        self.index = 0;
    }
}

#[derive(Clone)]
struct AllPassFilter {
    delay: DelayLine,
    feedback: f64,
}

impl AllPassFilter {
    fn new(size: usize) -> Self {
        Self {
            delay: DelayLine::new(size),
            feedback: 0.5_f64,
        }
    }

    fn reset(&mut self) {
        self.delay.reset();
    }

    fn process(&mut self, input: f64) -> f64 {
        let buffered_val = self.delay.read();
        let input_combined = input + buffered_val * self.feedback;
        self.delay.write(input_combined);
        buffered_val - input_combined
    }
}

/// Prime-power delay lengths for FDN (16 lines, L channel).
const FDN_DELAY_LENGTHS_L: [usize; 16] = [
    2048, 729, 625, 343, 1331, 2197, 289, 361, 529, 841, 961, 1369, 1681, 1849, 2209, 2809,
];

/// Prime-power delay lengths for FDN (R channel, different exponents for decorrelation).
const FDN_DELAY_LENGTHS_R: [usize; 16] = [
    4096, 2187, 1250, 343, 2662, 2197, 578, 361, 1058, 841, 1922, 1369, 1681, 3698, 2209, 2809,
];

/// Construye la matriz de Hadamard 16×16 normalizada (entradas ±0.25).
fn hadamard_16() -> [[f64; 16]; 16] {
    let mut h = [[0.0_f64; 16]; 16];
    h[0][0] = 1.0;
    let mut size = 1;
    while size < 16 {
        for i in 0..size {
            for j in 0..size {
                let v = h[i][j];
                h[i][j + size] = v;
                h[i + size][j] = v;
                h[i + size][j + size] = -v;
            }
        }
        size *= 2;
    }
    // Normalize: divide by sqrt(16) = 4.0
    for row in h.iter_mut() {
        for v in row.iter_mut() {
            *v *= 0.25;
        }
    }
    h
}

/// Efecto de reverberación FDN (Feedback Delay Network) con 16 líneas de delay.
#[derive(Clone)]
pub struct Reverb {
    pub enabled: bool,
    pub wet: f32,
    pub room_size: f32,
    #[allow(dead_code)]
    pub width: f32,
    #[allow(dead_code)]
    pub dry: f32,
    gain: f32,
    // FDN L channel
    delay_lines_l: Vec<DelayLine>,
    damping_states_l: Vec<f64>,
    pre_delay_l: DelayLine,
    // FDN R channel
    delay_lines_r: Vec<DelayLine>,
    damping_states_r: Vec<f64>,
    pre_delay_r: DelayLine,
    // Params
    hadamard: [[f64; 16]; 16],
    damping_coeff: f64,
    pre_delay_samples: usize,
    feedback_gain: f64,
    sample_rate: f64,
}

#[allow(dead_code)]
impl Reverb {
    pub fn new() -> Self {
        let sr: f64 = 44100.0;
        let hadamard = hadamard_16();
        let scale = sr / 44100.0;

        let delay_lines_l: Vec<DelayLine> = FDN_DELAY_LENGTHS_L
            .iter()
            .map(|&base| DelayLine::new(((base as f64) * scale).round() as usize))
            .collect();
        let delay_lines_r: Vec<DelayLine> = FDN_DELAY_LENGTHS_R
            .iter()
            .map(|&base| DelayLine::new(((base as f64) * scale).round() as usize))
            .collect();
        let damping_states_l = vec![0.0_f64; 16];
        let damping_states_r = vec![0.0_f64; 16];

        let mut r = Self {
            enabled: false,
            wet: 0.5,
            room_size: 0.5,
            width: 1.0,
            dry: 0.5,
            gain: 0.12,
            delay_lines_l,
            damping_states_l,
            pre_delay_l: DelayLine::new(1),
            delay_lines_r,
            damping_states_r,
            pre_delay_r: DelayLine::new(1),
            hadamard,
            damping_coeff: 0.0,
            pre_delay_samples: 1,
            feedback_gain: 0.0,
            sample_rate: sr,
        };
        r.update_params();
        r
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        if sample_rate <= 0.0 {
            return;
        }
        self.sample_rate = sample_rate as f64;
        let scale = self.sample_rate / 44100.0;

        self.delay_lines_l = FDN_DELAY_LENGTHS_L
            .iter()
            .map(|&base| DelayLine::new(((base as f64) * scale).round() as usize))
            .collect();
        self.delay_lines_r = FDN_DELAY_LENGTHS_R
            .iter()
            .map(|&base| DelayLine::new(((base as f64) * scale).round() as usize))
            .collect();
        self.update_params();
    }

    pub fn set_room_size(&mut self, value: f32) {
        self.room_size = value.clamp(0.0, 1.0);
        self.update_params();
    }

    pub fn set_wet(&mut self, value: f32) {
        self.wet = value.clamp(0.0, 1.0);
    }

    pub fn set_dry(&mut self, value: f32) {
        self.dry = value.clamp(0.0, 1.0);
    }

    pub fn set_damping(&mut self, _value: f32) {
        // Damping is now auto-derived from room_size.
        // Kept for backward compat — call update_params() to apply.
    }

    fn update_params(&mut self) {
        let rs = self.room_size as f64;
        self.damping_coeff = 0.05 + rs * 0.3;
        self.pre_delay_samples = (rs * 0.040 * self.sample_rate).round() as usize;
        self.pre_delay_l = DelayLine::new(self.pre_delay_samples.max(1));
        self.pre_delay_r = DelayLine::new(self.pre_delay_samples.max(1));
        self.feedback_gain = 0.7 + rs * 0.25;
    }

    fn process_channel(
        delay_lines: &mut [DelayLine],
        damping_states: &mut [f64],
        pre_delay: &mut DelayLine,
        hadamard: &[[f64; 16]; 16],
        damping_coeff: f64,
        feedback_gain: f64,
        sample: &mut f64,
        gain: f64,
        wet: f64,
    ) {
        let input_orig = *sample;
        let input_scaled = input_orig * gain as f64;

        let fdn_input = pre_delay.read();
        pre_delay.write(input_scaled);

        let mut delay_outs = [0.0_f64; 16];
        for i in 0..16 {
            let dout = delay_lines[i].read();
            damping_states[i] = (1.0 - damping_coeff) * dout + damping_coeff * damping_states[i];
            delay_outs[i] = damping_states[i];
        }

        let mut feedback = [0.0_f64; 16];
        for i in 0..16 {
            let mut sum = 0.0_f64;
            for j in 0..16 {
                sum += hadamard[i][j] * delay_outs[j];
            }
            feedback[i] = sum;
        }

        for i in 0..16 {
            delay_lines[i].write(fdn_input + feedback[i] * feedback_gain);
        }

        let reverb_out = delay_outs.iter().sum::<f64>() / 8.0;

        // Linear crossfade with auto RMS compensation
        let wet_mix = (wet as f64).clamp(0.0, 1.0);
        let dry_mix = 1.0 - wet_mix;
        let wet_level = 0.5;
        let comp = 1.0 / (dry_mix + wet_mix * wet_level).max(0.1);
        *sample = (reverb_out * wet_mix + input_orig * dry_mix) * comp;

        if !sample.is_finite() {
            *sample = 0.0;
        }
    }

    pub fn process(&mut self, frame: &mut [f64]) {
        if !self.enabled {
            return;
        }

        let Self {
            delay_lines_l,
            damping_states_l,
            pre_delay_l,
            delay_lines_r,
            damping_states_r,
            pre_delay_r,
            hadamard,
            damping_coeff,
            feedback_gain,
            gain,
            wet,
            ..
        } = self;

        for (ch, sample) in frame.iter_mut().enumerate() {
            if ch == 0 {
                Self::process_channel(
                    delay_lines_l,
                    damping_states_l,
                    pre_delay_l,
                    hadamard,
                    *damping_coeff,
                    *feedback_gain,
                    sample,
                    *gain as f64,
                    *wet as f64,
                );
            } else {
                Self::process_channel(
                    delay_lines_r,
                    damping_states_r,
                    pre_delay_r,
                    hadamard,
                    *damping_coeff,
                    *feedback_gain,
                    sample,
                    *gain as f64,
                    *wet as f64,
                );
            }
        }
    }

    pub fn reset_state(&mut self) {
        for dl in self.delay_lines_l.iter_mut() {
            dl.reset();
        }
        for dl in self.delay_lines_r.iter_mut() {
            dl.reset();
        }
        self.pre_delay_l.reset();
        self.pre_delay_r.reset();
        for s in self.damping_states_l.iter_mut() {
            *s = 0.0;
        }
        for s in self.damping_states_r.iter_mut() {
            *s = 0.0;
        }
    }
}

// --- Compressor ---

/// Compresor premium con detección RMS, soft knee, lookahead y makeup gain.
#[derive(Clone)]
pub struct Compressor {
    pub enabled: bool,
    pub threshold: f32, // dB
    pub intensity: f32, // 0.0..1.0

    // Internal — derived from intensity
    pub intensity_ratio: f32,
    intensity_attack: f32,
    intensity_release: f32,
    knee_width: f32,  // dB
    makeup_gain: f32, // linear

    // Internal state
    pub envelope: f64,
    envelope_rms: f64,
    sample_rate: f32,
    lookahead: VecDeque<Vec<f64>>,
    pub lookahead_samples: usize,
}

#[allow(dead_code)]
impl Compressor {
    /// Crea un compresor con valores predeterminados.
    pub fn new() -> Self {
        let sr: f32 = 44100.0;
        let ls = (0.001 * sr).round() as usize;
        let mut c = Self {
            enabled: false,
            threshold: -3.0,
            intensity: 0.5,
            intensity_ratio: 4.0,
            intensity_attack: 0.005,
            intensity_release: 0.1,
            knee_width: 0.0,
            makeup_gain: 1.0,
            envelope: 0.0,
            envelope_rms: 0.0,
            sample_rate: sr,
            lookahead: VecDeque::with_capacity(ls + 1),
            lookahead_samples: ls,
        };
        c.update_intensity_params();
        c
    }

    /// Establece la frecuencia de muestreo para el seguidor de envolvente.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.lookahead_samples = (0.001 * sample_rate).round() as usize;
        self.lookahead = VecDeque::with_capacity(self.lookahead_samples + 1);
        self.update_intensity_params();
    }

    /// Configura los parámetros del compresor (deprecated — usar `intensity`).
    #[deprecated(
        note = "Use `intensity` field instead. Ratio/attack/release params are now derived from intensity."
    )]
    pub fn set_params(&mut self, threshold: f32, _ratio: f32, _attack: f32, _release: f32) {
        self.threshold = threshold;
        self.intensity = 0.5;
        self.update_intensity_params();
    }

    /// Actualiza ratio, attack, release, knee_width y makeup_gain a partir de `intensity` (0..1).
    pub fn update_intensity_params(&mut self) {
        let i = self.intensity.clamp(0.0, 1.0);

        // 5-point interpolation table (intensity normalized to 0..1)
        let lerp = |t: f32, a: f32, b: f32| a + (b - a) * t;

        let ratio = if i <= 0.25 {
            let t = i / 0.25;
            lerp(t, 1.5, 2.5)
        } else if i <= 0.5 {
            let t = (i - 0.25) / 0.25;
            lerp(t, 2.5, 4.0)
        } else if i <= 0.75 {
            let t = (i - 0.5) / 0.25;
            lerp(t, 4.0, 8.0)
        } else {
            let t = (i - 0.75) / 0.25;
            lerp(t, 8.0, 20.0)
        };

        let att_ms = if i <= 0.25 {
            let t = i / 0.25;
            lerp(t, 30.0, 15.0)
        } else if i <= 0.5 {
            let t = (i - 0.25) / 0.25;
            lerp(t, 15.0, 8.0)
        } else if i <= 0.75 {
            let t = (i - 0.5) / 0.25;
            lerp(t, 8.0, 3.0)
        } else {
            let t = (i - 0.75) / 0.25;
            lerp(t, 3.0, 1.0)
        };

        let rel_ms = if i <= 0.25 {
            let t = i / 0.25;
            lerp(t, 200.0, 150.0)
        } else if i <= 0.5 {
            let t = (i - 0.25) / 0.25;
            lerp(t, 150.0, 100.0)
        } else if i <= 0.75 {
            let t = (i - 0.5) / 0.25;
            lerp(t, 100.0, 60.0)
        } else {
            let t = (i - 0.75) / 0.25;
            lerp(t, 60.0, 30.0)
        };

        let knee_db = if i <= 0.25 {
            let t = i / 0.25;
            lerp(t, 30.0, 20.0)
        } else if i <= 0.5 {
            let t = (i - 0.25) / 0.25;
            lerp(t, 20.0, 10.0)
        } else if i <= 0.75 {
            let t = (i - 0.5) / 0.25;
            lerp(t, 10.0, 5.0)
        } else {
            let t = (i - 0.75) / 0.25;
            lerp(t, 5.0, 0.0)
        };

        let mkup_db = if i <= 0.25 {
            let t = i / 0.25;
            lerp(t, 1.5, 3.0)
        } else if i <= 0.5 {
            let t = (i - 0.25) / 0.25;
            lerp(t, 3.0, 5.0)
        } else if i <= 0.75 {
            let t = (i - 0.5) / 0.25;
            lerp(t, 5.0, 7.0)
        } else {
            let t = (i - 0.75) / 0.25;
            lerp(t, 7.0, 9.0)
        };

        self.intensity_ratio = ratio;
        self.intensity_attack = att_ms / 1000.0;
        self.intensity_release = rel_ms / 1000.0;
        self.knee_width = knee_db;
        self.makeup_gain = 10.0_f32.powf(mkup_db / 20.0);
    }

    /// Procesa un frame aplicando compresión dinámica premium (RMS + soft knee + lookahead).
    pub fn process(&mut self, frame: &mut [f64]) {
        if !self.enabled {
            return;
        }

        // Push current frame into lookahead buffer
        self.lookahead.push_back(frame.to_vec());
        while self.lookahead.len() > self.lookahead_samples {
            self.lookahead.pop_front();
        }

        // Buffer not full yet — no output
        if self.lookahead.len() < self.lookahead_samples {
            return;
        }

        // Compute RMS of lookahead for envelope follower (stereo-linked)
        let total_samples: usize = self.lookahead.iter().map(|f| f.len()).sum();
        let rms_sq = if total_samples > 0 {
            let sum_sq: f64 = self
                .lookahead
                .iter()
                .flatten()
                .filter(|s| s.is_finite())
                .map(|s| s * s)
                .sum();
            if total_samples > 0 {
                sum_sq / total_samples as f64
            } else {
                0.0
            }
        } else {
            0.0
        };
        let rms = rms_sq.sqrt().max(1e-10);

        // Find future peak in lookahead
        let future_peak = self
            .lookahead
            .iter()
            .flatten()
            .map(|s| if s.is_finite() { s.abs() } else { 0.0 })
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let drive = future_peak.max(rms);

        // Envelope follower
        let attack_coeff =
            (-1.0_f64 / ((self.intensity_attack as f64) * (self.sample_rate as f64))).exp();
        let release_coeff =
            (-1.0_f64 / ((self.intensity_release as f64) * (self.sample_rate as f64))).exp();

        if drive > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * drive;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * drive;
        }

        // Envelope to dB
        let env_db = if self.envelope > 1e-6_f64 {
            20.0_f64 * self.envelope.log10()
        } else {
            -96.0_f64
        };

        // Soft knee gain reduction
        let threshold = self.threshold as f64;
        let ratio = self.intensity_ratio as f64;
        let knee_half = (self.knee_width as f64) * 0.5;
        let knee_low = threshold - knee_half;
        let knee_high = threshold + knee_half;

        let gain_reduction_db = if env_db > knee_high {
            (threshold - env_db) * (1.0_f64 - 1.0_f64 / ratio)
        } else if env_db > knee_low {
            let t = (env_db - knee_low) / self.knee_width as f64;
            let effective_ratio = 1.0_f64 + (ratio - 1.0_f64) * t * t;
            let effective_threshold = knee_low + self.knee_width as f64 * t * t * 0.5;
            (effective_threshold - env_db) * (1.0_f64 - 1.0_f64 / effective_ratio)
        } else {
            0.0
        };

        let gain = 10.0_f64.powf(gain_reduction_db / 20.0);
        let makeup = self.makeup_gain as f64;
        let makeup_active = if gain < 1.0 {
            1.0 + (makeup - 1.0) * (1.0 - gain).sqrt()
        } else {
            1.0
        };

        // Apply to front frame
        if let Some(front_frame) = self.lookahead.front_mut() {
            for s in front_frame.iter_mut() {
                *s *= gain * makeup_active;
                if !s.is_finite() {
                    *s = 0.0;
                }
            }
            // Copy front frame to output
            let n = frame.len().min(front_frame.len());
            frame[..n].copy_from_slice(&front_frame[..n]);
        }

        self.lookahead.pop_front();
    }

    /// Resetea la envolvente y el buffer de lookahead.
    pub fn reset_state(&mut self) {
        self.envelope = 0.0;
        self.envelope_rms = 0.0;
        self.lookahead.clear();
    }
}

// --- Biquad General Filter ---

/// Tipo de filtro biquad disponible en el DSP.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub enum BiquadFilterType {
    Peak,
    LowShelf,
    HighShelf,
    AllPass,
    LowPass,
    HighPass,
}

/// Filtro biquad genérico configurable (peak, low shelf, high shelf, etc.).
#[derive(Clone)]
pub struct BiquadFilter {
    filter_type: BiquadFilterType,
    freq: f32,
    pub gain: f32, // dB
    q: f32,
    a1: f64,
    a2: f64,
    b0: f64,
    b1: f64,
    b2: f64,
    states: Vec<BiquadState>,
    sample_rate: f32,
}

impl BiquadFilter {
    /// Crea un nuevo filtro biquad del tipo, frecuencia, ganancia y Q especificados.
    pub fn new(filter_type: BiquadFilterType, freq: f32, gain: f32, q: f32) -> Self {
        let mut filter = Self {
            filter_type,
            freq,
            gain,
            q,
            a1: 0.0,
            a2: 0.0,
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            states: vec![BiquadState::default(); 8],
            sample_rate: 44100.0,
        };
        filter.update_coefficients(44100.0);
        filter
    }

    /// Configura los parámetros del filtro y actualiza los coeficientes.
    pub fn set_params(&mut self, freq: f32, gain: f32, q: f32) {
        self.freq = freq;
        self.gain = gain;
        self.q = q;
        self.update_coefficients(self.sample_rate);
    }

    /// Establece la frecuencia de muestreo y actualiza los coeficientes.
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        self.update_coefficients(rate);
    }

    /// Redimensiona los estados internos para el número de canales dado.
    pub fn resize_channels(&mut self, channels: usize) {
        self.states.resize(channels, BiquadState::default());
    }

    /// Resetea el estado interno del filtro.
    pub fn reset_state(&mut self) {
        for state in &mut self.states {
            state.reset();
        }
    }

    /// Recalcula los coeficientes del filtro para el sample rate dado.
    pub fn update_coefficients(&mut self, sample_rate: f32) {
        // Cálculos de coeficientes en f64 para máxima precisión
        let w0 = 2.0 * std::f64::consts::PI * (self.freq as f64) / (sample_rate as f64);
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0f64.powf((self.gain as f64) / 40.0);
        let alpha = sin_w0 / (2.0 * (self.q as f64));

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            BiquadFilterType::Peak => (
                1.0 + alpha * a,
                -2.0 * cos_w0,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos_w0,
                1.0 - alpha / a,
            ),
            BiquadFilterType::LowShelf => {
                let sqa = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 + sqa),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0),
                    a * ((a + 1.0) - (a - 1.0) * cos_w0 - sqa),
                    (a + 1.0) + (a - 1.0) * cos_w0 + sqa,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0),
                    (a + 1.0) + (a - 1.0) * cos_w0 - sqa,
                )
            }
            BiquadFilterType::HighShelf => {
                let sqa = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 + sqa),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0),
                    a * ((a + 1.0) + (a - 1.0) * cos_w0 - sqa),
                    (a + 1.0) - (a - 1.0) * cos_w0 + sqa,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos_w0),
                    (a + 1.0) - (a - 1.0) * cos_w0 - sqa,
                )
            }
            BiquadFilterType::AllPass => (
                1.0 - alpha,
                -2.0 * cos_w0,
                1.0 + alpha,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            BiquadFilterType::LowPass => (
                (1.0 - cos_w0) / 2.0,
                1.0 - cos_w0,
                (1.0 - cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
            BiquadFilterType::HighPass => (
                (1.0 + cos_w0) / 2.0,
                -(1.0 + cos_w0),
                (1.0 + cos_w0) / 2.0,
                1.0 + alpha,
                -2.0 * cos_w0,
                1.0 - alpha,
            ),
        };

        if a0 > 0.0 {
            self.b0 = b0 / a0;
            self.b1 = b1 / a0;
            self.b2 = b2 / a0;
            self.a1 = a1 / a0;
            self.a2 = a2 / a0;
        }
    }

    /// Procesa un frame completo a través del filtro biquad con SIMD en x86_64.
    pub fn process_frame(&mut self, frame: &mut [f64]) {
        match self.filter_type {
            BiquadFilterType::Peak | BiquadFilterType::LowShelf | BiquadFilterType::HighShelf => {
                if self.gain.abs() < 0.01 {
                    return;
                }
            }
            _ => {}
        }

        let b0 = self.b0;
        let b1 = self.b1;
        let b2 = self.b2;
        let a1 = self.a1;
        let a2 = self.a2;

        #[cfg(target_arch = "x86_64")]
        {
            let max_ch = frame.len().min(self.states.len());
            let mut i = 0;
            while i + 4 <= max_ch {
                let s_ptr = self.states.as_mut_ptr();
                let s0 = unsafe { &mut *s_ptr.add(i) };
                let s1 = unsafe { &mut *s_ptr.add(i + 1) };
                let s2 = unsafe { &mut *s_ptr.add(i + 2) };
                let s3 = unsafe { &mut *s_ptr.add(i + 3) };

                let x = wide::f64x4::new([frame[i], frame[i + 1], frame[i + 2], frame[i + 3]]);
                let xs1 = wide::f64x4::new([s0.x1, s1.x1, s2.x1, s3.x1]);
                let xs2 = wide::f64x4::new([s0.x2, s1.x2, s2.x2, s3.x2]);
                let ys1 = wide::f64x4::new([s0.y1, s1.y1, s2.y1, s3.y1]);
                let ys2 = wide::f64x4::new([s0.y2, s1.y2, s2.y2, s3.y2]);

                let out = wide::f64x4::splat(b0) * x
                    + wide::f64x4::splat(b1) * xs1
                    + wide::f64x4::splat(b2) * xs2
                    - wide::f64x4::splat(a1) * ys1
                    - wide::f64x4::splat(a2) * ys2;
                let out = out.blend(
                    wide::f64x4::splat(0.0),
                    out.abs().cmp_lt(wide::f64x4::splat(1e-20)),
                );
                let arr = out.to_array();
                frame[i] = arr[0];
                frame[i + 1] = arr[1];
                frame[i + 2] = arr[2];
                frame[i + 3] = arr[3];

                s0.x2 = s0.x1;
                s0.x1 = frame[i];
                s0.y2 = s0.y1;
                s0.y1 = frame[i];
                s1.x2 = s1.x1;
                s1.x1 = frame[i + 1];
                s1.y2 = s1.y1;
                s1.y1 = frame[i + 1];
                s2.x2 = s2.x1;
                s2.x1 = frame[i + 2];
                s2.y2 = s2.y1;
                s2.y1 = frame[i + 2];
                s3.x2 = s3.x1;
                s3.x1 = frame[i + 3];
                s3.y2 = s3.y1;
                s3.y1 = frame[i + 3];

                i += 4;
            }
            while i < max_ch {
                let s = &mut self.states[i];
                let x = frame[i];
                let y = b0 * x + b1 * s.x1 + b2 * s.x2 - a1 * s.y1 - a2 * s.y2;
                frame[i] = if y.abs() < 1e-20 { 0.0 } else { y };
                s.x2 = s.x1;
                s.x1 = x;
                s.y2 = s.y1;
                s.y1 = frame[i];
                i += 1;
            }
        }

        #[cfg(not(target_arch = "x86_64"))]
        {
            for ch in 0..frame.len().min(self.states.len()) {
                let s = &mut self.states[ch];
                let x = frame[ch];
                let y = b0 * x + b1 * s.x1 + b2 * s.x2 - a1 * s.y1 - a2 * s.y2;
                frame[ch] = if y.abs() < 1e-20 { 0.0 } else { y };
                s.x2 = s.x1;
                s.x1 = x;
                s.y2 = s.y1;
                s.y1 = frame[ch];
            }
        }
    }
}

// --- New Audio Effects Definitions ---

/// Filtro Low-Shelf para realzar o atenuar frecuencias sub-graves.
#[derive(Clone)]
pub struct SubBass {
    pub enabled: bool,
    pub freq: f32,
    pub gain: f32,
    filter: BiquadFilter,
}
impl Default for SubBass {
    fn default() -> Self {
        Self {
            enabled: false,
            freq: 45.0,
            gain: 0.0,
            filter: BiquadFilter::new(BiquadFilterType::LowShelf, 45.0, 0.0, 1.0),
        }
    }
}
impl SubBass {
    /// Establece la frecuencia de muestreo del filtro sub-bass.
    /// Establece la frecuencia de muestreo del filtro sub-bass.
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.filter.set_sample_rate(rate);
    }
    /// Redimensiona los canales del filtro sub-bass.
    pub fn resize_channels(&mut self, ch: usize) {
        self.filter.resize_channels(ch);
    }
    /// Procesa un frame aplicando el filtro sub-bass.
    pub fn process(&mut self, frame: &mut [f64]) {
        if self.filter.gain != self.gain {
            self.filter.set_params(self.freq, self.gain, 1.0);
        }
        self.filter.process_frame(frame);
    }
    /// Resetea el estado interno del filtro sub-bass.
    pub fn reset_state(&mut self) {
        self.filter.reset_state();
    }
}

/// Filtro Peak para realzar o atenuar frecuencias medias-graves (~100 Hz).
#[derive(Clone)]
pub struct MidBass {
    pub enabled: bool,
    pub freq: f32,
    pub gain: f32,
    filter: BiquadFilter,
}
impl Default for MidBass {
    fn default() -> Self {
        Self {
            enabled: false,
            freq: 100.0,
            gain: 0.0,
            filter: BiquadFilter::new(BiquadFilterType::Peak, 100.0, 0.0, 0.8),
        }
    }
}
impl MidBass {
    /// Establece la frecuencia de muestreo del filtro mid-bass.
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.filter.set_sample_rate(rate);
    }
    /// Redimensiona los canales del filtro mid-bass.
    pub fn resize_channels(&mut self, ch: usize) {
        self.filter.resize_channels(ch);
    }
    /// Procesa un frame aplicando el filtro mid-bass.
    pub fn process(&mut self, frame: &mut [f64]) {
        if self.filter.gain != self.gain {
            self.filter.set_params(self.freq, self.gain, 0.8);
        }
        self.filter.process_frame(frame);
    }
    /// Resetea el estado interno del filtro mid-bass.
    pub fn reset_state(&mut self) {
        self.filter.reset_state();
    }
}

/// Refuerzo de frecuencias vocales con dos filtros peak (1.5 kHz y 3 kHz).
#[derive(Clone)]
pub struct VoiceBoost {
    pub enabled: bool,
    pub gain: f32,
    filter1: BiquadFilter,
    filter2: BiquadFilter,
}
impl Default for VoiceBoost {
    fn default() -> Self {
        Self {
            enabled: false,
            gain: 0.0,
            filter1: BiquadFilter::new(BiquadFilterType::Peak, 1500.0, 0.0, 0.8),
            filter2: BiquadFilter::new(BiquadFilterType::Peak, 3000.0, 0.0, 0.8),
        }
    }
}
impl VoiceBoost {
    /// Establece la frecuencia de muestreo de los filtros vocales.
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.filter1.set_sample_rate(rate);
        self.filter2.set_sample_rate(rate);
    }
    /// Redimensiona los canales de los filtros vocales.
    pub fn resize_channels(&mut self, ch: usize) {
        self.filter1.resize_channels(ch);
        self.filter2.resize_channels(ch);
    }
    /// Procesa un frame aplicando el refuerzo vocal.
    pub fn process(&mut self, frame: &mut [f64]) {
        if self.filter1.gain != (self.gain * 0.6) {
            self.filter1.set_params(1500.0, self.gain * 0.6, 0.8);
            self.filter2.set_params(3000.0, self.gain * 0.4, 0.8);
        }
        self.filter1.process_frame(frame);
        self.filter2.process_frame(frame);
    }
    /// Resetea el estado interno de los filtros vocales.
    pub fn reset_state(&mut self) {
        self.filter1.reset_state();
        self.filter2.reset_state();
    }
}

/// Modo de operación del expansor estéreo multicapa.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpanderMode {
    Hybrid,
    Surround,
}

#[derive(Clone)]
pub struct StereoExpanderHybrid {
    // Crossover 1: 250Hz (Low vs MidHigh)
    low_lp1: BiquadFilter,
    low_lp2: BiquadFilter,
    low_hp1: BiquadFilter,
    low_hp2: BiquadFilter,
    // Crossover 2: 5000Hz (Mid vs High)
    mid_lp1: BiquadFilter,
    mid_lp2: BiquadFilter,
    mid_hp1: BiquadFilter,
    mid_hp2: BiquadFilter,
    // Air Boost for Side Channel
    high_shelf_side: BiquadFilter,
    // Decorrelators for High Side channel (Natural texture)
    side_ap1: AllPassFilter,
    side_ap2: AllPassFilter,
    sample_rate: f32,
}

impl StereoExpanderHybrid {
    pub fn new() -> Self {
        let sr = 44100.0;
        let q = 0.7071; // Butterworth Q for LR4 stages
        Self {
            low_lp1: BiquadFilter::new(BiquadFilterType::LowPass, 250.0, 0.0, q),
            low_lp2: BiquadFilter::new(BiquadFilterType::LowPass, 250.0, 0.0, q),
            low_hp1: BiquadFilter::new(BiquadFilterType::HighPass, 250.0, 0.0, q),
            low_hp2: BiquadFilter::new(BiquadFilterType::HighPass, 250.0, 0.0, q),
            mid_lp1: BiquadFilter::new(BiquadFilterType::LowPass, 5000.0, 0.0, q),
            mid_lp2: BiquadFilter::new(BiquadFilterType::LowPass, 5000.0, 0.0, q),
            mid_hp1: BiquadFilter::new(BiquadFilterType::HighPass, 5000.0, 0.0, q),
            mid_hp2: BiquadFilter::new(BiquadFilterType::HighPass, 5000.0, 0.0, q),
            high_shelf_side: BiquadFilter::new(BiquadFilterType::HighShelf, 8000.0, 0.0, 0.5), // Frecuencia más alta para "Aire" puro
            side_ap1: AllPassFilter::new(225), // Delay corto para textura
            side_ap2: AllPassFilter::new(556), // Delay medio para profundidad
            sample_rate: sr,
        }
    }
}

#[derive(Clone)]
pub struct StereoExpanderSurround {
    // Crossover 1: 250Hz (Low vs MidHigh)
    low_lp1: BiquadFilter,
    low_lp2: BiquadFilter,
    low_hp1: BiquadFilter,
    low_hp2: BiquadFilter,
    // Crossover 2: 5000Hz (Mid vs High)
    mid_lp1: BiquadFilter,
    mid_lp2: BiquadFilter,
    mid_hp1: BiquadFilter,
    mid_hp2: BiquadFilter,
    // Air Boost for Side Channel
    high_shelf_side: BiquadFilter,
    // Decorrelators for Mid Side channel (Enveloping body)
    mid_ap1: AllPassFilter,
    mid_ap2: AllPassFilter,
    // Decorrelators for High Side channel (Natural texture/Air)
    side_ap1: AllPassFilter,
    side_ap2: AllPassFilter,
    side_ap3: AllPassFilter,
    sample_rate: f32,
}

impl StereoExpanderSurround {
    pub fn new() -> Self {
        let sr = 44100.0;
        let q = 0.7071;
        Self {
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

/// Expansor estéreo multicapa con modos Hybrid y Surround.
///
/// Separa el audio en bandas de frecuencia (low, mid, high) y aplica
/// expansión estéreo independiente con decorrelación de fase y air boost.
#[derive(Clone)]
pub struct MultiBandStereoExpander {
    pub enabled: bool,
    pub width: f32,
    pub mode: ExpanderMode,
    hybrid: StereoExpanderHybrid,
    surround: StereoExpanderSurround,
}

impl Default for MultiBandStereoExpander {
    fn default() -> Self {
        Self {
            enabled: false,
            width: 1.0,
            mode: ExpanderMode::Hybrid,
            hybrid: StereoExpanderHybrid::new(),
            surround: StereoExpanderSurround::new(),
        }
    }
}

impl MultiBandStereoExpander {
    /// Establece la frecuencia de muestreo y reconfigura los filtros internos.
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.hybrid.sample_rate = rate;
        self.hybrid.low_lp1.set_sample_rate(rate);
        self.hybrid.low_lp2.set_sample_rate(rate);
        self.hybrid.low_hp1.set_sample_rate(rate);
        self.hybrid.low_hp2.set_sample_rate(rate);
        self.hybrid.mid_lp1.set_sample_rate(rate);
        self.hybrid.mid_lp2.set_sample_rate(rate);
        self.hybrid.mid_hp1.set_sample_rate(rate);
        self.hybrid.mid_hp2.set_sample_rate(rate);
        self.hybrid.high_shelf_side.set_sample_rate(rate);
        // Los AllPass ya se ajustan internamente o tienen tamaños fijos pequeños para textura
        self.surround.sample_rate = rate;
        self.surround.low_lp1.set_sample_rate(rate);
        self.surround.low_lp2.set_sample_rate(rate);
        self.surround.low_hp1.set_sample_rate(rate);
        self.surround.low_hp2.set_sample_rate(rate);
        self.surround.mid_lp1.set_sample_rate(rate);
        self.surround.mid_lp2.set_sample_rate(rate);
        self.surround.mid_hp1.set_sample_rate(rate);
        self.surround.mid_hp2.set_sample_rate(rate);
        self.surround.high_shelf_side.set_sample_rate(rate);
    }

    /// Procesa un frame estéreo aplicando expansión multicapa.
    pub fn process(&mut self, frame: &mut [f64]) {
        if frame.len() != 2 || self.width == 1.0 {
            return;
        }

        match self.mode {
            ExpanderMode::Hybrid => self.process_hybrid(frame),
            ExpanderMode::Surround => self.process_surround(frame),
        }
    }

    fn process_hybrid(&mut self, frame: &mut [f64]) {
        let mut low_band = [frame[0], frame[1]];
        let mut mid_high_band = [frame[0], frame[1]];

        // 1. Separar Low (< 250Hz)
        self.hybrid.low_lp1.process_frame(&mut low_band);
        self.hybrid.low_lp2.process_frame(&mut low_band);
        self.hybrid.low_hp1.process_frame(&mut mid_high_band);
        self.hybrid.low_hp2.process_frame(&mut mid_high_band);

        // El Low se fuerza a Mono para mantener el punch
        // Compensamos con un ligero boost (+1.0dB) para recuperar presencia
        let low_mono = (low_band[0] + low_band[1]) * 0.5 * 1.0_f64;
        low_band[0] = low_mono;
        low_band[1] = low_mono;

        // 2. Separar Mid (250Hz - 5kHz) y High (> 5kHz)
        let mut mid_band = [mid_high_band[0], mid_high_band[1]];
        let mut high_band = [mid_high_band[0], mid_high_band[1]];

        self.hybrid.mid_lp1.process_frame(&mut mid_band);
        self.hybrid.mid_lp2.process_frame(&mut mid_band);
        self.hybrid.mid_hp1.process_frame(&mut high_band);
        self.hybrid.mid_hp2.process_frame(&mut high_band);

        // 3. Procesar Mid Band (Expansión moderada)
        self.expand_band(&mut mid_band, self.width as f64);

        // 4. Procesar High Band (Expansión suave + Air Boost)
        let high_width = (self.width as f64 - 1.0) * 0.8 + 1.0; // Reducida agresividad para mayor naturalidad
        self.expand_band(&mut high_band, high_width);

        // Aplicar Air Boost y Decorrelación solo al canal Side de la banda alta
        let h_mid = (high_band[0] + high_band[1]) * 0.5;
        let mut h_side_val = (high_band[0] - high_band[1]) * 0.5;

        // 1. Decorrelación de fase (textura orgánica)
        h_side_val = self.hybrid.side_ap1.process(h_side_val);
        h_side_val = self.hybrid.side_ap2.process(h_side_val);

        let mut h_side_buf = [h_side_val, 0.0];

        let air_gain = ((self.width - 1.0) * 1.5).clamp(0.0, 2.0); // 2db de air boost (Refinado)
        if air_gain > 0.0 {
            // Solo actualizamos coeficientes si la ganancia cambió, para evitar zumbidos (Zipper Noise)
            if (self.hybrid.high_shelf_side.gain - air_gain).abs() > 0.001 {
                self.hybrid.high_shelf_side.gain = air_gain;
                self.hybrid
                    .high_shelf_side
                    .update_coefficients(self.hybrid.sample_rate);
            }
            self.hybrid.high_shelf_side.process_frame(&mut h_side_buf);
        }

        high_band[0] = h_mid + h_side_buf[0];
        high_band[1] = h_mid - h_side_buf[0];

        frame[0] = low_band[0] + mid_band[0] + high_band[0] * 0.90;
        frame[1] = low_band[1] + mid_band[1] + high_band[1] * 0.90;
    }

    fn process_surround(&mut self, frame: &mut [f64]) {
        let mut low_band = [frame[0], frame[1]];
        let mut mid_high_band = [frame[0], frame[1]];

        self.surround.low_lp1.process_frame(&mut low_band);
        self.surround.low_lp2.process_frame(&mut low_band);
        self.surround.low_hp1.process_frame(&mut mid_high_band);
        self.surround.low_hp2.process_frame(&mut mid_high_band);

        let low_mono = (low_band[0] + low_band[1]) * 0.5 * 1.0_f64;
        low_band[0] = low_mono;
        low_band[1] = low_mono;

        let mut mid_band = [mid_high_band[0], mid_high_band[1]];
        let mut high_band = [mid_high_band[0], mid_high_band[1]];

        self.surround.mid_lp1.process_frame(&mut mid_band);
        self.surround.mid_lp2.process_frame(&mut mid_band);
        self.surround.mid_hp1.process_frame(&mut high_band);
        self.surround.mid_hp2.process_frame(&mut high_band);

        self.expand_band(&mut mid_band, self.width as f64);

        let m_mid = (mid_band[0] + mid_band[1]) * 0.5;
        let mut m_side = (mid_band[0] - mid_band[1]) * 0.5;
        m_side = self.surround.mid_ap1.process(m_side);
        m_side = self.surround.mid_ap2.process(m_side);
        mid_band[0] = m_mid + m_side;
        mid_band[1] = m_mid - m_side;

        let high_width = (self.width as f64 - 1.0) * 0.8 + 1.0;
        self.expand_band(&mut high_band, high_width);

        let h_mid = (high_band[0] + high_band[1]) * 0.5;
        let mut h_side_val = (high_band[0] - high_band[1]) * 0.5;

        h_side_val = self.surround.side_ap1.process(h_side_val);
        h_side_val = self.surround.side_ap2.process(h_side_val);
        h_side_val = self.surround.side_ap3.process(h_side_val);

        let mut h_side_buf = [h_side_val, 0.0];

        let air_gain = ((self.width - 1.0) * 1.5).clamp(0.0, 2.0);
        if air_gain > 0.0 {
            // Solo actualizamos coeficientes si la ganancia cambió, para evitar zumbidos (Zipper Noise)
            if (self.surround.high_shelf_side.gain - air_gain).abs() > 0.001 {
                self.surround.high_shelf_side.gain = air_gain;
                self.surround
                    .high_shelf_side
                    .update_coefficients(self.surround.sample_rate);
            }
            self.surround.high_shelf_side.process_frame(&mut h_side_buf);
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

    /// Resetea el estado interno de todos los filtros del expansor.
    pub fn reset_state(&mut self) {
        self.hybrid.low_lp1.reset_state();
        self.hybrid.low_lp2.reset_state();
        self.hybrid.low_hp1.reset_state();
        self.hybrid.low_hp2.reset_state();
        self.hybrid.mid_lp1.reset_state();
        self.hybrid.mid_lp2.reset_state();
        self.hybrid.mid_hp1.reset_state();
        self.hybrid.mid_hp2.reset_state();
        self.hybrid.high_shelf_side.reset_state();
        self.hybrid.side_ap1.reset();
        self.hybrid.side_ap2.reset();

        self.surround.low_lp1.reset_state();
        self.surround.low_lp2.reset_state();
        self.surround.low_hp1.reset_state();
        self.surround.low_hp2.reset_state();
        self.surround.mid_lp1.reset_state();
        self.surround.mid_lp2.reset_state();
        self.surround.mid_hp1.reset_state();
        self.surround.mid_hp2.reset_state();
        self.surround.high_shelf_side.reset_state();
        self.surround.mid_ap1.reset();
        self.surround.mid_ap2.reset();
        self.surround.side_ap1.reset();
        self.surround.side_ap2.reset();
        self.surround.side_ap3.reset();
    }
}

/// Puerta de ruido con seguidor de envolvente y atenuación suave.
#[derive(Clone)]
pub struct NoiseGate {
    pub enabled: bool,
    pub threshold: f32,
    pub attack: f32,
    pub release: f32,
    envelope: f64,
    sample_rate: f32,
}
impl Default for NoiseGate {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: -60.0,
            attack: 0.005,
            release: 0.1,
            envelope: 0.0,
            sample_rate: 44100.0,
        }
    }
}
impl NoiseGate {
    /// Establece la frecuencia de muestreo para el seguidor de envolvente.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }
    pub fn process(&mut self, frame: &mut [f64]) {
        let max_abs = frame
            .iter()
            .map(|s| s.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0_f64);
        let attack_coeff = (-1.0_f64 / ((self.attack as f64) * (self.sample_rate as f64))).exp();
        let release_coeff = (-1.0_f64 / ((self.release as f64) * (self.sample_rate as f64))).exp();

        if max_abs > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * max_abs;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * max_abs;
        }

        let env_db = if self.envelope > 1e-6_f64 {
            20.0_f64 * self.envelope.log10()
        } else {
            -96.0_f64
        };
        if env_db < (self.threshold as f64) {
            // Apply a smooth attenuation when under threshold (simple soft mute)
            let att = 10.0f64
                .powf((env_db - (self.threshold as f64)) / 20.0)
                .max(0.0001);
            for s in frame.iter_mut() {
                *s *= att;
            }
        }
    }
    pub fn reset_state(&mut self) {
        self.envelope = 0.0;
    }
}
/// FIR half-band 32-tap coefficients for 4x oversampling (Kaiser window β=6).
const FIR_HALFBAND_COEFFS: [f64; 32] = [
    0.0, -0.0013, 0.0, 0.0034, 0.0, -0.0076, 0.0, 0.0147, 0.0, -0.0264, 0.0, 0.0457, 0.0, -0.0807,
    0.0, 0.1589, 0.5, 0.1589, 0.0, -0.0807, 0.0, 0.0457, 0.0, -0.0264, 0.0, 0.0147, 0.0, -0.0076,
    0.0, 0.0034, 0.0, -0.0013,
];

/// Limitador premium con lookahead 2ms, oversampling 4x y release adaptativo.
#[derive(Clone)]
pub struct Limiter {
    pub enabled: bool,
    pub ceiling: f32,
    envelope: f64,
    sample_rate: f32,
    lookahead: VecDeque<Vec<f64>>,
    pub lookahead_samples: usize,
    oversample_buffer: Vec<f64>,
    rms_state: f64,
    pub crest_factor_smooth: f64,
    release_base: f32,
}

impl Default for Limiter {
    fn default() -> Self {
        Self {
            enabled: false,
            ceiling: -1.0,
            envelope: 0.0,
            sample_rate: 44100.0,
            lookahead: VecDeque::with_capacity(89),
            lookahead_samples: 88,
            oversample_buffer: Vec::with_capacity(512 * 4),
            rms_state: 0.0,
            crest_factor_smooth: 1.0,
            release_base: 0.05,
        }
    }
}

#[allow(dead_code)]
impl Limiter {
    /// Establece la frecuencia de muestreo.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.lookahead_samples = (0.002 * sample_rate as f64).round() as usize;
        self.lookahead = VecDeque::with_capacity(self.lookahead_samples + 1);
    }

    /// Sobremuestrea 4x (inserción de ceros + FIR half-band).
    fn upsample_4x(&mut self, frame: &[f64]) {
        let n = frame.len();
        let up_len = n * 4;
        self.oversample_buffer.resize(up_len, 0.0);

        // Zero-insert: every 4th sample is original, rest are 0
        for (i, sample) in frame.iter().enumerate() {
            self.oversample_buffer[i * 4] = *sample;
        }

        // Apply half-band FIR filter
        let mut filtered = vec![0.0_f64; up_len];
        for out_idx in 0..up_len {
            let mut sum = 0.0_f64;
            for k in 0..32 {
                let in_idx = out_idx as isize + k as isize - 16;
                if in_idx >= 0 && in_idx < up_len as isize {
                    sum += self.oversample_buffer[in_idx as usize] * FIR_HALFBAND_COEFFS[k];
                }
            }
            filtered[out_idx] = sum * 4.0; // Compensate zero-insertion energy loss
        }
        self.oversample_buffer = filtered;
    }

    /// Diezma 4x (toma cada 4ª muestra del buffer sobremuestreado).
    fn downsample_4x(&self, upsampled: &[f64], frame: &mut [f64]) {
        let n = frame.len();
        for i in 0..n {
            frame[i] = upsampled[i * 4];
        }
    }

    /// Procesa un frame con lookahead, oversampling true-peak y release adaptativo.
    pub fn process(&mut self, frame: &mut [f64]) {
        if !self.enabled {
            return;
        }

        self.lookahead.push_back(frame.to_vec());
        while self.lookahead.len() > self.lookahead_samples {
            self.lookahead.pop_front();
        }

        if self.lookahead.len() < self.lookahead_samples {
            return;
        }

        let oldest = self.lookahead.front().cloned().unwrap_or_default();
        let mut processed = oldest.clone();

        // True peak detection via zero-insert 4x
        let n = oldest.len();
        self.oversample_buffer.resize(n * 4, 0.0);
        for (i, sample) in oldest.iter().enumerate() {
            self.oversample_buffer[i * 4] = *sample;
        }
        let true_peak = self
            .oversample_buffer
            .iter()
            .filter(|s| s.is_finite())
            .map(|s| s.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // RMS of original frame for crest factor
        let rms_sq = oldest
            .iter()
            .filter(|s| s.is_finite())
            .map(|s| s * s)
            .sum::<f64>()
            / oldest.len().max(1) as f64;
        let rms = rms_sq.sqrt().max(1e-10);

        let crest = true_peak / rms;
        self.crest_factor_smooth = 0.9 * self.crest_factor_smooth + 0.1 * crest;
        let crest_norm = ((self.crest_factor_smooth - 1.0) / 19.0).clamp(0.0, 1.0);

        let release_secs = 0.1 - crest_norm * 0.09;

        // Envelope follower on original-rate (for true peak at original rate use max_abs)
        let max_abs = oldest
            .iter()
            .filter(|s| s.is_finite())
            .map(|s| s.abs())
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        let release_coeff = (-1.0_f64 / (release_secs * (self.sample_rate as f64))).exp();

        if max_abs > self.envelope {
            self.envelope = max_abs;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * max_abs;
        }

        let ceiling_lin = 10.0_f64.powf((self.ceiling as f64) / 20.0);
        if self.envelope > ceiling_lin {
            let attenuation = ceiling_lin / self.envelope;
            for s in processed.iter_mut() {
                *s *= attenuation;
            }
        }
        frame.copy_from_slice(&processed);

        self.lookahead.pop_front();

        for s in frame.iter_mut() {
            if !s.is_finite() {
                *s = 0.0;
            }
        }
    }

    pub fn reset_state(&mut self) {
        self.envelope = 0.0;
        self.rms_state = 0.0;
        self.crest_factor_smooth = 1.0;
        self.lookahead.clear();
        self.oversample_buffer.clear();
    }
}

/// Control de balance estéreo (panorama) entre canal izquierdo y derecho.
#[derive(Clone)]
pub struct StereoBalance {
    pub enabled: bool,
    pub balance: f32,
}
impl Default for StereoBalance {
    fn default() -> Self {
        Self {
            enabled: false,
            balance: 0.0,
        }
    }
}
impl StereoBalance {
    /// Procesa un frame estéreo ajustando el balance izquierda/derecha.
    pub fn process(&mut self, frame: &mut [f64]) {
        if frame.len() == 2 && self.balance != 0.0 {
            // balance -1.0 = left 100%, right 0%
            // balance 1.0 = right 100%, left 0%
            let gain_l = if self.balance > 0.0 {
                1.0_f64 - (self.balance as f64)
            } else {
                1.0_f64
            };
            let gain_r = if self.balance < 0.0 {
                1.0_f64 + (self.balance as f64)
            } else {
                1.0_f64
            };
            frame[0] *= gain_l;
            frame[1] *= gain_r;
        }
    }
}
