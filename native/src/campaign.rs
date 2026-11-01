//! Connected native campaign. CPU owns rules; the GPU consumes display copies.
use crate::{
    campaign_support::{self, ContactCandidate, Recon, Sensor, SensorKind},
    climate::SeasonState,
    clock::{GameClock, TICK_NANOS},
    construction::{
        BuildKind, BuildOrder, ConstructionEvent, ConstructionWorld, Resources, TEAM_BASES,
        WorkerState,
    },
    ecology::{Ecology, EcologyEffect, EcologyPatch, Taxon},
    ecosystem_physics::EcosystemPhysics,
    gpu::{Agent, Object, grown_object, hud::HudModel},
    habitat::HabitatWorld,
    logging,
    river::RiverFixture,
    tactics::{MovementClass, NavGrid, Role, StructureTarget, TacticalEvent, Tactics},
    terrain::{
        CELL_METERS, CH, CW, DYNAMIC_WATER, SOIL_MOISTURE_MASK, SOIL_MOISTURE_SHIFT, Terrain,
        TerrainSnapshot,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
    path::Path,
    time::Duration,
};

pub const PATCH_WIDTH: usize = 30;
pub const PATCH_HEIGHT: usize = 20;
pub const PATCH_METERS: u32 = 160;
pub const DEFAULT_MILITARY: u32 = 10_000;
pub const DEFAULT_WORKERS: u32 = 2_000;
const SHIP_ID: u64 = 1_000_000_000;
const BUILD_TOOLS: [BuildKind; 19] = [
    BuildKind::Road,
    BuildKind::Trench,
    BuildKind::Foundation,
    BuildKind::Smooth,
    BuildKind::Depot,
    BuildKind::Mine,
    BuildKind::Mill,
    BuildKind::Barracks,
    BuildKind::Solar,
    BuildKind::Farm,
    BuildKind::Camera,
    BuildKind::Radar,
    BuildKind::Wall,
    BuildKind::Turret,
    BuildKind::Dam,
    BuildKind::Bridge,
    BuildKind::Dock,
    BuildKind::Pen,
    BuildKind::Research,
];
const ACTION_TOOLS: [&str; 11] = [
    "Inspect",
    "Capture",
    "Research",
    "Breed",
    "Release",
    "Rally",
    "Boat",
    "Transport",
    "Board",
    "Disembark",
    "Cancel order",
];

#[derive(Clone, Serialize, Deserialize)]
pub struct DetailedAnimal {
    pub position: [f32; 2],
    pub previous: [f32; 2],
    pub last_seen: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CaptureOrder {
    pub worker: u64,
    pub team: u8,
    pub species: u32,
    pub patch: u32,
    pub captured: Option<u64>,
    pub releasing: bool,
    pub picked_up: bool,
    pub pen: u64,
    pub cohort: u64,
    #[serde(default)]
    pub handling_work_ticks: u64,
    #[serde(default)]
    pub last_known_position: Option<[f32; 2]>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct CampaignData {
    pub seed: u32,
    pub clock: GameClock,
    pub processed_tick: u64,
    pub paused: bool,
    #[serde(default)]
    pub visual_memory: Option<crate::visual_memory::VisualMemory>,
    #[serde(default)]
    pub unit_history: BTreeMap<u64, std::collections::VecDeque<(u64, String)>>,
    pub camera: [f32; 3],
    pub selected_tool: usize,
    pub flash: f32,
    #[serde(default)]
    pub flash_position: Option<[f32; 2]>,
    pub status: String,
    pub river: RiverFixture,
    pub construction: ConstructionWorld,
    pub nav: NavGrid,
    pub tactics: Tactics,
    pub ecology: Ecology,
    pub habitats: HabitatWorld,
    pub physical: EcosystemPhysics,
    pub recon: Recon,
    pub support: crate::faction_support::FactionSupport,
    pub ship_health: [u32; 2],
    pub ecological_damage: [u64; 2],
    pub ecological_initial: u64,
    pub base_water: [u64; 2],
    pub initial_base_water: u64,
    pub water_given_to_care: u64,
    pub capture_orders: Vec<CaptureOrder>,
    pub selected_species: Option<u32>,
    pub detailed: BTreeMap<u64, DetailedAnimal>,
    #[serde(default)]
    pub unit_order_queues:
        BTreeMap<u64, std::collections::VecDeque<crate::campaign_commands::PendingUnitOrder>>,
    pub last_cover: Vec<u16>,
    pub ground_cover_retained: Vec<u16>,
    pub rain_remainders: Vec<u64>,
    pub managed_per_patch: Vec<u32>,
    pub last_construction_revision: u64,
    pub winner: Option<u8>,
    pub outcome: String,
    pub automated: bool,
    pub next_enemy_order: u64,
    pub completed_jobs: u64,
    pub biological_events: u64,
    pub last_combat_tick: u64,
    pub blast_spoil_liters: [u64; 2],
    pub ore_delivered: [u64; 2],
    pub flow_boundaries: Vec<(usize, usize, usize)>,
    pub flow_outgoing: Vec<[u64; 4]>,
    pub flow_reference: Vec<u64>,
    pub initial_bio_per_patch: Vec<u64>,
    pub initial_cover_per_patch: Vec<u64>,
    pub last_ground_foliage: Vec<u32>,
    pub loss_projection_remainder: Vec<u64>,
    pub last_river_bio: Vec<u16>,
    pub river_growth_credit: Vec<u64>,
    pub initial_managed_cover_per_patch: Vec<u64>,
}
#[derive(Serialize, Deserialize)]
pub struct CampaignSave {
    pub version: u32,
    pub terrain: TerrainSnapshot,
    pub data: CampaignData,
}
#[derive(bevy::prelude::Resource)]
pub struct Campaign {
    pub terrain: Terrain,
    pub data: CampaignData,
}
impl Deref for Campaign {
    type Target = CampaignData;
    fn deref(&self) -> &CampaignData {
        &self.data
    }
}
impl DerefMut for Campaign {
    fn deref_mut(&mut self) -> &mut CampaignData {
        &mut self.data
    }
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn mix_hash(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58476d1ce4e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
pub fn patch_at(position: [f32; 2]) -> usize {
    ((position[1].max(0.) as usize / PATCH_METERS as usize).min(PATCH_HEIGHT - 1)) * PATCH_WIDTH
        + (position[0].max(0.) as usize / PATCH_METERS as usize).min(PATCH_WIDTH - 1)
}
pub fn patch_center(patch: usize) -> [f32; 2] {
    [
        ((patch % PATCH_WIDTH) as u32 * PATCH_METERS + PATCH_METERS / 2) as f32,
        ((patch / PATCH_WIDTH) as u32 * PATCH_METERS + PATCH_METERS / 2) as f32,
    ]
}
impl Campaign {
    pub fn new(seed: u32, military: u32, workers: u32) -> Result<Self, String> {
        Self::new_from_terrain(Terrain::new(seed), military, workers)
    }
    /// Named initial stress workload only. Existing organisms are never
    /// multiplied during a running world; normal creation limits stay intact.
    pub fn new_for_benchmark(seed: u32, military: u32, workers: u32) -> Result<Self, String> {
        Self::new_with_limits(Terrain::new(seed), military, workers, true, false, false)
    }
    /// Named diagnostic initial deployment, not the ordinary crashed-ship opening.
    /// Only military spawn origins change, before the clock starts; every pawn,
    /// weapon, worker, terrain sample and finite salvage stock is retained.
    pub fn new_battle_benchmark(seed: u32, military: u32, workers: u32) -> Result<Self, String> {
        Self::new_with_limits(Terrain::new(seed), military, workers, true, true, false)
    }
    pub fn new_from_terrain(terrain: Terrain, military: u32, workers: u32) -> Result<Self, String> {
        Self::new_with_limits(terrain, military, workers, false, false, false)
    }
    /// Player opening; full-count constructors remain independent fixtures.
    pub fn new_crash_landing(terrain: Terrain) -> Result<Self, String> {
        Self::new_with_limits(terrain, 8, 12, false, false, true)
    }
    fn new_with_limits(
        mut terrain: Terrain,
        military: u32,
        workers: u32,
        benchmark: bool,
        staged_frontline_clash: bool,
        opening: bool,
    ) -> Result<Self, String> {
        let (military_limit, worker_limit) = if benchmark {
            (200_000, 40_000)
        } else {
            (50_000, 20_000)
        };
        if military < 2 || military > military_limit || workers < 2 || workers > worker_limit {
            return Err(format!(
                "Explicit campaign counts outside {} creation limits military2..{} / workers2..{}; no counts were reduced",
                if benchmark { "benchmark" } else { "normal" },
                military_limit,
                worker_limit
            ));
        }
        let seed = terrain.seed;
        for i in 0..CW * CH {
            let c = terrain.cells[i];
            if c.flags & DYNAMIC_WATER == 0 {
                terrain
                    .set_water_depth_mm(
                        i,
                        c.water
                            .checked_mul(10)
                            .ok_or("Generated water depth overflow")?,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        let river = RiverFixture::new(&mut terrain)?;
        let nav = NavGrid::from_terrain(&terrain)?;
        let mut tactics = Tactics::new(seed);
        for team in 0..2u8 {
            let count = (military / 2 + u32::from(team == 0 && military % 2 != 0)) as usize;
            // A real forward detachment is part of the selected army, not an extra spawn.
            let forward = if opening { 0 } else { count.min(24) };
            let guards = if opening { count } else { (count - forward).min(24) };
            let bulk = count - forward - guards;
            let base = TEAM_BASES[team as usize];
            let inward = if team == 0 { 1. } else { -1. };
            // Keep the ship's pickup and worker corridor outside a parked army.
            // Every guard/staged/front soldier is drawn from this same count.
            let staging_offset = 160. + (bulk as f32).sqrt() * 5.;
            let center = if team == 0 {
                [2160., 1580.]
            } else {
                [2250., 1580.]
            };
            tactics.spawn_formation(
                team,
                Role::Soldier,
                if staged_frontline_clash {
                    [[1750., 1580.], [2666., 1580.]][team as usize]
                } else {
                    [base[0] + inward * staging_offset, base[1]]
                },
                bulk,
                &nav,
            )?;
            tactics.spawn_formation(
                team,
                Role::Soldier,
                [base[0] + inward * 96., base[1] - 100.],
                guards,
                &nav,
            )?;
            tactics.spawn_formation(team, Role::Soldier, center, forward, &nav)?;
            if opening {
                tactics.spawn_formation(team, Role::Scout, [base[0]+inward*32., base[1]+32.], 1, &nav)?;
            }
            tactics.spawn_formation(
                team,
                Role::Worker,
                [
                    TEAM_BASES[team as usize][0],
                    TEAM_BASES[team as usize][1] + 80.,
                ],
                (workers / 2 + u32::from(team == 0 && workers % 2 != 0)) as usize,
                &nav,
            )?;
        }
        let mut ecology = Ecology::new(seed, PATCH_WIDTH * PATCH_HEIGHT)?;
        ecology.set_patch_width(PATCH_WIDTH)?;
        let ecological_initial = ecology.census().biomass_units;
        let mut managed_per_patch = vec![0; PATCH_WIDTH * PATCH_HEIGHT];
        for local in 0..river.water.cells().len() {
            let global = RiverFixture::global_index(local).ok_or("Invalid managed map")?;
            let p = patch_at([
                ((global % CW) * CELL_METERS) as f32,
                ((global / CW) * CELL_METERS) as f32,
            ]);
            managed_per_patch[p] += 1;
        }
        let mut initial_bio_per_patch = vec![0u64; 600];
        for plant in &ecology.plants {
            initial_bio_per_patch[plant.patch as usize] += plant.live_units();
        }
        let mut initial_cover_per_patch = vec![0u64; 600];
        for cell in 0..CW * CH {
            let p = patch_at([((cell % CW) * 8) as f32, ((cell / CW) * 8) as f32]);
            initial_cover_per_patch[p] += terrain.initial_foliage_units(cell).unwrap_or(0) as u64;
        }
        let last_ground_foliage = river
            .ground
            .cells()
            .iter()
            .map(|c| c.foliage_units)
            .collect();
        let mut initial_managed_cover_per_patch = vec![0u64; 600];
        for (local, c) in river.ground.cells().iter().enumerate() {
            let g = RiverFixture::global_index(local).ok_or("Cover mapping")?;
            initial_managed_cover_per_patch
                [patch_at([((g % CW) * 8) as f32, ((g / CW) * 8) as f32])] +=
                c.foliage_units as u64;
        }
        let flow_boundaries = river
            .water
            .links()
            .iter()
            .enumerate()
            .filter_map(|(link, e)| {
                let a = RiverFixture::global_index(e.a)?;
                let b = RiverFixture::global_index(e.b)?;
                let pa = patch_at([((a % CW) * 8) as f32, ((a / CW) * 8) as f32]);
                let pb = patch_at([((b % CW) * 8) as f32, ((b / CW) * 8) as f32]);
                (pa != pb).then_some((link, pa, pb))
            })
            .collect();
        let mut campaign = Self {
            terrain,
            data: CampaignData {
                seed,
                clock: GameClock::default(),
                processed_tick: 0,
                paused: opening,
                visual_memory: None,
                unit_history: BTreeMap::new(),
                camera: [424., 1640., 0.85],
                selected_tool: 19,
                flash: -1.,
                flash_position: None,
                status: "Ready. Box-select troops; B opens construction and E opens ecology."
                    .into(),
                river,
                construction: ConstructionWorld::new(seed),
                nav,
                tactics,
                ecology,
                habitats: HabitatWorld::default(),
                physical: EcosystemPhysics::new(PATCH_WIDTH * PATCH_HEIGHT),
                support: crate::faction_support::FactionSupport::new(),
                recon: Recon::new(150, 100, 32.)?,
                ship_health: [10_000; 2],
                ecological_damage: [0; 2],
                ecological_initial,
                base_water: [100_000; 2],
                initial_base_water: 200_000,
                water_given_to_care: 0,
                capture_orders: Vec::new(),
                unit_order_queues: BTreeMap::new(),
                selected_species: None,
                detailed: BTreeMap::new(),
                last_cover: vec![32; PATCH_WIDTH * PATCH_HEIGHT],
                ground_cover_retained: vec![1000; PATCH_WIDTH * PATCH_HEIGHT],
                rain_remainders: vec![0; PATCH_WIDTH * PATCH_HEIGHT + 12288],
                managed_per_patch,
                last_construction_revision: 0,
                winner: None,
                outcome: String::new(),
                automated: false,
                next_enemy_order: 600,
                completed_jobs: 0,
                biological_events: 0,
                last_combat_tick: 0,
                blast_spoil_liters: [0; 2],
                ore_delivered: [0; 2],
                flow_boundaries,
                flow_outgoing: vec![[0; 4]; 600],
                flow_reference: vec![0; 600],
                initial_bio_per_patch,
                initial_cover_per_patch,
                last_ground_foliage,
                loss_projection_remainder: vec![0; 600],
                last_river_bio: vec![1000; 600],
                river_growth_credit: vec![0; 600],
                initial_managed_cover_per_patch,
            },
        };
        let initial_environment = campaign.environment(0)?;
        campaign
            .data
            .ecology
            .accept_initial_habitat(&initial_environment)?;
        campaign.update_ground_conditions()?;
        campaign.rebuild_structures()?;
        if opening {
            // Place the moving footprint over the landing region at tick zero.
            campaign.recon.satellite.team_phase_offsets[0] = 164;
            campaign.data.visual_memory = Some(crate::visual_memory::VisualMemory::survey(&campaign.terrain));
            campaign.status = "Landing secured. Select builders to plan construction; defenders protect the ship; camera trooper extends video.".into();
        }
        campaign.recon_update(0)?;
        campaign.refresh_observation(0)?;
        campaign.validate()?;
        logging::log("CAMPAIGN_NEW",&serde_json::json!({"seed":seed,"military":military,"workers":workers,"map_meters":[4800,3200],"ecology":campaign.ecology.census(),"scope":"Milestone B connected small map; production20km and remaining130traits in C"}).to_string());
        Ok(campaign)
    }
    pub fn advance(&mut self, elapsed: Duration) -> Result<u32, String> {
        self.advance_with_rate(elapsed, None, false, None)
            .map(|(steps, _)| steps)
    }
    /// Explicit stress entry point; ordinary game speed remains 1/2/4.
    /// Authorize real elapsed time once, then execute the same fixed rules.
    pub fn advance_benchmark(
        &mut self,
        elapsed: Duration,
        requested_rate: u32,
    ) -> Result<u32, String> {
        self.advance_with_rate(elapsed, Some(requested_rate), false, None)
            .map(|(steps, _)| steps)
    }
    /// Actual rule-step timings, bounded stack storage, benchmark use only.
    pub fn advance_benchmark_profiled(
        &mut self,
        elapsed: Duration,
        requested_rate: u32,
    ) -> Result<(u32, [u64; 64]), String> {
        self.advance_with_rate(elapsed, Some(requested_rate), true, None)
    }
    /// Stop at a command boundary without dropping any elapsed-time debt.
    /// The ordinary benchmark and game call budgets stay unchanged.
    pub(crate) fn advance_benchmark_profiled_budgeted(
        &mut self,
        elapsed: Duration,
        requested_rate: u32,
        maximum: u32,
    ) -> Result<(u32, [u64; 64]), String> {
        if !(1..=crate::campaign_benchmark::BENCHMARK_STEPS_PER_CALL).contains(&maximum) {
            return Err("Invalid bounded benchmark tick budget".into());
        }
        self.advance_with_rate(elapsed, Some(requested_rate), true, Some(maximum))
    }
    fn advance_with_rate(
        &mut self,
        elapsed: Duration,
        benchmark_rate: Option<u32>,
        profiled: bool,
        maximum_override: Option<u32>,
    ) -> Result<(u32, [u64; 64]), String> {
        let maximum = maximum_override.unwrap_or_else(|| {
            if benchmark_rate.is_some() {
                crate::campaign_benchmark::BENCHMARK_STEPS_PER_CALL
            } else if self.data.automated {
                2400u64.saturating_sub(self.data.clock.tick()).min(16) as u32
            } else {
                16
            }
        });
        let paused = self.data.paused || self.data.winner.is_some();
        if let Some(rate) = benchmark_rate {
            self.data.clock.advance_benchmark(elapsed, paused, 0, rate)
        } else {
            self.data.clock.advance(elapsed, paused, 0)
        }
        .map_err(|e| e.to_string())?;
        let mut committed = 0;
        let mut tick_nanos = [0u64; 64];
        for _ in 0..maximum {
            let paused = self.data.paused || self.data.winner.is_some();
            if self
                .data
                .clock
                .advance(Duration::ZERO, paused, 1)
                .map_err(|e| e.to_string())?
                == 0
            {
                break;
            }
            let began = profiled.then(std::time::Instant::now);
            self.step()?;
            if let Some(began) = began {
                tick_nanos[committed as usize] =
                    u64::try_from(began.elapsed().as_nanos()).unwrap_or(u64::MAX);
            }
            committed += 1;
        }
        Ok((committed, tick_nanos))
    }
    pub fn step(&mut self) -> Result<(), String> {
        let tick = self
            .data
            .processed_tick
            .checked_add(1)
            .ok_or("Campaign tick exhausted")?;
        if tick > self.data.clock.tick() {
            return Err("Campaign tick was not authorized by GameClock".into());
        }
        if tick % 20 == 1 {
            self.apply_weather(tick)?;
            self.update_ground_conditions()?;
        }
        self.data.river.advance_to(tick)?;
        self.accumulate_flow(tick)?;
        self.project_water_cover_loss()?;
        self.data.river.sync_terrain(&mut self.terrain)?;
        self.data.nav.refresh(&self.terrain)?;
        self.apply_navigation_structures()?;
        let workers = self.worker_states();
        let capture_workers: BTreeSet<u64> =
            self.data.capture_orders.iter().map(|o| o.worker).collect();
        let suspended: BTreeSet<u32> = capture_workers
            .iter()
            .chain(self.data.unit_order_queues.keys())
            .filter_map(|id| u32::try_from(*id).ok())
            .collect();
        let tasks = self
            .data
            .construction
            .worker_tasks_available(&workers, &suspended)?;
        for task in tasks {
            if !capture_workers.contains(&(task.worker_id as u64))
                && !self
                    .data
                    .unit_order_queues
                    .contains_key(&(task.worker_id as u64))
            {
                if let Err(error) = self.data.tactics.assign_on_grid_within(
                    task.worker_id as u64,
                    task.destination,
                    task.job_id,
                    &self.data.nav,
                    2.,
                ) {
                    if tick % 200 == 0 {
                        logging::log(
                            "WORKER_ROUTE_WAIT",
                            &format!("worker{}: {}", task.worker_id, error),
                        );
                    }
                }
            }
        }
        self.update_capture_orders()?;
        self.process_unit_queues(tick)?;
        let events = self.data.tactics.step(tick, &self.data.nav)?;
        self.tactical_events(events, tick)?;
        let workers = self.worker_states();
        let events = self.data.construction.step_available(
            tick,
            &mut self.terrain,
            Some(&mut self.data.river),
            &workers,
            &suspended,
        )?;
        let construction_changed = !events.is_empty();
        for event in events {
            self.construction_event(event)?;
        }
        if construction_changed {
            self.data
                .physical
                .synchronize_hosts(&mut self.data.ecology, &mut self.data.habitats)?;
        }
        if self.data.construction.navigation_revision != self.data.last_construction_revision {
            self.rebuild_structures()?;
        }
        self.data.support.step(
            tick,
            &self.data.construction,
            &mut self.data.tactics,
            &self.data.nav,
        )?;
        for shot in std::mem::take(&mut self.data.support.recent_shots) {
            self.data.last_combat_tick = tick;
            let patch = patch_at(shot.target_position);
            self.data.ecology.alert_threat(patch as u32, 200)?;
            let harm = self.data.ecology.damage_plants(patch as u32, 2, false)?;
            self.data.ecological_damage[shot.team as usize] = self.data.ecological_damage
                [shot.team as usize]
                .checked_add(harm.plant_units_removed)
                .ok_or("Turret collateral accounting overflow")?;
        }
        if tick % 20 == 0 {
            // Erosion can lower a roof as well as a deliberate excavation. Only
            // sparse, surviving underground sites need a surface-height check.
            let roof_cells: BTreeSet<usize> = self
                .data
                .habitats
                .nodes
                .values()
                .filter(|node| !node.collapsed)
                .filter_map(|node| {
                    let x = node.position.x.div_euclid(8000);
                    let y = node.position.y.div_euclid(8000);
                    (x >= 0 && y >= 0 && x < CW as i32 && y < CH as i32)
                        .then_some(y.max(0) as usize * CW + x.max(0) as usize)
                })
                .collect();
            let roof_edits = roof_cells
                .into_iter()
                .map(|cell| {
                    self.terrain
                        .elevation_mm(cell)
                        .map(|height| (cell, height))
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.reconcile_surface_habitat_edits(&roof_edits)?;
            self.data
                .physical
                .synchronize_hosts(&mut self.data.ecology, &mut self.data.habitats)?;
            let environment = self.environment(tick)?;
            let effects = self.data.ecology.step(tick, &environment)?;
            self.data.biological_events = self
                .data
                .biological_events
                .checked_add(effects.len() as u64)
                .ok_or("Ecology event counter overflow")?;
            let requests = effects
                .into_iter()
                .filter_map(|e| {
                    if let EcologyEffect::Engineering(r) = e {
                        Some(r)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            self.data.physical.step(
                tick,
                &requests,
                &mut self.data.ecology,
                &mut self.data.habitats,
                &mut self.terrain,
                &mut self.data.river,
                &environment,
                PATCH_WIDTH,
                PATCH_METERS,
            )?;
            self.project_biology_to_river(tick)?;
            self.update_biological_cover(&environment)?;
            self.production_and_care(tick)?;
            self.data.flow_outgoing.fill([0; 4]);
        }
        self.recon_update(tick)?;
        if tick % 20 == 0 {
            self.refresh_observation(tick)?;
        }
        self.move_detailed(tick)?;
        if tick >= self.data.next_enemy_order {
            self.enemy_order(tick)?;
            self.data.next_enemy_order = tick.checked_add(600).ok_or("Enemy schedule overflow")?;
        }
        self.check_outcome();
        self.data.processed_tick = tick;
        for (slot, cell) in self
            .data
            .last_ground_foliage
            .iter_mut()
            .zip(self.data.river.ground.cells())
        {
            *slot = cell.foliage_units;
        }
        if tick % 200 == 0 {
            self.validate()?;
            logging::log("CAMPAIGN", &self.report().to_string());
        }
        Ok(())
    }
    fn worker_states(&self) -> Vec<WorkerState> {
        self.data
            .tactics
            .agents
            .iter()
            .filter(|a| a.alive() && a.role == Role::Worker && a.embarked_on.is_none())
            .filter_map(|a| {
                u32::try_from(a.id).ok().map(|id| WorkerState {
                    id,
                    team: a.team,
                    position: a.position(),
                })
            })
            .collect()
    }
    fn update_ground_conditions(&mut self) -> Result<(), String> {
        let conditions = (0..PATCH_WIDTH * PATCH_HEIGHT)
            .map(|p| {
                [
                    self.data.ecology.root_reinforcement_permille(p as u32),
                    self.data.ecology.ground_cover_permille(p as u32),
                    self.data.ecology.canopy_permille(p as u32),
                ]
            })
            .collect::<Vec<_>>();
        for local in 0..self.data.river.ground.cells().len() {
            let global = RiverFixture::global_index(local).ok_or("Living-ground mapping")?;
            let p = patch_at([
                ((global % CW) * CELL_METERS) as f32,
                ((global / CW) * CELL_METERS) as f32,
            ]);
            let [root, cover, shade] = conditions[p];
            self.data
                .river
                .ground
                .set_biological_conditions(local, root, cover, shade)
                .map_err(|e| e.to_string())?;
            self.data
                .river
                .ground
                .set_substrate_porosity(
                    local,
                    self.data.physical.patches[p].upper.porosity_permille,
                )
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn account_cover_loss(
        &mut self,
        patch: usize,
        raw_units: u64,
        team: Option<u8>,
    ) -> Result<(), String> {
        let denominator = self.data.initial_cover_per_patch[patch];
        if denominator == 0 || raw_units == 0 {
            return Ok(());
        }
        let numerator = (self.data.initial_bio_per_patch[patch] as u128)
            .checked_mul(raw_units as u128)
            .and_then(|n| n.checked_add(self.data.loss_projection_remainder[patch] as u128))
            .ok_or("Cover projection overflow")?;
        let units = u64::try_from(numerator / denominator as u128)
            .map_err(|_| "Biological projection overflow")?;
        self.data.loss_projection_remainder[patch] = (numerator % denominator as u128) as u64;
        if units > 0 {
            let damage = self.data.ecology.damage_plants(patch as u32, units, true)?;
            if let Some(team) = team {
                self.data.ecological_damage[team as usize] = self.data.ecological_damage
                    [team as usize]
                    .checked_add(damage.plant_units_removed)
                    .ok_or("Damage projection overflow")?;
            }
        }
        Ok(())
    }
    fn project_water_cover_loss(&mut self) -> Result<(), String> {
        let mut lost = vec![0u64; 600];
        for (local, cell) in self.data.river.ground.cells().iter().enumerate() {
            let before = self.data.last_ground_foliage[local];
            if before > cell.foliage_units {
                let g = RiverFixture::global_index(local).ok_or("Cover loss mapping")?;
                let p = patch_at([((g % CW) * 8) as f32, ((g / CW) * 8) as f32]);
                lost[p] = lost[p]
                    .checked_add((before - cell.foliage_units) as u64)
                    .ok_or("Cover loss overflow")?;
            }
        }
        for (p, amount) in lost.into_iter().enumerate() {
            self.account_cover_loss(p, amount, None)?;
        }
        Ok(())
    }
    fn project_biology_to_river(&mut self, tick: u64) -> Result<(), String> {
        let mut fraction = vec![0; 600];
        for p in 0..600 {
            fraction[p] = self.data.ecology.plant_fraction_permille(p as u32);
            let before = self.data.last_river_bio[p];
            if fraction[p] < before {
                self.data.river_growth_credit[p] = 0;
            } else if fraction[p] > before {
                let credit = self.data.initial_managed_cover_per_patch[p]
                    * (fraction[p] - before) as u64
                    / 1000;
                self.data.river_growth_credit[p] = self.data.river_growth_credit[p]
                    .checked_add(credit)
                    .ok_or("Recovery projection overflow")?;
            }
        }
        for local in 0..self.data.river.ground.cells().len() {
            let g = RiverFixture::global_index(local).ok_or("Recovery mapping")?;
            let p = patch_at([((g % CW) * 8) as f32, ((g / CW) * 8) as f32]);
            let cell = self.data.river.ground.cells()[local];
            let before = self.data.last_river_bio[p];
            let after = fraction[p];
            if after < before && before > 0 {
                let remaining = cell.foliage_units as u64 * after as u64 / before as u64;
                let removed = cell.foliage_units - remaining as u32;
                if removed > 0 {
                    self.data.river.exchange_foliage(g, removed, 0)?;
                }
            } else if self.data.river_growth_credit[p] > 0
                && cell
                    .exposed_since
                    .is_some_and(|at| tick.saturating_sub(at) >= 1200)
                && cell.moisture_permille >= 200
                && self.data.ecology.plants.iter().any(|plant| {
                    plant.patch as usize == p && plant.individuals > 0 && plant.roots > 0
                })
            {
                let initial = self
                    .terrain
                    .initial_foliage_units(g)
                    .ok_or("Recovery capacity")?;
                let amount = self.data.river_growth_credit[p]
                    .min(initial.saturating_sub(cell.foliage_units) as u64)
                    .min(4) as u32;
                if amount > 0 {
                    self.data.river.exchange_foliage(g, 0, amount)?;
                    self.data.river_growth_credit[p] -= amount as u64;
                }
            }
        }
        self.data.last_river_bio = fraction;
        self.data.river.sync_terrain(&mut self.terrain)?;
        Ok(())
    }
    fn apply_weather(&mut self, tick: u64) -> Result<(), String> {
        let global = SeasonState::at_tick(tick, self.data.seed, 0);
        self.data
            .river
            .set_climate(global.evaporation_permille, global.infiltration_permille)?;
        for p in 0..PATCH_WIDTH * PATCH_HEIGHT {
            let climate = SeasonState::at_tick(tick, self.data.seed, p as u32);
            let nonmanaged = (PATCH_METERS as u64 * PATCH_METERS as u64)
                .checked_sub(self.data.managed_per_patch[p] as u64 * 64)
                .ok_or("Rain area overlap")?;
            let units = climate.rainfall_liters_per_patch_second as u64 * nonmanaged
                + self.data.rain_remainders[p];
            let rain = units / 256;
            self.data.rain_remainders[p] = units % 256;
            self.data.ecology.add_rain(p as u32, rain)?;
        }
        for local in 0..self.data.river.water.cells().len() {
            let global = RiverFixture::global_index(local).ok_or("Rain cell mapping")?;
            let p = patch_at([((global % CW) * 8) as f32, ((global / CW) * 8) as f32]);
            let climate = SeasonState::at_tick(tick, self.data.seed, p as u32);
            let r = PATCH_WIDTH * PATCH_HEIGHT + local;
            let units =
                climate.rainfall_liters_per_patch_second as u64 * 64 + self.data.rain_remainders[r];
            self.data.rain_remainders[r] = units % 256;
            self.data
                .river
                .water
                .add_boundary_water(local, units / 256)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn accumulate_flow(&mut self, tick: u64) -> Result<(), String> {
        if tick % 20 == 1 {
            self.data.flow_reference.fill(0);
            for (local, volume) in self
                .data
                .river
                .water
                .last_transport_volumes()
                .iter()
                .enumerate()
            {
                let g = RiverFixture::global_index(local).ok_or("Flow mapping")?;
                let p = patch_at([((g % CW) * 8) as f32, ((g / CW) * 8) as f32]);
                self.data.flow_reference[p] = self.data.flow_reference[p]
                    .checked_add(*volume)
                    .ok_or("Flow volume overflow")?;
            }
        }
        for (link, a, b) in &self.data.flow_boundaries {
            let q = self
                .data
                .river
                .water
                .last_link_flow_liters(*link)
                .ok_or("Flow link missing")?;
            if q == 0 {
                continue;
            }
            let (source, target) = if q > 0 { (*a, *b) } else { (*b, *a) };
            let direction = if target == source + 1 {
                0
            } else if source == target + 1 {
                1
            } else if target == source + PATCH_WIDTH {
                2
            } else {
                3
            };
            let v = u64::try_from(q.unsigned_abs()).map_err(|_| "Flow overflow")?;
            self.data.flow_outgoing[source][direction] = self.data.flow_outgoing[source][direction]
                .checked_add(v)
                .ok_or("Accumulated flow overflow")?;
        }
        Ok(())
    }
    fn flow_destination(&self, p: usize) -> Option<(usize, u64)> {
        let (d, v) = self.data.flow_outgoing[p]
            .iter()
            .enumerate()
            .max_by_key(|(_, v)| **v)?;
        if *v == 0 {
            return None;
        }
        let to = match d {
            0 => p + 1,
            1 => p.checked_sub(1)?,
            2 => p + PATCH_WIDTH,
            _ => p.checked_sub(PATCH_WIDTH)?,
        };
        (to < 600).then_some((to, *v))
    }
    pub fn environment(&self, tick: u64) -> Result<Vec<EcologyPatch>, String> {
        let mut env = Vec::with_capacity(PATCH_WIDTH * PATCH_HEIGHT);
        let connections = crate::ecology_navigation::macro_connections(
            &self.data.nav,
            PATCH_WIDTH,
            PATCH_HEIGHT,
        )?;
        for p in 0..PATCH_WIDTH * PATCH_HEIGHT {
            let climate = SeasonState::at_tick(tick, self.data.seed, p as u32);
            let center = patch_center(p);
            let cx = center[0] as usize / 8;
            let cy = center[1] as usize / 8;
            let mut depth_sum = 0u64;
            let mut disturbance = 0;
            let mut moisture = 0u64;
            let mut samples = 0u64;
            for y in cy.saturating_sub(10)..(cy + 10).min(CH) {
                for x in cx.saturating_sub(10)..(cx + 10).min(CW) {
                    let c = self.terrain.cells[y * CW + x];
                    let d = if c.flags & DYNAMIC_WATER != 0 {
                        c.water
                    } else {
                        c.water * 10
                    };
                    depth_sum += d as u64;
                    moisture += ((c.flags & SOIL_MOISTURE_MASK) >> SOIL_MOISTURE_SHIFT) as u64;
                    samples += 1;
                    if c.flags & 4 != 0 {
                        disturbance += 1;
                    }
                }
            }
            // Root-zone reserves are the biological owner. Conditions never copy those liters.
            let water = self.data.ecology.root_zones[p].shallow_water;
            let root_moisture = (water / 20).min(1000) as u16;
            env.push(EcologyPatch {
                water_depth_mm: (depth_sum / samples.max(1)) as u32,
                moisture_permille: root_moisture.max((moisture / samples.max(1)) as u16),
                temperature_milli_c: climate.temperature_milli_c,
                disturbance,
                accessible: true,
                land_connections: connections[p][0],
                water_connections: connections[p][1],
                shelter_capacity: self.data.habitats.shelter_capacity(p as u32),
                canopy_connected: self.data.ecology.canopy_permille(p as u32) > 100,
                flow_to: self.flow_destination(p).map(|(to, _)| to as u32),
                flow_fraction_permille: self.flow_destination(p).map_or(0, |(_, v)| {
                    ((v as u128 * 1000 / self.data.flow_reference[p].max(1) as u128).min(1000))
                        as u16
                }),
            });
        }
        Ok(env)
    }
    fn apply_navigation_structures(&mut self) -> Result<(), String> {
        let mut bridges = Vec::new();
        for b in self
            .data
            .construction
            .buildings
            .iter()
            .filter(|b| b.complete && b.health > 0 && b.kind == BuildKind::Bridge)
        {
            let n = (distance(b.position, b.end) / 8.).ceil().max(1.) as usize;
            for j in 0..=n {
                let f = j as f32 / n as f32;
                let p = [
                    b.position[0] + (b.end[0] - b.position[0]) * f,
                    b.position[1] + (b.end[1] - b.position[1]) * f,
                ];
                if let (Some(i), Some(deck)) = (self.data.nav.index(p), b.deck_height_mm) {
                    bridges.push((i, deck, deck - 500));
                }
            }
        }
        self.data.nav.replace_bridges(&bridges)?;
        let mut reefs = Vec::new();
        for reef in self.data.habitats.reefs.values() {
            if let Some(i) = self
                .data
                .physical
                .reef_sites
                .get(&reef.id)
                .and_then(|cell| {
                    self.data
                        .nav
                        .index([((*cell % CW) * 8 + 4) as f32, ((*cell / CW) * 8 + 4) as f32])
                })
            {
                reefs.push((i, reef.solid_height_mm));
            }
        }
        self.data.nav.replace_reefs(&reefs)?;
        Ok(())
    }
    fn rebuild_structures(&mut self) -> Result<(), String> {
        for i in 0..CW * CH {
            if self.terrain.cells[i].object != 0 {
                self.terrain
                    .set_occupancy(i, 0)
                    .map_err(|e| e.to_string())?;
            }
        }
        let mut targets = Vec::new();
        for team in 0..2 {
            targets.push(StructureTarget {
                id: SHIP_ID + team,
                team: team as u8,
                position_mm: [
                    (TEAM_BASES[team as usize][0] * 1000.) as i32,
                    (TEAM_BASES[team as usize][1] * 1000.) as i32,
                ],
                active: self.data.ship_health[team as usize] > 0,
            });
        }
        let mut object_id = 3u32;
        for b in &self.data.construction.buildings {
            if b.health == 0 {
                continue;
            }
            if b.kind.solid() {
                let order = BuildOrder {
                    team: b.team,
                    kind: b.kind,
                    position: b.position,
                    end: b.end,
                };
                for cell in ConstructionWorld::footprint_cells(order) {
                    self.terrain
                        .set_occupancy(cell, object_id)
                        .map_err(|e| e.to_string())?;
                }
            }
            targets.push(StructureTarget {
                id: b.id,
                team: b.team,
                position_mm: [
                    (b.position[0] * 1000.) as i32,
                    (b.position[1] * 1000.) as i32,
                ],
                active: b.health > 0,
            });
            object_id += 1;
        }
        for team in 0..2u8 {
            let housing = self
                .data
                .construction
                .buildings
                .iter()
                .filter(|b| {
                    b.team == team && b.kind == BuildKind::Pen && b.health > 0 && b.complete
                })
                .map(|b| crate::ecology::Housing {
                    id: b.id,
                    patch: patch_at(b.position) as u32,
                    capacity: 16,
                })
                .collect();
            self.data.ecology.set_housing(team, housing)?;
        }
        self.data.tactics.set_structure_targets(targets)?;
        self.data.last_construction_revision = self.data.construction.navigation_revision;
        self.data.nav.refresh(&self.terrain)?;
        self.apply_navigation_structures()?;
        Ok(())
    }
    pub fn record_unit_event(&mut self,id:u64,text:&str) {
        if !self.data.tactics.agent(id).is_some_and(|a|a.team==0){return;}
        let tick=self.data.clock.tick();
        let events=self.data.unit_history.entry(id).or_default();
        if events.back().is_some_and(|(t,s)|*t==tick&&s==text){return;}
        if events.len()==16{events.pop_front();}
        events.push_back((tick,text.into()));
    }
    fn tactical_events(&mut self, events: Vec<TacticalEvent>, tick: u64) -> Result<(), String> {
        for event in events {
            match event {
                TacticalEvent::StructureHit {
                    source,
                    structure,
                    damage,
                } => {
                    let team = self
                        .data
                        .tactics
                        .agent(source)
                        .ok_or("Unknown shooter")?
                        .team;
                    if structure >= SHIP_ID && structure < SHIP_ID + 2 {
                        let target = (structure - SHIP_ID) as usize;
                        self.data.ship_health[target] =
                            self.data.ship_health[target].saturating_sub(damage as u32);
                    } else {
                        let dam = self
                            .data
                            .construction
                            .buildings
                            .iter()
                            .find(|b| b.id == structure)
                            .is_some_and(|b| b.kind == BuildKind::Dam);
                        if self
                            .data
                            .construction
                            .damage_building(structure, damage as u32)?
                            && dam
                        {
                            self.data.river.set_dam(false)?;
                        }
                    }
                    self.data.last_combat_tick = tick;
                    let _ = team;
                }
                TacticalEvent::Shot { source, target, .. } => {
                    self.record_unit_event(source,"Fired a shot");
                    self.record_unit_event(target,"Hit by incoming fire");
                    self.data.last_combat_tick = tick;
                    if let (Some(a), Some(v)) = (
                        self.data.tactics.agent(source),
                        self.data.tactics.agent(target),
                    ) {
                        let team = a.team;
                        let patch = patch_at(v.position());
                        self.data.ecology.alert_threat(patch as u32, 200)?;
                        let damage = self.data.ecology.damage_plants(patch as u32, 2, false)?;
                        self.data.ecological_damage[team as usize] = self.data.ecological_damage
                            [team as usize]
                            .checked_add(damage.plant_units_removed)
                            .ok_or("Damage overflow")?;
                    }
                }
                TacticalEvent::NeedsSupply { agent, team } => {
                    let a = self
                        .data
                        .tactics
                        .agent(agent)
                        .ok_or("Unknown supply recipient")?;
                    if distance(a.position(), TEAM_BASES[team as usize]) < 180.
                        && self.data.construction.stores[team as usize].metal > 0
                    {
                        let offered =
                            self.data.construction.stores[team as usize].metal.min(10) as u16;
                        let accepted = self.data.tactics.give_ammunition(agent, offered)?;
                        if accepted > 0 {
                            self.data
                                .construction
                                .consume(team, Resources::new(accepted as u64, 0, 0, 0))?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
    fn construction_event(&mut self, event: ConstructionEvent) -> Result<(), String> {
        match event {
            ConstructionEvent::Completed {
                team,
                kind,
                position,
                ..
            } => {
                self.data.completed_jobs = self
                    .data
                    .completed_jobs
                    .checked_add(1)
                    .ok_or("Job counter overflow")?;
                if team == 0 || self.data.recon.visible(0, position) {
                    self.data.status = format!(
                        "{} completed for faction{} at {:.0},{:.0}",
                        kind.name(),
                        team,
                        position[0],
                        position[1]
                    );
                }
            }
            ConstructionEvent::HabitatCleared {
                team,
                cell,
                foliage_units,
            } => {
                let p = patch_at([((cell % CW) * 8) as f32, ((cell / CW) * 8) as f32]);
                self.account_cover_loss(p, foliage_units as u64, Some(team))?;
                self.data.last_cover[p] = u16::MAX;
            }
            ConstructionEvent::TerrainChanged { cells, .. } => {
                let edits = cells
                    .into_iter()
                    .map(|cell| {
                        self.terrain
                            .elevation_mm(cell)
                            .map(|height| (cell, height))
                            .map_err(|e| e.to_string())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.reconcile_surface_habitat_edits(&edits)?;
            }
            ConstructionEvent::Mined { team, amount, .. } => {
                self.data.ore_delivered[team as usize] = self.data.ore_delivered[team as usize]
                    .checked_add(amount)
                    .ok_or("Delivered ore overflow")?;
            }
            ConstructionEvent::Blocked { job, reason } => {
                if self
                    .data
                    .construction
                    .jobs
                    .iter()
                    .find(|j| j.id == job)
                    .is_some_and(|j| j.order.team == 0)
                {
                    self.data.status = reason;
                }
            }
        }
        Ok(())
    }
    fn sensors(&self) -> Vec<Sensor> {
        let mut sensors = Vec::new();
        for team in 0..2 {
            let mut sensor = Sensor::new(
                SHIP_ID + team,
                team as u8,
                SensorKind::ShipCamera,
                TEAM_BASES[team as usize],
            );
            sensor.enabled = self.data.ship_health[team as usize] > 0;
            sensors.push(sensor);
        }
        for b in self
            .data
            .construction
            .buildings
            .iter()
            .filter(|b| b.health > 0 && (b.kind.camera_spec().is_some() || b.kind == BuildKind::Radar))
        {
            let mut sensor = Sensor::new(
                b.id,
                b.team,
                if b.kind == BuildKind::Radar {
                    SensorKind::Radar
                } else {
                    SensorKind::Camera
                },
                b.position,
            );
            if let Some((height,range)) = b.kind.camera_spec() {
                sensor.height_mm=height; sensor.range_m=range;
                if b.kind==BuildKind::MountedCamera {
                    sensor.enabled=self.data.construction.mount_host(BuildOrder::at(b.team,b.kind,b.position)).is_some();
                }
            }
            sensors.push(sensor);
        }
        for a in self
            .data
            .tactics
            .agents
            .iter()
            .filter(|a| a.alive() && a.role == Role::Scout)
        {
            let mut sensor = Sensor::new(
                a.id + 2_000_000_000,
                a.team,
                SensorKind::CameraTrooper,
                a.position(),
            );
            sensor.at_base = distance(a.position(), TEAM_BASES[a.team as usize]) < 100.;
            sensors.push(sensor);
        }
        sensors
    }
    fn recon_update(&mut self, tick: u64) -> Result<(), String> {
        let sensors = self.sensors();
        let mut heights = vec![0; 15000];
        let mut objects = vec![0; 15000];
        let mut solid = vec![false; 15000];
        for y in 0..100 {
            for x in 0..150 {
                let i = y * 150 + x;
                let g = (y * 4 + 2) * CW + x * 4 + 2;
                heights[i] = self.terrain.elevation_mm(g).map_err(|e| e.to_string())?;
                objects[i] = (self
                    .data
                    .ecology
                    .canopy_permille(patch_at([x as f32 * 32. + 16., y as f32 * 32. + 16.]) as u32)
                    as u32
                    * 10)
                    .min(12_000);
                solid[i] = objects[i] > 2000;
            }
        }
        for b in self
            .data
            .construction
            .buildings
            .iter()
            .filter(|b| b.health > 0 && b.kind.solid())
        {
            let x = (b.position[0] as usize / 32).min(149);
            let y = (b.position[1] as usize / 32).min(99);
            objects[y * 150 + x] = 8000;
            solid[y * 150 + x] = true;
        }
        for team in 0..2u8 {
            self.data
                .recon
                .step_with_obstructions(tick, team, &sensors, &heights, &objects, &solid)?;
            let candidates = self
                .data
                .tactics
                .agents
                .iter()
                .filter(|a| a.alive() && a.team != team && a.embarked_on.is_none())
                .map(|a| ContactCandidate {
                    opaque_track: a.id,
                    position: a.position(),
                })
                .collect::<Vec<_>>();
            self.data.recon.sweep_contacts(tick, team, &candidates)?;
        }
        Ok(())
    }
    pub fn cohort_position(&self, c: &crate::ecology::Cohort) -> [f32; 2] {
        if let Some(p) = self
            .data
            .physical
            .tracked_position(c.id, &self.data.habitats)
        {
            return [p[0] as f32 / 1000., p[1] as f32 / 1000.];
        }
        if let Some(d) = self.data.detailed.get(&c.id) {
            return d.position;
        }
        let center = patch_center(c.patch as usize);
        let seed = mix_hash(c.id ^ self.data.seed as u64);
        [
            center[0] + ((seed & 255) as f32 - 128.) * 0.45,
            center[1] + (((seed >> 8) & 255) as f32 - 128.) * 0.45,
        ]
    }
    fn refresh_observation(&mut self, tick: u64) -> Result<(), String> {
        for p in 0..600 {
            for team in 0..2 {
                self.data.ecology.visible_patches[team][p] =
                    self.data.recon.visible(team as u8, patch_center(p));
            }
        }
        let mut observed = Vec::new();
        for c in self
            .data
            .ecology
            .cohorts
            .iter()
            .filter(|c| c.count > 0 && c.surface_observable && c.location_height_mm >= 0)
        {
            let position = self.cohort_position(c);
            for team in 0..2u8 {
                if self.data.recon.visible(team, position) {
                    observed.push((team, c.patch, c.species_id));
                }
            }
        }
        for plant in self
            .data
            .ecology
            .plants
            .iter()
            .filter(|p| p.individuals > 0)
        {
            for team in 0..2u8 {
                if self
                    .data
                    .recon
                    .visible(team, patch_center(plant.patch as usize))
                {
                    observed.push((team, plant.patch, plant.species_id));
                }
            }
        }
        for (team, patch, species) in observed {
            self.data.ecology.visible_patches[team as usize][patch as usize] = true;
            let knowledge = &mut self.data.ecology.knowledge[team as usize][species as usize];
            knowledge.observed = true;
            knowledge.observed_tick = tick;
        }
        let known = self
            .data
            .ecology
            .species
            .iter()
            .filter(|s| {
                self.data.ecology.knowledge[0][s.id as usize].observed
                    && s.taxon != Taxon::Plant
                    && s.taxon != Taxon::Fungus
            })
            .map(|s| s.id)
            .collect::<Vec<_>>();
        if self.data.selected_species.is_none() {
            self.data.selected_species = known.first().copied();
        }
        let visible = self
            .data
            .ecology
            .cohorts
            .iter()
            .filter(|c| {
                c.count > 0
                    && !self.data.detailed.contains_key(&c.id)
                    && c.surface_observable
                    && c.location_height_mm >= 0
                    && self.data.recon.visible(0, self.cohort_position(c))
            })
            .map(|c| c.id)
            .take(48)
            .collect::<Vec<_>>();
        for id in visible {
            if self.data.detailed.len() >= 5000 {
                break;
            }
            let before = self
                .data
                .ecology
                .cohorts
                .iter()
                .find(|c| c.id == id)
                .ok_or("Missing observed cohort")?;
            let position = self.cohort_position(before);
            let new_id = self.data.ecology.materialize_one(id)?;
            self.data.physical.observe_position(
                new_id,
                [(position[0] * 1000.) as i32, (position[1] * 1000.) as i32],
            )?;
            self.data.detailed.entry(new_id).or_insert(DetailedAnimal {
                position,
                previous: position,
                last_seen: tick,
            });
        }
        Ok(())
    }
    fn pen_access(&self, id: u64) -> Option<[f32; 2]> {
        let pen = self
            .data
            .construction
            .buildings
            .iter()
            .find(|b| b.id == id && b.health > 0 && b.kind == BuildKind::Pen)?;
        self.data
            .construction
            .jobs
            .iter()
            .find(|j| {
                j.order.team == pen.team
                    && j.order.kind == BuildKind::Pen
                    && j.order.position == pen.position
                    && matches!(j.phase, crate::construction::JobPhase::Complete)
            })
            .map(|j| j.work_position)
    }
    fn move_detailed(&mut self, tick: u64) -> Result<(), String> {
        let living: BTreeMap<u64, _> = self
            .data
            .ecology
            .cohorts
            .iter()
            .filter(|c| c.count == 1)
            .map(|c| {
                (
                    c.id,
                    (
                        c.patch,
                        c.species_id,
                        c.surface_observable,
                        c.location_height_mm,
                    ),
                )
            })
            .collect();
        let mut remove = Vec::new();
        for (id, detail) in &mut self.data.detailed {
            let Some((patch, species, observable, height)) = living.get(id) else {
                remove.push(*id);
                continue;
            };
            let center = patch_center(*patch as usize);
            if self.data.recon.visible(0, detail.position) {
                detail.last_seen = tick;
            }
            if tick.saturating_sub(detail.last_seen) > 200 {
                remove.push(*id);
                continue;
            }
            detail.previous = detail.position;
            if let Some(real) = self
                .data
                .physical
                .tracked_position(*id, &self.data.habitats)
            {
                detail.position = [real[0] as f32 / 1000., real[1] as f32 / 1000.];
                if !observable || *height < 0 {
                    continue;
                } else {
                    continue;
                }
            }
            if !observable || *height < 0 {
                continue;
            }
            if patch_at(detail.position) != *patch as usize {
                detail.position = center;
                detail.previous = center;
            }
            let h = mix_hash(*id ^ (tick / 100));
            let goal = [
                center[0] + ((h & 255) as f32 - 128.) * 0.5,
                center[1] + (((h >> 8) & 255) as f32 - 128.) * 0.5,
            ];
            let dx = goal[0] - detail.position[0];
            let dy = goal[1] - detail.position[1];
            let len = dx.hypot(dy).max(1.);
            let speed = if self.data.ecology.species[*species as usize].slow_movement {
                0.03
            } else {
                0.10
            };
            let next = [
                detail.position[0] + dx / len * speed,
                detail.position[1] + dy / len * speed,
            ];
            if let Some(i) = self.data.nav.index(next) {
                let habitat = self.data.ecology.species[*species as usize].habitat;
                let allowed = match habitat {
                    crate::ecology::Habitat::Water => self.data.nav.cells[i].water_depth_mm > 0,
                    crate::ecology::Habitat::Amphibious => !self.data.nav.cells[i].blocked,
                    crate::ecology::Habitat::Land => self.data.nav.passable(i, MovementClass::Foot),
                };
                if allowed {
                    detail.position = next;
                }
            }
        }
        for id in remove {
            self.data.detailed.remove(&id);
        }
        Ok(())
    }
    fn update_biological_cover(&mut self, environment: &[EcologyPatch]) -> Result<(), String> {
        for p in 0..PATCH_WIDTH * PATCH_HEIGHT {
            let fraction = self.data.ecology.plant_fraction_permille(p as u32);
            let band = (fraction as u32 * 32 / 1000) as u16;
            if self.data.last_cover[p] == band {
                continue;
            }
            self.data.last_cover[p] = band;
            let x0 = p % PATCH_WIDTH * 20;
            let y0 = p / PATCH_WIDTH * 20;
            for y in y0..y0 + 20 {
                for x in x0..x0 + 20 {
                    let cell = y * CW + x;
                    if RiverFixture::local_index(cell).is_some() {
                        continue;
                    }
                    if self.terrain.cells[cell].object != 0
                        || self.terrain.cells[cell].flags & 1 != 0
                    {
                        continue;
                    }
                    let initial = self
                        .terrain
                        .initial_foliage_units(cell)
                        .ok_or("Biological cover mapping")?;
                    let units = (initial as u64 * fraction as u64 / 1000) as u32;
                    self.terrain
                        .apply_biological_cover(
                            cell,
                            units,
                            environment[p].moisture_permille,
                            environment[p].disturbance > 0,
                        )
                        .map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(())
    }
    fn production_and_care(&mut self, tick: u64) -> Result<(), String> {
        for team in 0..2u8 {
            let completed = self
                .data
                .construction
                .buildings
                .iter()
                .filter(|b| b.team == team && b.health > 0 && b.complete)
                .cloned()
                .collect::<Vec<_>>();
            for b in completed {
                let patch = patch_at(b.position) as u32;
                if b.kind == BuildKind::Mill && tick % 100 == 0 {
                    let amount = self
                        .data
                        .ecology
                        .available_engineering_biomass(patch)
                        .min(4);
                    if amount > 0 {
                        self.data.ecology.export_biomass(patch, amount)?;
                        self.data
                            .construction
                            .credit_external(team, Resources::new(0, amount, 0, 0))?;
                        self.data.ecological_damage[team as usize] = self.data.ecological_damage
                            [team as usize]
                            .checked_add(amount)
                            .ok_or("Damage overflow")?;
                    }
                }
                if b.kind == BuildKind::Farm && tick % 100 == 0 {
                    let amount = self
                        .data
                        .ecology
                        .available_engineering_biomass(patch)
                        .min(4);
                    if amount > 0 {
                        self.data.ecology.export_biomass(patch, amount)?;
                        self.data
                            .construction
                            .credit_external(team, Resources::new(0, 0, amount, 0))?;
                    }
                }
            }
            if tick % 100 == 0
                && self
                    .data
                    .ecology
                    .captives
                    .iter()
                    .any(|c| c.team == team && c.housing.is_some())
                && self
                    .data
                    .construction
                    .buildings
                    .iter()
                    .any(|b| b.team == team && b.kind == BuildKind::Pen && b.health > 0)
            {
                let count = self
                    .data
                    .ecology
                    .captives
                    .iter()
                    .filter(|c| c.team == team && c.housing.is_some())
                    .count() as u64;
                let food = (count * 12).min(self.data.construction.stores[team as usize].rations);
                let water = (count * 12).min(self.data.base_water[team as usize]);
                self.data.ecology.supply_care(team, food, water)?;
                self.data
                    .construction
                    .consume(team, Resources::new(0, 0, food, 0))?;
                self.data.base_water[team as usize] -= water;
                self.data.water_given_to_care = self
                    .data
                    .water_given_to_care
                    .checked_add(water)
                    .ok_or("Care water overflow")?;
            }
        }
        Ok(())
    }
    fn enemy_order(&mut self, tick: u64) -> Result<(), String> {
        let kinds = [
            BuildKind::Mine,
            BuildKind::Solar,
            BuildKind::Camera,
            BuildKind::Barracks,
            BuildKind::Turret,
            BuildKind::Radar,
        ];
        let index = ((tick / 600).saturating_sub(1) as usize) % kinds.len();
        let base = TEAM_BASES[1];
        let position = if kinds[index] == BuildKind::Mine {
            self.data.construction.deposits[1].position
        } else {
            [
                base[0] - 100. - (index as f32 % 3.) * 70.,
                base[1] + 100. + (index as f32 / 3.).floor() * 80.,
            ]
        };
        if let Err(error) = self
            .data
            .construction
            .enqueue(BuildOrder::at(1, kinds[index], position), &self.terrain)
        {
            logging::log("ENEMY_ORDER_REJECTED", &error);
        }
        let observed = self
            .data
            .tactics
            .agents
            .iter()
            .filter(|a| a.team == 0 && a.alive() && self.data.recon.visible(1, a.position()))
            .map(|a| a.position())
            .next();
        let goal = observed.unwrap_or([2200., 1580.]);
        if let Err(error) = self.data.tactics.order_on_grid(1, goal, &self.data.nav) {
            logging::log("ENEMY_ROUTE_WAIT", &error);
        }
        Ok(())
    }
    /// Complete one actual custody-to-world handoff at authoritative millimeter
    /// coordinates. Preflight the two finite care-buffer transfers before the
    /// physical owner's own checked commit. This needs no whole-world clones.
    fn release_carried_animal_at(&mut self, id: u64, position_mm: [i32; 2]) -> Result<(), String> {
        if position_mm[0] < 0
            || position_mm[1] < 0
            || position_mm[0] >= (CW * CELL_METERS * 1000) as i32
            || position_mm[1] >= (CH * CELL_METERS * 1000) as i32
        {
            return Err("Custody handoff outside the resident world".into());
        }
        let captive = self
            .data
            .ecology
            .captives
            .iter()
            .find(|c| c.animal.id == id && c.animal.count > 0)
            .ok_or("Custody handoff needs an existing living captive")?;
        let patch = (position_mm[0] as usize / (PATCH_METERS as usize * 1000)
            + position_mm[1] as usize / (PATCH_METERS as usize * 1000) * PATCH_WIDTH)
            as u32;
        let zone = self
            .data
            .ecology
            .root_zones
            .get(patch as usize)
            .ok_or("Custody handoff has no ecological destination")?;
        zone.nutrients
            .checked_add(captive.care_food)
            .ok_or("Custody handoff food transfer overflow")?;
        zone.shallow_water
            .checked_add(captive.care_water)
            .ok_or("Custody handoff water transfer overflow")?;
        // transport_completed preflights its fallible physical operations. After
        // this checked commit, escape_captive performs the preflighted exact
        // buffer transfers and moves this existing owner into the wild ledger.
        self.data
            .physical
            .transport_completed(id, patch, position_mm, &mut self.data.habitats)?;
        self.data.ecology.escape_captive(id, patch)?;
        self.data.ecology.set_surface_observable(id, true, 0)?;
        self.data.ecology.set_breeding_refuge(id, None)?;
        let now = self.data.ecology.last_tick;
        self.data.ecology.set_engineering_busy(id, now)?;
        let position = [position_mm[0] as f32 / 1000., position_mm[1] as f32 / 1000.];
        self.data.detailed.insert(
            id,
            DetailedAnimal {
                position,
                previous: position,
                last_seen: self.data.clock.tick(),
            },
        );
        Ok(())
    }
    fn update_capture_orders(&mut self) -> Result<(), String> {
        let orders = std::mem::take(&mut self.data.capture_orders);
        for mut order in orders {
            let worker = self.data.tactics.agent(order.worker).cloned();
            if let Some(id) = order.captured {
                if !self
                    .data
                    .ecology
                    .captives
                    .iter()
                    .any(|c| c.animal.id == id && c.animal.count > 0)
                {
                    // Normal biological mortality may occur while awaiting pickup
                    // or in transit. It cancels the job; it is never a fatal
                    // missing-object error and never releases a replacement.
                    if worker.as_ref().is_some_and(|a| a.alive())
                        && !self.unit_has_manual_order(order.worker)
                    {
                        self.data.tactics.stop(order.worker)?;
                    }
                    logging::log("CAPTURE_CARGO_LOST", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"animal":id,"releasing":order.releasing}).to_string());
                    self.data.status = "Specimen transport cancelled because the animal is no longer in captivity.".into();
                    continue;
                }
            }
            if worker.as_ref().is_none_or(|a| !a.alive()) {
                if let Some(id) = order
                    .captured
                    .filter(|_| !order.releasing || order.picked_up)
                {
                    if self.data.ecology.captives.iter().any(|c| c.animal.id == id) {
                        let position_mm = worker
                            .as_ref()
                            .ok_or("Captured cargo lost its authoritative carrier position")?
                            .position_mm;
                        self.release_carried_animal_at(id, position_mm)?;
                        logging::log("CAPTURE_HANDLER_LOST", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"animal":id,"position_mm":position_mm}).to_string());
                    }
                }
                continue;
            }
            let worker = worker.unwrap();
            if let Some(id) = order
                .captured
                .filter(|_| !order.releasing || order.picked_up)
            {
                if self.data.ecology.captives.iter().any(|c| c.animal.id == id) {
                    self.data
                        .ecology
                        .set_captive_transit(id, patch_at(worker.position()) as u32)?;
                }
            }
            if self.unit_has_manual_order(order.worker) {
                self.data.capture_orders.push(order);
                continue;
            }
            let Some(pen_position) = self.pen_access(order.pen) else {
                self.data.status="Capture/release route lost its pen; specimen remains finite and requires evacuation.".into();
                continue;
            };
            let target = if order.releasing {
                if order.picked_up {
                    order
                        .last_known_position
                        .unwrap_or_else(|| patch_center(order.patch as usize))
                } else {
                    pen_position
                }
            } else if order.captured.is_some() {
                pen_position
            } else {
                let Some(c) = self.data.ecology.cohorts.iter().find(|c| {
                    c.id == order.cohort
                        && c.count > 0
                        && c.surface_observable
                        && c.location_height_mm >= 0
                }) else {
                    logging::log("CAPTURE_TARGET_LOST", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"cohort":order.cohort}).to_string());
                    self.data.status = "Capture target is no longer accessible.".into();
                    continue;
                };
                let actual = self.cohort_position(c);
                if self.data.recon.visible(order.team, actual) {
                    order.last_known_position = Some(actual);
                }
                let Some(last_known) = order.last_known_position else {
                    self.data.capture_orders.push(order);
                    continue;
                };
                last_known
            };
            if let Err(error) = self.data.tactics.assign_on_grid_within(
                order.worker,
                target,
                None,
                &self.data.nav,
                10.,
            ) {
                if self.data.processed_tick % 200 == 0 {
                    logging::log("HANDLER_ROUTE_WAIT", &error);
                }
                self.data.capture_orders.push(order);
                continue;
            }
            if distance(worker.position(), target) < 12. {
                if order.releasing {
                    if !order.picked_up {
                        let id = order.captured.ok_or("Release identity missing")?;
                        if let Some(c) = self
                            .data
                            .ecology
                            .captives
                            .iter_mut()
                            .find(|c| c.animal.id == id)
                        {
                            c.housing = None;
                        }
                        order.picked_up = true;
                    } else {
                        if !self.data.recon.visible(order.team, target) {
                            self.data.status =
                                "Release handler is waiting for video at the destination.".into();
                            self.data.capture_orders.push(order);
                            continue;
                        }
                        let id = order.captured.ok_or("Release cargo missing")?;
                        let cargo = self
                            .data
                            .ecology
                            .captives
                            .iter()
                            .find(|c| c.animal.id == id)
                            .ok_or("Release cargo ownership changed during handoff")?;
                        if cargo.team != order.team || cargo.animal.species_id != order.species {
                            return Err(
                                "Release cargo does not match its transport owner/species".into()
                            );
                        }
                        // Visibility of the exact destination is authoritative;
                        // its160m patch center may lie outside the camera cone.
                        self.data.ecology.visible_patches[order.team as usize]
                            [order.patch as usize] = true;
                        self.release_carried_animal_at(
                            id,
                            [(target[0] * 1000.) as i32, (target[1] * 1000.) as i32],
                        )?;
                        logging::log("CAPTURE_RELEASED", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"animal":id,"position":target}).to_string());
                        self.data.tactics.stop(order.worker)?;
                        self.data.status =
                            "Existing animal delivered and released into observed habitat.".into();
                        continue;
                    }
                } else if order.captured.is_none() {
                    let in_reach = self
                        .data
                        .ecology
                        .cohorts
                        .iter()
                        .find(|c| c.id == order.cohort)
                        .is_some_and(|c| {
                            let actual = self.cohort_position(c);
                            self.data.recon.visible(order.team, actual)
                                && distance(worker.position(), actual) < 12.
                        });
                    if !in_reach {
                        self.data.status = "Capture target left video coverage.".into();
                        self.data.capture_orders.push(order);
                        continue;
                    }
                    let required = self.data.ecology.capture_work_ticks(order.cohort)?;
                    order.handling_work_ticks = order
                        .handling_work_ticks
                        .checked_add(1)
                        .ok_or("Animal handling work overflow")?;
                    if order.handling_work_ticks < required {
                        self.data.capture_orders.push(order);
                        continue;
                    }
                    match self.data.ecology.capture_cohort(order.cohort, order.team) {
                        Ok(id) => {
                            order.captured = Some(id);
                            logging::log("CAPTURE_SECURED", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"animal":id,"pen":order.pen}).to_string());
                            self.data.status =
                                "Animal captured; handler is returning to its actual pen.".into();
                        }
                        Err(error) => {
                            logging::log("CAPTURE_REJECTED", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"cohort":order.cohort,"reason":error}).to_string());
                            self.data.status = error;
                            continue;
                        }
                    }
                } else {
                    let id = order.captured.unwrap();
                    if self.data.ecology.captives.iter().any(|c| c.animal.id == id) {
                        match self.data.ecology.house_captive(id, order.pen) {
                            Ok(()) => {
                                logging::log("CAPTURE_HOUSED", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"animal":id,"pen":order.pen}).to_string());
                                self.data.tactics.stop(order.worker)?;
                                self.data.status="Specimen delivered to its pen. Care and research can now proceed.".into();
                            }
                            Err(error) => {
                                if self.data.clock.tick() % 200 == 0 {
                                    logging::log("CAPTURE_HOUSING_WAIT", &serde_json::json!({"tick":self.data.clock.tick(),"worker":order.worker,"animal":id,"pen":order.pen,"reason":error}).to_string());
                                }
                                self.data.status = format!("Handler waiting for housing: {error}");
                                self.data.capture_orders.push(order);
                            }
                        }
                    }
                    continue;
                }
            }
            self.data.capture_orders.push(order);
        }
        Ok(())
    }
    fn check_outcome(&mut self) {
        if self.data.winner.is_some() {
            return;
        }
        for team in 0..2 {
            if self.data.ship_health[team] == 0 {
                self.data.winner = Some((1 - team) as u8);
                self.data.outcome = format!("Faction{} lost its landing ship", team);
                return;
            }
        }
        if self.data.ecological_damage.iter().sum::<u64>() > self.data.ecological_initial / 3
            && self.data.ecological_damage[0] != self.data.ecological_damage[1]
        {
            self.data.winner = Some(u8::from(
                self.data.ecological_damage[1] < self.data.ecological_damage[0],
            ));
            self.data.outcome =
                "Ecological stalemate: the faction causing greater damage loses".into();
        }
    }
    pub fn recruit_camera(&mut self) -> Result<(), String> {
        let cost = Resources::new(220, 30, 20, 60);
        self.data.construction.stores[0].checked_sub(cost)?;
        self.data
            .tactics
            .spawn_formation(0, Role::Scout, TEAM_BASES[0], 1, &self.data.nav)?;
        self.data.construction.consume(0, cost)?;
        self.data.status =
            "Unarmed camera trooper recruited; return it to the ship to recharge.".into();
        Ok(())
    }
    pub fn cycle_species(&mut self) {
        let known = self
            .data
            .ecology
            .species
            .iter()
            .filter(|s| {
                self.data.ecology.knowledge[0][s.id as usize].observed
                    && s.taxon != Taxon::Plant
                    && s.taxon != Taxon::Fungus
            })
            .map(|s| s.id)
            .collect::<Vec<_>>();
        if known.is_empty() {
            self.data.selected_species = None;
            return;
        }
        let next = self
            .data
            .selected_species
            .and_then(|id| known.iter().position(|s| *s == id))
            .map_or(0, |i| (i + 1) % known.len());
        self.data.selected_species = Some(known[next]);
    }
    pub fn place_segment(&mut self, from: [f32; 2], mut to: [f32; 2]) -> Result<(), String> {
        let kind = *BUILD_TOOLS
            .get(self.data.selected_tool)
            .ok_or("Choose a terrain line tool")?;
        if !matches!(
            kind,
            BuildKind::Road | BuildKind::Trench | BuildKind::Bridge
        ) {
            return Err("The selected tool is not a line order".into());
        }
        if !self.data.recon.visible(0, from) || !self.data.recon.visible(0, to) {
            return Err("Both ends of a planned line need current video".into());
        }
        if distance(from, to) < 2. {
            to = [(from[0] + 64.).min(4792.), from[1]];
        }
        let id = self.data.construction.enqueue(
            BuildOrder {
                team: 0,
                kind,
                position: from,
                end: to,
            },
            &self.terrain,
        )?;
        self.data.status = format!(
            "Order{} queued: {} {:.0}m. Workers must haul and build it.",
            id,
            kind.name(),
            distance(from, to)
        );
        Ok(())
    }
    pub fn click(&mut self, position: [f32; 2], right: bool) -> Result<(), String> {
        if self.data.winner.is_some() {
            return Err(self.data.outcome.clone());
        }
        if !self.data.recon.visible(0, position) {
            return Err(
                "No current video at that location. Extend cameras or wait for satellite coverage."
                    .into(),
            );
        }
        if right {
            let n = self
                .data
                .tactics
                .order_on_grid(0, position, &self.data.nav)?;
            self.data.status = format!("Rally order sent to{} military pawns", n);
            return Ok(());
        }
        if let Some(kind) = crate::ui_interaction::build_kind(self.data.selected_tool) {
            let end = if matches!(
                kind,
                BuildKind::Road | BuildKind::Trench | BuildKind::Bridge
            ) {
                [position[0] + 64., position[1]]
            } else {
                position
            };
            let id = self.data.construction.enqueue(
                BuildOrder {
                    team: 0,
                    kind,
                    position,
                    end,
                },
                &self.terrain,
            )?;
            self.data.status = format!(
                "Order{} queued:{}. A worker must collect and deliver the materials.",
                id,
                kind.name()
            );
            return Ok(());
        }
        let action = self.data.selected_tool - BUILD_TOOLS.len();
        let patch = patch_at(position) as u32;
        match action {
            0 => {
                let ids = self
                    .data
                    .ecology
                    .cohorts
                    .iter()
                    .filter(|c| {
                        c.patch == patch
                            && c.count > 0
                            && c.surface_observable
                            && c.location_height_mm >= 0
                            && self.data.recon.visible(0, self.cohort_position(c))
                            && distance(self.cohort_position(c), position) < 24.
                    })
                    .map(|c| c.species_id)
                    .collect::<Vec<_>>();
                if let Some(id) = ids.first() {
                    self.data.selected_species = Some(*id);
                    self.data.status=self.data.ecology.observed_inspect(0,*id).map(|v|format!("Observed {}. Research stage{}; capture and continued care reveal more.",v["name"].as_str().unwrap_or("organism"),v["research_stage"])).unwrap_or_else(||"No identified organism in video".into());
                } else {
                    self.data.status = "No accessible animal identified here.".into();
                }
            }
            1 => {
                let pen = self
                    .data
                    .construction
                    .buildings
                    .iter()
                    .find(|b| {
                        b.team == 0
                            && b.kind == BuildKind::Pen
                            && b.health > 0
                            && self.data.ecology.housing_available(0, b.id).unwrap_or(0)
                                > self
                                    .data
                                    .capture_orders
                                    .iter()
                                    .filter(|o| o.pen == b.id && !o.releasing)
                                    .count() as u32
                    })
                    .ok_or("Build a pen with free capacity before capture")?
                    .id;
                let species = self
                    .data
                    .selected_species
                    .ok_or("Select an observed animal species with TAB/Inspect")?;
                let cohort = self
                    .data
                    .ecology
                    .cohorts
                    .iter()
                    .filter(|c| {
                        c.patch == patch
                            && c.species_id == species
                            && c.count > 0
                            && c.surface_observable
                            && c.location_height_mm >= 0
                            && self.data.recon.visible(0, self.cohort_position(c))
                    })
                    .min_by(|a, b| {
                        distance(self.cohort_position(a), position)
                            .total_cmp(&distance(self.cohort_position(b), position))
                    })
                    .ok_or("No currently visible accessible animal of that species here")?
                    .id;
                let worker = self
                    .data
                    .tactics
                    .agents
                    .iter()
                    .filter(|a| {
                        a.alive()
                            && a.team == 0
                            && a.role == Role::Worker
                            && !self.unit_has_manual_order(a.id)
                            && !self.data.capture_orders.iter().any(|o| o.worker == a.id)
                    })
                    .min_by(|a, b| {
                        distance(a.position(), position)
                            .total_cmp(&distance(b.position(), position))
                    })
                    .ok_or("No available handler")?
                    .id;
                self.data.capture_orders.push(CaptureOrder {
                    worker,
                    team: 0,
                    species,
                    patch,
                    captured: None,
                    releasing: false,
                    picked_up: false,
                    pen,
                    cohort,
                    handling_work_ticks: 0,
                    last_known_position: self
                        .data
                        .ecology
                        .cohorts
                        .iter()
                        .find(|c| c.id == cohort)
                        .map(|c| self.cohort_position(c)),
                });
                self.data.status =
                    "Capture handler dispatched. It must reach the animal and return.".into();
            }
            2 => {
                if !self
                    .data
                    .construction
                    .buildings
                    .iter()
                    .any(|b| b.team == 0 && b.kind == BuildKind::Research && b.health > 0)
                {
                    return Err("Build a research building first".into());
                }
                self.data.ecology.research(
                    0,
                    self.data.selected_species.ok_or("Select a known species")?,
                )?;
                self.data.status =
                    "Research enabled; continued care and observation are required.".into();
            }
            3 => {
                self.data.ecology.breed(
                    0,
                    self.data.selected_species.ok_or("Select a known species")?,
                )?;
                self.data.status =
                    "Compatible captive breeding enabled; offspring require actual care and time."
                        .into();
            }
            4 => {
                let species = self.data.selected_species.ok_or("Select a known species")?;
                let captive = self
                    .data
                    .ecology
                    .captives
                    .iter()
                    .find(|c| {
                        c.team == 0
                            && c.housing.is_some()
                            && c.animal.species_id == species
                            && !self
                                .data
                                .capture_orders
                                .iter()
                                .any(|o| o.captured == Some(c.animal.id))
                    })
                    .ok_or("No delivered captive of the selected species")?;
                let animal = captive.animal.id;
                let pen = captive.housing.ok_or("Captive has no physical pen")?;
                let worker = self
                    .data
                    .tactics
                    .agents
                    .iter()
                    .filter(|a| {
                        a.alive()
                            && a.team == 0
                            && a.role == Role::Worker
                            && !self.unit_has_manual_order(a.id)
                            && !self.data.capture_orders.iter().any(|o| o.worker == a.id)
                    })
                    .min_by(|a, b| {
                        distance(a.position(), TEAM_BASES[0])
                            .total_cmp(&distance(b.position(), TEAM_BASES[0]))
                    })
                    .ok_or("No release handler available")?
                    .id;
                self.data.capture_orders.push(CaptureOrder {
                    worker,
                    team: 0,
                    species,
                    patch,
                    captured: Some(animal),
                    releasing: true,
                    picked_up: false,
                    pen,
                    cohort: animal,
                    handling_work_ticks: 0,
                    last_known_position: Some(position),
                });
                self.data.status =
                    "Release handler must collect the specimen and travel to this habitat.".into();
            }
            5 => {
                self.data
                    .tactics
                    .order_on_grid(0, position, &self.data.nav)?;
            }
            6 | 7 => {
                let role = if action == 6 {
                    Role::Boat
                } else {
                    Role::Transport
                };
                if !self.data.construction.buildings.iter().any(|b| {
                    b.team == 0
                        && b.kind == BuildKind::Dock
                        && b.health > 0
                        && distance(b.position, position) < 120.
                }) {
                    return Err("Vessel launch requires a completed nearby dock".into());
                }
                let cell = self
                    .data
                    .nav
                    .index(position)
                    .ok_or("Vessel launch outside map")?;
                if !self.data.nav.passable(cell, role.movement()) {
                    return Err(
                        "Insufficient connected water depth or overhead clearance for this vessel"
                            .into(),
                    );
                }
                let cost = Resources::new(500, 200, 20, 30);
                self.data.construction.stores[0].checked_sub(cost)?;
                self.data.tactics.spawn(0, role, position)?;
                self.data.construction.consume(0, cost)?;
                self.data.status = "Material-funded vessel launched.".into();
            }
            8 => {
                let transport = self
                    .data
                    .tactics
                    .agents
                    .iter()
                    .find(|a| {
                        a.team == 0
                            && a.alive()
                            && a.role == Role::Transport
                            && distance(a.position(), position) < 100.
                    })
                    .ok_or("No nearby transport")?
                    .id;
                let passengers = self
                    .data
                    .tactics
                    .agents
                    .iter()
                    .filter(|a| {
                        a.team == 0
                            && a.alive()
                            && a.role == Role::Soldier
                            && a.embarked_on.is_none()
                            && distance(a.position(), position) < 24.
                    })
                    .map(|a| a.id)
                    .take(12)
                    .collect::<Vec<_>>();
                for id in passengers {
                    self.data.tactics.board(id, transport)?;
                }
                self.data.status =
                    "Nearby eligible troops boarded the actual transport manifest.".into();
            }
            9 => {
                let passengers = self
                    .data
                    .tactics
                    .manifests
                    .values()
                    .flat_map(|v| v.iter().copied())
                    .collect::<Vec<_>>();
                for id in passengers {
                    if self.data.tactics.agent(id).is_some_and(|a| a.team == 0) {
                        self.data.tactics.disembark(id, position, &self.data.nav)?;
                    }
                }
            }
            10 => {
                let job = self
                    .data
                    .construction
                    .jobs
                    .iter()
                    .rev()
                    .find(|j| {
                        j.order.team == 0
                            && !matches!(
                                j.phase,
                                crate::construction::JobPhase::Complete
                                    | crate::construction::JobPhase::Cancelled
                            )
                    })
                    .ok_or("No pending order")?
                    .id;
                self.data.construction.cancel(job, 0)?;
            }
            _ => return Err("Unknown campaign tool".into()),
        }
        Ok(())
    }
    pub fn explode(&mut self, position: [f32; 2], team: u8) -> Result<(), String> {
        if team > 1 || !self.data.recon.visible(team, position) {
            return Err("Explosion target needs current video".into());
        }
        let cost = Resources::new(25, 0, 0, 20);
        self.data.construction.stores[team as usize].checked_sub(cost)?;
        let mut edits = Vec::new();
        let mut cut = 0u64;
        let cx = position[0] as i32 / 8;
        let cy = position[1] as i32 / 8;
        for y in (cy - 4).max(0)..=(cy + 4).min(CH as i32 - 1) {
            for x in (cx - 4).max(0)..=(cx + 4).min(CW as i32 - 1) {
                let dx = x * 8 + 4 - position[0] as i32;
                let dy = y * 8 + 4 - position[1] as i32;
                let radius2 = 28 * 28;
                let d2 = dx * dx + dy * dy;
                if d2 >= radius2 {
                    continue;
                }
                let cell = y as usize * CW + x as usize;
                let before = self.terrain.elevation_mm(cell).map_err(|e| e.to_string())?;
                let delta = 300 * (radius2 - d2) / radius2;
                let target = before.checked_sub(delta).ok_or("Crater height overflow")?;
                self.terrain
                    .validate_elevation_edit(cell, target)
                    .map_err(|e| e.to_string())?;
                cut = cut
                    .checked_add(delta as u64 * 64)
                    .ok_or("Crater spoil overflow")?;
                edits.push((cell, target));
            }
        }
        let next_spoil = self.data.blast_spoil_liters[team as usize]
            .checked_add(cut)
            .ok_or("Blast spoil overflow")?;
        let managed = edits
            .iter()
            .copied()
            .filter(|(cell, _)| RiverFixture::local_index(*cell).is_some())
            .collect::<Vec<_>>();
        self.data.river.edit_beds_material(&managed)?;
        for (cell, height) in &edits {
            self.terrain
                .set_elevation_mm(*cell, *height)
                .map_err(|e| e.to_string())?;
        }
        self.reconcile_surface_habitat_edits(&edits)?;
        self.damage_surface_habitats(position, 28., 40)?;
        self.data.blast_spoil_liters[team as usize] = next_spoil;
        let p = patch_at(position);
        let report = self.data.ecology.damage_plants(p as u32, 100, true)?;
        let killed = self.data.ecology.damage_animals(p as u32, 2)?;
        self.data.ecology.alert_threat(p as u32, 400)?;
        self.data
            .physical
            .synchronize_hosts(&mut self.data.ecology, &mut self.data.habitats)?;
        self.data.ecological_damage[team as usize] = self.data.ecological_damage[team as usize]
            .checked_add(report.plant_units_removed + killed.animals_killed * 64)
            .ok_or("Damage overflow")?;
        self.data.construction.consume(team, cost)?;
        let targets = self
            .data
            .tactics
            .agents
            .iter()
            .filter(|a| a.alive() && distance(a.position(), position) < 28.)
            .map(|a| a.id)
            .collect::<Vec<_>>();
        for id in targets {
            self.data.tactics.damage(id, 40, None)?;
        }
        self.data.river.sync_terrain(&mut self.terrain)?;
        self.data.flash = 0.;
        self.data.flash_position = Some(position);
        self.data.status = format!(
            "Artillery strike displaced{}Lofsoil; ecological damage charged to faction{}.",
            cut, team
        );
        Ok(())
    }
    pub fn automation(&mut self) -> Result<(), String> {
        self.data.automated = true;
        let base = TEAM_BASES[0];
        for (kind, offset) in [
            (BuildKind::Trench, [0., 110.]),
            (BuildKind::Pen, [90., 60.]),
            (BuildKind::Research, [-90., 60.]),
            (BuildKind::Camera, [120., -30.]),
            (BuildKind::Mine, [180., -190.]),
            (BuildKind::Solar, [0., -110.]),
        ] {
            let p = if kind == BuildKind::Mine {
                self.data.construction.deposits[0].position
            } else {
                [base[0] + offset[0], base[1] + offset[1]]
            };
            self.data
                .construction
                .enqueue(BuildOrder::at(0, kind, p), &self.terrain)?;
        }
        self.rebuild_structures()?;
        Ok(())
    }
    pub fn intel(&self) -> &[u32] {
        self.data.recon.masks(0)
    }
    pub fn draw_agents(&self, alpha: f32) -> Vec<Agent> {
        let alpha = alpha.clamp(0., 1.);
        let mut agents = Vec::new();
        for a in &self.data.tactics.agents {
            if !a.alive()
                || a.embarked_on.is_some()
                || matches!(a.role, Role::Boat | Role::Transport)
                || !self.data.recon.visible(0, a.position())
            {
                continue;
            }
            let pos = [
                (a.previous_position_mm[0] as f32 * (1. - alpha) + a.position_mm[0] as f32 * alpha)
                    * 0.001,
                (a.previous_position_mm[1] as f32 * (1. - alpha) + a.position_mm[1] as f32 * alpha)
                    * 0.001,
            ];
            agents.push(Agent {
                pos,
                facing: ((a.position_mm[1] - a.previous_position_mm[1]) as f32)
                    .atan2((a.position_mm[0] - a.previous_position_mm[0]) as f32),
                id: a.id as u32,
                goal: 0,
                team: a.team as u32,
                role: if a.role == Role::Worker {
                    0
                } else if a.role == Role::Scout {
                    2
                } else {
                    1
                },
                next: -1,
            });
        }
        let wildlife = self
            .data
            .ecology
            .cohorts
            .iter()
            .filter(|c| c.count == 1 && c.surface_observable && c.location_height_mm >= 0)
            .map(|c| (c.id, c))
            .collect::<BTreeMap<_, _>>();
        for (id, d) in &self.data.detailed {
            let Some(c) = wildlife.get(id) else { continue };
            if !self.data.recon.visible(0, d.position) {
                continue;
            }
            agents.push(Agent {
                pos: [
                    d.previous[0] * (1. - alpha) + d.position[0] * alpha,
                    d.previous[1] * (1. - alpha) + d.position[1] * alpha,
                ],
                facing: 0.,
                id: *id as u32,
                goal: 0,
                team: self.terrain.fauna_appearance(c.species_id),
                role: 4,
                next: -1,
            });
        }
        for e in self.data.recon.echoes(0) {
            if !self.data.recon.visible(0, e.position) {
                agents.push(Agent {
                    pos: e.position,
                    facing: 0.,
                    id: e.opaque_track as u32,
                    goal: 0,
                    team: 0,
                    role: 5,
                    next: -1,
                });
            }
        }
        agents
    }
    pub fn draw_objects(&self) -> Result<Vec<Object>, String> {
        let mut objects = Vec::new();
        for team in 0..2 {
            objects.push(grown_object(
                "ship",
                team,
                TEAM_BASES[team as usize],
                108.,
                8.,
                self.data.ship_health[team as usize] as f32 / 10000.,
                team,
            )?);
        }
        for b in self
            .data
            .construction
            .buildings
            .iter()
            .filter(|b| b.health > 0)
        {
            objects.push(grown_object(
                b.kind.atlas_kind(),
                b.team as u32,
                b.position,
                b.size_meters,
                b.kind.camera_spec().map_or(5., |(h,_)|h as f32*0.001),
                b.health as f32 / b.max_health as f32,
                b.id as u32,
            )?);
        }
        for j in self.data.construction.jobs.iter().filter(|j| {
            !j.order.kind.is_terrain()
                && !matches!(
                    j.phase,
                    crate::construction::JobPhase::Complete
                        | crate::construction::JobPhase::Cancelled
                )
        }) {
            objects.push(grown_object(
                j.order.kind.atlas_kind(),
                j.order.team as u32,
                j.order.position,
                40.,
                2.,
                j.work_done as f32 / j.work_required.max(1) as f32,
                j.id as u32,
            )?);
        }
        for a in self
            .data
            .tactics
            .agents
            .iter()
            .filter(|a| a.alive() && matches!(a.role, Role::Boat | Role::Transport))
        {
            objects.push(grown_object(
                "ship",
                a.team as u32,
                a.position(),
                36.,
                3.,
                a.health as f32 / 600.,
                a.id as u32,
            )?);
        }
        Ok(objects)
    }
    pub fn hud(&self) -> HudModel {
        let resources = self.data.construction.stores[0];
        let living = self
            .data
            .tactics
            .agents
            .iter()
            .filter(|a| a.team == 0 && a.alive() && self.data.recon.visible(0, a.position()))
            .count();
        let clock = SeasonState::at_tick(self.data.clock.tick(), self.data.seed, 0);
        let species = self
            .data
            .selected_species
            .and_then(|s| self.data.ecology.observed_inspect(0, s));
        let species_line = species
            .map(|s| {
                format!(
                    "{} | Study{} ticks | Captive{}",
                    s["name"].as_str().unwrap_or("Observed species"),
                    s["study_ticks"],
                    s["captive_count"]
                )
            })
            .unwrap_or_else(|| {
                "TAB cycles observed animals; Inspect identifies a visible animal.".into()
            });
        let status = if self.data.winner.is_some() {
            format!(
                "{} | Winner faction{}",
                self.data.outcome,
                self.data.winner.unwrap()
            )
        } else {
            self.data.status.clone()
        };
        HudModel{ui:None,lines:vec!["LAST WORLD | Native connected Milestone B slice | 4.8 x 3.2 km".into(),format!("Metal{}  Timber{}  Food{}  Power{} | Ship{} | Your pawns in video{}",resources.metal,resources.timber,resources.rations,resources.power,self.data.ship_health[0],living),format!("{}x  {}  {}s  Debt{} ticks | {}",self.data.clock.speed().multiplier(),if self.data.paused{"PAUSED"}else{"RUNNING"},self.data.clock.tick()/20,self.data.clock.pending_ticks(),clock.season_name()),format!("Your attributed ecological damage{} | Completed jobs{} | Captive animals{}",self.data.ecological_damage[0],self.data.completed_jobs,self.data.ecology.captives.iter().filter(|c|c.team==0).count()),species_line,status],tools:BUILD_TOOLS.iter().map(|k|k.name().to_owned()).chain(ACTION_TOOLS.iter().map(|s|s.to_string())).collect(),selected_tool:self.data.selected_tool,footer:vec!["WASD pan | Wheel zoom | Right click rally | Left click selected tool".into(),"SPACE pause | F1/F2/F3 1x/2x/4x | TAB species | F5 save | F9 load".into(),"Cameras provide video. Radar contacts are unidentified. Satellite sweeps every3minutes.".into(),"Capture needs a pen/handler; research needs a cared-for delivered specimen and research building.".into()]}
    }
    pub fn validate(&self) -> Result<(), String> {
        if let Some(memory) = &self.data.visual_memory { memory.validate()?; }
        if self.data.processed_tick > self.data.clock.tick()
            || self.data.clock.remainder_nanos() >= TICK_NANOS
        {
            return Err("Invalid campaign clock handoff".into());
        }
        self.validate_unit_order_queues()?;
        self.data.ecology.validate()?;
        self.data.tactics.validate()?;
        self.data.construction.validate()?;
        self.data.support.validate()?;
        self.data.habitats.validate().map_err(|e| e.to_string())?;
        self.data.physical.validate()?;
        self.data.recon.validate()?;
        self.data
            .river
            .water
            .validate()
            .map_err(|e| e.to_string())?;
        if !self.data.river.ground.totals().balanced()
            || !self
                .data
                .river
                .ground
                .combined_water_balanced(&self.data.river.water)
        {
            return Err("Campaign physical stocks do not balance".into());
        }
        let accounted_water = self
            .data
            .base_water
            .iter()
            .try_fold(self.data.water_given_to_care, |sum, value| {
                sum.checked_add(*value)
            });
        if accounted_water != Some(self.data.initial_base_water) {
            return Err("Base care water ownership mismatch".into());
        }
        if self.data.last_cover.len() != 600
            || self.data.managed_per_patch.len() != 600
            || self.data.rain_remainders.len() != 12888
            || self.data.ground_cover_retained.len() != 600
            || self.data.flow_outgoing.len() != 600
            || self.data.flow_reference.len() != 600
            || self.data.initial_bio_per_patch.len() != 600
            || self.data.initial_cover_per_patch.len() != 600
            || self.data.loss_projection_remainder.len() != 600
            || self.data.last_river_bio.len() != 600
            || self.data.river_growth_credit.len() != 600
            || self.data.initial_managed_cover_per_patch.len() != 600
            || self.data.last_ground_foliage.len() != self.data.river.ground.cells().len()
            || self.data.ecology.root_zones.len() != 600
            || self.data.nav.width != 300
            || self.data.nav.height != 200
            || self.data.nav.cell_mm != 16_000
            || self.data.nav.cells.len() != 60_000
        {
            return Err("Invalid saved campaign field dimensions".into());
        }
        if self.data.camera.iter().any(|v| !v.is_finite())
            || !(0.12..=6.).contains(&self.data.camera[2])
            || !self.data.flash.is_finite()
            || self.data.flash_position.is_some_and(|p| {
                p.iter().any(|v| !v.is_finite())
                    || p[0] < 0.
                    || p[1] < 0.
                    || p[0] >= (CW * CELL_METERS) as f32
                    || p[1] >= (CH * CELL_METERS) as f32
            })
            || self.data.selected_tool >= BUILD_TOOLS.len() + ACTION_TOOLS.len()
            || self.data.winner.is_some_and(|team| team > 1)
            || self.data.selected_species.is_some_and(|species| {
                !self.data.ecology.knowledge[0]
                    .get(species as usize)
                    .is_some_and(|k| k.observed)
            })
            || self.data.rain_remainders.iter().any(|r| *r >= 256)
            || self.data.last_river_bio.iter().any(|r| *r > 1000)
            || self.data.ground_cover_retained.iter().any(|r| *r > 1000)
            || self.data.managed_per_patch.iter().any(|n| *n > 400)
            || self
                .data
                .loss_projection_remainder
                .iter()
                .zip(&self.data.initial_cover_per_patch)
                .any(|(remainder, capacity)| {
                    if *capacity == 0 {
                        *remainder != 0
                    } else {
                        *remainder >= *capacity
                    }
                })
        {
            return Err("Invalid saved campaign projection or timing state".into());
        }
        let mut unique_links = BTreeSet::new();
        for &(link, a, b) in &self.data.flow_boundaries {
            let edge = self
                .data
                .river
                .water
                .links()
                .get(link)
                .ok_or("Invalid saved flow link")?;
            let from = RiverFixture::global_index(edge.a).ok_or("Invalid flow source")?;
            let to = RiverFixture::global_index(edge.b).ok_or("Invalid flow destination")?;
            let patch =
                |cell: usize| patch_at([((cell % CW) * 8) as f32, ((cell / CW) * 8) as f32]);
            if a == b || a != patch(from) || b != patch(to) || !unique_links.insert(link) {
                return Err("Invalid saved macro flow mapping".into());
            }
        }
        let mut handlers = BTreeSet::new();
        let mut carried = BTreeSet::new();
        for order in &self.data.capture_orders {
            if order.team > 1
                || order.patch >= 600
                || order.species as usize >= self.data.ecology.species.len()
                || !handlers.insert(order.worker)
                || order.last_known_position.is_some_and(|p| {
                    p.iter().any(|v| !v.is_finite())
                        || p[0] < 0.
                        || p[1] < 0.
                        || p[0] >= (CW * CELL_METERS) as f32
                        || p[1] >= (CH * CELL_METERS) as f32
                        || order.releasing && patch_at(p) as u32 != order.patch
                })
                || order.releasing && order.handling_work_ticks != 0
                || !order.releasing
                    && order.captured.is_none()
                    && self
                        .data
                        .ecology
                        .capture_work_ticks(order.cohort)
                        .is_ok_and(|required| order.handling_work_ticks > required)
                || order.captured.is_some_and(|id| !carried.insert(id))
                || !self
                    .data
                    .tactics
                    .agent(order.worker)
                    .is_some_and(|a| a.team == order.team && a.role == Role::Worker)
            {
                return Err("Invalid saved specimen transport ownership".into());
            }
        }
        if self.data.detailed.values().any(|d| {
            d.last_seen > self.data.clock.tick()
                || d.position.iter().chain(&d.previous).any(|v| !v.is_finite())
        }) {
            return Err("Invalid saved detailed-animal projection".into());
        }
        Ok(())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"kind":"connected_native_milestone_B_small_map_not_production20km","tick":self.data.processed_tick,"requested_speed":self.data.clock.speed().multiplier(),"pending_ticks":self.data.clock.pending_ticks(),"map_meters":[4800,3200],"tactics":self.data.tactics.totals,"field_support":self.data.support.report(),"military_rules_present":true,"civilian_jobs_present":true,"ecology":self.data.ecology.census(),"physical":self.data.physical.report(),"water":self.data.river.describe(),"completed_jobs":self.data.completed_jobs,"biology_events":self.data.biological_events,"attributed_ecological_damage":self.data.ecological_damage,"ship_health":self.data.ship_health,"winner":self.data.winner,"all160_implemented":false,"integrated_performance_qualified":false})
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let state = CampaignSave {
            version: 1,
            terrain: self.terrain.snapshot(),
            data: self.data.clone(),
        };
        let result = campaign_support::save_atomic(path, &state)?;
        logging::log(
            "CAMPAIGN_SAVE",
            &format!("{}; {:?}", path.display(), result),
        );
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, String> {
        let state: CampaignSave = campaign_support::load(path)?;
        if state.version != 1 {
            return Err("Unsupported campaign state version".into());
        }
        let terrain = Terrain::restore(&state.terrain)?;
        let mut game = Self {
            terrain,
            data: state.data,
        };
        if game.data.visual_memory.is_none() {
            // Additive migration: unseen legacy terrain starts from its seeded
            // survey, never from the current hidden runtime state.
            let mut survey = Terrain::new(game.seed);
            let _ = RiverFixture::new(&mut survey)?;
            game.data.visual_memory = Some(crate::visual_memory::VisualMemory::survey(&survey));
        }
        game.validate()?;
        logging::log(
            "CAMPAIGN_LOAD",
            &format!("{}; tick{}", path.display(), game.clock.tick()),
        );
        Ok(game)
    }
}

#[cfg(test)]
mod custody_mortality_tests {
    use super::*;
    use crate::{ecology::EcologyPatch, ecosystem_physics::PhysicalActor};

    #[test]
    fn paid_blast_keeps_its_actual_origin_and_rejected_strike_cannot_move_it() {
        let mut game = Campaign::new(78_421, 2, 2).unwrap();
        let point = TEAM_BASES[0];
        assert!(game.recon.visible(0, point));
        let before = game.construction.stores[0];
        game.explode(point, 0).unwrap();
        assert_eq!(game.flash_position, Some(point));
        assert_eq!(game.flash, 0.);
        assert_eq!(
            game.construction.stores[0],
            before.checked_sub(Resources::new(25, 0, 0, 20)).unwrap()
        );
        assert!(game.explode([-1000., -1000.], 0).is_err());
        assert_eq!(game.flash_position, Some(point));
        game.validate().unwrap();
    }

    fn capture_existing(game: &mut Campaign, drain_reserve: bool) -> (u64, u32, u64) {
        let source = game
            .data
            .ecology
            .cohorts
            .iter()
            .find(|c| c.count > 1 && c.surface_observable && c.location_height_mm >= 0)
            .unwrap()
            .id;
        let id = game.data.ecology.materialize_one(source).unwrap();
        let animal = game
            .data
            .ecology
            .cohorts
            .iter()
            .find(|c| c.id == id)
            .unwrap();
        let patch = animal.patch;
        let species = animal.species_id;
        let energy = animal.energy;
        if drain_reserve {
            game.data.ecology.spend_activity_energy(id, energy).unwrap();
        }
        game.data.ecology.observe(0, patch, true).unwrap();
        assert_eq!(game.data.ecology.capture_cohort(id, 0).unwrap(), id);
        let worker = game
            .data
            .tactics
            .agents
            .iter()
            .find(|a| a.team == 0 && a.role == Role::Worker && a.alive())
            .unwrap()
            .id;
        (id, species, worker)
    }
    fn transport(
        id: u64,
        species: u32,
        worker: u64,
        releasing: bool,
        picked_up: bool,
    ) -> CaptureOrder {
        CaptureOrder {
            worker,
            team: 0,
            species,
            patch: 0,
            captured: Some(id),
            releasing,
            picked_up,
            pen: u64::MAX,
            cohort: id,
            handling_work_ticks: 0,
            last_known_position: Some([100., 100.]),
        }
    }

    #[test]
    fn killed_handler_releases_same_animal_wallet_and_detail_at_actual_carrier_position() {
        let mut game = Campaign::new(78421, 2, 2).unwrap();
        let (id, species, worker) = capture_existing(&mut game, false);
        let original_patch = game
            .data
            .ecology
            .captives
            .iter()
            .find(|c| c.animal.id == id)
            .unwrap()
            .animal
            .patch;
        let old = [40_000, 40_000];
        // A derived tracked actor for the real captured identity. Its finite
        // mineral wallet is withdrawn from the existing physical patch stock.
        game.data.physical.patches[original_patch as usize].mineral_units -= 3;
        let actor: PhysicalActor = serde_json::from_value(serde_json::json!({
            "organism":id,"patch":original_patch,"position_mm":old,
            "materials":{"soil_mm3":0,"wood_units":0,"fiber_units":0,"mineral_units":3},
            "labor_ticks":12,"last_work_tick":0,"local_route":[],"route_cursor":0,
            "route_worksite":null,"route_terrain_revision":0,"route_water_revision":0,"travel_energy_mm":0
        })).unwrap();
        game.data.physical.actors.insert(id, actor);
        game.data.physical.validate().unwrap();
        let position_mm = game.data.tactics.agent(worker).unwrap().position_mm;
        assert_ne!(position_mm, old);
        let expected = [position_mm[0] as f32 / 1000., position_mm[1] as f32 / 1000.];
        let animals = game.data.ecology.census();
        let ledger = serde_json::to_value(&game.data.ecology.ledger).unwrap();
        // A rejected physical owner must leave ecology custody intact.
        let patches = std::mem::take(&mut game.data.physical.patches);
        assert!(game.release_carried_animal_at(id, position_mm).is_err());
        assert!(game.data.ecology.captives.iter().any(|c| c.animal.id == id));
        assert_eq!(game.data.physical.actors[&id].position_mm, old);
        game.data.physical.patches = patches;
        // These persisted fields are authoritative after save/load. Reject
        // off-world release points and impossible pre-earned handling work.
        let mut bad = transport(id, species, worker, true, false);
        bad.last_known_position = Some([f32::INFINITY, 0.]);
        game.data.capture_orders.push(bad);
        assert!(game.validate().is_err());
        game.data.capture_orders.clear();
        let mut bad = transport(id, species, worker, true, false);
        bad.last_known_position = Some(patch_center(1));
        game.data.capture_orders.push(bad);
        assert!(game.validate().is_err());
        game.data.capture_orders.clear();
        let mut bad = transport(id, species, worker, false, false);
        bad.captured = None;
        bad.cohort = game
            .data
            .ecology
            .cohorts
            .iter()
            .find(|c| c.count > 0)
            .unwrap()
            .id;
        bad.handling_work_ticks = u64::MAX;
        game.data.capture_orders.push(bad);
        assert!(game.validate().is_err());
        game.data.capture_orders.clear();
        game.data
            .capture_orders
            .push(transport(id, species, worker, false, false));
        game.data.tactics.damage(worker, u16::MAX, None).unwrap();
        game.update_capture_orders().unwrap();
        assert!(game.data.capture_orders.is_empty());
        assert!(!game.data.ecology.captives.iter().any(|c| c.animal.id == id));
        let animal = game
            .data
            .ecology
            .cohorts
            .iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(animal.patch as usize, patch_at(expected));
        assert_eq!(game.cohort_position(animal), expected);
        assert_eq!(game.data.detailed[&id].position, expected);
        assert_eq!(game.data.detailed[&id].previous, expected);
        assert_eq!(game.data.physical.actors[&id].position_mm, position_mm);
        assert_eq!(game.data.physical.actors[&id].materials.mineral_units, 3);
        assert_eq!(game.data.physical.actors[&id].labor_ticks, 0);
        let after = game.data.ecology.census();
        assert_eq!(
            after.wild_animals + after.captive_animals,
            animals.wild_animals + animals.captive_animals
        );
        assert_eq!(
            serde_json::to_value(&game.data.ecology.ledger).unwrap(),
            ledger
        );
        game.validate().unwrap();
    }

    #[test]
    fn actual_captive_mortality_cancels_return_and_both_release_phases_without_respawn() {
        let mut game = Campaign::new(78421, 2, 2).unwrap();
        let (id, species, worker) = capture_existing(&mut game, true);
        // Exhausted reserves came from a real paid activity, then actual finite
        // physiology kills the uncared-for captive. No dead count is fabricated.
        let environment = vec![EcologyPatch::default(); game.data.ecology.root_zones.len()];
        for tick in (20..=800).step_by(20) {
            game.data.ecology.step(tick, &environment).unwrap();
        }
        assert!(!game.data.ecology.captives.iter().any(|c| c.animal.id == id));
        assert!(
            game.data
                .ecology
                .ledger
                .death_reasons
                .iter()
                .any(|(reason, count)| reason.starts_with("captive/") && *count > 0)
        );
        let before = serde_json::to_value(game.data.ecology.census()).unwrap();
        let ledger = serde_json::to_value(&game.data.ecology.ledger).unwrap();
        for (releasing, picked_up) in [(false, false), (true, false), (true, true)] {
            game.data
                .capture_orders
                .push(transport(id, species, worker, releasing, picked_up));
            game.update_capture_orders().unwrap();
            assert!(game.data.capture_orders.is_empty());
            assert!(
                !game
                    .data
                    .ecology
                    .cohorts
                    .iter()
                    .any(|c| c.id == id && c.count > 0)
            );
            assert_eq!(
                serde_json::to_value(game.data.ecology.census()).unwrap(),
                before
            );
            assert_eq!(
                serde_json::to_value(&game.data.ecology.ledger).unwrap(),
                ledger
            );
        }
        game.validate().unwrap();
    }
}
