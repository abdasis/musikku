//! Uji integrasi untuk alur library: pemindaian, penyimpanan, dan playlist.
//!
//! Uji memakai database in-memory dan berkas audio sementara sehingga
//! tidak menyentuh data pengguna.

use musikku::core::model::{RepeatMode, Track};
use musikku::database::{
    Database, PlaylistRepository, SettingsRepository, SortDirection, TrackFilter, TrackRepository,
    TrackSort,
};

/// Bangun track uji tanpa perlu berkas di disk.
///
/// `file_path` harus unik karena penyaringan dilakukan lewat path.
fn sample_track(index: usize, title: &str, artist: &str, album: &str) -> Track {
    Track {
        id: 0,
        file_path: format!("/musik/track-{index}.mp3").into(),
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        album_artist: String::new(),
        duration_ms: 180_000,
        track_number: Some(index as u32),
        disc_number: None,
        year: Some(2023),
        genre: Some("Elektronik".to_string()),
        artwork_path: None,
        file_size: 4096,
        file_modified_at: None,
        scan_status: musikku::core::model::ScanStatus::Ready,
        last_played_at: None,
        play_count: 0,
        is_favorite: false,
    }
}

#[test]
fn track_upsert_creates_then_updates_the_same_row() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());

    let mut track = sample_track(0, "Judul Lama", "Artis", "Album");
    let id = repo.upsert(&track).expect("penyimpanan pertama gagal");

    track.title = "Judul Baru".to_string();
    track.id = id;
    let id_after = repo.upsert(&track).expect("penyimpanan kedua gagal");

    assert_eq!(id, id_after, "path yang sama harus memakai baris yang sama");
    assert_eq!(repo.count().expect("hitung gagal"), 1);

    let stored = repo
        .find_by_id(id)
        .expect("pencarian gagal")
        .expect("track harus ada");
    assert_eq!(stored.title, "Judul Baru");
}

#[test]
fn query_sorts_by_title_case_insensitively() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());

    for (index, (title, artist)) in [(0, ("zeta", "B")), (1, ("Alpha", "A")), (2, ("mid", "C"))] {
        repo.upsert(&sample_track(index, title, artist, "Album"))
            .expect("penyimpanan gagal");
    }

    let tracks = repo
        .query(
            &TrackFilter::default(),
            TrackSort::Title,
            SortDirection::Ascending,
        )
        .expect("query gagal");

    let titles: Vec<&str> = tracks.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["Alpha", "mid", "zeta"]);
}

#[test]
fn query_filters_by_search_term_across_fields() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());

    repo.upsert(&sample_track(
        0,
        "Bintang Laut",
        "Artis Satu",
        "Album Alpha",
    ))
    .expect("penyimpanan gagal");
    repo.upsert(&sample_track(1, "Lain", "Artis Dua", "Album Beta"))
        .expect("penyimpanan gagal");

    let filter = TrackFilter {
        search: Some("artis dua".to_string()),
        ..Default::default()
    };
    let tracks = repo
        .query(&filter, TrackSort::Title, SortDirection::Ascending)
        .expect("query gagal");

    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].artist, "Artis Dua");
}

#[test]
fn favorite_flag_survives_a_reload() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());

    let id = repo
        .upsert(&sample_track(0, "Favorit", "Artis", "Album"))
        .expect("penyimpanan gagal");
    repo.set_favorite(id, true).expect("ubah favorit gagal");

    let stored = repo
        .find_by_id(id)
        .expect("pencarian gagal")
        .expect("track harus ada");
    assert!(stored.is_favorite);

    let filter = TrackFilter {
        favorites_only: true,
        ..Default::default()
    };
    let favorites = repo
        .query(&filter, TrackSort::Title, SortDirection::Ascending)
        .expect("query gagal");
    assert_eq!(favorites.len(), 1);
}

#[test]
fn marking_played_increments_the_counter() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());

    let id = repo
        .upsert(&sample_track(0, "Diputar", "Artis", "Album"))
        .expect("penyimpanan gagal");

    repo.mark_played(id).expect("tandai diputar gagal");
    repo.mark_played(id).expect("tandai diputar gagal");

    let stored = repo
        .find_by_id(id)
        .expect("pencarian gagal")
        .expect("track harus ada");
    assert_eq!(stored.play_count, 2);
    assert!(stored.last_played_at.is_some());
}

#[test]
fn removing_a_track_leaves_the_file_untouched() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());

    let track = sample_track(0, "Dihapus", "Artis", "Album");
    let path = track.file_path.clone();
    let id = repo.upsert(&track).expect("penyimpanan gagal");

    repo.remove(id).expect("hapus gagal");

    assert_eq!(repo.count().expect("hitung gagal"), 0);
    assert!(!path.exists(), "berkas asli tidak boleh ikut terhapus");
}

#[test]
fn playlist_keeps_the_order_of_its_tracks() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let track_repo = TrackRepository::new(db.connection());
    let playlist_repo = PlaylistRepository::new(db.connection());

    let mut ids = Vec::new();
    for (index, title) in ["Pertama", "Kedua", "Ketiga"].iter().enumerate() {
        let id = track_repo
            .upsert(&sample_track(index, title, "Artis", "Album"))
            .expect("penyimpanan gagal");
        ids.push(id);
    }

    let playlist_id = playlist_repo
        .create("Test Playlist")
        .expect("buat playlist gagal");
    for id in &ids {
        playlist_repo
            .add_track(playlist_id, *id)
            .expect("tambah track gagal");
    }

    let stored = playlist_repo
        .track_ids(playlist_id)
        .expect("ambil track gagal");
    assert_eq!(
        stored, ids,
        "urutan playlist harus sesuai urutan penambahan"
    );

    let playlists = playlist_repo.list().expect("daftar playlist gagal");
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].track_count, 3);
}

#[test]
fn reordering_a_playlist_changes_the_track_order() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let track_repo = TrackRepository::new(db.connection());
    let playlist_repo = PlaylistRepository::new(db.connection());

    let mut ids = Vec::new();
    for (index, title) in ["A", "B", "C"].iter().enumerate() {
        let id = track_repo
            .upsert(&sample_track(index, title, "Artis", "Album"))
            .expect("penyimpanan gagal");
        ids.push(id);
    }

    let playlist_id = playlist_repo.create("Urut").expect("buat playlist gagal");
    for id in &ids {
        playlist_repo
            .add_track(playlist_id, *id)
            .expect("tambah track gagal");
    }

    let reordered = vec![ids[2], ids[0], ids[1]];
    playlist_repo
        .reorder(playlist_id, &reordered)
        .expect("urut ulang gagal");

    let stored = playlist_repo
        .track_ids(playlist_id)
        .expect("ambil track gagal");
    assert_eq!(stored, reordered);
}

#[test]
fn removing_a_track_from_a_playlist_closes_the_gap() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let track_repo = TrackRepository::new(db.connection());
    let playlist_repo = PlaylistRepository::new(db.connection());

    let mut ids = Vec::new();
    for (index, title) in ["A", "B", "C"].iter().enumerate() {
        let id = track_repo
            .upsert(&sample_track(index, title, "Artis", "Album"))
            .expect("penyimpanan gagal");
        ids.push(id);
    }

    let playlist_id = playlist_repo.create("Kosong").expect("buat playlist gagal");
    for id in &ids {
        playlist_repo
            .add_track(playlist_id, *id)
            .expect("tambah track gagal");
    }

    playlist_repo
        .remove_track(playlist_id, ids[1])
        .expect("hapus track gagal");

    let stored = playlist_repo
        .track_ids(playlist_id)
        .expect("ambil track gagal");
    assert_eq!(stored, vec![ids[0], ids[2]]);

    let playlists = playlist_repo.list().expect("daftar playlist gagal");
    assert_eq!(playlists[0].track_count, 2);
}

#[test]
fn deleting_a_playlist_keeps_the_tracks() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let track_repo = TrackRepository::new(db.connection());
    let playlist_repo = PlaylistRepository::new(db.connection());

    let track_id = track_repo
        .upsert(&sample_track(0, "Bertahan", "Artis", "Album"))
        .expect("penyimpanan gagal");

    let playlist_id = playlist_repo
        .create("Sementara")
        .expect("buat playlist gagal");
    playlist_repo
        .add_track(playlist_id, track_id)
        .expect("tambah track gagal");
    playlist_repo
        .remove(playlist_id)
        .expect("hapus playlist gagal");

    assert!(playlist_repo
        .list()
        .expect("daftar playlist gagal")
        .is_empty());
    assert_eq!(
        track_repo.count().expect("hitung gagal"),
        1,
        "berkas audio tidak boleh ikut terhapus"
    );
}

#[test]
fn settings_round_trip_through_the_database() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = SettingsRepository::new(db.connection());

    repo.set_bool("scan_on_startup", true)
        .expect("simpan bool gagal");
    repo.set_f64("volume", 0.42).expect("simpan float gagal");
    repo.set("theme", "dark").expect("simpan teks gagal");

    assert!(repo
        .get_bool("scan_on_startup", false)
        .expect("baca bool gagal"));
    assert_eq!(repo.get_f64("volume", 0.0).expect("baca float gagal"), 0.42);
    assert_eq!(
        repo.get("theme").expect("baca teks gagal"),
        Some("dark".to_string())
    );
}

#[test]
fn unknown_settings_fall_back_to_defaults() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = SettingsRepository::new(db.connection());

    assert!(!repo.get_bool("tidak_ada", false).expect("baca bool gagal"));
    assert_eq!(
        repo.get_f64("tidak_ada", 0.8).expect("baca float gagal"),
        0.8
    );
    assert!(repo.get("tidak_ada").expect("baca teks gagal").is_none());
}

#[test]
fn playback_state_survives_a_reload() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = musikku::database::PlaybackRepository::new(db.connection());

    // current_track_id harus menunjuk track yang benar-benar ada.
    let track_repo = TrackRepository::new(db.connection());
    let track_id = track_repo
        .upsert(&sample_track(0, "Diputar", "Artis", "Album"))
        .expect("penyimpanan gagal");

    let state = musikku::core::model::PlaybackState {
        current_track_id: Some(track_id),
        position_ms: 12_345,
        volume: 0.65,
        muted: true,
        shuffle_enabled: true,
        repeat_mode: RepeatMode::One,
        queue_order: vec![track_id],
    };

    repo.save_state(&state).expect("simpan state gagal");
    repo.save_queue(&state.queue_order)
        .expect("simpan antrean gagal");

    let loaded = repo.load_state().expect("baca state gagal");
    assert_eq!(loaded.current_track_id, Some(track_id));
    assert_eq!(loaded.position_ms, 12_345);
    assert_eq!(loaded.volume, 0.65);
    assert!(loaded.muted);
    assert!(loaded.shuffle_enabled);
    assert_eq!(loaded.repeat_mode, RepeatMode::One);
    assert_eq!(loaded.queue_order, vec![track_id]);
}

#[test]
fn renaming_a_playlist_keeps_its_tracks() {
    let db = Database::open_in_memory().expect("database harus terbuka");
    let track_repo = TrackRepository::new(db.connection());
    let playlist_repo = PlaylistRepository::new(db.connection());

    let track_id = track_repo
        .upsert(&sample_track(0, "Tetap", "Artis", "Album"))
        .expect("penyimpanan gagal");

    let playlist_id = playlist_repo
        .create("Nama Lama")
        .expect("buat playlist gagal");
    playlist_repo
        .add_track(playlist_id, track_id)
        .expect("tambah track gagal");

    playlist_repo
        .rename(playlist_id, "Nama Baru")
        .expect("ganti nama gagal");

    let playlists = playlist_repo.list().expect("daftar playlist gagal");
    assert_eq!(playlists.len(), 1);
    assert_eq!(playlists[0].name, "Nama Baru");
    assert_eq!(
        playlists[0].track_count, 1,
        "track playlist harus tetap ada"
    );
}

#[test]
fn migrations_are_idempotent() {
    // Membuka database yang sama dua kali tidak boleh merusak data.
    let db = Database::open_in_memory().expect("database harus terbuka");
    let repo = TrackRepository::new(db.connection());
    repo.upsert(&sample_track(0, "Tetap Ada", "Artis", "Album"))
        .expect("penyimpanan gagal");

    // Jalankan migrasi sekali lagi pada koneksi yang sama.
    musikku::database::migration::run(db.connection()).expect("migrasi ulang gagal");

    assert_eq!(repo.count().expect("hitung gagal"), 1);
}

/// Track dengan id dan path unik untuk pengujian antrean.
fn queue_track(id: i64) -> Track {
    let mut track = sample_track(id as usize, &format!("Lagu {id}"), "Artis", "Album");
    track.id = id;
    track.file_path = format!("/musik/antrean-{id}.mp3").into();
    track
}

#[test]
fn queue_reorder_moves_the_track_to_the_target_position() {
    use musikku::playback::Queue;

    let mut queue = Queue::new();
    for id in [10i64, 20, 30, 40] {
        queue.add(&queue_track(id));
    }

    // Lagu 40 dipindahkan ke posisi paling depan.
    queue.reorder(3, 0);
    assert_eq!(queue.order(), &[40, 10, 20, 30]);

    // Lagu 20 dipindahkan ke posisi terakhir.
    queue.reorder(2, 3);
    assert_eq!(queue.order(), &[40, 10, 30, 20]);
}

#[test]
fn queue_reorder_keeps_the_playing_track_in_sync() {
    use musikku::playback::Queue;

    let mut queue = Queue::new();
    for id in [10i64, 20, 30, 40] {
        queue.add(&queue_track(id));
    }

    // Lagu pada indeks 1 menjadi lagu yang sedang diputar.
    queue.set_current_index(1, Some(queue_track(20)));

    // Baris sebelum lagu aktif digeser ke belakang; indeks lagu
    // aktif ikut mundur satu posisi.
    queue.reorder(0, 2);
    assert_eq!(queue.order(), &[20, 30, 10, 40]);
    assert_eq!(queue.current_index(), Some(0));

    // Baris tepat di posisi lagu aktif dipindah; indeksnya ikut.
    queue.reorder(0, 2);
    assert_eq!(queue.order(), &[30, 10, 20, 40]);
    assert_eq!(queue.current_index(), Some(2));
}

#[test]
fn queue_reorder_ignores_out_of_range_positions() {
    use musikku::playback::Queue;

    let mut queue = Queue::new();
    for id in [10i64, 20] {
        queue.add(&queue_track(id));
    }

    queue.reorder(0, 9);
    assert_eq!(queue.order(), &[10, 20]);

    queue.reorder(9, 0);
    assert_eq!(queue.order(), &[10, 20]);
}

#[test]
fn drop_position_maps_a_point_to_the_row_underneath() {
    use musikku::ui::now_playing::row_index_at;

    // Empat baris setinggi 50 piksel.
    let heights = [50.0, 50.0, 50.0, 50.0];

    // Titik di baris pertama.
    assert_eq!(row_index_at(heights.iter().copied(), 10.0), Some(0));
    // Titik tepat di batas baris pertama dan kedua.
    assert_eq!(row_index_at(heights.iter().copied(), 50.0), Some(1));
    // Titik di baris ketiga.
    assert_eq!(row_index_at(heights.iter().copied(), 120.0), Some(2));
    // Titik di baris terakhir.
    assert_eq!(row_index_at(heights.iter().copied(), 190.0), Some(3));
}

#[test]
fn drop_position_beyond_the_last_row_uses_the_last_row() {
    use musikku::ui::now_playing::row_index_at;

    let heights = [50.0, 50.0];
    assert_eq!(row_index_at(heights.iter().copied(), 500.0), Some(1));
}

#[test]
fn drop_position_on_an_empty_list_has_no_row() {
    use musikku::ui::now_playing::row_index_at;

    let heights: [f64; 0] = [];
    assert_eq!(row_index_at(heights.iter().copied(), 10.0), None);
}
