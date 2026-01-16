use std::process::Command;

pub struct GitInfo {
    pub repository: String,
    pub branch: String,
    pub changes: String,
    pub ahead_behind: String,
}

pub fn snapshot() -> GitInfo {
    let status = git_status();
    let branch = status
        .as_ref()
        .and_then(|out| parse_branch(out))
        .unwrap_or_else(|| "-".to_string());

    let changes = status
        .as_ref()
        .and_then(|_| line_changes())
        .map(|(add, del)| format!("+{} -{}", add, del))
        .unwrap_or_else(|| "-".to_string());

    let repository = get_repository_name().unwrap_or_else(|| "-".to_string());

    let ahead_behind = status
        .as_ref()
        .and_then(|out| parse_ahead_behind(out))
        .unwrap_or_else(|| "-".to_string());

    GitInfo {
        repository,
        branch,
        changes,
        ahead_behind,
    }
}

fn git_status() -> Option<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-b"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn line_changes() -> Option<(u64, u64)> {
    let (add1, del1) = git_numstat(&["diff", "--numstat"]).unwrap_or((0, 0));
    let (add2, del2) = git_numstat(&["diff", "--numstat", "--cached"]).unwrap_or((0, 0));
    Some((add1 + add2, del1 + del2))
}

fn git_numstat(args: &[&str]) -> Option<(u64, u64)> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Some(parse_numstat_sum(&text))
}

fn parse_numstat_sum(text: &str) -> (u64, u64) {
    let mut add = 0u64;
    let mut del = 0u64;
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let a = parts.next();
        let d = parts.next();
        if let (Some(a), Some(d)) = (a, d) {
            if let Ok(v) = a.parse::<u64>() {
                add += v;
            }
            if let Ok(v) = d.parse::<u64>() {
                del += v;
            }
        }
    }
    (add, del)
}

fn parse_branch(status_output: &str) -> Option<String> {
    let first = status_output.lines().next()?;
    let branch_line = first.strip_prefix("## ")?;

    if branch_line.contains("(no branch)") || branch_line.starts_with("HEAD") {
        return Some("detached".to_string());
    }

    let name = branch_line
        .split_once("...")
        .map(|(left, _)| left)
        .unwrap_or(branch_line)
        .trim();

    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn get_repository_name() -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        // Try to get the directory name as fallback
        return get_toplevel_name();
    }

    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_repo_from_url(&url).or_else(get_toplevel_name)
}

fn get_toplevel_name() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    path.split('/').next_back().map(|s| s.to_string())
}

fn parse_repo_from_url(url: &str) -> Option<String> {
    // Handle SSH format: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@") {
        let path = rest.split(':').nth(1)?;
        let path = path.strip_suffix(".git").unwrap_or(path);
        return Some(path.to_string());
    }

    // Handle HTTPS format: https://github.com/owner/repo.git
    if url.starts_with("https://") || url.starts_with("http://") {
        let path = url.split('/').skip(3).collect::<Vec<_>>().join("/");
        let path = path.strip_suffix(".git").unwrap_or(&path);
        return Some(path.to_string());
    }

    None
}

fn parse_ahead_behind(status_output: &str) -> Option<String> {
    let first = status_output.lines().next()?;

    let mut ahead = 0;
    let mut behind = 0;

    if let Some(start) = first.find("[ahead ") {
        let rest = &first[start + 7..];
        if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
            ahead = rest[..end].parse().unwrap_or(0);
        }
    }

    if let Some(start) = first.find("behind ") {
        let rest = &first[start + 7..];
        if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
            behind = rest[..end].parse().unwrap_or(0);
        }
    }

    if ahead == 0 && behind == 0 {
        // Check if there's a tracking branch
        if first.contains("...") {
            Some("synced".to_string())
        } else {
            Some("-".to_string())
        }
    } else {
        Some(format!("↑{} ↓{}", ahead, behind))
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_ahead_behind, parse_branch, parse_numstat_sum, parse_repo_from_url};

    #[test]
    fn parse_branch_from_status() {
        let output = "## main...origin/main\n M src/main.rs\n";
        assert_eq!(parse_branch(output).as_deref(), Some("main"));
    }

    #[test]
    fn parse_branch_detached() {
        let output = "## HEAD (no branch)\n";
        assert_eq!(parse_branch(output).as_deref(), Some("detached"));
    }

    #[test]
    fn parse_numstat_totals() {
        let output = "10\t2\tsrc/main.rs\n3\t0\tREADME.md\n";
        assert_eq!(parse_numstat_sum(output), (13, 2));
    }

    #[test]
    fn parse_numstat_ignores_binary() {
        let output = "-\t-\timage.png\n1\t5\tfile.txt\n";
        assert_eq!(parse_numstat_sum(output), (1, 5));
    }

    #[test]
    fn parse_repo_ssh_format() {
        let url = "git@github.com:kokoichi206/cc-sakura-line.git";
        assert_eq!(
            parse_repo_from_url(url).as_deref(),
            Some("kokoichi206/cc-sakura-line")
        );
    }

    #[test]
    fn parse_repo_https_format() {
        let url = "https://github.com/kokoichi206/cc-sakura-line.git";
        assert_eq!(
            parse_repo_from_url(url).as_deref(),
            Some("kokoichi206/cc-sakura-line")
        );
    }

    #[test]
    fn parse_ahead_behind_both() {
        let output = "## main...origin/main [ahead 2, behind 1]\n";
        assert_eq!(parse_ahead_behind(output).as_deref(), Some("↑2 ↓1"));
    }

    #[test]
    fn parse_ahead_behind_synced() {
        let output = "## main...origin/main\n";
        assert_eq!(parse_ahead_behind(output).as_deref(), Some("synced"));
    }

    #[test]
    fn parse_ahead_behind_no_remote() {
        let output = "## feature-branch\n";
        assert_eq!(parse_ahead_behind(output).as_deref(), Some("-"));
    }
}
