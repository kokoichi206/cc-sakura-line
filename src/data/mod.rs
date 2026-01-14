mod claude;
mod clock;
mod context;
mod git;
mod session;

use serde_json::Value;
use std::time::Instant;

pub use claude::read_stdin_json;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub model: String,
    pub branch: String,
    pub git_changes: String,
    pub now_clock: String,
    pub version: String,
    pub context: String,
    pub context_remaining: String,
    pub session_clock: String,
}

pub fn collect_from_input(input: Option<&Value>) -> Snapshot {
    let git = git::snapshot();
    let context = context::from_input(input);

    Snapshot {
        model: claude::model(input),
        branch: git.branch,
        git_changes: git.changes,
        now_clock: clock::now_hms(),
        version: claude::version(input).unwrap_or_else(|| "-".to_string()),
        context: context.context,
        context_remaining: context.remaining,
        session_clock: session::from_input(input),
    }
}

pub fn collect_preview(started_at: Instant) -> Snapshot {
    let git = git::snapshot();
    let context = context::from_input(None);

    Snapshot {
        model: claude::model(None),
        branch: git.branch,
        git_changes: git.changes,
        now_clock: clock::now_hms(),
        version: claude::version(None).unwrap_or_else(|| "-".to_string()),
        context: context.context,
        context_remaining: context.remaining,
        session_clock: session::clock(started_at),
    }
}
