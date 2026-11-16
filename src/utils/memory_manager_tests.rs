//! RAM and purge decisions of the memory manager.
//!
//! The boundary tests read the pure predicates: the system hard cap, the process
//! cap and the purge cooldown are asserted from values. The timer test drives the
//! process-global purge timer under the lock that serializes every module touching
//! it. The two `sysinfo` probes exercise the real read path of the platform the
//! suite runs on and never assert an absolute RAM value: one requires a non-zero
//! RSS, the other only checks that the reported usage does not exceed the reported
//! total.

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

/// Drives the process-global purge timer through every branch it offers.
///
/// The timer is one atomic shared with the maintenance timer test, so this test
/// holds `PURGE_TIMER_TEST_LOCK` for its whole body: unchecked, either side can
/// back-date the other's state and a "due" assertion passes vacuously.
///
/// The 10-second RAM purge cooldown is deliberately out of scope. That decision
/// belongs to the pure `ram_purge_decision`, asserted by `purge_cooldown` above,
/// and its global stamp is only reachable through a real RAM probe, which tests
/// must not perform.
#[test]
fn global_purge_timer_branches() {
    let _timer_guard = PURGE_TIMER_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());

    // A freshly reset timer is not due.
    MemoryManager::reset_global_purge_timer();
    assert!(
        !MemoryManager::should_run_global_purge(0, false),
        "un temporizador recién reiniciado no vence"
    );

    // Past the fixed interval the check is due.
    MemoryManager::backdate_global_purge_timer_for_tests(FIXED_PURGE_INTERVAL_SECS + 1);
    assert!(
        MemoryManager::should_run_global_purge(0, false),
        "pasado el intervalo fijo la purga vence"
    );

    // The due branch stored the current stamp, so an immediate second check is
    // not due: the interval was consumed, not merely reported.
    assert!(
        !MemoryManager::should_run_global_purge(0, false),
        "la comprobación que venció consumió el intervalo"
    );

    // The scan guard returns before the timer is read, so a suppressed check
    // leaves the back-date in place and the next check still sees it.
    MemoryManager::backdate_global_purge_timer_for_tests(FIXED_PURGE_INTERVAL_SECS + 1);
    assert!(
        !MemoryManager::should_run_global_purge(0, true),
        "un escaneo en curso suprime la purga"
    );
    assert!(
        MemoryManager::should_run_global_purge(0, false),
        "la comprobación suprimida no consumió el intervalo"
    );
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
