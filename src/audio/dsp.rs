

pub struct DspChain {
    pub preamp_gain: f32, // Linear gain
    pub equalizer: Equalizer,
    pub reverb: Reverb,
    pub compressor: Compressor,
    pub bass_boost: BassBoost,
    pub voice_boost: VoiceBoost,
    pub stereo_expander: StereoExpander,
    pub replay_gain: ReplayGain,
    pub noise_gate: NoiseGate,
    pub crossfeed: Crossfeed,
    pub limiter: Limiter,
    pub hrtf: HrtfSpatializer,
    pub delay: Delay,
    pub chorus: Chorus,
    pub flanger: Flanger,
    pub phaser: Phaser,
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
            bass_boost: BassBoost::default(),
            voice_boost: VoiceBoost::default(),
            stereo_expander: StereoExpander::default(),
            replay_gain: ReplayGain::default(),
            noise_gate: NoiseGate::default(),
            crossfeed: Crossfeed::default(),
            limiter: Limiter::default(),
            hrtf: HrtfSpatializer::default(),
            delay: Delay::default(),
            chorus: Chorus::default(),
            flanger: Flanger::default(),
            phaser: Phaser::default(),
            stereo_balance: StereoBalance::default(),
            enabled: true,
        }
    }
}

impl DspChain {
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if !self.enabled {
            return;
        }

        // 1. ReplayGain (Pre-amp)
        if self.replay_gain.enabled {
            let gain = 10.0f32.powf(self.replay_gain.preamp / 20.0);
            for s in frame.iter_mut() {
                *s *= gain;
            }
        }

        // Apply Preamp (Only if Equalizer module is active/enabled as per UI logic)
        if self.equalizer.enabled {
            for s in frame.iter_mut() {
                *s *= self.preamp_gain;
            }
            // Apply EQ
            for (ch_idx, sample) in frame.iter_mut().enumerate() {
                self.equalizer.process(sample, ch_idx);
            }
        }

        // Noise Gate
        if self.noise_gate.enabled {
            self.noise_gate.process(frame);
        }

        // Bass Boost
        if self.bass_boost.enabled {
            self.bass_boost.process(frame);
        }

        // Voice Boost
        if self.voice_boost.enabled {
            self.voice_boost.process(frame);
        }

        // Compressor
        if self.compressor.enabled {
            self.compressor.process(frame);
        }

        // Flanger
        if self.flanger.enabled {
            self.flanger.process(frame);
        }

        // Phaser
        if self.phaser.enabled {
            self.phaser.process(frame);
        }

        // Chorus
        if self.chorus.enabled {
            self.chorus.process(frame);
        }

        // Delay
        if self.delay.enabled {
            self.delay.process(frame);
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
            if self.crossfeed.enabled {
                self.crossfeed.process(frame);
            }
            if self.stereo_balance.enabled {
                self.stereo_balance.process(frame);
            }
            if self.hrtf.enabled {
                self.hrtf.process(frame);
            }
        }

        // Limiter (Always last)
        if self.limiter.enabled {
            self.limiter.process(frame);
        }
    }
    
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.equalizer.set_sample_rate(sample_rate);
        self.bass_boost.set_sample_rate(sample_rate);
        self.voice_boost.set_sample_rate(sample_rate);
        // FIXME: self.compressor does not have set_sample_rate on basic setup, keeping it simple for now
        self.delay.set_sample_rate(sample_rate);
        self.chorus.set_sample_rate(sample_rate);
        self.flanger.set_sample_rate(sample_rate);
        self.phaser.set_sample_rate(sample_rate);
    }

    pub fn set_channel_count(&mut self, channels: usize) {
        self.equalizer.set_channel_count(channels);
        self.bass_boost.resize_channels(channels);
        self.voice_boost.resize_channels(channels);
        self.delay.resize_channels(channels);
        self.chorus.resize_channels(channels);
        self.flanger.resize_channels(channels);
        self.phaser.resize_channels(channels);
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
    // Persistence
    saved_bands_20: Vec<EqBand>,
    saved_bands_31: Vec<EqBand>,
    last_sample_rate: f32,
    last_channel_count: usize,
}

impl Equalizer {
    pub fn new(num_bands: usize) -> Self {
        // Create both sets initially
        let bands_20 = Self::create_bands(20);
        let bands_31 = Self::create_bands(31);
        
        // Default to num_bands, or fallback to 31 if invalid
        let active = if num_bands == 20 { bands_20.clone() } else { bands_31.clone() };

        Self {
            bands: active,
            enabled: true,
            saved_bands_20: bands_20,
            saved_bands_31: bands_31,
            last_sample_rate: 44100.0, // Default
            last_channel_count: 2, // Default
        }
    }

    fn create_bands(num_bands: usize) -> Vec<EqBand> {
        let freqs = match num_bands {
            20 => vec![
                22.4, 31.5, 45.0, 63.0, 90.0, 125.0, 180.0, 250.0, 355.0, 500.0, 
                710.0, 1000.0, 1400.0, 2000.0, 2800.0, 4000.0, 5600.0, 8000.0, 11200.0, 16000.0
            ],
            31 => vec![
                20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 
                200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 
                2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0
            ],
            _ => vec![1000.0], // Should not happen for our UI
        };

        if num_bands == 20 {
             freqs.into_iter().map(|f| {
                 let mut b = EqBand::new(f);
                 b.set_q(2.87); 
                 b
             }).collect()
        } else {
             freqs.into_iter().map(|f| {
                 let mut b = EqBand::new(f);
                 b.set_q(4.4); 
                 b
             }).collect()
        }
    }
    
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
    pub fn set_channel_count(&mut self, channels: usize) {
        self.last_channel_count = channels;
        for band in &mut self.bands {
            band.resize_channels(channels);
        }
    }

    pub fn process(&mut self, sample: &mut f32, channel_idx: usize) {
        if !self.enabled {
            return;
        }
        for band in &mut self.bands {
            band.process(sample, channel_idx);
        }
    }
    
    pub fn reset_all(&mut self) {
        for band in &mut self.bands { band.set_gain(0.0); }
        for band in &mut self.saved_bands_20 { band.set_gain(0.0); }
        for band in &mut self.saved_bands_31 { band.set_gain(0.0); }
    }


}

#[derive(Clone, Default)]
struct BiquadState {
    x1: f32, x2: f32, y1: f32, y2: f32,
}

#[derive(Clone)]
pub struct EqBand {
    pub frequency: f32,
    pub gain: f32, // dB
    pub q: f32,
    // Biquad coefficients (shared across channels)
    #[allow(dead_code)]
    a0: f32, a1: f32, a2: f32, b0: f32, b1: f32, b2: f32,
    // State per channel
    states: Vec<BiquadState>,
    last_sample_rate: f32, // Store last SR to re-calculate if needed in set_gain/q
}

impl EqBand {
    pub fn new(freq: f32) -> Self {
        let mut band = Self {
            frequency: freq,
            gain: 0.0,
            q: 1.41, 
            a0: 1.0, a1: 0.0, a2: 0.0, b0: 1.0, b1: 0.0, b2: 0.0,
            states: vec![BiquadState::default(); 8], // Pre-alloc for 7.1/8 channels default
            last_sample_rate: 44100.0,
        };
        band.update_coefficients(44100.0);
        band
    }

    pub fn set_gain(&mut self, gain_db: f32) {
        self.gain = gain_db;
        self.update_coefficients(self.last_sample_rate);
    }
    
    // Add set_sample_rate aware setter if needed, or just update coeffs after.

    pub fn set_q(&mut self, q: f32) {
        self.q = q;
        self.update_coefficients(self.last_sample_rate);
    }

    pub fn resize_channels(&mut self, channels: usize) {
        if self.states.len() != channels {
            self.states.resize(channels, BiquadState::default());
        }
    }

    /* 
     * Peaking EQ Filter Design
     */
    pub fn update_coefficients(&mut self, sample_rate: f32) {
        self.last_sample_rate = sample_rate;
        if sample_rate <= 0.0 { return; }

        let w0 = 2.0 * std::f32::consts::PI * self.frequency / sample_rate;
        let c = w0.cos();
        let s = w0.sin();
        let alpha = s / (2.0 * self.q); 
        
        let a = 10.0f32.powf(self.gain / 40.0); // A = 10^(dB/40)

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
             self.b0 = 1.0; self.b1 = 0.0; self.b2 = 0.0;
             self.a1 = 0.0; self.a2 = 0.0;
        }
    }

    pub fn process(&mut self, sample: &mut f32, channel_idx: usize) {
        // Validate channel index
        if channel_idx >= self.states.len() {
             // Optionally resize? Or just return.
             // Resizing in audio thread is bad. We assume set_channel_count was called.
             // Fallback: if index out of bounds, do nothing (bypass).
             return;
        }

        let state = &mut self.states[channel_idx];
        
        let x = *sample;
        let y = self.b0 * x + self.b1 * state.x1 + self.b2 * state.x2 - self.a1 * state.y1 - self.a2 * state.y2;
        
        // Denormal protection (simple)
        let y = if y.abs() < 1e-10 { 0.0 } else { y };
        
        state.x2 = state.x1;
        state.x1 = x;
        state.y2 = state.y1;
        state.y1 = y;
        
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

    pub fn process(&mut self, frame: &mut [f32]) {
        if !self.enabled { return; }

        for sample in frame.iter_mut() {
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

    pub fn process(&mut self, frame: &mut [f32]) {
        if !self.enabled { return; }

        // Find max peak in the frame for a simple stereo-linked envelope detector
        let max_abs = frame.iter().map(|s| s.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);

        // Envelope follower (Simple AR)
        let attack_coeff = (-1.0 / (self.attack * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release * self.sample_rate)).exp();

        if max_abs > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * max_abs;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * max_abs;
        }

        // Gain reduction calculation
        let env_db = if self.envelope > 1e-6 { 20.0 * self.envelope.log10() } else { -96.0 };
        
        if env_db > self.threshold {
            let gain_reduction_db = (self.threshold - env_db) * (1.0 - 1.0 / self.ratio);
            let gain = 10.0f32.powf(gain_reduction_db / 20.0);
            
            for s in frame.iter_mut() {
                *s *= gain;
            }
        }
    }
}

// --- Biquad General Filter ---

#[derive(Clone, Copy, PartialEq)]
pub enum BiquadFilterType {
    Peak,
    LowShelf,
    HighShelf,
    AllPass,
}

#[derive(Clone)]
pub struct BiquadFilter {
    filter_type: BiquadFilterType,
    freq: f32,
    pub gain: f32, // dB
    q: f32,
    a1: f32, a2: f32, b0: f32, b1: f32, b2: f32,
    states: Vec<BiquadState>,
    sample_rate: f32,
}

impl BiquadFilter {
    pub fn new(filter_type: BiquadFilterType, freq: f32, gain: f32, q: f32) -> Self {
        let mut filter = Self {
            filter_type, freq, gain, q,
            a1: 0.0, a2: 0.0, b0: 1.0, b1: 0.0, b2: 0.0,
            states: vec![BiquadState::default(); 8],
            sample_rate: 44100.0,
        };
        filter.update_coefficients(44100.0);
        filter
    }

    pub fn set_params(&mut self, freq: f32, gain: f32, q: f32) {
        self.freq = freq;
        self.gain = gain;
        self.q = q;
        self.update_coefficients(self.sample_rate);
    }

    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        self.update_coefficients(rate);
    }
    
    pub fn resize_channels(&mut self, channels: usize) {
        self.states.resize(channels, BiquadState::default());
    }

    pub fn update_coefficients(&mut self, sample_rate: f32) {
        use std::f32::consts::PI;
        let w0 = 2.0 * PI * self.freq / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let a = 10.0f32.powf(self.gain / 40.0);
        let alpha = sin_w0 / (2.0 * self.q);

        let (b0, b1, b2, a0, a1, a2) = match self.filter_type {
            BiquadFilterType::Peak => {
                (
                    1.0 + alpha * a,
                    -2.0 * cos_w0,
                    1.0 - alpha * a,
                    1.0 + alpha / a,
                    -2.0 * cos_w0,
                    1.0 - alpha / a,
                )
            },
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
            },
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
            },
            BiquadFilterType::AllPass => {
                (
                    1.0 - alpha,
                    -2.0 * cos_w0,
                    1.0 + alpha,
                    1.0 + alpha,
                    -2.0 * cos_w0,
                    1.0 - alpha,
                )
            }
        };

        if a0 > 0.0 {
            self.b0 = b0 / a0;
            self.b1 = b1 / a0;
            self.b2 = b2 / a0;
            self.a1 = a1 / a0;
            self.a2 = a2 / a0;
        }
    }

    pub fn process_frame(&mut self, frame: &mut [f32]) {
        if self.gain.abs() < 0.05 { return; } // Optimization
        for (i, sample) in frame.iter_mut().enumerate() {
            let s = &mut self.states[i];
            let out = self.b0 * *sample + self.b1 * s.x1 + self.b2 * s.x2 
                      - self.a1 * s.y1 - self.a2 * s.y2;
            s.x2 = s.x1;
            s.x1 = *sample;
            s.y2 = s.y1;
            s.y1 = out;
            *sample = out;
        }
    }
}

// --- New Audio Effects Definitions ---

#[derive(Clone)]
pub struct BassBoost { 
    pub enabled: bool, 
    pub freq: f32, 
    pub gain: f32,
    filter: BiquadFilter,
}
impl Default for BassBoost { 
    fn default() -> Self { 
        Self { enabled: false, freq: 100.0, gain: 0.0, filter: BiquadFilter::new(BiquadFilterType::LowShelf, 100.0, 0.0, 0.707) } 
    } 
}
impl BassBoost {
    pub fn set_sample_rate(&mut self, rate: f32) { self.filter.set_sample_rate(rate); }
    pub fn resize_channels(&mut self, ch: usize) { self.filter.resize_channels(ch); }
    pub fn process(&mut self, frame: &mut [f32]) {
        if self.filter.gain != self.gain {
            self.filter.set_params(self.freq, self.gain, 0.707);
        }
        self.filter.process_frame(frame);
    }
}

#[derive(Clone)]
pub struct VoiceBoost { 
    pub enabled: bool, 
    pub freq: f32, 
    pub gain: f32, 
    pub bandwidth: f32,
    filter: BiquadFilter,
}
impl Default for VoiceBoost { 
    fn default() -> Self { 
        Self { enabled: false, freq: 2000.0, gain: 0.0, bandwidth: 1.0, filter: BiquadFilter::new(BiquadFilterType::Peak, 2000.0, 0.0, 1.0) } 
    } 
}
impl VoiceBoost {
    pub fn set_sample_rate(&mut self, rate: f32) { self.filter.set_sample_rate(rate); }
    pub fn resize_channels(&mut self, ch: usize) { self.filter.resize_channels(ch); }
    pub fn process(&mut self, frame: &mut [f32]) {
        if self.filter.gain != self.gain {
            self.filter.set_params(self.freq, self.gain, self.bandwidth);
        }
        self.filter.process_frame(frame);
    }
}

#[derive(Clone)]
pub struct StereoExpander { 
    pub enabled: bool, 
    pub width: f32 
}
impl Default for StereoExpander { 
    fn default() -> Self { Self { enabled: false, width: 1.0 } } 
}
impl StereoExpander {
    pub fn process(&mut self, frame: &mut [f32]) {
        if frame.len() == 2 && self.width != 1.0 {
            let l = frame[0];
            let r = frame[1];
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5;
            frame[0] = mid + side * self.width;
            frame[1] = mid - side * self.width;
        }
    }
}

#[derive(Clone)]
pub struct ReplayGain { pub enabled: bool, pub preamp: f32 }
impl Default for ReplayGain { fn default() -> Self { Self { enabled: false, preamp: 0.0 } } }

#[derive(Clone)]
pub struct NoiseGate { 
    pub enabled: bool, 
    pub threshold: f32, 
    pub attack: f32, 
    pub release: f32,
    envelope: f32,
    sample_rate: f32,
}
impl Default for NoiseGate { 
    fn default() -> Self { Self { enabled: false, threshold: -60.0, attack: 0.005, release: 0.1, envelope: 0.0, sample_rate: 44100.0 } } 
}
impl NoiseGate {
    pub fn process(&mut self, frame: &mut [f32]) {
        let max_abs = frame.iter().map(|s| s.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
        let attack_coeff = (-1.0 / (self.attack * self.sample_rate)).exp();
        let release_coeff = (-1.0 / (self.release * self.sample_rate)).exp();
        
        if max_abs > self.envelope {
            self.envelope = attack_coeff * self.envelope + (1.0 - attack_coeff) * max_abs;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * max_abs;
        }

        let env_db = if self.envelope > 1e-6 { 20.0 * self.envelope.log10() } else { -96.0 };
        if env_db < self.threshold {
            // Apply a smooth attenuation when under threshold (simple soft mute)
            let att = 10.0f32.powf((env_db - self.threshold) / 20.0).max(0.0001);
            for s in frame.iter_mut() {
                *s *= att;
            }
        }
    }
}

#[derive(Clone)]
pub struct Crossfeed { 
    pub enabled: bool, 
    pub amount: f32,
    // Simple 1-pole lowpass for crossfeed signal to simulate head shadowing
    lp_l: f32, lp_r: f32,
}
impl Default for Crossfeed { 
    fn default() -> Self { Self { enabled: false, amount: 0.0, lp_l: 0.0, lp_r: 0.0 } } 
}
impl Crossfeed {
    pub fn process(&mut self, frame: &mut [f32]) {
        if frame.len() == 2 && self.amount > 0.0 {
            let l = frame[0];
            let r = frame[1];
            // Simple ~700Hz lowpass for the crossfeed
            self.lp_l = 0.9 * self.lp_l + 0.1 * l;
            self.lp_r = 0.9 * self.lp_r + 0.1 * r;
            frame[0] = l + self.lp_r * self.amount;
            frame[1] = r + self.lp_l * self.amount;
            
            // Normalize level slightly
            let norm = 1.0 / (1.0 + self.amount * 0.5);
            frame[0] *= norm;
            frame[1] *= norm;
        }
    }
}

#[derive(Clone)]
pub struct Limiter { 
    pub enabled: bool, 
    pub ceiling: f32, 
    pub release: f32,
    envelope: f32,
    sample_rate: f32,
}
impl Default for Limiter { 
    fn default() -> Self { Self { enabled: false, ceiling: -0.1, release: 0.05, envelope: 0.0, sample_rate: 44100.0 } } 
}
impl Limiter {
    pub fn process(&mut self, frame: &mut [f32]) {
        let max_abs = frame.iter().map(|s| s.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or(0.0);
        let release_coeff = (-1.0 / (self.release * self.sample_rate)).exp();
        
        // Fast attack (instant), smooth release
        if max_abs > self.envelope {
            self.envelope = max_abs;
        } else {
            self.envelope = release_coeff * self.envelope + (1.0 - release_coeff) * max_abs;
        }
        
        let ceiling_lin = 10.0f32.powf(self.ceiling / 20.0);
        if self.envelope > ceiling_lin {
            let attenuation = ceiling_lin / self.envelope;
            for s in frame.iter_mut() {
                *s *= attenuation;
            }
        }
    }
}

#[derive(Clone)]
pub struct HrtfSpatializer { pub enabled: bool }
impl Default for HrtfSpatializer { fn default() -> Self { Self { enabled: false } } }
impl HrtfSpatializer {
    pub fn process(&mut self, frame: &mut [f32]) {
        if !self.enabled || frame.len() < 2 { return; }
        // Placeholder para futura integración con crate `hrtf` + `rustfft`.
        // Por el momento previene el error en la llamada process_frame y hace bypass.
    }
}

#[derive(Clone)]
pub struct Delay { 
    pub enabled: bool, 
    pub time: f32, 
    pub feedback: f32, 
    pub mix: f32,
    buffer: Vec<Vec<f32>>,
    write_idx: usize,
    sample_rate: f32,
}
impl Default for Delay { 
    fn default() -> Self { Self { enabled: false, time: 0.3, feedback: 0.3, mix: 0.3, buffer: vec![vec![0.0; 88200]; 2], write_idx: 0, sample_rate: 44100.0 } } 
}
impl Delay {
    pub fn resize_channels(&mut self, ch: usize) {
        self.buffer.resize(ch, vec![0.0; (self.sample_rate * 2.0) as usize]);
    }
    pub fn set_sample_rate(&mut self, rate: f32) {
        self.sample_rate = rate;
        for b in &mut self.buffer { b.resize((rate * 2.0) as usize, 0.0); }
    }
    pub fn process(&mut self, frame: &mut [f32]) {
        let delay_samples = (self.time * self.sample_rate) as usize;
        let max_samples = self.buffer[0].len();
        if delay_samples == 0 || max_samples == 0 { return; }
        
        for (i, sample) in frame.iter_mut().enumerate() {
            if i >= self.buffer.len() { continue; }
            let buf = &mut self.buffer[i];
            
            let read_idx = (self.write_idx + max_samples - delay_samples) % max_samples;
            let delayed = buf[read_idx];
            
            buf[self.write_idx] = *sample + delayed * self.feedback;
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
        self.write_idx = (self.write_idx + 1) % max_samples;
    }
}

#[derive(Clone)]
pub struct Chorus { 
    pub enabled: bool, 
    pub rate: f32, 
    pub depth: f32, 
    pub mix: f32,
    buffer: Vec<Vec<f32>>,
    write_idx: usize,
    lfo_phase: f32,
    sample_rate: f32,
}
impl Default for Chorus { 
    fn default() -> Self { Self { enabled: false, rate: 1.0, depth: 0.5, mix: 0.5, buffer: vec![vec![0.0; 4410]; 2], write_idx: 0, lfo_phase: 0.0, sample_rate: 44100.0 } } 
}
impl Chorus {
    pub fn resize_channels(&mut self, ch: usize) { self.buffer.resize(ch, vec![0.0; (self.sample_rate * 0.1) as usize]); }
    pub fn set_sample_rate(&mut self, rate: f32) { self.sample_rate = rate; for b in &mut self.buffer { b.resize((rate * 0.1) as usize, 0.0); } }
    pub fn process(&mut self, frame: &mut [f32]) {
        let base_delay = 0.02; // 20ms
        let mod_delay = 0.01;  // 10ms
        let max_samples = self.buffer[0].len();
        if max_samples < 2 { return; }
        
        use std::f32::consts::PI;
        let lfo = (self.lfo_phase * 2.0 * PI).sin();
        self.lfo_phase += self.rate / self.sample_rate;
        if self.lfo_phase > 1.0 { self.lfo_phase -= 1.0; }
        
        let current_delay = base_delay + mod_delay * self.depth * lfo;
        let delay_samples = current_delay * self.sample_rate;
        
        for (i, sample) in frame.iter_mut().enumerate() {
            if i >= self.buffer.len() { continue; }
            let buf = &mut self.buffer[i];
            
            let read_idx_f = (self.write_idx as f32 + max_samples as f32 - delay_samples) % (max_samples as f32);
            let read_idx_i = read_idx_f as usize;
            let frac = read_idx_f - read_idx_i as f32;
            let next_idx = (read_idx_i + 1) % max_samples;
            
            let delayed = buf[read_idx_i] * (1.0 - frac) + buf[next_idx] * frac;
            buf[self.write_idx] = *sample;
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
        self.write_idx = (self.write_idx + 1) % max_samples;
    }
}

#[derive(Clone)]
pub struct Flanger { 
    pub enabled: bool, 
    pub rate: f32, 
    pub depth: f32, 
    pub feedback: f32, 
    pub mix: f32,
    buffer: Vec<Vec<f32>>,
    write_idx: usize,
    lfo_phase: f32,
    sample_rate: f32,
}
impl Default for Flanger { 
    fn default() -> Self { Self { enabled: false, rate: 0.5, depth: 0.5, feedback: 0.5, mix: 0.5, buffer: vec![vec![0.0; 882]; 2], write_idx: 0, lfo_phase: 0.0, sample_rate: 44100.0 } } 
}
impl Flanger {
    pub fn resize_channels(&mut self, ch: usize) { self.buffer.resize(ch, vec![0.0; (self.sample_rate * 0.02) as usize]); }
    pub fn set_sample_rate(&mut self, rate: f32) { self.sample_rate = rate; for b in &mut self.buffer { b.resize((rate * 0.02) as usize, 0.0); } }
    pub fn process(&mut self, frame: &mut [f32]) {
        let base_delay = 0.001; // 1ms
        let mod_delay = 0.004;  // 4ms
        let max_samples = self.buffer[0].len();
        if max_samples < 2 { return; }
        
        use std::f32::consts::PI;
        // Sweep 0 to 1 instead of -1 to 1 for distinct flanger comb effect
        let lfo = ((self.lfo_phase * 2.0 * PI).sin() * 0.5) + 0.5;
        self.lfo_phase += self.rate / self.sample_rate;
        if self.lfo_phase > 1.0 { self.lfo_phase -= 1.0; }
        
        let current_delay = base_delay + mod_delay * self.depth * lfo;
        let delay_samples = current_delay * self.sample_rate;
        
        for (i, sample) in frame.iter_mut().enumerate() {
            if i >= self.buffer.len() { continue; }
            let buf = &mut self.buffer[i];
            
            let read_idx_f = (self.write_idx as f32 + max_samples as f32 - delay_samples) % (max_samples as f32);
            let read_idx_i = read_idx_f as usize;
            let frac = read_idx_f - read_idx_i as f32;
            let next_idx = (read_idx_i + 1) % max_samples;
            
            let delayed = buf[read_idx_i] * (1.0 - frac) + buf[next_idx] * frac;
            buf[self.write_idx] = *sample + delayed * self.feedback;
            *sample = *sample * (1.0 - self.mix) + delayed * self.mix;
        }
        self.write_idx = (self.write_idx + 1) % max_samples;
    }
}

#[derive(Clone)]
pub struct Phaser { 
    pub enabled: bool, 
    pub rate: f32, 
    pub depth: f32, 
    pub feedback: f32, 
    pub mix: f32,
    filters: [BiquadFilter; 4],
    lfo_phase: f32,
    sample_rate: f32,
}
impl Default for Phaser { 
    fn default() -> Self { 
        let f = BiquadFilter::new(BiquadFilterType::AllPass, 1000.0, 0.0, 0.707);
        Self { enabled: false, rate: 0.5, depth: 0.5, feedback: 0.5, mix: 0.5, filters: [f.clone(), f.clone(), f.clone(), f.clone()], lfo_phase: 0.0, sample_rate: 44100.0 } 
    } 
}
impl Phaser {
    pub fn resize_channels(&mut self, ch: usize) { for f in &mut self.filters { f.resize_channels(ch); } }
    pub fn set_sample_rate(&mut self, rate: f32) { self.sample_rate = rate; for f in &mut self.filters { f.set_sample_rate(rate); } }
    pub fn process(&mut self, frame: &mut [f32]) {
        use std::f32::consts::PI;
        let lfo = ((self.lfo_phase * 2.0 * PI).sin() * 0.5) + 0.5; // 0..1
        self.lfo_phase += self.rate / self.sample_rate;
        if self.lfo_phase > 1.0 { self.lfo_phase -= 1.0; }
        
        let min_freq = 400.0f32;
        let max_freq = 4000.0f32;
        // Exponential sweep for better perception
        let current_freq = min_freq * (max_freq / min_freq).powf(lfo * self.depth);
        
        for f in &mut self.filters {
            f.set_params(current_freq, 0.0, 0.707);
        }
        
        let mut f_frame = frame.to_vec(); // Copy for dry/wet mix
        self.filters[0].process_frame(&mut f_frame);
        self.filters[1].process_frame(&mut f_frame);
        self.filters[2].process_frame(&mut f_frame);
        self.filters[3].process_frame(&mut f_frame);
        
        for (i, sample) in frame.iter_mut().enumerate() {
            *sample = *sample * (1.0 - self.mix) + f_frame[i] * self.mix;
        }
    }
}

#[derive(Clone)]
pub struct StereoBalance { 
    pub enabled: bool, 
    pub balance: f32 
}
impl Default for StereoBalance { 
    fn default() -> Self { Self { enabled: false, balance: 0.0 } } 
}
impl StereoBalance {
    pub fn process(&mut self, frame: &mut [f32]) {
        if frame.len() == 2 && self.balance != 0.0 {
            // balance -1.0 = left 100%, right 0%
            // balance 1.0 = right 100%, left 0%
            let gain_l = if self.balance > 0.0 { 1.0 - self.balance } else { 1.0 };
            let gain_r = if self.balance < 0.0 { 1.0 + self.balance } else { 1.0 };
            frame[0] *= gain_l;
            frame[1] *= gain_r;
        }
    }
}
