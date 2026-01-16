use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

const CACHE_TTL_SECS: u64 = 300; // 5 minutes

#[derive(Debug)]
struct Cache {
    contributions: u32,
    updated_at: SystemTime,
}

pub fn today_contributions() -> String {
    if let Ok(val) = env::var("CC_CONTRIBUTIONS") {
        if !val.is_empty() {
            return format!("🌲 {}", val);
        }
    }

    match get_contributions_cached() {
        Some(count) => format!("🌲 {}", count),
        None => "-".to_string(),
    }
}

fn get_contributions_cached() -> Option<u32> {
    let cache_path = cache_file_path()?;

    // Try to read from cache
    if let Some(cache) = read_cache(&cache_path) {
        let elapsed = SystemTime::now()
            .duration_since(cache.updated_at)
            .unwrap_or(Duration::from_secs(u64::MAX));

        if elapsed < Duration::from_secs(CACHE_TTL_SECS) {
            return Some(cache.contributions);
        }
    }

    // Cache miss or expired - fetch fresh data
    let contributions = fetch_today_contributions()?;
    write_cache(&cache_path, contributions);
    Some(contributions)
}

fn cache_file_path() -> Option<PathBuf> {
    let cache_dir = dirs_cache_dir()?;
    let app_cache = cache_dir.join("cc-sakura-line");
    Some(app_cache.join("github_contributions.txt"))
}

fn dirs_cache_dir() -> Option<PathBuf> {
    env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".cache"))
}

fn read_cache(path: &PathBuf) -> Option<Cache> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let content = fs::read_to_string(path).ok()?;
    let contributions = content.trim().parse().ok()?;
    Some(Cache {
        contributions,
        updated_at: modified,
    })
}

fn write_cache(path: &PathBuf, contributions: u32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, contributions.to_string());
}

fn fetch_today_contributions() -> Option<u32> {
    let username = get_github_username()?;
    let today = get_today_date();

    let query = format!(
        r#"{{
  user(login: "{}") {{
    contributionsCollection {{
      contributionCalendar {{
        weeks {{
          contributionDays {{
            contributionCount
            date
          }}
        }}
      }}
    }}
  }}
}}"#,
        username
    );

    let output = Command::new("gh")
        .args(["api", "graphql", "-f", &format!("query={}", query)])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;

    // Navigate to the contribution days
    let weeks = json
        .get("data")?
        .get("user")?
        .get("contributionsCollection")?
        .get("contributionCalendar")?
        .get("weeks")?
        .as_array()?;

    // Find today's contribution
    for week in weeks.iter().rev() {
        if let Some(days) = week.get("contributionDays").and_then(|d| d.as_array()) {
            for day in days.iter().rev() {
                if day.get("date").and_then(|d| d.as_str()) == Some(&today) {
                    return day
                        .get("contributionCount")
                        .and_then(|c| c.as_u64())
                        .map(|c| c as u32);
                }
            }
        }
    }

    Some(0)
}

fn get_github_username() -> Option<String> {
    if let Ok(val) = env::var("CC_GITHUB_USER") {
        if !val.is_empty() {
            return Some(val);
        }
    }

    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let username = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if username.is_empty() {
        None
    } else {
        Some(username)
    }
}

fn get_today_date() -> String {
    let output = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    output.unwrap_or_else(|| "1970-01-01".to_string())
}
