//! Native interaction fixture for the shared water model. This is not a20km
//! campaign, an animal dam-building behavior or an integrated combat test.
use crate::{
    ground::{GroundConfig, GroundInitial, GroundModel},
    terrain::{CELL_METERS, CW, Terrain},
    water::{Barrier, FlowLink, Inlet, Outlet, StorageCell, WaterGraph},
};

pub const WIDTH: usize = 96;
pub const HEIGHT: usize = 128;
pub const ORIGIN: [usize; 2] = [250, 40];
const DAM_ROW: usize = 80;
const CONDUCTANCE: u32 = 100;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RiverFixture {
    pub water: WaterGraph,
    pub ground: GroundModel,
    pub last_processed_tick: u64,
    pub dam_closed: bool,
    dam_links: Vec<usize>,
    crest_mm: i32,
    flow_vectors: Vec<[f32; 2]>,
    terrain_revision: u64,
    automated_test: bool,
}

/// Deterministic CPU integration evidence, without a GPU or frame-time claim.
pub fn interaction_report() -> Result<serde_json::Value, String> {
    let mut terrain = Terrain::new(78421);
    let mut river = RiverFixture::new(&mut terrain)?;
    let mut phases = vec![serde_json::json!({"phase":"initial_open", "water":river.describe()})];
    river.advance_to(1200)?;
    phases.push(
        serde_json::json!({"phase":"open_after_60_simulation_seconds", "water":river.describe()}),
    );
    let before = river.water.totals().stored_liters;
    river.set_dam(true)?;
    if river.water.totals().stored_liters != before {
        return Err("Closing dam created/deleted water".into());
    }
    phases
        .push(serde_json::json!({"phase":"dam_closed_before_next_tick", "water":river.describe()}));
    for (tick, phase) in [
        (1201, "closed_after_one_tick"),
        (1600, "closed_after_20s"),
        (2400, "closed_after_60s"),
        (3600, "closed_after_120s"),
    ] {
        river.advance_to(tick)?;
        phases.push(serde_json::json!({"phase":phase,"water":river.describe()}));
    }
    let before = river.water.totals().stored_liters;
    river.set_dam(false)?;
    if river.water.totals().stored_liters != before {
        return Err("Breach deleted/created water".into());
    }
    phases.push(serde_json::json!({"phase":"breached_before_next_tick","water":river.describe()}));
    river.advance_to(3601)?;
    phases.push(serde_json::json!({"phase":"breached_after_one_tick","water":river.describe()}));
    river.advance_to(4800)?;
    river.sync_terrain(&mut terrain)?;
    phases.push(serde_json::json!({"phase":"breached_after_60s","water":river.describe()}));
    Ok(
        serde_json::json!({"schema_version":2,"kind":"cpu_coupled_ground_water_correctness_not_performance", "seed":78421, "gpu_created":false, "ground_configuration":GroundConfig::default(), "phases":phases, "conserved":river.water.balanced() && river.ground.totals().balanced() && river.ground.combined_water_balanced(&river.water)}),
    )
}

impl RiverFixture {
    pub fn new(terrain: &mut Terrain) -> Result<Self, String> {
        let seed = terrain.seed;
        let mut cells = Vec::with_capacity(WIDTH * HEIGHT);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let wx = ((ORIGIN[0] + x) * CELL_METERS + CELL_METERS / 2) as f64;
                let wy = ((ORIGIN[1] + y) * CELL_METERS + CELL_METERS / 2) as f64;
                // Continuous seeded channel and banks; this is terrain geometry,
                // never a prepainted reservoir or primitive-shaped sprite.
                let center = 2390.
                    + (crate::terrain::noise(0., wy, 840., seed.wrapping_add(106)) - 0.5) * 250.
                    + (crate::terrain::noise(0., wy, 193., seed.wrapping_add(107)) - 0.5) * 45.;
                let half_width =
                    14. + crate::terrain::noise(0., wy, 230., seed.wrapping_add(108)) * 10.;
                let distance = (wx - center).abs();
                let bank_distance = (distance - half_width).max(0.);
                let bank = bank_distance * 22. + bank_distance * bank_distance * 0.018;
                let roughness =
                    (crate::terrain::noise(wx, wy, 31., seed.wrapping_add(220)) - 0.5) * 80.;
                let bed_mm = (16000. - y as f64 * 80. + bank + roughness).round() as i32;
                let depth_mm = if distance < half_width { 400 } else { 0 };
                cells.push(StorageCell {
                    bed_mm,
                    area_m2: (CELL_METERS * CELL_METERS) as u32,
                    volume_liters: depth_mm * (CELL_METERS * CELL_METERS) as u64,
                });
                let global = (ORIGIN[1] + y) * CW + ORIGIN[0] + x;
                terrain
                    .set_elevation_mm(global, bed_mm)
                    .map_err(|e| e.to_string())?;
                terrain
                    .set_water_depth_mm(global, depth_mm as u32)
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut links = Vec::with_capacity(WIDTH * HEIGHT * 2);
        let mut dam_links = Vec::new();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let a = y * WIDTH + x;
                if x + 1 < WIDTH {
                    links.push(FlowLink {
                        a,
                        b: a + 1,
                        conductance_lps_per_mm: CONDUCTANCE,
                        barrier: None,
                    });
                }
                if y + 1 < HEIGHT {
                    if y == DAM_ROW {
                        dam_links.push(links.len());
                    }
                    links.push(FlowLink {
                        a,
                        b: a + WIDTH,
                        conductance_lps_per_mm: CONDUCTANCE,
                        barrier: None,
                    });
                }
            }
        }
        let inlet = (0..WIDTH)
            .min_by_key(|&x| cells[x].bed_mm)
            .ok_or("No inlet")?;
        // Named external upstream supply. Rain/groundwater are not yet modeled.
        let inlets = vec![Inlet {
            cell: inlet,
            liters_per_second: 25_000,
        }];
        let outlets = (0..WIDTH)
            .map(|x| {
                let cell = (HEIGHT - 1) * WIDTH + x;
                Outlet {
                    cell,
                    level_mm: cells[cell].bed_mm,
                    conductance_lps_per_mm: CONDUCTANCE,
                }
            })
            .collect();
        let crest_mm = (16000 - DAM_ROW as i32 * 80) + 1400;
        let water = WaterGraph::new(cells, links, inlets, outlets).map_err(|e| e.to_string())?;
        let initial = water
            .cells()
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let global = (ORIGIN[1] + i / WIDTH) * CW + ORIGIN[0] + i % WIDTH;
                Ok(GroundInitial {
                    bed_mm: cell.bed_mm,
                    foliage_units: terrain.foliage_units(global).map_err(|e| e.to_string())?,
                    root_strength_permille: if terrain.cells[global].cover > 100. {
                        650
                    } else {
                        200
                    },
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let ground = GroundModel::new(&water, initial, GroundConfig::default())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            water,
            ground,
            last_processed_tick: 0,
            dam_closed: false,
            dam_links,
            crest_mm,
            flow_vectors: vec![[0.; 2]; WIDTH * HEIGHT],
            terrain_revision: terrain.revision(),
            automated_test: false,
        })
    }

    pub fn set_climate(
        &mut self,
        evaporation_permille: u16,
        infiltration_permille: u16,
    ) -> Result<(), String> {
        self.ground
            .set_climate(evaporation_permille, infiltration_permille)
            .map_err(|e| e.to_string())
    }

    pub fn set_biological_conditions(
        &mut self,
        local: usize,
        root_strength_permille: u16,
        cover_permille: u16,
        shade_permille: u16,
    ) -> Result<(), String> {
        self.ground
            .set_biological_conditions(
                local,
                root_strength_permille,
                cover_permille,
                shade_permille,
            )
            .map_err(|e| e.to_string())
    }

    pub fn set_substrate_porosity(&mut self, local: usize, permille: u16) -> Result<(), String> {
        self.ground
            .set_substrate_porosity(local, permille)
            .map_err(|e| e.to_string())
    }

    pub fn local_index(global: usize) -> Option<usize> {
        let x = global % CW;
        let y = global / CW;
        if (ORIGIN[0]..ORIGIN[0] + WIDTH).contains(&x)
            && (ORIGIN[1]..ORIGIN[1] + HEIGHT).contains(&y)
        {
            Some((y - ORIGIN[1]) * WIDTH + x - ORIGIN[0])
        } else {
            None
        }
    }

    pub fn global_index(local: usize) -> Option<usize> {
        (local < WIDTH * HEIGHT)
            .then(|| (ORIGIN[1] + local / WIDTH) * CW + ORIGIN[0] + local % WIDTH)
    }

    /// Conservative adapter for managed soil. Edits are in global terrain-cell
    /// coordinates; edits outside this patch are rejected by this adapter.
    pub fn edit_beds_material(
        &mut self,
        edits: &[(usize, i32)],
    ) -> Result<crate::ground::GroundMaterialTransfer, String> {
        let local = edits
            .iter()
            .map(|&(global, height)| {
                Self::local_index(global)
                    .map(|i| (i, height))
                    .ok_or_else(|| "Cell outside managed river patch".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.ground
            .edit_beds_material(&mut self.water, &local)
            .map_err(|e| e.to_string())
    }

    pub fn exchange_foliage(
        &mut self,
        global: usize,
        removed: u32,
        grown: u32,
    ) -> Result<(), String> {
        let local = Self::local_index(global).ok_or("Cell outside managed river patch")?;
        self.ground
            .exchange_foliage(local, removed, grown)
            .map_err(|e| e.to_string())
    }

    pub fn validate_dam_change(&self) -> Result<(), String> {
        if self
            .dam_links
            .iter()
            .any(|&i| i >= self.water.links().len())
        {
            return Err("Dam contains an invalid hydraulic link".into());
        }
        Ok(())
    }

    pub fn apply_construction(
        &mut self,
        edits: &[(usize, i32)],
        removed_foliage: &[(usize, u32)],
    ) -> Result<crate::ground::GroundMaterialTransfer, String> {
        let local_edits = edits
            .iter()
            .map(|&(i, h)| {
                Self::local_index(i)
                    .map(|j| (j, h))
                    .ok_or_else(|| "Cell outside managed river patch".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let local_foliage = removed_foliage
            .iter()
            .map(|&(i, n)| {
                Self::local_index(i)
                    .map(|j| (j, n))
                    .ok_or_else(|| "Cell outside managed river patch".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.ground
            .edit_materials_and_foliage(&mut self.water, &local_edits, &local_foliage)
            .map_err(|e| e.to_string())
    }

    pub fn set_dam(&mut self, closed: bool) -> Result<(), String> {
        self.validate_dam_change()?;
        // Alter a zero-width hydraulic barrier, retaining every liter. Physical
        // construction materials/animal work are separate future mechanisms.
        for &link in &self.dam_links {
            self.water
                .set_barrier(
                    link,
                    if closed {
                        Some(Barrier {
                            crest_mm: self.crest_mm,
                            permeability_permille: 5,
                        })
                    } else {
                        None
                    },
                )
                .map_err(|e| e.to_string())?;
        }
        self.dam_closed = closed;
        Ok(())
    }

    pub fn enable_automated_check(&mut self) {
        self.automated_test = true;
    }

    pub fn advance_to(&mut self, tick: u64) -> Result<(), String> {
        if tick < self.last_processed_tick {
            return Err("Water cannot move backwards in time".into());
        }
        while self.last_processed_tick < tick {
            if self.automated_test {
                match self.last_processed_tick + 1 {
                    400 => {
                        self.set_dam(true)?;
                        crate::logging::log("RIVER_CHECK_ACTION", "close barrier at tick400");
                    }
                    1600 => {
                        self.set_dam(false)?;
                        crate::logging::log("RIVER_CHECK_ACTION", "breach barrier at tick1600");
                    }
                    _ => {}
                }
            }
            self.water.step().map_err(|e| e.to_string())?;
            self.ground
                .step(&mut self.water, self.last_processed_tick + 1)
                .map_err(|e| e.to_string())?;
            self.last_processed_tick += 1;
        }
        if !self.water.totals().balanced()
            || !self.ground.totals().balanced()
            || !self.ground.combined_water_balanced(&self.water)
        {
            return Err("Coupled water/soil/biomass ledger does not balance".into());
        }
        Ok(())
    }

    /// Bed elevations in the solver are a geometry cache of CPU terrain, not
    /// another editable master. Existing liters survive a cut or fill.
    pub fn refresh_geometry(&mut self, terrain: &Terrain) -> Result<(), String> {
        if terrain.revision() == self.terrain_revision {
            return Ok(());
        }
        for i in 0..WIDTH * HEIGHT {
            let global = (ORIGIN[1] + i / WIDTH) * CW + ORIGIN[0] + i % WIDTH;
            if terrain.elevation_mm(global).map_err(|e| e.to_string())?
                != self.ground.cells()[i].bed_mm
            {
                return Err("Managed river ground edits require a material transaction; refusing to overwrite finite soil stocks".into());
            }
        }
        self.terrain_revision = terrain.revision();
        Ok(())
    }

    pub fn sync_terrain(&mut self, terrain: &mut Terrain) -> Result<(), String> {
        self.flow_vectors.fill([0.; 2]);
        for (i, link) in self.water.links().iter().enumerate() {
            let axis = if link.b == link.a + 1 { 0 } else { 1 };
            let flux = self.water.last_link_flow_liters(i).unwrap_or(0) as f32 * 0.5;
            self.flow_vectors[link.a][axis] += flux;
            self.flow_vectors[link.b][axis] += flux;
        }
        for (i, cell) in self.water.cells().iter().enumerate() {
            let global = (ORIGIN[1] + i / WIDTH) * CW + ORIGIN[0] + i % WIDTH;
            let ground = &self.ground.cells()[i];
            terrain
                .set_elevation_mm(global, ground.bed_mm)
                .map_err(|e| e.to_string())?;
            terrain
                .apply_biological_cover(
                    global,
                    ground.foliage_units,
                    ground.moisture_permille,
                    ground.disturbed,
                )
                .map_err(|e| e.to_string())?;
            let depth = u32::try_from(cell.depth_mm())
                .map_err(|_| "Water depth exceeds terrain representation")?;
            terrain
                .set_water_depth_mm(global, depth)
                .map_err(|e| e.to_string())?;
            let velocity = self.flow_vectors[i].map(|flux| {
                if depth == 0 {
                    0.
                } else {
                    // Quantized, capped cosmetic advection; no simulation force.
                    ((flux * crate::clock::TACTICAL_HZ as f32
                        / (CELL_METERS as f32 * depth as f32))
                        .clamp(-20., 20.)
                        * 32.)
                        .round()
                        / 32.
                }
            });
            terrain
                .set_water_visual_flow(global, velocity)
                .map_err(|e| e.to_string())?;
        }
        self.terrain_revision = terrain.revision();
        Ok(())
    }

    pub fn describe(&self) -> serde_json::Value {
        let totals = self.water.totals();
        let upstream_liters: u128 = self.water.cells()[..(DAM_ROW + 1) * WIDTH]
            .iter()
            .map(|c| c.volume_liters as u128)
            .sum();
        let wet_cells = self
            .water
            .cells()
            .iter()
            .filter(|c| c.depth_mm() >= 10)
            .count();
        serde_json::json!({
            "fixture": "isolated_river_water_no_military_or_campaign_rules",
            "cells": WIDTH * HEIGHT, "links": self.water.links().len(),
            "sample_meters": CELL_METERS, "processed_through_game_tick": self.last_processed_tick,
            "dam_closed": self.dam_closed, "dam_crest_mm": self.crest_mm,
            "stored_liters": totals.stored_liters, "initial_liters": totals.initial_liters,
            "external_inflow_liters": totals.added_liters, "boundary_outflow_liters": totals.drained_liters,
            "water_extracted_to_other_stocks_liters": totals.extracted_liters,
            "water_returned_from_other_stocks_liters": totals.returned_liters,
            "upstream_liters": upstream_liters, "wet_cells_10mm_or_more": wet_cells,
            "conserved": totals.balanced() && self.ground.totals().balanced() && self.ground.combined_water_balanced(&self.water),
            "ground": self.ground.totals(),
            "rainfall_groundwater_ecosystem_biology_implemented": false
        })
    }
}
