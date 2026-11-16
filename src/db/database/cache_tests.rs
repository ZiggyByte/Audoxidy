//! Area test for the prepared-statement cache: flushing it leaves the library
//! queries working, because the statements are re-prepared transparently.
//!
//! The test runs against an in-memory database, so no test touches the
//! application database.

use super::*;

#[test]
fn test_prepared_statement_cache_flush() {
    let db = Database::new_memory().unwrap();

    // Primer acceso: puebla la caché de sentencias preparadas.
    let first = db.get_all_songs().unwrap();
    assert!(first.is_empty());

    // Vaciar la caché no debe afectar a las consultas posteriores:
    // las sentencias se vuelven a preparar de forma transparente.
    db.flush_prepared_statements();

    let second = db.get_all_songs().unwrap();
    assert!(second.is_empty());
}
