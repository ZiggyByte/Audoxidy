//! Application shell module root.
//!
//! Holds the module declarations and re-exports, the shared seek predicate,
//! and the inline shell tests. The view composition (with its widget adapter)
//! lives in `view` and the update dispatcher in `update`.

mod init;
mod message;
mod persistence;
mod state;
mod subscription;
pub(in crate::gui::app) mod tick;
mod update;
mod view;

pub use message::{ActiveDialog, AppFocus, ExportFormat, ExportMode, Message};
pub use state::AudoxidyApp;

/// Indica si la búsqueda fina está habilitada para una duración dada.
///
/// La búsqueda fina solo tiene sentido cuando se conoce la duración de la
/// pista: si la duración no puede derivarse (`0.0`), la posición queda
/// indeterminada y cualquier salto debe ignorarse. Un valor negativo o no
/// finito (`NaN`) también devuelve `false`, porque la comparación es falsa.
pub(crate) fn fine_seek_enabled(total_duration: f64) -> bool {
    total_duration.is_finite() && total_duration > 0.0
}

#[cfg(test)]
mod app_tests {
    use super::{
        Message, fine_seek_enabled,
        subscription::external_control_to_message,
        update::memory::{PRELOAD_GC_SAFETY_SECS, gc_blocked_by_preload},
        update::playback::{SEEK_STEP_SECS, clamp_volume, seek_target},
    };
    use crate::integrations::control::ExternalControlEvent;

    #[test]
    fn gc_blocked_while_preload_active() {
        // La pre-carga está activa: el GC no corre sin importar la marca de fin.
        assert!(gc_blocked_by_preload(1000, true, None));
        assert!(gc_blocked_by_preload(1000, true, Some(995)));
    }

    #[test]
    fn gc_blocked_within_safety_window_after_preload() {
        // Dentro de la ventana de 20s posterior al fin de la pre-carga: bloqueado.
        assert!(gc_blocked_by_preload(1019, false, Some(1000)));
        // Justo en el límite (t = 20s): la ventana terminó → el GC puede correr.
        assert!(!gc_blocked_by_preload(1020, false, Some(1000)));
        assert!(!gc_blocked_by_preload(1025, false, Some(1000)));
    }

    #[test]
    fn gc_blocked_by_preload_window_is_20_secs() {
        // El seguro de pre-carga debe ser de 20s (20000ms) tras la mezcla.
        assert_eq!(PRELOAD_GC_SAFETY_SECS, 20);
    }

    #[test]
    fn gc_runs_when_no_preload_state_at_all() {
        // Sin pre-carga activa ni marca de fin: el GC corre normalmente.
        assert!(!gc_blocked_by_preload(1000, false, None));
    }

    #[test]
    fn seek_target_clamps() {
        // Dentro del rango: se aplica el desplazamiento tal cual.
        assert_eq!(seek_target(5.0, 10.0, 100.0), 15.0);
        // Sobrepasa el final: se recorta a la duración total.
        assert_eq!(seek_target(95.0, 10.0, 100.0), 100.0);
        // Retrocede antes del inicio: se recorta a cero.
        assert_eq!(seek_target(2.0, -10.0, 100.0), 0.0);
        // Sin duración conocida: nunca negativo.
        assert_eq!(seek_target(-5.0, 1.0, 0.0), 0.0);
        // Entrada no finita: se ignora y va a cero.
        assert_eq!(seek_target(f64::NAN, 1.0, 100.0), 0.0);
    }

    #[test]
    fn fine_seek_enabled_requires_known_duration() {
        // Duración desconocida o inválida: la búsqueda fina queda deshabilitada.
        assert!(!fine_seek_enabled(0.0));
        assert!(!fine_seek_enabled(-1.0));
        assert!(!fine_seek_enabled(f64::NAN));
        // Duración conocida y finita: la búsqueda fina se habilita.
        assert!(fine_seek_enabled(1.0));
        assert!(fine_seek_enabled(3_600.0));
    }

    #[test]
    fn clamp_volume_bounds() {
        assert_eq!(clamp_volume(0.5), 0.5);
        assert_eq!(clamp_volume(1.5), 1.0);
        assert_eq!(clamp_volume(-2.0), 0.0);
        assert_eq!(clamp_volume(f64::NAN), 0.0);
    }

    #[test]
    fn external_event_mapping() {
        let current = 30.0;
        let total = 100.0;

        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Play, current, total, false),
            Message::SetPlaying(true)
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Pause, current, total, true),
            Message::SetPlaying(false)
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Toggle, current, total, true),
            Message::SetPlaying(false)
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Toggle, current, total, false),
            Message::SetPlaying(true)
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Stop, current, total, true),
            Message::Stop
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Next, current, total, true),
            Message::NextTrack
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::Previous, current, total, true),
            Message::PreviousTrack
        ));
        // Salto fijo: ±SEEK_STEP_SECS, recortado al rango.
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::SeekForward, current, total, true),
            Message::SeekTo(v) if v == (current + SEEK_STEP_SECS) as f32
        ));
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::SeekBackward, current, total, true),
            Message::SeekTo(v) if v == (current - SEEK_STEP_SECS) as f32
        ));
        // Desplazamiento relativo explícito.
        assert!(matches!(
            external_control_to_message(
                ExternalControlEvent::SeekRelative(5.0),
                current,
                total,
                true
            ),
            Message::SeekTo(v) if v == 35.0
        ));
        // Posición absoluta fuera de rango: recortada al total.
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::SeekTo(250.0), current, total, true),
            Message::SeekTo(v) if v == total as f32
        ));
        // Volumen fuera de rango: recortado.
        assert!(matches!(
            external_control_to_message(ExternalControlEvent::SetVolume(1.5), current, total, true),
            Message::VolumeChanged(v) if v == 1.0
        ));
    }
}
