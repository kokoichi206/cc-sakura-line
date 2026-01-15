use serde_json::Value;
use std::time::Instant;

use crate::data::claude;

pub fn clock(started_at: Instant) -> String {
    let elapsed = started_at.elapsed();
    format_duration(elapsed.as_secs())
}

pub fn from_input(input: Option<&Value>) -> String {
    let root = match input {
        Some(root) => root,
        None => return "-".to_string(),
    };

    let ms = claude::lookup_u64(root, &["cost", "total_duration_ms"])
        .or_else(|| claude::lookup_u64(root, &["session", "total_duration_ms"]))
        .or_else(|| claude::lookup_u64(root, &["session", "duration_ms"]))
        .or_else(|| claude::lookup_u64(root, &["total_duration_ms"]))
        .or_else(|| claude::lookup_u64(root, &["elapsed_ms"]));

    match ms {
        Some(ms) => format_duration(ms / 1000),
        None => "-".to_string(),
    }
}

fn format_duration(total_secs: u64) -> String {
    if total_secs < 60 {
        return "<1m".to_string();
    }

    if total_secs < 3600 {
        let minutes = total_secs / 60;
        return format!("{}m", minutes);
    }

    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    if minutes == 0 {
        format!("{}h", hours)
    } else {
        format!("{}h{}m", hours, minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;

    #[test]
    fn format_duration_humanized() {
        assert_eq!(format_duration(0), "<1m");
        assert_eq!(format_duration(59), "<1m");
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(1932), "32m");
        assert_eq!(format_duration(3600), "1h");
        assert_eq!(format_duration(19920), "5h32m");
    }
}
