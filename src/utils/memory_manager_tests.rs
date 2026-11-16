//! RAM and purge decisions of the memory manager.
//!
//! The boundary tests read the pure predicates: the system hard cap, the process
//! cap and the purge cooldown are asserted from values. The two `sysinfo` probes
//! exercise the real read path of the platform the suite runs on and never assert
//! an absolute RAM value: one requires a non-zero RSS, the other only checks that
//! the reported usage does not exceed the reported total.

use super::*;

// Serializa las pruebas que leen el `SYSINFO` global mediante `try_lock`: si dos
// hilos compiten por él, `try_lock` falla y la lectura devuelve (0, 0), lo que
// hacía fallar estas aserciones de forma intermitente bajo `cargo test` paralelo.
static SYSINFO_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn hard_cap_boundary() {
    // Exactamente el 75% NO supera el hard cap (estricto).
    assert!(!system_ram_over_hard_cap(75, 100));
    // 75.000001% sí lo supera.
    assert!(system_ram_over_hard_cap(75_000_001, 100_000_000));
    // Un total desconocido (0) nunca se considera por encima.
    assert!(!system_ram_over_hard_cap(1, 0));
}

#[test]
fn process_cap_boundary() {
    // Exactamente el tope (500 MB) NO supera el límite (estricto).
    assert!(!process_ram_over_limit(500));
    assert!(process_ram_over_limit(501));
    assert!(!process_ram_over_limit(0));
}

#[test]
fn purge_cooldown() {
    // Sin exceso de RAM nunca se purga.
    assert_eq!(ram_purge_decision(false, 1_000, 0), None);
    // Con exceso, el cooldown de 10s es inclusivo en el límite.
    assert_eq!(ram_purge_decision(true, 1_000, 1_000), None);
    assert_eq!(ram_purge_decision(true, 1_009, 1_000), None);
    assert_eq!(ram_purge_decision(true, 1_010, 1_000), Some(1_010));
    assert_eq!(ram_purge_decision(true, 1_011, 1_000), Some(1_011));
}

#[test]
fn self_rss_probe_returns_nonzero() {
    let _guard = SYSINFO_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Ejercita la ruta real de `sysinfo` en la plataforma en la que corre el test;
    // un proceso en ejecución siempre tiene RSS > 0 si el sondeo funciona.
    assert!(
        MemoryManager::get_self_ram_mb() > 0,
        "el sondeo de RSS vía sysinfo devolvió 0"
    );
}

#[test]
fn system_ram_is_sane() {
    let _guard = SYSINFO_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // Ejercita la ruta real de lectura de RAM del sistema: un refresh que omitiera
    // `with_ram()` devolvería total = 0.
    let (used, total) = get_system_ram_bytes();
    assert!(total > 0, "sysinfo no reportó RAM total");
    assert!(used <= total, "RAM usada > total");
}
