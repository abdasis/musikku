//! MusikKu — pemutar musik lokal untuk Linux.
//!
//! Pustaka ini berisi seluruh logika aplikasi; binari `main.rs` hanya
//! menjadi titik masuk dan menjalankan siklus hidup aplikasi GTK.

pub mod app;
pub mod core;
pub mod database;
pub mod library;
pub mod mpris;
pub mod playback;
pub mod settings;
pub mod ui;
