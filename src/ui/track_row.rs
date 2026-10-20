//! Objek GObject yang membungkus satu track untuk dipakai di model daftar.
//!
//! `GtkColumnView` butuh tiap baris berupa objek GObject agar data tetap
//! terikat ke baris-nya saat daftar disaring atau disortir.

use std::cell::RefCell;
use std::sync::OnceLock;

use glib::prelude::*;
use glib::subclass::prelude::*;

use crate::core::model::Track;
use crate::core::time::format_duration;

/// Nama properti pada baris lagu.
mod prop {
    pub const TRACK_ID: &str = "track-id";
    pub const TITLE: &str = "title";
    pub const ARTIST: &str = "artist";
    pub const ALBUM: &str = "album";
    pub const DURATION: &str = "duration";
    pub const ARTWORK_PATH: &str = "artwork-path";
}

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct TrackRow {
        pub track: RefCell<Option<Track>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TrackRow {
        const NAME: &'static str = "MusikKuTrackRow";
        type Type = super::TrackRow;
    }

    impl ObjectImpl for TrackRow {
        fn properties() -> &'static [glib::ParamSpec] {
            static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();
            PROPERTIES.get_or_init(|| {
                vec![
                    glib::ParamSpecInt64::builder(prop::TRACK_ID)
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder(prop::TITLE)
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder(prop::ARTIST)
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder(prop::ALBUM)
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder(prop::DURATION)
                        .read_only()
                        .build(),
                    glib::ParamSpecString::builder(prop::ARTWORK_PATH)
                        .read_only()
                        .build(),
                ]
            })
        }

        fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            let mut track = self.track.borrow_mut();
            let Some(track) = track.as_mut() else {
                return;
            };

            match pspec.name() {
                prop::TRACK_ID => track.id = value.get::<i64>().unwrap_or(0),
                prop::TITLE => track.title = value.get::<String>().unwrap_or_default(),
                prop::ARTIST => track.artist = value.get::<String>().unwrap_or_default(),
                prop::ALBUM => track.album = value.get::<String>().unwrap_or_default(),
                prop::DURATION => {
                    track.duration_ms = value
                        .get::<String>()
                        .unwrap_or_default()
                        .parse()
                        .unwrap_or(0)
                }
                prop::ARTWORK_PATH => {
                    let text = value.get::<String>().unwrap_or_default();
                    track.artwork_path = if text.is_empty() {
                        None
                    } else {
                        Some(std::path::PathBuf::from(text))
                    };
                }
                _ => unreachable!("properti tidak dikenal: {}", pspec.name()),
            }
        }

        fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            let track = self.track.borrow();
            let track = track.as_ref();

            match pspec.name() {
                prop::TRACK_ID => track.map(|t| t.id).unwrap_or(0).to_value(),
                prop::TITLE => track
                    .map(|t| t.display_title().to_string())
                    .unwrap_or_default()
                    .to_value(),
                prop::ARTIST => track
                    .map(|t| t.display_artist().to_string())
                    .unwrap_or_default()
                    .to_value(),
                prop::ALBUM => track
                    .map(|t| t.display_album().to_string())
                    .unwrap_or_default()
                    .to_value(),
                prop::DURATION => track
                    .map(|t| format_duration(t.duration_ms))
                    .unwrap_or_default()
                    .to_value(),
                prop::ARTWORK_PATH => track
                    .and_then(|t| t.artwork_path.as_ref())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default()
                    .to_value(),
                _ => unreachable!("properti tidak dikenal: {}", pspec.name()),
            }
        }
    }
}

glib::wrapper! {
    /// Baris lagu yang dapat dipakai pada `GtkListStore`.
    pub struct TrackRow(ObjectSubclass<imp::TrackRow>);
}

impl TrackRow {
    /// Buat baris baru dari sebuah track.
    pub fn from_track(track: &Track) -> Self {
        let row: Self = glib::Object::builder().build();
        row.imp().track.replace(Some(track.clone()));
        row
    }

    /// Track yang dibungkus baris ini.
    pub fn track(&self) -> Option<Track> {
        self.imp().track.borrow().clone()
    }

    /// Id track, dipakai untuk mencari track terpilih.
    pub fn track_id(&self) -> i64 {
        self.imp()
            .track
            .borrow()
            .as_ref()
            .map(|t| t.id)
            .unwrap_or(0)
    }
}
