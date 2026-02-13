

pub struct DspChain {
    pub preamp_gain: f32, // Linear gain
    pub equalizer: Equalizer,
    pub reverb: Reverb,
    pub compressor: Compressor,
    pub enabled: bool,
}

impl Default for DspChain {
    fn default() -> Self {
        Self {
            preamp_gain: 1.0,
            equalizer: Equalizer::new(31), // Default 31 bands
            reverb: Reverb::new(),
            compressor: Compressor::new(),
            enabled: true,
        }
    }
}

impl DspChain {
    pub fn process(&mut self, sample: &mut f32) {
        if !self.enabled {
            return;
        }

        // Apply Preamp
        *sample *= self.preamp_gain;

        // Apply EQ
        self.equalizer.process(sample);
        
        // Apply Compressor
        self.compressor.process(sample);
        
        // Apply Reverb
        self.reverb.process(sample);
    }

    pub fn get_preamp_db(&self) -> f32 {
        if self.preamp_gain > 0.0 {
            20.0 * self.preamp_gain.log10()
        } else {
            -96.0
        }
    }

    pub fn set_preamp_db(&mut self, db: f32) {
        self.preamp_gain = 10.0f32.powf(db / 20.0);
    }
}

#[derive(Clone)]
pub struct Equalizer {
    pub bands: Vec<EqBand>,
    pub enabled: bool,
}

impl Equalizer {
    pub fn new(num_bands: usize) -> Self {
        let mut eq = Self {
            bands: Vec::new(),
            enabled: true,
        };
        eq.setup_bands(num_bands);
        eq
    }

    fn setup_bands(&mut self, num_bands: usize) {
        let freqs = match num_bands {
            10 => vec![31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0],
            15 => vec![25.0, 40.0, 63.0, 100.0, 160.0, 250.0, 400.0, 630.0, 1000.0, 1600.0, 2500.0, 4000.0, 6300.0, 10000.0, 16000.0],
            31 => vec![
                20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 
                1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0
            ],
            _ => vec![100.0, 1000.0, 10000.0], // Fallback
        };

        self.bands = freqs.into_iter().map(|f| EqBand::new(f)).collect();
    }

    pub fn process(&mut self, sample: &mut f32) {
        if !self.enabled {
            return;
        }
        for band in &mut self.bands {
            band.process(sample);
        }
    }
}

#[derive(Clone)]
pub struct EqBand {
    pub frequency: f32,
    pub gain: f32, // dB
    // Biquad state
    #[allow(dead_code)]
    a0: f32, a1: f32, a2: f32, b0: f32, b1: f32, b2: f32,
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl EqBand {
    pub fn new(freq: f32) -> Self {
        let mut band = Self {
            frequency: freq,
            gain: 0.0,
            a0: 1.0, a1: 0.0, a2: 0.0, b0: 1.0, b1: 0.0, b2: 0.0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
        };
        band.update_coefficients(44100.0); // Default sample rate, must be updated
        band
    }

    pub fn set_gain(&mut self, gain_db: f32, sample_rate: f32) {
        self.gain = gain_db;
        self.update_coefficients(sample_rate);
    }

    /* 
     * Peaking EQ Filter Design
     * Based on Robert Bristow-Johnson's Audio EQ Cookbook
     */
    pub fn update_coefficients(&mut self, sample_rate: f32) {
        let w0 = 2.0 * std::f32::consts::PI * self.frequency / sample_rate;
        let c = w0.cos();
        let s = w0.sin();
        let alpha = s / (2.0 * 1.0); // Q = 1.0 (Approx 1.4 bandwidth)
        
        let a = 10.0f32.powf(self.gain / 40.0); // A = 10^(dB/40)

        // Peaking EQ coeffs
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * c;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * c;
        let a2 = 1.0 - alpha / a;

        // Normalized
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    pub fn process(&mut self, sample: &mut f32) {
        let x = *sample;
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        
        // Denormal protection (simple)
        let y = if y.abs() < 1e-10 { 0.0 } else { y };
        
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        
        *sample = y;
    }
}
// --- Reverb (Freeverb implementation) ---

#[derive(Clone)]
struct DelayLine {
    buffer: Vec<f32>,
    index: usize,
}

impl DelayLine {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
        }
    }

    fn read(&self) -> f32 {
        self.buffer[self.index]
    }

    fn write(&mut self, value: f32) {
        self.buffer[self.index] = value;
        self.index = (self.index + 1) % self.buffer.len();
    }
}

#[derive(Clone)]
struct CombFilter {
    delay: DelayLine,
    feedback: f32,
    filter_state: f32,
    damp: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            delay: DelayLine::new(size),
            feedback: 0.5,
            filter_state: 0.0,
            damp: 0.5,
        }
    }

    fn set_feedback(&mut self, val: f32) {
        self.feedback = val;
    }

    fn set_damp(&mut self, val: f32) {
        self.damp = val;
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = self.delay.read();
        
        self.filter_state = output * (1.0 - self.damp) + self.filter_state * self.damp;
        
        let input_combined = input + self.filter_state * self.feedback;
        self.delay.write(input_combined);

        output
    }
}

#[derive(Clone)]
struct AllPassFilter {
    delay: DelayLine,
    feedback: f32,
}

impl AllPassFilter {
    fn new(size: usize) -> Self {
        Self {
            delay: DelayLine::new(size),
            feedback: 0.5,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let buffered_val = self.delay.read();
        let input_combined = input + buffered_val * self.feedback;
        self.delay.write(input_combined);
        
        buffered_val - input_combined // Standard AllPass formula
    }
}

#[derive(Clone)]
pub struct Reverb {
    combs: Vec<CombFilter>,
    allpasses: Vec<AllPassFilter>,
    pub enabled: bool,
    pub room_size: f32,
    pub damping: f32,
    #[allow(dead_code)]
    pub width: f32,
    pub wet: f32,
    pub dry: f32,
    gain: f32,
}

impl Reverb {
    pub fn new() -> Self {
        // Freeverb tunings (Stereo spread usually handled by offsetting sizes, here mono simplified for clarity then duplicated or offset)
        // Standard Schroeder/Moorer comb tunings
        let comb_tunings = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        let allpass_tunings = [556, 441, 341, 225];

        let combs = comb_tunings.iter().map(|&size| CombFilter::new(size)).collect();
        let allpasses = allpass_tunings.iter().map(|&size| AllPassFilter::new(size)).collect();

        let mut r = Self {
            combs,
            allpasses,
            enabled: false,
            room_size: 0.5,
            damping: 0.5,
            width: 1.0,
            wet: 0.3,
            dry: 0.8,
            gain: 0.015,
        };
        r.update_params();
        r
    }

    pub fn set_room_size(&mut self, value: f32) {
        self.room_size = value.clamp(0.0, 1.0);
        self.update_params();
    }

    pub fn set_damping(&mut self, value: f32) {
        self.damping = value.clamp(0.0, 1.0);
        self.update_params();
    }

    pub fn set_wet(&mut self, value: f32) {
        self.wet = value.clamp(0.0, 1.0);
    }

    pub fn set_dry(&mut self, value: f32) {
        self.dry = value.clamp(0.0, 1.0);
    }

    fn update_params(&mut self) {
        // Freeverb scale factors
        let feedback = self.room_size * 0.28 + 0.7;
        let damp = self.damping * 0.4;

        for comb in &mut self.combs {
            comb.set_feedback(feedback);
            comb.set_damp(damp);
        }
    }

    pub fn process(&mut self, sample: &mut f32) {
        if !self.enabled { return; }

        let input = *sample * self.gain;
        let mut out = 0.0;

        for comb in &mut self.combs {
            out += comb.process(input);
        }

        for allpass in &mut self.allpasses {
            out = allpass.process(out);
        }

        *sample = out * self.wet + *sample * self.dry;
    }
}

// --- Compressor ---

#[derive(Clone)]
pub struct Compressor {
    pub enabled: bool,
    pub threshold: f32, // dB
    pub ratio: f32,
    pub attack: f32, // secs
    pub release: f32, // secs
    
    // Internal state
    envelope: f32,
    sample_rate: f32,
}

impl Compressor {
    pub fn new() -> Self {
        Self {
            enabled: false,
            threshold: -10.0,
            ratio: 4.0,
            attack: 0.005,
            release: 0.1,
            envelope: 0.0,
            sample_rate: 44100.0,
        }
    }

    pub fn set_params(&mut self, threshold: f32, ratio: f32, attack: f32, release: f32) {
        self.threshold = threshold;
        self.ratio = ratio.max(1.0);
        self.attack = attack.max(0.001);
        self.release = release.max(0.001);
    }

    pub fn process(&mut self, sample: &mut f32) {
        if !self.enabled { return; }

        let input = *sample;
        let abs_input = input.abs();

        // Envelope follower (Simple AR)
        let attack_coeff = (-1.0 / (self.attack * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release * self.sample_rate)).exp();

        if abs_input > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * abs_input;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * abs_input;
        }

        // Gain reduction
        let env_db = if self.envelope > 1e-6 { 20.0 * self.envelope.log10() } else { -96.0 };
        
        if env_db > self.threshold {
            let gain_reduction_db = (self.threshold - env_db) * (1.0 - 1.0 / self.ratio);
            let gain = 10.0f32.powf(gain_reduction_db / 20.0);
            *sample *= gain;
        }
    }
}
