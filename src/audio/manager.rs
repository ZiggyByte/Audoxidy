use crate::audio::AudioError;
use crate::audio::engine::{AudioEngine, AudioState};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::audio::preset::EqPreset;

/// Fachada de alto nivel para el motor de audio.
///
/// Expone una API simplificada para reproducción, control de volumen,
/// DSP, EQ y configuración de dispositivo, ocultando la complejidad
/// del `AudioEngine`, los hilos de decodificación y el stream CPAL.
pub struct AudioManager {
    engine: AudioEngine,
    database: Arc<parking_lot::Mutex<Option<Arc<std::sync::Mutex<crate::db::Database>>>>>,
}

#[allow(dead_code)]
impl AudioManager {
    /// Crea un nuevo `AudioManager` con el motor de audio inicializado.
    pub fn new() -> Result<Self, AudioError> {
        let engine = AudioEngine::new()?;
        engine.start()?;

        Ok(Self {
            engine,
            database: Arc::new(parking_lot::Mutex::new(None)),
        })
    }

    /// Asigna la referencia a la base de datos para búsqueda de ReplayGain.
    pub fn set_database(&self, db: Arc<std::sync::Mutex<crate::db::Database>>) {
        *self.database.lock() = Some(db);
    }

    /// Devuelve la referencia a la base de datos, si está configurada.
    pub fn get_database(&self) -> Option<Arc<std::sync::Mutex<crate::db::Database>>> {
        self.database.lock().clone()
    }

    /// Devuelve una referencia clonable al estado compartido del motor.
    pub fn state(&self) -> Arc<RwLock<AudioState>> {
        self.engine.state.clone()
    }

    /// Inicia o reanuda la reproducción.
    pub fn play(&self) {
        self.engine.set_playing(true);
    }

    /// Establece el estado de reproducción.
    pub fn set_playing(&self, playing: bool) {
        self.engine.set_playing(playing);
    }

    /// Pausa la reproducción.
    pub fn pause(&self) {
        self.engine.set_playing(false);
    }

    /// Detiene la reproducción y resetea el estado.
    pub fn stop(&self) {
        self.engine.stop();
    }

    /// Devuelve `true` si el motor está reproduciendo activamente.
    pub fn is_playing(&self) -> bool {
        self.engine.state.read().is_playing
    }

    /// Alterna entre reproducción y pausa.
    pub fn toggle_play_pause(&self) {
        let playing = self.is_playing();
        self.engine.set_playing(!playing);
    }

    /// Busca a una posición específica en segundos.
    pub fn seek(&self, pos_sec: f64) {
        self.engine.seek(pos_sec);
    }

    #[allow(dead_code)]
    pub fn get_duration(&self) -> f64 {
        self.engine.state.read().total_duration_sec
    }

    /// Resetea el flag de fin de archivo (EOF).
    pub fn clear_eof(&self) {
        self.engine.state.write().eof_reached = false;
    }

    /// Carga y decodifica un archivo de audio.
    ///
    /// Si no se proporcionan ganancias ReplayGain, se consultan en la BD
    /// si está disponible.
    pub fn load_file(
        &self,
        path: &str,
        title: impl Into<String>,
        artist: impl Into<String>,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    ) -> Result<(), AudioError> {
        let (mut tg, mut ag) = (track_gain, album_gain);

        // Si no se pasaron ganancias (ej. desde el módulo Playlist), intentamos buscarlas nosotros en la BD
        if tg.is_none() && ag.is_none() {
            if let Some(db_arc) = &*self.database.lock() {
                // Usamos try_lock para evitar Deadlocks si el llamador ya tiene la BD bloqueada (ej. en el inicio de la App)
                if let Ok(db) = db_arc.try_lock() {
                    if let Ok((db_tg, db_ag)) = db.get_replay_gain_by_path(path) {
                        tg = db_tg;
                        ag = db_ag;
                    }
                }
            }
        }

        self.engine
            .decode_file(path, title.into(), artist.into(), tg, ag)
    }

    /// Pre-carga la siguiente canción en el hilo decodificador para transiciones sin cortes.
    ///
    /// Igual que `load_file` pero envía un comando de pre-carga (la canción se decodifica
    /// por adelantado y se promueve sin pausa cuando termina la actual).
    pub fn preload_next(
        &self,
        path: &str,
        title: impl Into<String>,
        artist: impl Into<String>,
        track_gain: Option<f64>,
        album_gain: Option<f64>,
    ) -> Result<(), AudioError> {
        let (mut tg, mut ag) = (track_gain, album_gain);
        if tg.is_none() && ag.is_none() {
            if let Some(db_arc) = &*self.database.lock() {
                if let Ok(db) = db_arc.try_lock() {
                    if let Ok((db_tg, db_ag)) = db.get_replay_gain_by_path(path) {
                        tg = db_tg;
                        ag = db_ag;
                    }
                }
            }
        }
        self.engine
            .preload_file(path, title.into(), artist.into(), tg, ag)
    }

    /// Descarta el estado de pre-carga actual en el hilo decodificador.
    pub fn clear_preload(&self) -> Result<(), AudioError> {
        self.engine.clear_preload()
    }

    /// Dispara un crossfade manual con la duración especificada en milisegundos.
    pub fn crossfade_next(&self, ms: f64) -> Result<(), AudioError> {
        self.engine.crossfade_next(ms)
    }

    /// Establece el volumen de reproducción (0.0 a 1.0).
    pub fn set_volume(&self, volume: f32) {
        self.engine.set_volume(volume);
    }

    // DSP Controls
    /// Devuelve la ganancia del preamplificador en dB.
    pub fn get_preamp_gain(&self) -> f32 {
        self.engine.dsp.read().get_preamp_db()
    }

    /// Establece la ganancia del preamplificador en dB.
    pub fn set_preamp_gain(&self, db: f32) {
        self.engine.dsp.write().set_preamp_db(db);
    }

    /// Establece la ganancia de una banda del ecualizador.
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

    /// Devuelve `true` si el ecualizador está activo.
    pub fn get_eq_enabled(&self) -> bool {
        self.engine.dsp.read().equalizer.enabled
    }

    /// Devuelve el número de bandas del ecualizador.
    pub fn get_eq_bands_count(&self) -> usize {
        self.engine.dsp.read().equalizer.bands.len()
    }

    /// Devuelve la frecuencia y ganancia de una banda del ecualizador.
    pub fn get_eq_band_info(&self, index: usize) -> Option<(f32, f32)> {
        let dsp = self.engine.dsp.read();
        dsp.equalizer
            .bands
            .get(index)
            .map(|b| (b.frequency, b.gain))
    }

    /// Cambia el modo del ecualizador entre 20 y 31 bandas.
    pub fn set_eq_mode(&self, num_bands: usize) {
        let mut dsp = self.engine.dsp.write();
        if dsp.equalizer.bands.len() != num_bands {
            dsp.equalizer.set_mode(num_bands);
        }
    }

    /// Restablece los valores predeterminados del DSP (preamp a 0 dB, EQ plano).
    pub fn reset_dsp_defaults(&self) {
        let mut dsp = self.engine.dsp.write();
        dsp.set_preamp_db(0.0);
        dsp.equalizer.reset_all();
    }

    /// Aplica un preset de EQ completo (preamp + todas las ganancias) atómicamente a través del RwLock.
    pub fn apply_eq_preset(&self, preset: &EqPreset) {
        let mut dsp = self.engine.dsp.write();
        dsp.set_preamp_db(preset.preamp_gain);
        dsp.equalizer
            .apply_preset_gains(&preset.get_gains_20(), &preset.get_gains_31());
    }

    /// Aplica ganancias de bandas de EQ directamente a ambos sets (20 y 31 bandas)
    /// sin modificar el preamplificador.
    pub fn apply_eq_preset_gains(&self, bands_20: &[f32], bands_31: &[f32]) {
        let mut dsp = self.engine.dsp.write();
        dsp.equalizer.apply_preset_gains(bands_20, bands_31);
    }

    /// Devuelve una copia del estado actual del motor de audio.
    pub fn get_state(&self) -> crate::audio::engine::AudioState {
        self.engine.state.read().clone()
    }

    // --- Configuration ---
    /// Devuelve la lista de hosts de audio disponibles.
    pub fn get_available_hosts(&self) -> Vec<String> {
        self.engine.get_available_hosts()
    }

    /// Devuelve la lista de dispositivos de salida disponibles.
    pub fn get_devices(&self) -> Vec<crate::audio::engine::AudioDeviceInfo> {
        self.engine.get_devices()
    }

    /// Aplica una configuración de audio completa.
    pub fn apply_audio_settings(
        &self,
        settings: crate::audio::engine::AudioSettings,
    ) -> Result<(), AudioError> {
        self.engine.apply_settings(settings)
    }

    /// Purga los buffers internos y reinicia el stream de audio.
    pub fn purge_buffers(&self) -> Result<(), AudioError> {
        self.engine.purge_buffers()
    }

    // --- Reverb Controls ---
    /// Activa o desactiva el efecto de reverberación.
    pub fn set_reverb_enabled(&self, enabled: bool) {
        self.engine.dsp.write().reverb.enabled = enabled;
    }

    /// Establece el tamaño de la habitación del reverb.
    pub fn set_reverb_room_size(&self, value: f32) {
        self.engine.dsp.write().reverb.set_room_size(value);
    }

    /// Establece el damping del reverb.
    pub fn set_reverb_damping(&self, value: f32) {
        self.engine.dsp.write().reverb.set_damping(value);
    }

    /// Establece el nivel de señal procesada (wet) del reverb.
    pub fn set_reverb_wet(&self, value: f32) {
        self.engine.dsp.write().reverb.set_wet(value);
    }

    /// Establece el nivel de señal seca (dry) del reverb.
    pub fn set_reverb_dry(&self, value: f32) {
        self.engine.dsp.write().reverb.set_dry(value);
    }

    // --- Compressor Controls ---
    /// Activa o desactiva el compresor.
    pub fn set_compressor_enabled(&self, enabled: bool) {
        self.engine.dsp.write().compressor.enabled = enabled;
    }

    /// Configura los parámetros del compresor.
    pub fn set_compressor_params(&self, threshold: f32, ratio: f32, attack: f32, release: f32) {
        self.engine
            .dsp
            .write()
            .compressor
            .set_params(threshold, ratio, attack, release);
    }

    // --- Direct DSP Access ---
    /// Ejecuta un closure con acceso de solo lectura a la cadena DSP.
    pub fn with_dsp<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&crate::audio::dsp::DspChain) -> R,
    {
        let dsp = self.engine.dsp.read();
        f(&dsp)
    }

    /// Ejecuta un closure con acceso de escritura a la cadena DSP.
    pub fn with_dsp_mut<F>(&self, f: F)
    where
        F: FnOnce(&mut crate::audio::dsp::DspChain),
    {
        let mut dsp = self.engine.dsp.write();
        f(&mut dsp);
    }

    /// Force limiter on (for normalization auto-on per D-25).
    /// Returns the previous limiter enabled state.
    pub fn force_limiter_on(&self) -> bool {
        let mut was_enabled = false;
        self.with_dsp_mut(|dsp| {
            was_enabled = dsp.force_limiter_on();
        });
        was_enabled
    }

    /// Restore limiter to its previous state.
    pub fn restore_limiter(&self, was_enabled: bool) {
        self.with_dsp_mut(|dsp| dsp.restore_limiter(was_enabled));
    }
}
