//! Tick, meter and periodic message handlers extracted from the update dispatcher.

use crate::audio::engine::{LATENCY_PEAK_US, UNDERRUNS};
use crate::gui::app::Message;
use crate::gui::app::persistence::log_persist;
use crate::gui::app::update::memory::gc_blocked_by_preload;
use crate::gui::app::update::meter::{METER_FADE_OUT_SECS, ballistic_step, peak_hold_step};
use iced::Task;

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_ticks(
        &mut self,
        message: Message,
        state: &crate::audio::engine::AudioState,
    ) -> Task<Message> {
        match message {
            Message::Tick => {
                // 0. Refrescar caché de estado del sistema de audio (cada 3s, no cada frame)
                if self.audio_center_manager.open {
                    self.audio_center_manager.refresh_system_status_cache();
                }

                // 1. Consultar al scanner si hay datos nuevos en la BD
                if self
                    .scanner
                    .db_dirty
                    .swap(false, std::sync::atomic::Ordering::Relaxed)
                {
                    self.db_needs_refresh = true;
                }

                // 2. Actualizar BD SOLO cuando el escáner marca cambios
                if self.db_needs_refresh {
                    self.db_needs_refresh = false;
                    let db_arc = self.database.clone();
                    return Task::perform(
                        async move {
                            // db-lock: solo lectura — refresco best-effort, se omite si el lock falla
                            if let Ok(db) = db_arc.lock() {
                                let songs = db.get_all_songs().unwrap_or_default();
                                let albums = db.get_grid_items_by_artist().unwrap_or_default();
                                let stats = db.get_library_stats().unwrap_or((0, 0, 0.0, 0.0, 0));
                                (songs, albums, stats)
                            } else {
                                (Vec::new(), Vec::new(), (0, 0, 0.0, 0.0, 0))
                            }
                        },
                        |(songs, albums, stats)| {
                            Message::InternalLibraryRefreshed(songs, albums, stats)
                        },
                    );
                }

                // 3. Sincronización de carátulas solo si hay reproducción activa
                if state.is_playing {
                    self.sync_player_art();

                    let duration = state.total_duration_sec;
                    let position = state.current_pos_sec;
                    if duration > 0.0 {
                        let remaining = duration - position;
                        // Prefetch de carátula y pre-carga de la siguiente canción
                        // ~15s antes del final (transiciones sin cortes). El dedup
                        // por handle/path evita repetir el trabajo en cada tick.
                        if remaining <= 15.0 {
                            if let Some(next_song) = self.playlist_manager.get_next_song_ref() {
                                if let Some(ref cover_path) = next_song.cover_path {
                                    if self.player_ui_state.prefetched_next_handle.is_none()
                                        && self.player_ui_state.current_cover_path
                                            != cover_path.as_ref()
                                    {
                                        self.player_ui_state.prefetched_next_handle =
                                            crate::utils::covers::load_cover_handle(cover_path);
                                    }
                                }
                                // Pre-cargar la siguiente canción una sola vez por pista.
                                if self.player_ui_state.preloaded_next_path.as_deref()
                                    != Some(next_song.file_path.as_ref())
                                {
                                    let _ = self.audio_manager.preload_next(
                                        &next_song.file_path,
                                        next_song.title.to_string(),
                                        next_song.artist_name.to_string(),
                                        next_song.album_title.to_string(),
                                        next_song.cover_path.as_deref().map(str::to_string),
                                        None,
                                        None,
                                    );
                                    self.player_ui_state.preloaded_next_path =
                                        Some(next_song.file_path.to_string());
                                    // La pre-carga está activa: el GC no debe correr.
                                    self.player_ui_state.preload_finished_at = None;
                                }
                            }
                        }
                    }
                }

                if state.eof_reached {
                    self.audio_manager.clear_eof();
                    self.playlist_manager.play_next(&self.audio_manager);
                    // Nueva pista activa: restablecer la pre-carga para la siguiente y
                    // marcar el fin de la pre-carga (el GC se reactiva 20s después).
                    self.player_ui_state.preloaded_next_path = None;
                    self.player_ui_state.preload_finished_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .ok();

                    if self.playlist_manager.shuffle_active {
                        if let Some(session) = &self.playlist_manager.shuffle_session {
                            // db-lock: persistencia — el guardado se omite si el lock falla
                            if let Ok(mut db) = self.database.lock() {
                                log_persist(
                                    "shuffle_session",
                                    db.save_shuffle_session(
                                        self.playlist_manager.active_playlist_id,
                                        session,
                                    ),
                                );
                            } else {
                                tracing::warn!("db-lock: lock no disponible");
                            }
                        }
                    }

                    self.sync_player_art();
                    self.persist_playlist_state();

                    if !self.is_mouse_over_playlist {
                        if let Some(idx) = self.playlist_manager.playing_song_idx {
                            return self.execute_playlist_autoscroll(idx);
                        }
                    }
                }

                // 4. Actualizar Caché de caracteres y textos base (para detección inteligente)
                if state.title != self.player_ui_state.last_title {
                    self.player_ui_state.last_title = state.title.clone();
                    self.player_ui_state.title_chars = state.title.chars().collect();
                    if self.player_ui_state.title_chars.len() <= 39 {
                        self.player_ui_state.display_title = state.title.clone();
                    }
                }
                if state.artist != self.player_ui_state.last_artist {
                    self.player_ui_state.last_artist = state.artist.clone();
                    self.player_ui_state.artist_chars = state.artist.chars().collect();
                    if self.player_ui_state.artist_chars.len() <= 40 {
                        self.player_ui_state.display_artist = state.artist.clone();
                    }
                }

                // Auto-guardado de persistencia cada ~20 segundos (40 ticks de 500ms)
                if self.player_ui_state.tick_count % 40 == 0 {
                    self.persist_playlist_state();
                }

                // Emisión de métricas desde la ruta no-RT de la GUI: convierte los
                // contadores atómicos publicados por el callback en eventos
                // estructurados que consume la capa de métricas.
                let ram_bytes =
                    crate::utils::memory_manager::MemoryManager::current_process_ram_bytes();
                tracing::info!(
                    target: "audoxidy::metrics",
                    metric = "ram_usage_bytes",
                    ram_usage_bytes = ram_bytes
                );

                let underruns = UNDERRUNS.swap(0, std::sync::atomic::Ordering::Relaxed);
                if underruns > 0 {
                    tracing::warn!(
                        target: "audoxidy::metrics",
                        metric = "underrun",
                        count = underruns
                    );
                }

                let peak = LATENCY_PEAK_US.load(std::sync::atomic::Ordering::Relaxed);
                if peak > 0 {
                    tracing::info!(
                        target: "audoxidy::metrics",
                        metric = "audio_latency",
                        latency_us = peak
                    );
                }

                // 5a. Hard Cap de RAM: si supera el 75%, purgar inmediatamente
                if crate::utils::memory_manager::MemoryManager::is_ram_over_hard_cap() {
                    tracing::debug!("Audoxidy GC: RAM over 75% hard cap — forcing immediate purge");
                    return Task::done(Message::GlobalMemoryPurge);
                }

                // 5b. Purga por timer o por umbral de RAM del propio reproductor.
                // Nunca durante un escaneo (extracción de metadatos y carátulas).
                // La purga por RAM tiene cooldown (10s) para evitar el bucle
                // GC → re-creación de pre-carga → GC cuando la RAM sigue alta.
                let is_scanning_now = self
                    .scanner
                    .is_scanning
                    .load(std::sync::atomic::Ordering::Relaxed);

                // Seguro de pre-carga: el GC no corre mientras la pre-carga está activa
                // ni en los 20s posteriores a su fin (evita interrumpir la transición
                // de canciones y la mezcla cruzada). Es el mismo patrón que el seguro
                // de escaneo.
                let now_secs = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let preload_active = self.player_ui_state.preloaded_next_path.is_some();
                let preload_finished_recently = gc_blocked_by_preload(
                    now_secs,
                    preload_active,
                    self.player_ui_state.preload_finished_at,
                );

                if !is_scanning_now && !preload_finished_recently {
                    let purge_timer =
                        crate::utils::memory_manager::MemoryManager::should_run_global_purge(
                            2,
                            is_scanning_now,
                        );
                    let ram_over =
                        crate::utils::memory_manager::MemoryManager::should_purge_for_ram();
                    if purge_timer || ram_over {
                        return Task::done(Message::GlobalMemoryPurge);
                    }
                }

                // Detección de fin de escáner para liberar memoria 30s después
                if self.was_scanning && !is_scanning_now {
                    self.scan_finished_at = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    );
                }
                self.was_scanning = is_scanning_now;

                if let Some(finished_at) = self.scan_finished_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    // Liberar memoria 40s después del escaneo
                    if now.saturating_sub(finished_at) >= 40 {
                        self.scan_finished_at = None;
                        tracing::debug!("Audoxidy GC: Scan complete, clearing memory");
                        crate::utils::covers::purge_old_covers(32); // Vaciar caché de covers generado por escáner

                        // Reiniciar el temporizador global para que no haga otra purga en 2 mins
                        crate::utils::memory_manager::MemoryManager::reset_global_purge_timer();

                        return Task::done(Message::GlobalMemoryPurge);
                    }
                }

                Task::none()
            }
            Message::SearchPulse => Task::none(),
            Message::DragTick => {
                if self.library_manager.is_dragging {
                    if let Some(pos) = self.player_ui_state.mouse_pos {
                        let win_h = self.window_size.1 as f32;
                        let threshold_top = 150.0;
                        let threshold_bottom = win_h - 100.0;

                        let mut scroll_delta = 0.0;
                        if pos.y < threshold_top {
                            scroll_delta = (pos.y - threshold_top) / 5.0;
                        } else if pos.y > threshold_bottom {
                            scroll_delta = (pos.y - threshold_bottom) / 5.0;
                        }

                        if scroll_delta != 0.0 {
                            let scroll_id = crate::gui::library::LIBRARY_SCROLL_ID.clone();
                            return iced::widget::operation::scroll_by(
                                scroll_id,
                                iced::widget::scrollable::AbsoluteOffset {
                                    x: 0.0,
                                    y: scroll_delta,
                                },
                            );
                        }
                    }
                }
                Task::none()
            }
            Message::MeterTick => {
                self.update_meter();
                return Task::none();
            }
            Message::UITick => {
                self.player_ui_state.tick_count = self.player_ui_state.tick_count.wrapping_add(1);

                let state = self.audio_manager.get_state();

                // Cálculo de marquesina cada 500ms
                let limit_t = 39;
                let limit_a = 40;

                self.player_ui_state.display_title = if self.player_ui_state.title_chars.len()
                    > limit_t
                {
                    let chars = &self.player_ui_state.title_chars;
                    let offset = (self.player_ui_state.tick_count as usize) % (chars.len() + 10);
                    if offset < chars.len() {
                        let end = (offset + limit_t).min(chars.len());
                        let mut s: String = chars[offset..end].iter().collect();
                        if offset + limit_t > chars.len() {
                            s.push_str("   ");
                            let needed = (offset + limit_t) - chars.len();
                            if needed > 3 {
                                let rem = needed - 3;
                                s.push_str(
                                    &chars[0..rem.min(chars.len())].iter().collect::<String>(),
                                );
                            }
                        }
                        s
                    } else {
                        chars[0..limit_t.min(chars.len())].iter().collect()
                    }
                } else {
                    state.title.clone()
                };

                self.player_ui_state.display_artist = if self.player_ui_state.artist_chars.len()
                    > limit_a
                {
                    let chars = &self.player_ui_state.artist_chars;
                    let offset = (self.player_ui_state.tick_count as usize) % (chars.len() + 10);
                    if offset < chars.len() {
                        let end = (offset + limit_a).min(chars.len());
                        let mut s: String = chars[offset..end].iter().collect();
                        if offset + limit_a > chars.len() {
                            s.push_str("   ");
                            let needed = (offset + limit_a) - chars.len();
                            if needed > 3 {
                                let rem = needed - 3;
                                s.push_str(
                                    &chars[0..rem.min(chars.len())].iter().collect::<String>(),
                                );
                            }
                        }
                        s
                    } else {
                        chars[0..limit_a.min(chars.len())].iter().collect()
                    }
                } else {
                    state.artist.clone()
                };

                if self.player_ui_state.is_active
                    && self.player_ui_state.tick_count > self.player_ui_state.active_until_tick
                {
                    self.player_ui_state.is_active = false;
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }

    /// Lee los 4 atómicos del data layer (MeterData), aplica ballistics,
    /// peak hold, pause fade y track reset, y escribe en
    /// `player_ui_state.meter`. Incluye cómputo M/S, correlación de fase,
    /// crest factor y hold infinito.
    fn update_meter(&mut self) {
        let state = self.audio_manager.get_state();
        let is_playing = state.is_playing;
        let current_path = state.path.clone();
        let infinite_hold = state.meter_infinite_hold;

        let m = &mut self.player_ui_state.meter;

        // Track change reset — also resets infinite hold values
        if current_path != m.last_track_path {
            let ms_mode = m.ms_mode;
            let show_numeric = m.show_numeric;
            *m = crate::gui::player::MeterUiState::default();
            m.last_track_path = current_path;
            m.ms_mode = ms_mode;
            m.show_numeric = show_numeric;
            return;
        }

        // Read atomics from data layer
        let meter_data = self.audio_manager.meter();
        let raw_peak_l = meter_data.read_peak_l();
        let raw_peak_r = meter_data.read_peak_r();
        let raw_rms_l = meter_data.read_rms_l();
        let raw_rms_r = meter_data.read_rms_r();

        // ── M/S computation: Mid = (L+R)/2, Side = (L-R)/2 ──
        let (display_peak_l, display_peak_r, display_rms_l, display_rms_r) = if m.ms_mode {
            let mid_peak = (raw_peak_l + raw_peak_r) / 2.0;
            let side_peak = (raw_peak_l - raw_peak_r) / 2.0;
            let mid_rms = (raw_rms_l + raw_rms_r) / 2.0;
            let side_rms = (raw_rms_l - raw_rms_r) / 2.0;
            (mid_peak, side_peak, mid_rms, side_rms)
        } else {
            (raw_peak_l, raw_peak_r, raw_rms_l, raw_rms_r)
        };

        if is_playing {
            m.is_fading_out = false;

            // Peak hold state machine per channel
            if infinite_hold {
                // Infinite hold: hold values only increase, never decay
                m.peak_hold_l = m.peak_hold_l.max(display_peak_l);
                m.peak_hold_r = m.peak_hold_r.max(display_peak_r);
                m.peak_hold_start_l = None;
                m.peak_hold_start_r = None;
                m.peak_decay_l = 0.0;
                m.peak_decay_r = 0.0;
            } else {
                // Normal hold: existing hold timer + decay logic
                let (new_hold_l, new_start_l, new_decay_l) = peak_hold_step(
                    m.peak_hold_l,
                    m.peak_hold_start_l,
                    m.peak_decay_l,
                    display_peak_l,
                );
                m.peak_hold_l = new_hold_l;
                m.peak_hold_start_l = new_start_l;
                m.peak_decay_l = new_decay_l;

                let (new_hold_r, new_start_r, new_decay_r) = peak_hold_step(
                    m.peak_hold_r,
                    m.peak_hold_start_r,
                    m.peak_decay_r,
                    display_peak_r,
                );
                m.peak_hold_r = new_hold_r;
                m.peak_hold_start_r = new_start_r;
                m.peak_decay_r = new_decay_r;
            }

            // Asymmetric ballistics — target is max of raw peak and held peak
            let target_l = display_peak_l.max(m.peak_hold_l);
            let target_r = display_peak_r.max(m.peak_hold_r);
            m.display_l = ballistic_step(m.display_l, target_l);
            m.display_r = ballistic_step(m.display_r, target_r);

            // ── Phase correlation: corr = (L·R) / (|L|·|R|) ──
            // Use RMS-weighted correlation for smoother readout
            m.phase_corr = if raw_rms_l.abs() > 0.001 && raw_rms_r.abs() > 0.001 {
                let corr = (raw_rms_l * raw_rms_r) / (raw_rms_l.abs() * raw_rms_r.abs());
                corr.clamp(-1.0, 1.0)
            } else {
                0.0 // Undefined for silence
            };

            // ── Crest factor: peak_dBFS - RMS_dBFS ──
            m.crest_l = if raw_rms_l > crate::gui::widgets::METER_MIN_DB + 1.0 {
                raw_peak_l - raw_rms_l
            } else {
                0.0
            };
            m.crest_r = if raw_rms_r > crate::gui::widgets::METER_MIN_DB + 1.0 {
                raw_peak_r - raw_rms_r
            } else {
                0.0
            };
        } else if !m.is_fading_out {
            // Transition to pause — capture current display values
            m.last_playing_l = m.display_l;
            m.last_playing_r = m.display_r;
            m.is_fading_out = true;
            m.fade_out_start = Some(std::time::Instant::now());
        }
        // Note: fade-out animation continues even when !is_playing,
        // because the subscription stays active while is_fading_out == true.

        // Pause fade animation
        if m.is_fading_out {
            if let Some(start) = m.fade_out_start {
                let elapsed = start.elapsed().as_secs_f32();
                let fade_secs = METER_FADE_OUT_SECS;
                let factor = (1.0 - elapsed / fade_secs).clamp(0.0, 1.0);
                m.display_l = m.last_playing_l * factor;
                m.display_r = m.last_playing_r * factor;
                if factor <= 0.0 {
                    m.is_fading_out = false;
                    m.display_l = 0.0;
                    m.display_r = 0.0;
                }
            }
        }
    }
}
