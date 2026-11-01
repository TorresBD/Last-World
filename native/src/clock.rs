//! CPU-owned simulation time. Rendering may interpolate the current tick, but
//! must never invent another gameplay clock from accumulated floating seconds.
//!
//! `advance` commits CPU ticks. Its returned batch must be executed exactly once
//! by the caller; it is not evidence that submitted GPU work has completed. The
//! current movement fixture will use this clock before campaign systems exist.
use std::{fmt, time::Duration};

pub const TACTICAL_HZ: u32 = 20;
pub const TICK_NANOS: u64 = 1_000_000_000 / TACTICAL_HZ as u64;
/// One hour of simulation time: 60 minutes at 1x, 30 at 2x, 15 at 4x,
/// provided the machine sustains the requested speed.
pub const SEASON_CYCLE_TICKS: u64 = 3_600 * TACTICAL_HZ as u64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SimulationSpeed {
    #[default]
    One,
    Two,
    Four,
}

impl SimulationSpeed {
    pub const fn multiplier(self) -> u32 {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
        }
    }
}

impl TryFrom<u32> for SimulationSpeed {
    type Error = ClockError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::One),
            2 => Ok(Self::Two),
            4 => Ok(Self::Four),
            other => Err(ClockError::UnsupportedSpeed(other)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    UnsupportedSpeed(u32),
    UnsupportedBenchmarkRate(u32),
    ElapsedTimeOverflow,
    BacklogOverflow,
    TickOverflow,
}

impl fmt::Display for ClockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSpeed(value) => {
                write!(f, "Unsupported simulation speed {value}; use 1, 2 or 4")
            }
            Self::UnsupportedBenchmarkRate(value) => {
                write!(f, "Benchmark rate {value} is outside 1..=100")
            }
            Self::ElapsedTimeOverflow => write!(f, "Simulation elapsed-time arithmetic overflow"),
            Self::BacklogOverflow => write!(f, "Simulation pending-tick backlog overflow"),
            Self::TickOverflow => write!(f, "Authoritative simulation tick overflow"),
        }
    }
}

impl std::error::Error for ClockError {}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GameClock {
    tick: u64,
    pending_ticks: u64,
    remainder_nanos: u64,
    speed: SimulationSpeed,
}

impl GameClock {
    pub const fn tick(&self) -> u64 {
        self.tick
    }

    /// Whole requested ticks not executed yet. Debt is retained under overload;
    /// do not silently clear it to claim the requested speed was achieved.
    pub const fn pending_ticks(&self) -> u64 {
        self.pending_ticks
    }

    pub const fn remainder_nanos(&self) -> u64 {
        self.remainder_nanos
    }

    pub const fn speed(&self) -> SimulationSpeed {
        self.speed
    }

    /// Changes only the rate of future elapsed time; existing debt and fractional
    /// time have already been charged at the previous speed and remain unchanged.
    pub fn set_speed(&mut self, speed: SimulationSpeed) {
        self.speed = speed;
    }

    /// Derived presentation value. Rule deadlines should compare integer ticks.
    pub fn seconds(&self) -> f64 {
        self.tick as f64 / TACTICAL_HZ as f64
    }

    pub const fn season_cycle_index(&self) -> u64 {
        self.tick / SEASON_CYCLE_TICKS
    }

    pub const fn season_tick(&self) -> u64 {
        self.tick % SEASON_CYCLE_TICKS
    }

    /// Rendering/continuous forcing convenience; deadlines use season_tick.
    pub fn season_phase(&self) -> f64 {
        self.season_tick() as f64 / SEASON_CYCLE_TICKS as f64
    }

    /// Add requested wall time and execute at most `max_steps` fixed ticks.
    /// Pausing accrues no new time and does not drain pre-existing debt. A work
    /// budget of zero accrues debt but performs no simulation. Arithmetic errors
    /// leave the entire clock unchanged.
    pub fn advance(
        &mut self,
        elapsed: Duration,
        paused: bool,
        max_steps: u32,
    ) -> Result<u32, ClockError> {
        self.advance_at_rate(elapsed, paused, max_steps, self.speed.multiplier())
    }

    /// Stress harness only. This uses the same tick/debt/remainder authority;
    /// it never makes another simulation clock or unlocks playable speeds.
    pub fn advance_benchmark(
        &mut self,
        elapsed: Duration,
        paused: bool,
        max_steps: u32,
        requested_rate: u32,
    ) -> Result<u32, ClockError> {
        if !(1..=100).contains(&requested_rate) {
            return Err(ClockError::UnsupportedBenchmarkRate(requested_rate));
        }
        self.advance_at_rate(elapsed, paused, max_steps, requested_rate)
    }

    fn advance_at_rate(
        &mut self,
        elapsed: Duration,
        paused: bool,
        max_steps: u32,
        requested_rate: u32,
    ) -> Result<u32, ClockError> {
        if paused {
            return Ok(0);
        }
        let requested_nanos = elapsed
            .as_nanos()
            .checked_mul(requested_rate as u128)
            .and_then(|nanos| nanos.checked_add(self.remainder_nanos as u128))
            .ok_or(ClockError::ElapsedTimeOverflow)?;
        let new_ticks = u64::try_from(requested_nanos / TICK_NANOS as u128)
            .map_err(|_| ClockError::BacklogOverflow)?;
        let pending_ticks = self
            .pending_ticks
            .checked_add(new_ticks)
            .ok_or(ClockError::BacklogOverflow)?;
        let steps = pending_ticks.min(max_steps as u64) as u32;
        let tick = self
            .tick
            .checked_add(steps as u64)
            .ok_or(ClockError::TickOverflow)?;

        self.tick = tick;
        self.pending_ticks = pending_ticks - steps as u64;
        self.remainder_nanos = (requested_nanos % TICK_NANOS as u128) as u64;
        Ok(steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partitioning_wall_time_does_not_change_ticks_at_any_allowed_speed() {
        for speed in [
            SimulationSpeed::One,
            SimulationSpeed::Two,
            SimulationSpeed::Four,
        ] {
            let mut whole = GameClock::default();
            whole.set_speed(speed);
            let mut partitioned = whole.clone();
            let parts = [1, 9_999_999, 39_000_001, 999_999_999, 2_345_678_901];
            let total_nanos: u64 = parts.iter().sum();
            whole
                .advance(Duration::from_nanos(total_nanos), false, u32::MAX)
                .unwrap();
            for nanos in parts {
                partitioned
                    .advance(Duration::from_nanos(nanos), false, u32::MAX)
                    .unwrap();
            }
            assert_eq!(whole, partitioned);
            assert_eq!(
                whole.tick(),
                total_nanos * speed.multiplier() as u64 / TICK_NANOS
            );
        }
    }

    #[test]
    fn bounded_work_retains_all_debt_and_pause_neither_accrues_nor_drains_it() {
        let mut clock = GameClock::default();
        assert_eq!(clock.advance(Duration::from_secs(1), false, 3), Ok(3));
        assert_eq!(clock.pending_ticks(), 17);
        let before_pause = clock.clone();
        assert_eq!(clock.advance(Duration::from_secs(60), true, 100), Ok(0));
        assert_eq!(clock, before_pause);
        assert_eq!(clock.advance(Duration::ZERO, false, 16), Ok(16));
        assert_eq!(clock.pending_ticks(), 1);
        assert_eq!(clock.advance(Duration::ZERO, false, 16), Ok(1));
        assert_eq!(clock.tick(), 20);
        assert_eq!(clock.pending_ticks(), 0);
    }

    #[test]
    fn speed_changes_do_not_reprice_previous_debt_or_fractional_time() {
        let mut clock = GameClock::default();
        assert_eq!(clock.advance(Duration::from_millis(125), false, 0), Ok(0));
        assert_eq!(clock.pending_ticks(), 2);
        assert_eq!(clock.remainder_nanos(), 25_000_000);
        clock.set_speed(SimulationSpeed::Four);
        assert_eq!(clock.advance(Duration::from_millis(25), false, 100), Ok(4));
        assert_eq!(clock.tick(), 4);
        assert_eq!(clock.remainder_nanos(), 25_000_000);
    }

    #[test]
    fn season_cycle_deadline_is_exact_and_scales_with_selected_speed() {
        let mut clock = GameClock::default();
        clock
            .advance(Duration::from_millis(3_599_950), false, u32::MAX)
            .unwrap();
        assert_eq!(clock.tick(), SEASON_CYCLE_TICKS - 1);
        assert_eq!(clock.season_cycle_index(), 0);
        assert!(clock.season_phase() < 1.);
        clock
            .advance(Duration::from_millis(50), false, u32::MAX)
            .unwrap();
        assert_eq!(clock.season_cycle_index(), 1);
        assert_eq!(clock.season_tick(), 0);
        assert_eq!(clock.seconds(), 3_600.);
        for (speed, wall_seconds) in [(SimulationSpeed::Two, 1_800), (SimulationSpeed::Four, 900)] {
            let mut accelerated = GameClock::default();
            accelerated.set_speed(speed);
            accelerated
                .advance(Duration::from_secs(wall_seconds), false, u32::MAX)
                .unwrap();
            assert_eq!(accelerated.tick(), SEASON_CYCLE_TICKS);
        }
    }

    #[test]
    fn overflow_is_explicit_and_transactional() {
        let mut exhausted = GameClock {
            tick: u64::MAX,
            ..GameClock::default()
        };
        let before = exhausted.clone();
        assert_eq!(
            exhausted.advance(Duration::from_millis(50), false, 1),
            Err(ClockError::TickOverflow)
        );
        assert_eq!(exhausted, before);

        let mut saturated = GameClock {
            pending_ticks: u64::MAX,
            ..GameClock::default()
        };
        let before = saturated.clone();
        assert_eq!(
            saturated.advance(Duration::from_millis(50), false, 1),
            Err(ClockError::BacklogOverflow)
        );
        assert_eq!(saturated, before);

        let mut huge_duration = GameClock::default();
        let before = huge_duration.clone();
        assert_eq!(
            huge_duration.advance(Duration::from_secs(u64::MAX), false, 1),
            Err(ClockError::BacklogOverflow)
        );
        assert_eq!(huge_duration, before);
    }

    #[test]
    fn unsupported_speeds_are_rejected_without_substitution() {
        for value in [0, 3, 8, 20, 32, u32::MAX] {
            assert_eq!(
                SimulationSpeed::try_from(value),
                Err(ClockError::UnsupportedSpeed(value))
            );
        }
        assert_eq!(SimulationSpeed::try_from(1), Ok(SimulationSpeed::One));
        assert_eq!(SimulationSpeed::try_from(2), Ok(SimulationSpeed::Two));
        assert_eq!(SimulationSpeed::try_from(4), Ok(SimulationSpeed::Four));
    }

    #[test]
    fn benchmark_hundred_rate_keeps_exact_debt_and_does_not_unlock_playable_speed() {
        let mut clock = GameClock::default();
        clock.set_speed(SimulationSpeed::Four);
        assert_eq!(
            clock.advance_benchmark(Duration::from_millis(50), false, 16, 100),
            Ok(16)
        );
        assert_eq!(clock.pending_ticks(), 84);
        assert_eq!(clock.speed(), SimulationSpeed::Four);
        assert_eq!(
            clock.advance_benchmark(Duration::ZERO, false, 100, 1),
            Ok(84)
        );
        assert_eq!(clock.tick(), 100);
        assert_eq!(clock.pending_ticks(), 0);
        assert_eq!(clock.advance(Duration::from_millis(50), false, 100), Ok(4));
        assert_eq!(clock.tick(), 104);
        assert!(SimulationSpeed::try_from(100).is_err());
        let before = clock.clone();
        assert_eq!(
            clock.advance_benchmark(Duration::from_secs(1), false, 16, 101),
            Err(ClockError::UnsupportedBenchmarkRate(101))
        );
        assert_eq!(clock, before);
    }

    #[test]
    fn benchmark_rate_changes_preserve_previously_charged_time_and_fraction() {
        let mut whole = GameClock::default();
        let mut partitioned = whole.clone();
        for rate in [1, 2, 4, 8, 16, 32, 64, 100] {
            whole
                .advance_benchmark(Duration::from_millis(51), false, 0, rate)
                .unwrap();
            for millis in [1, 7, 13, 30] {
                partitioned
                    .advance_benchmark(Duration::from_millis(millis), false, 0, rate)
                    .unwrap();
            }
        }
        assert_eq!(whole, partitioned);
        let paused = whole.clone();
        assert_eq!(
            whole.advance_benchmark(Duration::from_secs(1), true, 100, 100),
            Ok(0)
        );
        assert_eq!(whole, paused);
    }
}
