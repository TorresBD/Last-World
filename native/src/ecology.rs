//! Finite, CPU-authoritative macro ecology for the first connected native slice.
//!
//! Cohorts are plain records, not ECS entities. Captured/observed organisms keep
//! exact identity and condition when split out. Material and water have separate
//! auditable ledgers. The environment carries conditions, never copied stocks.
#[path = "ecology_behavior.rs"]
mod behavior;
use serde::{Deserialize, Serialize};

pub const ECOLOGY_INTERVAL: u64 = 20;
pub const SEASON_TICKS: u64 = 72_000;
pub const SELECTED_TRAITS: [u16; 30] = [
    11, 12, 46, 61, 64, 65, 67, 72, 75, 84, 86, 93, 96, 101, 102, 103, 104, 105, 106, 107, 121,
    125, 128, 129, 132, 135, 143, 156, 157, 159,
];
const PHYSICAL_TRAITS: [u16; 11] = [11, 12, 46, 64, 65, 67, 101, 102, 103, 104, 105];
const NEW_BODY: u64 = 24;
const NEW_RESERVE: u64 = 16;
const ADULT_BODY: u64 = 64;

fn add(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| "Ecology quantity overflow".into())
}
fn sub(a: u64, b: u64) -> Result<u64, String> {
    a.checked_sub(b)
        .ok_or_else(|| "Ecology quantity underflow".into())
}

fn h(seed: u64, a: u64) -> u64 {
    let mut x = seed ^ a.wrapping_mul(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraitMask(pub [u32; 5]);
impl TraitMask {
    pub fn has(&self, id: u16) -> bool {
        (1..=160).contains(&id) && self.0[((id - 1) / 32) as usize] & (1 << ((id - 1) % 32)) != 0
    }
    pub fn insert(&mut self, id: u16) {
        assert!((1..=160).contains(&id));
        self.0[((id - 1) / 32) as usize] |= 1 << ((id - 1) % 32);
    }
    pub fn ids(&self) -> Vec<u16> {
        (1..=160).filter(|&id| self.has(id)).collect()
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Taxon {
    Plant,
    Herbivore,
    Predator,
    Pollinator,
    Fungus,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Locomotion {
    Rooted,
    Walking,
    Climbing,
    Flying,
    Swimming,
    Burrowing,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Habitat {
    Land,
    Water,
    Amphibious,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Reproduction {
    Seeds,
    Eggs,
    LiveBirth,
    Spores,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Sex {
    Female,
    Male,
    Hermaphrodite,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Stage {
    Juvenile,
    Adult,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Custody {
    Wild(u32),
    Captive(u8),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Species {
    pub id: u32,
    pub name: String,
    pub taxon: Taxon,
    pub traits: TraitMask,
    pub body_mass_g: u32,
    pub body_radius_mm: u32,
    pub limbs: u8,
    pub locomotion: Locomotion,
    pub habitat: Habitat,
    pub food_species: Vec<u32>,
    pub minimum_temperature_milli_c: i32,
    pub maximum_temperature_milli_c: i32,
    pub metabolism_units_per_step: u16,
    pub water_units_per_step: u16,
    #[serde(default = "default_physiology_period")]
    pub metabolism_period_ticks: u64,
    #[serde(default = "default_water_period")]
    pub water_period_ticks: u64,
    #[serde(default = "default_land_habitat")]
    pub juvenile_habitat: Habitat,
    #[serde(default = "default_plant_output")]
    pub plant_output_units: u16,
    #[serde(default = "default_leaf_share")]
    pub leaf_output_permille: u16,
    #[serde(default = "default_nectar_share")]
    pub nectar_output_permille: u16,
    pub maturity_ticks: u64,
    pub lifespan_ticks: u64,
    pub gestation_ticks: u64,
    pub clutch_size: u16,
    pub reproduction: Reproduction,
    pub preferred_season: u8,
    pub nocturnal: bool,
    pub docile: bool,
    pub slow_movement: bool,
    pub milk_production: bool,
    pub social_group_size: u16,
    pub territorial_radius_m: u16,
    pub threat_response: u8,
    pub flood_tolerance_ticks: u64,
}
fn default_physiology_period() -> u64 {
    400
}
fn default_water_period() -> u64 {
    800
}
fn default_land_habitat() -> Habitat {
    Habitat::Land
}
fn default_plant_output() -> u16 {
    10
}
fn default_leaf_share() -> u16 {
    700
}
fn default_nectar_share() -> u16 {
    100
}
/// Integer fourth-root allometry: mass-specific turnover slows with size.
/// Reference50g: one normalized reserve unit per20sim-seconds; parameters are
/// development biology compression, not a real-world metabolic calibration.
fn physiology_period(body_mass_g: u32) -> u64 {
    let value = body_mass_g.max(50) as u64 * 200;
    let mut root = 1u64;
    while (root + 1).pow(4) <= value {
        root += 1
    }
    root * 2 * 20
}
impl Species {
    pub fn locomotion_for(&self, stage: Stage) -> Locomotion {
        if stage == Stage::Juvenile && self.traits.has(96) {
            if self.juvenile_habitat == Habitat::Water {
                Locomotion::Swimming
            } else {
                Locomotion::Walking
            }
        } else {
            self.locomotion
        }
    }
    pub fn habitat_for(&self, stage: Stage) -> Habitat {
        if stage == Stage::Juvenile && self.traits.has(96) {
            self.juvenile_habitat
        } else {
            self.habitat
        }
    }
    pub fn breeding_interval_ticks(&self) -> u64 {
        self.gestation_ticks
            .saturating_mul(2)
            .max(self.maturity_ticks / 2)
            .max(SEASON_TICKS / 24)
    }
    fn basal_period(&self, stage: Stage) -> u64 {
        if stage == Stage::Juvenile {
            (self.metabolism_period_ticks * 3 / 4).max(ECOLOGY_INTERVAL)
        } else {
            self.metabolism_period_ticks
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EcologyPatch {
    pub water_depth_mm: u32,
    pub moisture_permille: u16,
    pub temperature_milli_c: i32,
    pub disturbance: u16,
    pub accessible: bool,
    pub shelter_capacity: u32,
    pub canopy_connected: bool,
    pub flow_to: Option<u32>,
    pub flow_fraction_permille: u16,
    #[serde(default = "all_connections")]
    pub land_connections: u8,
    #[serde(default = "all_connections")]
    pub water_connections: u8,
}
fn all_connections() -> u8 {
    15
}
impl Default for EcologyPatch {
    fn default() -> Self {
        Self {
            water_depth_mm: 0,
            moisture_permille: 600,
            temperature_milli_c: 26_000,
            disturbance: 0,
            accessible: true,
            shelter_capacity: 0,
            canopy_connected: true,
            flow_to: None,
            flow_fraction_permille: 0,
            land_connections: 15,
            water_connections: 15,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RootZone {
    pub shallow_water: u64,
    pub deep_water: u64,
    pub nutrients: u64,
    pub litter: u64,
    pub fungal_biomass: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlantPatch {
    pub patch: u32,
    pub species_id: u32,
    pub individuals: u32,
    pub age_ticks: u64,
    pub shoots: u64,
    pub roots: u64,
    pub leaves: u64,
    pub fruit: u64,
    pub nectar: u64,
    pub seeds: u64,
    pub pollen: u64,
    pub serviced_pollen: u64,
    pub submerged_ticks: u64,
    pub initial_live_units: u64,
    #[serde(default)]
    pub initial_root_units: u64,
}
impl PlantPatch {
    pub fn live_units(&self) -> u64 {
        self.shoots
            + self.roots
            + self.leaves
            + self.fruit
            + self.nectar
            + self.pollen
            + self.serviced_pollen
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PollenTicket {
    pub species_id: u32,
    pub source_patch: u32,
    pub units: u64,
    pub expires_tick: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedTicket {
    pub species_id: u32,
    pub units: u64,
    pub due_tick: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpermTicket {
    pub donor_species: u32,
    pub doses: u32,
    pub expires_tick: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreedingRefuge {
    pub den_id: Option<u64>,
    pub den_slots: u32,
    pub nest_id: Option<u64>,
    pub nest_slots: u32,
    pub accessible: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cohort {
    pub id: u64,
    pub species_id: u32,
    pub patch: u32,
    pub count: u32,
    pub sex: Sex,
    pub stage: Stage,
    pub age_ticks: u64,
    pub body_units: u64,
    pub energy: u64,
    pub hydration: u64,
    pub health_permille: u16,
    pub stamina_permille: u16,
    pub dormant_until: u64,
    pub next_breeding_tick: u64,
    pub milk: u64,
    pub pollen: Option<PollenTicket>,
    pub carried_seeds: Vec<SeedTicket>,
    pub sperm: Option<SpermTicket>,
    pub location_height_mm: i32,
    pub surface_observable: bool,
    pub parent_id: Option<u64>,
    pub engineering_busy_until: u64,
    #[serde(default)]
    pub breeding_refuge: Option<BreedingRefuge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transit {
    pub organism: Cohort,
    pub destination: u32,
    #[serde(default)]
    pub from_patch: u32,
    pub arrival_tick: u64,
    #[serde(default)]
    pub departure_tick: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Housing {
    pub id: u64,
    pub patch: u32,
    pub capacity: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Captive {
    pub animal: Cohort,
    pub team: u8,
    pub care_food: u64,
    pub care_water: u64,
    pub breed_enabled: bool,
    pub housing: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clutch {
    pub id: u64,
    pub species_id: u32,
    pub count: u32,
    pub mother_id: u64,
    pub custody: Custody,
    pub patch: u32,
    pub housing: Option<u64>,
    pub hatch_tick: u64,
    pub material: u64,
    pub water: u64,
    pub temperature_sum: i64,
    pub temperature_samples: u64,
    pub guarded: bool,
    pub reproduction: Reproduction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeedStock {
    pub species_id: u32,
    pub patch: u32,
    pub units: u64,
    pub expires_tick: u64,
    pub afloat: bool,
    pub advection_remainder: u16,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Knowledge {
    pub observed: bool,
    pub study_ticks: u64,
    pub enabled: bool,
    pub observed_tick: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EcologyLedger {
    pub initial_material: u64,
    pub material_imported: u64,
    pub material_exported: u64,
    pub respired_material: u64,
    pub initial_water: u64,
    pub rain_water: u64,
    pub transferred_water_in: u64,
    pub water_exported: u64,
    pub evaporated_water: u64,
    pub initial_animals: u64,
    pub conceived_animals: u64,
    pub dead_animals: u64,
    #[serde(default)]
    pub death_reasons: std::collections::BTreeMap<String, u64>,
    pub initial_plants: u64,
    pub germinated_plants: u64,
    pub dead_plants: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DamageReport {
    pub animals_killed: u64,
    pub plants_killed: u64,
    pub plant_units_removed: u64,
    pub material_to_litter: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhysicalRequest {
    pub cohort_id: u64,
    pub species_id: u32,
    pub patch: u32,
    pub trait_id: u16,
    pub energy_budget: u64,
    pub individuals: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EcologyEffect {
    Engineering(PhysicalRequest),
    Migration {
        cohort_id: u64,
        source: u32,
        destination: u32,
        count: u32,
    },
    Birth {
        species_id: u32,
        patch: u32,
        count: u32,
    },
    Extinction {
        species_id: u32,
    },
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Census {
    pub wild_animals: u64,
    pub generation_accepted: bool,
    pub generation_relocations: u64,
    pub transit_animals: u64,
    pub captive_animals: u64,
    pub viable_offspring: u64,
    pub living_plants: u64,
    pub viable_seeds: u64,
    pub biomass_units: u64,
    pub root_zone_water: u64,
    pub animal_hydration: u64,
    pub dead_animals: u64,
    pub death_reasons: std::collections::BTreeMap<String, u64>,
    pub dead_plants: u64,
    pub extinct_species: Vec<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ecology {
    pub seed: u32,
    #[serde(default)]
    pub generation_accepted: bool,
    #[serde(default)]
    pub generation_relocations: u64,
    #[serde(default)]
    threat_until: Vec<u64>,
    pub patch_width: usize,
    pub last_tick: u64,
    pub species: Vec<Species>,
    pub plants: Vec<PlantPatch>,
    pub root_zones: Vec<RootZone>,
    pub cohorts: Vec<Cohort>,
    pub transits: Vec<Transit>,
    pub captives: Vec<Captive>,
    #[serde(default)]
    pub housing: [Vec<Housing>; 2],
    pub clutches: Vec<Clutch>,
    pub seeds: Vec<SeedStock>,
    pub knowledge: [Vec<Knowledge>; 2],
    pub visible_patches: [Vec<bool>; 2],
    pub ledger: EcologyLedger,
    pub extinct: Vec<bool>,
    pub next_id: u64,
    pub physical_effects_committed: [u64; 30],
    cached_canopy: Vec<u16>,
    cached_ground_cover: Vec<u16>,
    #[serde(default)]
    cached_root_reinforcement: Vec<u16>,
    cached_fraction: Vec<u16>,
    cached_rich_litter: Vec<bool>,
    plant_patch_offsets: Vec<usize>,
    plant_patch_indices: Vec<usize>,
    #[serde(default)]
    nursing_parent_ids: Vec<u64>,
    #[serde(skip)]
    regional_counts: std::collections::BTreeMap<(u32, u32), u32>,
}

impl Ecology {
    pub fn new(seed: u32, patch_count: usize) -> Result<Self, String> {
        if patch_count == 0 || patch_count > 1_000_000 {
            return Err("Ecology patch count outside1..=1,000,000".into());
        }
        let species = generate_species(seed);
        let mut e = Self {
            seed,
            generation_accepted: false,
            generation_relocations: 0,
            threat_until: vec![0; patch_count],
            patch_width: patch_count,
            last_tick: 0,
            root_zones: vec![
                RootZone {
                    shallow_water: 8_000,
                    deep_water: 20_000,
                    nutrients: 12_000,
                    litter: 0,
                    fungal_biomass: 80
                };
                patch_count
            ],
            plants: Vec::new(),
            cohorts: Vec::new(),
            transits: Vec::new(),
            captives: Vec::new(),
            housing: [Vec::new(), Vec::new()],
            clutches: Vec::new(),
            seeds: Vec::new(),
            knowledge: [
                vec![Knowledge::default(); species.len()],
                vec![Knowledge::default(); species.len()],
            ],
            visible_patches: [vec![false; patch_count], vec![false; patch_count]],
            extinct: vec![false; species.len()],
            species,
            ledger: EcologyLedger::default(),
            next_id: 1,
            physical_effects_committed: [0; 30],
            cached_canopy: vec![0; patch_count],
            cached_ground_cover: vec![0; patch_count],
            cached_root_reinforcement: vec![0; patch_count],
            cached_fraction: vec![0; patch_count],
            cached_rich_litter: vec![false; patch_count],
            plant_patch_offsets: vec![0; patch_count + 1],
            plant_patch_indices: Vec::new(),
            nursing_parent_ids: Vec::new(),
            regional_counts: std::collections::BTreeMap::new(),
        };
        for patch in 0..patch_count {
            let herb = 6 + (h(seed as u64, patch as u64) % 6) as u32;
            let foods = &e.species[herb as usize].food_species;
            let ground_foods: Vec<u32> = foods
                .iter()
                .copied()
                .filter(|&id| {
                    !e.species[id as usize].traits.has(157)
                        || e.species[herb as usize].locomotion == Locomotion::Flying
                })
                .collect();
            let compatible = if ground_foods.is_empty() {
                foods.clone()
            } else {
                ground_foods
            };
            for j in 0..2 {
                let sid = compatible[(h(seed as u64 + 37, patch as u64 * 2 + j)
                    % compatible.len() as u64) as usize];
                let p = PlantPatch {
                    patch: patch as u32,
                    species_id: sid,
                    individuals: 20,
                    age_ticks: 4_000,
                    shoots: 5_000,
                    roots: 2_000,
                    leaves: 1_000,
                    fruit: 300,
                    nectar: 100,
                    seeds: 120,
                    pollen: 100,
                    serviced_pollen: 0,
                    submerged_ticks: 0,
                    initial_live_units: 8_500,
                    initial_root_units: 2000,
                };
                e.plants.push(p);
            }
            // Explicit finite founding stocks; generation is called only for a new world.
            // Two reproductive cohorts share an age band; no minimum stock is maintained.
            for sex in [Sex::Female, Sex::Male] {
                let id = e.alloc_id()?;
                e.cohorts.push(Cohort {
                    id,
                    species_id: herb,
                    patch: patch as u32,
                    count: 12,
                    sex,
                    stage: Stage::Adult,
                    age_ticks: 6_000,
                    body_units: 12 * ADULT_BODY,
                    energy: 12 * 120,
                    hydration: 12 * 80,
                    health_permille: 1000,
                    stamina_permille: 1000,
                    dormant_until: 0,
                    next_breeding_tick: 600,
                    milk: 0,
                    pollen: None,
                    carried_seeds: Vec::new(),
                    sperm: None,
                    location_height_mm: 0,
                    surface_observable: true,
                    parent_id: None,
                    engineering_busy_until: 0,
                    breeding_refuge: None,
                });
            }
            if patch % 3 == 0 {
                for sex in [Sex::Female, Sex::Male] {
                    let id = e.alloc_id()?;
                    e.cohorts.push(Cohort {
                        id,
                        species_id: 12 + (h(seed as u64 + 211, patch as u64) % 2) as u32,
                        patch: patch as u32,
                        count: 1,
                        sex,
                        stage: Stage::Adult,
                        age_ticks: 6_000,
                        body_units: ADULT_BODY,
                        energy: 120,
                        hydration: 80,
                        health_permille: 1000,
                        stamina_permille: 1000,
                        dormant_until: 0,
                        next_breeding_tick: 1_200,
                        milk: 0,
                        pollen: None,
                        carried_seeds: Vec::new(),
                        sperm: None,
                        location_height_mm: 0,
                        surface_observable: true,
                        parent_id: None,
                        engineering_busy_until: 0,
                        breeding_refuge: None,
                    });
                }
            }
        }
        e.ledger.initial_material = e.material_total()?;
        e.ledger.initial_water = e.water_total()?;
        e.ledger.initial_animals = e.animal_total()?;
        e.ledger.initial_plants = e.plants.iter().map(|p| p.individuals as u64).sum();
        e.refresh_fields();
        e.validate()?;
        Ok(e)
    }
    /// Final new-world generation pass. No organism is created/deleted here.
    /// Incompatible placements are resolved before the authoritativeclock starts.
    pub fn accept_initial_habitat(&mut self, environment: &[EcologyPatch]) -> Result<(), String> {
        if self.last_tick != 0
            || self.generation_accepted
            || self.ledger.dead_animals != 0
            || self.ledger.conceived_animals != 0
            || !self.captives.is_empty()
            || !self.transits.is_empty()
        {
            return Err("Habitat acceptance is restricted to an unstarted new generation".into());
        }
        if environment.len() != self.root_zones.len() {
            return Err("Initial habitat dimensions mismatch".into());
        }
        let any_water = environment
            .iter()
            .any(|e| e.accessible && e.water_depth_mm >= 20);
        if !any_water {
            for s in &mut self.species {
                if s.traits.has(96) && s.juvenile_habitat == Habitat::Water {
                    s.juvenile_habitat = Habitat::Land;
                }
            }
        }
        let before = self.animal_total()?;
        let mut placements = Vec::with_capacity(self.cohorts.len());
        let mut assigned = vec![0u64; environment.len()];
        for c in &self.cohorts {
            let species = &self.species[c.species_id as usize];
            let current = c.patch as usize;
            let choice = (0..environment.len())
                .filter(|&p| {
                    Self::habitat_usable(species, c.stage, &environment[p])
                        && self.resource_goal(c, species, p, 0, environment)
                        && self.root_zones[p].shallow_water > 0
                })
                .min_by_key(|&p| {
                    let dx = (p % self.patch_width).abs_diff(current % self.patch_width);
                    let dy = (p / self.patch_width).abs_diff(current / self.patch_width);
                    (dx + dy) * 100 + assigned[p] as usize / 24
                });
            let Some(target) = choice else {
                return Err(format!(
                    "Generated species{} has no compatible food/water habitat; reject this draft seed",
                    c.species_id
                ));
            };
            assigned[target] = add(assigned[target], c.count as u64)?;
            placements.push(target as u32);
        }
        for (c, patch) in self.cohorts.iter_mut().zip(placements) {
            if c.patch != patch {
                self.generation_relocations = add(self.generation_relocations, c.count as u64)?;
                c.patch = patch;
            }
        }
        if before != self.animal_total()? {
            return Err("Generation relocation changed organism count".into());
        }
        self.generation_accepted = true;
        self.validate()?;
        Ok(())
    }
    pub fn set_patch_width(&mut self, width: usize) -> Result<(), String> {
        if width == 0 || !self.root_zones.len().is_multiple_of(width) {
            return Err("Invalid ecological patch width".into());
        }
        self.patch_width = width;
        Ok(())
    }
    fn neighbor(&self, patch: usize, salt: u64) -> Option<usize> {
        let x = patch % self.patch_width;
        let y = patch / self.patch_width;
        let height = self.root_zones.len() / self.patch_width;
        let mut options = [None; 4];
        if x > 0 {
            options[0] = Some(patch - 1)
        }
        if x + 1 < self.patch_width {
            options[1] = Some(patch + 1)
        }
        if y > 0 {
            options[2] = Some(patch - self.patch_width)
        }
        if y + 1 < height {
            options[3] = Some(patch + self.patch_width)
        }
        let start = (h(self.seed as u64, salt) % 4) as usize;
        (0..4).find_map(|i| options[(start + i) % 4])
    }
    fn habitat_usable(s: &Species, stage: Stage, e: &EcologyPatch) -> bool {
        e.accessible
            && match s.habitat_for(stage) {
                Habitat::Land => e.water_depth_mm < 500,
                Habitat::Water => e.water_depth_mm >= 20,
                Habitat::Amphibious => true,
            }
    }
    fn connected(
        &self,
        s: &Species,
        stage: Stage,
        from: usize,
        to: usize,
        env: &[EcologyPatch],
    ) -> bool {
        if s.locomotion_for(stage) == Locomotion::Flying {
            return true;
        }
        let (out, back) = if to + 1 == from {
            (1, 2)
        } else if from + 1 == to {
            (2, 1)
        } else if to + self.patch_width == from {
            (4, 8)
        } else if from + self.patch_width == to {
            (8, 4)
        } else {
            return false;
        };
        let mask = |e: &EcologyPatch| match s.habitat_for(stage) {
            Habitat::Land => e.land_connections,
            Habitat::Water => e.water_connections,
            Habitat::Amphibious => e.land_connections | e.water_connections,
        };
        mask(&env[from]) & out != 0 && mask(&env[to]) & back != 0
    }
    fn resource_goal(
        &self,
        c: &Cohort,
        s: &Species,
        patch: usize,
        kind: u8,
        env: &[EcologyPatch],
    ) -> bool {
        if kind == 1 {
            return self.root_zones[patch].shallow_water >= c.count as u64 * 4;
        }
        if kind == 3 {
            return env[patch].water_depth_mm >= 20;
        }
        if kind == 4 {
            return Self::habitat_usable(s, c.stage, &env[patch]);
        }
        if kind == 6 {
            return self.nearby_threats(patch) < self.nearby_threats(c.patch as usize);
        }
        if kind == 7 {
            let pop = |p: u32| {
                self.regional_counts
                    .get(&(p, c.species_id))
                    .copied()
                    .unwrap_or(0)
            };
            let source = behavior::destination_bias(
                pop(c.patch).saturating_add(c.count),
                s.social_group_size,
                s.territorial_radius_m,
                160 * 160,
                self.nearby_threats(c.patch as usize),
                s.threat_response,
            );
            let target = behavior::destination_bias(
                pop(patch as u32).saturating_add(c.count),
                s.social_group_size,
                s.territorial_radius_m,
                160 * 160,
                self.nearby_threats(patch),
                s.threat_response,
            );
            return target > source && self.resource_goal(c, s, patch, 0, env);
        }
        if kind == 5 {
            return env[patch].moisture_permille
                > env[c.patch as usize].moisture_permille.saturating_add(100)
                && self.resource_goal(c, s, patch, 0, env);
        }
        if s.taxon == Taxon::Predator {
            return self.cohorts.iter().any(|p| {
                p.patch == patch as u32
                    && p.count > 0
                    && p.surface_observable
                    && p.location_height_mm >= 0
                    && s.food_species.contains(&p.species_id)
            });
        }
        if c.stage == Stage::Juvenile && s.traits.has(96) && s.juvenile_habitat == Habitat::Water {
            return self.root_zones[patch].litter >= c.count as u64;
        }
        (self.plant_patch_offsets[patch]..self.plant_patch_offsets[patch + 1]).any(|index| {
            let p = &self.plants[self.plant_patch_indices[index]];
            if p.individuals == 0 {
                return false;
            }
            if kind == 2 {
                return c.pollen.as_ref().is_some_and(|ticket| {
                    ticket.species_id == p.species_id && ticket.source_patch != patch as u32
                }) && p.nectar > 0;
            }
            s.food_species.contains(&p.species_id)
                && (if s.taxon == Taxon::Pollinator
                    && !(c.stage == Stage::Juvenile && s.traits.has(96))
                {
                    p.nectar >= c.count as u64
                } else {
                    p.leaves + p.fruit >= c.count as u64
                })
                && (!self.species[p.species_id as usize].traits.has(157)
                    || p.shoots < 1000
                    || s.locomotion_for(c.stage) == Locomotion::Flying)
        })
    }
    /// Bounded habitat search supplies one real adjacent route segment. It
    /// never consumes remote food or teleports to the eventual target.
    fn seek_destination(
        &self,
        c: &Cohort,
        s: &Species,
        env: &[EcologyPatch],
        kind: u8,
    ) -> Option<usize> {
        let origin = c.patch as usize;
        let rows = self.root_zones.len() / self.patch_width;
        let mut visited = vec![false; env.len()];
        visited[origin] = true;
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((origin, origin, 0u8));
        while let Some((p, first, depth)) = queue.pop_front() {
            if depth > 0 && self.resource_goal(c, s, p, kind, env) {
                return Some(first);
            }
            if depth >= 4 {
                continue;
            }
            let x = p % self.patch_width;
            let y = p / self.patch_width;
            let next = [
                if x > 0 { Some(p - 1) } else { None },
                if x + 1 < self.patch_width {
                    Some(p + 1)
                } else {
                    None
                },
                if y > 0 {
                    Some(p - self.patch_width)
                } else {
                    None
                },
                if y + 1 < rows {
                    Some(p + self.patch_width)
                } else {
                    None
                },
            ];
            for to in next.into_iter().flatten() {
                if !visited[to]
                    && Self::habitat_usable(s, c.stage, &env[to])
                    && self.connected(s, c.stage, p, to, env)
                {
                    visited[to] = true;
                    queue.push_back((to, if depth == 0 { to } else { first }, depth + 1));
                }
            }
        }
        None
    }
    fn forage_destination(&self, c: &Cohort, s: &Species, env: &[EcologyPatch]) -> Option<usize> {
        self.seek_destination(c, s, env, 0)
    }
    fn travel_ticks(s: &Species, stage: Stage) -> u64 {
        if s.locomotion_for(stage) == Locomotion::Flying {
            80
        } else if s.slow_movement {
            800
        } else {
            400
        }
    }
    pub fn available_engineering_biomass(&self, patch: u32) -> u64 {
        self.plants
            .iter()
            .filter(|p| p.patch == patch)
            .map(|p| p.shoots + p.leaves)
            .sum()
    }
    /// Prepare/check a physical transaction first, then transfer its exact spent
    /// wood/fiber units out of living plant stock into the structure's owner.
    pub fn export_biomass(&mut self, patch: u32, amount: u64) -> Result<(), String> {
        self.check_patch(patch)?;
        if self.available_engineering_biomass(patch) < amount {
            return Err("Insufficient finite engineering biomass".into());
        }
        let exported = add(self.ledger.material_exported, amount)?;
        let mut left = amount;
        for p in self.plants.iter_mut().filter(|p| p.patch == patch) {
            let shoot = p.shoots.min(left);
            p.shoots -= shoot;
            left -= shoot;
            let leaves = p.leaves.min(left);
            p.leaves -= leaves;
            left -= leaves;
            if left == 0 {
                break;
            }
        }
        self.ledger.material_exported = exported;
        self.refresh_fields();
        Ok(())
    }
    fn alloc_id(&mut self) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = add(id, 1)?;
        Ok(id)
    }
    fn check_patch(&self, p: u32) -> Result<usize, String> {
        let p = p as usize;
        if p >= self.root_zones.len() {
            Err("Ecology patch outside world".into())
        } else {
            Ok(p)
        }
    }
    fn check_team(team: u8) -> Result<usize, String> {
        if team > 1 {
            Err("Ecology team outside0..=1".into())
        } else {
            Ok(team as usize)
        }
    }
    pub fn add_rain(&mut self, patch: u32, liters: u64) -> Result<(), String> {
        let p = self.check_patch(patch)?;
        let next = add(self.root_zones[p].shallow_water, liters)?;
        let rain = add(self.ledger.rain_water, liters)?;
        self.root_zones[p].shallow_water = next;
        self.ledger.rain_water = rain;
        Ok(())
    }
    /// Caller must withdraw these liters from their prior owner before transfer.
    pub fn add_water(&mut self, patch: u32, liters: u64) -> Result<(), String> {
        let p = self.check_patch(patch)?;
        let next = add(self.root_zones[p].shallow_water, liters)?;
        let input = add(self.ledger.transferred_water_in, liters)?;
        self.root_zones[p].shallow_water = next;
        self.ledger.transferred_water_in = input;
        Ok(())
    }
    /// Campaign visibility/habitat owner supplies actual exposure, not a surface-patch shortcut.
    pub fn set_surface_observable(
        &mut self,
        cohort_id: u64,
        observable: bool,
        height_mm: i32,
    ) -> Result<(), String> {
        let c = self
            .cohorts
            .iter_mut()
            .find(|c| c.id == cohort_id)
            .ok_or("Unknown habitat occupant")?;
        c.surface_observable = observable;
        c.location_height_mm = height_mm;
        Ok(())
    }
    pub fn capture_work_ticks(&self, cohort_id: u64) -> Result<u64, String> {
        let c = self
            .cohorts
            .iter()
            .find(|c| c.id == cohort_id && c.count > 0)
            .ok_or("No accessible animal")?;
        let s = &self.species[c.species_id as usize];
        Ok(behavior::handling_ticks(
            s.docile,
            s.body_mass_g,
            s.threat_response,
        ))
    }
    pub fn alert_threat(&mut self, patch: u32, duration_ticks: u64) -> Result<(), String> {
        let p = self.check_patch(patch)?;
        if duration_ticks == 0 || duration_ticks > SEASON_TICKS {
            return Err("Invalid biological threat duration".into());
        }
        let until = add(self.last_tick, duration_ticks)?;
        if self.threat_until.len() != self.root_zones.len() {
            self.threat_until.resize(self.root_zones.len(), 0)
        }
        self.threat_until[p] = self.threat_until[p].max(until);
        Ok(())
    }
    fn nearby_threats(&self, patch: usize) -> u32 {
        let x = patch % self.patch_width;
        let y = patch / self.patch_width;
        let rows = self.root_zones.len() / self.patch_width;
        let mut n = 0;
        for yy in y.saturating_sub(1)..=(y + 1).min(rows - 1) {
            for xx in x.saturating_sub(1)..=(x + 1).min(self.patch_width - 1) {
                if self
                    .threat_until
                    .get(yy * self.patch_width + xx)
                    .copied()
                    .unwrap_or(0)
                    > self.last_tick
                {
                    n += 1
                }
            }
        }
        n
    }
    pub fn set_breeding_refuge(
        &mut self,
        cohort_id: u64,
        claim: Option<BreedingRefuge>,
    ) -> Result<(), String> {
        if let Some(c) = self
            .cohorts
            .iter_mut()
            .find(|c| c.id == cohort_id && c.count > 0)
        {
            if claim.as_ref().is_some_and(|r| {
                r.den_id == Some(0)
                    || r.nest_id == Some(0)
                    || r.den_id.is_none() && r.den_slots > 0
                    || r.nest_id.is_none() && r.nest_slots > 0
            }) {
                return Err("Invalid physical breeding refuge reference".into());
            }
            c.breeding_refuge = claim;
            Ok(())
        } else if claim.is_none() {
            Ok(())
        } else {
            Err("Refuge claimant is no longer a living wild actor".into())
        }
    }
    pub fn set_engineering_busy(&mut self, cohort_id: u64, until_tick: u64) -> Result<(), String> {
        if until_tick < self.last_tick {
            return Err("Engineering deadline predates ecology tick".into());
        }
        let c = self
            .cohorts
            .iter_mut()
            .find(|c| c.id == cohort_id && c.count > 0)
            .ok_or("Missing engineering actor")?;
        c.engineering_busy_until = until_tick;
        Ok(())
    }
    pub fn plant_fraction_permille(&self, patch: u32) -> u16 {
        self.cached_fraction
            .get(patch as usize)
            .copied()
            .unwrap_or(0)
    }
    pub fn root_reinforcement_permille(&self, patch: u32) -> u16 {
        if let Some(value) = self.cached_root_reinforcement.get(patch as usize) {
            return *value;
        }
        let (root, initial) =
            self.plants
                .iter()
                .filter(|p| p.patch == patch)
                .fold((0u64, 0u64), |(a, b), p| {
                    (
                        a + p.roots,
                        b + if p.initial_root_units > 0 {
                            p.initial_root_units
                        } else {
                            (p.initial_live_units as u128 * 2000 / 8500) as u64
                        },
                    )
                });
        if initial == 0 {
            0
        } else {
            (root as u128 * 1000 / initial as u128).min(1000) as u16
        }
    }
    pub fn canopy_permille(&self, patch: u32) -> u16 {
        self.cached_canopy.get(patch as usize).copied().unwrap_or(0)
    }
    pub fn ground_cover_permille(&self, patch: u32) -> u16 {
        self.cached_ground_cover
            .get(patch as usize)
            .copied()
            .unwrap_or(0)
    }
    fn refresh_fields(&mut self) {
        let count = self.root_zones.len();
        let mut canopy = vec![0u64; count];
        let mut cover = vec![0u64; count];
        let mut live = vec![0u64; count];
        let mut initial = vec![0u64; count];
        let mut rich = vec![false; count];
        let mut roots = vec![0u64; count];
        let mut original_roots = vec![0u64; count];
        for p in &self.plants {
            let i = p.patch as usize;
            if i >= count {
                continue;
            }
            let s = &self.species[p.species_id as usize];
            roots[i] += p.roots;
            original_roots[i] += if p.initial_root_units > 0 {
                p.initial_root_units
            } else {
                (p.initial_live_units as u128 * 2000 / 8500) as u64
            };
            live[i] += p.live_units();
            initial[i] += p.initial_live_units;
            if s.traits.has(157) {
                canopy[i] += p.shoots + p.leaves
            }
            if s.traits.has(156) {
                cover[i] += p.leaves + p.shoots / 2
            }
            if s.traits.has(159) && p.individuals > 0 {
                rich[i] = true
            }
        }
        self.cached_canopy = canopy
            .into_iter()
            .map(|u| (u / 10).min(900) as u16)
            .collect();
        self.cached_ground_cover = cover.into_iter().map(|u| (u / 5).min(950) as u16).collect();
        self.cached_fraction = live
            .iter()
            .zip(initial)
            .map(|(&l, i)| {
                if i == 0 {
                    0
                } else {
                    (l as u128 * 1000 / i as u128).min(1000) as u16
                }
            })
            .collect();
        self.nursing_parent_ids = self
            .cohorts
            .iter()
            .chain(self.captives.iter().map(|c| &c.animal))
            .chain(self.transits.iter().map(|t| &t.organism))
            .filter(|c| c.stage == Stage::Juvenile && c.count > 0)
            .filter_map(|c| c.parent_id)
            .collect();
        self.nursing_parent_ids.sort_unstable();
        self.nursing_parent_ids.dedup();
        self.cached_root_reinforcement = roots
            .iter()
            .zip(original_roots)
            .map(|(&r, i)| {
                if i == 0 {
                    0
                } else {
                    (r as u128 * 1000 / i as u128).min(1000) as u16
                }
            })
            .collect();
        self.cached_rich_litter = rich;
        self.plant_patch_offsets.clear();
        self.plant_patch_offsets.resize(count + 1, 0);
        for p in &self.plants {
            if (p.patch as usize) < count {
                self.plant_patch_offsets[p.patch as usize + 1] += 1
            }
        }
        for i in 1..=count {
            self.plant_patch_offsets[i] += self.plant_patch_offsets[i - 1]
        }
        self.plant_patch_indices.clear();
        self.plant_patch_indices.resize(self.plants.len(), 0);
        let mut cursor = self.plant_patch_offsets[..count].to_vec();
        for (index, p) in self.plants.iter().enumerate() {
            let patch = p.patch as usize;
            if patch < count {
                self.plant_patch_indices[cursor[patch]] = index;
                cursor[patch] += 1;
            }
        }
    }
    pub fn observe(&mut self, team: u8, patch: u32, visible: bool) -> Result<(), String> {
        let t = Self::check_team(team)?;
        let p = self.check_patch(patch)?;
        self.visible_patches[t][p] = visible;
        if visible {
            for plant in self
                .plants
                .iter()
                .filter(|x| x.patch == patch && x.individuals > 0)
            {
                let k = &mut self.knowledge[t][plant.species_id as usize];
                k.observed = true;
                k.observed_tick = self.last_tick;
            }
            for c in self.cohorts.iter().filter(|x| {
                x.patch == patch && x.count > 0 && x.surface_observable && x.location_height_mm >= 0
            }) {
                let k = &mut self.knowledge[t][c.species_id as usize];
                k.observed = true;
                k.observed_tick = self.last_tick;
            }
        }
        Ok(())
    }
    pub fn observe_cohort(
        &mut self,
        team: u8,
        cohort_id: u64,
        visible: bool,
    ) -> Result<(), String> {
        let t = Self::check_team(team)?;
        let c = self
            .cohorts
            .iter()
            .find(|c| c.id == cohort_id && c.count > 0)
            .ok_or("No such living observer target")?;
        if visible && c.surface_observable && c.location_height_mm >= 0 {
            self.visible_patches[t][c.patch as usize] = true;
            let k = &mut self.knowledge[t][c.species_id as usize];
            k.observed = true;
            k.observed_tick = self.last_tick;
        }
        Ok(())
    }
    pub fn observed_inspect(&self, team: u8, species: u32) -> Option<serde_json::Value> {
        let t = Self::check_team(team).ok()?;
        let s = self.species.get(species as usize)?;
        let k = self.knowledge[t].get(species as usize)?;
        if !k.observed {
            return None;
        }
        let stage = (k.study_ticks / 800).min(3);
        let traits = if stage >= 3 {
            s.traits.ids()
        } else {
            Vec::new()
        };
        Some(
            serde_json::json!({"species_id":species,"name":s.name,"taxon":s.taxon,"body_mass_g":s.body_mass_g,"research_stage":stage,"study_ticks":k.study_ticks,"traits":traits,"traits_known":stage>=3,"captive_count":self.captives.iter().filter(|c|c.team==team&&c.animal.species_id==species).map(|c|c.animal.count as u64).sum::<u64>()}),
        )
    }
    pub fn capture(&mut self, patch: u32, species: u32, team: u8) -> Result<u64, String> {
        let t = Self::check_team(team)?;
        let p = self.check_patch(patch)?;
        if !self.visible_patches[t][p] {
            return Err("Capture requires current visual observation".into());
        }
        if !self.knowledge[t]
            .get(species as usize)
            .is_some_and(|k| k.observed)
        {
            return Err("Unknown species".into());
        }
        let has_female = self
            .captives
            .iter()
            .any(|c| c.team == team && c.animal.species_id == species && c.animal.sex != Sex::Male);
        let has_male = self.captives.iter().any(|c| {
            c.team == team && c.animal.species_id == species && c.animal.sex != Sex::Female
        });
        let ix = self
            .cohorts
            .iter()
            .position(|c| {
                c.patch == patch
                    && c.species_id == species
                    && c.count > 0
                    && c.surface_observable
                    && c.location_height_mm >= 0
                    && ((!has_female && !has_male)
                        || (has_female && has_male)
                        || (has_female && c.sex != Sex::Female)
                        || (has_male && c.sex != Sex::Male))
            })
            .ok_or("No reachable animal to capture")?;
        self.capture_index(ix, patch, team)
    }
    /// The campaign verifies the handler has reached this exact observed actor.
    pub fn capture_cohort(&mut self, cohort_id: u64, team: u8) -> Result<u64, String> {
        let t = Self::check_team(team)?;
        let ix = self
            .cohorts
            .iter()
            .position(|c| {
                c.id == cohort_id
                    && c.count > 0
                    && c.surface_observable
                    && c.location_height_mm >= 0
            })
            .ok_or("No accessible target organism")?;
        let patch = self.cohorts[ix].patch;
        let species = self.cohorts[ix].species_id;
        if !self.visible_patches[t][patch as usize] || !self.knowledge[t][species as usize].observed
        {
            return Err("Capture requires current visual observation of the target".into());
        }
        self.capture_index(ix, patch, team)
    }
    fn capture_index(&mut self, ix: usize, patch: u32, team: u8) -> Result<u64, String> {
        // An already materialized individual has a persistent physical identity
        // (including its habitat wallet). Capturing it transfers that owner;
        // only an aggregate cohort needs a newly identified split.
        let mut animal = if self.cohorts[ix].count == 1 {
            self.cohorts.remove(ix)
        } else {
            self.split_one(ix)?
        };
        animal.breeding_refuge = None;
        let id = animal.id;
        for clutch in self
            .clutches
            .iter_mut()
            .filter(|cl| cl.mother_id == id && cl.reproduction == Reproduction::LiveBirth)
        {
            clutch.custody = Custody::Captive(team);
            clutch.housing = None;
            clutch.patch = patch;
        }
        self.captives.push(Captive {
            animal,
            team,
            care_food: 0,
            care_water: 0,
            breed_enabled: false,
            housing: None,
        });
        self.validate()?;
        Ok(id)
    }
    /// Materialize one real animal for a tracked physical interaction. It leaves
    /// its cohort once; this never invents an animal or render-only placement.
    pub fn materialize_one(&mut self, cohort_id: u64) -> Result<u64, String> {
        let ix = self
            .cohorts
            .iter()
            .position(|c| c.id == cohort_id && c.count > 0)
            .ok_or("No such living cohort")?;
        if self.cohorts[ix].count == 1 {
            return Ok(cohort_id);
        }
        let animal = self.split_one(ix)?;
        let id = animal.id;
        self.cohorts.push(animal);
        self.validate()?;
        Ok(id)
    }
    fn split_one(&mut self, ix: usize) -> Result<Cohort, String> {
        let owner = self.cohorts[ix].id;
        let count = self.cohorts[ix].count;
        let gestations: Vec<usize> = self
            .clutches
            .iter()
            .enumerate()
            .filter(|(_, cl)| cl.mother_id == owner && cl.reproduction == Reproduction::LiveBirth)
            .map(|(i, _)| i)
            .collect();
        let carried = if gestations.len() >= count as usize {
            gestations.first().copied()
        } else {
            None
        };
        let id = self.alloc_id()?;
        let c = &mut self.cohorts[ix];
        if c.count == 0 {
            return Err("Cannot split an empty cohort".into());
        }
        let count = c.count as u64;
        let mut animal = c.clone();
        animal.id = id;
        animal.count = 1;
        animal.body_units = c.body_units / count;
        animal.energy = c.energy / count;
        animal.hydration = c.hydration / count;
        animal.milk = c.milk / count;
        animal.carried_seeds.clear();
        animal.pollen = None;
        if c.count > 1 {
            if let Some(pollen) = &mut c.pollen {
                let amount = pollen.units / count;
                if amount > 0 {
                    animal.pollen = Some(PollenTicket {
                        units: amount,
                        ..pollen.clone()
                    });
                    pollen.units -= amount;
                }
            }
        }
        animal.breeding_refuge = None;
        animal.sperm = None; // reproductive tickets remain with original owner; transfer below if last organism.
        if c.count == 1 {
            c.breeding_refuge = None;
            animal.pollen = c.pollen.take();
            animal.carried_seeds = std::mem::take(&mut c.carried_seeds);
            animal.sperm = c.sperm.take();
        }
        if c.count > 1 {
            if let Some(ticket) = &mut c.sperm {
                if ticket.doses > 0 {
                    animal.sperm = Some(SpermTicket {
                        donor_species: ticket.donor_species,
                        doses: 1,
                        expires_tick: ticket.expires_tick,
                    });
                    ticket.doses -= 1;
                }
            }
        }
        c.count -= 1;
        c.body_units -= animal.body_units;
        c.energy -= animal.energy;
        c.hydration -= animal.hydration;
        c.milk -= animal.milk;
        if let Some(index) = carried {
            self.clutches[index].mother_id = id;
        }
        Ok(animal)
    }
    pub fn is_housed(&self, captive: &Captive) -> bool {
        captive.housing.is_some_and(|id| {
            self.housing[captive.team as usize]
                .iter()
                .any(|h| h.id == id && h.patch == captive.animal.patch && h.capacity > 0)
        })
    }
    fn housing_occupancy(&self, team: u8, id: u64) -> Result<u64, String> {
        self.captives
            .iter()
            .filter(|c| c.team == team && c.housing == Some(id))
            .map(|c| c.animal.count as u64)
            .chain(
                self.clutches
                    .iter()
                    .filter(|c| c.custody == Custody::Captive(team) && c.housing == Some(id))
                    .map(|c| c.count as u64),
            )
            .try_fold(0, add)
    }
    pub fn housing_available(&self, team: u8, id: u64) -> Result<u32, String> {
        let t = Self::check_team(team)?;
        let house = self.housing[t]
            .iter()
            .find(|h| h.id == id)
            .ok_or("No completed holding pen")?;
        let used = self.housing_occupancy(team, id)?;
        let free = (house.capacity as u64)
            .checked_sub(used)
            .ok_or("Holding pen exceeds capacity")?;
        Ok(free as u32)
    }
    /// Construction owns actual pens. This synchronizes only their capacity
    /// references; destroying a pen removes care, not surviving animal stocks.
    pub fn set_housing(&mut self, team: u8, housing: Vec<Housing>) -> Result<(), String> {
        let t = Self::check_team(team)?;
        let mut ids = std::collections::HashSet::new();
        for h in &housing {
            self.check_patch(h.patch)?;
            if h.id == 0 || h.capacity == 0 || !ids.insert(h.id) {
                return Err("Invalid holding-pen capacity reference".into());
            }
        }
        self.housing[t] = housing;
        for c in self.captives.iter_mut().filter(|c| c.team == team) {
            if c.housing
                .is_some_and(|id| !self.housing[t].iter().any(|h| h.id == id))
            {
                c.housing = None;
            }
        }
        for c in self
            .clutches
            .iter_mut()
            .filter(|c| c.custody == Custody::Captive(team))
        {
            if c.housing
                .is_some_and(|id| !self.housing[t].iter().any(|h| h.id == id))
            {
                c.housing = None;
            }
        }
        let houses = self.housing[t].clone();
        for h in houses {
            let mut used = self.housing_occupancy(team, h.id)?;
            if used <= h.capacity as u64 {
                continue;
            }
            // Loss of capacity leaves whole existing organisms unhoused at the
            // same place. It never deletes a surplus or silently reduces counts.
            for c in self
                .captives
                .iter_mut()
                .rev()
                .filter(|c| c.team == team && c.housing == Some(h.id))
            {
                c.housing = None;
                used -= c.animal.count as u64;
                for clutch in self.clutches.iter_mut().filter(|cl| {
                    cl.mother_id == c.animal.id
                        && cl.reproduction == Reproduction::LiveBirth
                        && cl.housing == Some(h.id)
                }) {
                    clutch.housing = None;
                    used -= clutch.count as u64;
                }
                if used <= h.capacity as u64 {
                    break;
                }
            }
            if used > h.capacity as u64 {
                for clutch in
                    self.clutches.iter_mut().rev().filter(|cl| {
                        cl.custody == Custody::Captive(team) && cl.housing == Some(h.id)
                    })
                {
                    clutch.housing = None;
                    used -= clutch.count as u64;
                    if used <= h.capacity as u64 {
                        break;
                    }
                }
            }
        }
        Ok(())
    }
    /// Call only when the campaign's real transporter arrives at this pen.
    pub fn house_captive(&mut self, captive_id: u64, pen_id: u64) -> Result<(), String> {
        let ix = self
            .captives
            .iter()
            .position(|c| c.animal.id == captive_id)
            .ok_or("No captured animal")?;
        let team = self.captives[ix].team;
        let house = self.housing[team as usize]
            .iter()
            .find(|h| h.id == pen_id)
            .ok_or("No completed holding pen")?
            .clone();
        if self.captives[ix].housing == Some(pen_id) {
            return Ok(());
        }
        let gestations = self
            .clutches
            .iter()
            .filter(|cl| {
                cl.mother_id == captive_id
                    && cl.reproduction == Reproduction::LiveBirth
                    && cl.custody == Custody::Captive(team)
            })
            .map(|c| c.count as u64)
            .try_fold(0, add)?;
        let required = add(self.captives[ix].animal.count as u64, gestations)?;
        if (self.housing_available(team, pen_id)? as u64) < required {
            return Err("Holding pen has no space for this animal and its viable offspring".into());
        }
        self.captives[ix].housing = Some(pen_id);
        self.captives[ix].animal.patch = house.patch;
        for clutch in self.clutches.iter_mut().filter(|cl| {
            cl.mother_id == captive_id
                && cl.reproduction == Reproduction::LiveBirth
                && cl.custody == Custody::Captive(team)
        }) {
            clutch.housing = Some(pen_id);
            clutch.patch = house.patch;
        }
        self.validate()?;
        Ok(())
    }
    /// Care food/water are real transfers out of a player's supply ledger.
    pub fn supply_care(&mut self, team: u8, food: u64, water: u64) -> Result<(), String> {
        Self::check_team(team)?;
        let eligible: Vec<usize> = self
            .captives
            .iter()
            .enumerate()
            .filter(|(_, c)| c.team == team && self.is_housed(c))
            .map(|(i, _)| i)
            .collect();
        if eligible.is_empty() {
            return Err("Care deliveries require a real housed specimen".into());
        }
        let imported = add(self.ledger.material_imported, food)?;
        let water_in = add(self.ledger.transferred_water_in, water)?;
        let mut f = food;
        let mut w = water;
        let mut left = eligible.len() as u64;
        for i in eligible {
            let a = f / left;
            let b = w / left;
            self.captives[i].care_food = add(self.captives[i].care_food, a)?;
            self.captives[i].care_water = add(self.captives[i].care_water, b)?;
            f -= a;
            w -= b;
            left -= 1;
        }
        self.ledger.material_imported = imported;
        self.ledger.transferred_water_in = water_in;
        Ok(())
    }
    pub fn research(&mut self, team: u8, species: u32) -> Result<(), String> {
        let t = Self::check_team(team)?;
        if !self
            .captives
            .iter()
            .any(|c| c.team == team && c.animal.species_id == species && self.is_housed(c))
        {
            return Err("Research needs a real captive specimen and continuing care".into());
        }
        let k = self.knowledge[t]
            .get_mut(species as usize)
            .ok_or("Unknown species")?;
        k.enabled = true;
        Ok(())
    }
    pub fn breed(&mut self, team: u8, species: u32) -> Result<(), String> {
        let t = Self::check_team(team)?;
        if self.knowledge[t]
            .get(species as usize)
            .is_none_or(|k| k.study_ticks < 800)
        {
            return Err("Breeding requires initial husbandry research".into());
        }
        let mut ready = std::collections::HashSet::new();
        for female in self.captives.iter().filter(|c| {
            c.team == team
                && c.animal.species_id == species
                && c.animal.sex != Sex::Male
                && self.is_housed(c)
        }) {
            let pen = female.housing.unwrap();
            let male = self.captives.iter().any(|c| {
                c.team == team
                    && c.animal.species_id == species
                    && c.animal.sex != Sex::Female
                    && c.housing == Some(pen)
                    && c.animal.id != female.animal.id
            });
            let stored = female.animal.sperm.as_ref().is_some_and(|s| {
                s.doses > 0 && s.expires_tick > self.last_tick && s.donor_species == species
            });
            if (male || stored)
                && self.housing_available(team, pen)?
                    >= self.species[species as usize].clutch_size.min(2) as u32
            {
                ready.insert(pen);
            }
        }
        if ready.is_empty() {
            return Err("Breeding needs compatible housed reproductive roles and space for the viable offspring".into());
        }
        for c in self.captives.iter_mut().filter(|c| {
            c.team == team
                && c.animal.species_id == species
                && c.housing.is_some_and(|p| ready.contains(&p))
        }) {
            c.breed_enabled = true;
        }
        Ok(())
    }
    pub fn release(&mut self, team: u8, species: u32, patch: u32) -> Result<u64, String> {
        let t = Self::check_team(team)?;
        let p = self.check_patch(patch)?;
        if !self.visible_patches[t][p] {
            return Err("Release needs an observed destination".into());
        }
        let ix = self
            .captives
            .iter()
            .position(|c| c.team == team && c.animal.species_id == species)
            .ok_or("No captive specimen")?;
        let id = self.captives[ix].animal.id;
        self.escape_captive(id, patch)
    }
    /// Actual world transport updates this location; no care is usable en route.
    pub fn set_captive_transit(&mut self, id: u64, patch: u32) -> Result<(), String> {
        self.check_patch(patch)?;
        let c = self
            .captives
            .iter_mut()
            .find(|c| c.animal.id == id)
            .ok_or("No captive cargo")?;
        c.housing = None;
        c.animal.patch = patch;
        for clutch in self
            .clutches
            .iter_mut()
            .filter(|cl| cl.mother_id == id && cl.reproduction == Reproduction::LiveBirth)
        {
            clutch.housing = None;
            clutch.patch = patch;
        }
        Ok(())
    }
    /// Physics/handler loss releases an existing animal even when unobserved.
    /// This is not a player command and must use the real carrier's location.
    pub fn escape_captive(&mut self, id: u64, patch: u32) -> Result<u64, String> {
        let p = self.check_patch(patch)?;
        let ix = self
            .captives
            .iter()
            .position(|c| c.animal.id == id)
            .ok_or("No captive cargo")?;
        let captive = self.captives.remove(ix);
        let mut animal = captive.animal;
        animal.patch = patch;
        let id = animal.id;
        for clutch in &mut self.clutches {
            if clutch.mother_id == id && clutch.reproduction == Reproduction::LiveBirth {
                clutch.custody = Custody::Wild(patch);
                clutch.housing = None;
                clutch.patch = patch;
            }
        }
        self.root_zones[p].nutrients = add(self.root_zones[p].nutrients, captive.care_food)?;
        self.root_zones[p].shallow_water =
            add(self.root_zones[p].shallow_water, captive.care_water)?;
        self.cohorts.push(animal);
        self.validate()?;
        Ok(id)
    }
    pub fn harvest_milk(&mut self, team: u8, species: u32, maximum: u64) -> Result<u64, String> {
        Self::check_team(team)?;
        let housed: std::collections::HashSet<u64> = self
            .captives
            .iter()
            .filter(|c| c.team == team && self.is_housed(c))
            .map(|c| c.animal.id)
            .collect();
        let mut got = 0;
        for c in self.captives.iter_mut().filter(|c| {
            c.team == team && c.animal.species_id == species && housed.contains(&c.animal.id)
        }) {
            let n = c.animal.milk.min(maximum - got);
            c.animal.milk -= n;
            got += n;
            if got == maximum {
                break;
            }
        }
        self.ledger.material_exported = add(self.ledger.material_exported, got)?;
        Ok(got)
    }
    /// Remove actual tissue; it remains accounted litter, never free timber too.
    /// `uproot` determines whether viable roots survive; this controls resprouting.
    pub fn damage_plants(
        &mut self,
        patch: u32,
        amount: u64,
        uproot: bool,
    ) -> Result<DamageReport, String> {
        let p = self.check_patch(patch)?;
        let mut report = DamageReport::default();
        let mut remaining = amount;
        for plant in self
            .plants
            .iter_mut()
            .filter(|p| p.patch == patch && p.individuals > 0)
        {
            let before = plant.live_units();
            for stock in [
                &mut plant.leaves,
                &mut plant.shoots,
                &mut plant.fruit,
                &mut plant.nectar,
                &mut plant.pollen,
                &mut plant.serviced_pollen,
            ] {
                let n = (*stock).min(remaining);
                *stock -= n;
                remaining -= n;
            }
            if uproot {
                let n = plant.roots.min(remaining);
                plant.roots -= n;
                remaining -= n;
            }
            let lost = before - plant.live_units();
            report.plant_units_removed = add(report.plant_units_removed, lost)?;
            if plant.shoots + plant.roots + plant.leaves == 0 {
                report.plants_killed = add(report.plants_killed, plant.individuals as u64)?;
                plant.individuals = 0;
            }
            if remaining == 0 {
                break;
            }
        }
        report.material_to_litter = report.plant_units_removed;
        self.root_zones[p].litter = add(self.root_zones[p].litter, report.material_to_litter)?;
        self.ledger.dead_plants = add(self.ledger.dead_plants, report.plants_killed)?;
        self.refresh_fields();
        Ok(report)
    }
    fn record_deaths(&mut self, count: u64, reason: &str) -> Result<(), String> {
        let total = add(self.ledger.dead_animals, count)?;
        let n = add(
            self.ledger.death_reasons.get(reason).copied().unwrap_or(0),
            count,
        )?;
        self.ledger.dead_animals = total;
        self.ledger.death_reasons.insert(reason.into(), n);
        Ok(())
    }
    pub fn damage_animals(&mut self, patch: u32, count: u32) -> Result<DamageReport, String> {
        let p = self.check_patch(patch)?;
        let mut report = DamageReport::default();
        let mut left = count;
        for c in self
            .cohorts
            .iter_mut()
            .filter(|c| c.patch == patch && c.count > 0)
        {
            let n = c.count.min(left);
            let (mass, water) = take_cohort(c, n)?;
            self.root_zones[p].litter = add(self.root_zones[p].litter, mass)?;
            self.root_zones[p].shallow_water = add(self.root_zones[p].shallow_water, water)?;
            report.animals_killed += n as u64;
            report.material_to_litter = add(report.material_to_litter, mass)?;
            left -= n;
            if left == 0 {
                break;
            }
        }
        self.record_deaths(report.animals_killed, "physical_damage")?;
        Ok(report)
    }
    pub fn damage_cohort(&mut self, cohort_id: u64, count: u32) -> Result<DamageReport, String> {
        let ix = self
            .cohorts
            .iter()
            .position(|c| c.id == cohort_id)
            .ok_or("No such cohort")?;
        if self.cohorts[ix].count < count {
            return Err("Damage exceeds living occupants".into());
        }
        let p = self.cohorts[ix].patch as usize;
        let (mass, water) = take_cohort(&mut self.cohorts[ix], count)?;
        self.root_zones[p].litter = add(self.root_zones[p].litter, mass)?;
        self.root_zones[p].shallow_water = add(self.root_zones[p].shallow_water, water)?;
        self.record_deaths(count as u64, "physical_damage")?;
        Ok(DamageReport {
            animals_killed: count as u64,
            material_to_litter: mass,
            ..Default::default()
        })
    }
    pub fn destroy_clutch(&mut self, id: u64) -> Result<DamageReport, String> {
        let ix = self
            .clutches
            .iter()
            .position(|c| c.id == id)
            .ok_or("No such viable clutch")?;
        let c = self.clutches.remove(ix);
        let p = self.check_patch(c.patch)?;
        self.root_zones[p].litter = add(self.root_zones[p].litter, c.material)?;
        self.root_zones[p].shallow_water = add(self.root_zones[p].shallow_water, c.water)?;
        self.record_deaths(c.count as u64, "offspring/physical_damage")?;
        Ok(DamageReport {
            animals_killed: c.count as u64,
            material_to_litter: c.material,
            ..Default::default()
        })
    }
    /// Real travel/foraging labor consumes reserves without claiming that a
    /// construction trait completed. Caller first checks the finite budget.
    pub fn spend_activity_energy(&mut self, cohort_id: u64, amount: u64) -> Result<(), String> {
        let c = self
            .cohorts
            .iter_mut()
            .find(|c| c.id == cohort_id && c.count > 0)
            .ok_or("Missing active wildlife traveler")?;
        let remaining = sub(c.energy, amount)?;
        let respired = add(self.ledger.respired_material, amount)?;
        c.energy = remaining;
        self.ledger.respired_material = respired;
        Ok(())
    }
    /// Physical caller commits a successful material transaction before charging
    /// this energy. Requests are bounded to one cohort/trait window, not effects.
    pub fn commit_physical_effect(
        &mut self,
        cohort_id: u64,
        trait_id: u16,
        energy_cost: u64,
        success: bool,
    ) -> Result<(), String> {
        if !success {
            return Ok(());
        }
        let ti = SELECTED_TRAITS
            .iter()
            .position(|&id| id == trait_id)
            .ok_or("Unknown selected physical trait")?;
        let c = self
            .cohorts
            .iter_mut()
            .find(|c| c.id == cohort_id && c.count > 0)
            .ok_or("Physical actor no longer present")?;
        if !self.species[c.species_id as usize].traits.has(trait_id) {
            return Err("Actor lacks physical capability".into());
        }
        c.energy = sub(c.energy, energy_cost)?;
        self.ledger.respired_material = add(self.ledger.respired_material, energy_cost)?;
        self.physical_effects_committed[ti] = add(self.physical_effects_committed[ti], 1)?;
        Ok(())
    }
    pub fn step(
        &mut self,
        tick: u64,
        environment: &[EcologyPatch],
    ) -> Result<Vec<EcologyEffect>, String> {
        if tick != add(self.last_tick, ECOLOGY_INTERVAL)? {
            return Err("Ecology must process each due20tick interval exactly once".into());
        }
        if environment.len() != self.root_zones.len() {
            return Err("Ecology environment size mismatch".into());
        }
        for (i, e) in environment.iter().enumerate() {
            if e.moisture_permille > 1000
                || e.flow_fraction_permille > 1000
                || e.land_connections > 15
                || e.water_connections > 15
                || e.flow_to
                    .is_some_and(|p| p as usize >= environment.len() || p as usize == i)
            {
                return Err("Invalid ecological conditions/flow target".into());
            }
        }
        self.last_tick = tick;
        self.refresh_fields();
        let mut effects = Vec::new();
        self.advance_seeds(environment)?;
        self.advance_plants(environment)?;
        self.refresh_fields();
        self.advance_transits(environment)?;
        let profiles = self.species.clone();
        // A derived sparse view includes pending animals, unlike scanning the
        // progressively drained owner vector. It never owns organisms.
        self.regional_counts.clear();
        for c in &self.cohorts {
            let count = self
                .regional_counts
                .entry((c.patch, c.species_id))
                .or_default();
            *count = count
                .checked_add(c.count)
                .ok_or("Regional count overflow")?;
        }
        let mut pending = std::mem::take(&mut self.cohorts);
        pending.sort_by_key(|c| {
            (
                self.species[c.species_id as usize].taxon == Taxon::Predator,
                c.id,
            )
        });
        for mut c in pending {
            if c.count == 0 {
                continue;
            }
            let patch = c.patch as usize;
            // Temporarily omit this actor so destination projections can add it
            // once, both at its source and at a prospective destination.
            let regional = self
                .regional_counts
                .entry((c.patch, c.species_id))
                .or_default();
            *regional = regional.saturating_sub(c.count);
            let profile_index = c.species_id as usize;
            self.advance_animal_profile(
                &mut c,
                &environment[patch],
                false,
                &profiles[profile_index],
                false,
            )?;
            if c.count == 0 {
                continue;
            }
            let s = &profiles[c.species_id as usize];
            let mobile = c.surface_observable
                && c.engineering_busy_until <= tick
                && c.dormant_until <= tick
                && (c.location_height_mm == 0 || s.locomotion_for(c.stage) == Locomotion::Flying);
            let local_count = self
                .regional_counts
                .get(&(c.patch, c.species_id))
                .copied()
                .unwrap_or(0)
                .saturating_add(c.count);
            let pressure = behavior::movement_pressure(
                local_count,
                s.social_group_size,
                s.territorial_radius_m,
                160 * 160,
                self.nearby_threats(patch),
                s.threat_response,
            );
            let food_missing = !self.resource_goal(&c, s, patch, 0, environment);
            let thirsty =
                c.hydration < c.count as u64 * 40 && self.root_zones[patch].shallow_water == 0;
            let hungry = c.energy < c.count as u64 * 80 && food_missing;
            let breeding_water = s.traits.has(96)
                && s.juvenile_habitat == Habitat::Water
                && c.stage == Stage::Adult
                && c.next_breeding_tick <= tick
                && (tick / 18_000) % 4 == s.preferred_season as u64
                && environment[patch].water_depth_mm < 20;
            let pollen_visit = s.taxon == Taxon::Pollinator
                && c.stage == Stage::Adult
                && c.pollen.as_ref().is_some_and(|p| p.source_patch == c.patch)
                && c.age_ticks % (s.metabolism_period_ticks * 2) < ECOLOGY_INTERVAL;
            let seasonal = s.traits.has(61)
                && tick % (SEASON_TICKS / 4) == 0
                && ((tick / 18_000) % 4 == s.preferred_season as u64);
            let endangered = !Self::habitat_usable(s, c.stage, &environment[patch]);
            let kind = if self.nearby_threats(patch) > 0 && pressure > 100 {
                Some(6)
            } else if endangered {
                Some(4)
            } else if thirsty {
                Some(1)
            } else if hungry {
                Some(0)
            } else if breeding_water {
                Some(3)
            } else if pollen_visit {
                Some(2)
            } else if seasonal {
                Some(5)
            } else if pressure > 200 && c.age_ticks % 200 < ECOLOGY_INTERVAL {
                Some(7)
            } else {
                None
            };
            if mobile {
                if let Some(kind) = kind {
                    if let Some(next) = self.seek_destination(&c, s, environment, kind) {
                        let per_animal = if s.locomotion_for(c.stage) == Locomotion::Flying {
                            2
                        } else {
                            4
                        };
                        let cost = c.count as u64 * per_animal;
                        if c.energy >= cost {
                            c.energy -= cost;
                            self.ledger.respired_material =
                                add(self.ledger.respired_material, cost)?;
                            let id = c.id;
                            let count = c.count;
                            let duration = Self::travel_ticks(s, c.stage);
                            self.transits.push(Transit {
                                from_patch: c.patch,
                                organism: c,
                                destination: next as u32,
                                departure_tick: tick,
                                arrival_tick: tick + duration,
                            });
                            effects.push(EcologyEffect::Migration {
                                cohort_id: id,
                                source: patch as u32,
                                destination: next as u32,
                                count,
                            });
                            continue;
                        }
                    }
                }
            }
            if tick % 200 == 0
                && c.stage == Stage::Adult
                && c.energy > 20
                && c.engineering_busy_until <= tick
            {
                for trait_id in PHYSICAL_TRAITS {
                    if s.traits.has(trait_id) {
                        effects.push(EcologyEffect::Engineering(PhysicalRequest {
                            cohort_id: c.id,
                            species_id: c.species_id,
                            patch: c.patch,
                            trait_id,
                            energy_budget: c.energy.min(80),
                            individuals: c.count,
                        }));
                    }
                }
            }
            let regional = self
                .regional_counts
                .entry((c.patch, c.species_id))
                .or_default();
            *regional = regional
                .checked_add(c.count)
                .ok_or("Regional count overflow")?;
            self.cohorts.push(c);
        }
        self.wild_reproduction(environment)?;
        self.advance_captives(environment)?;
        self.nurse_young()?;
        self.advance_clutches(environment, &mut effects)?;
        self.detect_extinction(&mut effects)?;
        self.refresh_fields();
        self.validate()?;
        Ok(effects)
    }
    fn advance_plants(&mut self, env: &[EcologyPatch]) -> Result<(), String> {
        let profiles = self.species.clone();
        for i in 0..self.plants.len() {
            let pidx = self.plants[i].patch as usize;
            let s = &profiles[self.plants[i].species_id as usize];
            let condition = &env[pidx];
            let canopy = self.canopy_permille(pidx as u32);
            let groundcover = self.ground_cover_permille(pidx as u32);
            let plant = &mut self.plants[i];
            let zone = &mut self.root_zones[pidx];
            if plant.individuals == 0 {
                continue;
            }
            plant.age_ticks = add(plant.age_ticks, ECOLOGY_INTERVAL)?;
            if condition.water_depth_mm > 100 {
                plant.submerged_ticks = add(plant.submerged_ticks, ECOLOGY_INTERVAL)?;
            } else {
                plant.submerged_ticks = 0;
            }
            let flood_limit = if s.traits.has(125) {
                s.flood_tolerance_ticks * 4
            } else {
                s.flood_tolerance_ticks
            };
            if plant.submerged_ticks > flood_limit || plant.age_ticks > s.lifespan_ticks {
                let n = plant.shoots.min(8) + plant.leaves.min(4);
                let shoot = plant.shoots.min(8);
                plant.shoots -= shoot;
                plant.leaves -= n - shoot;
                zone.litter = add(zone.litter, n)?;
                if plant.submerged_ticks > flood_limit * 2
                    || plant.age_ticks > s.lifespan_ticks + 2_000
                {
                    let root = plant.roots.min(8);
                    plant.roots -= root;
                    zone.litter = add(zone.litter, root)?;
                }
            }
            let water_need = if s.traits.has(121) { 3 } else { 2 };
            let water_available = zone.shallow_water
                + if s.traits.has(121) {
                    zone.deep_water
                } else {
                    0
                };
            let comfortable = condition.temperature_milli_c >= s.minimum_temperature_milli_c
                && condition.temperature_milli_c <= s.maximum_temperature_milli_c;
            if comfortable
                && water_available >= water_need
                && zone.nutrients > 0
                && plant.submerged_ticks <= flood_limit
            {
                let competition = if groundcover > 700 && !s.traits.has(128) {
                    2
                } else {
                    4
                };
                let shade_penalty = if canopy > 700 && !s.traits.has(157) {
                    1
                } else {
                    0
                };
                let growth = (competition - shade_penalty.min(competition))
                    .min(zone.nutrients as usize) as u64;
                let growth = if plant.shoots + plant.leaves >= 30_000 {
                    0
                } else {
                    growth
                };
                if growth > 0 {
                    let shallow = zone.shallow_water.min(water_need);
                    zone.shallow_water -= shallow;
                    zone.deep_water -= water_need - shallow;
                    self.ledger.evaporated_water = add(self.ledger.evaporated_water, water_need)?;
                    zone.nutrients -= growth;
                    let root_growth = if growth >= 2 { 1 } else { 0 };
                    plant.roots = add(plant.roots, root_growth)?;
                    plant.shoots = add(plant.shoots, growth - root_growth)?;
                    // Deep-root maintenance uses real tissue and returns litter.
                    if s.traits.has(121) && plant.roots > 1 {
                        plant.roots -= 1;
                        zone.litter = add(zone.litter, 1)?;
                    }
                }
            }
            // Resprouting converts viable reserve roots, not the world seed.
            if s.traits.has(129) && plant.shoots < 200 && plant.roots > 10 && zone.shallow_water > 0
            {
                plant.roots -= 2;
                plant.shoots += 2;
                zone.shallow_water -= 1;
                self.ledger.evaporated_water = add(self.ledger.evaporated_water, 1)?;
            }
            let output_period = if condition.moisture_permille >= 500 {
                100
            } else if condition.moisture_permille >= 200 {
                200
            } else {
                600
            };
            if comfortable
                && plant.age_ticks >= s.maturity_ticks
                && plant.shoots > 100
                && self.last_tick % output_period == 0
            {
                let output = plant.shoots.min(s.plant_output_units as u64);
                let leaves = output * s.leaf_output_permille as u64 / 1000;
                let nectar = output * s.nectar_output_permille as u64 / 1000;
                let fruit = output / 10;
                let pollen = output - leaves - nectar - fruit;
                plant.shoots -= output;
                plant.leaves = add(plant.leaves, leaves)?;
                plant.nectar = add(plant.nectar, nectar)?;
                plant.fruit = add(plant.fruit, fruit)?;
                plant.pollen = add(plant.pollen, pollen)?;
                let can_seed = !s.traits.has(135) || plant.serviced_pollen > 0;
                if can_seed && plant.fruit > 0 {
                    plant.fruit -= 1;
                    plant.seeds = add(plant.seeds, 1)?;
                    if plant.serviced_pollen > 0 {
                        plant.serviced_pollen -= 1;
                        zone.litter = add(zone.litter, 1)?;
                    }
                }
            }
            // Broad canopy has a real water/upkeep cost and a coarse heat field.
            if s.traits.has(157)
                && plant.shoots > 100
                && self.last_tick % 100 == 0
                && zone.shallow_water > 0
            {
                zone.shallow_water -= 1;
                self.ledger.evaporated_water = add(self.ledger.evaporated_water, 1)?;
                plant.shoots -= 1;
                zone.litter = add(zone.litter, 1)?;
            }
            // Litter quality changes decomposition, not an unbounded daily
            // defoliation rate. Ordinary turnover is one finite unit per5s;
            // actual drought accelerates senescence rather than trait159 itself.
            let shed = if self.last_tick
                % if condition.moisture_permille < 150 {
                    40
                } else {
                    100
                }
                == 0
            {
                plant.leaves.min(1)
            } else {
                0
            };
            plant.leaves -= shed;
            zone.litter = add(zone.litter, shed)?;
            if plant.shoots + plant.roots + plant.leaves == 0 {
                self.ledger.dead_plants = add(self.ledger.dead_plants, plant.individuals as u64)?;
                plant.individuals = 0;
                let other = plant.fruit + plant.nectar + plant.pollen + plant.serviced_pollen;
                plant.fruit = 0;
                plant.nectar = 0;
                plant.pollen = 0;
                plant.serviced_pollen = 0;
                zone.litter = add(zone.litter, other)?;
            }
            if plant.seeds > 0 && self.last_tick % 400 == 0 {
                let n = plant.seeds.min(2);
                plant.seeds -= n;
                let persistent = s.traits.has(132);
                self.seeds.push(SeedStock {
                    species_id: plant.species_id,
                    patch: plant.patch,
                    units: n,
                    expires_tick: self.last_tick
                        + if persistent { SEASON_TICKS * 4 } else { 2_400 },
                    advection_remainder: 0,
                    afloat: s.traits.has(143) && condition.water_depth_mm > 10,
                });
            }
        }
        // Finite fungi/microbial decomposer biomass is not silently respawned.
        for (i, zone) in self.root_zones.iter_mut().enumerate() {
            let e = &env[i];
            if zone.fungal_biomass > 0 {
                let rich = self.cached_rich_litter[i];
                let rate = if e.moisture_permille > 150
                    && e.temperature_milli_c > 5_000
                    && e.water_depth_mm < 2_000
                {
                    if rich { 6 } else { 3 }
                } else {
                    1
                };
                let amount = zone.litter.min(rate);
                zone.litter -= amount;
                let grown = if amount >= 3 && zone.fungal_biomass < 500 {
                    1
                } else {
                    0
                };
                zone.fungal_biomass += grown;
                zone.nutrients = add(zone.nutrients, amount - grown)?;
                if self.last_tick % 100 == 0 {
                    zone.fungal_biomass -= 1;
                    self.ledger.respired_material = add(self.ledger.respired_material, 1)?;
                }
            }
        }
        Ok(())
    }
    fn advance_animal(
        &mut self,
        c: &mut Cohort,
        env: &EcologyPatch,
        captive: bool,
    ) -> Result<(), String> {
        let profile = self.species[c.species_id as usize].clone();
        self.advance_animal_profile(c, env, captive, &profile, false)
    }
    fn advance_animal_profile(
        &mut self,
        c: &mut Cohort,
        env: &EcologyPatch,
        captive: bool,
        species: &Species,
        travelling: bool,
    ) -> Result<(), String> {
        let patch = c.patch as usize;
        c.age_ticks = add(c.age_ticks, ECOLOGY_INTERVAL)?;
        let dry = (self.last_tick % SEASON_TICKS) >= 36_000;
        let dormancy_trigger =
            env.temperature_milli_c < 16_000 || dry && env.moisture_permille < 150;
        if species.traits.has(72)
            && dormancy_trigger
            && !travelling
            && c.breeding_refuge.as_ref().is_some_and(|r| {
                r.accessible
                    && ((r.den_id.is_some() && r.den_slots >= c.count)
                        || (r.nest_id.is_some() && r.nest_slots >= c.count))
            })
            && c.dormant_until == 0
        {
            c.dormant_until = ((self.last_tick / 18_000) + 1) * 18_000;
        }
        if c.dormant_until <= self.last_tick || env.water_depth_mm > 500 {
            c.dormant_until = 0;
        }
        let dormant = c.dormant_until > self.last_tick;
        let resting = (self.last_tick % 2_400 < 1_200) == species.nocturnal;
        let energy_period =
            species
                .basal_period(c.stage)
                .saturating_mul(if dormant { 5 } else { 1 });
        let need = if c.age_ticks % energy_period < ECOLOGY_INTERVAL {
            c.count as u64
                * species.metabolism_units_per_step as u64
                * if travelling { 2 } else { 1 }
        } else {
            0
        };
        if !captive
            && !travelling
            && !dormant
            && !resting
            && c.surface_observable
            && c.location_height_mm >= 0
            && c.engineering_busy_until <= self.last_tick
            && c.energy < c.count as u64 * 120
        {
            self.feed_wild(c, &species)?;
        }
        let paid = c.energy.min(need);
        c.energy -= paid;
        self.ledger.respired_material = add(self.ledger.respired_material, paid)?;
        let water_period = species
            .water_period_ticks
            .saturating_mul(if dormant { 5 } else { 1 });
        let drink_need = if c.age_ticks % water_period < ECOLOGY_INTERVAL {
            c.count as u64 * species.water_units_per_step as u64
        } else {
            0
        };
        let consumed = c.hydration.min(drink_need);
        c.hydration -= consumed;
        self.ledger.evaporated_water = add(self.ledger.evaporated_water, consumed)?;
        if !captive && !travelling && c.hydration < c.count as u64 * 60 {
            let requested = c.count as u64 * 4;
            let n = self.root_zones[patch].shallow_water.min(requested);
            self.root_zones[patch].shallow_water -= n;
            c.hydration = add(c.hydration, n)?;
        }
        let drought_threshold = if species.traits.has(75) { 2 } else { 12 };
        let unfit_water = match species.habitat_for(c.stage) {
            Habitat::Land => env.water_depth_mm > 700,
            Habitat::Water => env.water_depth_mm < 20,
            Habitat::Amphibious => false,
        };
        let shortage = paid < need
            || c.energy == 0
            || c.hydration < c.count as u64 * drought_threshold
            || unfit_water;
        let temperature = env.temperature_milli_c - self.canopy_permille(c.patch) as i32 * 5;
        if shortage
            || temperature < species.minimum_temperature_milli_c
            || temperature > species.maximum_temperature_milli_c
        {
            c.health_permille = c
                .health_permille
                .saturating_sub(if unfit_water { 80 } else { 25 });
        } else {
            c.health_permille = (c.health_permille + 5).min(1000);
        }
        c.stamina_permille = if resting || dormant {
            (c.stamina_permille + 20).min(1000)
        } else {
            c.stamina_permille.saturating_sub(5)
        };
        if c.stage == Stage::Juvenile
            && c.age_ticks >= species.maturity_ticks
            && c.energy >= c.count as u64 * (ADULT_BODY - NEW_BODY)
        {
            let cost = c.count as u64 * (ADULT_BODY - NEW_BODY);
            c.energy -= cost;
            c.body_units = add(c.body_units, cost)?;
            c.stage = Stage::Adult;
        }
        if species.traits.has(96) && c.stage == Stage::Adult {
            c.location_height_mm = if species.locomotion_for(c.stage) == Locomotion::Flying {
                2_000
            } else {
                0
            };
        }
        if c.sex != Sex::Male
            && species.milk_production
            && self.nursing_parent_ids.binary_search(&c.id).is_ok()
            && c.energy > c.count as u64 * 60
            && c.age_ticks % species.metabolism_period_ticks < ECOLOGY_INTERVAL
        {
            let n = (c.count as u64).min((c.count as u64 * 16).saturating_sub(c.milk));
            c.energy -= n;
            c.milk = add(c.milk, n)?;
        }
        if let Some(pollen) = &c.pollen {
            if pollen.expires_tick <= self.last_tick {
                let n = pollen.units;
                c.pollen = None;
                self.root_zones[patch].litter = add(self.root_zones[patch].litter, n)?;
            }
        }
        let carried = std::mem::take(&mut c.carried_seeds);
        for ticket in carried {
            if ticket.due_tick <= self.last_tick {
                self.seeds.push(SeedStock {
                    species_id: ticket.species_id,
                    patch: c.patch,
                    units: ticket.units,
                    expires_tick: self.last_tick + SEASON_TICKS,
                    advection_remainder: 0,
                    afloat: env.water_depth_mm > 10
                        && self.species[ticket.species_id as usize].traits.has(143),
                });
            } else {
                c.carried_seeds.push(ticket)
            }
        }
        if c.sperm
            .as_ref()
            .is_some_and(|s| s.expires_tick <= self.last_tick)
        {
            c.sperm = None;
        }
        if c.health_permille == 0 || c.age_ticks > species.lifespan_ticks {
            let n = c.count;
            let no_energy = c.energy == 0;
            let low_water = c.hydration < n as u64 * drought_threshold;
            let (mass, water) = take_cohort(c, n)?;
            self.root_zones[patch].litter = add(self.root_zones[patch].litter, mass)?;
            self.root_zones[patch].shallow_water =
                add(self.root_zones[patch].shallow_water, water)?;
            let reason = if c.age_ticks > species.lifespan_ticks {
                "old_age"
            } else if unfit_water {
                "unsuitable_habitat"
            } else if no_energy && low_water {
                "hunger_and_thirst"
            } else if no_energy {
                "starvation"
            } else if low_water {
                "dehydration"
            } else {
                "temperature"
            };
            self.record_deaths(
                n as u64,
                &format!(
                    "{}/{reason}",
                    if travelling {
                        "transit"
                    } else if captive {
                        "captive"
                    } else {
                        "wild"
                    }
                ),
            )?;
        }
        Ok(())
    }
    fn feed_wild(&mut self, c: &mut Cohort, species: &Species) -> Result<(), String> {
        let need = (c.count as u64 * 120)
            .saturating_sub(c.energy)
            .min(c.count as u64 * 6);
        if c.stage == Stage::Juvenile
            && species.traits.has(96)
            && species.juvenile_habitat == Habitat::Water
        {
            let food = self.root_zones[c.patch as usize].litter.min(need);
            self.root_zones[c.patch as usize].litter -= food;
            c.energy = add(c.energy, food)?;
            return Ok(());
        }
        if species.taxon == Taxon::Predator {
            if let Some(i) = self.cohorts.iter().position(|prey| {
                prey.patch == c.patch
                    && prey.count > 0
                    && prey.surface_observable
                    && prey.location_height_mm >= 0
                    && ((prey.location_height_mm as i64 - c.location_height_mm as i64).abs()
                        <= species.body_radius_mm as i64 * 2
                        || species.locomotion_for(c.stage) == Locomotion::Flying)
                    && species.food_species.contains(&prey.species_id)
            }) {
                let (mass, water) = take_cohort(&mut self.cohorts[i], 1)?;
                let intake = mass.min((c.count as u64 * 160).saturating_sub(c.energy));
                c.energy = add(c.energy, intake)?;
                self.root_zones[c.patch as usize].litter =
                    add(self.root_zones[c.patch as usize].litter, mass - intake)?;
                self.root_zones[c.patch as usize].shallow_water =
                    add(self.root_zones[c.patch as usize].shallow_water, water)?;
                self.record_deaths(1, "predation")?;
            }
            return Ok(());
        }
        let start = self.plant_patch_offsets[c.patch as usize];
        let end = self.plant_patch_offsets[c.patch as usize + 1];
        let mut left = need;
        for offset in start..end {
            let index = self.plant_patch_indices[offset];
            let sid = self.plants[index].species_id;
            if self.plants[index].individuals == 0 || !species.food_species.contains(&sid) {
                continue;
            }
            let high_canopy =
                self.species[sid as usize].traits.has(157) && self.plants[index].shoots > 1000;
            if high_canopy
                && c.location_height_mm < 1000
                && species.locomotion_for(c.stage) != Locomotion::Flying
            {
                continue;
            }
            let plant = &mut self.plants[index];
            if species.taxon == Taxon::Pollinator
                && !(c.stage == Stage::Juvenile && species.traits.has(96))
            {
                let food = plant.nectar.min(left);
                plant.nectar -= food;
                c.energy = add(c.energy, food)?;
                left -= food;
                if food > 0 && species.traits.has(106) {
                    if c.pollen.as_ref().is_some_and(|p| {
                        p.species_id == plant.species_id && p.source_patch != plant.patch
                    }) {
                        let pollen = c.pollen.take().unwrap();
                        plant.serviced_pollen = add(plant.serviced_pollen, pollen.units)?;
                    }
                    if c.pollen.is_none() && plant.pollen > 0 {
                        let carried = plant.pollen.min(c.count as u64);
                        plant.pollen -= carried;
                        c.pollen = Some(PollenTicket {
                            species_id: plant.species_id,
                            source_patch: plant.patch,
                            units: carried,
                            expires_tick: self.last_tick + species.metabolism_period_ticks * 6,
                        });
                    }
                    if c.energy > 0 {
                        c.energy -= 1;
                        self.ledger.respired_material = add(self.ledger.respired_material, 1)?;
                    }
                }
            } else {
                let fruit = plant.fruit.min(left);
                plant.fruit -= fruit;
                c.energy = add(c.energy, fruit)?;
                left -= fruit;
                if fruit > 0 && species.traits.has(107) && plant.seeds > 0 {
                    let n = plant.seeds.min(2);
                    plant.seeds -= n;
                    c.carried_seeds.push(SeedTicket {
                        species_id: plant.species_id,
                        units: n,
                        due_tick: self.last_tick + 200,
                    });
                }
                let leaves = plant.leaves.min(left);
                plant.leaves -= leaves;
                c.energy = add(c.energy, leaves)?;
                left -= leaves;
            }
            if left == 0 {
                break;
            }
        }
        Ok(())
    }
    fn advance_transits(&mut self, env: &[EcologyPatch]) -> Result<(), String> {
        let profiles = self.species.clone();
        let pending = std::mem::take(&mut self.transits);
        for mut transit in pending {
            let profile = &profiles[transit.organism.species_id as usize];
            if transit.arrival_tick <= self.last_tick
                && Self::habitat_usable(
                    profile,
                    transit.organism.stage,
                    &env[transit.destination as usize],
                )
            {
                transit.organism.patch = transit.destination;
                self.cohorts.push(transit.organism);
                continue;
            }
            if transit.arrival_tick <= self.last_tick {
                let origin = transit.organism.patch as usize;
                if Self::habitat_usable(profile, transit.organism.stage, &env[origin])
                    && transit.destination != origin as u32
                {
                    transit.from_patch = transit.destination;
                    transit.destination = origin as u32;
                    transit.departure_tick = self.last_tick;
                    transit.arrival_tick =
                        self.last_tick + Self::travel_ticks(profile, transit.organism.stage);
                }
            }
            let source = transit.organism.patch as usize;
            self.advance_animal_profile(&mut transit.organism, &env[source], false, profile, true)?;
            if transit.organism.count > 0 {
                self.transits.push(transit)
            }
        }
        Ok(())
    }
    fn wild_reproduction(&mut self, env: &[EcologyPatch]) -> Result<(), String> {
        let donors: Vec<(u64, u32, u32, Sex, u32)> = self
            .cohorts
            .iter()
            .filter(|c| c.count > 0 && c.stage == Stage::Adult && c.health_permille > 600)
            .map(|c| (c.id, c.species_id, c.patch, c.sex, c.count))
            .collect();
        let mut gestating = std::collections::HashMap::<u64, u32>::new();
        for cl in self
            .clutches
            .iter()
            .filter(|cl| cl.reproduction == Reproduction::LiveBirth)
        {
            *gestating.entry(cl.mother_id).or_default() += 1;
        }
        let mut conceptions = Vec::new();
        for c in &mut self.cohorts {
            if c.count == 0
                || c.sex == Sex::Male
                || c.stage != Stage::Adult
                || c.next_breeding_tick > self.last_tick
                || c.dormant_until > self.last_tick
                || c.engineering_busy_until > self.last_tick
            {
                continue;
            }
            let s = &self.species[c.species_id as usize];
            if s.reproduction == Reproduction::LiveBirth
                && gestating.get(&c.id).copied().unwrap_or(0) >= c.count
            {
                continue;
            }
            let condition = &env[c.patch as usize];
            if s.traits.has(96)
                && s.juvenile_habitat == Habitat::Water
                && condition.water_depth_mm < 20
            {
                continue;
            }
            if s.traits.has(64)
                && !c
                    .breeding_refuge
                    .as_ref()
                    .is_some_and(|r| r.accessible && r.den_id.is_some() && r.den_slots >= c.count)
            {
                continue;
            }
            if s.traits.has(65)
                && !c.breeding_refuge.as_ref().is_some_and(|r| {
                    r.accessible
                        && r.nest_id.is_some()
                        && r.nest_slots >= s.clutch_size.min(2) as u32
                })
            {
                continue;
            }
            if (self.last_tick / 18_000) % 4 != s.preferred_season as u64 {
                continue;
            }
            let mate = donors.iter().any(|&(id, sid, p, sex, count)| {
                sid == c.species_id
                    && p == c.patch
                    && sex != Sex::Female
                    && (id != c.id || count > 1)
            });
            let stored = c.sperm.as_ref().is_some_and(|ticket| {
                ticket.donor_species == c.species_id
                    && ticket.doses > 0
                    && ticket.expires_tick > self.last_tick
            });
            if !(mate || stored)
                || c.health_permille < 700
                || c.energy < c.count as u64 * 80
                || c.hydration < c.count as u64 * 8
            {
                continue;
            }
            let count = (s.clutch_size as u32)
                .min(2)
                .min((c.energy / (NEW_BODY + NEW_RESERVE)) as u32);
            if count == 0 {
                continue;
            }
            let storage_cost = if mate && s.traits.has(84) && !stored {
                2
            } else {
                0
            };
            if c.energy < count as u64 * (NEW_BODY + NEW_RESERVE) + storage_cost {
                continue;
            }
            if storage_cost > 0 {
                c.energy -= storage_cost;
                self.ledger.respired_material = add(self.ledger.respired_material, storage_cost)?;
                c.sperm = Some(SpermTicket {
                    donor_species: c.species_id,
                    doses: 4,
                    expires_tick: self.last_tick + SEASON_TICKS,
                });
            }
            let mass = count as u64 * (NEW_BODY + NEW_RESERVE);
            let water = count as u64 * 4;
            c.energy -= mass;
            c.hydration -= water;
            c.next_breeding_tick = self.last_tick + s.breeding_interval_ticks();
            if let Some(ticket) = &mut c.sperm {
                if ticket.doses > 0 {
                    ticket.doses -= 1;
                }
            }
            conceptions.push((
                c.id,
                c.species_id,
                count,
                Custody::Wild(c.patch),
                mass,
                water,
                s.gestation_ticks,
                s.traits.has(93),
                s.reproduction,
            ));
        }
        for (
            mother_id,
            species_id,
            count,
            custody,
            material,
            water,
            duration,
            guarded,
            reproduction,
        ) in conceptions
        {
            let id = self.alloc_id()?;
            self.clutches.push(Clutch {
                id,
                species_id,
                count,
                mother_id,
                custody,
                housing: None,
                patch: match custody {
                    Custody::Wild(p) => p,
                    Custody::Captive(_) => 0,
                },
                hatch_tick: self.last_tick + duration,
                material,
                water,
                temperature_sum: 0,
                temperature_samples: 0,
                guarded,
                reproduction,
            });
            self.ledger.conceived_animals = add(self.ledger.conceived_animals, count as u64)?;
        }
        Ok(())
    }
    fn advance_captives(&mut self, env: &[EcologyPatch]) -> Result<(), String> {
        let donors: Vec<(u8, u32, Sex, u64)> = self
            .captives
            .iter()
            .filter(|c| c.animal.stage == Stage::Adult && c.animal.count > 0 && self.is_housed(c))
            .map(|c| {
                (
                    c.team,
                    c.animal.species_id,
                    c.animal.sex,
                    c.housing.unwrap(),
                )
            })
            .collect();
        let mut occupancy: [std::collections::HashMap<u64, u64>; 2] = [
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        ];
        for team in 0..2u8 {
            for house in &self.housing[team as usize] {
                occupancy[team as usize].insert(house.id, self.housing_occupancy(team, house.id)?);
            }
        }
        let mut gestating = std::collections::HashMap::<u64, u32>::new();
        for cl in self
            .clutches
            .iter()
            .filter(|cl| cl.reproduction == Reproduction::LiveBirth)
        {
            *gestating.entry(cl.mother_id).or_default() += 1;
        }
        let mut studied = std::collections::HashSet::new();
        let pending = std::mem::take(&mut self.captives);
        for mut captive in pending {
            let patch = captive.animal.patch as usize;
            let housed = self.is_housed(&captive);
            let food = (if housed {
                captive.animal.count as u64 * 8
            } else {
                0
            })
            .min(captive.care_food)
            .min((captive.animal.count as u64 * 160).saturating_sub(captive.animal.energy));
            captive.care_food -= food;
            captive.animal.energy = add(captive.animal.energy, food)?;
            let water = (if housed {
                captive.animal.count as u64 * 4
            } else {
                0
            })
            .min(captive.care_water)
            .min((captive.animal.count as u64 * 80).saturating_sub(captive.animal.hydration));
            captive.care_water -= water;
            captive.animal.hydration = add(captive.animal.hydration, water)?;
            self.advance_animal(&mut captive.animal, &env[patch], true)?;
            if captive.animal.count == 0 {
                self.root_zones[patch].nutrients =
                    add(self.root_zones[patch].nutrients, captive.care_food)?;
                self.root_zones[patch].shallow_water =
                    add(self.root_zones[patch].shallow_water, captive.care_water)?;
                continue;
            }
            let k = &mut self.knowledge[captive.team as usize][captive.animal.species_id as usize];
            if k.enabled
                && housed
                && captive.animal.health_permille >= 700
                && captive.care_food > 0
                && captive.care_water > 0
                && captive.animal.energy >= captive.animal.count as u64 * 40
                && captive.animal.hydration >= captive.animal.count as u64 * 8
                && studied.insert((captive.team, captive.animal.species_id))
            {
                k.study_ticks = add(k.study_ticks, ECOLOGY_INTERVAL)?.min(2400);
            }
            let c = &mut captive.animal;
            let s = &self.species[c.species_id as usize];
            let partner = donors.iter().any(|&(team, sid, sex, pen)| {
                team == captive.team
                    && sid == c.species_id
                    && sex != Sex::Female
                    && captive.housing == Some(pen)
            });
            let pen = captive.housing.filter(|_| housed);
            let free = pen
                .and_then(|id| {
                    self.housing[captive.team as usize]
                        .iter()
                        .find(|h| h.id == id)
                        .map(|h| {
                            (h.capacity as u64).saturating_sub(
                                *occupancy[captive.team as usize].get(&id).unwrap_or(&0),
                            )
                        })
                })
                .unwrap_or(0);
            if captive.breed_enabled
                && (s.reproduction != Reproduction::LiveBirth
                    || gestating.get(&c.id).copied().unwrap_or(0) < c.count)
                && housed
                && free >= s.clutch_size.min(2) as u64
                && (partner
                    || c.sperm
                        .as_ref()
                        .is_some_and(|t| t.doses > 0 && t.expires_tick > self.last_tick))
                && c.sex != Sex::Male
                && c.stage == Stage::Adult
                && c.energy >= 80
                && c.hydration >= 8
                && c.next_breeding_tick <= self.last_tick
                && c.health_permille > 700
            {
                let stored = c
                    .sperm
                    .as_ref()
                    .is_some_and(|t| t.doses > 0 && t.expires_tick > self.last_tick);
                let storage_cost = if partner && s.traits.has(84) && !stored {
                    2
                } else {
                    0
                };
                if c.energy < 80 + storage_cost {
                    self.captives.push(captive);
                    continue;
                }
                if storage_cost > 0 {
                    c.energy -= storage_cost;
                    self.ledger.respired_material =
                        add(self.ledger.respired_material, storage_cost)?;
                    c.sperm = Some(SpermTicket {
                        donor_species: c.species_id,
                        doses: 4,
                        expires_tick: self.last_tick + SEASON_TICKS,
                    });
                }
                let count = (s.clutch_size as u32).min(2);
                let occupied = occupancy[captive.team as usize]
                    .get_mut(&pen.unwrap())
                    .ok_or("Missing housing occupancy")?;
                *occupied = add(*occupied, count as u64)?;
                let material = count as u64 * (NEW_BODY + NEW_RESERVE);
                let water = count as u64 * 4;
                c.energy -= material;
                c.hydration -= water;
                c.next_breeding_tick = self.last_tick + s.breeding_interval_ticks();
                if let Some(ticket) = &mut c.sperm {
                    if ticket.doses > 0 {
                        ticket.doses -= 1;
                    }
                }
                let cl = Clutch {
                    id: 0,
                    species_id: c.species_id,
                    count,
                    mother_id: c.id,
                    custody: Custody::Captive(captive.team),
                    housing: captive.housing,
                    patch: c.patch,
                    hatch_tick: self.last_tick + s.gestation_ticks,
                    material,
                    water,
                    temperature_sum: 0,
                    temperature_samples: 0,
                    guarded: s.traits.has(93),
                    reproduction: s.reproduction,
                };
                let id = self.alloc_id()?;
                self.clutches.push(Clutch { id, ..cl });
                self.ledger.conceived_animals = add(self.ledger.conceived_animals, count as u64)?;
            }
            self.captives.push(captive);
        }
        Ok(())
    }
    fn nurse_young(&mut self) -> Result<(), String> {
        let young: Vec<(u64, u64, u32, u32)> = self
            .cohorts
            .iter()
            .filter(|c| c.stage == Stage::Juvenile && c.count > 0)
            .filter_map(|c| c.parent_id.map(|p| (c.id, p, c.patch, c.count)))
            .collect();
        for (child, mother, patch, count) in young {
            let milk = if let Some(m) = self
                .cohorts
                .iter_mut()
                .find(|m| m.id == mother && m.patch == patch && m.count > 0)
            {
                let n = m.milk.min(count as u64 * 4);
                m.milk -= n;
                n
            } else {
                0
            };
            if let Some(c) = self.cohorts.iter_mut().find(|c| c.id == child) {
                c.energy = add(c.energy, milk)?;
            }
        }
        let young: Vec<(u64, u64, u8, u32, Option<u64>)> = self
            .captives
            .iter()
            .filter(|c| {
                c.animal.stage == Stage::Juvenile && c.animal.count > 0 && self.is_housed(c)
            })
            .filter_map(|c| {
                c.animal
                    .parent_id
                    .map(|p| (c.animal.id, p, c.team, c.animal.count, c.housing))
            })
            .collect();
        for (child, mother, team, count, housing) in young {
            let milk = if let Some(m) = self
                .captives
                .iter_mut()
                .find(|m| m.animal.id == mother && m.team == team && m.housing == housing)
            {
                let n = m.animal.milk.min(count as u64 * 4);
                m.animal.milk -= n;
                n
            } else {
                0
            };
            if let Some(c) = self.captives.iter_mut().find(|c| c.animal.id == child) {
                c.animal.energy = add(c.animal.energy, milk)?;
            }
        }
        Ok(())
    }
    fn advance_clutches(
        &mut self,
        env: &[EcologyPatch],
        effects: &mut Vec<EcologyEffect>,
    ) -> Result<(), String> {
        let mut available_mothers: std::collections::HashMap<u64, u32> = self
            .cohorts
            .iter()
            .chain(self.transits.iter().map(|t| &t.organism))
            .chain(self.captives.iter().map(|c| &c.animal))
            .filter(|c| c.count > 0)
            .map(|c| (c.id, c.count))
            .collect();
        let old = std::mem::take(&mut self.clutches);
        for mut clutch in old {
            if clutch.reproduction == Reproduction::LiveBirth {
                if let Some(parent) = self
                    .cohorts
                    .iter()
                    .chain(self.transits.iter().map(|t| &t.organism))
                    .chain(self.captives.iter().map(|c| &c.animal))
                    .find(|c| c.id == clutch.mother_id && c.count > 0)
                {
                    clutch.patch = parent.patch;
                }
            }
            let viable_gestation = if clutch.reproduction == Reproduction::LiveBirth {
                if let Some(slots) = available_mothers.get_mut(&clutch.mother_id) {
                    if *slots > 0 {
                        *slots -= 1;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                true
            };
            let patch = clutch.patch;
            let condition = &env[patch as usize];
            let species = &self.species[clutch.species_id as usize];
            let mother_alive = self
                .cohorts
                .iter()
                .any(|c| c.id == clutch.mother_id && c.count > 0)
                || self
                    .transits
                    .iter()
                    .any(|t| t.organism.id == clutch.mother_id && t.organism.count > 0)
                || self
                    .captives
                    .iter()
                    .any(|c| c.animal.id == clutch.mother_id && c.animal.count > 0);
            let can_guard = match clutch.custody {
                Custody::Wild(_) => self.cohorts.iter().any(|c| {
                    c.id == clutch.mother_id
                        && c.patch == patch
                        && c.energy > 0
                        && c.dormant_until <= self.last_tick
                }),
                Custody::Captive(team) => self.captives.iter().any(|c| {
                    c.team == team
                        && c.animal.id == clutch.mother_id
                        && c.animal.patch == patch
                        && c.animal.energy > 0
                        && c.housing == clutch.housing
                }),
            };
            let guarded = clutch.guarded && mother_alive && can_guard && condition.accessible;
            let flooded = species.habitat_for(Stage::Juvenile) != Habitat::Water
                && condition.water_depth_mm > if guarded { 1000 } else { 400 };
            if flooded
                || clutch.reproduction == Reproduction::LiveBirth
                    && (!mother_alive || !viable_gestation)
            {
                self.root_zones[patch as usize].litter =
                    add(self.root_zones[patch as usize].litter, clutch.material)?;
                self.root_zones[patch as usize].shallow_water =
                    add(self.root_zones[patch as usize].shallow_water, clutch.water)?;
                self.record_deaths(
                    clutch.count as u64,
                    if flooded {
                        "offspring/flooding"
                    } else {
                        "offspring/maternal_loss"
                    },
                )?;
                continue;
            }
            if guarded {
                let cost = 1;
                if let Some(c) = self
                    .cohorts
                    .iter_mut()
                    .find(|c| c.id == clutch.mother_id && c.energy > 0)
                {
                    c.energy -= cost;
                    self.ledger.respired_material = add(self.ledger.respired_material, cost)?;
                } else if let Some(c) = self
                    .captives
                    .iter_mut()
                    .find(|c| c.animal.id == clutch.mother_id && c.animal.energy > 0)
                {
                    c.animal.energy -= cost;
                    self.ledger.respired_material = add(self.ledger.respired_material, cost)?;
                }
            }
            let temperature =
                condition.temperature_milli_c - self.canopy_permille(patch) as i32 * 5;
            clutch.temperature_sum = clutch
                .temperature_sum
                .checked_add(temperature as i64)
                .ok_or("Incubation temperature sum overflow")?;
            clutch.temperature_samples = add(clutch.temperature_samples, 1)?;
            if clutch.hatch_tick > self.last_tick {
                self.clutches.push(clutch);
                continue;
            }
            let thermal = species.traits.has(86);
            let average = clutch.temperature_sum / clutch.temperature_samples as i64;
            let female = if thermal {
                if average < 26_000 { clutch.count } else { 0 }
            } else {
                (clutch.count + ((h(self.seed as u64, clutch.id) & 1) as u32)) / 2
            };
            let males = clutch.count - female;
            for (count, sex) in [(female, Sex::Female), (males, Sex::Male)] {
                if count == 0 {
                    continue;
                }
                let id = self.alloc_id()?;
                let c = Cohort {
                    id,
                    species_id: clutch.species_id,
                    patch,
                    count,
                    sex,
                    stage: Stage::Juvenile,
                    age_ticks: 0,
                    body_units: count as u64 * NEW_BODY,
                    energy: count as u64 * NEW_RESERVE,
                    hydration: count as u64 * 4,
                    health_permille: 1000,
                    stamina_permille: 1000,
                    dormant_until: 0,
                    next_breeding_tick: self.last_tick + 600,
                    milk: 0,
                    pollen: None,
                    carried_seeds: Vec::new(),
                    sperm: None,
                    location_height_mm: 0,
                    surface_observable: true,
                    parent_id: Some(clutch.mother_id),
                    engineering_busy_until: 0,
                    breeding_refuge: None,
                };
                match clutch.custody {
                    Custody::Wild(_) => self.cohorts.push(c),
                    Custody::Captive(team) => self.captives.push(Captive {
                        animal: c,
                        team,
                        care_food: 0,
                        care_water: 0,
                        breed_enabled: false,
                        housing: clutch.housing,
                    }),
                }
            }
            effects.push(EcologyEffect::Birth {
                species_id: clutch.species_id,
                patch,
                count: clutch.count,
            });
        }
        Ok(())
    }
    fn advance_seeds(&mut self, env: &[EcologyPatch]) -> Result<(), String> {
        let mut surface_biomass = vec![0u64; self.root_zones.len()];
        let mut newborn: std::collections::BTreeMap<(u32, u32), usize> =
            std::collections::BTreeMap::new();
        for (index, p) in self.plants.iter().enumerate() {
            surface_biomass[p.patch as usize] += p.shoots + p.leaves;
            if p.age_ticks == 0 {
                newborn.insert((p.patch, p.species_id), index);
            }
        }
        let pending = std::mem::take(&mut self.seeds);
        for mut seed in pending {
            if seed.units == 0 {
                continue;
            }
            let p = seed.patch as usize;
            if seed.expires_tick <= self.last_tick {
                self.root_zones[p].litter = add(self.root_zones[p].litter, seed.units)?;
                continue;
            }
            if self.species[seed.species_id as usize].traits.has(143) && env[p].water_depth_mm > 10
            {
                seed.afloat = true;
            }
            if seed.afloat && env[p].water_depth_mm > 10 {
                if let Some(destination) = env[p].flow_to {
                    let fraction = env[p].flow_fraction_permille as u128;
                    let numerator =
                        seed.units as u128 * fraction + seed.advection_remainder as u128;
                    let moved = (numerator / 1000) as u64;
                    seed.advection_remainder = (numerator % 1000) as u16;
                    if moved > 0 {
                        let transported = SeedStock {
                            patch: destination,
                            units: moved,
                            advection_remainder: 0,
                            ..seed.clone()
                        };
                        seed.units -= moved;
                        self.seeds.push(transported);
                    }
                }
                if seed.units > 0 {
                    self.seeds.push(seed);
                }
                continue;
            }
            seed.afloat = false;
            let s = &self.species[seed.species_id as usize];
            let cover = self.ground_cover_permille(seed.patch);
            let bare = surface_biomass[p] < 500;
            let eligible = env[p].accessible
                && env[p].water_depth_mm < 100
                && env[p].moisture_permille > 100
                && (bare || s.traits.has(128) && env[p].disturbance > 0 && cover < 700);
            if eligible && self.root_zones[p].nutrients >= 3 && self.root_zones[p].shallow_water > 0
            {
                seed.units -= 1;
                self.root_zones[p].nutrients -= 3;
                self.root_zones[p].shallow_water -= 1;
                self.ledger.evaporated_water = add(self.ledger.evaporated_water, 1)?;
                self.ledger.germinated_plants = add(self.ledger.germinated_plants, 1)?;
                surface_biomass[p] += 2;
                if let Some(&index) = newborn.get(&(seed.patch, seed.species_id)) {
                    let plant = &mut self.plants[index];
                    plant.individuals = plant
                        .individuals
                        .checked_add(1)
                        .ok_or("Plant count overflow")?;
                    plant.roots += 2;
                    plant.shoots += 2;
                    plant.age_ticks = 0;
                } else {
                    newborn.insert((seed.patch, seed.species_id), self.plants.len());
                    self.plants.push(PlantPatch {
                        patch: seed.patch,
                        species_id: seed.species_id,
                        individuals: 1,
                        age_ticks: 0,
                        shoots: 2,
                        roots: 2,
                        leaves: 0,
                        fruit: 0,
                        nectar: 0,
                        seeds: 0,
                        pollen: 0,
                        serviced_pollen: 0,
                        submerged_ticks: 0,
                        initial_live_units: 4,
                        initial_root_units: 2,
                    });
                }
            }
            if seed.units > 0 {
                self.seeds.push(seed)
            }
        }
        for plant in &mut self.plants {
            if plant.individuals == 0 && plant.seeds > 0 && self.last_tick % 100 == 0 {
                plant.seeds -= 1;
                self.seeds.push(SeedStock {
                    species_id: plant.species_id,
                    patch: plant.patch,
                    units: 1,
                    expires_tick: self.last_tick + SEASON_TICKS,
                    advection_remainder: 0,
                    afloat: env[plant.patch as usize].water_depth_mm > 10
                        && self.species[plant.species_id as usize].traits.has(143),
                });
            }
            let persistent = self.species[plant.species_id as usize].traits.has(132);
            if plant.seeds > 0 && self.last_tick % if persistent { SEASON_TICKS } else { 800 } == 0
            {
                plant.seeds -= 1;
                self.root_zones[plant.patch as usize].litter += 1;
            }
        }
        Ok(())
    }
    fn species_alive(&self, id: u32) -> bool {
        let s = &self.species[id as usize];
        if s.taxon == Taxon::Fungus {
            return self.root_zones.iter().any(|z| z.fungal_biomass > 0);
        }
        self.plants
            .iter()
            .any(|p| p.species_id == id && (p.individuals > 0 || p.seeds > 0))
            || self
                .cohorts
                .iter()
                .any(|c| c.species_id == id && c.count > 0)
            || self
                .transits
                .iter()
                .any(|t| t.organism.species_id == id && t.organism.count > 0)
            || self
                .captives
                .iter()
                .any(|c| c.animal.species_id == id && c.animal.count > 0)
            || self
                .clutches
                .iter()
                .any(|c| c.species_id == id && c.count > 0)
            || self.seeds.iter().any(|s| s.species_id == id && s.units > 0)
            || self.cohorts.iter().any(|c| {
                c.carried_seeds
                    .iter()
                    .any(|s| s.species_id == id && s.units > 0)
            })
            || self.transits.iter().any(|t| {
                t.organism
                    .carried_seeds
                    .iter()
                    .any(|s| s.species_id == id && s.units > 0)
            })
            || self.captives.iter().any(|c| {
                c.animal
                    .carried_seeds
                    .iter()
                    .any(|s| s.species_id == id && s.units > 0)
            })
    }
    fn detect_extinction(&mut self, effects: &mut Vec<EcologyEffect>) -> Result<(), String> {
        for id in 0..self.species.len() {
            let alive = self.species_alive(id as u32);
            if self.extinct[id] && alive {
                return Err("Extinct species was recreated without viable surviving stock".into());
            }
            if !alive && !self.extinct[id] {
                self.extinct[id] = true;
                effects.push(EcologyEffect::Extinction {
                    species_id: id as u32,
                });
            }
        }
        Ok(())
    }
    fn material_total(&self) -> Result<u64, String> {
        let mut total = 0;
        for p in &self.plants {
            total = add(
                total,
                [
                    p.shoots,
                    p.roots,
                    p.leaves,
                    p.fruit,
                    p.nectar,
                    p.seeds,
                    p.pollen,
                    p.serviced_pollen,
                ]
                .into_iter()
                .try_fold(0, add)?,
            )?
        }
        for z in &self.root_zones {
            total = add(total, add(add(z.nutrients, z.litter)?, z.fungal_biomass)?)?
        }
        for c in &self.cohorts {
            total = add(total, checked_cohort_material(c)?)?
        }
        for t in &self.transits {
            total = add(total, checked_cohort_material(&t.organism)?)?
        }
        for c in &self.captives {
            total = add(
                total,
                add(checked_cohort_material(&c.animal)?, c.care_food)?,
            )?
        }
        for c in &self.clutches {
            total = add(total, c.material)?
        }
        for s in &self.seeds {
            total = add(total, s.units)?
        }
        Ok(total)
    }
    fn water_total(&self) -> Result<u64, String> {
        let mut total = 0;
        for z in &self.root_zones {
            total = add(total, add(z.shallow_water, z.deep_water)?)?
        }
        for c in &self.cohorts {
            total = add(total, c.hydration)?
        }
        for t in &self.transits {
            total = add(total, t.organism.hydration)?
        }
        for c in &self.captives {
            total = add(total, add(c.animal.hydration, c.care_water)?)?
        }
        for c in &self.clutches {
            total = add(total, c.water)?
        }
        Ok(total)
    }
    fn animal_total(&self) -> Result<u64, String> {
        let mut n = 0;
        for c in &self.cohorts {
            n = add(n, c.count as u64)?
        }
        for t in &self.transits {
            n = add(n, t.organism.count as u64)?
        }
        for c in &self.captives {
            n = add(n, c.animal.count as u64)?
        }
        for c in &self.clutches {
            n = add(n, c.count as u64)?
        }
        Ok(n)
    }
    pub fn census(&self) -> Census {
        Census {
            wild_animals: self.cohorts.iter().map(|x| x.count as u64).sum(),
            generation_accepted: self.generation_accepted,
            generation_relocations: self.generation_relocations,
            transit_animals: self.transits.iter().map(|x| x.organism.count as u64).sum(),
            captive_animals: self.captives.iter().map(|x| x.animal.count as u64).sum(),
            viable_offspring: self.clutches.iter().map(|x| x.count as u64).sum(),
            living_plants: self.plants.iter().map(|x| x.individuals as u64).sum(),
            viable_seeds: self.plants.iter().map(|x| x.seeds).sum::<u64>()
                + self.seeds.iter().map(|x| x.units).sum::<u64>(),
            biomass_units: self.material_total().unwrap_or(u64::MAX),
            root_zone_water: self
                .root_zones
                .iter()
                .map(|z| z.shallow_water + z.deep_water)
                .sum(),
            animal_hydration: self.water_total().unwrap_or(u64::MAX)
                - self
                    .root_zones
                    .iter()
                    .map(|z| z.shallow_water + z.deep_water)
                    .sum::<u64>(),
            dead_animals: self.ledger.dead_animals,
            death_reasons: self.ledger.death_reasons.clone(),
            dead_plants: self.ledger.dead_plants,
            extinct_species: self
                .extinct
                .iter()
                .enumerate()
                .filter(|(_, b)| **b)
                .map(|(i, _)| i as u32)
                .collect(),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let patches = self.root_zones.len();
        let species = self.species.len();
        if patches == 0
            || patches > 1_000_000
            || self.patch_width == 0
            || !patches.is_multiple_of(self.patch_width)
            || species == 0
            || species > 4096
            || self.extinct.len() != species
        {
            return Err("Invalid ecological dimensions/catalogue".into());
        }
        if self.knowledge.iter().any(|k| k.len() != species)
            || self.visible_patches.iter().any(|v| v.len() != patches)
            || self.cached_canopy.len() != patches
            || self.cached_ground_cover.len() != patches
            || self.cached_fraction.len() != patches
            || self.cached_rich_litter.len() != patches
            || self.plant_patch_offsets.len() != patches + 1
            || self.plant_patch_indices.len() != self.plants.len()
        {
            return Err("Invalid ecology knowledge/cache dimensions".into());
        }
        for (i, s) in self.species.iter().enumerate() {
            if s.id as usize != i
                || s.minimum_temperature_milli_c > s.maximum_temperature_milli_c
                || s.metabolism_period_ticks < ECOLOGY_INTERVAL
                || s.water_period_ticks < ECOLOGY_INTERVAL
                || !s.metabolism_period_ticks.is_multiple_of(ECOLOGY_INTERVAL)
                || !s.water_period_ticks.is_multiple_of(ECOLOGY_INTERVAL)
                || s.metabolism_period_ticks > SEASON_TICKS
                || s.water_period_ticks > SEASON_TICKS * 4
                || s.leaf_output_permille as u32 + s.nectar_output_permille as u32 > 900
                || s.plant_output_units == 0
                || s.clutch_size == 0
                || s.maturity_ticks == 0
                || s.gestation_ticks == 0
                || s.lifespan_ticks < s.maturity_ticks
            {
                return Err("Invalid immutable species profile".into());
            }
            for &food in &s.food_species {
                let Some(prey) = self.species.get(food as usize) else {
                    return Err("Invalid food-web edge".into());
                };
                if matches!(s.taxon, Taxon::Plant | Taxon::Fungus)
                    || s.taxon == Taxon::Predator
                        && !matches!(prey.taxon, Taxon::Herbivore | Taxon::Pollinator)
                    || matches!(s.taxon, Taxon::Herbivore | Taxon::Pollinator)
                        && prey.taxon != Taxon::Plant
                {
                    return Err("Incompatible food-web edge".into());
                }
            }
        }
        for plant in &self.plants {
            if plant.patch as usize >= patches
                || plant.species_id as usize >= species
                || self.species[plant.species_id as usize].taxon != Taxon::Plant
            {
                return Err("Invalid plant cohort reference".into());
            }
        }
        let all = self
            .cohorts
            .iter()
            .chain(self.transits.iter().map(|t| &t.organism))
            .chain(self.captives.iter().map(|c| &c.animal));
        let mut owned_ids = std::collections::HashSet::new();
        for c in all {
            if c.id == 0
                || c.id >= self.next_id
                || !owned_ids.insert(c.id)
                || c.patch as usize >= patches
                || c.species_id as usize >= species
                || matches!(
                    self.species[c.species_id as usize].taxon,
                    Taxon::Plant | Taxon::Fungus
                )
                || c.health_permille > 1000
                || c.stamina_permille > 1000
            {
                return Err("Invalid or duplicated organism owner".into());
            }
            if c.count == 0 && (checked_cohort_material(c)? > 0 || c.hydration > 0) {
                return Err("Empty cohort still owns organism stocks".into());
            }
            if c.pollen.as_ref().is_some_and(|p| {
                p.species_id as usize >= species || p.source_patch as usize >= patches
            }) || c
                .carried_seeds
                .iter()
                .any(|s| s.species_id as usize >= species)
                || c.sperm
                    .as_ref()
                    .is_some_and(|s| s.donor_species != c.species_id)
            {
                return Err("Invalid organism reproductive/propagule reference".into());
            }
        }
        for t in &self.transits {
            if t.destination as usize >= patches || t.from_patch as usize >= patches {
                return Err("Invalid migration destination".into());
            }
        }
        for team in 0..2usize {
            let mut ids = std::collections::HashSet::new();
            for house in &self.housing[team] {
                if house.id == 0
                    || house.patch as usize >= patches
                    || house.capacity == 0
                    || !ids.insert(house.id)
                {
                    return Err("Invalid owned holding-pen reference".into());
                }
                if self.housing_occupancy(team as u8, house.id)? > house.capacity as u64 {
                    return Err("Holding pen exceeds finite capacity".into());
                }
            }
        }
        for c in &self.captives {
            if c.team > 1 {
                return Err("Invalid captive owner".into());
            }
            if c.housing.is_some() && !self.is_housed(c) {
                return Err("Captive refers to absent or wrong-location housing".into());
            }
        }
        for c in &self.clutches {
            if let Some(id) = c.housing {
                match c.custody {
                    Custody::Captive(team) if team < 2 => {
                        if !self.housing[team as usize]
                            .iter()
                            .any(|h| h.id == id && h.patch == c.patch)
                        {
                            return Err(
                                "Viable offspring refers to absent or wrong-location housing"
                                    .into(),
                            );
                        }
                    }
                    _ => return Err("Wild offspring cannot reserve a faction pen".into()),
                }
            }
            if c.id == 0
                || c.id >= self.next_id
                || !owned_ids.insert(c.id)
                || c.patch as usize >= patches
                || c.species_id as usize >= species
                || c.count == 0
                || c.material != c.count as u64 * (NEW_BODY + NEW_RESERVE)
                || c.water != c.count as u64 * 4
                || matches!(c.custody,Custody::Captive(t)if t>1)
            {
                return Err("Invalid viable offspring owner".into());
            }
        }
        for seed in &self.seeds {
            if seed.patch as usize >= patches
                || seed.species_id as usize >= species
                || seed.advection_remainder >= 1000
            {
                return Err("Invalid viable seed stock".into());
            }
        }
        let material = add(
            add(self.material_total()?, self.ledger.respired_material)?,
            self.ledger.material_exported,
        )?;
        let expected = add(self.ledger.initial_material, self.ledger.material_imported)?;
        if material != expected {
            return Err(format!(
                "Ecology material imbalance actual{material} expected{expected}"
            ));
        }
        let water = add(
            add(self.water_total()?, self.ledger.evaporated_water)?,
            self.ledger.water_exported,
        )?;
        let expected = add(
            add(self.ledger.initial_water, self.ledger.rain_water)?,
            self.ledger.transferred_water_in,
        )?;
        if water != expected {
            return Err(format!(
                "Ecology water imbalance actual{water} expected{expected}"
            ));
        }
        if add(self.animal_total()?, self.ledger.dead_animals)?
            != add(self.ledger.initial_animals, self.ledger.conceived_animals)?
        {
            return Err("Ecology organism count imbalance".into());
        }
        let plants = self
            .plants
            .iter()
            .map(|p| p.individuals as u64)
            .try_fold(0, add)?;
        if add(plants, self.ledger.dead_plants)?
            != add(self.ledger.initial_plants, self.ledger.germinated_plants)?
        {
            return Err("Ecology plant count imbalance".into());
        }
        let mut ids = std::collections::HashSet::new();
        for c in self
            .cohorts
            .iter()
            .chain(self.transits.iter().map(|t| &t.organism))
            .chain(self.captives.iter().map(|c| &c.animal))
        {
            if !ids.insert(c.id) {
                return Err("Duplicated animal/cohort owner".into());
            }
            if c.patch as usize >= self.root_zones.len()
                || c.species_id as usize >= self.species.len()
            {
                return Err("Invalid organism reference".into());
            }
        }
        Ok(())
    }
}

fn checked_cohort_material(c: &Cohort) -> Result<u64, String> {
    [
        c.body_units,
        c.energy,
        c.milk,
        c.pollen.as_ref().map_or(0, |p| p.units),
    ]
    .into_iter()
    .chain(c.carried_seeds.iter().map(|s| s.units))
    .try_fold(0, add)
}

/// Withdraw whole organisms exactly. Shared reserve records remain with the
/// surviving cohort; when it disappears all its remaining stocks move too.
fn take_cohort(c: &mut Cohort, count: u32) -> Result<(u64, u64), String> {
    if count > c.count {
        return Err("Cannot kill more organisms than exist".into());
    }
    if count == 0 {
        return Ok((0, 0));
    }
    let old = c.count as u64;
    let n = count as u64;
    let body = c.body_units * n / old;
    let energy = c.energy * n / old;
    let milk = c.milk * n / old;
    let water = c.hydration * n / old;
    c.body_units -= body;
    c.energy -= energy;
    c.milk -= milk;
    c.hydration -= water;
    c.count -= count;
    let mut mass = body + energy + milk;
    if c.count == 0 {
        if let Some(p) = c.pollen.take() {
            mass = add(mass, p.units)?
        }
        for s in c.carried_seeds.drain(..) {
            mass = add(mass, s.units)?
        }
        c.sperm = None;
    }
    Ok((mass, water))
}

fn generate_species(seed: u32) -> Vec<Species> {
    let prefixes = [
        "Violet", "Amber", "Jade", "Copper", "Azure", "Coral", "Moss", "Indigo",
    ];
    let suffixes = [
        "crown", "fan", "reed", "fern", "bloom", "creeper", "grazer", "burrower", "skipper",
        "weaver", "moth", "glider", "stalker", "hunter", "mycelium",
    ];
    let plans: &[&[u16]] = &[
        &[121, 129, 132, 157, 159],
        &[125, 128, 143, 156],
        &[121, 125, 132, 143],
        &[128, 129, 132, 156, 159],
        &[132, 135, 157],
        &[125, 135, 143, 156],
        &[12, 61, 64, 72, 75, 84, 102],
        &[11, 65, 93, 103, 104, 107],
        &[61, 67, 75, 84, 107],
        &[65, 86, 93, 96, 105],
        &[61, 86, 96, 106],
        &[11, 65, 72, 106],
        &[46, 64, 65, 93],
        &[12, 64, 72, 75, 101],
        &[],
    ];
    let mut result = Vec::new();
    for id in 0..15u32 {
        let k = h(seed as u64, id as u64);
        let plant = id < 6;
        let taxon = if plant {
            Taxon::Plant
        } else if id < 10 {
            Taxon::Herbivore
        } else if id < 12 {
            Taxon::Pollinator
        } else if id < 14 {
            Taxon::Predator
        } else {
            Taxon::Fungus
        };
        let mut traits = TraitMask::default();
        for &trait_id in plans[id as usize] {
            if h(k, trait_id as u64) % 3 != 0 {
                traits.insert(trait_id)
            }
        }
        // Seed-specific optional additions stay within anatomy/habitat families.
        if plant && k & 1 != 0 {
            traits.insert(159)
        }
        if !plant && taxon != Taxon::Fungus && k & 2 != 0 {
            traits.insert(75)
        }
        let food = match taxon {
            Taxon::Herbivore => (0..6)
                .filter(|p| h(k, *p as u64) & 1 != 0 || *p == if id % 2 == 0 { 1 } else { 3 })
                .collect(),
            Taxon::Pollinator => vec![4, 5],
            Taxon::Predator => (6..12)
                .filter(|p| h(k, *p as u64) & 1 != 0 || *p == 6 + id % 6)
                .collect(),
            _ => Vec::new(),
        };
        result.push(Species {
            id,
            name: format!("{} {}", prefixes[(k % 8) as usize], suffixes[id as usize]),
            taxon,
            traits,
            body_mass_g: if plant || taxon == Taxon::Fungus {
                0
            } else if taxon == Taxon::Predator {
                25_000 + ((k >> 8) % 40_000) as u32
            } else if taxon == Taxon::Pollinator {
                50 + ((k >> 8) % 1_000) as u32
            } else {
                100 + ((k >> 8) % 20_000) as u32
            },
            body_radius_mm: if plant {
                0
            } else {
                30 + ((k >> 16) % 300) as u32
            },
            limbs: if plant || taxon == Taxon::Fungus {
                0
            } else if id >= 9 && id <= 11 {
                6
            } else {
                4
            },
            locomotion: if plant || taxon == Taxon::Fungus {
                Locomotion::Rooted
            } else if id >= 10 && id <= 11 {
                Locomotion::Flying
            } else if traits.has(12) {
                Locomotion::Burrowing
            } else if traits.has(11) {
                Locomotion::Climbing
            } else {
                Locomotion::Walking
            },
            habitat: if id == 9 || id == 10 {
                Habitat::Amphibious
            } else {
                Habitat::Land
            },
            food_species: food,
            minimum_temperature_milli_c: 7_000,
            maximum_temperature_milli_c: 44_000,
            metabolism_units_per_step: 1,
            water_units_per_step: 1,
            metabolism_period_ticks: physiology_period(if plant || taxon == Taxon::Fungus {
                50
            } else if taxon == Taxon::Predator {
                25_000 + ((k >> 8) % 40_000) as u32
            } else if taxon == Taxon::Pollinator {
                50 + ((k >> 8) % 1_000) as u32
            } else {
                100 + ((k >> 8) % 20_000) as u32
            }),
            water_period_ticks: physiology_period(if plant || taxon == Taxon::Fungus {
                50
            } else if taxon == Taxon::Predator {
                25_000 + ((k >> 8) % 40_000) as u32
            } else if taxon == Taxon::Pollinator {
                50 + ((k >> 8) % 1_000) as u32
            } else {
                100 + ((k >> 8) % 20_000) as u32
            }) * 2,
            juvenile_habitat: if id == 9 {
                Habitat::Water
            } else {
                Habitat::Land
            },
            plant_output_units: if id == 4 || id == 5 { 16 } else { 10 },
            leaf_output_permille: if id == 4 || id == 5 { 250 } else { 700 },
            nectar_output_permille: if id == 4 || id == 5 { 500 } else { 100 },
            maturity_ticks: 2_400 + (k % 3) * 400,
            lifespan_ticks: if plant {
                SEASON_TICKS * 5
            } else {
                SEASON_TICKS * 2 + (k % 4) * 18_000
            },
            gestation_ticks: 600 + (k % 3) * 200,
            clutch_size: 2,
            reproduction: if plant {
                Reproduction::Seeds
            } else if id == 6 || id == 8 || id == 13 {
                Reproduction::LiveBirth
            } else if taxon == Taxon::Fungus {
                Reproduction::Spores
            } else {
                Reproduction::Eggs
            },
            preferred_season: ((k >> 4) % 4) as u8,
            nocturnal: !plant && k & 4 != 0,
            docile: id == 6 || id == 8,
            slow_movement: !plant && id % 3 == 0,
            milk_production: id == 6 || id == 8,
            social_group_size: if taxon == Taxon::Predator { 2 } else { 12 },
            territorial_radius_m: if taxon == Taxon::Predator { 300 } else { 80 },
            threat_response: if taxon == Taxon::Predator { 2 } else { 1 },
            flood_tolerance_ticks: 400,
        });
    }
    if result
        .iter()
        .any(|s| s.taxon == Taxon::Plant && s.traits.has(135))
        && !result
            .iter()
            .any(|s| s.taxon == Taxon::Pollinator && s.traits.has(106))
    {
        result[10].traits.insert(106);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn disable(mask: &mut TraitMask, id: u16) {
        mask.0[((id - 1) / 32) as usize] &= !(1 << ((id - 1) % 32));
    }
    fn rebase(e: &mut Ecology) {
        e.next_id = e
            .cohorts
            .iter()
            .map(|c| c.id)
            .chain(e.transits.iter().map(|t| t.organism.id))
            .chain(e.captives.iter().map(|c| c.animal.id))
            .chain(e.clutches.iter().map(|c| c.id))
            .max()
            .unwrap_or(0)
            + 1;
        e.ledger = EcologyLedger::default();
        e.ledger.initial_material = e.material_total().unwrap();
        e.ledger.initial_water = e.water_total().unwrap();
        e.ledger.initial_animals = e.animal_total().unwrap();
        e.ledger.initial_plants = e.plants.iter().map(|p| p.individuals as u64).sum();
        e.refresh_fields();
        e.validate().unwrap();
    }
    fn plant_world(id: u32) -> Ecology {
        let mut e = Ecology::new(31, 1).unwrap();
        e.plants.truncate(1);
        e.plants[0].species_id = id;
        let fixture_traits: &[u16] = match id {
            0 => &[121, 129, 132, 157, 159],
            1 => &[125, 128, 143, 156],
            4 => &[132, 135, 157],
            _ => &[],
        };
        for &trait_id in fixture_traits {
            e.species[id as usize].traits.insert(trait_id);
        }
        e.cohorts.clear();
        rebase(&mut e);
        e
    }
    #[test]
    fn generated_food_web_and_anatomy_are_compatible_and_deterministic() {
        let a = Ecology::new(99, 12).unwrap();
        let b = Ecology::new(99, 12).unwrap();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
        for species in &a.species {
            assert!(
                species
                    .traits
                    .ids()
                    .iter()
                    .all(|id| SELECTED_TRAITS.contains(id))
            );
            for &food in &species.food_species {
                let taxon = a.species[food as usize].taxon;
                match species.taxon {
                    Taxon::Predator => {
                        assert!(matches!(taxon, Taxon::Herbivore | Taxon::Pollinator))
                    }
                    Taxon::Herbivore | Taxon::Pollinator => assert_eq!(taxon, Taxon::Plant),
                    _ => panic!("Nonfeeding species has an edge"),
                }
            }
            if species.traits.has(121) {
                assert_eq!(species.taxon, Taxon::Plant)
            }
        }
        assert!(
            a.species
                .iter()
                .any(|s| s.milk_production && s.reproduction == Reproduction::LiveBirth)
        );
        assert!(a.species.iter().any(|s| s.nocturnal));
        assert!(a.species.iter().any(|s| s.docile));
        assert!(a.species.iter().any(|s| s.slow_movement));
    }
    #[test]
    fn cross_seed_population_water_material_ledgers_and_serialized_continuity() {
        for seed in 0..8 {
            let mut e = Ecology::new(seed, 12).unwrap();
            e.set_patch_width(4).unwrap();
            let env = vec![
                EcologyPatch {
                    water_depth_mm: 50,
                    shelter_capacity: 1000,
                    ..Default::default()
                };
                12
            ];
            for tick in (20..=2400).step_by(20) {
                e.step(tick, &env)
                    .unwrap_or_else(|error| panic!("seed{seed} tick{tick}: {error}"));
            }
            let bytes = serde_json::to_vec(&e).unwrap();
            let mut loaded: Ecology = serde_json::from_slice(&bytes).unwrap();
            loaded.validate().unwrap();
            e.step(2420, &env).unwrap();
            loaded.step(2420, &env).unwrap();
            assert_eq!(
                serde_json::to_vec(&e).unwrap(),
                serde_json::to_vec(&loaded).unwrap()
            );
        }
    }
    #[test]
    fn stale_due_ticks_rejected_without_mutation() {
        let mut e = Ecology::new(3, 2).unwrap();
        let before = serde_json::to_vec(&e).unwrap();
        assert!(e.step(40, &vec![EcologyPatch::default(); 2]).is_err());
        assert_eq!(before, serde_json::to_vec(&e).unwrap());
        e.step(20, &vec![EcologyPatch::default(); 2]).unwrap();
        let before = serde_json::to_vec(&e).unwrap();
        assert!(e.step(20, &vec![EcologyPatch::default(); 2]).is_err());
        assert_eq!(before, serde_json::to_vec(&e).unwrap());
    }
    #[test]
    fn capture_care_research_breeding_release_are_real_finite_transfers() {
        let mut e = Ecology::new(5, 1).unwrap();
        let sid = e.cohorts[0].species_id;
        e.species[sid as usize].habitat = Habitat::Land;
        e.species[sid as usize].gestation_ticks = 100;
        e.species[sid as usize].maturity_ticks = 40;
        let before = e.animal_total().unwrap();
        assert!(e.capture(0, sid, 0).is_err());
        assert!(e.observed_inspect(0, sid).is_none());
        e.observe(0, 0, true).unwrap();
        let first = e.capture(0, sid, 0).unwrap();
        let second = e.capture(0, sid, 0).unwrap();
        assert_ne!(first, second);
        assert_ne!(e.captives[0].animal.sex, e.captives[1].animal.sex);
        assert_eq!(e.animal_total().unwrap(), before);
        e.set_housing(
            0,
            vec![Housing {
                id: 1,
                patch: 0,
                capacity: 32,
            }],
        )
        .unwrap();
        e.house_captive(first, 1).unwrap();
        e.house_captive(second, 1).unwrap();
        e.supply_care(0, 10_000, 10_000).unwrap();
        e.research(0, sid).unwrap();
        let env = [EcologyPatch::default()];
        for tick in (20..=800).step_by(20) {
            e.step(tick, &env).unwrap();
        }
        assert_eq!(e.knowledge[0][sid as usize].study_ticks, 800);
        e.breed(0, sid).unwrap();
        for tick in (820..=1200).step_by(20) {
            e.step(tick, &env).unwrap();
        }
        assert!(e.ledger.conceived_animals > 0);
        assert!(e.captives.len() > 2);
        let total = e.animal_total().unwrap();
        e.release(0, sid, 0).unwrap();
        assert_eq!(e.animal_total().unwrap(), total);
        e.validate().unwrap();
        assert!(e.observed_inspect(1, sid).is_none());
        assert!(
            e.observed_inspect(0, sid).unwrap()["traits"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn materialized_capture_transport_housing_release_preserves_identity_and_finite_pregnancy() {
        let mut e = Ecology::new(7, 24).unwrap();
        let owner = e
            .cohorts
            .iter()
            .find(|c| c.species_id == 6 && c.sex == Sex::Female)
            .unwrap()
            .id;
        let count = e.animal_total().unwrap();
        let material = e.material_total().unwrap();
        let water = e.water_total().unwrap();
        let id = e.materialize_one(owner).unwrap();
        assert_eq!(e.animal_total().unwrap(), count);
        assert_eq!(e.material_total().unwrap(), material);
        assert_eq!(e.water_total().unwrap(), water);
        let patch = e.cohorts.iter().find(|c| c.id == id).unwrap().patch;
        // Real mate, reserve-funded conception, and real plant-owned propagules
        // supply every stock used in this handoff; no fixture animals are added.
        disable(&mut e.species[6].traits, 64);
        e.species[6].traits.insert(84);
        e.species[6].preferred_season = 0;
        e.last_tick = 600;
        e.wild_reproduction(&vec![EcologyPatch::default(); 24])
            .unwrap();
        assert_eq!(e.clutches.iter().filter(|cl| cl.mother_id == id).count(), 1);
        let plant = &mut e.plants[0];
        plant.seeds -= 2;
        plant.pollen -= 3;
        let seed_species = plant.species_id;
        let c = e.cohorts.iter_mut().find(|c| c.id == id).unwrap();
        c.carried_seeds.push(SeedTicket {
            species_id: seed_species,
            units: 2,
            due_tick: 1000,
        });
        c.pollen = Some(PollenTicket {
            species_id: seed_species,
            source_patch: 0,
            units: 3,
            expires_tick: 1000,
        });
        c.breeding_refuge = Some(BreedingRefuge {
            den_id: Some(10),
            den_slots: 1,
            nest_id: None,
            nest_slots: 0,
            accessible: true,
        });
        let mut expected = c.clone();
        expected.breeding_refuge = None;
        let animals = e.animal_total().unwrap();
        let material = e.material_total().unwrap();
        let water = e.water_total().unwrap();
        let next_id = e.next_id;
        e.observe(0, patch, true).unwrap();
        assert_eq!(e.capture_cohort(id, 0).unwrap(), id);
        assert_eq!(e.next_id, next_id);
        assert!(!e.cohorts.iter().any(|c| c.id == id));
        assert_eq!(
            serde_json::to_value(&e.captives.last().unwrap().animal).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let destination = (patch + 1) % 24;
        e.set_captive_transit(id, destination).unwrap();
        assert_eq!(
            e.clutches
                .iter()
                .find(|cl| cl.mother_id == id)
                .unwrap()
                .custody,
            Custody::Captive(0)
        );
        assert_eq!(
            e.clutches
                .iter()
                .find(|cl| cl.mother_id == id)
                .unwrap()
                .patch,
            destination
        );
        e.set_housing(
            0,
            vec![Housing {
                id: 77,
                patch: destination,
                capacity: 4,
            }],
        )
        .unwrap();
        e.house_captive(id, 77).unwrap();
        assert!(e.is_housed(e.captives.iter().find(|c| c.animal.id == id).unwrap()));
        e.observe(0, destination, true).unwrap();
        assert_eq!(e.release(0, 6, destination).unwrap(), id);
        expected.patch = destination;
        assert_eq!(
            serde_json::to_value(e.cohorts.iter().find(|c| c.id == id).unwrap()).unwrap(),
            serde_json::to_value(&expected).unwrap()
        );
        let pregnancy = e.clutches.iter().find(|cl| cl.mother_id == id).unwrap();
        assert_eq!(pregnancy.custody, Custody::Wild(destination));
        assert_eq!(pregnancy.patch, destination);
        assert_eq!(e.animal_total().unwrap(), animals);
        assert_eq!(e.material_total().unwrap(), material);
        assert_eq!(e.water_total().unwrap(), water);
        e.validate().unwrap();
    }
    #[test]
    fn physical_pen_delivery_capacity_reserves_offspring_and_loss_stops_care_without_deleting_animals()
     {
        let mut e = Ecology::new(5, 1).unwrap();
        let sid = e.cohorts[0].species_id;
        e.species[sid as usize].habitat = Habitat::Land;
        e.species[sid as usize].gestation_ticks = 100;
        e.observe(0, 0, true).unwrap();
        let female = e.capture(0, sid, 0).unwrap();
        let male = e.capture(0, sid, 0).unwrap();
        assert!(e.supply_care(0, 100, 100).is_err());
        assert!(e.research(0, sid).is_err());
        e.knowledge[0][sid as usize].study_ticks = 800;
        e.set_housing(
            0,
            vec![Housing {
                id: 1,
                patch: 0,
                capacity: 2,
            }],
        )
        .unwrap();
        e.house_captive(female, 1).unwrap();
        e.house_captive(male, 1).unwrap();
        assert_eq!(e.housing_available(0, 1).unwrap(), 0);
        assert!(e.breed(0, sid).is_err());
        e.set_housing(
            0,
            vec![Housing {
                id: 1,
                patch: 0,
                capacity: 4,
            }],
        )
        .unwrap();
        e.supply_care(0, 10_000, 10_000).unwrap();
        e.research(0, sid).unwrap();
        e.breed(0, sid).unwrap();
        for c in &mut e.captives {
            c.animal.next_breeding_tick = 20;
        }
        let env = [EcologyPatch::default()];
        e.step(20, &env).unwrap();
        assert_eq!(
            e.clutches
                .iter()
                .filter(|c| c.custody == Custody::Captive(0))
                .map(|c| c.count)
                .sum::<u32>(),
            2
        );
        assert_eq!(e.housing_available(0, 1).unwrap(), 0);
        for tick in (40..=120).step_by(20) {
            e.step(tick, &env).unwrap();
        }
        assert_eq!(e.captives.iter().map(|c| c.animal.count).sum::<u32>(), 4);
        assert_eq!(e.housing_available(0, 1).unwrap(), 0);
        let extra = e.capture(0, sid, 0).unwrap();
        assert!(e.house_captive(extra, 1).is_err());
        let animals = e.animal_total().unwrap();
        let research = e.knowledge[0][sid as usize].study_ticks;
        e.set_housing(0, Vec::new()).unwrap();
        assert!(e.captives.iter().all(|c| c.housing.is_none()));
        e.step(140, &env).unwrap();
        assert_eq!(e.animal_total().unwrap(), animals);
        assert_eq!(e.knowledge[0][sid as usize].study_ticks, research);
        e.validate().unwrap();
    }
    #[test]
    fn a_completed_pen_does_not_give_eggs_flood_immunity() {
        let mut e = Ecology::new(5, 1).unwrap();
        let sid = e.cohorts[0].species_id;
        e.species[sid as usize].habitat = Habitat::Land;
        disable(&mut e.species[sid as usize].traits, 96);
        e.species[sid as usize].reproduction = Reproduction::Eggs;
        e.species[sid as usize].gestation_ticks = 200;
        e.observe(0, 0, true).unwrap();
        let female = e.capture(0, sid, 0).unwrap();
        let male = e.capture(0, sid, 0).unwrap();
        e.set_housing(
            0,
            vec![Housing {
                id: 1,
                patch: 0,
                capacity: 4,
            }],
        )
        .unwrap();
        e.house_captive(female, 1).unwrap();
        e.house_captive(male, 1).unwrap();
        e.supply_care(0, 10_000, 10_000).unwrap();
        e.knowledge[0][sid as usize].study_ticks = 800;
        e.breed(0, sid).unwrap();
        for c in &mut e.captives {
            c.animal.next_breeding_tick = 20;
        }
        e.step(20, &[EcologyPatch::default()]).unwrap();
        assert_eq!(e.housing_available(0, 1).unwrap(), 0);
        let deaths = e.ledger.dead_animals;
        e.step(
            40,
            &[EcologyPatch {
                water_depth_mm: 2_000,
                ..Default::default()
            }],
        )
        .unwrap();
        assert!(e.clutches.iter().all(|c| c.custody != Custody::Captive(0)));
        assert_eq!(e.ledger.dead_animals, deaths + 2);
        assert_eq!(e.housing_available(0, 1).unwrap(), 2);
        e.validate().unwrap();
    }
    #[test]
    fn separate_pens_do_not_allow_remote_breeding() {
        let mut e = Ecology::new(5, 1).unwrap();
        let sid = e.cohorts[0].species_id;
        e.observe(0, 0, true).unwrap();
        let first = e.capture(0, sid, 0).unwrap();
        let second = e.capture(0, sid, 0).unwrap();
        e.set_housing(
            0,
            vec![
                Housing {
                    id: 1,
                    patch: 0,
                    capacity: 8,
                },
                Housing {
                    id: 2,
                    patch: 0,
                    capacity: 8,
                },
            ],
        )
        .unwrap();
        e.house_captive(first, 1).unwrap();
        e.house_captive(second, 2).unwrap();
        e.knowledge[0][sid as usize].study_ticks = 800;
        assert!(e.breed(0, sid).is_err());
        e.house_captive(second, 1).unwrap();
        e.breed(0, sid).unwrap();
        e.validate().unwrap();
    }
    #[test]
    fn hidden_burrow_animals_cannot_be_discovered_captured_or_eaten_through_ground() {
        let mut e = Ecology::new(5, 1).unwrap();
        let sid = e.cohorts[0].species_id;
        for c in &mut e.cohorts {
            c.surface_observable = false;
            c.location_height_mm = -1000;
        }
        e.observe(0, 0, true).unwrap();
        assert!(e.observed_inspect(0, sid).is_none());
        assert!(e.capture(0, sid, 0).is_err());
        let before = e.ledger.dead_animals;
        let mut predator = e.cohorts.last().unwrap().clone();
        predator.species_id = 12;
        predator.energy = 0;
        e.species[12].food_species = vec![sid];
        e.feed_wild(&mut predator, &e.species[12].clone()).unwrap();
        assert_eq!(e.ledger.dead_animals, before);
    }
    #[test]
    fn materialization_preserves_finite_pollen_seeds_sperm_and_counts() {
        let mut e = Ecology::new(8, 1).unwrap();
        e.cohorts[0].energy -= 5;
        e.cohorts[0].pollen = Some(PollenTicket {
            species_id: 0,
            source_patch: 0,
            units: 2,
            expires_tick: 100,
        });
        e.cohorts[0].carried_seeds.push(SeedTicket {
            species_id: 0,
            units: 3,
            due_tick: 80,
        });
        e.cohorts[0].sperm = Some(SpermTicket {
            donor_species: e.cohorts[0].species_id,
            doses: 4,
            expires_tick: 300,
        });
        rebase(&mut e);
        let owner = e.cohorts[0].id;
        let total = e.material_total().unwrap();
        let count = e.animal_total().unwrap();
        let id = e.materialize_one(owner).unwrap();
        assert_ne!(owner, id);
        assert_eq!(e.material_total().unwrap(), total);
        assert_eq!(e.animal_total().unwrap(), count);
        let doses = e
            .cohorts
            .iter()
            .filter_map(|c| c.sperm.as_ref())
            .map(|s| s.doses)
            .sum::<u32>();
        assert_eq!(doses, 4);
        e.validate().unwrap();
    }
    #[test]
    fn founding_habitat_acceptance_relocates_only_existing_counts_once() {
        let mut e = Ecology::new(78421, 24).unwrap();
        e.set_patch_width(6).unwrap();
        let environment = vec![EcologyPatch::default(); 24];
        let before = (
            e.animal_total().unwrap(),
            e.material_total().unwrap(),
            e.water_total().unwrap(),
        );
        e.accept_initial_habitat(&environment).unwrap();
        assert_eq!(
            before,
            (
                e.animal_total().unwrap(),
                e.material_total().unwrap(),
                e.water_total().unwrap()
            )
        );
        assert!(e.generation_accepted);
        assert!(e.cohorts.iter().all(|c| Ecology::habitat_usable(
            &e.species[c.species_id as usize],
            c.stage,
            &environment[c.patch as usize]
        )));
        assert!(e.accept_initial_habitat(&environment).is_err());
        e.step(20, &environment).unwrap();
        assert!(e.accept_initial_habitat(&environment).is_err());
        e.validate().unwrap();
    }
    #[test]
    fn thirst_search_respects_actual_border_masks_and_never_drinks_remote_stock() {
        let mut e = Ecology::new(19, 3).unwrap();
        let c = e.cohorts[0].clone();
        let mut s = e.species[c.species_id as usize].clone();
        s.habitat = Habitat::Land;
        s.locomotion = Locomotion::Walking;
        e.root_zones[0].shallow_water = 0;
        e.root_zones[1].shallow_water = 0;
        let mut environment = vec![EcologyPatch::default(); 3];
        environment[1].land_connections = 1;
        environment[2].land_connections = 0;
        assert_eq!(e.seek_destination(&c, &s, &environment, 1), None);
        environment[1].land_connections = 3;
        environment[2].land_connections = 1;
        let water = e.root_zones[2].shallow_water;
        assert_eq!(e.seek_destination(&c, &s, &environment, 1), Some(1));
        assert_eq!(e.root_zones[2].shallow_water, water);
        assert_eq!(e.root_zones[0].shallow_water, 0);
    }
    #[test]
    fn aggregate_shelter_cannot_authorize_breeding_and_split_does_not_clone_claim() {
        let mut e = Ecology::new(8, 1).unwrap();
        e.cohorts.retain(|c| c.species_id < 12);
        let sid = e.cohorts[0].species_id as usize;
        e.species[sid].traits = TraitMask::default();
        e.species[sid].traits.insert(64);
        e.species[sid].traits.insert(65);
        e.species[sid].preferred_season = 0;
        e.species[sid].reproduction = Reproduction::Eggs;
        e.cohorts[0].next_breeding_tick = 0;
        e.last_tick = 20;
        let environment = [EcologyPatch {
            shelter_capacity: 1000,
            ..Default::default()
        }];
        rebase(&mut e);
        e.wild_reproduction(&environment).unwrap();
        assert!(e.clutches.is_empty());
        let mother = e.cohorts[0].id;
        e.set_breeding_refuge(
            mother,
            Some(BreedingRefuge {
                den_id: Some(8),
                den_slots: 12,
                nest_id: Some(9),
                nest_slots: 2,
                accessible: true,
            }),
        )
        .unwrap();
        e.wild_reproduction(&environment).unwrap();
        assert_eq!(e.clutches.len(), 1);
        assert_eq!(e.clutches[0].mother_id, mother);
        let single = e.materialize_one(mother).unwrap();
        assert!(
            e.cohorts
                .iter()
                .find(|c| c.id == single)
                .unwrap()
                .breeding_refuge
                .is_none()
        );
        assert!(
            e.cohorts
                .iter()
                .find(|c| c.id == mother)
                .unwrap()
                .breeding_refuge
                .is_some()
        );
        e.validate().unwrap();
    }
    #[test]
    fn migration_moves_once_and_transit_seed_keeps_species_viable() {
        let mut e = Ecology::new(3, 2).unwrap();
        let mut c = e.cohorts.remove(0);
        c.species_id = 6;
        c.count = 1;
        c.energy = 1000;
        c.hydration = 1000;
        c.age_ticks = 100;
        c.stage = Stage::Adult;
        e.species[6].traits.insert(61);
        e.species[6].preferred_season = 1;
        e.cohorts = vec![c.clone()];
        e.last_tick = 17_980;
        let environment = [
            EcologyPatch {
                moisture_permille: 200,
                ..Default::default()
            },
            EcologyPatch {
                moisture_permille: 900,
                ..Default::default()
            },
        ];
        // The destination must contain actual reachable food, not only a
        // favorable seasonal score.
        e.species[6].food_species = (0..6).collect();
        rebase(&mut e);
        e.step(18_000, &environment).unwrap();
        assert_eq!(e.transits.len(), 1);
        assert_eq!(e.transits[0].organism.age_ticks, 120);
        e.step(18_020, &environment).unwrap();
        assert_eq!(e.transits[0].organism.age_ticks, 140);
        assert_eq!(
            e.animal_total().unwrap() + e.ledger.dead_animals,
            e.ledger.initial_animals + e.ledger.conceived_animals
        );
        e.plants.retain(|p| p.species_id != 0);
        e.seeds.retain(|s| s.species_id != 0);
        e.transits[0].organism.carried_seeds.push(SeedTicket {
            species_id: 0,
            units: 1,
            due_tick: 19_000,
        });
        assert!(e.species_alive(0));
    }
    #[test]
    fn dormancy_spends_reserves_and_flooding_removes_its_protection() {
        let mut e = Ecology::new(5, 1).unwrap();
        let mut c = e.cohorts[0].clone();
        c.species_id = 6;
        c.count = 1;
        c.energy = 200;
        c.hydration = 80;
        c.breeding_refuge = Some(BreedingRefuge {
            den_id: Some(9),
            den_slots: 1,
            nest_id: None,
            nest_slots: 0,
            accessible: true,
        });
        e.species[6].traits.insert(72);
        e.last_tick = 20;
        let env = EcologyPatch {
            temperature_milli_c: 10_000,
            shelter_capacity: 1,
            ..Default::default()
        };
        let before = c.energy;
        e.advance_animal(&mut c, &env, false).unwrap();
        assert!(c.dormant_until > 20);
        assert!(c.energy <= before);
        // Consume one real due dormant basal interval; a longer period reduces
        // work but never creates a cost-free dormant animal.
        c.age_ticks = e.species[6].basal_period(c.stage) * 5 - ECOLOGY_INTERVAL;
        e.last_tick = 40;
        e.advance_animal(&mut c, &env, false).unwrap();
        assert!(c.energy < before);
        e.last_tick = 60;
        e.advance_animal(
            &mut c,
            &EcologyPatch {
                water_depth_mm: 1000,
                ..env
            },
            false,
        )
        .unwrap();
        assert_eq!(c.dormant_until, 0);
        assert!(c.health_permille < 1000);
    }
    #[test]
    fn dehydration_tolerance_changes_damage_but_does_not_remove_water_need() {
        let mut e = Ecology::new(2, 1).unwrap();
        e.root_zones[0].shallow_water = 0;
        let mut a = e.cohorts[0].clone();
        a.species_id = 6;
        a.count = 1;
        a.energy = 200;
        a.hydration = 7;
        e.species[6].traits.insert(75);
        let mut b = a.clone();
        e.last_tick = 20;
        let env = EcologyPatch::default();
        e.advance_animal(&mut a, &env, true).unwrap();
        disable(&mut e.species[6].traits, 75);
        e.advance_animal(&mut b, &env, true).unwrap();
        assert!(a.health_permille > b.health_permille);
        for tick in (40..=2400).step_by(20) {
            e.last_tick = tick;
            e.advance_animal(&mut a, &env, true).unwrap();
            if a.count == 0 {
                break;
            }
        }
        assert_eq!(a.count, 0);
    }
    #[test]
    fn deep_roots_access_only_finite_deep_store_and_flood_tolerance_is_limited() {
        let mut deep = plant_world(0);
        deep.root_zones[0].shallow_water = 0;
        deep.root_zones[0].deep_water = 10;
        let mut ordinary = deep.clone();
        disable(&mut ordinary.species[0].traits, 121);
        deep.last_tick = 20;
        ordinary.last_tick = 20;
        let env = [EcologyPatch::default()];
        deep.advance_plants(&env).unwrap();
        ordinary.advance_plants(&env).unwrap();
        assert!(deep.root_zones[0].deep_water < 10);
        assert_eq!(ordinary.root_zones[0].deep_water, 10);
        let mut tolerant = plant_world(1);
        let mut normal = tolerant.clone();
        disable(&mut normal.species[1].traits, 125);
        let flooded = [EcologyPatch {
            water_depth_mm: 500,
            ..Default::default()
        }];
        for tick in (20..=500).step_by(20) {
            tolerant.last_tick = tick;
            normal.last_tick = tick;
            tolerant.advance_plants(&flooded).unwrap();
            normal.advance_plants(&flooded).unwrap();
        }
        assert!(tolerant.plants[0].shoots > normal.plants[0].shoots);
    }
    #[test]
    fn pioneer_seed_bank_resprouting_and_buoyancy_use_real_viable_stocks() {
        let mut pioneer = plant_world(1);
        pioneer.plants[0].shoots = 2000;
        pioneer.plants[0].leaves = 100;
        pioneer.refresh_fields();
        pioneer.seeds.push(SeedStock {
            species_id: 1,
            patch: 0,
            units: 1,
            expires_tick: 1000,
            advection_remainder: 0,
            afloat: false,
        });
        let mut ordinary = pioneer.clone();
        disable(&mut ordinary.species[1].traits, 128);
        let disturbed = [EcologyPatch {
            disturbance: 1,
            ..Default::default()
        }];
        let before = pioneer.plants.iter().map(|p| p.individuals).sum::<u32>();
        pioneer.advance_seeds(&disturbed).unwrap();
        ordinary.advance_seeds(&disturbed).unwrap();
        assert_eq!(
            pioneer.plants.iter().map(|p| p.individuals).sum::<u32>(),
            before + 1
        );
        assert_eq!(
            ordinary.plants.iter().map(|p| p.individuals).sum::<u32>(),
            before
        );
        let mut roots = plant_world(0);
        roots.plants[0].shoots = 0;
        let mut none = roots.clone();
        disable(&mut none.species[0].traits, 129);
        roots.last_tick = 20;
        none.last_tick = 20;
        roots.advance_plants(&[EcologyPatch::default()]).unwrap();
        none.advance_plants(&[EcologyPatch::default()]).unwrap();
        assert!(roots.plants[0].shoots > none.plants[0].shoots);
        let mut bank = plant_world(0);
        let mut short = bank.clone();
        disable(&mut short.species[0].traits, 132);
        bank.last_tick = 800;
        short.last_tick = 800;
        bank.advance_seeds(&[EcologyPatch::default()]).unwrap();
        short.advance_seeds(&[EcologyPatch::default()]).unwrap();
        assert_eq!(bank.plants[0].seeds, short.plants[0].seeds + 1);
        let mut water = Ecology::new(8, 2).unwrap();
        water.seeds.push(SeedStock {
            species_id: 1,
            patch: 0,
            units: 7,
            expires_tick: 1000,
            advection_remainder: 0,
            afloat: true,
        });
        let env = [
            EcologyPatch {
                water_depth_mm: 100,
                flow_to: Some(1),
                flow_fraction_permille: 1000,
                ..Default::default()
            },
            EcologyPatch::default(),
        ];
        water.advance_seeds(&env).unwrap();
        assert_eq!(water.seeds[0].patch, 1);
        assert_eq!(water.seeds[0].units, 7);
    }
    #[test]
    fn buoyant_seeds_follow_actual_flux_fraction_and_preserve_fractional_credit_without_double_hops()
     {
        let mut e = Ecology::new(8, 3).unwrap();
        e.species[1].traits.insert(143);
        e.seeds.push(SeedStock {
            species_id: 1,
            patch: 0,
            units: 3,
            expires_tick: 1000,
            afloat: true,
            advection_remainder: 0,
        });
        rebase(&mut e);
        let mut env = vec![
            EcologyPatch {
                water_depth_mm: 100,
                ..Default::default()
            };
            3
        ];
        env[0].flow_to = Some(1);
        env[1].flow_to = Some(2);
        env[1].flow_fraction_permille = 1000;
        e.last_tick = 20;
        e.advance_seeds(&env).unwrap();
        assert_eq!(e.seeds[0].patch, 0);
        assert_eq!(e.seeds[0].units, 3);
        env[0].flow_fraction_permille = 333;
        e.last_tick = 40;
        e.advance_seeds(&env).unwrap();
        assert_eq!(e.seeds[0].patch, 0);
        assert_eq!(e.seeds[0].advection_remainder, 999);
        let saved = serde_json::to_vec(&e).unwrap();
        let mut restored: Ecology = serde_json::from_slice(&saved).unwrap();
        e.last_tick = 60;
        restored.last_tick = 60;
        e.advance_seeds(&env).unwrap();
        restored.advance_seeds(&env).unwrap();
        assert_eq!(
            serde_json::to_vec(&e).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
        assert_eq!(e.seeds.iter().map(|s| s.units).sum::<u64>(), 3);
        assert!(e.seeds.iter().any(|s| s.patch == 1 && s.units == 1));
        assert!(!e.seeds.iter().any(|s| s.patch == 2));
        e.validate().unwrap();
    }
    #[test]
    fn pollination_and_internal_seed_transport_have_finite_pollen_rewards_and_delay() {
        let mut e = plant_world(4);
        e.plants[0].serviced_pollen = 0;
        e.last_tick = 100;
        let before = e.plants[0].seeds;
        e.advance_plants(&[EcologyPatch::default()]).unwrap();
        assert_eq!(e.plants[0].seeds, before);
        e.plants[0].serviced_pollen = 1;
        e.advance_plants(&[EcologyPatch::default()]).unwrap();
        assert_eq!(e.plants[0].seeds, before + 1);
        let mut c = Ecology::new(5, 1).unwrap().cohorts[0].clone();
        c.species_id = 10;
        c.count = 1;
        c.energy = 0;
        c.pollen = Some(PollenTicket {
            species_id: 4,
            source_patch: 9,
            units: 2,
            expires_tick: 1000,
        });
        e.species[10].traits.insert(106);
        e.species[10].food_species = vec![4];
        e.feed_wild(&mut c, &e.species[10].clone()).unwrap();
        assert!(e.plants[0].serviced_pollen >= 2);
        assert!(c.energy > 0);
        c.species_id = 7;
        c.location_height_mm = 2000;
        c.energy = 0;
        e.species[7].food_species = vec![4];
        e.species[7].traits.insert(107);
        let seeds = e.plants[0].seeds;
        e.feed_wild(&mut c, &e.species[7].clone()).unwrap();
        assert!(c.carried_seeds.iter().map(|s| s.units).sum::<u64>() > 0);
        assert!(e.plants[0].seeds < seeds);
        assert!(c.carried_seeds[0].due_tick > e.last_tick);
    }
    #[test]
    fn canopy_cover_and_nutrient_rich_litter_follow_living_stocks_and_cost_resources() {
        let mut e = plant_world(0);
        e.species[0].traits.insert(156);
        e.refresh_fields();
        let shade = e.canopy_permille(0);
        let cover = e.ground_cover_permille(0);
        let water = e.root_zones[0].shallow_water;
        e.last_tick = 100;
        e.advance_plants(&[EcologyPatch::default()]).unwrap();
        assert!(e.root_zones[0].shallow_water < water);
        e.damage_plants(0, u64::MAX, true).unwrap();
        assert!(e.canopy_permille(0) < shade);
        assert!(e.ground_cover_permille(0) < cover);
        assert_eq!(e.plants[0].individuals, 0);
        assert!(e.root_zones[0].litter > 0);
        let mut rich = plant_world(0);
        rich.root_zones[0].litter = 100;
        let mut plain = rich.clone();
        disable(&mut plain.species[0].traits, 159);
        rich.refresh_fields();
        plain.refresh_fields();
        rich.last_tick = 20;
        plain.last_tick = 20;
        rich.advance_plants(&[EcologyPatch::default()]).unwrap();
        plain.advance_plants(&[EcologyPatch::default()]).unwrap();
        assert!(rich.root_zones[0].nutrients > plain.root_zones[0].nutrients);
    }
    #[test]
    fn incubation_role_commits_once_and_guarding_has_a_finite_cost() {
        let mut e = Ecology::new(7, 1).unwrap();
        let mother = e.cohorts[0].id;
        let mut c = e.cohorts[0].clone();
        c.species_id = 9;
        c.count = 1;
        c.energy = 100;
        let mother = e.cohorts[0].id.max(mother);
        e.species[9].traits.insert(86);
        e.species[9].traits.insert(93);
        e.clutches.push(Clutch {
            id: 900,
            species_id: 9,
            count: 2,
            mother_id: mother,
            custody: Custody::Wild(0),
            housing: None,
            patch: 0,
            hatch_tick: 40,
            material: 80,
            water: 8,
            temperature_sum: 0,
            temperature_samples: 0,
            guarded: true,
            reproduction: Reproduction::Eggs,
        });
        rebase(&mut e);
        e.last_tick = 20;
        let before = e.cohorts[0].energy;
        let env = [EcologyPatch {
            temperature_milli_c: 35_000,
            ..Default::default()
        }];
        let mut effects = Vec::new();
        e.advance_clutches(&env, &mut effects).unwrap();
        assert!(e.cohorts[0].energy < before);
        let saved = serde_json::to_vec(&e).unwrap();
        let mut load: Ecology = serde_json::from_slice(&saved).unwrap();
        e.last_tick = 40;
        load.last_tick = 40;
        e.advance_clutches(&env, &mut effects).unwrap();
        load.advance_clutches(&env, &mut Vec::new()).unwrap();
        assert_eq!(
            serde_json::to_vec(&e).unwrap(),
            serde_json::to_vec(&load).unwrap()
        );
        assert!(
            e.cohorts
                .iter()
                .filter(|c| c.species_id == 9 && c.stage == Stage::Juvenile)
                .all(|c| c.sex == Sex::Male)
        );
    }
    #[test]
    fn metamorphosis_pays_body_growth_and_milk_moves_only_from_actual_parent() {
        let mut e = Ecology::new(8, 1).unwrap();
        let mut young = e.cohorts[0].clone();
        young.species_id = 10;
        e.species[10].traits.insert(96);
        e.species[10].juvenile_habitat = Habitat::Water;
        young.count = 1;
        young.stage = Stage::Juvenile;
        young.age_ticks = e.species[10].maturity_ticks;
        young.body_units = NEW_BODY;
        young.energy = 200;
        young.hydration = 100;
        assert_eq!(
            e.species[10].locomotion_for(Stage::Juvenile),
            Locomotion::Swimming
        );
        assert_eq!(e.species[10].habitat_for(Stage::Juvenile), Habitat::Water);
        assert_eq!(
            e.species[10].locomotion_for(Stage::Adult),
            Locomotion::Flying
        );
        e.root_zones[0].litter = 20;
        let mut larva = young.clone();
        larva.energy = 0;
        let nectar = e.plants.iter().map(|p| p.nectar).sum::<u64>();
        e.feed_wild(&mut larva, &e.species[10].clone()).unwrap();
        assert!(larva.energy > 0);
        assert!(e.root_zones[0].litter < 20);
        assert_eq!(e.plants.iter().map(|p| p.nectar).sum::<u64>(), nectar);
        assert!(larva.pollen.is_none());
        let before = young.energy;
        e.last_tick = 20;
        e.advance_animal(
            &mut young,
            &EcologyPatch {
                water_depth_mm: 100,
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert_eq!(young.stage, Stage::Adult);
        assert_eq!(young.body_units, ADULT_BODY);
        assert!(young.energy <= before - (ADULT_BODY - NEW_BODY));
        assert_eq!(young.location_height_mm, 2000);
        let parent = e.cohorts[0].id;
        e.cohorts[0].milk = 10;
        young.id = e.alloc_id().unwrap();
        young.species_id = e.cohorts[0].species_id;
        young.stage = Stage::Juvenile;
        young.parent_id = Some(parent);
        young.patch = e.cohorts[0].patch;
        young.energy = 0;
        e.cohorts.push(young);
        let material = e.material_total().unwrap();
        e.nurse_young().unwrap();
        assert_eq!(e.cohorts.last().unwrap().energy, 4);
        assert_eq!(e.cohorts[0].milk, 6);
        assert_eq!(e.material_total().unwrap(), material);
    }
    #[test]
    fn stored_sperm_is_finite_and_pregnancy_follows_capture_release_without_duplication() {
        let mut e = Ecology::new(7, 2).unwrap();
        let mut female = e.cohorts[0].clone();
        female.species_id = 6;
        female.count = 1;
        female.sex = Sex::Female;
        female.energy = 200;
        female.hydration = 80;
        female.next_breeding_tick = 20;
        female.sperm = Some(SpermTicket {
            donor_species: 6,
            doses: 1,
            expires_tick: 100,
        });
        e.cohorts = vec![female];
        e.species[6].traits.insert(84);
        disable(&mut e.species[6].traits, 64);
        e.species[6].preferred_season = 0;
        e.species[6].reproduction = Reproduction::LiveBirth;
        e.last_tick = 20;
        rebase(&mut e);
        e.wild_reproduction(&[EcologyPatch::default(), EcologyPatch::default()])
            .unwrap();
        assert_eq!(e.clutches.len(), 1);
        assert_eq!(e.cohorts[0].sperm.as_ref().unwrap().doses, 0);
        e.cohorts[0].next_breeding_tick = 20;
        e.wild_reproduction(&[EcologyPatch::default(), EcologyPatch::default()])
            .unwrap();
        assert_eq!(e.clutches.len(), 1);
        let organisms = e.animal_total().unwrap();
        e.observe(0, 0, true).unwrap();
        let new_id = e.capture(0, 6, 0).unwrap();
        assert_eq!(e.clutches[0].mother_id, new_id);
        assert_eq!(e.clutches[0].custody, Custody::Captive(0));
        e.observe(0, 1, true).unwrap();
        e.release(0, 6, 1).unwrap();
        assert_eq!(e.clutches[0].patch, 1);
        assert_eq!(e.clutches[0].custody, Custody::Wild(1));
        assert_eq!(e.animal_total().unwrap(), organisms);
        e.validate().unwrap();
        e.clutches[0].hatch_tick = 40;
        e.last_tick = 40;
        e.advance_clutches(
            &[EcologyPatch::default(), EcologyPatch::default()],
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(e.animal_total().unwrap(), organisms);
        assert!(
            e.cohorts
                .iter()
                .filter(|c| c.stage == Stage::Juvenile)
                .all(|c| c.patch == 1)
        );
        e.validate().unwrap();
    }
    #[test]
    fn concurrent_live_gestations_need_distinct_real_mothers_and_follow_materialization() {
        let mut e = Ecology::new(7, 1).unwrap();
        let mut mothers = e.cohorts[0].clone();
        mothers.species_id = 6;
        mothers.sex = Sex::Female;
        mothers.count = 2;
        mothers.body_units = 2 * ADULT_BODY;
        mothers.energy = 400;
        mothers.hydration = 160;
        mothers.next_breeding_tick = 20;
        mothers.sperm = Some(SpermTicket {
            donor_species: 6,
            doses: 4,
            expires_tick: 2000,
        });
        let owner = mothers.id;
        e.cohorts = vec![mothers];
        e.species[6].reproduction = Reproduction::LiveBirth;
        e.species[6].preferred_season = 0;
        disable(&mut e.species[6].traits, 64);
        e.last_tick = 20;
        rebase(&mut e);
        let env = [EcologyPatch::default()];
        e.wild_reproduction(&env).unwrap();
        e.cohorts[0].next_breeding_tick = 20;
        e.wild_reproduction(&env).unwrap();
        assert_eq!(e.clutches.len(), 2);
        e.cohorts[0].next_breeding_tick = 20;
        e.wild_reproduction(&env).unwrap();
        assert_eq!(e.clutches.len(), 2);
        let detailed = e.materialize_one(owner).unwrap();
        assert_eq!(
            e.clutches.iter().filter(|cl| cl.mother_id == owner).count(),
            1
        );
        assert_eq!(
            e.clutches
                .iter()
                .filter(|cl| cl.mother_id == detailed)
                .count(),
            1
        );
        e.damage_cohort(owner, 1).unwrap();
        e.advance_clutches(&env, &mut Vec::new()).unwrap();
        assert_eq!(e.clutches.len(), 1);
        assert_eq!(e.clutches[0].mother_id, detailed);
        assert_eq!(e.ledger.dead_animals, 3);
        e.validate().unwrap();
    }
    #[test]
    fn malformed_saved_references_and_stock_overflow_return_errors_instead_of_panics() {
        let original = Ecology::new(10, 2).unwrap();
        let mut bad = original.clone();
        bad.plants[0].patch = u32::MAX;
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.cohorts[0].species_id = u32::MAX;
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.cohorts[1].id = bad.cohorts[0].id;
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.plants[0].shoots = u64::MAX;
        assert!(bad.validate().is_err());
        let mut bad = original.clone();
        bad.species[6].food_species = vec![12];
        assert!(bad.validate().is_err());
        let mut bad = original;
        bad.patch_width = 3;
        assert!(bad.validate().is_err());
    }
    #[test]
    fn extinction_never_recovers_from_generation_seed_without_surviving_stages() {
        let mut e = plant_world(0);
        e.plants[0].seeds = 0;
        e.damage_plants(0, u64::MAX, true).unwrap();
        rebase(&mut e);
        let mut effects = Vec::new();
        e.detect_extinction(&mut effects).unwrap();
        assert!(e.extinct[0]);
        let env = [EcologyPatch {
            disturbance: 100,
            ..Default::default()
        }];
        for tick in (20..=2000).step_by(20) {
            e.step(tick, &env).unwrap();
        }
        assert_eq!(e.plants[0].individuals, 0);
        assert!(e.extinct[0]);
    }
}
