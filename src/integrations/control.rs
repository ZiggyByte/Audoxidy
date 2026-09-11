//! Canal de eventos externos de control de reproducción.
//!
//! Los controles multimedia del sistema (MPRIS/SMTC) y, en el futuro, los
//! atajos globales de teclado son productores de eventos ajenos al hilo
//! principal. Todos escriben en un único canal tipado cuyo receptor es
//! propiedad de la aplicación, de modo que ésta sea la única responsable de
//! traducir cada evento a los mensajes internos de reproducción.

/// Acción de control originada fuera de la interfaz.
///
/// `SeekForward`/`SeekBackward` representan el salto fijo (10 s) que emiten
/// los eventos `Seek` sin desplazamiento explícito. `SeekRelative` lleva el
/// desplazamiento firmado en segundos que proporciona `SeekBy`, para no
/// descartar la duración solicitada por el cliente MPRIS/SMTC.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExternalControlEvent {
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Previous,
    SeekForward,
    SeekBackward,
    SeekRelative(f64),
    SeekTo(f64),
    SetVolume(f64),
}

/// Emisor no bloqueante del canal de eventos externos.
///
/// Se consume desde hilos ajenos al runtime de la interfaz (D-Bus, SMTC o un
/// futuro listener de atajos), por lo que `send` nunca bloquea ni entra en
/// pánico: un receptor ausente solo registra una advertencia.
#[derive(Clone)]
pub struct ExternalControlSender(crossbeam::channel::Sender<ExternalControlEvent>);

impl ExternalControlSender {
    /// Envía un evento sin bloquear ni entrar en pánico si no hay receptor.
    pub fn send(&self, event: ExternalControlEvent) {
        if let Err(e) = self.0.send(event) {
            tracing::warn!("canal de control externo desconectado; evento descartado: {e}");
        }
    }
}

/// Crea el canal de eventos externos junto con su emisor.
///
/// El canal es ilimitado para que los eventos que lleguen antes del primer
/// sondeo de la suscripción de la interfaz queden en búfer en lugar de
/// perderse. El emisor envuelve el envío: si el receptor ya no existe, el
/// error se registra y se descarta el evento en vez de propagar un pánico.
pub fn external_control_channel() -> (
    ExternalControlSender,
    crossbeam::channel::Receiver<ExternalControlEvent>,
) {
    let (tx, rx) = crossbeam::channel::unbounded();
    (ExternalControlSender(tx), rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_round_trip() {
        let (tx, rx) = external_control_channel();
        tx.send(ExternalControlEvent::Next);
        assert_eq!(rx.recv().ok(), Some(ExternalControlEvent::Next));

        // Sin emisor el receptor queda desconectado: el siguiente recv es Err.
        drop(tx);
        assert!(rx.recv().is_err());
    }

    #[test]
    fn event_is_send_and_clone() {
        let event = ExternalControlEvent::SeekTo(12.5);
        let clone = event;
        let handle = std::thread::spawn(move || clone);
        assert_eq!(handle.join().ok(), Some(ExternalControlEvent::SeekTo(12.5)));
    }
}
