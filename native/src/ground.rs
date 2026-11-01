//! Local, CPU-owned soil/biomass/water coupling. Geometry is quantized to whole
//! millimeters; transported stocks are exact integer liters/material units.
//! This is an RTS erosion proxy, not calibrated sediment mechanics or CFD.
use crate::{
    clock::TACTICAL_HZ,
    water::{WaterError, WaterGraph},
};
use serde::{Deserialize, Serialize};
use std::fmt;

// Quantized surface-film policy: submillimeter residual liters remain real
// water but do not represent complete submergence across an8m sample. A tiny
// film also cannot scour soil or uproot plants from a large velocity ratio.
const AIR_EXPOSURE_DEPTH_MM: u64 = 1;
const MIN_EROSION_DEPTH_MM: u64 = 10;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GroundConfig {
    pub erodible_layer_mm: u32,
    pub soil_capacity_mm: u32,
    pub infiltration_mm_per_second: u32,
    pub dry_delay_ticks: u64,
    pub evaporation_mm_per_second: u32,
    pub erosion_velocity_threshold_mm_per_second: u32,
    pub erosion_mm_per_second: u32,
    /// New erosion can occupy at most this share of the local surface-water
    /// volume. Existing excess after flow/infiltration settles over later ticks.
    pub suspended_capacity_permille: u16,
    pub deposition_velocity_threshold_mm_per_second: u32,
    pub deposition_mm_per_second: u32,
    pub foliage_velocity_threshold_mm_per_second: u32,
    pub foliage_units_per_second: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::water::{Barrier, FlowLink, Outlet, StorageCell};

    fn graph(
        beds: &[i32],
        volumes: &[u64],
        links: Vec<FlowLink>,
        outlets: Vec<Outlet>,
    ) -> WaterGraph {
        WaterGraph::new(
            beds.iter()
                .zip(volumes)
                .map(|(&bed_mm, &volume_liters)| StorageCell {
                    bed_mm,
                    area_m2: 1,
                    volume_liters,
                })
                .collect(),
            links,
            vec![],
            outlets,
        )
        .unwrap()
    }
    fn quiet_config() -> GroundConfig {
        GroundConfig {
            infiltration_mm_per_second: 0,
            evaporation_mm_per_second: 0,
            erosion_mm_per_second: 0,
            deposition_mm_per_second: 0,
            foliage_units_per_second: 0,
            ..GroundConfig::default()
        }
    }
    fn model(w: &WaterGraph, c: GroundConfig) -> GroundModel {
        GroundModel::new(
            w,
            w.cells()
                .iter()
                .map(|s| GroundInitial {
                    bed_mm: s.bed_mm,
                    foliage_units: 5,
                    root_strength_permille: 0,
                })
                .collect(),
            c,
        )
        .unwrap()
    }
    fn tick(w: &mut WaterGraph, g: &mut GroundModel, t: u64) {
        w.step().unwrap();
        g.step(w, t).unwrap();
        assert!(g.totals().balanced());
        assert!(g.combined_water_balanced(w));
    }
    fn seed_suspension(w: &mut WaterGraph, g: &mut GroundModel, amount: u64) {
        let c = &mut g.cells[0];
        c.bed_sediment_liters -= amount;
        c.suspended_sediment_liters += amount;
        c.bed_mm = c.hard_floor_mm + (c.bed_sediment_liters / c.area_m2 as u64) as i32;
        w.set_bed(0, c.bed_mm).unwrap();
        g.totals.bed_sediment_liters -= amount as u128;
        g.totals.suspended_sediment_liters += amount as u128;
    }
    fn link(a: usize, b: usize) -> FlowLink {
        FlowLink {
            a,
            b,
            conductance_lps_per_mm: 100,
            barrier: None,
        }
    }

    #[test]
    fn erosion_and_washout_change_depth_without_creating_soil_or_biomass() {
        let mut w = graph(&[1000, 0], &[1000, 0], vec![link(0, 1)], vec![]);
        let mut g = model(
            &w,
            GroundConfig {
                erodible_layer_mm: 3,
                erosion_mm_per_second: 20,
                erosion_velocity_threshold_mm_per_second: 0,
                foliage_units_per_second: 20,
                foliage_velocity_threshold_mm_per_second: 0,
                ..quiet_config()
            },
        );
        for t in 1..=20 {
            tick(&mut w, &mut g, t);
        }
        assert!(g.cells()[0].bed_mm < 1000);
        assert!(g.cells()[0].bed_mm >= 997);
        assert!(g.totals().cumulative_eroded_liters > 0);
        assert!(g.totals().live_foliage_units < g.totals().initial_foliage_units);
        assert!(g.cells().iter().any(|c| c.disturbed));
        let live = g.totals().live_foliage_units;
        for t in 21..=40 {
            tick(&mut w, &mut g, t);
        }
        assert!(g.totals().live_foliage_units <= live); // no rescue regrowth
    }

    #[test]
    fn material_snapshot_moves_only_one_connection_per_tick() {
        let mut w = graph(
            &[0, 0, -100],
            &[1000, 500, 0],
            vec![link(0, 1), link(1, 2)],
            vec![],
        );
        let mut g = model(
            &w,
            GroundConfig {
                erodible_layer_mm: 100,
                ..quiet_config()
            },
        );
        seed_suspension(&mut w, &mut g, 40);
        tick(&mut w, &mut g, 1);
        assert!(g.cells()[1].suspended_sediment_liters > 0);
        assert_eq!(g.cells()[2].suspended_sediment_liters, 0);
    }

    #[test]
    fn disconnected_material_does_not_cross_a_sealed_ridge() {
        let mut edge = link(0, 1);
        edge.barrier = Some(Barrier {
            crest_mm: 5000,
            permeability_permille: 0,
        });
        let mut w = graph(&[0, 0], &[1000, 0], vec![edge], vec![]);
        let mut g = model(&w, quiet_config());
        seed_suspension(&mut w, &mut g, 40);
        for t in 1..=10 {
            tick(&mut w, &mut g, t);
        }
        assert_eq!(g.cells()[0].suspended_sediment_liters, 40);
        assert_eq!(g.cells()[1].suspended_sediment_liters, 0);
    }

    #[test]
    fn dry_deposition_raises_the_bed_using_existing_sediment() {
        let mut w = graph(&[0], &[0], vec![], vec![]);
        let mut g = model(
            &w,
            GroundConfig {
                erodible_layer_mm: 10,
                deposition_mm_per_second: 20,
                ..quiet_config()
            },
        );
        seed_suspension(&mut w, &mut g, 4);
        tick(&mut w, &mut g, 1);
        // No water remains to support even fractional-volume suspension.
        assert_eq!(g.cells()[0].bed_mm, 0);
        assert_eq!(g.cells()[0].suspended_sediment_liters, 0);
        for t in 2..=4 {
            tick(&mut w, &mut g, t);
        }
        assert_eq!(g.cells()[0].bed_mm, 0);
        assert_eq!(g.cells()[0].current_deposit_subset_liters, 4);
        assert_eq!(g.totals().cumulative_deposited_liters, 4);
    }

    #[test]
    fn soil_keeps_moisture_during_air_delay_then_dries_and_rewetting_resets_it() {
        let mut w = graph(
            &[0],
            &[20],
            vec![],
            vec![Outlet {
                cell: 0,
                level_mm: 0,
                conductance_lps_per_mm: 100,
            }],
        );
        let mut g = model(
            &w,
            GroundConfig {
                soil_capacity_mm: 10,
                infiltration_mm_per_second: 20,
                evaporation_mm_per_second: 20,
                dry_delay_ticks: 3,
                ..quiet_config()
            },
        );
        let mut t = 0;
        while g.cells()[0].exposed_since.is_none() {
            t += 1;
            tick(&mut w, &mut g, t);
            assert!(t < 20);
        }
        let since = g.cells()[0].exposed_since.unwrap();
        let wet = g.cells()[0].soil_water_liters;
        assert!(wet > 0);
        for _ in 0..2 {
            t += 1;
            tick(&mut w, &mut g, t);
            assert_eq!(g.cells()[0].soil_water_liters, wet);
        }
        t += 1;
        tick(&mut w, &mut g, t);
        assert_eq!(t - since, 3);
        assert!(g.cells()[0].soil_water_liters < wet);
        // Fund rewetting from the existing ground owner, not hidden new water.
        let released = g.cells[0].soil_water_liters.min(2);
        assert!(released > 0);
        g.cells[0].soil_water_liters -= released;
        g.totals.soil_water_liters -= released as u128;
        g.totals.returned_to_surface_liters += released as u128;
        w.return_water(0, released).unwrap();
        t += 1;
        tick(&mut w, &mut g, t);
        assert_eq!(g.cells()[0].exposed_since, None);
        t += 1;
        tick(&mut w, &mut g, t);
        assert!(g.cells()[0].exposed_since.is_some_and(|n| n > since));
    }

    #[test]
    fn invalid_tick_and_reused_water_flow_do_not_commit_ground_or_exchange() {
        let mut w = graph(&[0], &[10], vec![], vec![]);
        let mut g = model(&w, GroundConfig::default());
        let before = g.totals();
        let water_before = w.totals();
        assert_eq!(g.step(&mut w, 0), Err(GroundError::OutOfOrderTick));
        assert_eq!(g.totals(), before);
        assert_eq!(w.totals(), water_before);
        assert_eq!(g.step(&mut w, 1), Err(GroundError::StaleWaterFlow));
        assert_eq!(g.totals(), before);
        tick(&mut w, &mut g, 1);
        let before = g.totals();
        let water_before = w.totals();
        assert_eq!(g.step(&mut w, 2), Err(GroundError::StaleWaterFlow));
        assert_eq!(g.totals(), before);
        assert_eq!(w.totals(), water_before);
    }

    #[test]
    fn outlet_material_leaves_through_a_named_sink_and_remains_in_the_ledger() {
        let mut w = graph(
            &[0],
            &[1000],
            vec![],
            vec![Outlet {
                cell: 0,
                level_mm: -1000,
                conductance_lps_per_mm: 100,
            }],
        );
        let mut g = model(&w, quiet_config());
        seed_suspension(&mut w, &mut g, 40);
        tick(&mut w, &mut g, 1);
        assert!(g.totals().sediment_outflow_liters > 0);
        assert!(g.cells()[0].suspended_sediment_liters < 40);
    }

    #[test]
    fn saturated_soil_with_submillimeter_film_still_gets_air_and_can_dry() {
        let mut w = WaterGraph::new(
            vec![StorageCell {
                bed_mm: 0,
                area_m2: 64,
                volume_liters: 65,
            }],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let mut g = model(
            &w,
            GroundConfig {
                soil_capacity_mm: 1,
                infiltration_mm_per_second: 20,
                evaporation_mm_per_second: 20,
                dry_delay_ticks: 2,
                ..quiet_config()
            },
        );
        tick(&mut w, &mut g, 1);
        assert_eq!(w.cells()[0].volume_liters, 1);
        assert_eq!(g.cells()[0].soil_water_liters, 64);
        tick(&mut w, &mut g, 2);
        assert_eq!(g.cells()[0].exposed_since, Some(2));
        assert_eq!(g.cells()[0].soil_water_liters, 64);
        tick(&mut w, &mut g, 3);
        assert_eq!(g.cells()[0].soil_water_liters, 64);
        tick(&mut w, &mut g, 4);
        assert_eq!(g.cells()[0].soil_water_liters, 0);
        assert_eq!(g.totals().evaporated_liters, 64);
        tick(&mut w, &mut g, 5);
        assert_eq!(w.cells()[0].volume_liters, 0);
        assert_eq!(g.totals().evaporated_liters, 65);
    }

    #[test]
    fn shallow_flow_film_does_not_scour_soil_or_detach_foliage() {
        let mut w = WaterGraph::new(
            vec![
                StorageCell {
                    bed_mm: 10000,
                    area_m2: 64,
                    volume_liters: 576,
                },
                StorageCell {
                    bed_mm: 0,
                    area_m2: 64,
                    volume_liters: 0,
                },
            ],
            vec![link(0, 1)],
            vec![],
            vec![],
        )
        .unwrap();
        let mut g = model(
            &w,
            GroundConfig {
                erosion_mm_per_second: 20,
                erosion_velocity_threshold_mm_per_second: 0,
                foliage_units_per_second: 20,
                foliage_velocity_threshold_mm_per_second: 0,
                ..quiet_config()
            },
        );
        tick(&mut w, &mut g, 1);
        assert_eq!(g.totals().cumulative_eroded_liters, 0);
        assert_eq!(
            g.totals().live_foliage_units,
            g.totals().initial_foliage_units
        );
    }

    #[test]
    fn low_water_caps_new_erosion_and_closing_flow_stops_further_scouring() {
        let mut w = graph(&[1000, 0], &[100, 0], vec![link(0, 1)], vec![]);
        let mut g = model(
            &w,
            GroundConfig {
                erosion_mm_per_second: 10000,
                erosion_velocity_threshold_mm_per_second: 0,
                suspended_capacity_permille: 100,
                ..quiet_config()
            },
        );
        tick(&mut w, &mut g, 1);
        // Each post-flow50L store can fund at most5L of new suspension,
        // regardless of the much larger configured erosion rate/soil stock.
        assert!(g.totals().cumulative_eroded_liters > 0);
        assert!(g.totals().cumulative_eroded_liters <= 10);
        let before = g.totals().cumulative_eroded_liters;
        w.set_barrier(
            0,
            Some(Barrier {
                crest_mm: 50000,
                permeability_permille: 0,
            }),
        )
        .unwrap();
        for t in 2..=30 {
            tick(&mut w, &mut g, t);
        }
        assert_eq!(g.totals().cumulative_eroded_liters, before);
    }

    #[test]
    fn excessive_suspension_settles_at_a_bounded_rate_even_in_fast_water() {
        let mut w = graph(&[1000, 0], &[1000, 0], vec![link(0, 1)], vec![]);
        let mut g = model(
            &w,
            GroundConfig {
                deposition_mm_per_second: 20,
                deposition_velocity_threshold_mm_per_second: 0,
                ..quiet_config()
            },
        );
        seed_suspension(&mut w, &mut g, 200);
        tick(&mut w, &mut g, 1);
        assert!(w.last_link_flow_liters(0).unwrap() > 0);
        assert!(g.totals().cumulative_deposited_liters > 0);
        assert!(g.totals().cumulative_deposited_liters <= 2); // one1L budget per cell
        assert!(g.cells().iter().any(|c| c.suspended_sediment_liters > 0));
    }

    #[test]
    fn dry_submillimeter_sediment_residue_becomes_real_bed_stock() {
        let mut w = WaterGraph::new(
            vec![StorageCell {
                bed_mm: 0,
                area_m2: 64,
                volume_liters: 0,
            }],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let mut g = model(&w, quiet_config());
        seed_suspension(&mut w, &mut g, 1);
        tick(&mut w, &mut g, 1);
        assert_eq!(g.cells()[0].suspended_sediment_liters, 0);
        assert_eq!(g.totals().cumulative_deposited_liters, 1);
        assert_eq!(g.cells()[0].current_deposit_subset_liters, 1);
        assert_eq!(g.cells()[0].bed_mm, 0);
    }

    #[test]
    fn wet_settling_can_deposit_less_than_one_geometry_millimeter() {
        let mut w = WaterGraph::new(
            vec![StorageCell {
                bed_mm: 0,
                area_m2: 64,
                volume_liters: 640,
            }],
            vec![],
            vec![],
            vec![],
        )
        .unwrap();
        let mut g = model(
            &w,
            GroundConfig {
                deposition_mm_per_second: 20,
                ..quiet_config()
            },
        );
        seed_suspension(&mut w, &mut g, 1);
        tick(&mut w, &mut g, 1);
        assert_eq!(g.cells()[0].suspended_sediment_liters, 0);
        assert_eq!(g.totals().cumulative_deposited_liters, 1);
        assert!(w.cells()[0].volume_liters > 0);
    }

    #[test]
    fn current_root_and_cover_loss_changes_erosion_without_moving_stocks_on_assignment() {
        let mut protected_water = graph(&[1000, 0], &[1000, 0], vec![link(0, 1)], vec![]);
        let mut bare_water = protected_water.clone();
        let config = GroundConfig {
            erosion_mm_per_second: 20,
            erosion_velocity_threshold_mm_per_second: 5000,
            ..quiet_config()
        };
        let mut protected = model(&protected_water, config);
        let mut bare = model(&bare_water, config);
        let before = protected.totals();
        let wet = protected_water.totals();
        for i in 0..2 {
            protected
                .set_biological_conditions(i, 1000, 1000, 1000)
                .unwrap();
            bare.set_biological_conditions(i, 0, 0, 0).unwrap();
        }
        assert_eq!(protected.totals(), before);
        assert_eq!(protected_water.totals(), wet);
        tick(&mut protected_water, &mut protected, 1);
        tick(&mut bare_water, &mut bare, 1);
        assert_eq!(protected.totals().cumulative_eroded_liters, 0);
        assert!(bare.totals().cumulative_eroded_liters > 0);
        assert!(protected.totals().balanced() && bare.totals().balanced());
        assert!(
            protected.combined_water_balanced(&protected_water)
                && bare.combined_water_balanced(&bare_water)
        );
        assert!(protected.set_biological_conditions(0, 1001, 0, 0).is_err());
    }
    #[test]
    fn live_cover_roots_and_canopy_change_actual_infiltration_and_evaporation_conservatively() {
        let mut wet = graph(&[1000], &[100], vec![], vec![]);
        let mut bare_water = wet.clone();
        let config = GroundConfig {
            infiltration_mm_per_second: 20,
            ..quiet_config()
        };
        let mut living = model(&wet, config);
        let mut bare = model(&bare_water, config);
        living
            .set_biological_conditions(0, 1000, 1000, 1000)
            .unwrap();
        bare.set_biological_conditions(0, 0, 0, 0).unwrap();
        for t in 1..=20 {
            tick(&mut wet, &mut living, t);
            tick(&mut bare_water, &mut bare, t);
        }
        assert!(living.totals().soil_water_liters > bare.totals().soil_water_liters);
        // A real paid soil-mixing action changes porosity while conserving its
        // finite substrate/nutrients; the projection then changes actual water uptake.
        let mut habitats = crate::habitat::HabitatWorld::default();
        let mut upper = crate::habitat::SoilLayer {
            mineral_mm3: 1000,
            nutrients: 100,
            porosity_permille: 400,
        };
        let mut lower = crate::habitat::SoilLayer {
            mineral_mm3: 1000,
            nutrients: 20,
            porosity_permille: 600,
        };
        let mut effort = crate::habitat::WorkBudget {
            energy: 100,
            work_ticks: 100,
        };
        habitats
            .mix_soil(0, &mut upper, &mut lower, 500, &mut effort)
            .unwrap();
        assert_eq!(upper.porosity_permille, 500);
        assert_eq!(upper.nutrients + lower.nutrients, 120);
        assert!(effort.energy < 100);
        let mut original_water = graph(&[1000], &[100], vec![], vec![]);
        let mut mixed_water = original_water.clone();
        let mut original = model(&original_water, config);
        let mut mixed = model(&mixed_water, config);
        mixed
            .set_substrate_porosity(0, upper.porosity_permille)
            .unwrap();
        for t in 1..=20 {
            tick(&mut original_water, &mut original, t);
            tick(&mut mixed_water, &mut mixed, t);
        }
        assert!(mixed.totals().soil_water_liters > original.totals().soil_water_liters);
        assert!(mixed.combined_water_balanced(&mixed_water));
        assert!(mixed.set_substrate_porosity(0, 1001).is_err());
        let mut sun_water = graph(
            &[1000],
            &[1000],
            vec![],
            vec![Outlet {
                cell: 0,
                level_mm: 1000,
                conductance_lps_per_mm: 100,
            }],
        );
        let mut shade_water = sun_water.clone();
        let config = GroundConfig {
            infiltration_mm_per_second: 20,
            evaporation_mm_per_second: 20,
            dry_delay_ticks: 0,
            ..quiet_config()
        };
        let mut sun = model(&sun_water, config);
        let mut shade = model(&shade_water, config);
        sun.set_biological_conditions(0, 0, 0, 0).unwrap();
        shade.set_biological_conditions(0, 0, 0, 1000).unwrap();
        for t in 1..=17 {
            tick(&mut sun_water, &mut sun, t);
            tick(&mut shade_water, &mut shade, t);
        }
        assert!(shade.totals().soil_water_liters > sun.totals().soil_water_liters);
        assert!(sun.totals().evaporated_liters > shade.totals().evaporated_liters);
        assert!(
            shade.combined_water_balanced(&shade_water) && sun.combined_water_balanced(&sun_water)
        );
    }
    #[test]
    fn fractional_climate_rates_keep_accumulation_when_weather_changes() {
        let mut w = graph(&[1000], &[100], vec![], vec![]);
        let mut g = model(
            &w,
            GroundConfig {
                infiltration_mm_per_second: 1,
                ..quiet_config()
            },
        );
        g.set_climate(1000, 500).unwrap();
        for t in 1..=19 {
            tick(&mut w, &mut g, t);
        }
        assert_eq!(g.totals().soil_water_liters, 0);
        g.set_climate(1000, 1000).unwrap();
        for t in 20..=30 {
            tick(&mut w, &mut g, t);
        }
        assert_eq!(g.totals().soil_water_liters, 1);
        assert!(g.set_climate(4001, 1000).is_err());
        assert!(g.combined_water_balanced(&w));
    }
    #[test]
    fn construction_cut_fill_transfers_exact_soil_without_moving_water() {
        let mut w = graph(&[1000, 1000], &[100, 50], vec![], vec![]);
        let mut g = model(&w, quiet_config());
        let water = w.totals();
        let t = g
            .edit_beds_material(&mut w, &[(0, 998), (1, 1002)])
            .unwrap();
        assert_eq!(
            t,
            GroundMaterialTransfer {
                cut_liters: 2,
                fill_liters: 2
            }
        );
        assert_eq!(w.totals(), water);
        assert_eq!(g.totals().construction_exported_liters, 2);
        assert_eq!(g.totals().construction_imported_liters, 2);
        assert!(g.totals().balanced());
        let data = serde_json::to_vec(&g).unwrap();
        let mut restored: GroundModel = serde_json::from_slice(&data).unwrap();
        tick(&mut w, &mut restored, 1);
        assert_eq!(restored.cells()[0].bed_mm, 998);
        assert_eq!(restored.cells()[1].bed_mm, 1002);
    }
    #[test]
    fn invalid_construction_transaction_commits_no_partial_cell() {
        let mut w = graph(&[1000, 1000], &[100, 50], vec![], vec![]);
        let mut g = model(&w, quiet_config());
        let before = g.totals();
        assert!(
            g.edit_beds_material(&mut w, &[(0, 999), (1, -1000)])
                .is_err()
        );
        assert_eq!(g.totals(), before);
        assert_eq!(w.cells()[0].bed_mm, 1000);
        assert_eq!(g.cells()[0].bed_mm, 1000);
        assert!(g.edit_beds_material(&mut w, &[(0, 999), (0, 998)]).is_err());
        assert_eq!(g.totals(), before);
    }
    #[test]
    fn invalid_biomass_part_of_construction_rolls_back_all_staged_soil() {
        let mut w = graph(&[1000, 1000], &[10, 10], vec![], vec![]);
        let mut g = model(&w, quiet_config());
        let before = g.totals();
        assert!(
            g.edit_materials_and_foliage(&mut w, &[(0, 999)], &[(1, 6)])
                .is_err()
        );
        assert_eq!(g.totals(), before);
        assert_eq!(g.cells()[0].bed_mm, 1000);
        assert_eq!(w.cells()[0].bed_mm, 1000);
        let moved = g
            .edit_materials_and_foliage(&mut w, &[(0, 999)], &[(0, 3)])
            .unwrap();
        assert_eq!(moved.cut_liters, 1);
        assert_eq!(g.cells()[0].foliage_units, 2);
        assert!(g.totals().balanced());
    }
    #[test]
    fn explicit_foliage_exchange_requires_available_units_and_keeps_owners_exact() {
        let mut w = graph(&[1000], &[0], vec![], vec![]);
        let mut g = model(&w, quiet_config());
        let before = g.totals();
        assert!(g.exchange_foliage(0, 6, 0).is_err());
        assert_eq!(g.totals(), before);
        g.exchange_foliage(0, 3, 2).unwrap();
        assert_eq!(g.cells()[0].foliage_units, 4);
        assert_eq!(g.totals().foliage_exported_units, 3);
        assert_eq!(g.totals().foliage_imported_units, 2);
        tick(&mut w, &mut g, 1);
        assert!(g.totals().balanced());
    }
}
impl Default for GroundConfig {
    fn default() -> Self {
        Self {
            erodible_layer_mm: 500,
            soil_capacity_mm: 100,
            infiltration_mm_per_second: 1,
            dry_delay_ticks: 60 * TACTICAL_HZ as u64,
            evaporation_mm_per_second: 1,
            erosion_velocity_threshold_mm_per_second: 600,
            erosion_mm_per_second: 2,
            suspended_capacity_permille: 100,
            deposition_velocity_threshold_mm_per_second: 150,
            deposition_mm_per_second: 2,
            foliage_velocity_threshold_mm_per_second: 400,
            foliage_units_per_second: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct GroundInitial {
    pub bed_mm: i32,
    pub foliage_units: u32,
    pub root_strength_permille: u16,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundCell {
    pub bed_mm: i32,
    pub foliage_units: u32,
    pub foliage_capacity_units: u32,
    pub living_root_strength_permille: u16,
    pub living_cover_permille: u16,
    pub living_shade_permille: u16,
    pub substrate_porosity_permille: u16,
    biological_conditions_supplied: bool,
    pub moisture_permille: u16,
    pub disturbed: bool,
    pub exposed_since: Option<u64>,
    pub bed_sediment_liters: u64,
    /// Subset of bed_sediment_liters, never added twice in the mass ledger.
    pub current_deposit_subset_liters: u64,
    pub suspended_sediment_liters: u64,
    pub mobile_debris_units: u64,
    pub deposited_debris_units: u64,
    pub soil_water_liters: u64,
    hard_floor_mm: i32,
    area_m2: u32,
    root_strength_permille: u16,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundTotals {
    pub initial_sediment_liters: u128,
    #[serde(default, skip_serializing_if = "zero128")]
    pub construction_imported_liters: u128,
    #[serde(default, skip_serializing_if = "zero128")]
    pub construction_exported_liters: u128,
    #[serde(default, skip_serializing_if = "zero128")]
    pub foliage_imported_units: u128,
    #[serde(default, skip_serializing_if = "zero128")]
    pub foliage_exported_units: u128,
    pub bed_sediment_liters: u128,
    pub current_deposit_subset_liters: u128,
    pub suspended_sediment_liters: u128,
    pub sediment_outflow_liters: u128,
    pub initial_foliage_units: u128,
    pub live_foliage_units: u128,
    pub mobile_debris_units: u128,
    pub deposited_debris_units: u128,
    pub biomass_outflow_units: u128,
    pub initial_soil_water_liters: u128,
    pub soil_water_liters: u128,
    pub infiltrated_liters: u128,
    pub evaporated_liters: u128,
    pub returned_to_surface_liters: u128,
    pub cumulative_eroded_liters: u128,
    pub cumulative_deposited_liters: u128,
}
fn zero128(n: &u128) -> bool {
    *n == 0
}
impl GroundTotals {
    pub fn balanced(&self) -> bool {
        let sediment = self
            .bed_sediment_liters
            .checked_add(self.suspended_sediment_liters)
            .and_then(|v| v.checked_add(self.sediment_outflow_liters))
            .and_then(|v| v.checked_add(self.construction_exported_liters));
        let biomass = self
            .live_foliage_units
            .checked_add(self.mobile_debris_units)
            .and_then(|v| v.checked_add(self.deposited_debris_units))
            .and_then(|v| v.checked_add(self.biomass_outflow_units))
            .and_then(|v| v.checked_add(self.foliage_exported_units));
        let wet_in = self
            .initial_soil_water_liters
            .checked_add(self.infiltrated_liters);
        let wet_out = self
            .soil_water_liters
            .checked_add(self.evaporated_liters)
            .and_then(|v| v.checked_add(self.returned_to_surface_liters));
        sediment.is_some()
            && biomass.is_some()
            && sediment
                == self
                    .initial_sediment_liters
                    .checked_add(self.construction_imported_liters)
            && biomass
                == self
                    .initial_foliage_units
                    .checked_add(self.foliage_imported_units)
            && wet_in.is_some()
            && wet_in == wet_out
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundMaterialTransfer {
    pub cut_liters: u64,
    pub fill_liters: u64,
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroundStep {
    pub eroded_liters: u128,
    pub deposited_liters: u128,
    pub detached_foliage_units: u128,
    pub sediment_outflow_liters: u128,
    pub biomass_outflow_units: u128,
    pub infiltrated_liters: u128,
    pub evaporated_liters: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GroundError {
    InvalidConfig,
    InvalidInitial(usize),
    OutOfOrderTick,
    StaleWaterFlow,
    ChangedTopology,
    Overflow(&'static str),
    ConservationFailure,
    Water(WaterError),
}
impl fmt::Display for GroundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig => write!(f, "Invalid ground configuration"),
            Self::InvalidInitial(i) => write!(f, "Invalid ground initial cell {i}"),
            Self::OutOfOrderTick => {
                write!(f, "Ground ticks must be contiguous authoritative ticks")
            }
            Self::StaleWaterFlow => write!(
                f,
                "Ground requires exactly one fresh water-transfer generation"
            ),
            Self::ChangedTopology => write!(
                f,
                "Ground/water topology changed without a conservative remap"
            ),
            Self::Overflow(s) => write!(f, "Ground arithmetic overflow: {s}"),
            Self::ConservationFailure => write!(f, "Ground material/water ledger did not balance"),
            Self::Water(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for GroundError {}
impl From<WaterError> for GroundError {
    fn from(e: WaterError) -> Self {
        Self::Water(e)
    }
}
fn add(a: u128, b: u128, s: &'static str) -> Result<u128, GroundError> {
    a.checked_add(b).ok_or(GroundError::Overflow(s))
}
fn u64value(v: u128, s: &'static str) -> Result<u64, GroundError> {
    u64::try_from(v).map_err(|_| GroundError::Overflow(s))
}
fn scaled_rate(
    rate: u64,
    climate: u16,
    biology: u16,
    porosity: u16,
    remainder: &mut u64,
) -> Result<u64, GroundError> {
    // One stable denominator preserves fractions across both weather and live
    // biology changes; never round a sub-litre rate away each tick.
    let numerator =
        rate as u128 * climate as u128 * biology as u128 * porosity as u128 + *remainder as u128;
    let denominator = TACTICAL_HZ as u128 * 1_000_000 * 400;
    *remainder = (numerator % denominator) as u64;
    u64value(numerator / denominator, "climate/biology rate")
}

fn rate(rate: u64, remainder: &mut u64) -> Result<u64, GroundError> {
    let n = rate
        .checked_add(*remainder)
        .ok_or(GroundError::Overflow("rate remainder"))?;
    *remainder = n % TACTICAL_HZ as u64;
    Ok(n / TACTICAL_HZ as u64)
}

#[derive(Clone, Serialize, Deserialize)]
struct Scratch {
    cells: Vec<GroundCell>,
    rates: Vec<[u64; 5]>,
    flux: Vec<u128>,
    incoming_sediment: Vec<u128>,
    incoming_biomass: Vec<u128>,
    outgoing_sediment: Vec<u64>,
    outgoing_biomass: Vec<u64>,
    withdrawals: Vec<u64>,
    returns: Vec<u64>,
    changed: Vec<usize>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct GroundModel {
    config: GroundConfig,
    climate_evaporation_permille: u16,
    climate_infiltration_permille: u16,
    cells: Vec<GroundCell>,
    rates: Vec<[u64; 5]>,
    totals: GroundTotals,
    last_tick: u64,
    last_water_generation: u64,
    link_count: usize,
    outlet_count: usize,
    scratch: Scratch,
}
impl GroundModel {
    pub fn new(
        water: &WaterGraph,
        initial: Vec<GroundInitial>,
        config: GroundConfig,
    ) -> Result<Self, GroundError> {
        let rates = [
            config.infiltration_mm_per_second,
            config.evaporation_mm_per_second,
            config.erosion_mm_per_second,
            config.deposition_mm_per_second,
            config.foliage_units_per_second,
        ];
        if initial.len() != water.cells().len()
            || config.erodible_layer_mm > 1_000_000
            || config.soil_capacity_mm == 0
            || config.soil_capacity_mm > 100_000
            || rates.iter().any(|&n| n > 100_000)
            || config.suspended_capacity_permille > 1000
        {
            return Err(GroundError::InvalidConfig);
        }
        let mut cells = Vec::with_capacity(initial.len());
        let mut totals = GroundTotals::default();
        for (i, (init, w)) in initial.into_iter().zip(water.cells()).enumerate() {
            if init.root_strength_permille > 1000 || init.bed_mm != w.bed_mm {
                return Err(GroundError::InvalidInitial(i));
            }
            let floor = init
                .bed_mm
                .checked_sub(config.erodible_layer_mm as i32)
                .ok_or(GroundError::InvalidInitial(i))?;
            let bed = w.area_m2 as u64 * config.erodible_layer_mm as u64;
            totals.initial_sediment_liters = add(
                totals.initial_sediment_liters,
                bed as u128,
                "initial sediment",
            )?;
            totals.initial_foliage_units = add(
                totals.initial_foliage_units,
                init.foliage_units as u128,
                "initial foliage",
            )?;
            cells.push(GroundCell {
                bed_mm: init.bed_mm,
                foliage_units: init.foliage_units,
                foliage_capacity_units: init.foliage_units,
                living_root_strength_permille: init.root_strength_permille,
                living_cover_permille: 1000,
                living_shade_permille: 0,
                substrate_porosity_permille: 400,
                biological_conditions_supplied: false,
                moisture_permille: 0,
                disturbed: false,
                exposed_since: if w.depth_mm() < AIR_EXPOSURE_DEPTH_MM {
                    Some(0)
                } else {
                    None
                },
                bed_sediment_liters: bed,
                current_deposit_subset_liters: 0,
                suspended_sediment_liters: 0,
                mobile_debris_units: 0,
                deposited_debris_units: 0,
                soil_water_liters: 0,
                hard_floor_mm: floor,
                area_m2: w.area_m2,
                root_strength_permille: init.root_strength_permille,
            });
        }
        totals.bed_sediment_liters = totals.initial_sediment_liters;
        totals.live_foliage_units = totals.initial_foliage_units;
        let n = cells.len();
        Ok(Self {
            config,
            climate_evaporation_permille: 1000,
            climate_infiltration_permille: 1000,
            cells: cells.clone(),
            rates: vec![[0; 5]; n],
            totals,
            last_tick: 0,
            last_water_generation: water.transport_generation(),
            link_count: water.links().len(),
            outlet_count: water.outlets().len(),
            scratch: Scratch {
                cells,
                rates: vec![[0; 5]; n],
                flux: vec![0; n],
                incoming_sediment: vec![0; n],
                incoming_biomass: vec![0; n],
                outgoing_sediment: vec![0; n],
                outgoing_biomass: vec![0; n],
                withdrawals: vec![0; n],
                returns: vec![0; n],
                changed: Vec::with_capacity(n),
            },
        })
    }
    /// Changes rates without creating water or resetting fractional accumulation.
    pub fn set_climate(
        &mut self,
        evaporation_permille: u16,
        infiltration_permille: u16,
    ) -> Result<(), GroundError> {
        if evaporation_permille > 4000 || infiltration_permille > 4000 {
            return Err(GroundError::InvalidConfig);
        }
        self.climate_evaporation_permille = evaporation_permille;
        self.climate_infiltration_permille = infiltration_permille;
        Ok(())
    }
    /// These are current living-root/cover/canopy conditions from the ecology
    /// owner, not a second biomass stock. Changing them moves no water or soil.
    /// Uncoupled standalone fixtures retain their calibrated original profile.
    pub fn set_biological_conditions(
        &mut self,
        index: usize,
        root_strength_permille: u16,
        cover_permille: u16,
        shade_permille: u16,
    ) -> Result<(), GroundError> {
        if [root_strength_permille, cover_permille, shade_permille]
            .iter()
            .any(|&n| n > 1000)
        {
            return Err(GroundError::InvalidConfig);
        }
        let c = self
            .cells
            .get_mut(index)
            .ok_or(GroundError::InvalidInitial(index))?;
        c.living_root_strength_permille = root_strength_permille;
        c.living_cover_permille = cover_permille;
        c.living_shade_permille = shade_permille;
        c.biological_conditions_supplied = true;
        Ok(())
    }
    /// Physical substrate condition from the material owner;400 is the original
    /// reference permeability. Mixing/compaction changes rates, never stock.
    pub fn set_substrate_porosity(
        &mut self,
        index: usize,
        permille: u16,
    ) -> Result<(), GroundError> {
        if permille > 1000 {
            return Err(GroundError::InvalidConfig);
        }
        let c = self
            .cells
            .get_mut(index)
            .ok_or(GroundError::InvalidInitial(index))?;
        c.substrate_porosity_permille = permille;
        Ok(())
    }
    pub fn cells(&self) -> &[GroundCell] {
        &self.cells
    }
    pub fn totals(&self) -> GroundTotals {
        self.totals
    }
    pub fn last_tick(&self) -> u64 {
        self.last_tick
    }
    pub fn changed_cells(&self) -> &[usize] {
        &self.scratch.changed
    }
    /// Transfer construction material at existing cells, transactionally. The
    /// caller must fund `fill_liters` and receive `cut_liters` in its spoil owner.
    /// Bed cache changes never change a single liter of surface/soil water.
    pub fn edit_beds_material(
        &mut self,
        water: &mut WaterGraph,
        edits: &[(usize, i32)],
    ) -> Result<GroundMaterialTransfer, GroundError> {
        self.edit_materials_and_foliage(water, edits, &[])
    }

    /// One staged physical commit for construction that also clears vegetation.
    /// All bed and biomass removals validate before any owned quantity changes.
    pub fn edit_materials_and_foliage(
        &mut self,
        water: &mut WaterGraph,
        edits: &[(usize, i32)],
        foliage_removals: &[(usize, u32)],
    ) -> Result<GroundMaterialTransfer, GroundError> {
        let mut staged = Vec::with_capacity(edits.len() + foliage_removals.len());
        let mut transfer = GroundMaterialTransfer::default();
        let mut totals = self.totals;
        for &(i, target) in edits {
            if staged.iter().any(|(j, _)| *j == i) {
                return Err(GroundError::InvalidInitial(i));
            }
            let mut cell = *self.cells.get(i).ok_or(GroundError::InvalidInitial(i))?;
            let wc = water.cells().get(i).ok_or(GroundError::ChangedTopology)?;
            if wc.bed_mm != cell.bed_mm || wc.area_m2 != cell.area_m2 {
                return Err(GroundError::ChangedTopology);
            }
            if target < cell.hard_floor_mm || !(-1_000_000..=1_000_000).contains(&target) {
                return Err(GroundError::InvalidInitial(i));
            }
            let delta = target as i64 - cell.bed_mm as i64;
            let liters = delta
                .unsigned_abs()
                .checked_mul(cell.area_m2 as u64)
                .ok_or(GroundError::Overflow("construction volume"))?;
            if delta < 0 {
                cell.bed_sediment_liters = cell
                    .bed_sediment_liters
                    .checked_sub(liters)
                    .ok_or(GroundError::ConservationFailure)?;
                let deposit_removed = cell.current_deposit_subset_liters.min(liters);
                cell.current_deposit_subset_liters -= deposit_removed;
                totals.current_deposit_subset_liters -= deposit_removed as u128;
                totals.bed_sediment_liters -= liters as u128;
                totals.construction_exported_liters = add(
                    totals.construction_exported_liters,
                    liters as u128,
                    "construction export",
                )?;
                transfer.cut_liters = transfer
                    .cut_liters
                    .checked_add(liters)
                    .ok_or(GroundError::Overflow("cut total"))?;
            } else {
                cell.bed_sediment_liters = cell
                    .bed_sediment_liters
                    .checked_add(liters)
                    .ok_or(GroundError::Overflow("construction fill"))?;
                cell.current_deposit_subset_liters = cell
                    .current_deposit_subset_liters
                    .checked_add(liters)
                    .ok_or(GroundError::Overflow("construction deposit"))?;
                totals.current_deposit_subset_liters = add(
                    totals.current_deposit_subset_liters,
                    liters as u128,
                    "construction deposit total",
                )?;
                totals.bed_sediment_liters = add(
                    totals.bed_sediment_liters,
                    liters as u128,
                    "construction bed total",
                )?;
                totals.construction_imported_liters = add(
                    totals.construction_imported_liters,
                    liters as u128,
                    "construction import",
                )?;
                transfer.fill_liters = transfer
                    .fill_liters
                    .checked_add(liters)
                    .ok_or(GroundError::Overflow("fill total"))?;
            }
            cell.bed_mm = target;
            cell.disturbed |= delta != 0;
            staged.push((i, cell));
        }
        let mut removed_indices = std::collections::BTreeSet::new();
        for &(index, units) in foliage_removals {
            if !removed_indices.insert(index) {
                return Err(GroundError::InvalidInitial(index));
            }
            let position = staged.iter().position(|(i, _)| *i == index);
            let mut c = match position {
                Some(i) => staged[i].1,
                None => *self
                    .cells
                    .get(index)
                    .ok_or(GroundError::InvalidInitial(index))?,
            };
            let wc = water
                .cells()
                .get(index)
                .ok_or(GroundError::ChangedTopology)?;
            if wc.bed_mm != self.cells[index].bed_mm || wc.area_m2 != c.area_m2 {
                return Err(GroundError::ChangedTopology);
            }
            c.foliage_units = c
                .foliage_units
                .checked_sub(units)
                .ok_or(GroundError::ConservationFailure)?;
            c.disturbed |= units != 0;
            totals.live_foliage_units = totals
                .live_foliage_units
                .checked_sub(units as u128)
                .ok_or(GroundError::ConservationFailure)?;
            totals.foliage_exported_units = add(
                totals.foliage_exported_units,
                units as u128,
                "construction biomass export",
            )?;
            if let Some(i) = position {
                staged[i].1 = c;
            } else {
                staged.push((index, c));
            }
        }
        if !totals.balanced() {
            return Err(GroundError::ConservationFailure);
        }
        for &(i, c) in &staged {
            water.set_bed(i, c.bed_mm)?;
        }
        for (i, c) in staged {
            self.cells[i] = c;
            if !self.scratch.changed.contains(&i) {
                self.scratch.changed.push(i);
            }
        }
        self.totals = totals;
        Ok(transfer)
    }

    /// Explicit external biological exchange. Removed units leave this owner;
    /// additions must be funded by a separate real growth/propagule transaction.
    pub fn exchange_foliage(
        &mut self,
        index: usize,
        remove: u32,
        add_units: u32,
    ) -> Result<(), GroundError> {
        let mut c = *self
            .cells
            .get(index)
            .ok_or(GroundError::InvalidInitial(index))?;
        c.foliage_units = c
            .foliage_units
            .checked_sub(remove)
            .and_then(|n| n.checked_add(add_units))
            .ok_or(GroundError::ConservationFailure)?;
        if c.foliage_units > c.foliage_capacity_units {
            return Err(GroundError::ConservationFailure);
        }
        let mut t = self.totals;
        t.live_foliage_units = t
            .live_foliage_units
            .checked_sub(remove as u128)
            .and_then(|n| n.checked_add(add_units as u128))
            .ok_or(GroundError::ConservationFailure)?;
        t.foliage_imported_units = add(
            t.foliage_imported_units,
            add_units as u128,
            "foliage import",
        )?;
        t.foliage_exported_units = add(t.foliage_exported_units, remove as u128, "foliage export")?;
        if !t.balanced() {
            return Err(GroundError::ConservationFailure);
        }
        c.disturbed |= remove != 0;
        self.cells[index] = c;
        self.totals = t;
        if !self.scratch.changed.contains(&index) {
            self.scratch.changed.push(index);
        }
        Ok(())
    }

    pub fn combined_water_balanced(&self, water: &WaterGraph) -> bool {
        let w = water.totals();
        let g = self.totals;
        if !w.balanced()
            || !g.balanced()
            || w.extracted_liters != g.infiltrated_liters
            || w.returned_liters != g.returned_to_surface_liters
        {
            return false;
        }
        w.initial_liters
            .checked_add(g.initial_soil_water_liters)
            .and_then(|v| v.checked_add(w.added_liters))
            == w.stored_liters
                .checked_add(g.soil_water_liters)
                .and_then(|v| v.checked_add(w.drained_liters))
                .and_then(|v| v.checked_add(g.evaporated_liters))
    }
    /// One ordinary tick. Water must have stepped once immediately beforehand.
    /// No source arrays, settings or rendering data are allocated here.
    pub fn step(&mut self, water: &mut WaterGraph, tick: u64) -> Result<GroundStep, GroundError> {
        if self.last_tick.checked_add(1) != Some(tick) {
            return Err(GroundError::OutOfOrderTick);
        }
        if self.last_water_generation.checked_add(1) != Some(water.transport_generation()) {
            return Err(GroundError::StaleWaterFlow);
        }
        if self.cells.len() != water.cells().len()
            || self.link_count != water.links().len()
            || self.outlet_count != water.outlets().len()
        {
            return Err(GroundError::ChangedTopology);
        }
        if self
            .cells
            .iter()
            .zip(water.cells())
            .any(|(g, w)| g.bed_mm != w.bed_mm || g.area_m2 != w.area_m2)
        {
            return Err(GroundError::ChangedTopology);
        }
        let s = &mut self.scratch;
        s.cells.copy_from_slice(&self.cells);
        s.rates.copy_from_slice(&self.rates);
        s.flux.fill(0);
        s.incoming_sediment.fill(0);
        s.incoming_biomass.fill(0);
        s.outgoing_sediment.fill(0);
        s.outgoing_biomass.fill(0);
        s.withdrawals.fill(0);
        s.returns.fill(0);
        s.changed.clear();
        for (i, link) in water.links().iter().enumerate() {
            let q = water.last_link_flow_liters(i).unwrap_or(0).unsigned_abs();
            s.flux[link.a] = add(s.flux[link.a], q, "cell flux")?;
            s.flux[link.b] = add(s.flux[link.b], q, "cell flux")?;
        }
        for (i, outlet) in water.outlets().iter().enumerate() {
            s.flux[outlet.cell] = add(
                s.flux[outlet.cell],
                water.last_outlet_flow_liters(i).unwrap_or(0) as u128,
                "outlet flux",
            )?;
        }
        let mut result = GroundStep::default();
        for i in 0..s.cells.len() {
            let c = &mut s.cells[i];
            let w = water.cells()[i];
            if w.volume_liters == 0
                && c.soil_water_liters == 0
                && c.suspended_sediment_liters == 0
                && c.mobile_debris_units == 0
                && s.flux[i] == 0
                && c.exposed_since.is_some()
            {
                // Nothing is moving, wet or pending transport. Persistent
                // exposure/disturbance/foliage stays intact without physiology.
                continue;
            }
            let width = c.area_m2.isqrt().max(1) as u128;
            let velocity = s.flux[i]
                .checked_mul(TACTICAL_HZ as u128 * 1000)
                .ok_or(GroundError::Overflow("flow velocity"))?
                / (2 * width * w.depth_mm().max(1) as u128);
            let velocity = velocity.min(u32::MAX as u128) as u32;
            let root_multiplier = if c.biological_conditions_supplied {
                1000 + c.living_root_strength_permille as u64 + c.living_cover_permille as u64 / 2
            } else {
                1000 + c.root_strength_permille as u64
            };
            let infiltration_biology = if c.biological_conditions_supplied {
                750 + c.living_root_strength_permille / 4 + c.living_cover_permille / 4
            } else {
                1000
            };
            let evaporation_biology = if c.biological_conditions_supplied {
                1000 - c.living_shade_permille / 2 - c.living_cover_permille / 4
            } else {
                1000
            };
            let erosion_threshold = self.config.erosion_velocity_threshold_mm_per_second as u64
                * root_multiplier
                / 1000;
            if velocity as u64 > erosion_threshold && w.depth_mm() >= MIN_EROSION_DEPTH_MM {
                let mm = rate(self.config.erosion_mm_per_second as u64, &mut s.rates[i][0])?;
                let capacity = w.volume_liters as u128
                    * self.config.suspended_capacity_permille as u128
                    / 1000;
                let free = if capacity > c.suspended_sediment_liters as u128 {
                    capacity - c.suspended_sediment_liters as u128
                } else {
                    0
                };
                let whole_mm_capacity = free / c.area_m2 as u128 * c.area_m2 as u128;
                let liters = (mm * c.area_m2 as u64)
                    .min(c.bed_sediment_liters / c.area_m2 as u64 * c.area_m2 as u64)
                    .min(whole_mm_capacity as u64);
                c.bed_sediment_liters -= liters;
                c.current_deposit_subset_liters -= liters.min(c.current_deposit_subset_liters);
                c.suspended_sediment_liters = c
                    .suspended_sediment_liters
                    .checked_add(liters)
                    .ok_or(GroundError::Overflow("eroded suspension"))?;
                if liters != 0 {
                    c.disturbed = true;
                }
                result.eroded_liters = add(result.eroded_liters, liters as u128, "eroded total")?;
            }
            let foliage_threshold = self.config.foliage_velocity_threshold_mm_per_second as u64
                * root_multiplier
                / 1000;
            if velocity as u64 > foliage_threshold && w.depth_mm() >= MIN_EROSION_DEPTH_MM {
                let detached = rate(
                    self.config.foliage_units_per_second as u64,
                    &mut s.rates[i][2],
                )?
                .min(c.foliage_units as u64);
                c.foliage_units -= detached as u32;
                c.mobile_debris_units = c
                    .mobile_debris_units
                    .checked_add(detached)
                    .ok_or(GroundError::Overflow("detached biomass"))?;
                if detached != 0 {
                    c.disturbed = true;
                }
                result.detached_foliage_units = add(
                    result.detached_foliage_units,
                    detached as u128,
                    "detached total",
                )?;
            }
            if w.volume_liters != 0 {
                let capacity = c.area_m2 as u64 * self.config.soil_capacity_mm as u64;
                let infiltrate = scaled_rate(
                    c.area_m2 as u64 * self.config.infiltration_mm_per_second as u64,
                    self.climate_infiltration_permille,
                    infiltration_biology,
                    c.substrate_porosity_permille,
                    &mut s.rates[i][3],
                )?
                .min(capacity - c.soil_water_liters)
                .min(w.volume_liters);
                c.soil_water_liters += infiltrate;
                s.withdrawals[i] = infiltrate;
                result.infiltrated_liters = add(
                    result.infiltrated_liters,
                    infiltrate as u128,
                    "infiltration",
                )?;
            }
            if w.depth_mm() >= AIR_EXPOSURE_DEPTH_MM {
                c.exposed_since = None;
            } else {
                let since = *c.exposed_since.get_or_insert(tick);
                if tick - since >= self.config.dry_delay_ticks && c.soil_water_liters != 0 {
                    let evaporate = scaled_rate(
                        c.area_m2 as u64 * self.config.evaporation_mm_per_second as u64,
                        self.climate_evaporation_permille,
                        evaporation_biology,
                        400,
                        &mut s.rates[i][4],
                    )?
                    .min(c.soil_water_liters);
                    c.soil_water_liters -= evaporate;
                    result.evaporated_liters =
                        add(result.evaporated_liters, evaporate as u128, "evaporation")?;
                }
            }
            // Store velocity in otherwise separate scratch to decide deposition
            // after all suspended/debris transfers have committed to staging.
            s.flux[i] = velocity as u128;
        }
        // Every transfer reads these same post-detachment source stocks. Incoming
        // material cannot be transported onward again during this tick.
        for i in 0..water.links().len() + water.outlets().len() {
            let (from, to, q) = if i < water.links().len() {
                let l = water.links()[i];
                let q = water.last_link_flow_liters(i).unwrap_or(0);
                if q >= 0 {
                    (l.a, Some(l.b), q as u64)
                } else {
                    (l.b, Some(l.a), (-q) as u64)
                }
            } else {
                let j = i - water.links().len();
                (
                    water.outlets()[j].cell,
                    None,
                    water.last_outlet_flow_liters(j).unwrap_or(0),
                )
            };
            if q == 0 {
                continue;
            }
            let available = water.last_transport_volumes()[from];
            if available == 0 {
                return Err(GroundError::ConservationFailure);
            }
            let sed = u64value(
                s.cells[from].suspended_sediment_liters as u128 * q as u128 / available as u128,
                "sediment request",
            )?;
            let bio = u64value(
                s.cells[from].mobile_debris_units as u128 * q as u128 / available as u128,
                "biomass request",
            )?;
            s.outgoing_sediment[from] = s.outgoing_sediment[from]
                .checked_add(sed)
                .ok_or(GroundError::Overflow("outgoing sediment"))?;
            s.outgoing_biomass[from] = s.outgoing_biomass[from]
                .checked_add(bio)
                .ok_or(GroundError::Overflow("outgoing biomass"))?;
            if let Some(to) = to {
                s.incoming_sediment[to] =
                    add(s.incoming_sediment[to], sed as u128, "incoming sediment")?;
                s.incoming_biomass[to] =
                    add(s.incoming_biomass[to], bio as u128, "incoming biomass")?;
            } else {
                result.sediment_outflow_liters = add(
                    result.sediment_outflow_liters,
                    sed as u128,
                    "sediment boundary",
                )?;
                result.biomass_outflow_units = add(
                    result.biomass_outflow_units,
                    bio as u128,
                    "biomass boundary",
                )?;
            }
        }
        for i in 0..s.cells.len() {
            let c = &mut s.cells[i];
            c.suspended_sediment_liters = u64value(
                (c.suspended_sediment_liters
                    .checked_sub(s.outgoing_sediment[i])
                    .ok_or(GroundError::ConservationFailure)? as u128)
                    .checked_add(s.incoming_sediment[i])
                    .ok_or(GroundError::Overflow("suspension"))?,
                "suspension",
            )?;
            c.mobile_debris_units = u64value(
                (c.mobile_debris_units
                    .checked_sub(s.outgoing_biomass[i])
                    .ok_or(GroundError::ConservationFailure)? as u128)
                    .checked_add(s.incoming_biomass[i])
                    .ok_or(GroundError::Overflow("mobile debris"))?,
                "mobile debris",
            )?;
            let remaining_water = water.cells()[i].volume_liters - s.withdrawals[i];
            let dry = remaining_water == 0;
            let capacity =
                remaining_water as u128 * self.config.suspended_capacity_permille as u128 / 1000;
            let overloaded = c.suspended_sediment_liters as u128 > capacity;
            if c.suspended_sediment_liters != 0
                && (s.flux[i] <= self.config.deposition_velocity_threshold_mm_per_second as u128
                    || dry
                    || overloaded)
            {
                let mm = rate(
                    self.config.deposition_mm_per_second as u64,
                    &mut s.rates[i][1],
                )?;
                // A dry cell cannot keep material suspended. Wet settling is
                // rate-limited, including excess concentrations in fast water.
                // Partial liters enter real bed stock; geometry retains <1mm
                // quantization rather than marooning dust without any water.
                let deposit = if dry {
                    c.suspended_sediment_liters
                } else {
                    (mm * c.area_m2 as u64).min(c.suspended_sediment_liters)
                };
                c.suspended_sediment_liters -= deposit;
                c.bed_sediment_liters = c
                    .bed_sediment_liters
                    .checked_add(deposit)
                    .ok_or(GroundError::Overflow("deposited bed"))?;
                c.current_deposit_subset_liters = c
                    .current_deposit_subset_liters
                    .checked_add(deposit)
                    .ok_or(GroundError::Overflow("deposit subset"))?;
                result.deposited_liters =
                    add(result.deposited_liters, deposit as u128, "deposit total")?;
                if deposit != 0 {
                    c.disturbed = true;
                }
            }
            if dry && c.mobile_debris_units != 0 {
                c.deposited_debris_units = c
                    .deposited_debris_units
                    .checked_add(c.mobile_debris_units)
                    .ok_or(GroundError::Overflow("debris deposit"))?;
                c.mobile_debris_units = 0;
            }
            let elevation =
                c.hard_floor_mm as i128 + (c.bed_sediment_liters / c.area_m2 as u64) as i128;
            c.bed_mm =
                i32::try_from(elevation).map_err(|_| GroundError::Overflow("bed elevation"))?;
            c.moisture_permille = (c.soil_water_liters as u128 * 1000
                / (c.area_m2 as u128 * self.config.soil_capacity_mm as u128))
                as u16;
            if *c != self.cells[i] {
                s.changed.push(i);
            }
        }
        let mut totals = GroundTotals {
            initial_sediment_liters: self.totals.initial_sediment_liters,
            construction_imported_liters: self.totals.construction_imported_liters,
            construction_exported_liters: self.totals.construction_exported_liters,
            foliage_imported_units: self.totals.foliage_imported_units,
            foliage_exported_units: self.totals.foliage_exported_units,
            initial_foliage_units: self.totals.initial_foliage_units,
            initial_soil_water_liters: self.totals.initial_soil_water_liters,
            sediment_outflow_liters: add(
                self.totals.sediment_outflow_liters,
                result.sediment_outflow_liters,
                "cumulative sediment outflow",
            )?,
            biomass_outflow_units: add(
                self.totals.biomass_outflow_units,
                result.biomass_outflow_units,
                "cumulative biomass outflow",
            )?,
            infiltrated_liters: add(
                self.totals.infiltrated_liters,
                result.infiltrated_liters,
                "cumulative infiltration",
            )?,
            evaporated_liters: add(
                self.totals.evaporated_liters,
                result.evaporated_liters,
                "cumulative evaporation",
            )?,
            cumulative_eroded_liters: add(
                self.totals.cumulative_eroded_liters,
                result.eroded_liters,
                "cumulative erosion",
            )?,
            cumulative_deposited_liters: add(
                self.totals.cumulative_deposited_liters,
                result.deposited_liters,
                "cumulative deposition",
            )?,
            returned_to_surface_liters: self.totals.returned_to_surface_liters,
            ..GroundTotals::default()
        };
        for c in &s.cells {
            totals.bed_sediment_liters = add(
                totals.bed_sediment_liters,
                c.bed_sediment_liters as u128,
                "bed stock",
            )?;
            totals.current_deposit_subset_liters = add(
                totals.current_deposit_subset_liters,
                c.current_deposit_subset_liters as u128,
                "deposit subset",
            )?;
            totals.suspended_sediment_liters = add(
                totals.suspended_sediment_liters,
                c.suspended_sediment_liters as u128,
                "suspended stock",
            )?;
            totals.live_foliage_units = add(
                totals.live_foliage_units,
                c.foliage_units as u128,
                "live stock",
            )?;
            totals.mobile_debris_units = add(
                totals.mobile_debris_units,
                c.mobile_debris_units as u128,
                "mobile stock",
            )?;
            totals.deposited_debris_units = add(
                totals.deposited_debris_units,
                c.deposited_debris_units as u128,
                "deposited stock",
            )?;
            totals.soil_water_liters = add(
                totals.soil_water_liters,
                c.soil_water_liters as u128,
                "soil water stock",
            )?;
        }
        if !totals.balanced() {
            return Err(GroundError::ConservationFailure);
        }
        water.exchange_cells(&s.withdrawals, &s.returns)?;
        // Indices and i32 elevations were all prevalidated. These geometry cache
        // writes cannot fail after successful exchange for this fixed topology.
        for &i in &s.changed {
            water.set_bed(i, s.cells[i].bed_mm)?;
        }
        self.cells.copy_from_slice(&s.cells);
        self.rates.copy_from_slice(&s.rates);
        self.totals = totals;
        self.last_tick = tick;
        self.last_water_generation = water.transport_generation();
        Ok(result)
    }
}
