use std::time::Instant;

/// Moving average tracker for transfer speed (bytes per second) and estimated time to completion (ETA).
#[derive(Debug, Clone)]
pub struct ThroughputTracker {
    last_instant: Option<Instant>,
    last_bytes: u64,
    smoothed_speed: f64,
    alpha: f64,
}

impl Default for ThroughputTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroughputTracker {
    /// Create a new tracker with default exponential smoothing factor (alpha = 0.3).
    pub fn new() -> Self {
        Self::with_alpha(0.3)
    }

    /// Create a tracker with a customized smoothing factor between 0.0 and 1.0.
    pub fn with_alpha(alpha: f64) -> Self {
        Self {
            last_instant: None,
            last_bytes: 0,
            smoothed_speed: 0.0,
            alpha: alpha.clamp(0.01, 1.0),
        }
    }

    /// Resets the current time sample interval without clearing the smoothed speed.
    /// Crucial after a pause/resume so elapsed pause time does not dilute throughput metrics.
    pub fn reset_sample(&mut self) {
        self.last_instant = None;
    }

    /// Resets all metrics back to initial state.
    pub fn reset_all(&mut self) {
        self.last_instant = None;
        self.last_bytes = 0;
        self.smoothed_speed = 0.0;
    }

    /// Update with the latest byte count and time instant.
    /// Returns `(speed_bps, eta_seconds)`.
    pub fn update(&mut self, current_bytes: u64, total_bytes: u64, now: Instant) -> (u64, Option<u64>) {
        if let Some(prev_instant) = self.last_instant {
            let dt = now.saturating_duration_since(prev_instant).as_secs_f64();
            if dt > 0.0 {
                let delta_bytes = current_bytes.saturating_sub(self.last_bytes);
                let instant_speed = (delta_bytes as f64) / dt;

                if self.smoothed_speed == 0.0 {
                    self.smoothed_speed = instant_speed;
                } else {
                    self.smoothed_speed = self.alpha * instant_speed + (1.0 - self.alpha) * self.smoothed_speed;
                }

                self.last_instant = Some(now);
                self.last_bytes = current_bytes;
            }
        } else {
            self.last_instant = Some(now);
            self.last_bytes = current_bytes;
        }

        let speed_bps = self.smoothed_speed.round() as u64;
        let eta_seconds = self.calculate_eta(current_bytes, total_bytes);

        (speed_bps, eta_seconds)
    }

    fn calculate_eta(&self, current_bytes: u64, total_bytes: u64) -> Option<u64> {
        if current_bytes >= total_bytes {
            Some(0)
        } else if self.smoothed_speed > 0.0 {
            let remaining = total_bytes.saturating_sub(current_bytes) as f64;
            let seconds = (remaining / self.smoothed_speed).ceil();
            Some(seconds as u64)
        } else {
            None
        }
    }

    /// Returns the currently calculated smoothed speed in bytes/sec.
    pub fn current_speed(&self) -> u64 {
        self.smoothed_speed.round() as u64
    }
}
