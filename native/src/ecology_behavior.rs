//! Small deterministic baseline behavior policies, evaluated per cohort/intent
//! rather than per rendered frame. These functions return work durations and
//! preference scores only. They never own, clamp, split, spawn or move animals.
//!
//! Threat codes follow the first species profiles:0=cautious,1=avoid/flee,
//! 2=defensive. Unknown codes use cautious behavior. Scores are deliberately
//! bounded utilities, not population bounds. Ordinary hunger, thirst, real
//! habitat connectivity, finite reserves and transfer costs remain authoritative
//! in Ecology; a favorable score cannot create food or make a route traversable.

/// Real handler labor at20Hz. Docility reduces restraint/acclimation work;
/// larger bodies and a defensive response increase it. Even a docile tiny
/// animal requires actual work. Mass bands use integer logarithms so extreme
/// but valid body sizes cannot create an unbounded scheduling calculation.
pub fn handling_ticks(docile: bool, body_mass_g: u32, threat_response: u8) -> u64 {
    let kilograms = (body_mass_g as u64).div_ceil(1000).max(1);
    let size_work = kilograms.ilog2() as u64 * 10;
    let temperament_work = match threat_response {
        1 => 10,
        2 => 20,
        _ => 0,
    };
    if docile {
        40 + size_work + temperament_work
    } else {
        80 + size_work + temperament_work
    }
}

/// Soft impetus to look for another suitable patch. This consumes actual
/// regional conspecific counts, not a claim that every cohort is one herd.
/// Several preferred groups can coexist in one macro patch without automatic
/// expulsion. Lack of companions, severe crowding and observed threats can
/// motivate a search; the caller still chooses whether a usable destination and
/// affordable journey exist. Zero organisms produce zero movement pressure.
pub fn movement_pressure(
    local_count: u32,
    social_group_size: u16,
    territorial_radius_m: u16,
    patch_area_m2: u32,
    nearby_threats: u32,
    threat_response: u8,
) -> u16 {
    if local_count == 0 {
        return 0;
    }
    if patch_area_m2 == 0 {
        return 1000;
    }
    let preferred = (social_group_size as u128).max(1);
    let count = local_count as u128;
    let isolated = if preferred > 1 && count < preferred {
        (preferred - count) * 120 / preferred
    } else {
        0
    };
    // Four social groups is a soft crowding reference at this macro scale,
    // not a habitat capacity, reproduction limit or hidden population ceiling.
    let excess = count.saturating_sub(preferred * 4);
    let territory = territory_weight(territorial_radius_m, patch_area_m2);
    let crowding = (excess * territory / (preferred * 16)).min(200);
    let threat = nearby_threats as u128
        * match threat_response {
            1 => 240,
            2 => 60,
            _ => 120,
        };
    (isolated + crowding + threat).min(1000) as u16
}

/// Rank already reachable, resource-eligible destinations. Pass projected
/// regional occupancy INCLUDING the arriving cohort; this prevents several
/// cohorts from independently treating an occupied territory as vacant.
/// Social species favor a useful group size, territorial species avoid excess
/// competition, and observed threats reduce attraction. No score changes an
/// organism count; buffered transit commit remains the only movement owner.
pub fn destination_bias(
    destination_conspecifics: u32,
    social_group_size: u16,
    territorial_radius_m: u16,
    patch_area_m2: u32,
    nearby_threats: u32,
    threat_response: u8,
) -> i32 {
    if patch_area_m2 == 0 {
        return -1000;
    }
    let preferred = (social_group_size as u128).max(1);
    let count = destination_conspecifics as u128;
    let social = if preferred > 1 && count > 0 {
        let error = count.abs_diff(preferred).min(preferred);
        (preferred - error) * 180 / preferred
    } else {
        0
    };
    let groups = count.div_ceil(preferred);
    let crowding =
        (groups.saturating_sub(1) * territory_weight(territorial_radius_m, patch_area_m2) / 4)
            .min(500);
    let threat = (nearby_threats as u128
        * match threat_response {
            1 => 220,
            2 => 40,
            _ => 100,
        })
    .min(1000);
    (social as i32 - crowding as i32 - threat as i32).max(-1000)
}

/// Territory footprint relative to the real patch area, using355/113 for pi.
/// Only the preference weight is saturated; all organism/area inputs remain
/// untouched and there is no territorial occupancy reservation in this helper.
fn territory_weight(radius_m: u16, area_m2: u32) -> u128 {
    if radius_m == 0 || area_m2 == 0 {
        return 0;
    }
    let radius = radius_m as u128;
    (radius * radius * 355 * 1000 / (113 * area_m2 as u128)).min(4000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docile_handling_pays_real_time_but_less_than_identical_wild_animal() {
        for mass in [0, 100, 1500, 30_000, 5_000_000, u32::MAX] {
            let docile = handling_ticks(true, mass, 1);
            let wild = handling_ticks(false, mass, 1);
            assert!(docile >= 40);
            assert_eq!(wild - docile, 40);
        }
        assert!(handling_ticks(false, 100_000, 2) > handling_ticks(false, 1000, 2));
        assert!(handling_ticks(false, 30_000, 2) > handling_ticks(false, 30_000, 0));
    }

    #[test]
    fn regional_founders_are_not_treated_as_one_oversized_herd() {
        assert_eq!(movement_pressure(24, 12, 80, 25_600, 0, 1), 0);
        assert_eq!(movement_pressure(2, 2, 300, 25_600, 0, 2), 0);
        assert!(movement_pressure(96, 12, 80, 25_600, 0, 1) > 0);
        assert!(movement_pressure(1, 12, 80, 25_600, 0, 1) > 0);
    }

    #[test]
    fn observed_threats_change_response_without_making_defensive_animals_immune() {
        let flee = movement_pressure(12, 12, 80, 25_600, 2, 1);
        let defend = movement_pressure(12, 12, 80, 25_600, 2, 2);
        assert!(flee > defend);
        assert!(defend > 0);
        assert!(
            destination_bias(12, 12, 80, 25_600, 2, 1) < destination_bias(12, 12, 80, 25_600, 0, 1)
        );
        assert!(
            destination_bias(12, 12, 80, 25_600, 2, 1) < destination_bias(12, 12, 80, 25_600, 2, 2)
        );
    }

    #[test]
    fn projected_group_size_and_territory_change_destination_ranking() {
        let useful = destination_bias(12, 12, 80, 25_600, 0, 1);
        assert!(useful > destination_bias(1, 12, 80, 25_600, 0, 1));
        assert!(useful > destination_bias(48, 12, 80, 25_600, 0, 1));
        assert!(
            destination_bias(24, 12, 300, 25_600, 0, 1)
                < destination_bias(24, 12, 80, 25_600, 0, 1)
        );
        assert!(
            destination_bias(48, 12, 300, 25_600, 0, 1)
                < destination_bias(48, 12, 300, 1_000_000, 0, 1)
        );
        assert!(
            destination_bias(2, 1, 300, 25_600, 0, 2) < destination_bias(1, 1, 300, 25_600, 0, 2)
        );
    }

    #[test]
    fn extreme_inputs_bound_utilities_and_never_require_population_floors() {
        for count in [0, 1, 24, u32::MAX] {
            for group in [0, 1, 12, u16::MAX] {
                for area in [0, 1, 25_600, u32::MAX] {
                    for threats in [0, 1, u32::MAX] {
                        let pressure =
                            movement_pressure(count, group, u16::MAX, area, threats, 255);
                        let bias = destination_bias(count, group, u16::MAX, area, threats, 255);
                        assert!(pressure <= 1000);
                        assert!((-1000..=180).contains(&bias));
                        if count == 0 {
                            assert_eq!(pressure, 0);
                        }
                    }
                }
            }
        }
        assert_eq!(destination_bias(1, 12, 80, 0, 0, 1), -1000);
    }
}
