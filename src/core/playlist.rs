//! Playlist service: playlist CRUD, item enablement, and persistence.
//!
//! All methods follow the acquire→copy→release pattern: they acquire the
//! database lock, read or mutate data, release the lock, and return plain
//! data or a typed core error. No lock is held across an audio manager call.

use crate::db::database::{PlaylistData, PlaylistFolderGroup, PlaylistSongRef, ShuffleSession};

use super::Core;
use super::error::CoreError;

impl Core {
    /// Creates a playlist and returns its id.
    pub fn create_playlist(&self, name: &str, is_system: bool) -> Result<i64, CoreError> {
        let db = self.lock_db()?;
        Ok(db.create_playlist(name, is_system)?)
    }

    /// Returns all playlists ordered by their sort order.
    pub fn get_all_playlists(&self) -> Result<Vec<PlaylistData>, CoreError> {
        let db = self.lock_db()?;
        Ok(db.get_all_playlists()?)
    }

    /// Renames a playlist; returns whether a row was updated.
    pub fn rename_playlist(&self, playlist_id: i64, new_name: &str) -> Result<bool, CoreError> {
        let db = self.lock_db()?;
        Ok(db.rename_playlist(playlist_id, new_name)?)
    }

    /// Deletes a playlist; returns whether a row was removed.
    pub fn delete_playlist(&self, playlist_id: i64) -> Result<bool, CoreError> {
        let db = self.lock_db()?;
        Ok(db.delete_playlist(playlist_id)?)
    }

    /// Removes every item from a playlist.
    pub fn clear_playlist(&self, playlist_id: i64) -> Result<(), CoreError> {
        let db = self.lock_db()?;
        db.clear_playlist(playlist_id)?;
        Ok(())
    }

    /// Adds one song to a playlist.
    pub fn add_song_to_playlist(&self, playlist_id: i64, song_id: i64) -> Result<bool, CoreError> {
        let db = self.lock_db()?;
        Ok(db.add_song_to_playlist(playlist_id, song_id)?)
    }

    /// Adds several songs to a playlist; returns how many were inserted.
    pub fn add_songs_to_playlist(
        &self,
        playlist_id: i64,
        song_ids: &[i64],
    ) -> Result<usize, CoreError> {
        let mut db = self.lock_db()?;
        Ok(db.add_songs_to_playlist(playlist_id, song_ids)?)
    }

    /// Removes several songs from a playlist.
    pub fn batch_remove_songs_from_playlist(
        &self,
        playlist_id: i64,
        song_ids: &[i64],
    ) -> Result<(), CoreError> {
        let mut db = self.lock_db()?;
        db.batch_remove_songs_from_playlist(playlist_id, song_ids)?;
        Ok(())
    }

    /// Returns the ids of every song in a playlist.
    pub fn get_playlist_all_song_ids(&self, playlist_id: i64) -> Result<Vec<i64>, CoreError> {
        let db = self.lock_db()?;
        Ok(db.get_playlist_all_song_ids(playlist_id)?)
    }

    /// Returns the playlist songs grouped by their folder.
    pub fn get_playlist_songs_grouped_by_folder(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<PlaylistFolderGroup>, CoreError> {
        let db = self.lock_db()?;
        Ok(db.get_playlist_songs_grouped_by_folder(playlist_id)?)
    }

    /// Returns a playlist row by id.
    pub fn get_playlist_by_id(&self, playlist_id: i64) -> Result<Option<PlaylistData>, CoreError> {
        let db = self.lock_db()?;
        Ok(db.get_playlist_by_id(playlist_id)?)
    }

    /// Loads the persisted shuffle session for a playlist.
    pub fn load_shuffle_session(
        &self,
        playlist_id: i64,
    ) -> Result<Option<ShuffleSession>, CoreError> {
        let db = self.lock_db()?;
        Ok(db.load_shuffle_session(playlist_id)?)
    }

    /// Sets the enabled flag of one playlist item.
    pub fn set_song_enabled_in_playlist(
        &self,
        playlist_id: i64,
        song_id: i64,
        enabled: bool,
    ) -> Result<(), CoreError> {
        let db = self.lock_db()?;
        db.set_song_enabled_in_playlist(playlist_id, song_id, enabled)?;
        Ok(())
    }

    /// Toggles the enabled flag of one playlist item.
    pub fn toggle_song_enabled_in_playlist(
        &self,
        playlist_id: i64,
        song_id: i64,
    ) -> Result<bool, CoreError> {
        let db = self.lock_db()?;
        Ok(db.toggle_song_enabled_in_playlist(playlist_id, song_id)?)
    }

    /// Sets the enabled flag of every item in a folder group.
    pub fn set_folder_enabled_in_playlist(
        &self,
        playlist_id: i64,
        folder_path: &str,
        enabled: bool,
    ) -> Result<(), CoreError> {
        let db = self.lock_db()?;
        db.set_folder_enabled_in_playlist(playlist_id, folder_path, enabled)?;
        Ok(())
    }

    /// Toggles the enabled flag of every item in a folder group.
    pub fn toggle_folder_enabled_in_playlist(
        &self,
        playlist_id: i64,
        folder_path: &str,
    ) -> Result<(), CoreError> {
        let db = self.lock_db()?;
        db.toggle_folder_enabled_in_playlist(playlist_id, folder_path)?;
        Ok(())
    }

    /// Searches the songs of a playlist by query text.
    pub fn search_playlist_songs(
        &self,
        playlist_id: i64,
        query: &str,
    ) -> Result<Vec<PlaylistSongRef>, CoreError> {
        let db = self.lock_db()?;
        Ok(db.search_playlist_songs(playlist_id, query)?)
    }

    /// Persists the playback position and shuffle state of a playlist.
    #[allow(clippy::too_many_arguments)]
    pub fn update_playlist_persistence(
        &self,
        playlist_id: i64,
        last_song_id: Option<i64>,
        last_pos_sec: f64,
        is_playing: bool,
        shuffle_active: bool,
        repeat_mode: i32,
        shuffle_pos: usize,
        shuffle_id: Option<String>,
    ) -> Result<(), CoreError> {
        let db = self.lock_db()?;
        db.update_playlist_persistence(
            playlist_id,
            last_song_id,
            last_pos_sec,
            is_playing,
            shuffle_active,
            repeat_mode,
            shuffle_pos,
            shuffle_id,
        )?;
        Ok(())
    }

    /// Creates a playlist and appends the given songs to it.
    ///
    /// A failure to append songs is logged but does not undo the creation.
    pub fn create_playlist_with_songs(
        &self,
        name: &str,
        song_ids: &[i64],
    ) -> Result<i64, CoreError> {
        let mut db = self.lock_db()?;
        let new_id = db.create_playlist(name, false)?;
        if !song_ids.is_empty()
            && let Err(e) = db.add_songs_to_playlist(new_id, song_ids)
        {
            tracing::warn!(
                target: "audoxidy::persistence",
                playlist_id = new_id,
                count = song_ids.len(),
                error = %e,
                "No se pudieron añadir las canciones a la nueva playlist"
            );
        }
        Ok(new_id)
    }

    /// Creates a user playlist that copies every item of a source playlist.
    pub fn rename_system_playlist(&self, source_id: i64, new_name: &str) -> Result<i64, CoreError> {
        let mut db = self.lock_db()?;
        let new_id = db.create_playlist(new_name, false)?;
        if let Ok(all_songs) = db.get_playlist_all_song_ids(source_id)
            && let Err(e) = db.add_songs_to_playlist(new_id, &all_songs)
        {
            tracing::warn!(
                target: "audoxidy::persistence",
                playlist_id = new_id,
                count = all_songs.len(),
                error = %e,
                "No se pudieron copiar las canciones a la nueva playlist"
            );
        }
        Ok(new_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn make_core_for_test() -> Core {
        let paths = super::super::types::CorePaths::desktop_default();
        let db = Arc::new(Mutex::new(
            crate::db::Database::new_memory().expect("in-memory db"),
        ));
        let scanner = Arc::new(crate::db::scanner::Scanner::new(db.clone()));
        let audio_manager =
            Arc::new(crate::audio::AudioManager::new().expect("audio manager init"));
        Core::new(audio_manager, db, scanner, paths)
    }

    #[test]
    fn test_create_and_list_playlists() {
        let core = make_core_for_test();
        let id = core.create_playlist("Lista de prueba", false).unwrap();
        assert!(id > 0);
        let playlists = core.get_all_playlists().unwrap();
        assert!(playlists.iter().any(|p| p.id == id));
    }

    #[test]
    fn test_rename_and_delete_playlist() {
        let core = make_core_for_test();
        let id = core.create_playlist("Original", false).unwrap();
        assert!(core.rename_playlist(id, "Renombrada").unwrap());
        assert!(core.delete_playlist(id).unwrap());
    }

    #[test]
    fn test_add_and_remove_songs_empty_batch() {
        let core = make_core_for_test();
        let id = core.create_playlist("Con canciones", false).unwrap();
        assert_eq!(core.add_songs_to_playlist(id, &[]).unwrap(), 0);
        core.batch_remove_songs_from_playlist(id, &[]).unwrap();
    }

    #[test]
    fn test_clear_playlist_removes_items() {
        let core = make_core_for_test();
        let id = core.create_playlist("Vaciar", false).unwrap();
        core.clear_playlist(id).unwrap();
        assert!(core.get_playlist_all_song_ids(id).unwrap().is_empty());
    }
}
