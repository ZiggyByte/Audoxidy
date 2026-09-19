//! Audio center message dispatcher.

use crate::audio::AudioManager;
use crate::audio::device_manager::{AudioSettings, BitDepth, ChannelConfig};

use super::messages::{AudioCenterMessage, AudioStateToggle, DspEffect, SystemSelection};
use super::state::{AudioCenterManager, log_persist};

impl AudioCenterManager {
    /// Apply an EQ preset to the manager state and AudioManager/DSP.
    fn apply_eq_preset_to_state(
        &mut self,
        preset: &crate::audio::preset::EqPreset,
        audio_manager: &AudioManager,
    ) {
        self.selected_preset = Some(preset.clone());
        self.preamp_gain = preset.preamp_gain;
        audio_manager.apply_eq_preset(preset);
        let gains = if self.equalizer_bands_31 {
            preset.get_gains_31()
        } else {
            preset.get_gains_20()
        };
        // Round to 1 decimal place
        let rounded: Vec<f32> = gains.iter().map(|&g| (g * 10.0).round() / 10.0).collect();
        self.eq_band_gains = rounded;
    }

    pub fn update(
        &mut self,
        message: AudioCenterMessage,
        audio_manager: &AudioManager,
        db: &std::sync::Mutex<crate::db::Database>,
    ) {
        match message {
            AudioCenterMessage::TabSelected(tab) => {
                self.selected_tab = tab;
            }
            AudioCenterMessage::DragStart => {}
            AudioCenterMessage::HostSelected(host) => {
                self.selected_host = Some(host);
                self.apply_enabled = true;
            }
            AudioCenterMessage::DeviceSelected(device) => {
                self.selected_device = Some(device);
                self.apply_enabled = true;
            }
            AudioCenterMessage::SampleRateSelected(rate) => {
                self.selected_sample_rate = rate;
                self.apply_enabled = true;
            }
            AudioCenterMessage::BitDepthSelected(depth) => {
                self.selected_bit_depth = depth;
                self.apply_enabled = true;
            }
            AudioCenterMessage::ChannelsManualSelected(ch) => {
                self.selected_channels_manual = ch;
                self.apply_enabled = true;
            }
            AudioCenterMessage::BufferSizeSelected(size) => {
                self.selected_buffer_size = size;
                self.apply_enabled = true;
            }
            AudioCenterMessage::SystemRateSelected(rate) => {
                self.system_rate = rate;
                self.apply_enabled = true;
            }
            AudioCenterMessage::SystemQuantumSelected(quantum) => {
                self.system_quantum = quantum;
                self.apply_enabled = true;
            }
            AudioCenterMessage::ResetToDefaults => {
                self.selected_sample_rate = Some(48000);
                self.selected_bit_depth = BitDepth::Bits32Float;
                self.selected_buffer_size = None;
                self.selected_channels_manual = 2;
                self.system_rate = SystemSelection::Default;
                self.system_quantum = SystemSelection::Default;

                if self.cached_hosts.contains(&"ALSA".to_string()) {
                    self.selected_host = Some("ALSA".to_string());
                } else if let Some(first) = self.cached_hosts.first() {
                    self.selected_host = Some(first.clone());
                }

                let target = self
                    .cached_devices
                    .iter()
                    .find(|d| d.name.to_lowercase().contains("pipewire"))
                    .or_else(|| {
                        self.cached_devices
                            .iter()
                            .find(|d| d.name.to_lowercase().contains("pulse"))
                    })
                    .or_else(|| {
                        self.cached_devices
                            .iter()
                            .find(|d| d.name.to_lowercase().contains("default"))
                    });

                if let Some(d) = target {
                    self.selected_device = Some(d.name.clone());
                } else if let Some(first) = self.cached_devices.first() {
                    self.selected_device = Some(first.name.clone());
                }

                self.apply_enabled = true;
            }
            AudioCenterMessage::ApplySettings => {
                // 0. Aplicar PRIMERO el reloj del sistema (PipeWire/PulseAudio).
                //    Debe ir ANTES de construir el stream: si se aplica después,
                //    PipeWire reconfigura el grafo en caliente y el stream existente
                //    queda desconectado del sink — su callback sigue drenando el
                //    ringbuf pero el audio nunca llega al dispositivo.
                let rate_val = match self.system_rate {
                    SystemSelection::Default => None,
                    SystemSelection::Automatic => self.selected_sample_rate,
                    SystemSelection::Fixed(r) => Some(r),
                };
                let quantum_val = match self.system_quantum {
                    SystemSelection::Default => None,
                    SystemSelection::Automatic => {
                        // Quantum estimado: tasa × 10ms de latencia base (no se puede
                        // esperar el negociado real porque el stream aún no existe).
                        let rate = self.selected_sample_rate.unwrap_or(48000);
                        let frames = (rate as f64 * 10.0 / 1000.0) as u32;
                        Some(if frames < 256 {
                            256
                        } else if frames < 512 {
                            512
                        } else if frames < 1024 {
                            1024
                        } else if frames < 2048 {
                            2048
                        } else if frames < 4096 {
                            4096
                        } else {
                            8192
                        })
                    }
                    SystemSelection::Fixed(q) => Some(q),
                };
                let has_pw = crate::integrations::system_audio::is_pipewire_active();
                if has_pw {
                    crate::integrations::system_audio::apply_pipewire_clock(rate_val, quantum_val);
                    // Esperar a que PipeWire renegocie el grafo con el nuevo reloj
                    // antes de construir el stream de salida.
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    crate::integrations::system_audio::persist_pipewire_conf(rate_val, quantum_val);
                } else {
                    // PulseAudio: no hay comando en vivo para forzar el reloj (el
                    // daemon.conf aplica al reiniciar el servicio); se persiste la
                    // configuración ANTES de conectar el stream y PulseAudio
                    // resamplea el stream a la tasa del sink de forma transparente.
                    crate::integrations::system_audio::persist_pulse_conf(rate_val, quantum_val);
                }

                // 1. Aplicar la configuración del reproductor (stream) — el grafo ya
                //    está a la tasa forzada, así el stream se conecta directamente.
                // Id estable del dispositivo seleccionado: se usa para aplicar y persistir.
                let selected_device_id = self
                    .cached_devices
                    .iter()
                    .find(|d| Some(&d.name) == self.selected_device.as_ref())
                    .and_then(|d| d.id.clone());
                let settings = AudioSettings {
                    host_id: self.selected_host.clone(),
                    device_name: self.selected_device.clone(),
                    device_id: selected_device_id.clone(),
                    sample_rate: self.selected_sample_rate,
                    bit_depth: Some(self.selected_bit_depth.clone()),
                    channels: ChannelConfig::Manual(self.selected_channels_manual),
                    buffer_size: self.selected_buffer_size,
                };
                if let Err(e) = audio_manager.apply_audio_settings(settings) {
                    // Si el stream no pudo construirse (p. ej. el dispositivo no
                    // soporta la combinación tasa×canales), se notifica claramente:
                    // antes el error se ignoraba y el reproductor quedaba mudo.
                    tracing::error!(
                        "No se pudo aplicar la configuración de audio ({}): el dispositivo \
                         puede no soportar la combinación de tasa de muestreo y canales \
                         elegida. La reproducción queda detenida hasta aplicar una \
                         configuración válida.",
                        e
                    );
                }

                // Guardar los ajustes en la base de datos para la persistencia
                if let Some(db_arc) = audio_manager.get_database() {
                    // db-lock: persistencia — el guardado se omite si el lock falla
                    if let Ok(db) = db_arc.lock() {
                        log_persist(
                            "audio_host",
                            db.set_setting(
                                "audio_host",
                                self.selected_host.as_deref().unwrap_or(""),
                            ),
                        );
                        log_persist(
                            "audio_device",
                            db.set_setting(
                                "audio_device",
                                self.selected_device.as_deref().unwrap_or(""),
                            ),
                        );
                        // Id estable del dispositivo seleccionado: sobrevive a renombrados
                        // y es la clave preferida en la próxima resolución.
                        log_persist(
                            "audio_device_id",
                            db.set_setting(
                                "audio_device_id",
                                selected_device_id.as_deref().unwrap_or(""),
                            ),
                        );
                        log_persist(
                            "audio_sample_rate",
                            db.set_setting(
                                "audio_sample_rate",
                                &self
                                    .selected_sample_rate
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| "auto".to_string()),
                            ),
                        );
                        log_persist(
                            "audio_bit_depth",
                            db.set_setting(
                                "audio_bit_depth",
                                match self.selected_bit_depth {
                                    BitDepth::Bits16 => "16",
                                    BitDepth::Bits24 => "24",
                                    BitDepth::Bits32Float => "32",
                                },
                            ),
                        );
                        log_persist(
                            "audio_channels",
                            db.set_setting(
                                "audio_channels",
                                &self.selected_channels_manual.to_string(),
                            ),
                        );
                        log_persist(
                            "audio_buffer_size",
                            db.set_setting(
                                "audio_buffer_size",
                                &self
                                    .selected_buffer_size
                                    .map(|b| b.to_string())
                                    .unwrap_or_else(|| "auto".to_string()),
                            ),
                        );
                        let sys_rate_str = match self.system_rate {
                            SystemSelection::Default => "default".to_string(),
                            SystemSelection::Automatic => "auto".to_string(),
                            SystemSelection::Fixed(r) => format!("fixed:{}", r),
                        };
                        log_persist(
                            "audio_system_rate",
                            db.set_setting("audio_system_rate", &sys_rate_str),
                        );

                        let sys_quantum_str = match self.system_quantum {
                            SystemSelection::Default => "default".to_string(),
                            SystemSelection::Automatic => "auto".to_string(),
                            SystemSelection::Fixed(q) => format!("fixed:{}", q),
                        };
                        log_persist(
                            "audio_system_quantum",
                            db.set_setting("audio_system_quantum", &sys_quantum_str),
                        );
                    } else {
                        tracing::warn!("db-lock: lock no disponible");
                    }
                }

                self.apply_enabled = false;
            }
            AudioCenterMessage::RestartService => {
                // Reinicio de los servicios de audio del sistema (PipeWire/Pulse).
                std::thread::spawn(crate::integrations::system_audio::restart_audio_services);
            }
            AudioCenterMessage::Close => {
                self.save_eq_settings_to_db(db);
                self.save_dsp_settings_to_db(audio_manager, db);
                self.open = false;
                self.window_pos = None;
            }
            AudioCenterMessage::EqToggleSelected(b) => {
                self.equalizer_enabled = b;
                audio_manager.set_eq_enabled(b);
                if b {
                    audio_manager.set_preamp_gain(self.preamp_gain);
                }
            }
            AudioCenterMessage::EqBandsSelected(is_31) => {
                self.equalizer_bands_31 = is_31;
                audio_manager.set_eq_mode(if is_31 { 31 } else { 20 });
                // Resync
                let count = if is_31 { 31 } else { 20 };
                let mut bands = Vec::new();
                for i in 0..count {
                    bands.push(audio_manager.get_eq_band_info(i).unwrap_or((0.0, 0.0)).1);
                }
                self.eq_band_gains = bands;
            }
            AudioCenterMessage::EqPreampChanged(val) => {
                let rounded = (val * 10.0).round() / 10.0;
                let clamped = rounded.clamp(-9.0, 9.0);
                self.preamp_gain = clamped;
                if self.equalizer_enabled {
                    audio_manager.set_preamp_gain(clamped);
                }
            }
            AudioCenterMessage::EqBandChanged(idx, val) => {
                let rounded = (val * 10.0).round() / 10.0;
                let clamped = rounded.clamp(-9.0, 9.0);
                if idx < self.eq_band_gains.len() {
                    self.eq_band_gains[idx] = clamped;
                    audio_manager.set_eq_band_gain(idx, clamped);
                }
            }
            AudioCenterMessage::EqPresetSelected(preset) => {
                self.apply_eq_preset_to_state(&preset, audio_manager);
            }

            // EQ Preset Management handlers
            AudioCenterMessage::EqPresetIconLoad => {
                // Handled at app.rs level — opens ActiveDialog::EqPresetLoad with state backup
            }
            AudioCenterMessage::EqPresetIconSave => {
                // Handled at app.rs level — opens ActiveDialog::EqPresetSave
            }
            AudioCenterMessage::EqPresetIconReset => {
                // Reset: selected_preset = None, preamp = 0.0, all bands = 0.0, reset DSP
                audio_manager.reset_dsp_defaults();
                self.selected_preset = None;
                self.preamp_gain = 0.0;
                let count = if self.equalizer_bands_31 { 31 } else { 20 };
                self.eq_band_gains = vec![0.0; count];
            }
            AudioCenterMessage::EqPresetLoadSelected(Some(idx)) => {
                let preset_opt = {
                    let presets = self.get_unified_presets();
                    presets.get(idx).map(|&p| p.clone())
                };
                if let Some(preset) = preset_opt {
                    self.apply_eq_preset_to_state(&preset, audio_manager);
                }
            }
            AudioCenterMessage::EqPresetLoadSelected(None) => {
                // No preset selected — nothing to preview
            }
            AudioCenterMessage::EqPresetLoadConfirm => {
                // Preset already applied as preview — the state is already committed.
                // The dialog is closed at the app.rs level.
                // Keep the current selected_preset, preamp_gain, and eq_band_gains
            }
            AudioCenterMessage::EqPresetSaveInput(name) => {
                self.pending_preset_name = name;
            }
            AudioCenterMessage::EqPresetSaveConfirm => {
                if !self.pending_preset_name.is_empty() {
                    let name = std::mem::take(&mut self.pending_preset_name);
                    let preset = crate::audio::preset::EqPreset::new(
                        &name,
                        self.preamp_gain,
                        if self.equalizer_bands_31 {
                            Some(crate::audio::preset::EqPreset::convert_31_to_20(
                                &self.eq_band_gains,
                            ))
                        } else {
                            Some(self.eq_band_gains.clone())
                        },
                        if self.equalizer_bands_31 {
                            Some(self.eq_band_gains.clone())
                        } else {
                            Some(crate::audio::preset::EqPreset::convert_20_to_31(
                                &self.eq_band_gains,
                            ))
                        },
                    );
                    // db-lock: persistencia — el guardado se omite si el lock falla
                    if let Ok(db_lock) = db.lock() {
                        let b20 = preset
                            .bands_20
                            .as_ref()
                            .map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect());
                        let b31 = preset
                            .bands_31
                            .as_ref()
                            .map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect());
                        if let Err(e) =
                            db_lock.save_eq_preset_raw(&preset.name, preset.preamp_gain, b20, b31)
                        {
                            tracing::warn!(
                                "No se pudo guardar el preset EQ '{}': {}",
                                preset.name,
                                e
                            );
                        }
                    } else {
                        tracing::warn!("db-lock: lock no disponible");
                    }
                    self.custom_presets.push(preset);
                }
            }
            AudioCenterMessage::EqPresetDelete(name) => {
                let is_builtin = crate::audio::preset::EqPreset::default_presets()
                    .iter()
                    .any(|p| p.name == name)
                    && !self.custom_presets.iter().any(|p| p.name == name);
                if is_builtin {
                    if !self.hidden_builtins.contains(&name) {
                        self.hidden_builtins.push(name.clone());
                    }
                } else {
                    // db-lock: persistencia — el guardado se omite si el lock falla
                    if let Ok(db_lock) = db.lock() {
                        if let Err(e) = db_lock.delete_eq_preset_by_name(&name) {
                            tracing::warn!("No se pudo borrar el preset EQ '{}': {}", name, e);
                        }
                    } else {
                        tracing::warn!("db-lock: lock no disponible");
                    }
                    self.custom_presets.retain(|p| p.name != name);
                }
            }
            AudioCenterMessage::EqPresetRestoreDefaults => {
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db_lock) = db.lock() {
                    if let Err(e) = db_lock.clear_all_eq_presets() {
                        tracing::warn!(
                            "No se pudieron limpiar los presets EQ personalizados: {}",
                            e
                        );
                    }
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                self.custom_presets.clear();
                self.hidden_builtins.clear();
            }
            AudioCenterMessage::EqPresetFileImported(Some((path, data))) => {
                // Read file content
                if let Ok(content) = String::from_utf8(data) {
                    match crate::audio::preset::preset_from_json(&content) {
                        Ok(preset) => {
                            // Save to DB
                            // db-lock: persistencia — el guardado se omite si el lock falla
                            if let Ok(db_lock) = db.lock() {
                                let b20 = preset
                                    .bands_20
                                    .as_ref()
                                    .map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect());
                                let b31 = preset
                                    .bands_31
                                    .as_ref()
                                    .map(|v| v.iter().flat_map(|f| f.to_le_bytes()).collect());
                                if let Err(e) = db_lock.save_eq_preset_raw(
                                    &preset.name,
                                    preset.preamp_gain,
                                    b20,
                                    b31,
                                ) {
                                    tracing::warn!(
                                        "No se pudo guardar el preset EQ '{}': {}",
                                        preset.name,
                                        e
                                    );
                                }
                            } else {
                                tracing::warn!("db-lock: lock no disponible");
                            }
                            // Add to custom presets (dedup by name)
                            if !self.custom_presets.iter().any(|p| p.name == preset.name) {
                                self.custom_presets.push(preset.clone());
                            }
                            // Apply the imported preset
                            self.apply_eq_preset_to_state(&preset, audio_manager);
                        }
                        Err(e) => {
                            tracing::error!("Error parsing EQ preset JSON from {:?}: {e}", path);
                        }
                    }
                }
            }
            AudioCenterMessage::EqPresetFileImported(None) => {
                // User cancelled the dialog
            }
            AudioCenterMessage::EqPresetFileExported(Some(path)) => {
                // Extract filename from path to use as preset name (issue #4)
                let file_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Custom")
                    .to_string();
                // Build preset with BOTH band sets at correct sizes (issue #3)
                let bands_20 = if self.equalizer_bands_31 {
                    Some(crate::audio::preset::EqPreset::convert_31_to_20(
                        &self.eq_band_gains,
                    ))
                } else {
                    Some(self.eq_band_gains.clone())
                };
                let bands_31 = if self.equalizer_bands_31 {
                    Some(self.eq_band_gains.clone())
                } else {
                    Some(crate::audio::preset::EqPreset::convert_20_to_31(
                        &self.eq_band_gains,
                    ))
                };
                let preset = crate::audio::preset::EqPreset::new(
                    &file_name,
                    self.preamp_gain,
                    bands_20,
                    bands_31,
                );
                match crate::audio::preset::preset_to_json(&preset) {
                    Ok(json) => {
                        if let Err(e) = std::fs::write(&path, json) {
                            tracing::error!(
                                "No se pudo escribir el preset EQ en {:?}: {}",
                                path,
                                e
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error serializing EQ preset to JSON: {e}");
                    }
                }
            }
            AudioCenterMessage::EqPresetFileExported(None) => {
                // User cancelled the dialog
            }

            // Tab 3: Effects Messages Handlers
            AudioCenterMessage::DspToggle(effect, enabled) => match effect {
                DspEffect::SubBass => {
                    audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.enabled = enabled)
                }
                DspEffect::MidBass => {
                    audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.enabled = enabled)
                }
                DspEffect::VoiceBoost => {
                    audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.enabled = enabled)
                }
                DspEffect::NoiseGate => {
                    audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.enabled = enabled)
                }
                DspEffect::StereoExpander => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.enabled = enabled)
                }
                DspEffect::StereoBalance => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.enabled = enabled)
                }
                DspEffect::Compressor => {
                    audio_manager.with_dsp_mut(|dsp| dsp.compressor.enabled = enabled)
                }
                DspEffect::Limiter => {
                    audio_manager.with_dsp_mut(|dsp| dsp.limiter.enabled = enabled)
                }
                DspEffect::Reverb => audio_manager.with_dsp_mut(|dsp| dsp.reverb.enabled = enabled),
                DspEffect::CompressorIntensity | DspEffect::ReverbRoomSize => {}
            },
            AudioCenterMessage::DspValueChanged(effect, val) => match effect {
                DspEffect::SubBass => audio_manager.with_dsp_mut(|dsp| dsp.sub_bass.gain = val),
                DspEffect::MidBass => audio_manager.with_dsp_mut(|dsp| dsp.mid_bass.gain = val),
                DspEffect::VoiceBoost => {
                    audio_manager.with_dsp_mut(|dsp| dsp.voice_boost.gain = val)
                }
                DspEffect::NoiseGate => {
                    audio_manager.with_dsp_mut(|dsp| dsp.noise_gate.threshold = val)
                }
                DspEffect::StereoExpander => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_expander.width = val)
                }
                DspEffect::StereoBalance => {
                    audio_manager.with_dsp_mut(|dsp| dsp.stereo_balance.balance = val)
                }
                DspEffect::Compressor => {
                    audio_manager.with_dsp_mut(|dsp| dsp.compressor.threshold = val)
                }
                DspEffect::CompressorIntensity => audio_manager.with_dsp_mut(|dsp| {
                    dsp.compressor.intensity = val / 100.0;
                    dsp.compressor.update_intensity_params();
                }),
                DspEffect::Limiter => audio_manager.with_dsp_mut(|dsp| dsp.limiter.ceiling = val),
                DspEffect::Reverb => audio_manager.with_dsp_mut(|dsp| dsp.reverb.set_wet(val)),
                DspEffect::ReverbRoomSize => {
                    audio_manager.with_dsp_mut(|dsp| dsp.reverb.set_room_size(val))
                }
            },
            AudioCenterMessage::AudioStateToggle(toggle, enabled) => {
                let state_arc = audio_manager.state();
                let mut state = state_arc.write();
                match toggle {
                    AudioStateToggle::DownmixCenter => state.downmix_center_enabled = enabled,
                    AudioStateToggle::DownmixLfe => state.downmix_lfe_enabled = enabled,
                    AudioStateToggle::DownmixSurround => state.downmix_surround_enabled = enabled,
                }
            }
            AudioCenterMessage::AudioStateValueChanged(toggle, val) => {
                let state_arc = audio_manager.state();
                let mut state = state_arc.write();
                match toggle {
                    AudioStateToggle::DownmixCenter => state.downmix_center = val,
                    AudioStateToggle::DownmixLfe => state.downmix_lfe = val,
                    AudioStateToggle::DownmixSurround => state.downmix_surround = val,
                }
            }
            AudioCenterMessage::StereoExpanderModeToggled(is_surround) => {
                audio_manager.with_dsp_mut(|dsp| {
                    dsp.stereo_expander.mode = if is_surround {
                        crate::audio::dsp::ExpanderMode::Surround
                    } else {
                        crate::audio::dsp::ExpanderMode::Hybrid
                    };
                });
            }
            AudioCenterMessage::SliderHoverActive(_) => {
                // Manejado en app.rs para AppFocus routing
            }
            // Tab 4: Volumen y Mezcla handlers
            AudioCenterMessage::VolumenFadesToggle(enabled) => {
                audio_manager.state().write().fades_enabled = enabled;
                self.volumen_fades_enabled = enabled;
                // Like the other sub-functions (fade_in/fade_out), the individual
                // smooth_volume_enabled flag is NOT touched here — its state is
                // preserved so re-enabling the group restores it. The decoder gates
                // smoothing on `fades_enabled && smooth_volume_enabled`, so disabling
                // the master alone is enough to stop smoothing.
            }
            AudioCenterMessage::VolumenSmoothVolumeToggle(enabled) => {
                audio_manager.state().write().smooth_volume_enabled = enabled;
                self.volumen_smooth_volume_enabled = enabled;
            }
            AudioCenterMessage::VolumenFadeInChanged(val) => {
                audio_manager.state().write().fade_in_ms = val as f32;
                self.volumen_fade_in_ms = val;
            }
            AudioCenterMessage::VolumenFadeOutChanged(val) => {
                audio_manager.state().write().fade_out_ms = val as f32;
                self.volumen_fade_out_ms = val;
            }
            AudioCenterMessage::VolumenSilenceToggle(enabled) => {
                audio_manager.state().write().silence_enabled = enabled;
                self.volumen_silence_enabled = enabled;
            }
            AudioCenterMessage::VolumenSilenceDurationChanged(val) => {
                audio_manager.state().write().silence_duration_ms = val as f32;
                self.volumen_silence_duration_ms = val;
            }
            AudioCenterMessage::VolumenSilenceThresholdChanged(val) => {
                audio_manager.state().write().silence_threshold_db = val as f32;
                self.volumen_silence_threshold_db = val;
            }
            AudioCenterMessage::VolumenSilenceEdgeTrimToggle(enabled) => {
                audio_manager.state().write().silence_edge_trim_enabled = enabled;
                self.volumen_silence_edge_trim_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgFixedToggle(enabled) => {
                audio_manager.state().write().rg_fixed_enabled = enabled;
                self.volumen_rg_fixed_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgFixedChanged(val) => {
                audio_manager.state().write().rg_fixed_db = val as f32;
                self.volumen_rg_fixed_db = val;
            }
            AudioCenterMessage::VolumenFadeInToggle(enabled) => {
                audio_manager.state().write().fade_in_enabled = enabled;
                self.volumen_fade_in_enabled = enabled;
            }
            AudioCenterMessage::VolumenFadeOutToggle(enabled) => {
                audio_manager.state().write().fade_out_enabled = enabled;
                self.volumen_fade_out_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgMasterToggle(enabled) => {
                audio_manager.state().write().rg_master_enabled = enabled;
                self.volumen_rg_master_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgTrackToggle(enabled) => {
                audio_manager.state().write().replay_gain_track_enabled = enabled;
                self.volumen_rg_track_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgAlbumToggle(enabled) => {
                audio_manager.state().write().replay_gain_album_enabled = enabled;
                self.volumen_rg_album_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgAnalyzeRtToggle(enabled) => {
                audio_manager.state().write().rg_analyze_rt_enabled = enabled;
                self.volumen_rg_analyze_rt_enabled = enabled;
            }
            AudioCenterMessage::VolumenRgOffsetAlbumChanged(val) => {
                audio_manager.state().write().rg_offset_album_db = val as f32;
                self.volumen_rg_offset_album_db = val;
            }
            AudioCenterMessage::VolumenRgOffsetTrackChanged(val) => {
                audio_manager.state().write().rg_offset_track_db = val as f32;
                self.volumen_rg_offset_track_db = val;
            }
            AudioCenterMessage::VolumenRgOffsetRtChanged(val) => {
                audio_manager.state().write().rg_offset_rt_db = val as f32;
                self.volumen_rg_offset_rt_db = val;
            }
            AudioCenterMessage::CrossfadeToggle(enabled) => {
                audio_manager.state().write().crossfade_enabled = enabled;
                self.crossfade_enabled = enabled;
            }
            AudioCenterMessage::CrossfadeManualToggle(enabled) => {
                audio_manager.state().write().crossfade_manual_enabled = enabled;
                self.crossfade_manual_enabled = enabled;
            }
            AudioCenterMessage::CrossfadeManualChanged(val) => {
                audio_manager.state().write().crossfade_manual_ms = val as f32;
                self.crossfade_manual_ms = val;
            }
            AudioCenterMessage::CrossfadeAutoToggle(enabled) => {
                audio_manager.state().write().crossfade_auto_enabled = enabled;
                self.crossfade_auto_enabled = enabled;
            }
            AudioCenterMessage::CrossfadeAutoChanged(val) => {
                audio_manager.state().write().crossfade_auto_ms = val as f32;
                self.crossfade_auto_ms = val;
            }
        }
    }
}
