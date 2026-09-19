//! iced subscription and theme wiring for `AudoxidyApp`.

use crate::core::CoreEvent;
use crate::integrations::control::{
    ExternalControlEvent, ExternalControlReceiver, external_control_stream,
};
use iced::Theme;

use super::Message;
use super::state::AudoxidyApp;
use super::update::playback::{SEEK_STEP_SECS, clamp_volume, seek_target};

/// Traduce un evento externo a su mensaje interno equivalente.
///
/// Reutiliza los mensajes existentes para que el comportamiento (mezcla
/// cruzada, persistencia, recarga de carátulas) sea idéntico al de los
/// controles propios de la aplicación.
pub(in crate::gui::app) fn external_control_to_message(
    event: ExternalControlEvent,
    current_pos: f64,
    total_duration: f64,
    is_playing: bool,
) -> Message {
    match event {
        ExternalControlEvent::Play => Message::SetPlaying(true),
        ExternalControlEvent::Pause => Message::SetPlaying(false),
        ExternalControlEvent::Toggle => Message::SetPlaying(!is_playing),
        ExternalControlEvent::Stop => Message::Stop,
        ExternalControlEvent::Next => Message::NextTrack,
        ExternalControlEvent::Previous => Message::PreviousTrack,
        ExternalControlEvent::SeekForward | ExternalControlEvent::SeekBackward => {
            let delta = if matches!(event, ExternalControlEvent::SeekForward) {
                SEEK_STEP_SECS
            } else {
                -SEEK_STEP_SECS
            };
            Message::SeekTo(seek_target(current_pos, delta, total_duration) as f32)
        }
        ExternalControlEvent::SeekRelative(delta) => {
            Message::SeekTo(seek_target(current_pos, delta, total_duration) as f32)
        }
        ExternalControlEvent::SeekTo(target) => {
            Message::SeekTo(seek_target(target, 0.0, total_duration) as f32)
        }
        ExternalControlEvent::SetVolume(volume) => Message::VolumeChanged(clamp_volume(volume)),
    }
}

/// Wrapper around the crossbeam CoreEvent receiver for iced subscription identity.
///
/// The recipe hash must be stable across subscription evaluations to avoid
/// canceling and recreating the bridge thread on every update. Hashing only
/// by `TypeId` achieves this.
#[derive(Clone)]
pub struct CoreEventReceiver(pub crossbeam::channel::Receiver<CoreEvent>);

impl std::hash::Hash for CoreEventReceiver {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::any::TypeId::of::<CoreEvent>().hash(state);
    }
}

/// Adapts the synchronous CoreEvent receiver to an async stream for iced.
///
/// A dedicated bridge thread performs the blocking `recv()` and forwards
/// each event to an unbounded async channel. If the subscription stream
/// is replaced or dropped, `unbounded_send` fails and the bridge thread exits.
fn core_event_stream(
    rx: crossbeam::channel::Receiver<CoreEvent>,
) -> impl iced::futures::Stream<Item = CoreEvent> + Send + 'static {
    let (tx, out) = iced::futures::channel::mpsc::unbounded();

    let spawn_result = std::thread::Builder::new()
        .name("core-event-bridge".into())
        .spawn(move || {
            while let Ok(event) = rx.recv() {
                if tx.unbounded_send(event).is_err() {
                    break;
                }
            }
        });
    if let Err(e) = spawn_result {
        tracing::error!("No se pudo crear el hilo puente de CoreEvent: {e}");
    }

    out
}

impl AudoxidyApp {
    pub fn theme(&self) -> Theme {
        crate::gui::theme::custom_theme()
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        // 1. Tick Standard (Progreso): 500ms
        // 2. Tick Idle (Scanner/Pausa): 5000ms
        let is_scanning = self
            .scanner
            .is_scanning
            .load(std::sync::atomic::Ordering::Relaxed);
        let tick_interval = if self.audio_manager.get_state().is_playing || is_scanning {
            std::time::Duration::from_millis(500)
        } else {
            std::time::Duration::from_millis(5000)
        };
        let mut subs = vec![iced::time::every(tick_interval).map(|_| Message::Tick)];

        // 3. UI/Marquee Tick: 500ms
        // Solo se activa si la UI está activa (controles) o si algún texto requiere efecto marquesina
        let needs_marquee = self.player_ui_state.title_chars.len() > 39
            || self.player_ui_state.artist_chars.len() > 40;

        if self.player_ui_state.is_active
            || (self.audio_manager.get_state().is_playing && needs_marquee)
        {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(500)).map(|_| Message::UITick),
            );
        }

        // 4. Hybrid Auto-scroll: 32ms (solo si se está arrastrando)
        if self.library_manager.is_dragging {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(32)).map(|_| Message::DragTick),
            );
        }

        // 5. VU Meter Tick: 30ms (~33Hz) — smooth meter animation
        //    Active when playing OR fading out (pause fade needs ticks to animate)
        if self.audio_manager.get_state().is_playing || self.player_ui_state.meter.is_fading_out {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(30)).map(|_| Message::MeterTick),
            );
        }

        // 6. Search Pulse: 100ms (solo si hay algo en búsqueda para el Grid)
        if !self.library_manager.search_query.is_empty()
            || !self.filters_manager.search_query.is_empty()
        {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(100))
                    .map(|_| Message::SearchPulse),
            );
        }

        let mouse_evs = iced::event::listen_with(|event, _status, window_id| {
            // Registrar actividad global (Modo Fantasma / Garbage Collector)
            match event {
                iced::Event::Mouse(_) | iced::Event::Keyboard(_) | iced::Event::Touch(_) => {
                    crate::utils::memory_manager::MemoryManager::register_activity();
                }
                _ => {}
            }

            // Capturar el ID de la ventana si aún no lo tenemos
            if let iced::Event::Window(iced::window::Event::Opened { .. }) = event {
                return Some(Message::SetWindowId(window_id));
            }

            if let iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) = event {
                Some(Message::PlayerMouseMoved(position))
            } else if let iced::Event::Mouse(iced::mouse::Event::ButtonPressed(
                iced::mouse::Button::Left,
            )) = event
            {
                if _status == iced::event::Status::Ignored {
                    Some(Message::LibraryMarqueeStart(iced::Point::new(0.0, 0.0)))
                } else {
                    None
                }
            } else if let iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                iced::mouse::Button::Left,
            )) = event
            {
                Some(Message::GlobalMouseRelease)
            } else if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key,
                modifiers,
                ..
            }) = event
            {
                if _status == iced::event::Status::Ignored {
                    Some(Message::GlobalKeyDown(key, modifiers))
                } else {
                    None
                }
            } else if let iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(
                modifiers,
            )) = event
            {
                Some(Message::ModifiersChanged(modifiers))
            } else if let iced::Event::Window(iced::window::Event::Resized(new_size)) = event {
                Some(Message::WindowResized(
                    new_size.width as u32,
                    new_size.height as u32,
                ))
            } else {
                None
            }
        });

        subs.push(mouse_evs);

        // 6. Stepper hold-to-repeat: while a NumberStepper arrow is held
        //    (GLOBAL_STEPPER_HOLD != 0), emit a periodic Tick so iced keeps redrawing
        //    and the widget's RedrawRequested handler drives the auto-repeat with real
        //    timestamps. Without this the runtime stops redrawing (no animation) and
        //    the hold would fire only once.
        if crate::gui::widgets::GLOBAL_STEPPER_HOLD.load(std::sync::atomic::Ordering::Relaxed) != 0
        {
            subs.push(
                iced::time::every(std::time::Duration::from_millis(30)).map(|_| Message::Tick),
            );
        }

        // Eventos externos: controles multimedia del sistema (y, en el futuro,
        // atajos globales). La suscripción se añade siempre y su receta tiene un
        // hash estable, de modo que el runtime mantiene vivo un único stream
        // puente en lugar de recrearlo en cada actualización.
        subs.push(
            iced::Subscription::run_with(
                ExternalControlReceiver(self.external_control_rx.clone()),
                |rx| external_control_stream(rx.0.clone()),
            )
            .map(Message::ExternalControl),
        );

        // CoreEvent bridge: edge-transition events from the Core domain layer.
        // Same hash-stable recipe pattern as the external control bridge.
        subs.push(
            iced::Subscription::run_with(CoreEventReceiver(self.core_event_rx.clone()), |rx| {
                core_event_stream(rx.0.clone())
            })
            .map(Message::CoreEvent),
        );

        iced::Subscription::batch(subs)
    }
}
