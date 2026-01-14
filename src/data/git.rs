use std::process::Command;

pub struct GitInfo {
    pub branch: String,
    pub changes: String,
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

    GitInfo { branch, changes }
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

#[cfg(test)]
mod tests {
    use super::{parse_branch, parse_numstat_sum};

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
}
