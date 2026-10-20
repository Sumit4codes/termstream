use std::time::{Duration, Instant};

pub struct Clock {
    base_pts: f64,
    start_instant: Option<Instant>,
    paused_instant: Option<Instant>,
    paused_duration: Duration,
    is_paused: bool,
}

impl Clock {
    pub fn new(initial_pts: f64) -> Self {
        Self {
            base_pts: initial_pts,
            start_instant: None,
            paused_instant: None,
            paused_duration: Duration::ZERO,
            is_paused: false,
        }
    }

    pub fn start(&mut self) {
        self.start_instant = Some(Instant::now());
        self.paused_duration = Duration::ZERO;
        self.paused_instant = None;
        self.is_paused = false;
    }

    pub fn get_pts(&self) -> f64 {
        if let Some(start) = self.start_instant {
            let elapsed = if self.is_paused {
                if let Some(p) = self.paused_instant {
                    p.duration_since(start).saturating_sub(self.paused_duration)
                } else {
                    Duration::ZERO
                }
            } else {
                start.elapsed().saturating_sub(self.paused_duration)
            };
            self.base_pts + elapsed.as_secs_f64()
        } else {
            self.base_pts
        }
    }

    pub fn pause(&mut self) {
        if !self.is_paused {
            self.is_paused = true;
            self.paused_instant = Some(Instant::now());
        }
    }

    pub fn resume(&mut self) {
        if self.is_paused {
            if let Some(p) = self.paused_instant.take() {
                self.paused_duration += p.elapsed();
            }
            self.is_paused = false;
        }
    }

    pub fn toggle_pause(&mut self) -> bool {
        if self.is_paused {
            self.resume();
            false
        } else {
            self.pause();
            true
        }
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn seek_to(&mut self, target_pts: f64) {
        self.base_pts = target_pts.max(0.0);
        self.start_instant = Some(Instant::now());
        self.paused_duration = Duration::ZERO;
        if self.is_paused {
            self.paused_instant = Some(Instant::now());
        } else {
            self.paused_instant = None;
        }
    }
}
