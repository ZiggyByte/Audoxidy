use std::sync::Arc;
use crate::audio::engine::{AudioEngine, AudioState};
use parking_lot::RwLock;

pub struct AudioManager {
    engine: AudioEngine,
}

impl AudioManager {
    pub fn new() -> Result<Self, String> {
        let engine = AudioEngine::new()?;
        engine.start()?;
        
        Ok(Self {
            engine,
        })
    }

    pub fn state(&self) -> Arc<RwLock<AudioState>> {
        self.engine.state.clone()
    }

    pub fn play(&self) {
        self.engine.set_playing(true);
    }

    pub fn set_playing(&self, playing: bool) {
        self.engine.set_playing(playing);
    }

    pub fn pause(&self) {
        self.engine.set_playing(false);
    }

    pub fn stop(&self) {
        self.engine.stop();
    }

    pub fn is_playing(&self) -> bool {
        self.engine.state.read().is_playing
    }

    pub fn toggle_play_pause(&self) {
        let playing = self.is_playing();
        self.engine.set_playing(!playing);
    }

    pub fn seek(&self, pos_sec: f64) {
        self.engine.seek(pos_sec);
    }

    #[allow(dead_code)]
    pub fn get_duration(&self) -> f64 {
        self.engine.state.read().total_duration_sec
    }

    pub fn load_file(&self, path: &str) -> Result<(), String> {
        self.engine.decode_file(path)
    }

    pub fn set_volume(&self, volume: f32) {
        self.engine.set_volume(volume);
    }
    
    // DSP Controls
    pub fn get_preamp_gain(&self) -> f32 {
        self.engine.dsp.read().get_preamp_db()
    }

    pub fn set_preamp_gain(&self, db: f32) {
        self.engine.dsp.write().set_preamp_db(db);
    }

    pub fn set_eq_band_gain(&self, index: usize, db: f32) {
        let mut dsp = self.engine.dsp.write();
        if index < dsp.equalizer.bands.len() {
            // Need current sample rate for filter update
            // For now use default 44100 or read from state?
            // Ideally DspChain should know sample rate or update it on stream start.
            // But bands update needs it.
            // Let's assume 44100 for update, or better:
            // The bands update_coefficients should be called when sample rate changes too.
            // For now let's pass 44100.0 or the current output rate.
            dsp.equalizer.bands[index].set_gain(db);
        }
    }

    #[allow(dead_code)]
    pub fn set_eq_enabled(&self, enabled: bool) {
        self.engine.dsp.write().equalizer.enabled = enabled;
    }
    
    pub fn get_eq_bands_count(&self) -> usize {
        self.engine.dsp.read().equalizer.bands.len()
    }

    pub fn get_eq_band_info(&self, index: usize) -> Option<(f32, f32)> {
        let dsp = self.engine.dsp.read();
        dsp.equalizer.bands.get(index).map(|b| (b.frequency, b.gain))
    }

    pub fn set_eq_mode(&self, num_bands: usize) {
        let mut dsp = self.engine.dsp.write();
        if dsp.equalizer.bands.len() != num_bands {
            dsp.equalizer.set_mode(num_bands);
        }
    }
    
    pub fn reset_dsp_defaults(&self) {
        let mut dsp = self.engine.dsp.write();
        dsp.set_preamp_db(0.0);
        dsp.equalizer.reset_all();
    }

    pub fn get_state(&self) -> crate::audio::engine::AudioState {
        self.engine.state.read().clone()
    }

    // --- Configuration ---
    pub fn get_available_hosts(&self) -> Vec<String> {
        self.engine.get_available_hosts()
    }

    pub fn get_devices(&self) -> Vec<crate::audio::engine::AudioDeviceInfo> {
        self.engine.get_devices()
    }

    pub fn apply_audio_settings(&self, settings: crate::audio::engine::AudioSettings) -> Result<(), String> {
        self.engine.apply_settings(settings)
    }

    // --- Reverb Controls ---
    pub fn set_reverb_enabled(&self, enabled: bool) {
        self.engine.dsp.write().reverb.enabled = enabled;
    }

    pub fn set_reverb_room_size(&self, value: f32) {
        self.engine.dsp.write().reverb.set_room_size(value);
    }

    pub fn set_reverb_damping(&self, value: f32) {
        self.engine.dsp.write().reverb.set_damping(value);
    }

    pub fn set_reverb_wet(&self, value: f32) {
        self.engine.dsp.write().reverb.set_wet(value);
    }

    pub fn set_reverb_dry(&self, value: f32) {
        self.engine.dsp.write().reverb.set_dry(value);
    }

    // --- Compressor Controls ---
    pub fn set_compressor_enabled(&self, enabled: bool) {
        self.engine.dsp.write().compressor.enabled = enabled;
    }

    pub fn set_compressor_params(&self, threshold: f32, ratio: f32, attack: f32, release: f32) {
        self.engine.dsp.write().compressor.set_params(threshold, ratio, attack, release);
    }

    // --- Direct DSP Access ---
    pub fn with_dsp<F, R>(&self, f: F) -> R 
    where F: FnOnce(&crate::audio::dsp::DspChain) -> R {
        let dsp = self.engine.dsp.read();
        f(&dsp)
    }

    pub fn with_dsp_mut<F>(&self, f: F)
    where F: FnOnce(&mut crate::audio::dsp::DspChain) {
        let mut dsp = self.engine.dsp.write();
        f(&mut dsp);
    }
}
