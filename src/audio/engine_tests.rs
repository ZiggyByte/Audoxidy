//! Pure engine tests for the output callback, the command channel and the stream metadata.
//!
//! Every test drives a helper the engine uses from its own side: a ring buffer it
//! fills and drains by hand, a bounded command channel, and the atomics that carry a
//! seek and the stream metadata. No audio device is opened and no stream is built, so
//! the engine's decisions are pinned without audio hardware. The single thread here is
//! the one test-local sender that proves a full control queue delays the blocking send;
//! the decoder thread and the worker pools are never started.

use super::*;
use ringbuf::traits::{Observer, Producer};

/// El callback publica el tamaño de buffer medido en frames y copia la
/// señal del anillo a la salida.
#[test]
fn buffer_size_published_via_atomic() {
    let rb = HeapRb::<f32>::new(1024);
    let (mut producer, consumer) = rb.split();
    let samples = [0.1f32, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
    assert_eq!(producer.push_slice(&samples), samples.len());

    let buffer_size = Arc::new(AtomicU32::new(0));
    let is_playing = Arc::new(AtomicBool::new(true));
    let consumer_mutex = Arc::new(Mutex::new(Some(consumer)));

    let mut out = [0.0f32; 8];
    AudioEngine::write_data_impl(&mut out, 2, &buffer_size, &is_playing, &consumer_mutex);

    assert_eq!(buffer_size.load(Ordering::Relaxed), 4);
    for (dst, src) in out.iter().zip(samples.iter()) {
        assert_eq!(dst, src);
    }
}

/// Con `is_playing` en falso el callback entrega silencio sin consumir el anillo.
#[test]
fn silence_when_not_playing() {
    let rb = HeapRb::<f32>::new(1024);
    let (mut producer, consumer) = rb.split();
    let samples = [1.0f32; 8];
    assert_eq!(producer.push_slice(&samples), samples.len());

    let buffer_size = Arc::new(AtomicU32::new(0));
    let is_playing = Arc::new(AtomicBool::new(false));
    let consumer_mutex = Arc::new(Mutex::new(Some(consumer)));

    let mut out = [1.0f32; 8];
    AudioEngine::write_data_impl(&mut out, 2, &buffer_size, &is_playing, &consumer_mutex);

    assert!(out.iter().all(|s| *s == 0.0));
    let guard = consumer_mutex.lock();
    assert_eq!(guard.as_ref().unwrap().occupied_len(), samples.len());
}

/// Un comando de control nunca se pierde: con la cola llena el envío
/// bloqueante espera a que el receptor libere espacio y luego lo entrega.
#[test]
fn control_command_survives_full_channel() {
    let (tx, rx) = bounded::<AudioCommand>(1);
    assert!(tx.try_send(AudioCommand::ClearPreload).is_ok());

    let tx_control = tx.clone();
    let sender = std::thread::spawn(move || {
        send_control(&tx_control, AudioCommand::Stop);
    });

    // Liberar un hueco desbloquea el envío de control.
    assert!(matches!(rx.recv(), Ok(AudioCommand::ClearPreload)));
    assert!(matches!(rx.recv(), Ok(AudioCommand::Stop)));
    sender.join().expect("control sender thread panicked");
}

/// Una pre-carga es best-effort: con la cola llena se descarta sin bloquear.
#[test]
fn preload_command_dropped_when_full() {
    let (tx, rx) = bounded::<AudioCommand>(1);
    assert!(tx.try_send(AudioCommand::ClearPreload).is_ok());

    send_preload(&tx, AudioCommand::CrossfadeNext(5.0));

    assert!(matches!(rx.try_recv(), Ok(AudioCommand::ClearPreload)));
    assert!(rx.try_recv().is_err());
}

/// El slot de búsqueda guarda solo la posición más reciente y se limpia al tomarla.
#[test]
fn latest_seek_wins_and_clears() {
    let slot = AtomicU64::new(NO_SEEK);
    store_latest_seek(&slot, 1.0);
    store_latest_seek(&slot, 2.0);
    assert_eq!(take_latest_seek(&slot), Some(2.0));
    assert_eq!(take_latest_seek(&slot), None);

    store_latest_seek(&slot, f64::NAN);
    store_latest_seek(&slot, f64::INFINITY);
    assert_eq!(take_latest_seek(&slot), None);
}

/// La contabilidad del stream publica canales, buffer, id y profundidad de
/// bits; sin ella el decodificador llena el anillo con un layout que no
/// coincide con el del callback.
#[test]
fn stream_metadata_publishes_channels_buffer_and_bit_depth() {
    let mut state = AudioState::default();
    store_stream_metadata(
        &mut state,
        96_000,
        6,
        cpal::BufferSize::Fixed(512),
        cpal::SampleFormat::I32,
        Some("alsa:card".to_string()),
    );
    assert_eq!(state.device_sample_rate, 96_000);
    assert_eq!(state.channels, 6);
    assert_eq!(state.buffer_size, 512);
    assert_eq!(state.bit_depth_display, "24/32-bit Int");
    assert_eq!(state.device_id.as_deref(), Some("alsa:card"));

    // Sin buffer fijo se publica 0; el formato define la etiqueta.
    store_stream_metadata(
        &mut state,
        48_000,
        2,
        cpal::BufferSize::Default,
        cpal::SampleFormat::F32,
        None,
    );
    assert_eq!(state.channels, 2);
    assert_eq!(state.buffer_size, 0);
    assert_eq!(state.bit_depth_display, "32-bit Float");
    assert_eq!(state.device_id, None);
}

/// El drenado vacía el anillo completo aunque supere el tamaño de lote,
/// cediendo el lock entre lotes.
#[test]
fn drain_consumer_empties_ring_above_batch_size() {
    let rb = HeapRb::<f32>::new(16_384);
    let (mut producer, consumer) = rb.split();
    let samples = vec![1.0f32; 10_000];
    assert_eq!(producer.push_slice(&samples), samples.len());

    let consumer_mutex = Arc::new(Mutex::new(Some(consumer)));
    drain_consumer(&consumer_mutex);

    let guard = consumer_mutex.lock();
    assert_eq!(guard.as_ref().unwrap().occupied_len(), 0);
}
