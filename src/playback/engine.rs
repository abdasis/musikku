//! Mesin pemutar audio berbasis GStreamer.
//!
//! Engine ini membungkus `gst_play::Play` dan tidak tahu-menahu soal UI
//! maupun antrean; ia hanya mengubah berkas dan melaporkan status.

use std::path::Path;
use std::sync::{Arc, Mutex};


use crate::core::model::Track;

/// Callback yang dipanggil saat terjadi error pemutaran.
pub type ErrorHandler = Box<dyn Fn(String) + Send + Sync>;

/// Callback yang dipanggil saat lagu habis diputar.
pub type EndOfStreamHandler = Box<dyn Fn() + Send + Sync>;

/// Status pemutaran yang dilaporkan engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Stopped,
    Buffering,
    Playing,
    Paused,
}

impl PlaybackStatus {
    pub fn is_playing(self) -> bool {
        matches!(self, PlaybackStatus::Playing)
    }

    pub fn is_active(self) -> bool {
        matches!(
            self,
            PlaybackStatus::Playing | PlaybackStatus::Paused | PlaybackStatus::Buffering
        )
    }
}

/// Tuas volume dan bisu.
#[derive(Debug, Clone, Copy, PartialEq)]
struct VolumeState {
    volume: f64,
    muted: bool,
}

/// Mesin pemutar.
///
/// Semua method harus dipanggil dari thread utama GTK.
pub struct AudioEngine {
    play: gst_play::Play,
    /// Adapter yang menyediakan sinyal status dan error GStreamer.
    adapter: gst_play::PlaySignalAdapter,
    status: Mutex<PlaybackStatus>,
    volume: Mutex<VolumeState>,
    /// Posisi terakhir yang diketahui, dipakai saat UI belum sempat bertanya.
    position_ms: Mutex<i64>,
    duration_ms: Mutex<i64>,
    /// Dipanggil ketika engine gagal memutar sebuah berkas.
    on_error: Mutex<Option<ErrorHandler>>,
    /// Dipanggil ketika pemutaran selesai (lagu habis).
    on_eos: Mutex<Option<EndOfStreamHandler>>,
}

impl AudioEngine {
    /// Buat engine baru dan siapkan sinyal GStreamer.
    pub fn new() -> Arc<Self> {
        gst::init().expect("GStreamer gagal diinisialisasi");

        let play = gst_play::Play::new(None::<gst_play::PlayVideoRenderer>);

        // Sink audio otomatis: pipeline dapat diarahkan ke perangkat lain
        // lewat PipeWire/PulseAudio tanpa perubahan kode.
        let mut config = play.config();
        config.set("audio-sink", "autoaudiosink");
        if let Err(err) = play.set_config(config) {
            log::warn!("Gagal mengatur konfigurasi pemutaran: {err}");
        }

        let adapter = gst_play::PlaySignalAdapter::new(&play);

        let engine = Arc::new(Self {
            play,
            adapter,
            status: Mutex::new(PlaybackStatus::Stopped),
            volume: Mutex::new(VolumeState {
                volume: 0.8,
                muted: false,
            }),
            position_ms: Mutex::new(0),
            duration_ms: Mutex::new(0),
            on_error: Mutex::new(None),
            on_eos: Mutex::new(None),
        });

        engine.connect_signals();
        engine
    }

    fn connect_signals(self: &Arc<Self>) {
        let engine = Arc::clone(self);

        self.adapter.connect_state_changed(move |_, state| {
            let status = match state {
                gst_play::PlayState::Playing => PlaybackStatus::Playing,
                gst_play::PlayState::Paused => PlaybackStatus::Paused,
                gst_play::PlayState::Buffering => PlaybackStatus::Buffering,
                _ => PlaybackStatus::Stopped,
            };
            *lock(&engine.status) = status;
        });

        let engine = Arc::clone(self);
        self.adapter.connect_end_of_stream(move |_| {
            if let Some(callback) = lock(&engine.on_eos).as_ref() {
                callback();
            }
        });

        let engine = Arc::clone(self);
        self.adapter.connect_error(move |_, error, _| {
            let message = error.to_string();
            log::error!("Kesalahan pemutaran: {message}");
            if let Some(callback) = lock(&engine.on_error).as_ref() {
                callback(message);
            }
        });
    }

    /// Pasang callback yang dipanggil saat terjadi error pemutaran.
    pub fn set_error_handler(&self, handler: impl Fn(String) + Send + Sync + 'static) {
        *lock(&self.on_error) = Some(Box::new(handler));
    }

    /// Pasang callback yang dipanggil saat lagu habis diputar.
    pub fn set_eos_handler(&self, handler: impl Fn() + Send + Sync + 'static) {
        *lock(&self.on_eos) = Some(Box::new(handler));
    }

    /// Muat dan mulai memutar sebuah track.
    pub fn play_track(&self, track: &Track) {
        let Some(uri) = Self::path_to_uri(&track.file_path) else {
            self.report_error(format!(
                "Path tidak valid: {}",
                track.file_path.to_string_lossy()
            ));
            return;
        };

        self.play.set_uri(Some(&uri));
        self.play.play();
        *lock(&self.duration_ms) = track.duration_ms;
        *lock(&self.position_ms) = 0;
    }

    /// Muat sebuah track tanpa memutar.
    pub fn load_track(&self, track: &Track) {
        let Some(uri) = Self::path_to_uri(&track.file_path) else {
            return;
        };
        self.play.set_uri(Some(&uri));
        *lock(&self.duration_ms) = track.duration_ms;
        *lock(&self.position_ms) = 0;
    }

    /// Mulai atau lanjutkan pemutaran.
    pub fn play(&self) {
        self.play.play();
    }

    /// Jeda pemutaran.
    pub fn pause(&self) {
        self.play.pause();
    }

    /// Berhenti sepenuhnya dan kosongkan pipeline.
    pub fn stop(&self) {
        self.play.stop();
        self.play.set_uri(None);
        *lock(&self.status) = PlaybackStatus::Stopped;
        *lock(&self.position_ms) = 0;
    }

    /// Lompat ke posisi tertentu dalam milidetik.
    pub fn seek(&self, position_ms: i64) {
        if position_ms < 0 {
            return;
        }
        let position = gst::ClockTime::from_mseconds(position_ms as u64);
        self.play.seek(position);
        *lock(&self.position_ms) = position_ms;
    }

    /// Posisi playback saat ini dalam milidetik.
    pub fn position_ms(&self) -> i64 {
        match self.play.position() {
            Some(time) => {
                let ms = time.mseconds() as i64;
                *lock(&self.position_ms) = ms;
                ms
            }
            None => *lock(&self.position_ms),
        }
    }

    /// Durasi total lagu saat ini dalam milidetik.
    pub fn duration_ms(&self) -> i64 {
        if let Some(info) = self.play.media_info() {
            if let Some(duration) = info.duration() {
                let ms = duration.mseconds() as i64;
                *lock(&self.duration_ms) = ms;
                return ms;
            }
        }
        *lock(&self.duration_ms)
    }

    /// Atur volume; nilai 0.0 berarti bisu.
    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        self.play.set_volume(volume);
        lock(&self.volume).volume = volume;
    }

    pub fn volume(&self) -> f64 {
        lock(&self.volume).volume
    }

    /// Aktifkan atau matikan bisu.
    pub fn set_muted(&self, muted: bool) {
        self.play.set_mute(muted);
        lock(&self.volume).muted = muted;
    }

    pub fn is_muted(&self) -> bool {
        lock(&self.volume).muted
    }

    /// Status pemutaran saat ini.
    pub fn status(&self) -> PlaybackStatus {
        *lock(&self.status)
    }

    /// Apakah sink audio tersedia di sistem.
    pub fn has_audio_output() -> bool {
        gst::ElementFactory::make("autoaudiosink").build().is_ok()
    }

    /// Laporkan error ke callback yang terdaftar.
    fn report_error(&self, message: String) {
        log::error!("Kesalahan pemutaran: {message}");
        if let Some(callback) = lock(&self.on_error).as_ref() {
            callback(message);
        }
    }

    /// Ubah path berkas menjadi URI `file://`.
    ///
    /// Path non-ASCII dan yang mengandung spasi tetap aman karena
    /// komponen path di-escape satu per satu.
    pub fn path_to_uri(path: &Path) -> Option<String> {
        let encoded = url::Url::from_file_path(path).ok()?;
        Some(encoded.to_string())
    }
}

/// Kunci mutex dan kembalikan nilainya; mengabaikan poisoning karena
/// tidak ada state yang tidak pulih setelah panic.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_uri_escapes_spaces_and_unicode() {
        let path = Path::new("/home/user/MusikKu/Lagu Saya.mp3");
        let uri = AudioEngine::path_to_uri(path).expect("URI harus terbentuk");
        assert!(uri.starts_with("file:///home/user/MusikKu/"));
        assert!(!uri.contains(' '));
        assert!(uri.contains("%20"));
    }

    #[test]
    fn path_to_uri_keeps_unicode_escaped() {
        let path = Path::new("/home/user/MusikKu/Lagu Ä.mp3");
        let uri = AudioEngine::path_to_uri(path).expect("URI harus terbentuk");
        assert!(uri.starts_with("file:///"));
        assert!(!uri.contains('Ä'));
    }
}
