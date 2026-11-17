//! Cover service: owner-scoped file names, resolution and the orphan sweep.
//!
//! The service keeps no in-memory cover cache and wraps no path into an iced
//! handle: it composes the name an owner's file is stored under, resolves that
//! file when it exists, and diffs the cache directory against the rows that
//! reference it. The decoded handles live in the GUI store, which is the only
//! layer that knows what the viewport shows.
//!
//! Lock order compliance: the sweep reads the database once and releases the
//! lock before touching the filesystem, so no directory work runs under it.

use std::path::PathBuf;

use crate::utils::observability::PERSISTENCE_TARGET;

use super::Core;
use super::error::CoreError;

/// How a cover-cache sweep ended: it either diffed the directory or a guard
/// kept it away from a cache whose contents were still changing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SweepOutcome {
    /// The directory was read and every unreferenced file was attempted.
    Completed,
    /// A library scan was running, so a file could belong to a row not yet committed.
    SkippedScanning,
    /// Cover writes were queued or in flight, so a file could be unreferenced yet.
    SkippedCoverWork,
}

/// Result of one cover-cache sweep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverSweepReport {
    /// Files removed successfully.
    pub removed: usize,
    /// Files whose removal failed; each failure was logged.
    pub failed: usize,
    /// Whether the sweep ran or why it was skipped.
    pub outcome: SweepOutcome,
}

impl Core {
    /// Resolves the cover file belonging to one album owner.
    ///
    /// The owner and the content hash both compose the name, so two albums
    /// holding byte-identical pictures resolve two different files. Returns
    /// `None` when the name cannot be composed or the file is absent.
    pub fn resolve_cover_path(&self, album_hash: &str, cover_hash: &str) -> Option<PathBuf> {
        let file_name = crate::utils::covers::album_cover_file_name(album_hash, cover_hash).ok()?;
        let path = self.paths.cover_cache_dir.join(file_name);
        if path.exists() { Some(path) } else { None }
    }

    /// Clears the cover art negative cache (failed-path lookups).
    ///
    /// Useful after a re-scan to allow retrying paths that previously
    /// failed to load.
    pub fn clear_cover_cache(&self) {
        crate::utils::covers::clear_negative_cover_cache();
    }

    /// Enqueues a cover art processing job.
    ///
    /// `file_name` is the complete name the writer stores — owner, content hash
    /// and `.avif` included. It is forwarded untouched, so a bare hash must be
    /// composed by the caller first. The job is processed asynchronously on the
    /// cover thread pool.
    pub fn enqueue_cover_job(&self, data: Vec<u8>, file_name: String) {
        crate::utils::covers::enqueue_cover_job(data, file_name);
    }

    /// Removes every `.avif` file in the cover cache directory that neither an
    /// album nor a song references.
    ///
    /// `scanning` and `cover_work_in_flight` are parameters instead of reads, so
    /// the policy is testable without a scanner or a writer. The callers that run
    /// off the update thread read both guards inside their worker immediately
    /// before the sweep: a scan or a cover write can start between the dispatch
    /// and the execution, so a value captured at dispatch would guard the wrong
    /// instant.
    ///
    /// The reference set is read under one database lock, which is released
    /// before any directory work. Candidates come only from `read_dir` inside the
    /// injected cache directory, so a crafted row cannot direct a deletion
    /// outside the cache.
    pub fn sweep_orphan_covers(
        &self,
        scanning: bool,
        cover_work_in_flight: bool,
    ) -> Result<CoverSweepReport, CoreError> {
        if scanning {
            return Ok(CoverSweepReport {
                removed: 0,
                failed: 0,
                outcome: SweepOutcome::SkippedScanning,
            });
        }
        if cover_work_in_flight {
            return Ok(CoverSweepReport {
                removed: 0,
                failed: 0,
                outcome: SweepOutcome::SkippedCoverWork,
            });
        }

        let referenced = {
            let db = self.lock_db()?;
            db.referenced_cover_file_names()?
        };

        let entries = match std::fs::read_dir(&self.paths.cover_cache_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // Nothing has been written yet, so nothing can be an orphan.
                return Ok(CoverSweepReport {
                    removed: 0,
                    failed: 0,
                    outcome: SweepOutcome::Completed,
                });
            }
            // Any other kind is a real failure: a permission error must not read
            // as a clean sweep that found nothing to remove.
            Err(error) => return Err(error.into()),
        };

        let mut removed = 0usize;
        let mut failed = 0usize;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(
                        target: PERSISTENCE_TARGET,
                        error = %error,
                        "No se pudo leer una entrada del directorio de carátulas"
                    );
                    continue;
                }
            };

            // The writer only produces regular files. A symlink or a directory is
            // not followed and not removed.
            match entry.file_type() {
                Ok(file_type) if file_type.is_file() => {}
                Ok(_) => continue,
                Err(error) => {
                    tracing::warn!(
                        target: PERSISTENCE_TARGET,
                        error = %error,
                        "No se pudo leer el tipo de una entrada de carátulas"
                    );
                    continue;
                }
            }

            let path = entry.path();
            if path.extension() != Some(std::ffi::OsStr::new("avif")) {
                continue;
            }

            let is_referenced = path
                .file_name()
                .is_some_and(|file_name| referenced.contains(file_name));
            if is_referenced {
                continue;
            }

            match std::fs::remove_file(&path) {
                Ok(()) => removed += 1,
                Err(error) => {
                    failed += 1;
                    tracing::warn!(
                        target: PERSISTENCE_TARGET,
                        path = %path.display(),
                        error = %error,
                        "No se pudo eliminar una carátula sin referencias"
                    );
                }
            }
        }

        tracing::info!(
            target: "audoxidy::metrics",
            metric = "cover_sweep",
            removed,
            failed,
            "cover sweep finished"
        );

        Ok(CoverSweepReport {
            removed,
            failed,
            outcome: SweepOutcome::Completed,
        })
    }
}

#[cfg(test)]
#[path = "covers_tests.rs"]
mod tests;
