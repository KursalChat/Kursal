use std::time::{Duration, Instant};

pub const MAX_FAILURES: u32 = 5;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct IpRecord {
    failures: u32,
    window_start: Option<Instant>,
}

impl IpRecord {
    pub fn is_limited(&mut self, now: Instant) -> bool {
        match self.window_start {
            Some(start) if now.duration_since(start) < FAILURE_WINDOW => {
                self.failures >= MAX_FAILURES
            }
            _ => false,
        }
    }

    pub fn record_failure(&mut self, now: Instant) {
        match self.window_start {
            Some(start) if now.duration_since(start) < FAILURE_WINDOW => {
                self.failures += 1;
            }
            _ => {
                self.failures = 1;
                self.window_start = Some(now);
            }
        }
    }

    pub fn reset(&mut self) {
        self.failures = 0;
        self.window_start = None;
    }

    pub fn is_stale(&self, now: Instant) -> bool {
        match self.window_start {
            Some(start) => now.duration_since(start) >= FAILURE_WINDOW,
            None => true,
        }
    }
}
