#[cfg(test)]
mod integration_tests {
    use crate::audio::decoder::{AudioDecoder, DecodeStreamInfo, DecodedPacket};
    use crate::audio::engine::AudioEngine;
    use crate::audio::error::AudioError;
    use crate::db::database::{Database, SongData, SongMetadataExtended};
    use std::sync::Arc;

    // ── Mock Decoder ──

    struct MockDecoder {
        frames_generated: usize,
        max_frames: usize,
        sample_rate: u32,
        channels: u16,
    }

    impl MockDecoder {
        fn new() -> Self {
            Self {
                frames_generated: 0,
                max_frames: 44100,
                sample_rate: 44100,
                channels: 2,
            }
        }
    }

    impl AudioDecoder for MockDecoder {
        fn open(&mut self, _path: &str) -> Result<DecodeStreamInfo, AudioError> {
            self.frames_generated = 0;
            Ok(DecodeStreamInfo {
                sample_rate: self.sample_rate,
                channels: self.channels,
                total_duration_sec: self.max_frames as f64 / self.sample_rate as f64,
                channel_count: self.channels as usize,
            })
        }

        fn decode_next(&mut self) -> Result<Option<DecodedPacket>, AudioError> {
            const CHUNK: usize = 512;
            if self.frames_generated >= self.max_frames {
                return Ok(None);
            }
            let remaining = self.max_frames - self.frames_generated;
            let frames = remaining.min(CHUNK);
            let ch = self.channels as usize;
            let mut data = Vec::with_capacity(frames * ch);
            let base = self.frames_generated;
            for i in 0..frames {
                let val = (2.0 * std::f64::consts::PI * 440.0 * (base + i) as f64
                    / self.sample_rate as f64)
                    .sin();
                for _ in 0..ch {
                    data.push(val * 0.5);
                }
            }
            self.frames_generated += frames;
            Ok(Some(DecodedPacket {
                data,
                frames,
                channels: ch,
                sample_rate: self.sample_rate,
            }))
        }

        fn seek(&mut self, time_secs: f64) -> Result<(), AudioError> {
            self.frames_generated = (time_secs * self.sample_rate as f64) as usize;
            Ok(())
        }

        fn reset(&mut self) {
            self.frames_generated = 0;
        }
    }

    // ── 3.4.1: Test de carga y reproducción simulada ──

    #[test]
    fn test_mock_decoder_generates_audio() {
        let mut decoder = MockDecoder::new();
        let info = decoder.open("/fake/test.flac").unwrap();
        assert_eq!(info.sample_rate, 44100);
        assert_eq!(info.channels, 2);
        assert!((info.total_duration_sec - 1.0).abs() < 0.01);

        let packet = decoder.decode_next().unwrap().unwrap();
        assert_eq!(packet.frames, 512);
        assert_eq!(packet.channels, 2);
        assert_eq!(packet.data.len(), 1024);
        assert!(packet.data.iter().any(|&s| s.abs() > 0.01));
    }

    #[test]
    fn test_mock_decoder_reports_eof() {
        let mut decoder = MockDecoder::new();
        decoder.max_frames = 100;
        decoder.open("/fake/test.flac").unwrap();
        let mut total = 0;
        while let Ok(Some(p)) = decoder.decode_next() {
            total += p.frames;
        }
        assert!(total >= 100);
        assert_eq!(decoder.decode_next().unwrap().is_none(), true);
    }

    #[test]
    fn test_mock_decoder_seek() {
        let mut decoder = MockDecoder::new();
        decoder.open("/fake/test.flac").unwrap();
        decoder.seek(0.5).unwrap();
        let packet = decoder.decode_next().unwrap().unwrap();
        let expected = (2.0 * std::f64::consts::PI * 440.0 * 0.5).sin() * 0.5;
        assert!((packet.data[0] - expected).abs() < 0.01);
    }

    // ── 3.4.2: Test de escaneo y consulta en BD ──

    fn make_song(path: &str, title: &str, artist: &str) -> SongData {
        SongData {
            full_file_path: Arc::from(path.to_string().into_boxed_str()),
            title: Some(Arc::from(title.to_string().into_boxed_str())),
            artist: Some(Arc::from(artist.to_string().into_boxed_str())),
            ..Default::default()
        }
    }

    fn make_extended() -> SongMetadataExtended {
        SongMetadataExtended::default()
    }

    #[test]
    fn test_database_scan_and_query() {
        let mut db = Database::new_memory().expect("Failed to create in-memory DB");

        let mut song1 = make_song("/music/test_song.flac", "Test Title", "Test Artist");
        song1.album = Some(Arc::from("Test Album".to_string().into_boxed_str()));
        song1.album_artist = Some(Arc::from("Test Album Artist".to_string().into_boxed_str()));
        song1.genre = Some(Arc::from("Rock".to_string().into_boxed_str()));
        song1.release_year = Some(Arc::from("2024".to_string().into_boxed_str()));
        song1.track_number = Some(Arc::from("1".to_string().into_boxed_str()));
        song1.duration_secs = Some(180.0);
        song1.format = Some(Arc::from("FLAC".to_string().into_boxed_str()));
        song1.bit_depth = Some(24);
        song1.sample_rate = Some(96000);
        song1.size = Some(50_000_000);
        song1.channels = Some(2);

        db.insert_song_full(&song1, &make_extended(), None, vec![], false)
            .unwrap();

        let mut song2 = make_song("/music/test_song2.mp3", "Another Title", "Another Artist");
        song2.format = Some(Arc::from("MP3".to_string().into_boxed_str()));

        db.insert_song_full(&song2, &make_extended(), None, vec![], false)
            .unwrap();

        let songs = db.get_all_songs().unwrap();
        assert_eq!(songs.len(), 2);

        // search_songs devuelve Vec<Arc<SongData>>
        let results = db.search_songs("Test Title").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title.as_deref(), Some("Test Title"));

        let results = db.search_songs("Another").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_database_empty_search() {
        let mut db = Database::new_memory().expect("Failed to create in-memory DB");
        let results = db.search_songs("nonexistent").unwrap();
        assert!(results.is_empty());
    }

    // ── 3.4.3: Test de operaciones CRUD de playlists ──

    #[test]
    fn test_playlist_crud_operations() {
        let mut db = Database::new_memory().expect("Failed to create in-memory DB");

        let id = db.create_playlist("Test Playlist", false).unwrap();
        assert!(id > 0);

        let playlists = db.get_all_playlists().unwrap();
        // 1 nuestra + 2 del sistema = 3
        assert_eq!(playlists.len(), 3);
        assert!(
            playlists.iter().any(|p| p.name == "Test Playlist"),
            "Playlist should exist"
        );

        db.rename_playlist(id, "Renamed Playlist").unwrap();
        let playlists = db.get_all_playlists().unwrap();
        assert_eq!(playlists.len(), 3);
        assert!(
            playlists.iter().any(|p| p.name == "Renamed Playlist"),
            "Renamed playlist should have new name"
        );
        assert!(
            !playlists.iter().any(|p| p.name == "Test Playlist"),
            "Old name should not exist"
        );

        db.delete_playlist(id).unwrap();
        let playlists = db.get_all_playlists().unwrap();
        assert_eq!(playlists.len(), 2);
        assert!(
            !playlists.iter().any(|p| p.id == id),
            "Deleted playlist should not appear"
        );
    }

    #[test]
    fn test_playlist_multiple_crud() {
        let mut db = Database::new_memory().expect("Failed to create in-memory DB");

        let id1 = db.create_playlist("Playlist A", false).unwrap();
        let id2 = db.create_playlist("Playlist B", false).unwrap();
        let id3 = db.create_playlist("Playlist C", false).unwrap();

        let all = db.get_all_playlists().unwrap();
        // 3 nuestras + 2 del sistema (Todas las canciones, En reproducción)
        assert_eq!(all.len(), 5);

        db.delete_playlist(id2).unwrap();
        let remaining = db.get_all_playlists().unwrap();
        assert_eq!(remaining.len(), 4);
        assert!(remaining.iter().any(|p| p.id == id1));
        assert!(remaining.iter().any(|p| p.id == id3));
    }

    // ── 3.4.1 adicional: Verificar DI de AudioEngine con MockDecoder ──

    #[test]
    fn test_audio_engine_with_mock_decoder() {
        let mock = Box::new(MockDecoder::new());
        let engine = AudioEngine::new_with_decoder(Some(mock));

        match engine {
            Ok(e) => {
                assert!(e.custom_decoder.lock().is_some());
            }
            Err(_) => {
                // Sin dispositivo de audio (CI) — esperado
                println!(
                    "AudioEngine::new_with_decoder failed (expected in CI without audio)"
                );
            }
        }
    }
}
