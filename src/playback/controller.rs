//! Pengendali playback: menggabungkan mesin audio, antrean, dan mode pengulangan.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::core::model::{RepeatMode, Track};

use super::engine::{AudioEngine, PlaybackStatus};
use super::queue::Queue;

/// Callback yang menerima perubahan playback.
pub type PlaybackListener = Box<dyn Fn(PlaybackEvent) + Send + Sync>;

/// Event playback yang perlu diperbarui UI.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    /// Lagu yang sedang diputar berubah.
    ///
    /// Nilainya dibungkus agar ukuran enum tetap kecil.
    TrackChanged(Option<Box<Track>>),
    /// Status pemutaran berubah.
    StatusChanged(PlaybackStatus),
    /// Antrean berubah.
    QueueChanged,
    /// Posisi playback berubah.
    PositionChanged(i64),
    /// Volume atau status bisu berubah.
    VolumeChanged(f64, bool),
    /// Terjadi error pemutaran.
    Error(String),
}

/// Pengendali playback.
///
/// Menyimpan satu-satunya sumber kebenaran atas antrean, mode pengulangan,
/// dan status pemutaran; UI hanya membaca dan mengirim perintah.
pub struct PlaybackController {
    engine: Arc<AudioEngine>,
    queue: Mutex<Queue>,
    /// Track yang dikenal, dipakai untuk mencari track berdasarkan id antrean.
    track_cache: Mutex<HashMap<i64, Track>>,
    repeat_mode: Mutex<RepeatMode>,
    shuffle: Mutex<bool>,
    /// Seed pengacakan, diacak ulang tiap satu siklus.
    shuffle_seed: Mutex<u64>,
    listeners: Mutex<Vec<PlaybackListener>>,
    /// Pesan error terakhir, dipakai untuk menampilkan pesan ke pengguna.
    last_error: Mutex<Option<String>>,
}

impl PlaybackController {
    pub fn new(engine: Arc<AudioEngine>) -> Arc<Self> {
        let controller = Arc::new(Self {
            engine: Arc::clone(&engine),
            queue: Mutex::new(Queue::new()),
            track_cache: Mutex::new(HashMap::new()),
            repeat_mode: Mutex::new(RepeatMode::Off),
            shuffle: Mutex::new(false),
            shuffle_seed: Mutex::new(0x2545_F491_4F6C_DD1D),
            listeners: Mutex::new(Vec::new()),
            last_error: Mutex::new(None),
        });

        let ctrl = Arc::clone(&controller);
        engine.set_error_handler(move |message| {
            *lock(&ctrl.last_error) = Some(message.clone());
            ctrl.emit(PlaybackEvent::Error(message));
        });

        let ctrl = Arc::clone(&controller);
        engine.set_eos_handler(move || {
            ctrl.handle_end_of_stream();
        });

        controller
    }

    /// Daftarkan listener yang menerima perubahan playback.
    pub fn add_listener(&self, listener: impl Fn(PlaybackEvent) + Send + Sync + 'static) {
        lock(&self.listeners).push(Box::new(listener));
    }

    fn emit(&self, event: PlaybackEvent) {
        for listener in lock(&self.listeners).iter() {
            listener(event.clone());
        }
    }

    /// Status pemutaran saat ini.
    pub fn status(&self) -> PlaybackStatus {
        self.engine.status()
    }

    /// Lagu yang sedang diputar.
    pub fn current_track(&self) -> Option<Track> {
        lock(&self.queue).current_track().cloned()
    }

    /// Antrean putar.
    pub fn queue(&self) -> Queue {
        lock(&self.queue).clone()
    }

    /// Mode pengulangan saat ini.
    pub fn repeat_mode(&self) -> RepeatMode {
        *lock(&self.repeat_mode)
    }

    /// Ubah mode pengulangan.
    pub fn set_repeat_mode(&self, mode: RepeatMode) {
        *lock(&self.repeat_mode) = mode;
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    /// Ubah status acak.
    pub fn set_shuffle(&self, enabled: bool) {
        *lock(&self.shuffle) = enabled;
        if enabled {
            self.reseed();
        }
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    pub fn shuffle_enabled(&self) -> bool {
        *lock(&self.shuffle)
    }

    /// Pesan error terakhir, bila ada.
    pub fn last_error(&self) -> Option<String> {
        lock(&self.last_error).clone()
    }

    /// Kosongkan pesan error yang sudah ditampilkan.
    pub fn clear_error(&self) {
        *lock(&self.last_error) = None;
    }

    /// Mainkan daftar track dari indeks tertentu, menggantikan antrean.
    pub fn play_tracks(&self, tracks: &[Track], start_index: usize) {
        if tracks.is_empty() {
            return;
        }

        let start_id = tracks[start_index.min(tracks.len() - 1)].id;
        let ordered = if self.shuffle_enabled() {
            self.shuffled_tracks(tracks, start_id)
        } else {
            tracks.to_vec()
        };

        let start = ordered.iter().position(|t| t.id == start_id).unwrap_or(0);

        self.cache_tracks(&ordered);
        lock(&self.queue).replace(&ordered, start);
        self.emit(PlaybackEvent::QueueChanged);

        self.start_current();
    }

    /// Mainkan satu track, menggantikan antrean.
    pub fn play_single(&self, track: &Track) {
        self.play_tracks(std::slice::from_ref(track), 0);
    }

    /// Tambahkan track ke ujung antrean tanpa mengganggu pemutaran.
    pub fn enqueue(&self, track: &Track) {
        self.cache_tracks(std::slice::from_ref(track));
        lock(&self.queue).add(track);
        self.emit(PlaybackEvent::QueueChanged);
    }

    /// Tambahkan beberapa track ke ujung antrean.
    pub fn enqueue_many(&self, tracks: &[Track]) {
        self.cache_tracks(tracks);
        lock(&self.queue).add_many(tracks);
        self.emit(PlaybackEvent::QueueChanged);
    }

    /// Hapus track dari antrean.
    pub fn remove_from_queue(&self, track_id: i64) {
        let removed_current = lock(&self.queue).remove(track_id);
        lock(&self.track_cache).remove(&track_id);
        self.emit(PlaybackEvent::QueueChanged);

        if removed_current {
            if lock(&self.queue).current_index().is_none() {
                self.engine.stop();
                self.emit(PlaybackEvent::TrackChanged(None));
            } else {
                self.start_current();
            }
        }
    }

    /// Kosongkan antrean.
    pub fn clear_queue(&self) {
        lock(&self.queue).clear();
        lock(&self.track_cache).clear();
        self.engine.stop();
        self.emit(PlaybackEvent::QueueChanged);
        self.emit(PlaybackEvent::TrackChanged(None));
    }

    /// Susun ulang antrean.
    pub fn reorder_queue(&self, from: usize, to: usize) {
        lock(&self.queue).reorder(from, to);
        self.emit(PlaybackEvent::QueueChanged);
    }

    /// Lanjutkan pemutaran.
    pub fn playback_resume(&self) {
        self.engine.play();
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    /// Jeda pemutaran.
    pub fn playback_pause(&self) {
        self.engine.pause();
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    /// Hentikan pemutaran.
    pub fn playback_stop(&self) {
        self.engine.stop();
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    /// Mainkan atau jeda pemutaran.
    pub fn toggle_play_pause(&self) {
        match self.engine.status() {
            PlaybackStatus::Playing => self.engine.pause(),
            PlaybackStatus::Paused | PlaybackStatus::Buffering => self.engine.play(),
            PlaybackStatus::Stopped => {
                if lock(&self.queue).current_track().is_some() {
                    self.start_current();
                }
            }
        }
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    /// Lagu berikutnya.
    pub fn next(&self) {
        let repeat = self.repeat_mode();
        let Some(index) = lock(&self.queue).next_index(repeat) else {
            // Akhir antrean tanpa repeat: hentikan pemutaran.
            self.engine.stop();
            self.emit(PlaybackEvent::StatusChanged(PlaybackStatus::Stopped));
            return;
        };

        if let Some(track) = self.track_at(index) {
            lock(&self.queue).set_current_index(index, Some(track));
            self.emit(PlaybackEvent::TrackChanged(
                self.current_track().map(Box::new),
            ));
            self.start_current();
        }
    }

    /// Lagu sebelumnya.
    pub fn previous(&self) {
        let repeat = self.repeat_mode();
        let Some(index) = lock(&self.queue).previous_index(repeat) else {
            return;
        };

        if let Some(track) = self.track_at(index) {
            lock(&self.queue).set_current_index(index, Some(track));
            self.emit(PlaybackEvent::TrackChanged(
                self.current_track().map(Box::new),
            ));
            self.start_current();
        }
    }

    /// Lompat ke posisi tertentu.
    pub fn seek(&self, position_ms: i64) {
        self.engine.seek(position_ms);
        self.emit(PlaybackEvent::PositionChanged(position_ms));
    }

    /// Atur volume.
    pub fn set_volume(&self, volume: f64) {
        self.engine.set_volume(volume);
        self.emit(PlaybackEvent::VolumeChanged(
            self.engine.volume(),
            self.engine.is_muted(),
        ));
    }

    pub fn volume(&self) -> f64 {
        self.engine.volume()
    }

    /// Aktifkan atau matikan bisu.
    pub fn set_muted(&self, muted: bool) {
        self.engine.set_muted(muted);
        self.emit(PlaybackEvent::VolumeChanged(
            self.engine.volume(),
            self.engine.is_muted(),
        ));
    }

    pub fn is_muted(&self) -> bool {
        self.engine.is_muted()
    }

    /// Posisi playback saat ini.
    pub fn position_ms(&self) -> i64 {
        self.engine.position_ms()
    }

    /// Durasi lagu saat ini.
    pub fn duration_ms(&self) -> i64 {
        self.engine.duration_ms()
    }

    /// Laporkan posisi playback terbaru ke listener.
    ///
    /// Dipanggil timer UI agar slider posisi tetap akurat.
    pub fn emit_position_tick(&self) {
        if self.engine.status().is_active() {
            self.emit(PlaybackEvent::PositionChanged(self.engine.position_ms()));
        }
    }

    /// Mulai memutar lagu yang sedang aktif dalam antrean.
    fn start_current(&self) {
        let Some(track) = self.current_track() else {
            return;
        };

        self.engine.play_track(&track);
        self.emit(PlaybackEvent::TrackChanged(Some(Box::new(track))));
        self.emit(PlaybackEvent::StatusChanged(self.engine.status()));
    }

    /// Dipanggil ketika satu lagu selesai diputar.
    fn handle_end_of_stream(&self) {
        match self.repeat_mode() {
            RepeatMode::One => {
                // Ulangi lagu yang sama dari awal.
                self.seek(0);
                self.engine.play();
            }
            RepeatMode::Off | RepeatMode::All => self.next(),
        }
    }

    /// Ambil track pada indeks tertentu dari antrean.
    fn track_at(&self, index: usize) -> Option<Track> {
        let id = lock(&self.queue).order().get(index).copied()?;
        lock(&self.track_cache).get(&id).cloned()
    }

    /// Simpan track agar bisa ditemukan kembali lewat id antrean.
    fn cache_tracks(&self, tracks: &[Track]) {
        let mut cache = lock(&self.track_cache);
        for track in tracks {
            cache.insert(track.id, track.clone());
        }
    }

    /// Acak urutan track, tetapi tetap menempatkan `start_id` di posisi pertama.
    fn shuffled_tracks(&self, tracks: &[Track], start_id: i64) -> Vec<Track> {
        let mut others: Vec<Track> = tracks
            .iter()
            .filter(|t| t.id != start_id)
            .cloned()
            .collect();
        self.reseed();

        // Generator linear congruential sederhana, cukup untuk pengacakan antrean.
        let mut seed = *lock(&self.shuffle_seed);
        for index in (1..others.len()).rev() {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let swap = (seed >> 33) as usize % (index + 1);
            others.swap(index, swap);
        }

        let mut result = Vec::with_capacity(tracks.len());
        if let Some(first) = tracks.iter().find(|t| t.id == start_id) {
            result.push(first.clone());
        }
        result.extend(others);
        result
    }

    /// Acak ulang seed pengacakan.
    fn reseed(&self) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x2545_F491_4F6C_DD1D);
        *lock(&self.shuffle_seed) = seed | 1;
    }
}

/// Kunci mutex dan kembalikan nilainya; mengabaikan poisoning karena
/// tidak ada state yang tidak pulih setelah panic.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}
