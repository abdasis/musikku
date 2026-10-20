//! Seed database dev dengan hasil pemindaian folder musik.
//!
//! Jalankan dengan: `cargo run --example seed_library -- <folder> <database>`

use std::rc::Rc;

use musikku::database::{Database, TrackRepository};
use musikku::library::{LibraryService, ScanControl, Scanner};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let folder = args.next().unwrap_or_else(|| "/tmp/musik-test".to_string());
    let database_path = args
        .next()
        .unwrap_or_else(|| "/tmp/musikku-seed.db".to_string());

    let database = Rc::new(Database::open(std::path::Path::new(&database_path))?);
    let artwork_dir = std::path::PathBuf::from("/tmp/musikku-artwork-test");
    let service = LibraryService::new(Rc::clone(&database), artwork_dir.clone());

    service.add_folder(std::path::Path::new(&folder))?;

    let scanner = Scanner::new(artwork_dir);
    let control = ScanControl::new();
    let known = std::collections::HashSet::new();
    let result = scanner.scan_folder(std::path::Path::new(&folder), &control, &known);

    for track in &result.tracks {
        service.persist_track(track);
    }

    let repo = TrackRepository::new(database.connection());
    println!("tersimpan {} lagu ke {}", repo.count()?, database_path);

    Ok(())
}
