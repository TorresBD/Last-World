//! Campaign bridge for finite biological engineering. Optional construction
//! intents are queued; metabolism/hydrology never wait for this bounded worker.
//! Water/animals/plants retain their existing owners. This resource owns only
//! loose construction material, geological reserves and unspent labor time.
use crate::{
    ecology::{Custody, Ecology, EcologyPatch, PhysicalRequest, Stage},
    habitat::{
        HabitatError, HabitatEvent, HabitatWorld, Materials, Occupancy, ShelterKind, Site,
        SoilLayer, WorkBudget,
    },
    river::{ORIGIN, RiverFixture, WIDTH as RIVER_WIDTH},
    terrain::{CELL_METERS, CH, CW, DYNAMIC_WATER, Terrain},
    water::Barrier,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const INTERVAL: u64 = 20;
const ACTIVE_JOBS: usize = 4;
const STALLED_JOB_TICKS: u64 = 2400;
const MM3_PER_LITER: u64 = 1_000_000;
const CELL_MM3_PER_MM: u64 = (CELL_METERS * CELL_METERS) as u64 * MM3_PER_LITER;
const UNDERGROUND_PER_PATCH: u64 = 2_000_000_000;
const LAYER_MM3: u64 = 100_000_000;
const MINERAL_PER_PATCH: u64 = 256;
fn plus(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| "Physical ledger overflow".into())
}
fn key(id: u64, trait_id: u16) -> u128 {
    (u128::from(id) << 16) | u128::from(trait_id)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicalPatch {
    pub underground_mm3: u64,
    pub mineral_units: u64,
    pub upper: SoilLayer,
    pub lower: SoilLayer,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PhysicalActor {
    pub organism: u64,
    pub patch: u32,
    pub position_mm: [i32; 2],
    pub materials: Materials,
    pub labor_ticks: u64,
    pub last_work_tick: u64,
    local_route: Vec<usize>,
    route_cursor: usize,
    route_worksite: Option<usize>,
    route_terrain_revision: u64,
    route_water_revision: u64,
    travel_energy_mm: u64,
    #[serde(default)]
    last_progress_tick: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PendingEngineering {
    pub request: PhysicalRequest,
    pub queued_tick: u64,
    pub actor: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
struct SurfacePool {
    remaining_mm3: u64,
    floor_mm: i32,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EcosystemPhysics {
    pub patches: Vec<PhysicalPatch>,
    pub actors: BTreeMap<u64, PhysicalActor>,
    source_actor: BTreeMap<u64, u64>,
    observed_positions: BTreeMap<u64, [i32; 2]>,
    pending: BTreeMap<u128, PendingEngineering>,
    active: Vec<u128>,
    surface_pools: BTreeMap<usize, SurfacePool>,
    pub last_tick: u64,
    initial_soil_mm3: u64,
    initial_mineral_units: u64,
    surface_received_mm3: u64,
    bio_received_units: u64,
    bio_returned_units: u64,
    cavity_debris_units: u64,
    stored_habitat_material: Materials,
    committed: BTreeMap<u16, u64>,
    rejected: u64,
    stale_intents: u64,
    selected_cursor: u128,
    #[serde(default)]
    retry_after: BTreeMap<u128, u64>,
    #[serde(default)]
    stalled_releases: u64,
    pub dam_links: BTreeMap<u64, Vec<usize>>,
    pub dam_sites: BTreeMap<u64, usize>,
    pub reef_sites: BTreeMap<u64, usize>,
    hydraulic_base: BTreeMap<usize, Option<Barrier>>,
    hydraulic_last_applied: BTreeMap<usize, Option<Barrier>>,
    pub structure_positions: BTreeMap<u64, [i32; 2]>,
    #[serde(default)]
    nest_owners: BTreeMap<u64, u64>,
    trap_owners: BTreeMap<u64, u64>,
    trap_arrivals: Vec<TrapArrival>,
    next_trap_encounter: BTreeMap<u64, u64>,
    flood_exposure_ticks: BTreeMap<u64, u64>,
}
impl EcosystemPhysics {
    pub fn new(patch_count: usize) -> Self {
        let patches = (0..patch_count)
            .map(|_| PhysicalPatch {
                underground_mm3: UNDERGROUND_PER_PATCH,
                mineral_units: MINERAL_PER_PATCH,
                upper: SoilLayer {
                    mineral_mm3: LAYER_MM3,
                    nutrients: 0,
                    porosity_permille: 400,
                },
                lower: SoilLayer {
                    mineral_mm3: LAYER_MM3,
                    nutrients: 0,
                    porosity_permille: 600,
                },
            })
            .collect();
        Self {
            patches,
            actors: BTreeMap::new(),
            source_actor: BTreeMap::new(),
            observed_positions: BTreeMap::new(),
            pending: BTreeMap::new(),
            active: vec![],
            surface_pools: BTreeMap::new(),
            last_tick: 0,
            initial_soil_mm3: (patch_count as u64)
                .checked_mul(UNDERGROUND_PER_PATCH + LAYER_MM3 * 2)
                .expect("Physical world size overflow"),
            initial_mineral_units: (patch_count as u64)
                .checked_mul(MINERAL_PER_PATCH)
                .expect("Physical world size overflow"),
            surface_received_mm3: 0,
            bio_received_units: 0,
            bio_returned_units: 0,
            cavity_debris_units: 0,
            stored_habitat_material: Materials::default(),
            committed: BTreeMap::new(),
            rejected: 0,
            stale_intents: 0,
            selected_cursor: 0,
            retry_after: BTreeMap::new(),
            stalled_releases: 0,
            dam_links: BTreeMap::new(),
            dam_sites: BTreeMap::new(),
            reef_sites: BTreeMap::new(),
            hydraulic_base: BTreeMap::new(),
            hydraulic_last_applied: BTreeMap::new(),
            structure_positions: BTreeMap::new(),
            nest_owners: BTreeMap::new(),
            trap_owners: BTreeMap::new(),
            trap_arrivals: vec![],
            next_trap_encounter: BTreeMap::new(),
            flood_exposure_ticks: BTreeMap::new(),
        }
    }
    pub fn step(
        &mut self,
        tick: u64,
        requests: &[PhysicalRequest],
        ecology: &mut Ecology,
        habitats: &mut HabitatWorld,
        terrain: &mut Terrain,
        river: &mut RiverFixture,
        environment: &[EcologyPatch],
        patch_width: usize,
        patch_meters: u32,
    ) -> Result<(), String> {
        if tick
            != self
                .last_tick
                .checked_add(INTERVAL)
                .ok_or("Physical tick overflow")?
            || ecology.last_tick != tick
        {
            return Err("Physical bridge must process each due ecology tick once".into());
        }
        if patch_width == 0
            || patch_meters == 0
            || environment.len() != self.patches.len()
            || ecology.root_zones.len() != self.patches.len()
        {
            return Err("Physical patch layout mismatch".into());
        }
        self.synchronize_hosts(ecology, habitats)?;
        self.reconcile_hydraulics(habitats, terrain, river)?;
        self.retry_after.retain(|_, until| *until > tick);
        for request in requests {
            if request.patch as usize >= self.patches.len() {
                return Err("Engineering intent outside world".into());
            }
            let k = key(request.cohort_id, request.trait_id);
            self.pending.entry(k).or_insert_with(|| PendingEngineering {
                request: request.clone(),
                queued_tick: tick,
                actor: None,
            });
        }
        // No more than four real animals accrue labor at once. Unassigned
        // optional intents remain ordinary animals, feeding/migrating normally.
        let active_keys = self.active.clone();
        let mut blocked = Vec::new();
        for k in active_keys {
            let Some(job) = self.pending.get(&k).cloned() else {
                continue;
            };
            let Some(id) = job.actor else {
                blocked.push(k);
                continue;
            };
            let Some(c) = ecology
                .cohorts
                .iter()
                .find(|c| c.id == id && c.count > 0)
                .cloned()
            else {
                blocked.push(k);
                continue;
            };
            let actor = self
                .actors
                .get(&id)
                .ok_or("Missing active physical actor")?;
            if c.patch != job.request.patch
                || tick.saturating_sub(actor.last_progress_tick) >= STALLED_JOB_TICKS
            {
                blocked.push(k);
                self.stalled_releases = plus(self.stalled_releases, 1)?;
                continue;
            }
            let before_position = actor.position_mm;
            let before_labor = actor.labor_ticks;
            let species = &ecology.species[c.species_id as usize];
            let resting = (tick % 2400 < 1200) == species.nocturnal;
            let elapsed = tick - self.actors[&id].last_work_tick;
            if c.dormant_until <= tick
                && !resting
                && c.stamina_permille >= 100
                && c.health_permille > 0
                && c.patch == job.request.patch
            {
                if c.energy < 30 {
                    blocked.push(k);
                } else {
                    let progress = if job.request.trait_id == 65
                        && occupied_den_room(habitats, id).is_some()
                    {
                        Some(elapsed)
                    } else {
                        self.walk_to_worksite(
                            id,
                            &job.request,
                            elapsed,
                            ecology,
                            terrain,
                            river,
                            patch_width,
                            patch_meters,
                        )?
                    };
                    match progress {
                        Some(unused_ticks) => {
                            let a = self.actors.get_mut(&id).unwrap();
                            a.labor_ticks = plus(a.labor_ticks, unused_ticks)?.min(600);
                            let c = ecology.cohorts.iter_mut().find(|c| c.id == id).unwrap();
                            c.stamina_permille = c.stamina_permille.saturating_sub(10);
                            c.engineering_busy_until = plus(plus(tick, INTERVAL)?, 1)?;
                        }
                        None => blocked.push(k),
                    }
                }
            }
            let actor = self.actors.get_mut(&id).unwrap();
            if actor.position_mm != before_position || actor.labor_ticks > before_labor {
                actor.last_progress_tick = tick;
            }
            actor.last_work_tick = tick;
        }
        for k in blocked {
            self.retry_after.insert(k, plus(tick, STALLED_JOB_TICKS)?);
            if let Some(job) = self.pending.remove(&k) {
                if let Some(id) = job.actor {
                    if let Some(c) = ecology.cohorts.iter_mut().find(|c| c.id == id) {
                        c.engineering_busy_until = tick;
                    }
                }
            }
            self.active.retain(|&id| id != k);
            self.rejected = plus(self.rejected, 1)?;
        }
        let active = std::mem::take(&mut self.active);
        for k in active {
            let Some(job) = self.pending.get(&k).cloned() else {
                continue;
            };
            match self.execute(
                &job,
                ecology,
                habitats,
                terrain,
                river,
                environment,
                patch_width,
                patch_meters,
            )? {
                Execution::Waiting => self.active.push(k),
                Execution::Committed => {
                    self.pending.remove(&k);
                }
                Execution::Rejected => {
                    self.pending.remove(&k);
                    self.rejected = plus(self.rejected, 1)?;
                    if let Some(id) = job.actor {
                        if let Some(c) = ecology.cohorts.iter_mut().find(|c| c.id == id) {
                            c.engineering_busy_until = tick;
                        }
                    }
                }
            }
        }
        // A bounded scan, with a persistent cursor, avoids one blocked intent
        // starving every later patch. No simulation quantity is discarded.
        let mut candidates: Vec<u128> = self
            .pending
            .range((
                std::ops::Bound::Excluded(self.selected_cursor),
                std::ops::Bound::Unbounded,
            ))
            .map(|(&k, _)| k)
            .take(64)
            .collect();
        if candidates.len() < 64 {
            candidates.extend(
                self.pending
                    .range(..=self.selected_cursor)
                    .map(|(&k, _)| k)
                    .take(64 - candidates.len()),
            );
        }
        for k in candidates {
            if self.active.len() >= ACTIVE_JOBS {
                break;
            }
            self.selected_cursor = k;
            if self.retry_after.get(&k).is_some_and(|until| *until > tick) {
                continue;
            }
            if self.active.contains(&k) {
                continue;
            }
            let Some(job) = self.pending.get(&k).cloned() else {
                continue;
            };
            let mapped = self
                .source_actor
                .get(&job.request.cohort_id)
                .copied()
                .unwrap_or(job.request.cohort_id);
            if let Some(o) = habitats.occupants.get(&mapped) {
                let eligible = match o.location {
                    Occupancy::Underground(_) => false,
                    Occupancy::Shelter(_) => {
                        job.request.trait_id == 65 && occupied_den_room(habitats, mapped).is_some()
                    }
                    Occupancy::Trap(_) => false,
                    Occupancy::Canopy(_) => matches!(job.request.trait_id, 11 | 65 | 103 | 104),
                };
                if !eligible {
                    self.pending.remove(&k);
                    self.rejected = plus(self.rejected, 1)?;
                    continue;
                }
            }
            if self.active.iter().any(|key| {
                self.pending
                    .get(key)
                    .is_some_and(|j| j.actor == Some(mapped))
            }) {
                continue;
            }
            let candidate = ecology
                .cohorts
                .iter()
                .filter(|c| {
                    (c.id == mapped || c.id == job.request.cohort_id)
                        && c.count > 0
                        && c.patch == job.request.patch
                        && c.species_id == job.request.species_id
                        && c.stage == Stage::Adult
                        && ecology.species[c.species_id as usize]
                            .traits
                            .has(job.request.trait_id)
                })
                .min_by_key(|c| c.id != mapped)
                .cloned();
            let Some(candidate) = candidate else {
                self.pending.remove(&k);
                self.stale_intents = plus(self.stale_intents, 1)?;
                continue;
            };
            let species = &ecology.species[candidate.species_id as usize];
            if candidate.dormant_until > tick
                || candidate.stamina_permille < 100
                || candidate.health_permille == 0
                || candidate.energy < 30
                || (tick % 2400 < 1200) == species.nocturnal
            {
                continue;
            }
            let nested = (job.request.trait_id == 65)
                .then(|| occupied_den_room(habitats, candidate.id))
                .flatten();
            let (mut target, _) = if let Some(room) = nested {
                let point = habitats.nodes[&room].position;
                let cell = cell_at([point.x, point.y]).ok_or("Den site outside terrain")?;
                (cell, cell_center(cell))
            } else {
                self.worksite(&job.request, terrain, river, patch_width, patch_meters)?
            };
            if matches!(job.request.trait_id, 12 | 64) {
                if let Some(entrance) = habitats.nodes.values().find(|node| {
                    node.patch == candidate.patch
                        && node.kind == crate::habitat::NodeKind::Entrance
                        && !node.collapsed
                }) {
                    target = cell_at([entrance.position.x, entrance.position.y])
                        .ok_or("Existing entrance outside terrain")?;
                } else if job.request.trait_id == 64 {
                    continue;
                }
            }
            if matches!(job.request.trait_id, 11 | 103 | 104)
                && ecology.canopy_permille(candidate.patch) == 0
            {
                continue;
            }
            if nested.is_none() && !worksite_usable(target, job.request.trait_id, terrain, river) {
                continue;
            }
            let known = self
                .actors
                .get(&candidate.id)
                .map(|a| a.position_mm)
                .or_else(|| self.observed_positions.get(&candidate.id).copied());
            let (position, route) = if nested.is_some() {
                (known.unwrap_or(cell_center(target)), vec![])
            } else if let Some(position) = known {
                let Some(start) = cell_at(position) else {
                    continue;
                };
                let Some(route) = local_route(
                    start,
                    target,
                    candidate.patch,
                    job.request.trait_id,
                    species,
                    terrain,
                    river,
                    patch_width,
                    patch_meters,
                ) else {
                    continue;
                };
                (position, route)
            } else {
                let Some(seed) = cohort_fine_origin(
                    candidate.patch,
                    target,
                    job.request.trait_id,
                    species,
                    terrain,
                    river,
                    patch_width,
                    patch_meters,
                ) else {
                    continue;
                };
                seed
            };
            let id = if candidate.count == 1 {
                candidate.id
            } else {
                ecology.materialize_one(candidate.id)?
            };
            self.source_actor.insert(job.request.cohort_id, id);
            self.observed_positions.remove(&id);
            self.actors.entry(id).or_insert(PhysicalActor {
                organism: id,
                patch: job.request.patch,
                position_mm: position,
                materials: Materials::default(),
                labor_ticks: 0,
                last_work_tick: tick,
                local_route: vec![],
                route_cursor: 0,
                route_worksite: None,
                route_terrain_revision: 0,
                route_water_revision: 0,
                travel_energy_mm: 0,
                last_progress_tick: tick,
            });
            self.pending.get_mut(&k).unwrap().actor = Some(id);
            self.actors.get_mut(&id).unwrap().last_work_tick = tick;
            self.actors.get_mut(&id).unwrap().labor_ticks = 0;
            let actor = self.actors.get_mut(&id).unwrap();
            actor.route_worksite = Some(target);
            actor.local_route = route;
            actor.route_cursor = 0;
            actor.route_terrain_revision = terrain.revision();
            actor.route_water_revision = river.water.transport_generation();
            actor.last_progress_tick = tick;
            if let Some(c) = ecology.cohorts.iter_mut().find(|c| c.id == id) {
                c.engineering_busy_until = plus(plus(tick, INTERVAL)?, 1)?;
            }
            self.active.push(k);
        }
        self.reconcile_hydraulics(habitats, terrain, river)?;
        // Harvesting or cavity/thinning work above can remove the last actual
        // supporting tissue during this same interval.
        self.synchronize_hosts(ecology, habitats)?;
        self.synchronize_hazards(
            tick,
            ecology,
            habitats,
            terrain,
            river,
            patch_width,
            patch_meters,
        )?;
        self.leave_hungry_refuges(tick, ecology, habitats)?;
        self.advance_traps(tick, ecology, habitats)?;
        self.attach_real_clutches(ecology, habitats)?;
        self.synchronize_breeding_refuges(ecology, habitats)?;
        let living_fine: BTreeSet<u64> = ecology
            .cohorts
            .iter()
            .filter(|c| c.count == 1)
            .map(|c| c.id)
            .collect();
        self.observed_positions
            .retain(|id, _| living_fine.contains(id));
        self.stored_habitat_material = habitat_materials(habitats)?;
        self.last_tick = tick;
        self.validate()?;
        ecology.validate()?;
        habitats.validate().map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Reconcile the one aggregate canopy host per macro patch with its
    /// biological owner. This is intentionally not a per-tree simulation:
    /// partial thinning changes the ecological canopy field, while the shared
    /// host finally fails when its actual living canopy shoot/leaf stock is0.
    /// No standing biomass is exported again on failure; Ecology already owns
    /// the biological harvest/death transaction. Only existing built shelter
    /// material moves to HabitatWorld debris.
    pub fn synchronize_hosts(
        &mut self,
        ecology: &mut Ecology,
        habitats: &mut HabitatWorld,
    ) -> Result<(), String> {
        let mut living_canopy = BTreeMap::<u32, u64>::new();
        for plant in &ecology.plants {
            if plant.individuals > 0 && ecology.species[plant.species_id as usize].traits.has(157) {
                let current = living_canopy.entry(plant.patch).or_default();
                *current = plus(*current, plus(plant.shoots, plant.leaves)?)?;
            }
        }
        let hosts: Vec<_> = habitats
            .supports
            .values()
            .filter(|s| s.id == 0x8000_0000_0000_0000u64 + u64::from(s.patch))
            .map(|s| (s.id, s.patch, s.position, s.host_alive))
            .collect();
        for (host, patch, position, alive) in hosts {
            let biomass = living_canopy.get(&patch).copied().unwrap_or(0);
            if !alive {
                // Real regrowth can support a new canopy on this aggregate
                // reference. Destroyed cavities/nests stay destroyed and all
                // old links remain severed; no structure or clutch is rescued.
                if biomass >= 10 {
                    let support = habitats.supports.get_mut(&host).unwrap();
                    support.host_alive = true;
                    support.capacity = u32::try_from((biomass / 10).min(u64::from(u32::MAX)))
                        .map_err(|_| "Regrown host capacity overflow")?;
                    habitats.revision = plus(habitats.revision, 1)?;
                }
                continue;
            }
            if biomass > 0 {
                continue;
            }
            let events = habitats.destroy_host(host).map_err(|e| e.to_string())?;
            for event in events {
                match event {
                    HabitatEvent::OccupantExposed {
                        organism, fall_mm, ..
                    } => {
                        let Some(index) = ecology
                            .cohorts
                            .iter()
                            .position(|c| c.id == organism && c.patch == patch && c.count > 0)
                        else {
                            // Dead/captured/migrated references cannot injure a
                            // different animal or duplicate a previous death.
                            continue;
                        };
                        let c = &ecology.cohorts[index];
                        let can_land = ecology.species[c.species_id as usize]
                            .locomotion_for(c.stage)
                            == crate::ecology::Locomotion::Flying;
                        let fall_damage = if can_land {
                            0
                        } else {
                            fall_mm.saturating_sub(1000).div_ceil(100).min(1000) as u16
                        };
                        ecology.cohorts[index].surface_observable = true;
                        ecology.cohorts[index].location_height_mm = 0;
                        ecology.cohorts[index].health_permille = ecology.cohorts[index]
                            .health_permille
                            .saturating_sub(fall_damage);
                        if let Some(actor) = self.actors.get_mut(&organism) {
                            actor.position_mm = [position.x, position.y];
                        }
                        if ecology.cohorts[index].health_permille == 0 {
                            let count = ecology.cohorts[index].count;
                            ecology.damage_cohort(organism, count)?;
                        }
                    }
                    HabitatEvent::ClutchExposed { clutch, .. } => {
                        if ecology.clutches.iter().any(|c| c.id == clutch) {
                            ecology.destroy_clutch(clutch)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        self.stored_habitat_material = habitat_materials(habitats)?;
        self.synchronize_breeding_refuges(ecology, habitats)?;
        Ok(())
    }

    /// Project exact physical reservations, never a patch-wide capacity copied
    /// onto every mother. An owned nest is reserved to its actual builder; the
    /// normal path/work transaction must already have reached its local site.
    pub fn synchronize_breeding_refuges(
        &self,
        ecology: &mut Ecology,
        habitats: &HabitatWorld,
    ) -> Result<(), String> {
        let mut claims = BTreeMap::<u64, crate::ecology::BreedingRefuge>::new();
        let mut room_reservations = BTreeMap::<u64, u32>::new();
        for occupant in habitats.occupants.values() {
            let Occupancy::Shelter(id) = occupant.location else {
                continue;
            };
            let Some(shelter) = habitats.shelters.get(&id) else {
                continue;
            };
            if !matches!(shelter.kind, ShelterKind::Den | ShelterKind::Cavity)
                || shelter.integrity == 0
                || shelter.flooded
                || shelter
                    .host
                    .is_some_and(|id| !habitats.supports.get(&id).is_some_and(|s| s.host_alive))
                || shelter.burrow_node.is_some_and(|id| {
                    !habitats.nodes.get(&id).is_some_and(|n| {
                        !n.collapsed && n.flooded_depth_mm < occupant.body_width_mm
                    })
                })
            {
                continue;
            }
            if ecology
                .cohorts
                .iter()
                .any(|c| c.id == occupant.organism && c.count > 0 && c.patch == shelter.patch)
            {
                claims.insert(
                    occupant.organism,
                    crate::ecology::BreedingRefuge {
                        den_id: Some(id),
                        den_slots: occupant.group_size,
                        nest_id: None,
                        nest_slots: 0,
                        accessible: true,
                    },
                );
            }
        }
        for (&nest, &owner) in &self.nest_owners {
            let Some(shelter) = habitats.shelters.get(&nest) else {
                continue;
            };
            if shelter.kind != ShelterKind::Nest
                || shelter.integrity == 0
                || shelter.flooded
                || shelter.clutch.is_some()
            {
                continue;
            }
            let Some(c) = ecology.cohorts.iter().find(|c| {
                c.id == owner
                    && c.count > 0
                    && c.patch == shelter.patch
                    && c.stage == Stage::Adult
                    && c.sex != crate::ecology::Sex::Male
            }) else {
                continue;
            };
            let Some(actor) = self.actors.get(&owner) else {
                continue;
            };
            let Some(&position) = self.structure_positions.get(&nest) else {
                continue;
            };
            let Some(cell) = cell_at(position) else {
                continue;
            };
            if shelter.burrow_node.is_none() && !at_worksite(actor.position_mm, cell, 65) {
                continue;
            }
            let reachable = if let Some(room) = shelter.burrow_node {
                occupied_den_room(habitats, owner) == Some(room)
            } else if let Some(host) = shelter.host {
                habitats.supports.get(&host).is_some_and(|s| s.host_alive)
                    && (matches!(habitats.occupants.get(&owner).map(|o| o.location), Some(Occupancy::Canopy(id)) if id == host)
                        || ecology.species[c.species_id as usize].locomotion_for(c.stage)
                            == crate::ecology::Locomotion::Flying)
            } else {
                c.surface_observable && c.location_height_mm >= 0
            };
            if !reachable {
                continue;
            }
            let claim = claims
                .entry(owner)
                .or_insert(crate::ecology::BreedingRefuge {
                    den_id: None,
                    den_slots: 0,
                    nest_id: None,
                    nest_slots: 0,
                    accessible: true,
                });
            // Multiple constructed nests do not grant one mother duplicated
            // simultaneous reservations. The smallest persistent ID wins.
            if claim.nest_id.is_none() {
                let slots = if let Some(room) = shelter.burrow_node {
                    let reserved = room_reservations.entry(room).or_default();
                    let free = room_free_slots(habitats, room).saturating_sub(*reserved);
                    let slots = free.min(shelter.capacity);
                    *reserved = reserved.saturating_add(slots);
                    slots
                } else {
                    shelter.capacity
                };
                if slots == 0 {
                    continue;
                }
                claim.nest_id = Some(nest);
                claim.nest_slots = slots;
            }
        }
        let ids: Vec<_> = ecology.cohorts.iter().map(|c| c.id).collect();
        for id in ids {
            ecology.set_breeding_refuge(id, claims.remove(&id))?;
        }
        Ok(())
    }

    fn worksite(
        &self,
        request: &PhysicalRequest,
        terrain: &Terrain,
        river: &RiverFixture,
        width: usize,
        meters: u32,
    ) -> Result<(usize, [i32; 2]), String> {
        let sites = match request.trait_id {
            101 => Some(&self.dam_sites),
            105 => Some(&self.reef_sites),
            _ => None,
        };
        if let Some(sites) = sites {
            if let Some(&cell) = sites.values().find(|&&cell| {
                let x = (cell % CW) * CELL_METERS;
                let y = (cell / CW) * CELL_METERS;
                x / (meters as usize) == request.patch as usize % width
                    && y / (meters as usize) == request.patch as usize / width
            }) {
                return Ok((cell, cell_center(cell)));
            }
        }
        choose_cell(
            request.patch,
            request.trait_id,
            terrain,
            river,
            width,
            meters,
        )
    }
    /// Natural structures contribute independent obstruction layers. Damage can
    /// remove only its own layer; a weaker new animal dam cannot open a human dam.
    pub fn reconcile_hydraulics(
        &mut self,
        habitats: &HabitatWorld,
        terrain: &Terrain,
        river: &mut RiverFixture,
    ) -> Result<(), String> {
        let mut layers: BTreeMap<usize, Vec<Barrier>> = BTreeMap::new();
        for (dam_id, links) in &self.dam_links {
            let Some(dam) = habitats.dams.get(dam_id) else {
                continue;
            };
            for &link in links {
                if link >= river.water.links().len() {
                    return Err("Saved animal dam hydraulic link outofrange".into());
                }
                if dam.integrity > 0 && dam.invested_wood_units > 0 {
                    layers.entry(link).or_default().push(Barrier {
                        crest_mm: dam.crest_mm,
                        permeability_permille: dam.permeability_permille(),
                    });
                } else {
                    layers.entry(link).or_default();
                }
            }
        }
        for (reef_id, &cell) in &self.reef_sites {
            let Some(reef) = habitats.reefs.get(reef_id) else {
                continue;
            };
            let Some(local) = RiverFixture::local_index(cell) else {
                continue;
            };
            let crest = terrain
                .elevation_mm(cell)
                .map_err(|e| e.to_string())?
                .checked_add(
                    i32::try_from(reef.solid_height_mm).map_err(|_| "Reef crest overflow")?,
                )
                .ok_or("Reef crest overflow")?;
            for (link, edge) in river
                .water
                .links()
                .iter()
                .enumerate()
                .filter(|(_, edge)| edge.a == local || edge.b == local)
            {
                let _ = edge;
                if reef.solid_height_mm > 0 {
                    layers.entry(link).or_default().push(Barrier {
                        crest_mm: crest,
                        permeability_permille: 700,
                    });
                } else {
                    layers.entry(link).or_default();
                }
            }
        }
        let mut edits = Vec::new();
        for (link, owned) in layers {
            let actual = river.water.links()[link].barrier;
            if self
                .hydraulic_last_applied
                .get(&link)
                .is_some_and(|&old| old != actual)
            {
                self.hydraulic_base.insert(link, actual);
            }
            let base = *self.hydraulic_base.entry(link).or_insert(actual);
            let mut combined = base;
            for b in owned {
                combined = Some(combined.map_or(b, |old| Barrier {
                    crest_mm: old.crest_mm.max(b.crest_mm),
                    permeability_permille: old.permeability_permille.min(b.permeability_permille),
                }));
            }
            edits.push((link, combined));
        }
        for (link, barrier) in edits {
            river
                .water
                .set_barrier(link, barrier)
                .map_err(|e| e.to_string())?;
            self.hydraulic_last_applied.insert(link, barrier);
        }
        Ok(())
    }
    /// Before physical control starts, the campaign can hand over an already
    /// observed animal's true fine position. Existing physical actors never reset.
    pub fn observe_position(&mut self, id: u64, position_mm: [i32; 2]) -> Result<(), String> {
        if id == 0 || cell_at(position_mm).is_none() {
            return Err("Observed physical position outside resident world".into());
        }
        if !self.actors.contains_key(&id) {
            self.observed_positions.insert(id, position_mm);
        }
        Ok(())
    }

    /// Explicit custody-to-world handoff, called only after the handler really
    /// arrives and Ecology releases this existing identity at the verified
    /// point. The caller supplies its matching authoritative macro patch.
    /// This is deliberately the only API that may override an existing known
    /// position: ordinary camera observation never teleports an actor.
    pub fn transport_completed(
        &mut self,
        organism: u64,
        patch: u32,
        position_mm: [i32; 2],
        habitats: &mut HabitatWorld,
    ) -> Result<(), String> {
        if organism == 0 || patch as usize >= self.patches.len() || cell_at(position_mm).is_none() {
            return Err("Invalid physical custody handoff identity, patch or position".into());
        }
        // Preflight the derived total before touching either owner's records.
        // Releasing occupancy does not remove a structure, eggs or material.
        let material = habitat_materials(habitats)?;
        if habitats.occupants.contains_key(&organism) {
            habitats
                .release_occupant(organism)
                .map_err(|e| e.to_string())?;
        }
        let mut sources: BTreeSet<u64> = self
            .source_actor
            .iter()
            .filter_map(|(&source, &actor)| (actor == organism).then_some(source))
            .collect();
        sources.insert(organism);
        let cancelled: BTreeSet<u128> = self
            .pending
            .iter()
            .filter_map(|(&key, job)| {
                (sources.contains(&job.request.cohort_id) || job.actor == Some(organism))
                    .then_some(key)
            })
            .collect();
        self.pending.retain(|key, _| !cancelled.contains(key));
        self.active.retain(|key| !cancelled.contains(key));
        self.retry_after
            .retain(|key, _| !sources.contains(&((*key >> 16) as u64)));
        self.source_actor
            .retain(|&source, actor| source != organism && *actor != organism);
        self.nest_owners.retain(|_, owner| *owner != organism);
        self.trap_owners.retain(|_, owner| *owner != organism);
        self.trap_arrivals
            .retain(|arrival| arrival.source != organism);
        self.flood_exposure_ticks.remove(&organism);
        if let Some(actor) = self.actors.get_mut(&organism) {
            actor.patch = patch;
            actor.position_mm = position_mm;
            // Work already spent on an abandoned old-site intent cannot be
            // redeemed as work at a new site. Wallet material and the fractional
            // movement-energy debit survive: neither is refunded or minted.
            actor.labor_ticks = 0;
            actor.local_route.clear();
            actor.route_cursor = 0;
            actor.route_worksite = None;
            actor.route_terrain_revision = 0;
            actor.route_water_revision = 0;
            actor.last_work_tick = self.last_tick;
            actor.last_progress_tick = self.last_tick;
            self.observed_positions.remove(&organism);
        } else {
            // A known fine position is not another animal or a free engineering
            // actor. Future actual work still needs a live Ecology owner.
            self.observed_positions.insert(organism, position_mm);
        }
        self.stored_habitat_material = material;
        Ok(())
    }
    /// Returns unused current elapsed ticks only after the animal reaches a real
    /// work stand-site. Traveling ticks never become construction labor credit.
    fn walk_to_worksite(
        &mut self,
        id: u64,
        request: &PhysicalRequest,
        elapsed: u64,
        ecology: &mut Ecology,
        terrain: &Terrain,
        river: &RiverFixture,
        width: usize,
        meters: u32,
    ) -> Result<Option<u64>, String> {
        let c = ecology
            .cohorts
            .iter()
            .find(|c| c.id == id && c.count > 0)
            .ok_or("Missing walking actor")?
            .clone();
        let species = ecology.species[c.species_id as usize].clone();
        let target = self
            .actors
            .get(&id)
            .and_then(|a| a.route_worksite)
            .map(Ok)
            .unwrap_or_else(|| {
                self.worksite(request, terrain, river, width, meters)
                    .map(|s| s.0)
            })?;
        if !worksite_usable(target, request.trait_id, terrain, river) {
            return Ok(None);
        }
        let actor = self
            .actors
            .get_mut(&id)
            .ok_or("Missing physical walk state")?;
        if at_worksite(actor.position_mm, target, request.trait_id) {
            actor.route_worksite = Some(target);
            return Ok(Some(elapsed));
        }
        let start = cell_at(actor.position_mm).ok_or("Physical actor position outside terrain")?;
        if actor.route_worksite != Some(target)
            || actor.route_cursor >= actor.local_route.len()
            || !remaining_route_usable(
                start,
                &actor.local_route[actor.route_cursor..],
                &species,
                terrain,
                river,
            )
        {
            let Some(mut route) = local_route(
                start,
                target,
                request.patch,
                request.trait_id,
                &species,
                terrain,
                river,
                width,
                meters,
            ) else {
                return Ok(None);
            };
            // BFS reasons in cells. Entering the goal cell is not arriving at
            // its center: retain a real final paid leg when replanning inside
            // that cell, including after a save or unrelated water update.
            if route.is_empty() && !at_worksite(actor.position_mm, target, request.trait_id) {
                route.push(start);
            }
            actor.local_route = route;
            actor.route_cursor = 0;
            actor.route_worksite = Some(target);
            actor.route_terrain_revision = terrain.revision();
            actor.route_water_revision = river.water.transport_generation();
        }
        let mut left = elapsed;
        let mut moved = 0u64;
        while left > 0 && actor.route_cursor < actor.local_route.len() {
            let next = actor.local_route[actor.route_cursor];
            let dest = cell_center(next);
            let dx = i64::from(dest[0]) - i64::from(actor.position_mm[0]);
            let dy = i64::from(dest[1]) - i64::from(actor.position_mm[1]);
            let distance =
                (i128::from(dx) * i128::from(dx) + i128::from(dy) * i128::from(dy)) as u128;
            let distance = distance.isqrt() as u64;
            if distance == 0 {
                actor.route_cursor += 1;
                continue;
            }
            let base = if species.locomotion == crate::ecology::Locomotion::Flying {
                150u64
            } else if species.slow_movement {
                50
            } else {
                100
            };
            let cell = terrain.cells[next];
            let speed = if cell.flags & 1 != 0 && water_depth(next, terrain, river) <= 300 {
                base * 2
            } else {
                base * (1000 - (cell.cover.max(0.).min(255.) as u64) * 2) / 1000
            }
            .max(20);
            let ticks = distance.div_ceil(speed);
            if ticks <= left {
                actor.position_mm = dest;
                actor.route_cursor += 1;
                left -= ticks;
                moved = plus(moved, distance)?;
            } else {
                let amount = speed
                    .checked_mul(left)
                    .ok_or("Walking displacement overflow")?
                    .min(distance);
                actor.position_mm[0] = i32::try_from(
                    i64::from(actor.position_mm[0])
                        + dx * i64::try_from(amount).map_err(|_| "Walk overflow")?
                            / distance as i64,
                )
                .map_err(|_| "Walk position overflow")?;
                actor.position_mm[1] = i32::try_from(
                    i64::from(actor.position_mm[1])
                        + dy * i64::try_from(amount).map_err(|_| "Walk overflow")?
                            / distance as i64,
                )
                .map_err(|_| "Walk position overflow")?;
                moved = plus(moved, amount)?;
                left = 0;
            }
        }
        let energy_mm = plus(actor.travel_energy_mm, moved)?;
        let energy = energy_mm / 10_000;
        actor.travel_energy_mm = energy_mm % 10_000;
        ecology.spend_activity_energy(id, energy)?;
        if at_worksite(actor.position_mm, target, request.trait_id) {
            Ok(Some(left))
        } else {
            Ok(Some(0))
        }
    }
    fn execute(
        &mut self,
        job: &PendingEngineering,
        ecology: &mut Ecology,
        habitats: &mut HabitatWorld,
        terrain: &mut Terrain,
        river: &mut RiverFixture,
        environment: &[EcologyPatch],
        patch_width: usize,
        patch_meters: u32,
    ) -> Result<Execution, String> {
        let Some(id) = job.actor else {
            return Ok(Execution::Rejected);
        };
        let Some(c) = ecology
            .cohorts
            .iter()
            .find(|c| {
                c.id == id
                    && c.count == 1
                    && c.patch == job.request.patch
                    && c.species_id == job.request.species_id
            })
            .cloned()
        else {
            return Ok(Execution::Rejected);
        };
        let species = &ecology.species[c.species_id as usize];
        if !species.traits.has(job.request.trait_id) || c.dormant_until > self.last_tick + INTERVAL
        {
            return Ok(Execution::Rejected);
        }
        let actor = self
            .actors
            .get(&id)
            .ok_or("Missing physical actor")?
            .clone();
        let nested_room = (job.request.trait_id == 65)
            .then(|| occupied_den_room(habitats, id))
            .flatten();
        let (cell, position) = if let Some(room) = nested_room {
            let point = habitats.nodes[&room].position;
            let position = [point.x, point.y];
            (
                cell_at(position).ok_or("Den nest outside resident world")?,
                position,
            )
        } else {
            actor
                .route_worksite
                .map(|cell| (cell, cell_center(cell)))
                .map(Ok)
                .unwrap_or_else(|| {
                    self.worksite(&job.request, terrain, river, patch_width, patch_meters)
                })?
        };
        let patch = job.request.patch as usize;
        if nested_room.is_none() && !at_worksite(actor.position_mm, cell, job.request.trait_id) {
            return Ok(Execution::Waiting);
        }
        if nested_room.is_none() && !worksite_usable(cell, job.request.trait_id, terrain, river) {
            return Ok(Execution::Rejected);
        }
        let height = terrain.elevation_mm(cell).map_err(|e| e.to_string())?;
        let depth = water_depth(cell, terrain, river);
        let mut candidate_patch = self.patches[patch].clone();
        let previous_shelter_id = habitats.shelters.keys().next_back().copied().unwrap_or(0);
        let mut wallet = actor.materials;
        let mut eco = ecology.clone();
        let mut bio_received = self.bio_received_units;
        let mut bio_returned = self.bio_returned_units;
        let mut cavity_debris = self.cavity_debris_units;
        let (wood, fiber, mineral): (u64, u64, u64) = match job.request.trait_id {
            65 => (2, 4, 0),
            46 if depth > 0 => (0, 8, 0),
            101 => (4, 0, 0),
            105 => (0, 0, 2),
            _ => (0, 0, 0),
        };
        let take_wood = wood.saturating_sub(wallet.wood_units);
        let take_fiber = fiber.saturating_sub(wallet.fiber_units);
        let take_mineral = mineral.saturating_sub(wallet.mineral_units);
        let harvest = plus(take_wood, take_fiber)?;
        if eco.available_engineering_biomass(job.request.patch) < harvest
            || candidate_patch.mineral_units < take_mineral
        {
            return Ok(Execution::Rejected);
        }
        let gather_energy = plus(harvest, take_mineral)?;
        let gather_work = plus(harvest, take_mineral)?
            .checked_mul(10)
            .ok_or("Gather work overflow")?;
        if actor.labor_ticks < gather_work {
            return Ok(Execution::Waiting);
        }
        if c.energy < gather_energy {
            return Ok(Execution::Rejected);
        }
        if harvest > 0 {
            eco.export_biomass(job.request.patch, harvest)?;
            bio_received = plus(bio_received, harvest)?;
        }
        wallet.wood_units = plus(wallet.wood_units, take_wood)?;
        wallet.fiber_units = plus(wallet.fiber_units, take_fiber)?;
        wallet.mineral_units = plus(wallet.mineral_units, take_mineral)?;
        candidate_patch.mineral_units -= take_mineral;
        let energy = (c.energy - gather_energy).min(job.request.energy_budget);
        let mut effort = WorkBudget {
            energy,
            work_ticks: actor.labor_ticks - gather_work,
        };
        let before_effort = effort;
        let mut material_before = wallet;
        let available_surface = self.surface_available(cell, terrain, river)?;
        let site = Site {
            patch: job.request.patch,
            position_mm: position,
            surface_z_mm: height,
            water_depth_mm: depth,
            host_id: (eco.canopy_permille(job.request.patch) > 0
                && eco.available_engineering_biomass(job.request.patch) > 0
                && (job.request.trait_id != 65
                    || species.traits.has(11)
                    || species.locomotion_for(c.stage) == crate::ecology::Locomotion::Flying))
                .then_some(0x8000_0000_0000_0000u64 + u64::from(job.request.patch)),
            host_height_mm: if eco.canopy_permille(job.request.patch) > 0 {
                6000
            } else {
                0
            },
            available_substrate_mm3: if job.request.trait_id == 12 {
                candidate_patch.underground_mm3
            } else {
                available_surface
            },
            host_biomass_units: eco.available_engineering_biomass(job.request.patch),
            group_size: c.count,
            body_width_mm: species
                .body_radius_mm
                .checked_mul(2)
                .ok_or("Body width overflow")?,
        };
        let mut after_habitat = None;
        let mut prepared = None;
        let events;
        if let Some(room) = nested_room {
            let mut staged = habitats.clone();
            if harvest > 0 {
                // The biological source is coarse at the actual entrance, not
                // inside the den. Pay the real connected tunnel round trip;
                // blocked/flooded routes cannot supply hidden free material.
                let old = staged.occupants[&id].clone();
                staged.occupants.get_mut(&id).unwrap().location = Occupancy::Underground(room);
                let entrance = staged
                    .nodes
                    .values()
                    .find(|node| {
                        node.patch == c.patch
                            && node.kind == crate::habitat::NodeKind::Entrance
                            && !node.collapsed
                            && staged.route(room, node.id, old.body_width_mm).is_ok()
                    })
                    .map(|node| node.id);
                let Some(entrance) = entrance else {
                    return Ok(Execution::Rejected);
                };
                if staged.travel_burrow(id, entrance, &mut effort).is_err()
                    || staged.travel_burrow(id, room, &mut effort).is_err()
                {
                    return Ok(Execution::Waiting);
                }
                staged.occupants.insert(id, old);
            }
            let slots = room_free_slots(&staged, room).min(2);
            if slots == 0 {
                return Ok(Execution::Rejected);
            }
            let aperture = staged.nodes[&room].aperture_mm;
            match staged.create_shelter(
                c.patch,
                ShelterKind::Nest,
                None,
                Some(room),
                slots,
                aperture,
                &mut wallet,
                &mut effort,
            ) {
                Ok((_, emitted)) => events = emitted,
                Err(HabitatError::InsufficientWork) => return Ok(Execution::Waiting),
                Err(_) => return Ok(Execution::Rejected),
            }
            after_habitat = Some(staged);
        } else if job.request.trait_id == 102 {
            let mut staged = habitats.clone();
            let nutrients = eco.root_zones[patch].nutrients.min(100);
            eco.root_zones[patch].nutrients -= nutrients;
            eco.ledger.material_exported = plus(eco.ledger.material_exported, nutrients)?;
            bio_received = plus(bio_received, nutrients)?;
            candidate_patch.upper.nutrients = plus(candidate_patch.upper.nutrients, nutrients)?;
            match staged.mix_soil(
                job.request.patch,
                &mut candidate_patch.upper,
                &mut candidate_patch.lower,
                10_000_000,
                &mut effort,
            ) {
                Ok(event) => events = vec![event],
                Err(HabitatError::InsufficientWork) => return Ok(Execution::Waiting),
                Err(_) => return Ok(Execution::Rejected),
            }
            let returned = candidate_patch.upper.nutrients;
            candidate_patch.upper.nutrients = 0;
            eco.root_zones[patch].nutrients = plus(eco.root_zones[patch].nutrients, returned)?;
            eco.ledger.material_imported = plus(eco.ledger.material_imported, returned)?;
            bio_returned = plus(bio_returned, returned)?;
            after_habitat = Some(staged);
        } else {
            match habitats.prepare_engineering(job.request.trait_id, id, site, wallet, effort) {
                Ok(p) => {
                    events = p.events().to_vec();
                    effort.energy -= p.energy_spent();
                    effort.work_ticks -= p.work_ticks_spent();
                    wallet = p.resulting_materials();
                    prepared = Some(p);
                }
                Err(HabitatError::InsufficientWork) => {
                    if c.energy < 30 {
                        return Ok(Execution::Rejected);
                    }
                    return Ok(Execution::Waiting);
                }
                Err(_) => return Ok(Execution::Rejected),
            }
        }
        let spent = plus(before_effort.energy - effort.energy, gather_energy)?;
        let spent_ticks = plus(before_effort.work_ticks - effort.work_ticks, gather_work)?;
        if c.energy < spent {
            return Ok(Execution::Rejected);
        }
        let mut surface_pool = self
            .surface_pools
            .get(&cell)
            .cloned()
            .unwrap_or(SurfacePool {
                remaining_mm3: 0,
                floor_mm: height.checked_sub(500).ok_or("Surface floor overflow")?,
            });
        let mut received_surface = self.surface_received_mm3;
        let mut target_height = None;
        let mut staged_river: Option<RiverFixture> = None;
        let mut new_dam_links: Option<(u64, Vec<usize>)> = None;
        let mut new_reef_site: Option<(u64, usize)> = None;
        let mut new_bases = BTreeMap::new();
        for event in &events {
            match event {
                HabitatEvent::TerrainExcavated { volume_mm3, .. } => {
                    candidate_patch.underground_mm3 = candidate_patch
                        .underground_mm3
                        .checked_sub(*volume_mm3)
                        .ok_or("Underground source exhausted")?;
                }
                HabitatEvent::TerrainDepressed { soil_mm3, .. } => {
                    let needed = soil_mm3.saturating_sub(surface_pool.remaining_mm3);
                    if needed > 0 {
                        let mm = needed.div_ceil(CELL_MM3_PER_MM);
                        let mm_i32 = i32::try_from(mm).map_err(|_| "Surface edit too large")?;
                        let target = height.checked_sub(mm_i32).ok_or("Surface edit overflow")?;
                        if target < surface_pool.floor_mm {
                            return Ok(Execution::Rejected);
                        }
                        terrain
                            .validate_elevation_edit(cell, target)
                            .map_err(|e| e.to_string())?;
                        let transferred = mm
                            .checked_mul(CELL_MM3_PER_MM)
                            .ok_or("Surface volume overflow")?;
                        if RiverFixture::local_index(cell).is_some() {
                            let mut r = river.clone();
                            let volume = match r.edit_beds_material(&[(cell, target)]) {
                                Ok(v) => v,
                                Err(_) => return Ok(Execution::Rejected),
                            };
                            if volume.cut_liters.checked_mul(MM3_PER_LITER) != Some(transferred)
                                || volume.fill_liters != 0
                            {
                                return Err("Surface fine/coarse transfer mismatch".into());
                            }
                            staged_river = Some(r);
                        }
                        surface_pool.remaining_mm3 = plus(surface_pool.remaining_mm3, transferred)?;
                        received_surface = plus(received_surface, transferred)?;
                        target_height = Some(target);
                    }
                    surface_pool.remaining_mm3 = surface_pool
                        .remaining_mm3
                        .checked_sub(*soil_mm3)
                        .ok_or("Fine surface source underflow")?;
                }
                HabitatEvent::CanopyThinned { biomass_units, .. } => {
                    eco.export_biomass(job.request.patch, *biomass_units)?;
                    bio_received = plus(bio_received, *biomass_units)?;
                }
                HabitatEvent::HostExcavated {
                    removed_wood_units, ..
                } => {
                    eco.export_biomass(job.request.patch, *removed_wood_units)?;
                    bio_received = plus(bio_received, *removed_wood_units)?;
                    cavity_debris = plus(cavity_debris, *removed_wood_units)?;
                }
                HabitatEvent::DamBarrierChanged {
                    dam,
                    crest_mm,
                    permeability_permille,
                    ..
                } => {
                    if RiverFixture::local_index(cell).is_none() {
                        return Ok(Execution::Rejected);
                    }
                    let links = self.dam_links.get(dam).cloned().unwrap_or_else(|| {
                        dam_crossing_links(
                            cell,
                            job.request.patch,
                            patch_width,
                            patch_meters,
                            river,
                        )
                    });
                    if links.is_empty() {
                        return Ok(Execution::Rejected);
                    }
                    let r = staged_river.get_or_insert_with(|| river.clone());
                    let before = r.water.totals().stored_liters;
                    for &link in &links {
                        new_bases
                            .entry(link)
                            .or_insert(river.water.links()[link].barrier);
                        let inherited = r.water.links()[link].barrier;
                        r.water
                            .set_barrier(
                                link,
                                Some(Barrier {
                                    crest_mm: inherited
                                        .map_or(*crest_mm, |b| b.crest_mm.max(*crest_mm)),
                                    permeability_permille: inherited
                                        .map_or(*permeability_permille, |b| {
                                            b.permeability_permille.min(*permeability_permille)
                                        }),
                                }),
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    if before != r.water.totals().stored_liters {
                        return Err("Dam changed water inventory".into());
                    }
                    new_dam_links = Some((*dam, links));
                }
                HabitatEvent::ReefClearanceChanged {
                    reef,
                    solid_height_mm,
                    ..
                } => {
                    new_reef_site = Some((*reef, cell));
                    if let Some(local) = RiverFixture::local_index(cell) {
                        let r = staged_river.get_or_insert_with(|| river.clone());
                        let links: Vec<usize> = r
                            .water
                            .links()
                            .iter()
                            .enumerate()
                            .filter(|(_, l)| l.a == local || l.b == local)
                            .map(|(i, _)| i)
                            .collect();
                        for i in links {
                            new_bases.entry(i).or_insert(river.water.links()[i].barrier);
                            let old = r.water.links()[i].barrier;
                            let crest = height
                                .checked_add(
                                    i32::try_from(*solid_height_mm)
                                        .map_err(|_| "Reef height overflow")?,
                                )
                                .ok_or("Reef crest overflow")?;
                            r.water
                                .set_barrier(
                                    i,
                                    Some(Barrier {
                                        crest_mm: old.map_or(crest, |b| b.crest_mm.max(crest)),
                                        permeability_permille: old
                                            .map_or(700, |b| b.permeability_permille.min(700)),
                                    }),
                                )
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
                _ => {}
            }
        }
        eco.commit_physical_effect(id, job.request.trait_id, spent, true)?;
        eco.set_engineering_busy(id, self.last_tick + INTERVAL)?;
        eco.validate()?;
        let commits = plus(*self.committed.get(&job.request.trait_id).unwrap_or(&0), 1)?;
        // Everything fallible above was staged. One prevalidated terrain cell
        // and the prepared same-revision habitat/wallet now commit together.
        if let Some(p) = prepared {
            let mut before = before_effort;
            habitats
                .commit_engineering(p, &mut material_before, &mut before)
                .map_err(|e| e.to_string())?;
        }
        if let Some(h) = after_habitat {
            *habitats = h;
        }
        if let Some(r) = staged_river {
            *river = r;
        }
        if let Some(target) = target_height {
            terrain
                .set_elevation_mm(cell, target)
                .map_err(|e| e.to_string())?;
        }
        self.patches[patch] = candidate_patch;
        self.surface_received_mm3 = received_surface;
        self.bio_received_units = bio_received;
        self.bio_returned_units = bio_returned;
        self.cavity_debris_units = cavity_debris;
        if events
            .iter()
            .any(|e| matches!(e, HabitatEvent::TerrainDepressed { .. }))
        {
            self.surface_pools.insert(cell, surface_pool);
        }
        let a = self.actors.get_mut(&id).unwrap();
        a.materials = wallet;
        a.labor_ticks = a
            .labor_ticks
            .checked_sub(spent_ticks)
            .ok_or("Labor overspent")?;
        a.patch = job.request.patch;
        // Position remains the actual arrived stand-site (e.g. a dry bank).
        // The structure itself is located at the separate worksite position.
        if let Some((id, links)) = new_dam_links {
            self.dam_links.insert(id, links);
            self.dam_sites.insert(id, cell);
        }
        if let Some((id, cell)) = new_reef_site {
            self.reef_sites.entry(id).or_insert(cell);
        }
        for (link, base) in new_bases {
            self.hydraulic_base.entry(link).or_insert(base);
            self.hydraulic_last_applied
                .insert(link, river.water.links()[link].barrier);
        }
        self.reconcile_hydraulics(habitats, terrain, river)?;
        for s in habitats
            .shelters
            .values()
            .filter(|s| s.patch == job.request.patch)
        {
            self.structure_positions.entry(s.id).or_insert(position);
            if job.request.trait_id == 65
                && s.kind == ShelterKind::Nest
                && s.id > previous_shelter_id
            {
                self.nest_owners.insert(s.id, id);
            }
        }
        if job.request.trait_id == 46 {
            for trap in habitats
                .traps
                .values()
                .filter(|t| t.patch == job.request.patch)
            {
                self.trap_owners.entry(trap.id).or_insert(id);
            }
        }
        self.committed.insert(job.request.trait_id, commits);
        // This is a derived total of the one physical material owner, not a
        // second stock. Keep the committed snapshot valid immediately, rather
        // than relying on the outer scheduled step to refresh it later.
        self.stored_habitat_material = habitat_materials(habitats)?;
        *ecology = eco;
        let _ = environment;
        Ok(Execution::Committed)
    }
    fn surface_available(
        &self,
        cell: usize,
        terrain: &Terrain,
        river: &RiverFixture,
    ) -> Result<u64, String> {
        let pool = self.surface_pools.get(&cell).map_or(0, |p| p.remaining_mm3);
        if let Some(i) = RiverFixture::local_index(cell) {
            plus(
                pool,
                river.ground.cells()[i]
                    .bed_sediment_liters
                    .checked_mul(MM3_PER_LITER)
                    .ok_or("Surface stock overflow")?,
            )
        } else {
            let height = terrain.elevation_mm(cell).map_err(|e| e.to_string())?;
            let floor = self
                .surface_pools
                .get(&cell)
                .map_or(height - 500, |p| p.floor_mm);
            plus(
                pool,
                u64::try_from(height - floor)
                    .map_err(|_| "Surface below geological floor")?
                    .checked_mul(CELL_MM3_PER_MM)
                    .ok_or("Surface stock overflow")?,
            )
        }
    }
    fn attach_real_clutches(
        &mut self,
        ecology: &Ecology,
        habitats: &mut HabitatWorld,
    ) -> Result<(), String> {
        let live: BTreeSet<u64> = ecology.clutches.iter().map(|c| c.id).collect();
        let stale: Vec<u64> = habitats
            .shelters
            .values()
            .filter(|s| s.clutch.is_some_and(|(id, _)| !live.contains(&id)))
            .map(|s| s.id)
            .collect();
        for id in stale {
            habitats.shelters.get_mut(&id).unwrap().clutch = None;
            habitats.revision = plus(habitats.revision, 1)?;
        }
        let attached: BTreeSet<u64> = habitats
            .shelters
            .values()
            .filter_map(|s| s.clutch.map(|(id, _)| id))
            .collect();
        let mut claimed = attached;
        let nests: Vec<u64> = habitats
            .shelters
            .values()
            .filter(|s| {
                s.kind == ShelterKind::Nest && s.integrity > 0 && !s.flooded && s.clutch.is_none()
            })
            .map(|s| s.id)
            .collect();
        for nest in nests {
            let n = &habitats.shelters[&nest];
            if let Some(c) = ecology.clutches.iter().find(|c| {
                matches!(c.custody,Custody::Wild(p)if p==n.patch)
                    && !claimed.contains(&c.id)
                    && c.count <= n.capacity
                    && n.burrow_node
                        .is_none_or(|room| room_free_slots(habitats, room) >= c.count)
                    && self.nest_owners.get(&nest) == Some(&c.mother_id)
                    && ecology.species[c.species_id as usize].traits.has(65)
            }) {
                habitats
                    .attach_clutch(nest, c.id, c.count)
                    .map_err(|e| e.to_string())?;
                claimed.insert(c.id);
            }
        }
        Ok(())
    }
    fn synchronize_hazards(
        &mut self,
        tick: u64,
        ecology: &mut Ecology,
        habitats: &mut HabitatWorld,
        terrain: &Terrain,
        river: &RiverFixture,
        patch_width: usize,
        patch_meters: u32,
    ) -> Result<(), String> {
        // Entrance-local water avoids flooding a dry distant entrance merely
        // because another cell in the160m macro patch is wet. These are hazard
        // projections of the finite hydraulic owner, not extra water stocks.
        let mut reached = BTreeMap::<u64, i64>::new();
        let mut queue = std::collections::VecDeque::new();
        for node in habitats.nodes.values() {
            if node.kind == crate::habitat::NodeKind::Entrance && !node.collapsed {
                if let Some(cell) = cell_at([node.position.x, node.position.y]) {
                    let d = water_depth(cell, terrain, river);
                    let z = i64::from(terrain.elevation_mm(cell).map_err(|e| e.to_string())?)
                        + i64::from(d);
                    if d > 0 && z > i64::from(node.position.z) {
                        reached.insert(node.id, z);
                        queue.push_back(node.id);
                    }
                }
            }
        }
        while let Some(id) = queue.pop_front() {
            let z = reached[&id];
            for edge in habitats.passages.values() {
                if edge.blocked || edge.centerline.iter().any(|p| i64::from(p.z) >= z) {
                    continue;
                }
                let next = if edge.from == id {
                    edge.to
                } else if edge.to == id {
                    edge.from
                } else {
                    continue;
                };
                if habitats.nodes[&next].collapsed
                    || reached.get(&next).is_some_and(|&old| old >= z)
                {
                    continue;
                }
                reached.insert(next, z);
                queue.push_back(next);
            }
        }
        for node in habitats.nodes.values_mut() {
            node.flooded_depth_mm = reached.get(&node.id).map_or(0, |&z| {
                u32::try_from((z - i64::from(node.position.z)).max(0)).unwrap_or(u32::MAX)
            });
        }
        let mut clutch_losses = Vec::new();
        for shelter in habitats.shelters.values_mut() {
            let position = shelter
                .host
                .and_then(|id| {
                    habitats
                        .supports
                        .get(&id)
                        .map(|s| [s.position.x, s.position.y])
                })
                .or_else(|| self.structure_positions.get(&shelter.id).copied())
                .unwrap_or(patch_center(shelter.patch, patch_width, patch_meters)?);
            shelter.flooded = if let Some(node) = shelter.burrow_node {
                habitats.nodes[&node].flooded_depth_mm >= shelter.aperture_mm
            } else if let Some(cell) = cell_at(position) {
                water_depth(cell, terrain, river) > 0
                    && i64::from(terrain.elevation_mm(cell).map_err(|e| e.to_string())?)
                        + i64::from(water_depth(cell, terrain, river))
                        > i64::from(shelter.flood_crest_mm)
            } else {
                false
            };
            if shelter.flooded {
                if let Some((id, _)) = shelter.clutch.take() {
                    clutch_losses.push(id);
                }
            }
        }
        for id in clutch_losses {
            if ecology.clutches.iter().any(|c| c.id == id) {
                ecology.destroy_clutch(id)?;
            }
        }
        let mut release = Vec::new();
        let mut drown = Vec::new();
        for o in habitats.occupants.values() {
            let Some(c) = ecology
                .cohorts
                .iter()
                .find(|c| c.id == o.organism && c.count > 0)
                .cloned()
            else {
                release.push(o.organism);
                continue;
            };
            let (patch, observable, height, flood) = match o.location {
                Occupancy::Underground(id) => {
                    let n = &habitats.nodes[&id];
                    (
                        n.patch,
                        false,
                        n.position.z - n.surface_z_mm,
                        n.flooded_depth_mm >= o.body_width_mm,
                    )
                }
                Occupancy::Shelter(id) => {
                    let s = &habitats.shelters[&id];
                    let height = s.burrow_node.map_or(0, |n| {
                        habitats.nodes[&n].position.z - habitats.nodes[&n].surface_z_mm
                    });
                    (s.patch, s.kind == ShelterKind::Nest, height, s.flooded)
                }
                Occupancy::Canopy(id) => {
                    let s = &habitats.supports[&id];
                    (
                        s.patch,
                        true,
                        i32::try_from(s.height_mm).map_err(|_| "Canopy height overflow")?,
                        false,
                    )
                }
                Occupancy::Trap(id) => {
                    let t = &habitats.traps[&id];
                    (t.patch, true, -(t.depth_mm as i32), false)
                }
            };
            if c.patch != patch {
                release.push(o.organism);
                continue;
            }
            ecology.set_surface_observable(o.organism, observable, height)?;
            if flood {
                let aquatic = ecology.species[c.species_id as usize].habitat
                    == crate::ecology::Habitat::Water
                    || ecology.species[c.species_id as usize].habitat
                        == crate::ecology::Habitat::Amphibious
                        && c.stage == Stage::Juvenile;
                if !aquatic {
                    let exposure = plus(
                        *self.flood_exposure_ticks.get(&o.organism).unwrap_or(&0),
                        INTERVAL,
                    )?;
                    self.flood_exposure_ticks.insert(o.organism, exposure);
                    let animal = ecology
                        .cohorts
                        .iter_mut()
                        .find(|c| c.id == o.organism)
                        .unwrap();
                    animal.dormant_until = 0;
                    if exposure >= 60 {
                        animal.health_permille = animal.health_permille.saturating_sub(50);
                    }
                    if animal.health_permille == 0 {
                        drown.push((o.organism, c.count));
                    }
                }
            } else {
                self.flood_exposure_ticks.remove(&o.organism);
            }
        }
        for (id, count) in drown {
            ecology.damage_cohort(id, count)?;
            release.push(id);
        }
        for id in release {
            self.flood_exposure_ticks.remove(&id);
            if habitats.occupants.contains_key(&id) {
                habitats.release_occupant(id).map_err(|e| e.to_string())?;
            }
            if let Some(c) = ecology
                .cohorts
                .iter_mut()
                .find(|c| c.id == id && c.count > 0)
            {
                c.surface_observable = true;
                c.location_height_mm = 0;
            }
        }
        habitats.last_hazard_tick = tick;
        habitats.revision = plus(habitats.revision, 1)?;
        Ok(())
    }
    fn leave_hungry_refuges(
        &mut self,
        tick: u64,
        ecology: &mut Ecology,
        habitats: &mut HabitatWorld,
    ) -> Result<(), String> {
        let ids: Vec<u64> = habitats.occupants.keys().copied().collect();
        for id in ids {
            let Some(c) = ecology
                .cohorts
                .iter()
                .find(|c| c.id == id && c.count > 0)
                .cloned()
            else {
                continue;
            };
            if c.energy >= 30 && !self.flood_exposure_ticks.contains_key(&id)
                || c.dormant_until > tick
                || self
                    .active
                    .iter()
                    .any(|k| self.pending.get(k).is_some_and(|p| p.actor == Some(id)))
            {
                continue;
            }
            let species = &ecology.species[c.species_id as usize];
            let resting = (tick % 2400 < 1200) == species.nocturnal;
            if resting || c.stamina_permille < 50 {
                continue;
            }
            let Some(actor) = self.actors.get_mut(&id) else {
                continue;
            };
            actor.labor_ticks = plus(actor.labor_ticks, tick - actor.last_work_tick)?.min(600);
            actor.last_work_tick = tick;
            let mut effort = WorkBudget {
                energy: c.energy,
                work_ticks: actor.labor_ticks,
            };
            let before = effort;
            let mut staged = habitats.clone();
            let location = staged.occupants[&id].location;
            let capability;
            match location {
                Occupancy::Canopy(support) => {
                    let height = u64::from(staged.supports[&support].height_mm);
                    let energy = height.div_ceil(1000);
                    let work = height.div_ceil(100);
                    if effort.energy < energy || effort.work_ticks < work {
                        continue;
                    }
                    effort.energy -= energy;
                    effort.work_ticks -= work;
                    staged.release_occupant(id).map_err(|e| e.to_string())?;
                    capability = 11;
                }
                Occupancy::Underground(_) | Occupancy::Shelter(_) => {
                    let node = match location {
                        Occupancy::Underground(n) => n,
                        Occupancy::Shelter(s) => {
                            let Some(n) = staged.shelters[&s].burrow_node else {
                                continue;
                            };
                            staged.occupants.get_mut(&id).unwrap().location =
                                Occupancy::Underground(n);
                            n
                        }
                        _ => unreachable!(),
                    };
                    let exits: Vec<u64> = staged
                        .nodes
                        .values()
                        .filter(|n| {
                            n.patch == c.patch
                                && n.kind == crate::habitat::NodeKind::Entrance
                                && !n.collapsed
                                && staged
                                    .route(node, n.id, staged.occupants[&id].body_width_mm)
                                    .is_ok()
                        })
                        .map(|n| n.id)
                        .collect();
                    let Some(exit) = exits.first() else { continue };
                    if staged.travel_burrow(id, *exit, &mut effort).is_err() {
                        continue;
                    }
                    staged.exit_burrow(id).map_err(|e| e.to_string())?;
                    capability = if species.traits.has(12) { 12 } else { 64 };
                }
                Occupancy::Trap(_) => continue,
            }
            let spent = before.energy - effort.energy;
            ecology.commit_physical_effect(id, capability, spent, true)?;
            ecology.set_surface_observable(id, true, 0)?;
            actor.labor_ticks = effort.work_ticks;
            *habitats = staged;
        }
        Ok(())
    }
    fn advance_traps(
        &mut self,
        tick: u64,
        ecology: &mut Ecology,
        habitats: &mut HabitatWorld,
    ) -> Result<(), String> {
        let arrivals = std::mem::take(&mut self.trap_arrivals);
        for arrival in arrivals {
            if arrival.arrival_tick > tick {
                self.trap_arrivals.push(arrival);
                continue;
            }
            let Some(trap) = habitats.traps.get(&arrival.trap) else {
                continue;
            };
            if trap.integrity == 0 || trap.trapped.len() >= trap.capacity as usize {
                continue;
            }
            let Some(c) = ecology
                .cohorts
                .iter()
                .find(|c| {
                    c.id == arrival.source
                        && c.count > 0
                        && c.patch == trap.patch
                        && c.surface_observable
                        && !habitats.occupants.contains_key(&c.id)
                })
                .cloned()
            else {
                continue;
            };
            // Encounters are delayed, not reservations. Another trap may
            // already hold the source's last individual, or it may have entered
            // a refuge since scheduling. Revalidate before materializing so a
            // stale arrival cannot claim an existing occupancy or split stock.
            let width = ecology.species[c.species_id as usize]
                .body_radius_mm
                .checked_mul(2)
                .ok_or("Trap body width overflow")?;
            if width > trap.max_prey_width_mm {
                continue;
            }
            let patch = trap.patch;
            let position = [trap.position.x, trap.position.y];
            if c.count == 1 {
                if let Some(actual) = self
                    .actors
                    .get(&c.id)
                    .map(|a| a.position_mm)
                    .or_else(|| self.observed_positions.get(&c.id).copied())
                {
                    let dx = i64::from(actual[0]) - i64::from(position[0]);
                    let dy = i64::from(actual[1]) - i64::from(position[1]);
                    let reach = i64::from(trap.max_prey_width_mm) * 2;
                    if dx * dx + dy * dy > reach * reach {
                        continue;
                    }
                }
            }
            let prey = ecology.materialize_one(c.id)?;
            habitats.intercept(arrival.trap, prey, width).map_err(|e| {
                format!(
                    "Trap interception failed at tick {tick}: trap {} source {} prey {prey}: {e}",
                    arrival.trap, arrival.source
                )
            })?;
            self.actors.entry(prey).or_insert(PhysicalActor {
                organism: prey,
                patch,
                position_mm: position,
                materials: Materials::default(),
                labor_ticks: 0,
                last_work_tick: tick,
                local_route: vec![],
                route_cursor: 0,
                route_worksite: None,
                route_terrain_revision: 0,
                route_water_revision: 0,
                travel_energy_mm: 0,
                last_progress_tick: tick,
            });
        }
        let traps: Vec<u64> = habitats
            .traps
            .values()
            .filter(|t| t.integrity > 0)
            .map(|t| t.id)
            .collect();
        for id in traps {
            let Some(&owner) = self.trap_owners.get(&id) else {
                continue;
            };
            let Some(builder) = ecology
                .cohorts
                .iter()
                .find(|c| c.id == owner && c.count > 0)
                .cloned()
            else {
                continue;
            };
            let species = ecology.species[builder.species_id as usize].clone();
            let victims: Vec<u64> = habitats.traps[&id].trapped.iter().copied().collect();
            for prey in victims {
                let Some(victim) = ecology
                    .cohorts
                    .iter()
                    .find(|c| c.id == prey && c.count > 0)
                    .cloned()
                else {
                    if habitats.occupants.contains_key(&prey) {
                        habitats.release_occupant(prey).map_err(|e| e.to_string())?;
                    }
                    continue;
                };
                ecology.set_engineering_busy(prey, plus(plus(tick, INTERVAL)?, 1)?)?;
                let current_energy = ecology
                    .cohorts
                    .iter()
                    .find(|c| c.id == owner && c.count > 0)
                    .map_or(120, |c| c.energy);
                if current_energy < 100 && species.food_species.contains(&victim.species_id) {
                    let report = ecology.damage_cohort(prey, 1)?;
                    let edible = (report.material_to_litter / 2).min(120 - current_energy);
                    ecology.root_zones[victim.patch as usize].litter = ecology.root_zones
                        [victim.patch as usize]
                        .litter
                        .checked_sub(edible)
                        .ok_or("Trap meal counted twice")?;
                    let predator = ecology
                        .cohorts
                        .iter_mut()
                        .find(|c| c.id == owner && c.count > 0)
                        .ok_or("Trap builder lost during meal")?;
                    predator.energy = plus(predator.energy, edible)?;
                    habitats.release_occupant(prey).map_err(|e| e.to_string())?;
                }
            }
            let trap = &habitats.traps[&id];
            if !trap.trapped.is_empty()
                || self.trap_arrivals.iter().any(|a| a.trap == id)
                || self.next_trap_encounter.get(&id).is_some_and(|&t| t > tick)
            {
                continue;
            }
            let prey = ecology
                .cohorts
                .iter()
                .find(|c| {
                    c.id != owner
                        && c.count > 0
                        && c.patch == trap.patch
                        && c.surface_observable
                        && !habitats.occupants.contains_key(&c.id)
                        && species.food_species.contains(&c.species_id)
                        && ecology.species[c.species_id as usize].body_radius_mm * 2
                            <= trap.max_prey_width_mm
                })
                .map(|c| c.id);
            if let Some(source) = prey {
                self.trap_arrivals.push(TrapArrival {
                    trap: id,
                    source,
                    arrival_tick: plus(tick, 400)?,
                });
                self.next_trap_encounter.insert(id, plus(tick, 800)?);
            }
        }
        Ok(())
    }
    pub fn tracked_position(&self, id: u64, habitats: &HabitatWorld) -> Option<[i32; 3]> {
        if let Some(o) = habitats.occupants.get(&id) {
            match o.location {
                Occupancy::Underground(n) => {
                    let p = habitats.nodes.get(&n)?.position;
                    return Some([p.x, p.y, p.z]);
                }
                Occupancy::Canopy(n) => {
                    let s = habitats.supports.get(&n)?;
                    return Some([
                        s.position.x,
                        s.position.y,
                        s.position.z.checked_add(i32::try_from(s.height_mm).ok()?)?,
                    ]);
                }
                Occupancy::Shelter(n) => {
                    let s = habitats.shelters.get(&n)?;
                    if let Some(n) = s.burrow_node {
                        let p = habitats.nodes.get(&n)?.position;
                        return Some([p.x, p.y, p.z]);
                    }
                }
                Occupancy::Trap(n) => {
                    let t = habitats.traps.get(&n)?;
                    return Some([
                        t.position.x,
                        t.position.y,
                        t.position.z.checked_sub(i32::try_from(t.depth_mm).ok()?)?,
                    ]);
                }
            }
        }
        self.actors
            .get(&id)
            .map(|a| [a.position_mm[0], a.position_mm[1], 0])
    }
    pub fn validate(&self) -> Result<(), String> {
        for (pending_key, job) in &self.pending {
            if *pending_key != key(job.request.cohort_id, job.request.trait_id)
                || job.request.patch as usize >= self.patches.len()
                || job.queued_tick > self.last_tick
            {
                return Err("Physical pending intent invalid".into());
            }
        }
        let mut active_actors = BTreeSet::new();
        for key in &self.active {
            if let Some(id) = self.pending.get(key).and_then(|j| j.actor) {
                if !active_actors.insert(id) || !self.actors.contains_key(&id) {
                    return Err("Physical actor assigned duplicate labor".into());
                }
            }
        }
        if self.active.len() > ACTIVE_JOBS
            || self.active.iter().any(|id| !self.pending.contains_key(id))
        {
            return Err("Physical active queue invalid".into());
        }
        let mut soil = 0u64;
        let mut mineral = 0u64;
        let mut biomass = self.cavity_debris_units;
        for p in &self.patches {
            soil = plus(
                soil,
                plus(
                    p.underground_mm3,
                    plus(p.upper.mineral_mm3, p.lower.mineral_mm3)?,
                )?,
            )?;
            mineral = plus(mineral, p.mineral_units)?;
            biomass = plus(biomass, plus(p.upper.nutrients, p.lower.nutrients)?)?;
        }
        for (id, a) in &self.actors {
            if *id != a.organism || a.patch as usize >= self.patches.len() || a.labor_ticks > 600 {
                return Err("Physical actor invalid".into());
            }
            soil = plus(soil, a.materials.soil_mm3)?;
            mineral = plus(mineral, a.materials.mineral_units)?;
            biomass = plus(
                biomass,
                plus(a.materials.wood_units, a.materials.fiber_units)?,
            )?;
        }
        for p in self.surface_pools.values() {
            soil = plus(soil, p.remaining_mm3)?;
        }
        soil = plus(soil, self.stored_habitat_material.soil_mm3)?;
        mineral = plus(mineral, self.stored_habitat_material.mineral_units)?;
        biomass = plus(
            biomass,
            plus(
                self.stored_habitat_material.wood_units,
                self.stored_habitat_material.fiber_units,
            )?,
        )?;
        if soil != plus(self.initial_soil_mm3, self.surface_received_mm3)? {
            return Err(format!(
                "Physical soil imbalance{soil} expected{}",
                self.initial_soil_mm3 + self.surface_received_mm3
            ));
        }
        if mineral != self.initial_mineral_units {
            return Err("Physical mineral imbalance".into());
        }
        if plus(biomass, self.bio_returned_units)? != self.bio_received_units {
            return Err("Physical biological material imbalance".into());
        }
        Ok(())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"last_tick":self.last_tick,"queued_optional_intents":self.pending.len(),"active_jobs":self.active.len(),"stalled_optional_jobs_released":self.stalled_releases,"optional_retry_cooldowns":self.retry_after.len(),"active_progress":self.active.iter().filter_map(|k|self.pending.get(k)).filter_map(|j|j.actor.and_then(|id|self.actors.get(&id)).map(|a|serde_json::json!({"organism":a.organism,"trait_id":j.request.trait_id,"patch":a.patch,"position_mm":a.position_mm,"worksite":a.route_worksite,"earned_labor_ticks":a.labor_ticks,"last_progress_tick":a.last_progress_tick}))).collect::<Vec<_>>(),"tracked_real_actors":self.actors.len(),"committed_traits":self.committed,"rejected_optional_intents":self.rejected,"stale_optional_intents":self.stale_intents,"surface_received_mm3":self.surface_received_mm3,"biomass_received_from_ecology":self.bio_received_units,"biomass_returned_to_ecology":self.bio_returned_units,"conserved":self.validate().is_ok(),"underground_water":"derived_connected_hazard_from_authoritative_surface;no_separate_volume_or_momentum","dams":"actual_links_inside_managed_river_only;static_generated_water_elsewhere","raw_ground_cover":"visual_density_is_never_timber_or_fiber_supply","canopy_hosts":"one_aggregate_reference_per_macro_patch;fails_at_zero_actual_living_canopy_tissue;no_per_tree_precision","breeding_refuges":"specific_reserved_den_group_slots_and_unique_builder_owned_nest;underground_nest_shares_actual_room_with_adults_and_eggs"})
    }
}
#[derive(Clone, Serialize, Deserialize)]
struct TrapArrival {
    trap: u64,
    source: u64,
    arrival_tick: u64,
}
#[derive(Clone, Copy)]
enum Execution {
    Waiting,
    Committed,
    Rejected,
}
fn cell_center(cell: usize) -> [i32; 2] {
    [
        ((cell % CW) * CELL_METERS + CELL_METERS / 2) as i32 * 1000,
        ((cell / CW) * CELL_METERS + CELL_METERS / 2) as i32 * 1000,
    ]
}
fn occupied_den_room(habitats: &HabitatWorld, organism: u64) -> Option<u64> {
    let occupant = habitats.occupants.get(&organism)?;
    let Occupancy::Shelter(id) = occupant.location else {
        return None;
    };
    let shelter = habitats.shelters.get(&id)?;
    if shelter.kind != ShelterKind::Den || shelter.integrity == 0 || shelter.flooded {
        return None;
    }
    let node = shelter.burrow_node?;
    let room = habitats.nodes.get(&node)?;
    (!room.collapsed && room.flooded_depth_mm < occupant.body_width_mm).then_some(node)
}

fn room_free_slots(habitats: &HabitatWorld, room: u64) -> u32 {
    let Some(node) = habitats.nodes.get(&room) else {
        return 0;
    };
    let adults: u64 = habitats
        .occupants
        .values()
        .filter(|o| match o.location {
            Occupancy::Underground(id) => id == room,
            Occupancy::Shelter(id) => habitats
                .shelters
                .get(&id)
                .is_some_and(|s| s.burrow_node == Some(room)),
            _ => false,
        })
        .map(|o| u64::from(o.group_size))
        .sum();
    let eggs: u64 = habitats
        .shelters
        .values()
        .filter(|s| s.burrow_node == Some(room))
        .filter_map(|s| s.clutch.map(|(_, count)| u64::from(count)))
        .sum();
    u64::from(node.capacity).saturating_sub(adults.saturating_add(eggs)) as u32
}

fn at_worksite(position: [i32; 2], target: usize, trait_id: u16) -> bool {
    let Some(cell) = cell_at(position) else {
        return false;
    };
    if trait_id == 101 {
        let dx = (cell % CW).abs_diff(target % CW);
        let dy = (cell / CW).abs_diff(target / CW);
        dx <= 1 && dy <= 1
    } else {
        let dest = cell_center(target);
        let dx = i64::from(position[0]) - i64::from(dest[0]);
        let dy = i64::from(position[1]) - i64::from(dest[1]);
        dx * dx + dy * dy <= 1_000_000
    }
}
fn worksite_usable(cell: usize, trait_id: u16, terrain: &Terrain, river: &RiverFixture) -> bool {
    if cell >= terrain.cells.len() || terrain.cells[cell].object != 0 {
        return false;
    }
    let depth = water_depth(cell, terrain, river);
    match trait_id {
        67 => depth > 0 && depth <= 300,
        101 => depth > 0 && RiverFixture::local_index(cell).is_some(),
        105 => (200..=20_000).contains(&depth),
        _ => depth <= 300,
    }
}
fn organism_cell_usable(
    cell: usize,
    species: &crate::ecology::Species,
    terrain: &Terrain,
    river: &RiverFixture,
) -> bool {
    if cell >= terrain.cells.len() || terrain.cells[cell].object != 0 {
        return false;
    }
    if species.locomotion_for(Stage::Adult) == crate::ecology::Locomotion::Flying {
        return true;
    }
    match species.habitat_for(Stage::Adult) {
        crate::ecology::Habitat::Land => water_depth(cell, terrain, river) <= 300,
        crate::ecology::Habitat::Water => water_depth(cell, terrain, river) >= 20,
        crate::ecology::Habitat::Amphibious => true,
    }
}
fn remaining_route_usable(
    start: usize,
    route: &[usize],
    species: &crate::ecology::Species,
    terrain: &Terrain,
    river: &RiverFixture,
) -> bool {
    let flying = species.locomotion_for(Stage::Adult) == crate::ecology::Locomotion::Flying;
    let mut previous = start;
    for &next in route {
        if !organism_cell_usable(next, species, terrain, river)
            || (previous % CW).abs_diff(next % CW) + (previous / CW).abs_diff(next / CW) > 1
            || !flying
                && (i64::from(terrain.elevation_mm(next).unwrap_or(i32::MAX))
                    - i64::from(terrain.elevation_mm(previous).unwrap_or(i32::MIN)))
                .unsigned_abs()
                    > 1500
        {
            return false;
        }
        previous = next;
    }
    true
}
/// A coarse animal has no invented fine position to preserve. Resolve that
/// missing detail once into the nearest real, compatible cell connected to its
/// accepted site. Existing/observed identities always use their known position.
fn cohort_fine_origin(
    patch: u32,
    target: usize,
    trait_id: u16,
    species: &crate::ecology::Species,
    terrain: &Terrain,
    river: &RiverFixture,
    width: usize,
    meters: u32,
) -> Option<([i32; 2], Vec<usize>)> {
    let center = patch_center(patch, width, meters).ok()?;
    let x0 = patch as usize % width * meters as usize / CELL_METERS;
    let y0 = patch as usize / width * meters as usize / CELL_METERS;
    let span = meters as usize / CELL_METERS;
    if span == 0 || span > 64 {
        return None;
    }
    let index = |cell: usize| {
        let x = cell % CW;
        let y = cell / CW;
        (x >= x0 && x < x0 + span && y >= y0 && y < y0 + span).then(|| (y - y0) * span + x - x0)
    };
    // One reverse bounded flood, rather than one path search per candidate.
    let mut toward = vec![usize::MAX; span * span];
    let mut queue = std::collections::VecDeque::new();
    for y in y0..(y0 + span).min(CH) {
        for x in x0..(x0 + span).min(CW) {
            let cell = y * CW + x;
            if organism_cell_usable(cell, species, terrain, river)
                && at_worksite(cell_center(cell), target, trait_id)
            {
                toward[index(cell)?] = cell;
                queue.push_back(cell);
            }
        }
    }
    while let Some(cell) = queue.pop_front() {
        let x = cell % CW;
        let y = cell / CW;
        for next in [
            x.checked_sub(1).map(|x| y * CW + x),
            (x + 1 < CW).then_some(cell + 1),
            y.checked_sub(1).map(|y| y * CW + x),
            (y + 1 < CH).then_some(cell + CW),
        ]
        .into_iter()
        .flatten()
        {
            let Some(j) = index(next) else {
                continue;
            };
            if toward[j] == usize::MAX
                && remaining_route_usable(cell, &[next], species, terrain, river)
            {
                toward[j] = cell;
                queue.push_back(next);
            }
        }
    }
    let mut selected = None;
    for y in y0..(y0 + span).min(CH) {
        for x in x0..(x0 + span).min(CW) {
            let cell = y * CW + x;
            if toward[index(cell)?] == usize::MAX {
                continue;
            }
            let position = cell_center(cell);
            let distance = (i64::from(position[0]) - i64::from(center[0])).unsigned_abs()
                + (i64::from(position[1]) - i64::from(center[1])).unsigned_abs();
            let candidate = (distance, cell);
            if selected.is_none_or(|old| candidate < old) {
                selected = Some(candidate);
            }
        }
    }
    let (_, start) = selected?;
    let mut at = start;
    let mut route = Vec::new();
    while toward[index(at)?] != at {
        at = toward[index(at)?];
        route.push(at);
    }
    Some((cell_center(start), route))
}

fn local_route(
    start: usize,
    target: usize,
    patch: u32,
    trait_id: u16,
    species: &crate::ecology::Species,
    terrain: &Terrain,
    river: &RiverFixture,
    width: usize,
    meters: u32,
) -> Option<Vec<usize>> {
    let x0 = (patch as usize % width) * meters as usize / CELL_METERS;
    let y0 = (patch as usize / width) * meters as usize / CELL_METERS;
    let span = meters as usize / CELL_METERS;
    if span == 0 || span > 64 {
        return None;
    }
    let index = |cell: usize| {
        let x = cell % CW;
        let y = cell / CW;
        (x >= x0 && x < x0 + span && y >= y0 && y < y0 + span).then(|| (y - y0) * span + x - x0)
    };
    let start_local = index(start)?;
    let mut previous = vec![usize::MAX; span * span];
    previous[start_local] = start;
    let mut queue = std::collections::VecDeque::from([start]);
    let mut goal = None;
    while let Some(cell) = queue.pop_front() {
        if at_worksite(cell_center(cell), target, trait_id) {
            goal = Some(cell);
            break;
        }
        let x = cell % CW;
        let y = cell / CW;
        for next in [
            x.checked_sub(1).map(|x| y * CW + x),
            (x + 1 < CW).then_some(cell + 1),
            y.checked_sub(1).map(|y| y * CW + x),
            (y + 1 < CH).then_some(cell + CW),
        ]
        .into_iter()
        .flatten()
        {
            let Some(j) = index(next) else { continue };
            if previous[j] != usize::MAX {
                continue;
            }
            let c = terrain.cells[next];
            if c.object != 0 {
                continue;
            }
            let depth = water_depth(next, terrain, river);
            let flying = species.locomotion == crate::ecology::Locomotion::Flying;
            let water_ok = if flying {
                true
            } else {
                match species.habitat {
                    crate::ecology::Habitat::Land => depth <= 300,
                    crate::ecology::Habitat::Water => depth >= 20,
                    crate::ecology::Habitat::Amphibious => true,
                }
            };
            if !water_ok {
                continue;
            }
            if !flying
                && (i64::from(terrain.elevation_mm(next).ok()?)
                    - i64::from(terrain.elevation_mm(cell).ok()?))
                .unsigned_abs()
                    > 1500
            {
                continue;
            }
            previous[j] = cell;
            queue.push_back(next);
        }
    }
    let mut at = goal?;
    let mut path = Vec::new();
    while at != start {
        path.push(at);
        at = previous[index(at)?];
    }
    path.reverse();
    Some(path)
}
fn patch_center(p: u32, width: usize, meters: u32) -> Result<[i32; 2], String> {
    let x = (p as usize % width)
        .checked_mul(meters as usize)
        .and_then(|n| n.checked_add(meters as usize / 2))
        .ok_or("Patch coordinate overflow")?;
    let y = (p as usize / width)
        .checked_mul(meters as usize)
        .and_then(|n| n.checked_add(meters as usize / 2))
        .ok_or("Patch coordinate overflow")?;
    Ok([
        i32::try_from(x)
            .map_err(|_| "Patch coordinate overflow")?
            .checked_mul(1000)
            .ok_or("Patch coordinate overflow")?,
        i32::try_from(y)
            .map_err(|_| "Patch coordinate overflow")?
            .checked_mul(1000)
            .ok_or("Patch coordinate overflow")?,
    ])
}
fn cell_at(pos: [i32; 2]) -> Option<usize> {
    if pos[0] < 0 || pos[1] < 0 {
        return None;
    }
    let x = pos[0] as usize / (CELL_METERS * 1000);
    let y = pos[1] as usize / (CELL_METERS * 1000);
    (x < CW && y < CH).then_some(y * CW + x)
}
fn water_depth(cell: usize, terrain: &Terrain, river: &RiverFixture) -> u32 {
    if let Some(i) = RiverFixture::local_index(cell) {
        river.water.cells()[i].depth_mm().min(u64::from(u32::MAX)) as u32
    } else {
        let c = terrain.cells[cell];
        if c.flags & DYNAMIC_WATER != 0 {
            c.water
        } else {
            c.water.checked_mul(10).expect("generated depth range")
        }
    }
}
fn choose_cell(
    patch: u32,
    trait_id: u16,
    terrain: &Terrain,
    river: &RiverFixture,
    width: usize,
    meters: u32,
) -> Result<(usize, [i32; 2]), String> {
    let center = patch_center(patch, width, meters)?;
    let initial = cell_at(center).ok_or("Physical patch outside resident terrain")?;
    let need_water = matches!(trait_id, 67 | 101 | 105);
    let x0 = (patch as usize % width) * meters as usize / CELL_METERS;
    let y0 = (patch as usize / width) * meters as usize / CELL_METERS;
    let span = (meters as usize / CELL_METERS).max(1);
    let mut best = initial;
    let mut score = 0u32;
    for y in y0..(y0 + span).min(CH) {
        for x in x0..(x0 + span).min(CW) {
            let i = y * CW + x;
            if terrain.cells[i].object != 0 {
                continue;
            }
            let d = water_depth(i, terrain, river);
            let s = if need_water {
                if trait_id == 67 {
                    if d > 0 && d <= 300 { u32::MAX - d } else { 0 }
                } else if trait_id == 105 && !(200..=20_000).contains(&d) {
                    0
                } else if trait_id == 101 && RiverFixture::local_index(i).is_none() {
                    0
                } else {
                    d
                }
            } else {
                u32::MAX - d
            };
            if s > score {
                best = i;
                score = s;
            }
        }
    }
    Ok((
        best,
        [
            ((best % CW) * CELL_METERS + CELL_METERS / 2) as i32 * 1000,
            ((best / CW) * CELL_METERS + CELL_METERS / 2) as i32 * 1000,
        ],
    ))
}
fn dam_crossing_links(
    cell: usize,
    patch: u32,
    width: usize,
    meters: u32,
    river: &RiverFixture,
) -> Vec<usize> {
    let Some(local) = RiverFixture::local_index(cell) else {
        return vec![];
    };
    let row = local / RIVER_WIDTH;
    let x0 = (patch as usize % width) * meters as usize / CELL_METERS;
    let x1 = x0 + meters as usize / CELL_METERS;
    river
        .water
        .links()
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            l.a / RIVER_WIDTH == row && l.b / RIVER_WIDTH == row + 1 && {
                let x = ORIGIN[0] + l.a % RIVER_WIDTH;
                x >= x0 && x < x1
            }
        })
        .map(|(i, _)| i)
        .collect()
}
fn habitat_materials(h: &HabitatWorld) -> Result<Materials, String> {
    let mut m = h.debris;
    for s in h.shelters.values() {
        m.soil_mm3 = plus(m.soil_mm3, s.material.soil_mm3)?;
        m.wood_units = plus(m.wood_units, s.material.wood_units)?;
        m.fiber_units = plus(m.fiber_units, s.material.fiber_units)?;
        m.mineral_units = plus(m.mineral_units, s.material.mineral_units)?;
    }
    for t in h.traps.values() {
        m.soil_mm3 = plus(m.soil_mm3, plus(t.materials.soil_mm3, t.backfilled_mm3)?)?;
        m.wood_units = plus(m.wood_units, t.materials.wood_units)?;
        m.fiber_units = plus(m.fiber_units, t.materials.fiber_units)?;
        m.mineral_units = plus(m.mineral_units, t.materials.mineral_units)?;
    }
    for d in h.dams.values() {
        m.wood_units = plus(m.wood_units, d.invested_wood_units)?;
    }
    for r in h.reefs.values() {
        m.mineral_units = plus(m.mineral_units, r.mineral_units)?;
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> (
        Terrain,
        RiverFixture,
        Ecology,
        HabitatWorld,
        EcosystemPhysics,
        Vec<EcologyPatch>,
    ) {
        let mut t = Terrain::new(78421);
        let r = RiverFixture::new(&mut t).unwrap();
        let e = Ecology::new(78421, 600).unwrap();
        (
            t,
            r,
            e,
            HabitatWorld::default(),
            EcosystemPhysics::new(600),
            vec![EcologyPatch::default(); 600],
        )
    }
    /// Explicit fixture placement uses an existing finite organism. It does not
    /// create population, food, water, building material or reserve energy.
    fn actor(
        trait_id: u16,
        patch: u32,
        e: &mut Ecology,
        p: &mut EcosystemPhysics,
        t: &Terrain,
        r: &RiverFixture,
    ) -> PhysicalRequest {
        let source = e
            .cohorts
            .iter()
            .find(|c| c.count > 0 && e.species[c.species_id as usize].traits.has(trait_id))
            .unwrap()
            .id;
        let id = e.materialize_one(source).unwrap();
        let c = e.cohorts.iter_mut().find(|c| c.id == id).unwrap();
        c.patch = patch;
        let request = PhysicalRequest {
            cohort_id: id,
            species_id: c.species_id,
            patch,
            trait_id,
            energy_budget: 80,
            individuals: 1,
        };
        p.actors.insert(
            id,
            PhysicalActor {
                organism: id,
                patch,
                position_mm: choose_cell(patch, trait_id, t, r, 30, 160).unwrap().1,
                materials: Materials::default(),
                labor_ticks: 600,
                last_work_tick: 20,
                local_route: vec![],
                route_cursor: 0,
                route_worksite: None,
                route_terrain_revision: 0,
                route_water_revision: 0,
                travel_energy_mm: 0,
                last_progress_tick: 20,
            },
        );
        e.last_tick = 20;
        request
    }
    #[test]
    fn real_nested_construction_transfers_finite_biomass_and_survives_json_save() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let request = actor(65, 0, &mut e, &mut p, &t, &r);
        let before = e.ledger.material_exported;
        let job = PendingEngineering {
            request: request.clone(),
            queued_tick: 20,
            actor: Some(request.cohort_id),
        };
        assert!(matches!(
            p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap(),
            Execution::Committed
        ));
        assert_eq!(e.ledger.material_exported - before, 6);
        assert_eq!(h.shelters.len(), 1);
        assert_eq!(h.shelters.values().next().unwrap().material.wood_units, 2);
        assert_eq!(h.shelters.values().next().unwrap().material.fiber_units, 4);
        p.validate().unwrap();
        e.validate().unwrap();
        h.validate().unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let restored: EcosystemPhysics = serde_json::from_str(&json).unwrap();
        restored.validate().unwrap();
        assert_eq!(serde_json::to_string(&restored).unwrap(), json);
    }
    #[test]
    fn bent_tunnel_spoil_is_paid_from_underground_and_never_lowers_the_surface() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let request = actor(12, 0, &mut e, &mut p, &t, &r);
        let (cell, _) = choose_cell(0, 12, &t, &r, 30, 160).unwrap();
        let height = t.elevation_mm(cell).unwrap();
        let underground = p.patches[0].underground_mm3;
        let job = PendingEngineering {
            request: request.clone(),
            queued_tick: 20,
            actor: Some(request.cohort_id),
        };
        assert!(matches!(
            p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap(),
            Execution::Committed
        ));
        assert_eq!(t.elevation_mm(cell).unwrap(), height);
        assert!(!h.passages.is_empty());
        let removed = underground - p.patches[0].underground_mm3;
        assert!(removed > 0);
        assert_eq!(p.actors[&request.cohort_id].materials.soil_mm3, removed);
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        p.validate().unwrap();
        e.validate().unwrap();
    }
    #[test]
    fn fine_pit_uses_exact_remainder_owner_instead_of_free_or_lost_coarse_soil() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let request = actor(46, 0, &mut e, &mut p, &t, &r);
        let (cell, _) = choose_cell(0, 46, &t, &r, 30, 160).unwrap();
        assert_eq!(water_depth(cell, &t, &r), 0);
        let before = t.elevation_mm(cell).unwrap();
        let job = PendingEngineering {
            request: request.clone(),
            queued_tick: 20,
            actor: Some(request.cohort_id),
        };
        assert!(matches!(
            p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap(),
            Execution::Committed
        ));
        let fine = h.traps.values().next().unwrap().excavated_mm3;
        assert_eq!(fine, 54_000_000);
        assert_eq!(before - t.elevation_mm(cell).unwrap(), 1);
        assert_eq!(p.surface_received_mm3, 64_000_000);
        assert_eq!(p.surface_pools[&cell].remaining_mm3, 10_000_000);
        assert_eq!(p.actors[&request.cohort_id].materials.soil_mm3, fine);
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        p.validate().unwrap();
    }
    /// Two real, paid webs target the same finite source before it reaches its
    /// last individual. Other animals stay alive in a separate fixture patch.
    fn overlapping_trap_encounters(
        second_arrival_tick: u64,
    ) -> (Ecology, HabitatWorld, EcosystemPhysics, [u64; 2], u64) {
        let (_, _, mut e, mut h, mut p, _) = world();
        let builder_species = e
            .species
            .iter()
            .find(|s| s.taxon == crate::ecology::Taxon::Predator && s.traits.has(46))
            .unwrap()
            .id;
        let builders: Vec<_> = e
            .cohorts
            .iter()
            .filter(|c| c.species_id == builder_species && c.count == 1)
            .map(|c| c.id)
            .take(2)
            .collect();
        assert_eq!(builders.len(), 2);
        let food = e.species[builder_species as usize].food_species.clone();
        let source = e
            .cohorts
            .iter()
            .find(|c| c.count > 1 && food.contains(&c.species_id))
            .unwrap()
            .id;
        let width = e.species[e
            .cohorts
            .iter()
            .find(|c| c.id == source)
            .unwrap()
            .species_id as usize]
            .body_radius_mm
            * 2;
        let mut traps = [0; 2];
        for i in 0..2 {
            // These setup costs use actual biomass and the two builders' own
            // reserves; neither construction nor interception adds organisms.
            e.export_biomass(0, 8).unwrap();
            p.bio_received_units += 8;
            let mut materials = Materials {
                fiber_units: 8,
                ..Default::default()
            };
            let mut effort = WorkBudget {
                energy: e
                    .cohorts
                    .iter()
                    .find(|c| c.id == builders[i])
                    .unwrap()
                    .energy,
                work_ticks: 200,
            };
            let before_energy = effort.energy;
            traps[i] = h
                .build_trap(
                    0,
                    crate::habitat::TrapKind::Web,
                    crate::habitat::Point3Mm::new(4000 + i as i32, 4000, 1000),
                    width,
                    1,
                    0,
                    &mut materials,
                    &mut effort,
                )
                .unwrap()
                .0;
            assert_eq!(materials, Materials::default());
            e.spend_activity_energy(builders[i], before_energy - effort.energy)
                .unwrap();
            p.trap_owners.insert(traps[i], builders[i]);
            p.trap_arrivals.push(TrapArrival {
                trap: traps[i],
                source,
                arrival_tick: if i == 0 { 20 } else { second_arrival_tick },
            });
        }
        while e.cohorts.iter().find(|c| c.id == source).unwrap().count > 1 {
            e.materialize_one(source).unwrap();
        }
        for c in &mut e.cohorts {
            c.patch = if c.id == source || builders.contains(&c.id) {
                0
            } else {
                1
            };
        }
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        e.validate().unwrap();
        h.validate().unwrap();
        p.validate().unwrap();
        (e, h, p, traps, source)
    }

    #[test]
    fn delayed_traps_claim_last_individual_once_with_simultaneous_and_saved_arrivals() {
        for save_between_arrivals in [false, true] {
            let (mut e, mut h, mut p, traps, source) =
                overlapping_trap_encounters(if save_between_arrivals { 40 } else { 20 });
            let census = serde_json::to_value(e.census()).unwrap();
            let ledger = serde_json::to_value(&e.ledger).unwrap();
            let cohort_records = e.cohorts.len();
            p.advance_traps(20, &mut e, &mut h).unwrap();
            if save_between_arrivals {
                assert_eq!(p.trap_arrivals.len(), 1);
                let saved = serde_json::to_vec(&(&e, &h, &p)).unwrap();
                (e, h, p) = serde_json::from_slice(&saved).unwrap();
                assert_eq!(serde_json::to_vec(&(&e, &h, &p)).unwrap(), saved);
                p.advance_traps(40, &mut e, &mut h).unwrap();
            }
            assert_eq!(h.occupants.len(), 1);
            assert_eq!(h.occupants[&source].location, Occupancy::Trap(traps[0]));
            assert_eq!(h.traps[&traps[0]].trapped, BTreeSet::from([source]));
            assert!(h.traps[&traps[1]].trapped.is_empty());
            // An empty competing trap cannot schedule the occupied singleton
            // again, even though a trapped animal remains surface observable.
            p.advance_traps(1000, &mut e, &mut h).unwrap();
            assert!(p.trap_arrivals.is_empty());
            assert_eq!(e.cohorts.len(), cohort_records);
            assert_eq!(serde_json::to_value(e.census()).unwrap(), census);
            assert_eq!(serde_json::to_value(&e.ledger).unwrap(), ledger);
            e.validate().unwrap();
            h.validate().unwrap();
            p.validate().unwrap();
        }
    }

    #[test]
    fn delayed_trap_cannot_capture_source_that_left_the_surface() {
        let (mut e, mut h, mut p, _, source) = overlapping_trap_encounters(20);
        e.set_surface_observable(source, false, 0).unwrap();
        let before = serde_json::to_vec(&e).unwrap();
        p.advance_traps(20, &mut e, &mut h).unwrap();
        assert!(h.occupants.is_empty());
        assert!(h.traps.values().all(|trap| trap.trapped.is_empty()));
        assert!(p.trap_arrivals.is_empty());
        assert_eq!(serde_json::to_vec(&e).unwrap(), before);
        e.validate().unwrap();
        h.validate().unwrap();
        p.validate().unwrap();
    }

    #[test]
    fn paid_animal_dam_changes_actual_water_links_without_creating_liters() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let patch = (0..600u32)
            .find(|&patch| {
                let (cell, _) = choose_cell(patch, 101, &t, &r, 30, 160).unwrap();
                RiverFixture::local_index(cell).is_some() && water_depth(cell, &t, &r) > 0
            })
            .unwrap();
        let request = actor(101, patch, &mut e, &mut p, &t, &r);
        let before = r.water.totals().stored_liters;
        let job = PendingEngineering {
            request: request.clone(),
            queued_tick: 20,
            actor: Some(request.cohort_id),
        };
        assert!(matches!(
            p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap(),
            Execution::Committed
        ));
        assert_eq!(r.water.totals().stored_liters, before);
        assert!(!p.dam_links.is_empty());
        let links = p.dam_links.values().next().unwrap();
        assert!(links.iter().all(|&i| {
            r.water.links()[i]
                .barrier
                .is_some_and(|b| b.permeability_permille < 1000)
        }));
        assert_eq!(h.dams.values().next().unwrap().invested_wood_units, 4);
        assert_eq!(p.bio_received_units, 4);
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        p.validate().unwrap();
        e.validate().unwrap();
    }
    #[test]
    fn soil_mixing_changes_actual_root_nutrient_access_and_conserves_both_owners() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let request = actor(102, 0, &mut e, &mut p, &t, &r);
        let before = e.root_zones[0].nutrients;
        let job = PendingEngineering {
            request: request.clone(),
            queued_tick: 20,
            actor: Some(request.cohort_id),
        };
        assert!(matches!(
            p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap(),
            Execution::Committed
        ));
        assert!(e.root_zones[0].nutrients < before);
        assert_eq!(
            before - e.root_zones[0].nutrients,
            p.patches[0].lower.nutrients
        );
        assert_eq!(p.patches[0].upper.porosity_permille, 500);
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        p.validate().unwrap();
        e.validate().unwrap();
    }
    #[test]
    fn queued_builder_gets_no_free_work_and_only_real_elapsed_awake_ticks_fund_tunnel() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let c = e
            .cohorts
            .iter()
            .find(|c| e.species[c.species_id as usize].traits.has(12))
            .unwrap()
            .clone();
        let id = e.materialize_one(c.id).unwrap();
        let position = choose_cell(c.patch, 12, &t, &r, 30, 160).unwrap().1;
        p.observe_position(id, position).unwrap();
        e.species[c.species_id as usize].nocturnal = false;
        let request = PhysicalRequest {
            cohort_id: id,
            species_id: c.species_id,
            patch: c.patch,
            trait_id: 12,
            energy_budget: 80,
            individuals: 1,
        };
        e.step(20, &env).unwrap();
        p.step(
            20,
            &[request],
            &mut e,
            &mut h,
            &mut t,
            &mut r,
            &env,
            30,
            160,
        )
        .unwrap();
        assert!(h.passages.is_empty());
        assert!(p.actors.values().all(|a| a.labor_ticks == 0));
        for tick in (40..=1800).step_by(20) {
            e.step(tick, &env).unwrap();
            p.step(tick, &[], &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap();
            if !h.passages.is_empty() {
                break;
            }
        }
        assert!(!h.passages.is_empty());
        p.validate().unwrap();
        e.validate().unwrap();
        h.validate().unwrap();
    }
    #[test]
    fn observed_builder_walks_over_multiple_updates_without_teleport_or_free_work() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        for y in 0..20 {
            for x in 0..20 {
                let i = y * CW + x;
                t.set_elevation_mm(i, 1000).unwrap();
                t.set_water_depth_mm(i, 0).unwrap();
            }
        }
        let source = e
            .cohorts
            .iter()
            .find(|c| {
                e.species[c.species_id as usize].traits.has(46)
                    && e.species[c.species_id as usize].taxon == crate::ecology::Taxon::Predator
            })
            .unwrap()
            .id;
        let id = e.materialize_one(source).unwrap();
        let c = e.cohorts.iter_mut().find(|c| c.id == id).unwrap();
        c.patch = 0;
        let sid = c.species_id;
        e.species[sid as usize].nocturnal = false;
        let request = PhysicalRequest {
            cohort_id: id,
            species_id: sid,
            patch: 0,
            trait_id: 46,
            energy_budget: 80,
            individuals: 1,
        };
        let start = [132000, 84000];
        p.observe_position(id, start).unwrap();
        let census = e.census();
        e.step(20, &env).unwrap();
        p.step(
            20,
            &[request],
            &mut e,
            &mut h,
            &mut t,
            &mut r,
            &env,
            30,
            160,
        )
        .unwrap();
        assert_eq!(p.actors[&id].position_mm, start);
        assert_eq!(p.actors[&id].labor_ticks, 0);
        let mut previous = start;
        for tick in [40, 60, 80, 100] {
            e.step(tick, &env).unwrap();
            p.step(tick, &[], &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap();
            let now = p.actors[&id].position_mm;
            let distance = (i64::from(now[0]) - i64::from(previous[0])).abs()
                + (i64::from(now[1]) - i64::from(previous[1])).abs();
            assert!(
                distance > 0 && distance <= 3000,
                "tick{tick} previous{previous:?} now{now:?} active{:?} rejected{} cenergy{} cstamina{} route{:?}",
                p.active,
                p.rejected,
                e.cohorts
                    .iter()
                    .find(|c| c.id == id)
                    .map_or(0, |c| c.energy),
                e.cohorts
                    .iter()
                    .find(|c| c.id == id)
                    .map_or(0, |c| c.stamina_permille),
                p.actors[&id].local_route
            );
            assert_eq!(p.actors[&id].labor_ticks, 0);
            assert!(h.traps.is_empty());
            p.observe_position(id, start).unwrap();
            assert_eq!(p.actors[&id].position_mm, now);
            previous = now;
        }
        let after = e.census();
        assert_eq!(
            after.wild_animals + after.transit_animals + (after.dead_animals - census.dead_animals),
            census.wild_animals + census.transit_animals
        );
        assert_eq!(e.cohorts.iter().find(|c| c.id == id).unwrap().count, 1);
        p.validate().unwrap();
        e.validate().unwrap();
    }
    #[test]
    fn bounded_local_route_does_not_cross_solid_walls_or_disconnected_ridges() {
        let (mut t, r, e, _, _, _) = world();
        for y in 0..20 {
            for x in 0..20 {
                let i = y * CW + x;
                t.set_elevation_mm(i, 1000).unwrap();
                t.set_water_depth_mm(i, 0).unwrap();
                if x == 10 {
                    t.set_occupancy(i, 1).unwrap();
                }
            }
        }
        let species = e.species.iter().find(|s| s.traits.has(12)).unwrap();
        let start = 10 * CW + 16;
        let target = 0;
        assert!(local_route(start, target, 0, 12, species, &t, &r, 30, 160).is_none());
        for y in 0..20 {
            t.set_occupancy(y * CW + 10, 0).unwrap();
            t.set_elevation_mm(y * CW + 10, 10000).unwrap();
        }
        assert!(local_route(start, target, 0, 12, species, &t, &r, 30, 160).is_none());
    }
    #[test]
    fn animal_obstruction_layers_cannot_open_human_barrier_and_breach_removes_only_own_layer() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        let patch = (0..600u32)
            .find(|&patch| {
                let (cell, _) = choose_cell(patch, 101, &t, &r, 30, 160).unwrap();
                RiverFixture::local_index(cell).is_some() && water_depth(cell, &t, &r) > 0
            })
            .unwrap();
        let (cell, _) = choose_cell(patch, 101, &t, &r, 30, 160).unwrap();
        let links = dam_crossing_links(cell, patch, 30, 160, &r);
        let human = Barrier {
            crest_mm: t.elevation_mm(cell).unwrap() + 5000,
            permeability_permille: 5,
        };
        r.water.set_barrier(links[0], Some(human)).unwrap();
        let before = r.water.totals().stored_liters;
        let request = actor(101, patch, &mut e, &mut p, &t, &r);
        let job = PendingEngineering {
            request: request.clone(),
            queued_tick: 20,
            actor: Some(request.cohort_id),
        };
        assert!(matches!(
            p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                .unwrap(),
            Execution::Committed
        ));
        assert_eq!(r.water.links()[links[0]].barrier, Some(human));
        let id = *h.dams.keys().next().unwrap();
        h.damage_dam(id, 100).unwrap();
        p.reconcile_hydraulics(&h, &t, &mut r).unwrap();
        assert_eq!(r.water.links()[links[0]].barrier, Some(human));
        assert!(
            links
                .iter()
                .skip(1)
                .all(|&i| r.water.links()[i].barrier.is_none())
        );
        assert_eq!(r.water.totals().stored_liters, before);
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        p.validate().unwrap();
        e.validate().unwrap();
    }

    #[test]
    fn actual_canopy_loss_collapses_cavity_and_paid_nest_once_without_new_biomass() {
        let (_, _, mut e, mut h, mut p, env) = world();
        let patch = e
            .plants
            .iter()
            .find(|plant| {
                plant.individuals > 0
                    && plant.patch % 3 != 0
                    && e.species[plant.species_id as usize].traits.has(157)
            })
            .unwrap()
            .patch;
        let source = e
            .cohorts
            .iter()
            .find(|c| c.species_id == 7 && c.sex == crate::ecology::Sex::Female && c.count > 2)
            .unwrap()
            .id;
        // Existing founders pay for this fixture's real clutch. Controlled
        // eligibility does not inject organisms, egg material or reserves.
        e.species[7].preferred_season = 0;
        e.species[7].reproduction = crate::ecology::Reproduction::Eggs;
        for trait_id in [64u16, 65, 96] {
            e.species[7].traits.0[((trait_id - 1) / 32) as usize] &= !(1 << ((trait_id - 1) % 32));
        }
        let mother = e.cohorts.iter_mut().find(|c| c.id == source).unwrap();
        mother.patch = patch;
        mother.next_breeding_tick = 20;
        e.cohorts
            .iter_mut()
            .find(|c| c.species_id == 7 && c.sex == crate::ecology::Sex::Male)
            .unwrap()
            .patch = patch;
        e.step(20, &env).unwrap();
        let (clutch, eggs) = e
            .clutches
            .iter()
            .find(|c| c.mother_id == source)
            .map(|c| (c.id, c.count))
            .unwrap();
        let canopy_animal = e.materialize_one(source).unwrap();
        let cavity_animal = e.materialize_one(source).unwrap();
        let width = e.species[7].body_radius_mm * 2;
        let host = 0x8000_0000_0000_0000u64 + u64::from(patch);
        let xy = patch_center(patch, 30, 160).unwrap();
        h.register_support(
            host,
            patch,
            crate::habitat::Point3Mm::new(xy[0], xy[1], 1000),
            6000,
            8,
        )
        .unwrap();
        let available = e
            .cohorts
            .iter()
            .find(|c| c.id == canopy_animal)
            .unwrap()
            .energy;
        let mut effort = WorkBudget {
            energy: available,
            work_ticks: 600,
        };
        h.climb(canopy_animal, 1, width, host, &mut effort).unwrap();
        e.export_biomass(patch, 8).unwrap();
        p.bio_received_units += 8;
        p.cavity_debris_units += 2;
        let mut wallet = Materials {
            wood_units: 2,
            fiber_units: 4,
            ..Default::default()
        };
        let (nest, _) = h
            .create_shelter(
                patch,
                ShelterKind::Nest,
                Some(host),
                None,
                8,
                width + 50,
                &mut wallet,
                &mut effort,
            )
            .unwrap();
        let (cavity, _) = h
            .create_shelter(
                patch,
                ShelterKind::Cavity,
                Some(host),
                None,
                2,
                width + 50,
                &mut wallet,
                &mut effort,
            )
            .unwrap();
        h.reserve_shelter(cavity, cavity_animal, 1, width).unwrap();
        h.attach_clutch(nest, clutch, eggs).unwrap();
        e.spend_activity_energy(canopy_animal, available - effort.energy)
            .unwrap();
        e.set_surface_observable(canopy_animal, true, 6000).unwrap();
        e.set_surface_observable(cavity_animal, false, 6000)
            .unwrap();
        let live = e.census().wild_animals;
        let health = e
            .cohorts
            .iter()
            .find(|c| c.id == canopy_animal)
            .unwrap()
            .health_permille;
        let before = habitat_materials(&h).unwrap();
        e.damage_plants(patch, 1, false).unwrap();
        p.synchronize_hosts(&mut e, &mut h).unwrap();
        assert!(h.supports[&host].host_alive);
        assert_eq!(h.occupants.len(), 2);
        e.damage_plants(patch, u64::MAX, true).unwrap();
        let biological = e.census();
        p.synchronize_hosts(&mut e, &mut h).unwrap();
        assert!(!h.supports[&host].host_alive);
        assert_eq!(h.shelters[&nest].integrity, 0);
        assert_eq!(h.shelters[&cavity].integrity, 0);
        assert!(h.occupants.is_empty());
        assert!(!e.clutches.iter().any(|c| c.id == clutch));
        assert_eq!(e.census().wild_animals, live);
        assert_eq!(
            e.census().dead_animals,
            biological.dead_animals + u64::from(eggs)
        );
        assert_eq!(h.debris.wood_units, 2);
        assert_eq!(h.debris.fiber_units, 4);
        assert_eq!(habitat_materials(&h).unwrap(), before);
        let fallen = e.cohorts.iter().find(|c| c.id == canopy_animal).unwrap();
        assert!(fallen.surface_observable && fallen.location_height_mm == 0);
        assert!(fallen.health_permille < health);
        let condition = fallen.health_permille;
        p.synchronize_hosts(&mut e, &mut h).unwrap();
        assert_eq!(
            e.cohorts
                .iter()
                .find(|c| c.id == canopy_animal)
                .unwrap()
                .health_permille,
            condition
        );
        assert_eq!(
            e.census().dead_animals,
            biological.dead_animals + u64::from(eggs)
        );
        assert_eq!(habitat_materials(&h).unwrap(), before);
        e.validate().unwrap();
        p.validate().unwrap();
        h.validate().unwrap();
        let mut saved: EcosystemPhysics =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        saved.synchronize_hosts(&mut e, &mut h).unwrap();
        assert_eq!(
            e.census().dead_animals,
            biological.dead_animals + u64::from(eggs)
        );
        assert_eq!(h.debris.wood_units, 2);
    }

    #[test]
    fn paid_nested_nest_and_exact_mother_claim_share_real_den_capacity() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        for y in 0..20 {
            for x in 0..20 {
                t.set_elevation_mm(y * CW + x, 1000).unwrap();
                t.set_water_depth_mm(y * CW + x, 0).unwrap();
                t.set_occupancy(y * CW + x, 0).unwrap();
            }
        }
        let first = actor(12, 0, &mut e, &mut p, &t, &r);
        e.species[first.species_id as usize].traits.insert(64);
        e.species[first.species_id as usize].traits.insert(65);
        let animal = e
            .cohorts
            .iter_mut()
            .find(|c| c.id == first.cohort_id)
            .unwrap();
        animal.sex = crate::ecology::Sex::Female;
        for trait_id in [12, 64, 65] {
            let request = PhysicalRequest {
                trait_id,
                ..first.clone()
            };
            let job = PendingEngineering {
                request,
                queued_tick: 20,
                actor: Some(first.cohort_id),
            };
            assert!(
                matches!(
                    p.execute(&job, &mut e, &mut h, &mut t, &mut r, &env, 30, 160)
                        .unwrap(),
                    Execution::Committed
                ),
                "trait{trait_id}"
            );
            p.validate().unwrap();
        }
        let room = occupied_den_room(&h, first.cohort_id).unwrap();
        let nest = *p.nest_owners.keys().next().unwrap();
        assert_eq!(h.shelters[&nest].burrow_node, Some(room));
        assert_eq!(h.nodes[&room].capacity, 3);
        assert_eq!(h.shelter_capacity(0), 3);
        assert_eq!(h.shelters[&nest].capacity, 2);
        p.synchronize_hazards(20, &mut e, &mut h, &t, &r, 30, 160)
            .unwrap();
        p.synchronize_breeding_refuges(&mut e, &h).unwrap();
        let claim = e
            .cohorts
            .iter()
            .find(|c| c.id == first.cohort_id)
            .unwrap()
            .breeding_refuge
            .as_ref()
            .unwrap();
        assert_eq!(claim.den_slots, 1);
        assert_eq!(claim.nest_id, Some(nest));
        assert_eq!(claim.nest_slots, 2);
        // Another real mother in the same patch cannot borrow this nest. Her
        // physical room reservation also removes a slot from the shared room.
        let source = e
            .cohorts
            .iter()
            .find(|c| c.id != first.cohort_id && c.species_id == first.species_id && c.count > 1)
            .unwrap()
            .id;
        let second = e.materialize_one(source).unwrap();
        let c = e.cohorts.iter_mut().find(|c| c.id == second).unwrap();
        c.patch = 0;
        c.sex = crate::ecology::Sex::Female;
        let energy = c.energy;
        let width = e.species[first.species_id as usize].body_radius_mm * 2;
        let entrance = h
            .nodes
            .values()
            .find(|n| n.kind == crate::habitat::NodeKind::Entrance)
            .unwrap()
            .id;
        let den = h
            .shelters
            .values()
            .find(|s| s.kind == ShelterKind::Den)
            .unwrap()
            .id;
        let mut work = WorkBudget {
            energy,
            work_ticks: 600,
        };
        h.enter_burrow(second, 1, width, entrance).unwrap();
        h.travel_burrow(second, room, &mut work).unwrap();
        h.reserve_shelter(den, second, 1, width).unwrap();
        e.spend_activity_energy(second, energy - work.energy)
            .unwrap();
        p.synchronize_breeding_refuges(&mut e, &h).unwrap();
        let mother = e
            .cohorts
            .iter()
            .find(|c| c.id == first.cohort_id)
            .unwrap()
            .breeding_refuge
            .as_ref()
            .unwrap();
        let other = e
            .cohorts
            .iter()
            .find(|c| c.id == second)
            .unwrap()
            .breeding_refuge
            .as_ref()
            .unwrap();
        assert_eq!(mother.nest_slots, 1);
        assert_eq!(other.den_slots, 1);
        assert_eq!(other.nest_id, None);
        assert_eq!(
            h.attach_clutch(nest, 999_999, 2),
            Err(HabitatError::CapacityExceeded)
        );
        let saved: EcosystemPhysics =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        saved.synchronize_breeding_refuges(&mut e, &h).unwrap();
        assert_eq!(
            e.cohorts
                .iter()
                .find(|c| c.id == first.cohort_id)
                .unwrap()
                .breeding_refuge
                .as_ref()
                .unwrap()
                .nest_slots,
            1
        );
        h.validate().unwrap();
        e.validate().unwrap();
        p.validate().unwrap();
    }

    #[test]
    fn same_cell_final_approach_moves_and_pays_time_across_water_changes_and_save() {
        let (mut t, mut r, mut e, _, mut p, _) = world();
        for y in 0..20 {
            for x in 0..20 {
                let cell = y * CW + x;
                t.set_elevation_mm(cell, 1000).unwrap();
                t.set_water_depth_mm(cell, 0).unwrap();
                t.set_occupancy(cell, 0).unwrap();
                t.cells[cell].cover = 0.;
            }
        }
        let request = actor(12, 0, &mut e, &mut p, &t, &r);
        let species = &mut e.species[request.species_id as usize];
        species.locomotion = crate::ecology::Locomotion::Walking;
        species.habitat = crate::ecology::Habitat::Land;
        species.slow_movement = false;
        let id = request.cohort_id;
        let a = p.actors.get_mut(&id).unwrap();
        a.position_mm = [500, 4000];
        a.route_worksite = Some(0);
        a.labor_ticks = 0;
        let unused = p
            .walk_to_worksite(id, &request, 20, &mut e, &t, &r, 30, 160)
            .unwrap();
        assert_eq!(unused, Some(0));
        assert_eq!(p.actors[&id].position_mm, [2500, 4000]);
        assert_eq!(p.actors[&id].labor_ticks, 0);
        let mut restored: EcosystemPhysics =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        r.advance_to(20).unwrap();
        t.set_elevation_mm(100 * CW + 100, 1001).unwrap();
        assert_ne!(
            r.water.transport_generation(),
            restored.actors[&id].route_water_revision
        );
        let unused = restored
            .walk_to_worksite(id, &request, 20, &mut e, &t, &r, 30, 160)
            .unwrap();
        assert_eq!(restored.actors[&id].position_mm, [4000, 4000]);
        assert_eq!(unused, Some(5));
        assert_eq!(restored.actors[&id].travel_energy_mm, 3500);
        assert_eq!(restored.actors[&id].labor_ticks, 0);
    }

    #[test]
    fn coarse_materialization_uses_connected_real_habitat_and_known_animal_never_teleports() {
        let (mut t, mut r, mut e, mut h, mut known, env) = world();
        for y in 0..20 {
            for x in 0..20 {
                let cell = y * CW + x;
                t.set_elevation_mm(cell, 1000).unwrap();
                t.set_water_depth_mm(cell, if y == 0 { 0 } else { 1000 })
                    .unwrap();
                t.set_occupancy(cell, 0).unwrap();
            }
        }
        let request = actor(12, 0, &mut e, &mut known, &t, &r);
        let species = &mut e.species[request.species_id as usize];
        species.locomotion = crate::ecology::Locomotion::Walking;
        species.habitat = crate::ecology::Habitat::Land;
        species.nocturnal = false;
        known
            .actors
            .get_mut(&request.cohort_id)
            .unwrap()
            .position_mm = [80000, 80000];
        let count = e.census().wild_animals;
        known
            .step(
                20,
                &[request.clone()],
                &mut e,
                &mut h,
                &mut t,
                &mut r,
                &env,
                30,
                160,
            )
            .unwrap();
        assert!(known.active.is_empty());
        assert_eq!(known.actors[&request.cohort_id].position_mm, [80000, 80000]);
        let mut coarse = EcosystemPhysics::new(600);
        coarse
            .step(
                20,
                &[request.clone()],
                &mut e,
                &mut h,
                &mut t,
                &mut r,
                &env,
                30,
                160,
            )
            .unwrap();
        assert_eq!(coarse.active.len(), 1);
        let resolved = &coarse.actors[&request.cohort_id];
        let cell = cell_at(resolved.position_mm).unwrap();
        assert_eq!(cell / CW, 0);
        assert!(organism_cell_usable(
            cell,
            &e.species[request.species_id as usize],
            &t,
            &r
        ));
        assert_eq!(resolved.labor_ticks, 0);
        assert_eq!(e.census().wild_animals, count);
        coarse.validate().unwrap();
        e.validate().unwrap();
    }

    #[test]
    fn stalled_engineers_release_slots_with_saved_cooldowns_and_keep_wallets_and_identity() {
        let (mut t, mut r, mut e, mut h, mut p, env) = world();
        for y in 0..20 {
            for x in 0..20 {
                t.set_elevation_mm(y * CW + x, 1000).unwrap();
                t.set_water_depth_mm(y * CW + x, 0).unwrap();
                t.set_occupancy(y * CW + x, 0).unwrap();
            }
        }
        let mut old = Vec::new();
        for _ in 0..4 {
            let request = actor(12, 0, &mut e, &mut p, &t, &r);
            let a = p.actors.get_mut(&request.cohort_id).unwrap();
            a.labor_ticks = 0;
            a.last_progress_tick = 0;
            let k = key(request.cohort_id, 12);
            p.pending.insert(
                k,
                PendingEngineering {
                    request: request.clone(),
                    queued_tick: 20,
                    actor: Some(request.cohort_id),
                },
            );
            p.active.push(k);
            old.push(request);
        }
        let next = actor(12, 0, &mut e, &mut p, &t, &r);
        e.species[next.species_id as usize].nocturnal = false;
        e.species[next.species_id as usize].habitat = crate::ecology::Habitat::Land;
        e.export_biomass(0, 1).unwrap();
        p.bio_received_units = 1;
        p.actors
            .get_mut(&old[0].cohort_id)
            .unwrap()
            .materials
            .wood_units = 1;
        p.last_tick = STALLED_JOB_TICKS - INTERVAL;
        e.last_tick = STALLED_JOB_TICKS;
        let count = e.census().wild_animals;
        p.step(
            STALLED_JOB_TICKS,
            &[next.clone()],
            &mut e,
            &mut h,
            &mut t,
            &mut r,
            &env,
            30,
            160,
        )
        .unwrap();
        assert_eq!(p.stalled_releases, 4);
        assert_eq!(p.active, vec![key(next.cohort_id, 12)]);
        assert_eq!(p.actors[&old[0].cohort_id].materials.wood_units, 1);
        assert!(
            old.iter()
                .all(|r| p.retry_after[&key(r.cohort_id, 12)] > STALLED_JOB_TICKS)
        );
        let mut saved: EcosystemPhysics =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        e.last_tick += INTERVAL;
        saved
            .step(
                STALLED_JOB_TICKS + INTERVAL,
                &old,
                &mut e,
                &mut h,
                &mut t,
                &mut r,
                &env,
                30,
                160,
            )
            .unwrap();
        assert_eq!(saved.active, vec![key(next.cohort_id, 12)]);
        assert_eq!(saved.actors[&old[0].cohort_id].materials.wood_units, 1);
        assert_eq!(e.census().wild_animals, count);
        saved.validate().unwrap();
        e.validate().unwrap();
    }

    #[test]
    fn verified_custody_handoff_preserves_wallet_and_counts_cancels_old_jobs_and_saves_exact_xy() {
        let (t, r, mut e, mut h, mut p, _) = world();
        let request = actor(11, 0, &mut e, &mut p, &t, &r);
        let id = request.cohort_id;
        p.last_tick = 20;
        let source = e
            .cohorts
            .iter()
            .find(|c| c.id != id && c.species_id == request.species_id && c.count > 1)
            .unwrap()
            .id;
        let other = e
            .cohorts
            .iter()
            .find(|c| {
                c.id != id
                    && c.id != source
                    && c.count > 0
                    && e.species[c.species_id as usize].traits.has(11)
            })
            .unwrap()
            .clone();
        let original = p.actors[&id].position_mm;
        e.export_biomass(0, 13).unwrap();
        p.bio_received_units = 13;
        let mut wallet = Materials {
            wood_units: 5,
            fiber_units: 8,
            ..Default::default()
        };
        let reserve = e.cohorts.iter().find(|c| c.id == id).unwrap().energy;
        let mut effort = WorkBudget {
            energy: reserve,
            work_ticks: 600,
        };
        let width = e.species[request.species_id as usize].body_radius_mm * 2;
        h.register_support(
            99,
            0,
            crate::habitat::Point3Mm::new(original[0], original[1], 0),
            3000,
            8,
        )
        .unwrap();
        h.climb(id, 1, width, 99, &mut effort).unwrap();
        let (nest, _) = h
            .create_shelter(
                0,
                ShelterKind::Nest,
                Some(99),
                None,
                2,
                width + 50,
                &mut wallet,
                &mut effort,
            )
            .unwrap();
        e.spend_activity_energy(id, reserve - effort.energy)
            .unwrap();
        e.set_surface_observable(id, true, 3000).unwrap();
        p.actors.get_mut(&id).unwrap().materials = wallet;
        p.actors.get_mut(&id).unwrap().travel_energy_mm = 4321;
        p.actors.get_mut(&id).unwrap().labor_ticks = 50;
        p.actors.get_mut(&id).unwrap().route_worksite = Some(cell_at(original).unwrap());
        p.actors.get_mut(&id).unwrap().local_route = vec![cell_at(original).unwrap()];
        p.nest_owners.insert(nest, id);
        p.source_actor.insert(source, id);
        let active_key = key(source, 11);
        p.pending.insert(
            active_key,
            PendingEngineering {
                request: PhysicalRequest {
                    cohort_id: source,
                    ..request.clone()
                },
                queued_tick: 20,
                actor: Some(id),
            },
        );
        p.active.push(active_key);
        let own_key = key(id, 65);
        p.pending.insert(
            own_key,
            PendingEngineering {
                request: PhysicalRequest {
                    trait_id: 65,
                    ..request.clone()
                },
                queued_tick: 20,
                actor: None,
            },
        );
        let other_key = key(other.id, 11);
        p.pending.insert(
            other_key,
            PendingEngineering {
                request: PhysicalRequest {
                    cohort_id: other.id,
                    species_id: other.species_id,
                    patch: other.patch,
                    trait_id: 11,
                    energy_budget: 80,
                    individuals: other.count,
                },
                queued_tick: 20,
                actor: None,
            },
        );
        p.retry_after.insert(own_key, 1000);
        p.retry_after.insert(other_key, 1000);
        p.stored_habitat_material = habitat_materials(&h).unwrap();
        p.observe_position(id, [original[0] + 8000, original[1]])
            .unwrap();
        assert_eq!(p.actors[&id].position_mm, original);
        let before = e.census();
        let actual_material = habitat_materials(&h).unwrap();
        e.observe_cohort(0, id, true).unwrap();
        let captive = e.capture_cohort(id, 0).unwrap();
        assert_eq!(
            captive, id,
            "capturing an already materialized individual must preserve identity"
        );
        e.set_captive_transit(captive, 1).unwrap();
        e.observe(0, 1, true).unwrap();
        let released = e.release(0, request.species_id, 1).unwrap();
        assert_eq!(released, id);
        e.set_surface_observable(released, true, 0).unwrap();
        e.set_breeding_refuge(released, None).unwrap();
        let delivery = [191234, 51234];
        p.transport_completed(released, 1, delivery, &mut h)
            .unwrap();
        assert_eq!(p.actors[&id].position_mm, delivery);
        assert_eq!(p.actors[&id].patch, 1);
        assert_eq!(p.actors[&id].materials, wallet);
        assert_eq!(p.actors[&id].travel_energy_mm, 4321);
        assert_eq!(p.actors[&id].labor_ticks, 0);
        assert!(p.actors[&id].local_route.is_empty());
        assert_eq!(p.actors[&id].route_worksite, None);
        assert!(!h.occupants.contains_key(&id));
        assert_eq!(h.shelters[&nest].integrity, 100);
        assert_eq!(habitat_materials(&h).unwrap(), actual_material);
        assert!(!p.nest_owners.contains_key(&nest));
        assert!(!p.source_actor.values().any(|&actor| actor == id));
        assert_eq!(
            p.pending.keys().copied().collect::<Vec<_>>(),
            vec![other_key]
        );
        assert!(p.active.is_empty());
        assert!(!p.retry_after.contains_key(&own_key));
        assert!(p.retry_after.contains_key(&other_key));
        assert_eq!(
            p.tracked_position(id, &h),
            Some([delivery[0], delivery[1], 0])
        );
        let after = e.census();
        assert_eq!(
            after.wild_animals + after.captive_animals + after.transit_animals,
            before.wild_animals + before.captive_animals + before.transit_animals
        );
        assert_eq!(after.biomass_units, before.biomass_units);
        assert_eq!(after.animal_hydration, before.animal_hydration);
        let mut saved: EcosystemPhysics =
            serde_json::from_slice(&serde_json::to_vec(&p).unwrap()).unwrap();
        saved.observe_position(id, original).unwrap();
        assert_eq!(saved.actors[&id].position_mm, delivery);
        assert_eq!(saved.actors[&id].materials, wallet);
        assert_eq!(saved.actors[&id].travel_energy_mm, 4321);
        assert_eq!(saved.active.len(), 0);
        saved.validate().unwrap();
        e.validate().unwrap();
        h.validate().unwrap();
        let unchanged = serde_json::to_value(&saved).unwrap();
        assert!(
            saved
                .transport_completed(id, 600, delivery, &mut h)
                .is_err()
        );
        assert!(saved.transport_completed(id, 1, [-1, 0], &mut h).is_err());
        assert_eq!(serde_json::to_value(&saved).unwrap(), unchanged);
    }
}
