//! Repository track: operasi baca/tulis terhadap tabel `tracks`.

use std::path::Path;
use std::time::SystemTime;

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::core::model::Track;
use crate::core::time::{from_unix_seconds, to_unix_seconds};

/// Kolom yang selalu dipilih saat membaca track.
const TRACK_COLUMNS: &str = "id, file_path, title, artist, album, album_artist, duration_ms, \
     track_number, disc_number, year, genre, artwork_path, file_size, file_modified_at, \
     scan_status, last_played_at, play_count, is_favorite";

/// Baca track dari baris database.
fn row_to_track(row: &Row<'_>) -> rusqlite::Result<Track> {
    let file_path: String = row.get("file_path")?;
    let artwork_path: Option<String> = row.get("artwork_path")?;
    let file_modified_at: Option<i64> = row.get("file_modified_at")?;
    let last_played_at: Option<i64> = row.get("last_played_at")?;
    let scan_status: String = row.get("scan_status")?;

    Ok(Track {
        id: row.get("id")?,
        file_path: Path::new(&file_path).to_path_buf(),
        title: row.get("title")?,
        artist: row.get("artist")?,
        album: row.get("album")?,
        album_artist: row.get("album_artist")?,
        duration_ms: row.get("duration_ms")?,
        track_number: row.get("track_number")?,
        disc_number: row.get("disc_number")?,
        year: row.get("year")?,
        genre: row.get("genre")?,
        artwork_path: artwork_path.map(|p| Path::new(&p).to_path_buf()),
        file_size: row.get("file_size")?,
        file_modified_at: file_modified_at.and_then(from_unix_seconds),
        scan_status: scan_status.parse().unwrap_or_default(),
        last_played_at: last_played_at.and_then(from_unix_seconds),
        play_count: row.get("play_count")?,
        is_favorite: row.get::<_, i64>("is_favorite")? != 0,
    })
}

/// Urutan sortable yang dipakai UI library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSort {
    Title,
    Artist,
    Album,
    Duration,
    Year,
    TrackNumber,
}

impl TrackSort {
    /// Kolom utama pengurutan.
    fn primary_column(self) -> &'static str {
        match self {
            TrackSort::Title => "title",
            TrackSort::Artist => "artist",
            TrackSort::Album => "album",
            TrackSort::Duration => "duration_ms",
            TrackSort::Year => "year",
            TrackSort::TrackNumber => "track_number",
        }
    }

    /// Kolom pendamping yang menentukan urutan dalam grup nilai utama.
    fn secondary_columns(self) -> &'static str {
        match self {
            TrackSort::Title => "id",
            TrackSort::Artist => "album COLLATE NOCASE ASC, id ASC",
            TrackSort::Album => "disc_number ASC, track_number ASC, id ASC",
            TrackSort::Duration => "id ASC",
            TrackSort::Year => "album COLLATE NOCASE ASC, id ASC",
            TrackSort::TrackNumber => "disc_number ASC, id ASC",
        }
    }

    /// Whether the primary column holds text and needs case-insensitive sorting.
    fn is_textual(self) -> bool {
        matches!(
            self,
            TrackSort::Title | TrackSort::Artist | TrackSort::Album
        )
    }
}

/// Arah pengurutan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    fn sql(self) -> &'static str {
        match self {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        }
    }
}

/// Filter pilihan untuk query track.
#[derive(Debug, Clone, Default)]
pub struct TrackFilter {
    /// Pencarian bebas pada judul, artis, album, dan nama berkas.
    pub search: Option<String>,
    /// Batasi hasil ke track favorit's saja.
    pub favorites_only: bool,
    /// Batasi hasil ke album tertentu.
    pub album: Option<String>,
    /// Batasi hasil ke artis tertentu.
    pub artist: Option<String>,
    /// Batasi hasil ke track yang ada di folder tertentu.
    pub folder: Option<std::path::PathBuf>,
    /// Batasi jumlah hasil; `None` berarti tanpa batas.
    pub limit: Option<usize>,
}

/// Repository track di atas koneksi database.
pub struct TrackRepository<'a> {
    conn: &'a Connection,
}

impl<'a> TrackRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Ambil seluruh track sesuai filter dan urutan.
    pub fn query(
        &self,
        filter: &TrackFilter,
        sort: TrackSort,
        direction: SortDirection,
    ) -> rusqlite::Result<Vec<Track>> {
        let mut sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE 1 = 1");
        let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(search) = filter
            .search
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let pattern = format!("%{}%", search.to_lowercase());
            sql.push_str(
                " AND (LOWER(title) LIKE ?1 OR LOWER(artist) LIKE ?1 OR LOWER(album) LIKE ?1 \
                 OR LOWER(file_path) LIKE ?1)",
            );
            args.push(Box::new(pattern));
        }

        if filter.favorites_only {
            sql.push_str(" AND is_favorite = 1");
        }

        if let Some(album) = filter.album.as_ref() {
            sql.push_str(" AND album = ?");
            args.push(Box::new(album.clone()));
        }

        if let Some(artist) = filter.artist.as_ref() {
            sql.push_str(" AND (artist = ? OR album_artist = ?)");
            args.push(Box::new(artist.clone()));
            args.push(Box::new(artist.clone()));
        }

        if let Some(folder) = filter.folder.as_ref() {
            sql.push_str(" AND file_path LIKE ?");
            args.push(Box::new(format!("{}%", folder.to_string_lossy())));
        }

        // Arah pengurutan hanya berlaku pada kolom utama; kolom pendamping
        // selalu menaik agar album dan track number tetap logis.
        let collation = if sort.is_textual() {
            "COLLATE NOCASE"
        } else {
            ""
        };
        let direction = direction.sql();
        sql.push_str(&format!(
            " ORDER BY {} {collation} {direction}, {}",
            sort.primary_column(),
            sort.secondary_columns()
        ));

        if let Some(limit) = filter.limit {
            sql.push_str(" LIMIT ?");
            args.push(Box::new(limit as i64));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|a| a.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_track)?;

        let mut tracks = Vec::new();
        for row in rows {
            tracks.push(row?);
        }
        Ok(tracks)
    }

    /// Ambil satu track berdasarkan id.
    pub fn find_by_id(&self, id: i64) -> rusqlite::Result<Option<Track>> {
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id = ?1");
        self.conn
            .query_row(&sql, params![id], row_to_track)
            .optional()
    }

    /// Ambil beberapa track berdasarkan id, mempertahankan urutan input.
    pub fn find_by_ids(&self, ids: &[i64]) -> rusqlite::Result<Vec<Track>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
        let sql = format!("SELECT {TRACK_COLUMNS} FROM tracks WHERE id IN ({placeholders})");

        let mut stmt = self.conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
        let rows = stmt.query_map(refs.as_slice(), row_to_track)?;

        let mut found: Vec<Track> = Vec::new();
        for row in rows {
            found.push(row?);
        }

        // Kembalikan sesuai urutan pemanggilan.
        Ok(ids
            .iter()
            .filter_map(|id| found.iter().find(|t| t.id == *id).cloned())
            .collect())
    }

    /// Tambah track baru; bila path sudah ada, perbarui metadatanya.
    pub fn upsert(&self, track: &Track) -> rusqlite::Result<i64> {
        let file_path = track.file_path.to_string_lossy().to_string();
        let artwork = track
            .artwork_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        let modified = track.file_modified_at.map(to_unix_seconds);

        self.conn.execute(
            "INSERT INTO tracks (
                file_path, title, artist, album, album_artist, duration_ms, track_number,
                disc_number, year, genre, artwork_path, file_size, file_modified_at, scan_status
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(file_path) DO UPDATE SET
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                album_artist = excluded.album_artist,
                duration_ms = excluded.duration_ms,
                track_number = excluded.track_number,
                disc_number = excluded.disc_number,
                year = excluded.year,
                genre = excluded.genre,
                artwork_path = COALESCE(excluded.artwork_path, tracks.artwork_path),
                file_size = excluded.file_size,
                file_modified_at = excluded.file_modified_at,
                scan_status = excluded.scan_status",
            params![
                file_path,
                track.title,
                track.artist,
                track.album,
                track.album_artist,
                track.duration_ms,
                track.track_number,
                track.disc_number,
                track.year,
                track.genre,
                artwork,
                track.file_size,
                modified,
                track.scan_status.as_str(),
            ],
        )?;

        let id: i64 = self.conn.query_row(
            "SELECT id FROM tracks WHERE file_path = ?1",
            params![file_path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    /// Tandai track sudah diputar.
    pub fn mark_played(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tracks
             SET last_played_at = ?2, play_count = play_count + 1
             WHERE id = ?1",
            params![id, to_unix_seconds(SystemTime::now())],
        )?;
        Ok(())
    }

    /// Ubah status favorit sebuah track.
    pub fn set_favorite(&self, id: i64, favorite: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tracks SET is_favorite = ?2 WHERE id = ?1",
            params![id, i64::from(favorite)],
        )?;
        Ok(())
    }

    /// Hapus track dari library tanpa menghapus berkas fisiknya.
    pub fn remove(&self, id: i64) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Jumlah track dalam library.
    pub fn count(&self) -> rusqlite::Result<i64> {
        self.conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |row| row.get(0))
    }

    /// Daftar path file yang sudah terindeks, untuk mendeteksi file baru.
    pub fn existing_paths(&self) -> rusqlite::Result<std::collections::HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT file_path FROM tracks")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut paths = std::collections::HashSet::new();
        for row in rows {
            paths.insert(row?);
        }
        Ok(paths)
    }

    /// Tandai track yang file-nya hilang.
    pub fn mark_missing(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE tracks SET scan_status = 'missing' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}
