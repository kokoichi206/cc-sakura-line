use serde_json::Value;
use std::env;

use crate::data::claude;

pub struct ContextInfo {
    pub context: String,
    pub remaining: String,
}

pub fn from_input(input: Option<&Value>) -> ContextInfo {
    let label_override = env_or("CC_CONTEXT_LABEL", "");
    let remaining_override = env_or("CC_CONTEXT_REMAINING", "");

    let used_env = env_u64("CC_CONTEXT_USED");
    let total_env = env_u64("CC_CONTEXT_TOTAL");
    let (used, total) = if used_env.is_some() || total_env.is_some() {
        (used_env, total_env)
    } else {
        context_from_json(input)
    };

    let context = if let Some(label) = label_override {
        if !label.is_empty() {
            label
        } else {
            format_context(used, total)
        }
    } else {
        format_context(used, total)
    };

    let remaining = if let Some(rem) = remaining_override {
        if !rem.is_empty() {
            rem
        } else {
            format_remaining(used, total)
        }
    } else {
        format_remaining(used, total)
    };

    ContextInfo { context, remaining }
}

fn context_from_json(input: Option<&Value>) -> (Option<u64>, Option<u64>) {
    let root = match input {
        Some(root) => root,
        None => return (None, None),
    };

    let total = claude::lookup_u64(root, &["context_window", "context_window_size"]);

    let usage = [
        claude::lookup_u64(root, &["context_window", "current_usage", "input_tokens"]),
        claude::lookup_u64(root, &["context_window", "current_usage", "output_tokens"]),
        claude::lookup_u64(
            root,
            &[
                "context_window",
                "current_usage",
                "cache_creation_input_tokens",
            ],
        ),
        claude::lookup_u64(
            root,
            &["context_window", "current_usage", "cache_read_input_tokens"],
        ),
    ];

    let mut used_sum: Option<u64> = None;
    for value in usage.into_iter().flatten() {
        used_sum = Some(used_sum.unwrap_or(0) + value);
    }

    (used_sum, total)
}

fn format_context(used: Option<u64>, total: Option<u64>) -> String {
    match (used, total) {
        (Some(used), Some(total)) if total > 0 => format!("{}/{}", used, total),
        (Some(used), None) => format!("{}", used),
        _ => "-".to_string(),
    }
}

fn format_remaining(used: Option<u64>, total: Option<u64>) -> String {
    match (used, total) {
        (Some(used), Some(total)) if total > 0 => {
            let used_pct = (used as f64 / total as f64) * 100.0;
            let remaining_pct = (100.0 - used_pct).round().max(0.0);
            format!("{}% left", remaining_pct as u64)
        }
        _ => "-".to_string(),
    }
}

/// Read an env var or return a fallback; empty defaults yield `None`.
pub fn env_or(key: &str, default: &str) -> Option<String> {
    match env::var(key) {
        Ok(val) => Some(val),
        Err(_) if default.is_empty() => None,
        Err(_) => Some(default.to_string()),
    }
}

fn env_u64(key: &str) -> Option<u64> {
    env::var(key).ok().and_then(|v| v.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::{format_context, format_remaining};

    #[test]
    fn format_context_with_total() {
        assert_eq!(format_context(Some(120), Some(200)), "120/200");
    }

    #[test]
    fn format_remaining_percent() {
        assert_eq!(format_remaining(Some(50), Some(200)), "75% left");
    }

    #[test]
    fn format_remaining_unknown() {
        assert_eq!(format_remaining(None, None), "-");
    }
}
