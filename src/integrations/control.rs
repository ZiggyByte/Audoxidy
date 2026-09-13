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

/// Receptor envuelto para usarse como identidad de una suscripción de iced.
///
/// La identidad de la receta (`Hash`) debe ser constante: si cambiara entre
/// evaluaciones de `subscription()`, el runtime cancelaría y volvería a
/// arrancar el stream, creando un hilo puente nuevo en cada actualización.
/// Por eso el hash solo incluye el `TypeId` del evento y nunca el estado del
/// canal.
#[derive(Clone)]
pub struct ExternalControlReceiver(pub crossbeam::channel::Receiver<ExternalControlEvent>);

impl std::hash::Hash for ExternalControlReceiver {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::any::TypeId::of::<ExternalControlEvent>().hash(state);
    }
}

/// Adapta el receptor síncrono a un stream consumible por una suscripción de iced.
///
/// Un hilo puente dedicado se encarga del `recv()` bloqueante y reenvía cada
/// evento al canal asíncrono ilimitado; así ningún worker de tokio queda
/// bloqueado esperando eventos. Si el stream de la suscripción se reemplaza o
/// se descarta, `unbounded_send` falla y el hilo puente termina.
pub fn external_control_stream(
    rx: crossbeam::channel::Receiver<ExternalControlEvent>,
) -> impl iced::futures::Stream<Item = ExternalControlEvent> + Send + 'static {
    let (tx, out) = iced::futures::channel::mpsc::unbounded();

    // Si el hilo no puede crearse, `tx` se descarta con él y `out` termina:
    // no se propaga un pánico desde el camino de la suscripción. El fallo se
    // registra a nivel error porque el puente de control externo quedaría muerto.
    let spawn_result = std::thread::Builder::new()
        .name("ext-control-bridge".into())
        .spawn(move || {
            while let Ok(event) = rx.recv() {
                if tx.unbounded_send(event).is_err() {
                    // El stream de la suscripción fue reemplazado/dropeado.
                    break;
                }
            }
        });
    if let Err(e) = spawn_result {
        tracing::error!("No se pudo crear el hilo puente de control externo: {e}");
    }

    out
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

    #[test]
    fn receiver_hash_is_stable() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let (_tx_a, rx_a) = crossbeam::channel::unbounded();
        let (_tx_b, rx_b) = crossbeam::channel::unbounded();

        let mut hasher_clone = DefaultHasher::new();
        ExternalControlReceiver(rx_a.clone()).hash(&mut hasher_clone);
        let mut hasher_same = DefaultHasher::new();
        ExternalControlReceiver(rx_a).hash(&mut hasher_same);
        let mut hasher_other = DefaultHasher::new();
        ExternalControlReceiver(rx_b).hash(&mut hasher_other);

        assert_eq!(hasher_clone.finish(), hasher_same.finish());
        assert_eq!(hasher_clone.finish(), hasher_other.finish());
    }
}
