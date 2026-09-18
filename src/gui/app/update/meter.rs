//! VU-meter popup message handlers extracted from the update dispatcher.

use crate::gui::app::{Message, persistence::log_persist};
use iced::Task;

/// Duración del hold de pico antes de comenzar el decaimiento (segundos).
pub(in crate::gui::app) const METER_HOLD_TIME_SECS: f32 = 1.5;
/// Tasa de decaimiento del hold de pico (dB por segundo).
pub(in crate::gui::app) const METER_DECAY_RATE_DB_PER_SEC: f32 = 1.8;
/// Coeficiente de suavizado de ataque (rise rápido, ~5 ms a 30 ms de tick).
pub(in crate::gui::app) const METER_ATTACK_ALPHA: f32 = 0.85;
/// Coeficiente de suavizado de release (caída lenta, ~600 ms a 30 ms de tick).
pub(in crate::gui::app) const METER_RELEASE_ALPHA: f32 = 0.05;
/// Duración del fade-out al pausar (segundos).
pub(in crate::gui::app) const METER_FADE_OUT_SECS: f32 = 0.4;

/// Paso de la máquina de estados de hold de pico.
///
/// Retorna `(new_hold_value, new_hold_start, new_decay_value)`.
pub(in crate::gui::app) fn peak_hold_step(
    hold_value: f32,
    hold_start: Option<std::time::Instant>,
    decay_value: f32,
    raw_peak: f32,
) -> (f32, Option<std::time::Instant>, f32) {
    let (mut new_hold, mut new_start, mut new_decay) = (hold_value, hold_start, decay_value);

    if let Some(start) = new_start {
        // Estado Holding — el hold está activo
        if start.elapsed().as_secs_f32() > METER_HOLD_TIME_SECS {
            // Transición Holding → Decaying
            new_decay = new_hold;
            new_start = None;
        }
    }

    if new_start.is_none() && new_decay > 0.0 {
        // Estado Decaying — decaer a METER_DECAY_RATE_DB_PER_SEC por tick
        new_decay -= METER_DECAY_RATE_DB_PER_SEC * 0.030;
        if new_decay <= raw_peak {
            // Transición Decaying → Idle
            new_decay = 0.0;
        }
    }

    // Si la señal supera el hold actual, snap hacia arriba (re-entrar Holding)
    if raw_peak > new_hold {
        new_hold = raw_peak;
        new_start = Some(std::time::Instant::now());
        new_decay = 0.0;
    } else if new_start.is_none() && new_decay <= 0.0 {
        // Idle — el hold sigue al nivel del decay o es cero
        new_hold = raw_peak.max(new_decay);
    }

    (new_hold, new_start, new_decay)
}

/// Paso de ballistics asimétrico: ataque rápido, release lento.
pub(in crate::gui::app) fn ballistic_step(current: f32, target: f32) -> f32 {
    let alpha = if target > current {
        METER_ATTACK_ALPHA
    } else {
        METER_RELEASE_ALPHA
    };
    current + alpha * (target - current)
}

impl crate::gui::app::AudoxidyApp {
    pub(in crate::gui::app) fn handle_meter(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MeterRightClicked(pos) => {
                self.player_ui_state.meter_popup_open = true;
                self.player_ui_state.meter_popup_pos = Some(pos);
                Task::none()
            }
            Message::MeterPopupClose => {
                self.player_ui_state.meter_popup_open = false;
                Task::none()
            }
            Message::MeterHoldTimeChanged(v) => {
                let clamped = v.clamp(500.0, 5000.0);
                {
                    let state_arc = self.audio_manager.state();
                    let mut s = state_arc.write();
                    s.meter_hold_time_ms = clamped;
                }
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db_lock) = self.database.lock() {
                    log_persist(
                        "meter_hold_time_ms",
                        db_lock.set_setting("meter_hold_time_ms", &format!("{:.0}", clamped)),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                self.player_ui_state.meter_popup_open = false;
                Task::none()
            }
            Message::MeterRmsWindowChanged(v) => {
                let clamped = v.clamp(50.0, 1000.0);
                {
                    let state_arc = self.audio_manager.state();
                    let mut s = state_arc.write();
                    s.meter_rms_window_ms = clamped;
                }
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db_lock) = self.database.lock() {
                    log_persist(
                        "meter_rms_window_ms",
                        db_lock.set_setting("meter_rms_window_ms", &format!("{:.0}", clamped)),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                self.player_ui_state.meter_popup_open = false;
                Task::none()
            }
            Message::MeterMsModeToggle(v) => {
                self.player_ui_state.meter.ms_mode = v;
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db_lock) = self.database.lock() {
                    log_persist(
                        "meter_ms_mode",
                        db_lock.set_setting("meter_ms_mode", if v { "1" } else { "0" }),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                self.player_ui_state.meter_popup_open = false;
                Task::none()
            }
            Message::MeterInfiniteHoldToggle(v) => {
                {
                    let state_arc = self.audio_manager.state();
                    let mut s = state_arc.write();
                    s.meter_infinite_hold = v;
                }
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db_lock) = self.database.lock() {
                    log_persist(
                        "meter_infinite_hold",
                        db_lock.set_setting("meter_infinite_hold", if v { "1" } else { "0" }),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                self.player_ui_state.meter_popup_open = false;
                Task::none()
            }
            Message::MeterResetPeak => {
                self.player_ui_state.meter.peak_hold_l = crate::gui::widgets::METER_MIN_DB;
                self.player_ui_state.meter.peak_hold_r = crate::gui::widgets::METER_MIN_DB;
                self.player_ui_state.meter.peak_hold_start_l = None;
                self.player_ui_state.meter.peak_hold_start_r = None;
                self.player_ui_state.meter.peak_decay_l = 0.0;
                self.player_ui_state.meter.peak_decay_r = 0.0;
                Task::none()
            }
            Message::MeterToggleEnabled(v) => {
                {
                    let state_arc = self.audio_manager.state();
                    let mut s = state_arc.write();
                    s.meter_enabled = v;
                }
                // db-lock: persistencia — el guardado se omite si el lock falla
                if let Ok(db_lock) = self.database.lock() {
                    log_persist(
                        "meter_enabled",
                        db_lock.set_setting("meter_enabled", if v { "1" } else { "0" }),
                    );
                } else {
                    tracing::warn!("db-lock: lock no disponible");
                }
                Task::none()
            }
            _ => Task::none(),
        }
    }
}
