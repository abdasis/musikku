//! Antrean putar: daftar track aktif beserta posisi lagu yang sedang diputar.

use crate::core::model::Track;

/// Antrean putar.
///
/// Antrean bertahan antar halaman karena tidak pernah di-reset saat
/// pengguna berpindah tampilan.
#[derive(Debug, Default, Clone)]
pub struct Queue {
    /// Urutan track id dalam antrean.
    order: Vec<i64>,
    /// Indeks lagu yang sedang diputar di dalam `order`.
    current_index: Option<usize>,
    /// Track yang sedang diputar, dipakai untuk tampilan.
    current_track: Option<Track>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Bangun antrean dari urutan track id yang disimpan.
    pub fn from_order(order: Vec<i64>) -> Self {
        Self {
            order,
            current_index: None,
            current_track: None,
        }
    }

    /// Track id dalam antrean sesuai urutannya.
    pub fn order(&self) -> &[i64] {
        &self.order
    }

    /// Track yang sedang diputar.
    pub fn current_track(&self) -> Option<&Track> {
        self.current_track.as_ref()
    }

    /// Indeks lagu aktif dalam antrean.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Jumlah track dalam antrean.
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// Tambahkan track ke ujung antrean.
    ///
    /// Track yang sudah ada dipindahkan ke posisi baru, bukan diduplikasi.
    pub fn add(&mut self, track: &Track) {
        self.remove(track.id);
        self.order.push(track.id);

        if self.current_index.is_none() {
            self.current_index = Some(self.order.len() - 1);
            self.current_track = Some(track.clone());
        }
    }

    /// Tambahkan beberapa track sekaligus.
    pub fn add_many(&mut self, tracks: &[Track]) {
        for track in tracks {
            self.add(track);
        }
    }

    /// Ganti seluruh isi antrean.
    pub fn replace(&mut self, tracks: &[Track], start_index: usize) {
        self.order.clear();
        self.order.extend(tracks.iter().map(|t| t.id));
        self.current_index = if self.order.is_empty() {
            None
        } else {
            Some(start_index.min(self.order.len() - 1))
        };
        self.current_track = tracks
            .get(self.current_index.unwrap_or(0))
            .cloned();
    }

    /// Hapus track dari antrean.
    ///
    /// Mengembalikan `true` bila track yang sedang diputar ikut terhapus.
    pub fn remove(&mut self, track_id: i64) -> bool {
        let Some(position) = self.order.iter().position(|id| *id == track_id) else {
            return false;
        };

        self.order.remove(position);

        let removed_current = self.current_index == Some(position);
        match self.current_index {
            Some(_index) if removed_current => {
                // Lagu aktif dihapus: lanjut ke lagu berikutnya bila ada.
                if self.order.is_empty() {
                    self.current_index = None;
                    self.current_track = None;
                } else {
                    let next = position.min(self.order.len() - 1);
                    self.current_index = Some(next);
                }
            }
            Some(index) if position < index => {
                self.current_index = Some(index - 1);
            }
            _ => {}
        }

        removed_current
    }

    /// Kosongkan antrean.
    pub fn clear(&mut self) {
        self.order.clear();
        self.current_index = None;
        self.current_track = None;
    }

    /// Atur lagu aktif berdasarkan indeks dalam antrean.
    pub fn set_current_index(&mut self, index: usize, track: Option<Track>) {
        if index < self.order.len() {
            self.current_index = Some(index);
            self.current_track = track;
        }
    }

    /// Pindahkan track dari satu posisi ke posisi lain.
    pub fn reorder(&mut self, from: usize, to: usize) {
        if from >= self.order.len() || to >= self.order.len() || from == to {
            return;
        }

        let track_id = self.order.remove(from);
        self.order.insert(to, track_id);

        // Indeks lagu aktif menyesuaikan bila bergeser.
        if let Some(current) = self.current_index {
            self.current_index = Some(match current {
                c if c == from => to,
                c if from < c && c <= to => c - 1,
                c if to <= c && c < from => c + 1,
                c => c,
            });
        }
    }

    /// Id track berikutnya setelah lagu aktif, mengikuti mode pengulangan.
    pub fn next_index(&self, repeat: crate::core::model::RepeatMode) -> Option<usize> {
        self.step(1, repeat)
    }

    /// Id track sebelumnya sebelum lagu aktif, mengikuti mode pengulangan.
    pub fn previous_index(&self, repeat: crate::core::model::RepeatMode) -> Option<usize> {
        self.step(-1, repeat)
    }

    fn step(&self, delta: isize, repeat: crate::core::model::RepeatMode) -> Option<usize> {
        if self.order.is_empty() {
            return None;
        }

        let current = self.current_index.unwrap_or(0) as isize;
        let length = self.order.len() as isize;
        let next = current + delta;

        if next < 0 {
            return match repeat {
                crate::core::model::RepeatMode::All => Some((length - 1) as usize),
                _ => Some(0),
            };
        }

        if next >= length {
            return match repeat {
                crate::core::model::RepeatMode::All => Some(0),
                crate::core::model::RepeatMode::One => Some(current as usize),
                _ => None,
            };
        }

        Some(next as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::RepeatMode;

    fn track(id: i64) -> Track {
        Track {
            id,
            file_path: format!("/music/{id}.mp3").into(),
            title: format!("Lagu {id}"),
            artist: "Artis".into(),
            album: "Album".into(),
            album_artist: String::new(),
            duration_ms: 1000,
            track_number: None,
            disc_number: None,
            year: None,
            genre: None,
            artwork_path: None,
            file_size: 0,
            file_modified_at: None,
            scan_status: crate::core::model::ScanStatus::Ready,
            last_played_at: None,
            play_count: 0,
            is_favorite: false,
        }
    }

    #[test]
    fn adding_first_track_makes_it_current() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        assert_eq!(queue.current_index(), Some(0));
        assert_eq!(queue.current_track().map(|t| t.id), Some(1));
    }

    #[test]
    fn adding_duplicate_moves_it_to_the_end() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        queue.add(&track(2));
        queue.add(&track(1));
        assert_eq!(queue.order(), &[2, 1]);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn removing_track_before_current_shifts_index() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        queue.add(&track(2));
        queue.add(&track(3));
        queue.set_current_index(2, Some(track(3)));
        queue.remove(1);
        assert_eq!(queue.current_index(), Some(1));
    }

    #[test]
    fn removing_current_track_moves_to_next() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        queue.add(&track(2));
        queue.add(&track(3));
        queue.set_current_index(1, Some(track(2)));
        assert!(queue.remove(2));
        assert_eq!(queue.current_index(), Some(1));
    }

    #[test]
    fn next_wraps_only_when_repeat_all() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        queue.add(&track(2));
        queue.set_current_index(1, Some(track(2)));

        assert_eq!(queue.next_index(RepeatMode::Off), None);
        assert_eq!(queue.next_index(RepeatMode::All), Some(0));
    }

    #[test]
    fn moving_track_updates_current_index() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        queue.add(&track(2));
        queue.add(&track(3));
        queue.set_current_index(0, Some(track(1)));

        queue.reorder(0, 2);
        assert_eq!(queue.order(), &[2, 3, 1]);
        assert_eq!(queue.current_index(), Some(2));
    }

    #[test]
    fn clearing_queue_resets_current() {
        let mut queue = Queue::new();
        queue.add(&track(1));
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.current_index(), None);
    }
}
