use serde_json::Value;
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::data::claude;

/// One quota window. Keep used_percentage numeric so the gauge can be drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageGauge {
    pub used_percentage: f64,
    pub reset_eta: String,
}

pub struct RateLimitInfo {
    pub five_hour: Option<UsageGauge>,
    pub seven_day: Option<UsageGauge>,
}

pub fn from_input(input: Option<&Value>) -> RateLimitInfo {
    let now = unix_now();

    RateLimitInfo {
        five_hour: gauge(
            input,
            "five_hour",
            "CC_RATE_FIVE_HOUR_USED",
            "CC_RATE_FIVE_HOUR_RESET",
            now,
        ),
        seven_day: gauge(
            input,
            "seven_day",
            "CC_RATE_SEVEN_DAY_USED",
            "CC_RATE_SEVEN_DAY_RESET",
            now,
        ),
    }
}

fn gauge(
    input: Option<&Value>,
    window_key: &str,
    used_env: &str,
    reset_env: &str,
    now: Option<u64>,
) -> Option<UsageGauge> {
    let used_percentage = env_f64(used_env)
        .or_else(|| lookup(input, window_key, "used_percentage"))?
        .clamp(0.0, 100.0);

    let reset_eta = env_label(reset_env).unwrap_or_else(|| {
        let resets_at = lookup(input, window_key, "resets_at");
        match (resets_at, now) {
            (Some(resets_at), Some(now)) => {
                format_reset_eta((resets_at as u64).saturating_sub(now))
            }
            _ => "-".to_string(),
        }
    });

    Some(UsageGauge {
        used_percentage,
        reset_eta,
    })
}

fn lookup(input: Option<&Value>, window_key: &str, field: &str) -> Option<f64> {
    claude::lookup_f64(input?, &["rate_limits", window_key, field])
}

fn format_reset_eta(total_secs: u64) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    if total_secs < MINUTE {
        return "<1m".to_string();
    }

    if total_secs < HOUR {
        return format!("{}m", total_secs / MINUTE);
    }

    if total_secs < DAY {
        let hours = total_secs / HOUR;
        let minutes = (total_secs % HOUR) / MINUTE;
        return if minutes == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h{}m", hours, minutes)
        };
    }

    let days = total_secs / DAY;
    let hours = (total_secs % DAY) / HOUR;
    if hours == 0 {
        format!("{}d", days)
    } else {
        format!("{}d{}h", days, hours)
    }
}

/// `resets_at` is unix seconds. Treating it as milliseconds shifts the countdown by 1000x.
fn unix_now() -> Option<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|elapsed| elapsed.as_secs())
}

fn env_label(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.is_empty())
}

fn env_f64(key: &str) -> Option<f64> {
    env::var(key).ok()?.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::{format_reset_eta, from_input, gauge};
    use serde_json::json;

    #[test]
    fn format_reset_eta_humanized() {
        assert_eq!(format_reset_eta(0), "<1m");
        assert_eq!(format_reset_eta(59), "<1m");
        assert_eq!(format_reset_eta(60), "1m");
        assert_eq!(format_reset_eta(2460), "41m");
        assert_eq!(format_reset_eta(7200), "2h");
        assert_eq!(format_reset_eta(9660), "2h41m");
        assert_eq!(format_reset_eta(259200), "3d");
        assert_eq!(format_reset_eta(273600), "3d4h");
    }

    #[test]
    fn gauge_reads_claude_code_payload() {
        let input = json!({
            "rate_limits": {
                "five_hour": { "used_percentage": 53.4, "resets_at": 1_800_009_660u64 }
            }
        });

        let five_hour = gauge(
            Some(&input),
            "five_hour",
            "CC_RATE_TEST_UNSET_USED",
            "CC_RATE_TEST_UNSET_RESET",
            Some(1_800_000_000),
        )
        .expect("five_hour present");

        assert_eq!(five_hour.used_percentage, 53.4);
        assert_eq!(five_hour.reset_eta, "2h41m");
    }

    #[test]
    fn gauge_clamps_out_of_range() {
        let input = json!({ "rate_limits": { "five_hour": { "used_percentage": 140 } } });
        let five_hour = gauge(
            Some(&input),
            "five_hour",
            "CC_RATE_TEST_UNSET_USED",
            "CC_RATE_TEST_UNSET_RESET",
            Some(0),
        )
        .expect("five_hour present");
        assert_eq!(five_hour.used_percentage, 100.0);
    }

    #[test]
    fn gauge_missing_reset_keeps_placeholder() {
        let input = json!({ "rate_limits": { "seven_day": { "used_percentage": 9 } } });
        let seven_day = gauge(
            Some(&input),
            "seven_day",
            "CC_RATE_TEST_UNSET_USED",
            "CC_RATE_TEST_UNSET_RESET",
            Some(0),
        )
        .expect("seven_day present");
        assert_eq!(seven_day.reset_eta, "-");
    }

    #[test]
    fn absent_rate_limits_yield_none() {
        let input = json!({ "model": { "display_name": "Opus" } });
        let info = from_input(Some(&input));
        assert!(info.five_hour.is_none());
        assert!(info.seven_day.is_none());
    }
}
