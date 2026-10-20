//! Modul library: pemindaian folder dan pembacaan metadata.

pub mod metadata;
pub mod scanner;
pub mod service;

pub use scanner::{ScanControl, ScanProgress, ScanResult, Scanner};
pub use service::{LibraryEvent, LibraryService, ScanHandle};
