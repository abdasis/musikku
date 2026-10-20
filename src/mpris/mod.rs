//! Integrasi MPRIS agar aplikasi dapat dikendalikan dari media control
//! desktop dan menampilkan informasi lagu di panel sistem.

use std::sync::{Arc, Mutex, MutexGuard};

use mpris_server::{
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, Property, Time, TrackId, Volume,
};
use tokio::sync::mpsc;

use crate::core::model::{RepeatMode, Track};
use crate::playback::{PlaybackController, PlaybackStatus as EngineStatus};

/// Data yang dibagikan ke server MPRIS.
///
/// Server berjalan di thread async, sedangkan playback berjalan di thread
/// utama; data ditukar lewat mutex agar tetap konsisten.
#[derive(Default, Clone)]
pub struct MprisState {
    pub track: Option<Track>,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub volume: f64,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub playing: bool,
}

/// Perintah dari MPRIS ke aplikasi.
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Next,
    Previous,
    Stop,
    Seek(i64),
    SetVolume(f64),
    SetMuted(bool),
    SetShuffle(bool),
    SetRepeat(RepeatMode),
}

/// State bersama antara thread utama dan server MPRIS.
static SHARED_STATE: Mutex<Option<MprisState>> = Mutex::new(None);

/// Perbarui state MPRIS dari thread utama.
pub fn update_shared_state(snapshot: MprisState) {
    *lock(&SHARED_STATE) = Some(snapshot);
}

/// Baca state MPRIS terakhir.
pub fn shared_state() -> MprisState {
    lock(&SHARED_STATE).clone().unwrap_or_default()
}

/// Pengendali MPRIS.
pub struct MprisServer {
    server: Option<mpris_server::Server<MprisPlayer>>,
}

impl MprisServer {
    /// Siapkan server MPRIS. Kembalian `None` bila D-Bus tidak tersedia.
    pub async fn new(controller: Arc<PlaybackController>) -> Option<Arc<Self>> {
        let (tx, mut rx) = mpsc::unbounded_channel::<MprisCommand>();

        // Perintah MPRIS harus dijalankan di thread utama GTK karena
        // objek GStreamer tidak aman dipanggil dari thread lain.
        std::thread::spawn(move || {
            let context = glib::MainContext::default();
            while let Some(command) = rx.blocking_recv() {
                let controller = Arc::clone(&controller);
                context.invoke(move || {
                    handle_command(&controller, command);
                });
            }
        });

        let player = MprisPlayer {
            commands: tx.clone(),
        };

        let server = mpris_server::Server::new_with_all("io.github.musikku.MusikKu", player)
            .await
            .ok()?;

        let mpris = Arc::new(Self {
            server: Some(server),
        });

        Some(mpris)
    }

    /// Perbarui data yang dilihat desktop.
    pub fn update(&self, snapshot: MprisState) {
        update_shared_state(snapshot);
    }

    /// Kirim sinyal bahwa metadata lagu berubah.
    pub async fn emit_metadata_changed(&self) {
        if let Some(server) = &self.server {
            let metadata = build_metadata(&shared_state());
            let _ = server
                .properties_changed([Property::Metadata(metadata)])
                .await;
        }
    }

    /// Kirim sinyal bahwa status pemutaran berubah.
    pub async fn emit_playback_status_changed(&self) {
        if let Some(server) = &self.server {
            let status = if shared_state().playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            };
            let _ = server
                .properties_changed([Property::PlaybackStatus(status)])
                .await;
        }
    }
}

/// Terapkan perintah MPRIS ke pengendali playback.
fn handle_command(controller: &Arc<PlaybackController>, command: MprisCommand) {
    match command {
        MprisCommand::Play => controller.playback_resume(),
        MprisCommand::Pause => controller.playback_pause(),
        MprisCommand::PlayPause => controller.toggle_play_pause(),
        MprisCommand::Next => controller.next(),
        MprisCommand::Previous => controller.previous(),
        MprisCommand::Stop => controller.playback_stop(),
        MprisCommand::Seek(position) => controller.seek(position),
        MprisCommand::SetVolume(volume) => controller.set_volume(volume),
        MprisCommand::SetMuted(muted) => controller.set_muted(muted),
        MprisCommand::SetShuffle(shuffle) => controller.set_shuffle(shuffle),
        MprisCommand::SetRepeat(repeat) => controller.set_repeat_mode(repeat),
    }
}

/// Bangun metadata MPRIS dari state saat ini.
fn build_metadata(state: &MprisState) -> Metadata {
    let mut metadata = Metadata::new();

    let Some(track) = &state.track else {
        return metadata;
    };

    metadata.set_title(Some(track.display_title()));
    metadata.set_artist(Some([track.display_artist().to_string()]));
    metadata.set_album(Some(track.display_album().to_string()));

    if let Some(number) = track.track_number {
        metadata.set_track_number(Some(number as i32));
    }
    if let Some(year) = track.year {
        metadata.set("xesam:contentCreated", Some(year.to_string()));
    }

    metadata
}

/// Hasil operasi D-Bus.
type MprisResult<T> = mpris_server::zbus::fdo::Result<T>;

/// Implementasi antarmuka MPRIS.
struct MprisPlayer {
    commands: mpsc::UnboundedSender<MprisCommand>,
}

impl MprisPlayer {
    fn send(&self, command: MprisCommand) {
        let _ = self.commands.send(command);
    }
}

impl mpris_server::RootInterface for MprisPlayer {
    async fn raise(&self) -> MprisResult<()> {
        Ok(())
    }

    async fn quit(&self) -> MprisResult<()> {
        self.send(MprisCommand::Stop);
        Ok(())
    }

    async fn can_quit(&self) -> MprisResult<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> MprisResult<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> MprisResult<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> MprisResult<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> MprisResult<bool> {
        Ok(false)
    }

    async fn identity(&self) -> MprisResult<String> {
        Ok("MusikKu".to_string())
    }

    async fn desktop_entry(&self) -> MprisResult<String> {
        Ok("io.github.musikku.MusikKu".to_string())
    }

    async fn supported_uri_schemes(&self) -> MprisResult<Vec<String>> {
        Ok(vec!["file".to_string()])
    }

    async fn supported_mime_types(&self) -> MprisResult<Vec<String>> {
        Ok(vec!["audio/mpeg".to_string(), "audio/flac".to_string()])
    }
}

impl mpris_server::PlayerInterface for MprisPlayer {
    async fn next(&self) -> MprisResult<()> {
        self.send(MprisCommand::Next);
        Ok(())
    }

    async fn previous(&self) -> MprisResult<()> {
        self.send(MprisCommand::Previous);
        Ok(())
    }

    async fn pause(&self) -> MprisResult<()> {
        self.send(MprisCommand::Pause);
        Ok(())
    }

    async fn play_pause(&self) -> MprisResult<()> {
        self.send(MprisCommand::PlayPause);
        Ok(())
    }

    async fn stop(&self) -> MprisResult<()> {
        self.send(MprisCommand::Stop);
        Ok(())
    }

    async fn play(&self) -> MprisResult<()> {
        self.send(MprisCommand::Play);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> MprisResult<()> {
        self.send(MprisCommand::Seek(offset.as_micros() / 1000));
        Ok(())
    }

    async fn set_position(&self, _track_id: TrackId, position: Time) -> MprisResult<()> {
        self.send(MprisCommand::Seek(position.as_micros() / 1000));
        Ok(())
    }

    async fn open_uri(&self, _uri: String) -> MprisResult<()> {
        Ok(())
    }

    async fn playback_status(&self) -> MprisResult<PlaybackStatus> {
        Ok(if shared_state().playing {
            PlaybackStatus::Playing
        } else {
            PlaybackStatus::Paused
        })
    }

    async fn loop_status(&self) -> MprisResult<LoopStatus> {
        Ok(match shared_state().repeat {
            RepeatMode::Off => LoopStatus::None,
            RepeatMode::All => LoopStatus::Playlist,
            RepeatMode::One => LoopStatus::Track,
        })
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> mpris_server::zbus::Result<()> {
        let repeat = match loop_status {
            LoopStatus::None => RepeatMode::Off,
            LoopStatus::Playlist => RepeatMode::All,
            LoopStatus::Track => RepeatMode::One,
        };
        self.send(MprisCommand::SetRepeat(repeat));
        Ok(())
    }

    async fn rate(&self) -> MprisResult<PlaybackRate> {
        Ok(1.0)
    }

    async fn set_rate(&self, _rate: PlaybackRate) -> mpris_server::zbus::Result<()> {
        Ok(())
    }

    async fn shuffle(&self) -> MprisResult<bool> {
        Ok(shared_state().shuffle)
    }

    async fn set_shuffle(&self, shuffle: bool) -> mpris_server::zbus::Result<()> {
        self.send(MprisCommand::SetShuffle(shuffle));
        Ok(())
    }

    async fn metadata(&self) -> MprisResult<Metadata> {
        Ok(build_metadata(&shared_state()))
    }

    async fn volume(&self) -> MprisResult<Volume> {
        Ok(shared_state().volume)
    }

    async fn set_volume(&self, volume: Volume) -> mpris_server::zbus::Result<()> {
        self.send(MprisCommand::SetVolume(volume));
        Ok(())
    }

    async fn position(&self) -> MprisResult<Time> {
        Ok(Time::from_micros(shared_state().position_ms * 1000))
    }

    async fn minimum_rate(&self) -> MprisResult<PlaybackRate> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> MprisResult<PlaybackRate> {
        Ok(1.0)
    }

    async fn can_go_next(&self) -> MprisResult<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> MprisResult<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> MprisResult<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> MprisResult<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> MprisResult<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> MprisResult<bool> {
        Ok(true)
    }
}

impl mpris_server::TrackListInterface for MprisPlayer {
    async fn get_tracks_metadata(&self, _track_ids: Vec<TrackId>) -> MprisResult<Vec<Metadata>> {
        Ok(vec![])
    }

    async fn add_track(
        &self,
        _uri: mpris_server::Uri,
        _after_track: TrackId,
        _set_as_current: bool,
    ) -> MprisResult<()> {
        Ok(())
    }

    async fn remove_track(&self, _track_id: TrackId) -> MprisResult<()> {
        Ok(())
    }

    async fn go_to(&self, _track_id: TrackId) -> MprisResult<()> {
        Ok(())
    }

    async fn tracks(&self) -> MprisResult<Vec<TrackId>> {
        Ok(vec![])
    }

    async fn can_edit_tracks(&self) -> MprisResult<bool> {
        Ok(false)
    }
}

impl mpris_server::PlaylistsInterface for MprisPlayer {
    async fn activate_playlist(&self, _playlist_id: mpris_server::PlaylistId) -> MprisResult<()> {
        Ok(())
    }

    async fn get_playlists(
        &self,
        _index: u32,
        _max_count: u32,
        _order: mpris_server::PlaylistOrdering,
        _reverse_order: bool,
    ) -> MprisResult<Vec<mpris_server::Playlist>> {
        Ok(vec![])
    }

    async fn playlist_count(&self) -> MprisResult<u32> {
        Ok(0)
    }

    async fn orderings(&self) -> MprisResult<Vec<mpris_server::PlaylistOrdering>> {
        Ok(vec![])
    }

    async fn active_playlist(&self) -> MprisResult<Option<mpris_server::Playlist>> {
        Ok(None)
    }
}

/// Status pemutaran dari engine, diubah menjadi status MPRIS.
pub fn to_mpris_status(status: EngineStatus) -> bool {
    status.is_playing()
}

/// Kunci mutex dan kembalikan nilainya.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|err| err.into_inner())
}
