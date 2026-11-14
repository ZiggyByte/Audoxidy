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

    /// Rebuilds the whole index from the songs it is expected to mirror.
    ///
    /// This is the repair path for a mismatch. `rebuild` alone only regenerates
    /// the index from the content rows that exist, so a missing content row
    /// would survive the repair and keep the counts mismatched. The index is
    /// regenerated first so the content-table triggers, which delete from the
    /// index, always run against rows the index actually holds; the content is
    /// then repopulated from `SONGS` and the index is regenerated again.
    pub fn rebuild_fts_index(&self) -> Result<()> {
        let transaction = self.conn.unchecked_transaction()?;
        transaction.execute("INSERT INTO SONGS_FTS(SONGS_FTS) VALUES('rebuild')", [])?;
        transaction.execute("DELETE FROM SONGS_FTS_CONTENT", [])?;
        transaction.execute(
            "INSERT INTO SONGS_FTS_CONTENT (song_id, title, artist_name, album_title)
             SELECT s.id, s.title, ar.name, al.title
             FROM SONGS s
             JOIN ARTISTS ar ON s.artist_id = ar.id
             JOIN ALBUMS al ON s.album_id = al.id",
            [],
        )?;
        transaction.execute("INSERT INTO SONGS_FTS(SONGS_FTS) VALUES('rebuild')", [])?;
        transaction.commit()
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
