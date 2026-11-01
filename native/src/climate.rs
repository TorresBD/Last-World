//! Deterministic jungle climate conditions from the single authoritative clock.
//! These are forcing values, not water/heat/material stock owners. Callers must
//! apply rain as a named input and debit real stores for uptake/evaporation.
use serde::{Deserialize, Serialize};
pub const SEASON_CYCLE_TICKS: u64 = 72_000;
pub const CLIMATE_REFERENCE_PATCH_AREA_M2: u32 = 256;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JungleSeason {
    Dry,
    RainsArriving,
    Wet,
    RainsRetreating,
}
impl JungleSeason {
    pub fn name(self) -> &'static str {
        match self {
            Self::Dry => "dry season",
            Self::RainsArriving => "rains arriving",
            Self::Wet => "wet season",
            Self::RainsRetreating => "rains retreating",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeasonState {
    pub cycle_index: u64,
    pub cycle_tick: u64,
    pub season: JungleSeason,
    pub temperature_milli_c: i32,
    /// For the256m² reference patch; scale using the actual area and retain a
    /// fractional remainder at the input boundary rather than rounding perframe.
    pub rainfall_liters_per_patch_second: u32,
    pub moisture_forcing_permille: u16,
    pub evaporation_permille: u16,
    pub infiltration_permille: u16,
    pub soil_saturated: bool,
    pub soil_drought_stressed: bool,
}
impl SeasonState {
    pub fn at_tick(tick: u64, seed: u32, patch: u32) -> Self {
        let cycle_tick = tick % SEASON_CYCLE_TICKS;
        // The shared season remains global; local weather/anatomy conditions
        // have deterministic bounded differences rather than every tile changing
        // to the same rainfall state at a quarter-cycle boundary.
        let h = (seed as u64)
            .wrapping_mul(6364136223846793005)
            .wrapping_add((patch as u64).wrapping_mul(1442695040888963407));
        let local_offset = (h >> 32) % 1200;
        let local_phase = (cycle_tick + local_offset) % SEASON_CYCLE_TICKS;
        let wet = triangle(local_phase, SEASON_CYCLE_TICKS);
        let season = match cycle_tick {
            0..=8999 | 63000..=71999 => JungleSeason::Dry,
            9000..=26999 => JungleSeason::RainsArriving,
            27000..=44999 => JungleSeason::Wet,
            _ => JungleSeason::RainsRetreating,
        };
        let rain = if wet < 350 {
            0
        } else if wet < 650 {
            1
        } else if wet < 850 {
            2
        } else {
            4
        };
        let local_temperature = ((h >> 48) % 2001) as i32 - 1000;
        Self {
            cycle_index: tick / SEASON_CYCLE_TICKS,
            cycle_tick,
            season,
            temperature_milli_c: 31_500 - wet as i32 * 7 + local_temperature,
            rainfall_liters_per_patch_second: rain,
            moisture_forcing_permille: wet,
            evaporation_permille: (1500 - wet as u32) as u16,
            infiltration_permille: (800 + wet as u32 / 3) as u16,
            soil_saturated: wet >= 850,
            soil_drought_stressed: wet < 200,
        }
    }
    pub fn season_name(self) -> &'static str {
        self.season.name()
    }
}
fn triangle(tick: u64, period: u64) -> u16 {
    let half = period / 2;
    let rising = if tick <= half { tick } else { period - tick };
    (rising * 1000 / half) as u16
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_year_is_integer_clock_deterministic_and_repeats_conditions() {
        for t in [0, 1, 8999, 9000, 18000, 27000, 36000, 54000, 71999] {
            let a = SeasonState::at_tick(t, 4321, 7);
            let b = SeasonState::at_tick(t + SEASON_CYCLE_TICKS, 4321, 7);
            assert_eq!(a.cycle_tick, b.cycle_tick);
            assert_eq!(b.cycle_index, a.cycle_index + 1);
            assert_eq!(a.temperature_milli_c, b.temperature_milli_c);
            assert_eq!(
                a.rainfall_liters_per_patch_second,
                b.rainfall_liters_per_patch_second
            );
            assert_eq!(a, SeasonState::at_tick(t, 4321, 7));
        }
    }
    #[test]
    fn living_jungle_has_real_wet_dry_forcing_without_universal_snow() {
        let dry = SeasonState::at_tick(0, 12, 0);
        let wet = SeasonState::at_tick(36000, 12, 0);
        assert!(wet.rainfall_liters_per_patch_second > dry.rainfall_liters_per_patch_second);
        assert!(dry.evaporation_permille > wet.evaporation_permille);
        assert!(wet.moisture_forcing_permille > dry.moisture_forcing_permille);
        for t in 0..SEASON_CYCLE_TICKS {
            let s = SeasonState::at_tick(t, 12, 0);
            assert!(s.temperature_milli_c > 20_000 && s.temperature_milli_c < 34_000);
            assert!(s.evaporation_permille <= 1500);
        }
        assert_eq!(dry.season_name(), "dry season");
        assert_eq!(wet.season_name(), "wet season");
    }
    #[test]
    fn save_value_roundtrip_and_max_clock_do_not_overflow() {
        let s = SeasonState::at_tick(u64::MAX, u32::MAX, u32::MAX);
        let restored: SeasonState =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(s, restored);
        assert_ne!(
            SeasonState::at_tick(18000, 23, 1),
            SeasonState::at_tick(18000, 23, 2)
        );
    }
}
