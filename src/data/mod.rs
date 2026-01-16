mod claude;
mod clock;
mod context;
mod git;
mod github;
mod session;

use serde_json::Value;
use std::time::Instant;

pub use claude::read_stdin_json;

#[derive(Clone, Debug)]
pub struct Snapshot {
    // Row 1: Claude
    pub model: String,
    pub version: String,
    pub contributions: String,
    pub session_clock: String,
    // Row 2: Git
    pub repository: String,
    pub branch: String,
    pub git_changes: String,
    pub ahead_behind: String,
    // Row 3: Context
    pub context: String,
    pub context_remaining: String,
    pub now_clock: String,
}

pub fn collect_from_input(input: Option<&Value>) -> Snapshot {
    let git = git::snapshot();
    let context = context::from_input(input);

    Snapshot {
        // Row 1: Claude
        model: claude::model(input),
        version: claude::version(input).unwrap_or_else(|| "-".to_string()),
        contributions: github::today_contributions(),
        session_clock: session::from_input(input),
        // Row 2: Git
        repository: git.repository,
        branch: git.branch,
        git_changes: git.changes,
        ahead_behind: git.ahead_behind,
        // Row 3: Context
        context: context.context,
        context_remaining: context.remaining,
        now_clock: clock::now_hms(),
    }
}

pub fn collect_preview(started_at: Instant) -> Snapshot {
    let git = git::snapshot();
    let context = context::from_input(None);

    Snapshot {
        // Row 1: Claude
        model: claude::model(None),
        version: claude::version(None).unwrap_or_else(|| "-".to_string()),
        contributions: github::today_contributions(),
        session_clock: session::clock(started_at),
        // Row 2: Git
        repository: git.repository,
        branch: git.branch,
        git_changes: git.changes,
        ahead_behind: git.ahead_behind,
        // Row 3: Context
        context: context.context,
        context_remaining: context.remaining,
        now_clock: clock::now_hms(),
    }
}
