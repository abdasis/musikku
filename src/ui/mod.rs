//! Modul UI: widget, halaman, dan jendela utama.

pub mod artwork;
pub mod context_menu;
pub mod folder_dialog;
pub mod now_playing;
pub mod pages;
pub mod player_bar;
pub mod playlist_dialog;
pub mod sidebar;
pub mod track_list;
pub mod track_row;
pub mod window;

pub use window::MainWindow;

use crate::settings::Theme;

/// Gaya CSS aplikasi.
///
/// Mostly rely on semantic colors libadwaita; CSS hanya untuk hal yang
/// benar-benar tidak tersedia lewat widget bawaan.
const APP_CSS: &str = r#"
.player-bar {
  background-color: @card_bg_color;
  border-top: 1px solid alpha(currentColor, 0.08);
  padding: 6px 12px;
  min-height: 80px;
}

.player-title {
  font-weight: bold;
}

.player-artist,
.now-playing-artist {
  font-size: 0.85em;
  opacity: 0.7;
}

.player-time {
  font-size: 0.8em;
  font-feature-settings: "tnum";
  opacity: 0.75;
}

.now-playing {
  padding: 0 4px;
}

.now-playing-title {
  font-weight: bold;
  font-size: 1.05em;
}

.now-playing-panel {
  background-color: @card_bg_color;
  border-left: 1px solid alpha(currentColor, 0.08);
}

.play-button {
  min-width: 44px;
  min-height: 44px;
  padding: 0;
}

.queue-list {
  background: transparent;
}

.queue-list row {
  border-radius: 8px;
  padding: 2px 4px;
}

.queue-active {
  background-color: alpha(@accent_bg_color, 0.16);
}

.library-scroller {
  background: transparent;
}

.artwork {
  border-radius: 8px;
  background-color: alpha(currentColor, 0.06);
}

.artwork-placeholder {
  opacity: 0.35;
}

.home-page {
  background: transparent;
}

.title-1 {
  font-weight: 800;
  font-size: 1.6em;
}
"#;

/// Muat CSS aplikasi ke dalam display default.
pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_string(APP_CSS);

    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

/// Terapkan tema tampilan yang dipilih.
pub fn apply_theme(theme: Theme) {
    let manager = adw::StyleManager::default();
    let scheme = match theme {
        Theme::System => adw::ColorScheme::Default,
        Theme::Light => adw::ColorScheme::ForceLight,
        Theme::Dark => adw::ColorScheme::ForceDark,
    };
    manager.set_color_scheme(scheme);
}
