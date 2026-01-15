use std::time::{Duration, Instant};

use crate::data::{self, Snapshot};

pub struct App {
    pub snapshot: Snapshot,
    started_at: Instant,
    last_tick: Instant,
}

impl App {
    pub fn new_preview() -> Self {
        let started_at = Instant::now();
        let snapshot = data::collect_preview(started_at);
        let last_tick = Instant::now();
        Self {
            snapshot,
            started_at,
            last_tick,
        }
    }

    pub fn tick(&mut self) {
        self.snapshot = data::collect_preview(self.started_at);
        self.last_tick = Instant::now();
    }

    pub fn last_tick_elapsed(&self) -> Duration {
        self.last_tick.elapsed()
    }
}
