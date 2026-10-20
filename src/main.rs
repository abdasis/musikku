//! Titik masuk aplikasi MusikKu.

use musikku::app::MusikKuApp;

fn main() -> glib::ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let application = MusikKuApp::new();
    application.run()
}
