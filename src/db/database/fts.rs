//! Consistency checks and repair for the full-text index.
//!
//! `SONGS_FTS_CONTENT` holds the text FTS5 indexes, so comparing its row count
//! with `SONGS` detects drift cheaply, `rebuild` regenerates the index from those
//! rows, and the two-value `integrity-check` compares the index against them.
//! The statements run on the shared connection; the caller owns the lock.

use rusqlite::Result;

use super::Database;

impl Database {
    /// Counts the content rows the full-text index reads its text from.
    pub fn fts_content_count(&self) -> Result<i64> {
        self.conn
            .prepare_cached("SELECT count(*) FROM SONGS_FTS_CONTENT")?
            .query_row([], |row| row.get(0))
    }

    /// Counts the song rows the content table is expected to mirror.
    pub fn songs_count(&self) -> Result<i64> {
        self.conn
            .prepare_cached("SELECT count(*) FROM SONGS")?
            .query_row([], |row| row.get(0))
    }

    /// Compares both counts to detect a content row that is missing or left over.
    ///
    /// This is the cheap startup detector: it never walks the index, so it can
    /// only prove that every song has one content row, not that the tokens are
    /// right. A deeper check belongs to the test suite.
    pub fn is_fts_consistent(&self) -> Result<bool> {
        Ok(self.fts_content_count()? == self.songs_count()?)
    }

    /// Rebuilds the whole index from the content table.
    ///
    /// This is the repair path for a mismatch: it discards the index rows and
    /// regenerates them from the current content rows.
    pub fn rebuild_fts_index(&self) -> Result<()> {
        self.conn
            .execute("INSERT INTO SONGS_FTS(SONGS_FTS) VALUES('rebuild')", [])?;
        Ok(())
    }

    /// Runs the integrity check that compares the index with the content table.
    ///
    /// The `rank = 1` argument is what makes the check compare both sides: the
    /// single-value form only inspects the index's internal b-trees and passes
    /// on an index that no longer matches its content rows.
    pub fn verify_fts_integrity(&self) -> Result<()> {
        self.conn.execute(
            "INSERT INTO SONGS_FTS(SONGS_FTS, rank) VALUES('integrity-check', 1)",
            [],
        )?;
        Ok(())
    }
}
