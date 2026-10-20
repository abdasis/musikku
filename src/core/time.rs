//! Format waktu dan durasi untuk tampilan.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Ubah milidetik menjadi `mm:ss` (atau `h:mm:ss` bilamelebihi satu jam).
pub fn format_duration(ms: i64) -> String {
    if ms < 0 {
        return "0:00".to_string();
    }
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// Ubah `SystemTime` menjadi timestamp detik sejak epoch.
pub fn to_unix_seconds(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Ubah timestamp detik epoch kembali menjadi `SystemTime`.
pub fn from_unix_seconds(seconds: i64) -> Option<SystemTime> {
    if seconds <= 0 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))
}

/// Format durasi relatif ringkas, mis. "3 hari lalu".
pub fn format_relative(time: SystemTime) -> String {
    let elapsed = SystemTime::now()
        .duration_since(time)
        .unwrap_or(Duration::ZERO)
        .as_secs();

    match elapsed {
        s if s < 60 => "Baru saja".to_string(),
        s if s < 3600 => format!("{} menit lalu", s / 60),
        s if s < 86_400 => format!("{} jam lalu", s / 3600),
        s if s < 2_592_000 => format!("{} hari lalu", s / 86_400),
        s if s < 31_536_000 => format!("{} bulan lalu", s / 2_592_000),
        s => format!("{} tahun lalu", s / 31_536_000),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_under_one_minute_has_no_leading_zero() {
        assert_eq!(format_duration(0), "0:00");
        assert_eq!(format_duration(5_000), "0:05");
        assert_eq!(format_duration(65_000), "1:05");
    }

    #[test]
    fn duration_over_one_hour_includes_hours() {
        assert_eq!(format_duration(3_661_000), "1:01:01");
    }

    #[test]
    fn negative_duration_falls_back_to_zero() {
        assert_eq!(format_duration(-1), "0:00");
    }

    #[test]
    fn unix_seconds_round_trip() {
        let time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let seconds = to_unix_seconds(time);
        assert_eq!(from_unix_seconds(seconds), Some(time));
    }

    #[test]
    fn zero_timestamp_has_no_system_time() {
        assert_eq!(from_unix_seconds(0), None);
    }
}
