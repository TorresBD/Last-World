//! Bounded frame-duration sampling for diagnostics, independent of simulation.
//!
//! Quantiles use the nearest-rank definition: rank = ceil(p * n), starting at 1.
//! When the recorder fills, quantiles describe the most recent retained samples.
//! `max_ms` and `mean_ms` still describe every accepted sample since `clear`.
//! Recording never grows the allocation beyond the requested capacity; summary
//! sorts a bounded copy only when the caller requests a report.

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct FrameSummary {
    pub retained_samples: usize,
    pub total_samples: u64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
}

#[derive(Debug)]
pub struct FrameTimings {
    samples_ms: Vec<f64>,
    capacity: usize,
    next: usize,
    total_samples: u64,
    max_ms: f64,
    mean_ms: f64,
}

impl FrameTimings {
    /// A reporting window must retain at least one sample.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "FrameTimings capacity must be positive");
        Self {
            samples_ms: Vec::with_capacity(capacity),
            capacity,
            next: 0,
            total_samples: 0,
            max_ms: 0.,
            mean_ms: 0.,
        }
    }

    /// Ignore negative/nonfinite durations, including conversion overflow.
    /// Zero is retained because timer resolution can legitimately yield zero.
    pub fn record_seconds(&mut self, seconds: f64) {
        let ms = seconds * 1000.;
        if !ms.is_finite() || ms < 0. {
            return;
        }
        self.total_samples = self
            .total_samples
            .checked_add(1)
            .expect("FrameTimings sample counter exhausted");
        self.max_ms = self.max_ms.max(ms);
        // Online mean avoids an unbounded duration sum overflowing.
        self.mean_ms += (ms - self.mean_ms) / self.total_samples as f64;
        if self.samples_ms.len() < self.capacity {
            self.samples_ms.push(ms);
        } else {
            self.samples_ms[self.next] = ms;
            self.next = (self.next + 1) % self.capacity;
        }
    }

    pub fn summary(&self) -> Option<FrameSummary> {
        if self.samples_ms.is_empty() {
            return None;
        }
        let mut sorted = self.samples_ms.clone();
        sorted.sort_unstable_by(f64::total_cmp);
        let percentile = |percent: usize| {
            // Equivalent to ceil(n * percent / 100), without n * percent overflow.
            let rank =
                (sorted.len() / 100) * percent + ((sorted.len() % 100) * percent).div_ceil(100);
            sorted[rank - 1]
        };
        Some(FrameSummary {
            retained_samples: sorted.len(),
            total_samples: self.total_samples,
            median_ms: percentile(50),
            p95_ms: percentile(95),
            p99_ms: percentile(99),
            max_ms: self.max_ms,
            mean_ms: self.mean_ms,
        })
    }

    /// Start another reporting window while retaining the existing allocation.
    pub fn clear(&mut self) {
        self.samples_ms.clear();
        self.next = 0;
        self.total_samples = 0;
        self.max_ms = 0.;
        self.mean_ms = 0.;
    }
}

#[cfg(test)]
mod tests {
    use super::FrameTimings;

    #[test]
    fn nearest_rank_quantiles_preserve_a_single_frame_spike() {
        let mut timings = FrameTimings::new(100);
        for ms in 1..=99 {
            timings.record_seconds(ms as f64 / 1000.);
        }
        timings.record_seconds(2.);
        let result = timings.summary().unwrap();
        assert_eq!(result.total_samples, 100);
        assert_eq!(result.retained_samples, 100);
        assert!((result.median_ms - 50.).abs() < 1e-10);
        assert!((result.p95_ms - 95.).abs() < 1e-10);
        assert!((result.p99_ms - 99.).abs() < 1e-10);
        assert_eq!(result.max_ms, 2000.);
        assert!((result.mean_ms - 69.5).abs() < 1e-10);
    }

    #[test]
    fn invalid_durations_do_not_poison_the_report() {
        let mut timings = FrameTimings::new(8);
        assert!(timings.summary().is_none());
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., f64::MAX] {
            timings.record_seconds(value);
        }
        assert!(timings.summary().is_none());
        timings.record_seconds(0.);
        timings.record_seconds(0.016);
        let result = timings.summary().unwrap();
        assert_eq!(result.total_samples, 2);
        assert_eq!(result.median_ms, 0.);
        assert_eq!(result.p99_ms, 16.);
        assert_eq!(result.mean_ms, 8.);
    }

    #[test]
    fn bounded_ring_reports_recent_quantiles_and_window_max_then_clears() {
        let mut timings = FrameTimings::new(3);
        for seconds in [5., 0.01, 0.02, 0.03, 0.04] {
            timings.record_seconds(seconds);
        }
        let result = timings.summary().unwrap();
        assert_eq!(result.total_samples, 5);
        assert_eq!(result.retained_samples, 3);
        assert_eq!(result.median_ms, 30.);
        assert_eq!(result.p95_ms, 40.);
        assert_eq!(result.max_ms, 5000.);
        assert!((result.mean_ms - 1020.).abs() < 1e-10);
        let capacity = timings.samples_ms.capacity();
        timings.clear();
        assert!(timings.summary().is_none());
        assert_eq!(timings.samples_ms.capacity(), capacity);
        timings.record_seconds(0.007);
        let result = timings.summary().unwrap();
        assert_eq!(result.total_samples, 1);
        assert_eq!(result.max_ms, 7.);
    }
}
