//! Pembacaan metadata berkas audio memakai crate `lofty`.

use std::path::Path;

use lofty::prelude::*;
use lofty::tag::ItemKey;

use crate::core::model::{ScanStatus, Track};

/// Metadata mentah yang berhasil dibaca dari sebuah berkas.
pub struct FileMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub duration_ms: i64,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    /// Data gambar album (cover), jika ada.
    pub artwork: Option<Vec<u8>>,
    /// Tipe gambar album, mis. "image/jpeg".
    pub artwork_mime: Option<String>,
}

/// Baca metadata dari sebuah berkas audio.
///
/// Mengembalikan `Err` bila format tidak dikenali atau berkasnya rusak.
/// Berkas yang_metadata-nya kosong tetap valid: nilai kosong diisit nanti
/// oleh pemanggil dari nama berkas.
pub fn read_metadata(path: &Path) -> Result<FileMetadata, lofty::error::FileParseError> {
    let tagged = lofty::read_from_path(path)?;
    let duration_ms = tagged.properties().duration().as_millis() as i64;

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let mut metadata = FileMetadata {
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        album_artist: String::new(),
        duration_ms,
        track_number: None,
        disc_number: None,
        year: None,
        genre: None,
        artwork: None,
        artwork_mime: None,
    };

    if let Some(tag) = tag {
        metadata.title = tag
            .get_string(ItemKey::TrackTitle)
            .unwrap_or_default()
            .to_string();
        metadata.artist = tag
            .get_string(ItemKey::TrackArtist)
            .unwrap_or_default()
            .to_string();
        metadata.album = tag
            .get_string(ItemKey::AlbumTitle)
            .unwrap_or_default()
            .to_string();
        metadata.album_artist = tag
            .get_string(ItemKey::AlbumArtist)
            .unwrap_or_default()
            .to_string();
        metadata.genre = tag.get_string(ItemKey::Genre).map(str::to_string);

        if let Some(number) = tag.get_string(ItemKey::TrackNumber) {
            metadata.track_number = parse_leading_number(number);
        }
        if let Some(number) = tag.get_string(ItemKey::DiscNumber) {
            metadata.disc_number = parse_leading_number(number);
        }

        // Tahun bisa disimpan sebagai "year" atau sebagai bagian dari
        // "date"/"recording date"; keduanya perlu diperiksa.
        metadata.year = tag
            .get_string(ItemKey::Year)
            .and_then(parse_leading_number)
            .or_else(|| {
                tag.get_string(ItemKey::RecordingDate)
                    .and_then(parse_leading_number)
            })
            .or_else(|| {
                tag.get_string(ItemKey::ReleaseDate)
                    .and_then(parse_leading_number)
            });

        if let Some(picture) = tag.pictures().first() {
            metadata.artwork = Some(picture.data().to_vec());
            metadata.artwork_mime = Some(
                picture
                    .mime_type()
                    .map(|m| m.as_str().to_string())
                    .unwrap_or_else(|| "image/jpeg".to_string()),
            );
        }
    }

    Ok(metadata)
}

/// Ambil angka di awal string, mis. "3/12" -> 3 dan "1998" -> 1998.
fn parse_leading_number(value: &str) -> Option<u32> {
    let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Susun track dari metadata berkas dan informasi sistem berkas.
pub fn build_track(path: &Path, metadata: &FileMetadata, artwork_path: Option<std::path::PathBuf>) -> Track {
    let file_size = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(0);
    let file_modified_at = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok();

    Track {
        id: 0,
        file_path: path.to_path_buf(),
        title: metadata.title.clone(),
        artist: metadata.artist.clone(),
        album: metadata.album.clone(),
        album_artist: metadata.album_artist.clone(),
        duration_ms: metadata.duration_ms,
        track_number: metadata.track_number,
        disc_number: metadata.disc_number,
        year: metadata.year,
        genre: metadata.genre.clone(),
        artwork_path,
        file_size,
        file_modified_at,
        scan_status: ScanStatus::Ready,
        last_played_at: None,
        play_count: 0,
        is_favorite: false,
    }
}

/// Tulis data gambar album ke berkas dan kembalikan path-nya.
pub fn save_artwork(data: &[u8], mime: Option<&str>, target_dir: &Path) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;

    let extension = match mime {
        Some(m) if m.contains("png") => "png",
        _ => "jpg",
    };
    std::fs::create_dir_all(target_dir)?;

    // Nama berkas diturunkan dari hash isi agar cover yang sama tidak
    // ditulis berulang kali.
    let digest = simple_hash(data);
    let file_name = format!("{digest}.{extension}");
    let path = target_dir.join(file_name);

    if !path.exists() {
        let mut file = std::fs::File::create(&path)?;
        file.write_all(data)?;
    }

    Ok(path)
}

/// Hash isi yang sederhana dan stabil (FNV-1a 64-bit).
fn simple_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}
