//! Finite construction, crash salvage, excavation and worker-driven logistics.
//! Rendering is a cache of this state; no visual instance owns materials.
use crate::{
    river::RiverFixture,
    terrain::{CELL_METERS, CH, CW, HH, Terrain, WW},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const WORK_REACH_METERS: f32 = 22.;
pub const TEAM_BASES: [[f32; 2]; 2] = [[424., 1640.], [4376., 1712.]];
const CELL_AREA: u64 = (CELL_METERS * CELL_METERS) as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resources {
    pub metal: u64,
    pub timber: u64,
    pub rations: u64,
    pub power: u64,
}
impl Resources {
    pub const fn new(metal: u64, timber: u64, rations: u64, power: u64) -> Self {
        Self {
            metal,
            timber,
            rations,
            power,
        }
    }
    pub fn checked_add(self, b: Self) -> Result<Self, String> {
        Ok(Self {
            metal: self.metal.checked_add(b.metal).ok_or("Metal overflow")?,
            timber: self.timber.checked_add(b.timber).ok_or("Timber overflow")?,
            rations: self
                .rations
                .checked_add(b.rations)
                .ok_or("Rations overflow")?,
            power: self.power.checked_add(b.power).ok_or("Power overflow")?,
        })
    }
    pub fn checked_sub(self, b: Self) -> Result<Self, String> {
        Ok(Self {
            metal: self
                .metal
                .checked_sub(b.metal)
                .ok_or("Insufficient metal")?,
            timber: self
                .timber
                .checked_sub(b.timber)
                .ok_or("Insufficient timber")?,
            rations: self
                .rations
                .checked_sub(b.rations)
                .ok_or("Insufficient rations")?,
            power: self
                .power
                .checked_sub(b.power)
                .ok_or("Insufficient power")?,
        })
    }
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }
    fn values(self) -> [u64; 4] {
        [self.metal, self.timber, self.rations, self.power]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildKind {
    Road,
    Trench,
    Foundation,
    Smooth,
    Depot,
    Mine,
    Mill,
    Barracks,
    Solar,
    Farm,
    Camera,
    Radar,
    Wall,
    Turret,
    Dam,
    Bridge,
    Dock,
    Pen,
    Research,
    FieldCamera,
    CameraTower,
    MountedCamera,
}
impl BuildKind {
    /// Every construction tool, in the existing player-facing tool order.
    pub const ALL: [Self; 22] = [
        Self::Road,
        Self::Trench,
        Self::Foundation,
        Self::Smooth,
        Self::Depot,
        Self::Mine,
        Self::Mill,
        Self::Barracks,
        Self::Solar,
        Self::Farm,
        Self::Camera,
        Self::Radar,
        Self::Wall,
        Self::Turret,
        Self::Dam,
        Self::Bridge,
        Self::Dock,
        Self::Pen,
        Self::Research,
        Self::FieldCamera,
        Self::CameraTower,
        Self::MountedCamera,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::Trench => "trench",
            Self::Foundation => "foundation",
            Self::Smooth => "smooth",
            Self::Depot => "depot",
            Self::Mine => "mine",
            Self::Mill => "mill",
            Self::Barracks => "barracks",
            Self::Solar => "solar",
            Self::Farm => "farm",
            Self::Camera => "Camera pole",
            Self::FieldCamera => "Field camera",
            Self::CameraTower => "Camera tower",
            Self::MountedCamera => "Mounted camera",
            Self::Radar => "radar",
            Self::Wall => "wall",
            Self::Turret => "turret",
            Self::Dam => "dam",
            Self::Bridge => "bridge",
            Self::Dock => "dock",
            Self::Pen => "pen",
            Self::Research => "research",
        }
    }
    pub fn atlas_kind(self) -> &'static str {
        match self {
            Self::Road => "road",
            Self::Trench => "trench",
            Self::Depot => "depot",
            Self::Mine => "mine",
            Self::Mill => "mill",
            Self::Barracks => "barracks",
            Self::Solar => "solar",
            Self::Farm => "farm",
            Self::Camera | Self::FieldCamera | Self::CameraTower | Self::MountedCamera => "camera",
            Self::Radar => "radar",
            Self::Wall | Self::Dam => "wall",
            Self::Turret => "turret",
            // These tools explicitly reuse existing grown depot artwork.
            Self::Foundation
            | Self::Smooth
            | Self::Pen
            | Self::Research
            | Self::Bridge
            | Self::Dock => "depot",
        }
    }
    /// The legacy Camera identity remains the original 8m pole in old saves.
    pub fn camera_spec(self) -> Option<(u32,f32)> {
        match self { Self::Camera=>Some((8_000,420.)),Self::FieldCamera=>Some((2_000,240.)),
            Self::CameraTower=>Some((18_000,720.)),Self::MountedCamera=>Some((7_000,360.)),_=>None }
    }
    pub fn is_terrain(self) -> bool {
        matches!(
            self,
            Self::Road | Self::Trench | Self::Foundation | Self::Smooth
        )
    }
    pub fn solid(self) -> bool {
        !matches!(
            self,
            Self::Road
                | Self::Trench
                | Self::Foundation
                | Self::Smooth
                | Self::Bridge
                | Self::Dock
                | Self::Farm
                | Self::Solar
                | Self::MountedCamera
        )
    }
    pub fn base_cost(self) -> Resources {
        match self {
            Self::Road => Resources::new(6, 0, 0, 2),
            Self::Trench | Self::Smooth => Resources::new(0, 0, 0, 4),
            Self::Foundation => Resources::new(12, 0, 0, 8),
            Self::Depot => Resources::new(180, 100, 0, 20),
            Self::Mine => Resources::new(180, 80, 0, 30),
            Self::Mill => Resources::new(120, 100, 0, 20),
            Self::Barracks => Resources::new(220, 180, 0, 30),
            Self::Solar => Resources::new(160, 0, 0, 0),
            Self::Farm => Resources::new(40, 40, 20, 10),
            Self::Camera => Resources::new(60, 10, 0, 20),
            Self::FieldCamera => Resources::new(35, 0, 0, 10),
            Self::CameraTower => Resources::new(160, 100, 0, 40),
            Self::MountedCamera => Resources::new(45, 0, 0, 15),
            Self::Radar => Resources::new(200, 40, 0, 80),
            Self::Wall => Resources::new(100, 40, 0, 10),
            Self::Turret => Resources::new(220, 40, 0, 30),
            Self::Dam => Resources::new(80, 300, 0, 40),
            Self::Bridge => Resources::new(220, 240, 0, 40),
            Self::Dock => Resources::new(200, 180, 0, 30),
            Self::Pen => Resources::new(100, 160, 0, 20),
            Self::Research => Resources::new(260, 120, 0, 100),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BuildOrder {
    pub team: u8,
    pub kind: BuildKind,
    pub position: [f32; 2],
    pub end: [f32; 2],
}
impl BuildOrder {
    pub fn at(team: u8, kind: BuildKind, position: [f32; 2]) -> Self {
        Self {
            team,
            kind,
            position,
            end: position,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobPhase {
    Pickup,
    Hauling,
    Working,
    Complete,
    Cancelled,
    Blocked,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TerrainEdit {
    pub cell: usize,
    pub before_mm: i32,
    pub target_mm: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildJob {
    pub id: u64,
    pub order: BuildOrder,
    pub materials: Resources,
    pub phase: JobPhase,
    pub worker: Option<u32>,
    pub delivered: bool,
    pub pickup_position: [f32; 2],
    pub work_position: [f32; 2],
    pub work_done: u32,
    pub work_required: u32,
    pub terrain_edits: Vec<TerrainEdit>,
    pub expected_terrain_revision: u64,
    pub blocked_reason: Option<String>,
    pub material_consumed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub id: u64,
    pub kind: BuildKind,
    pub team: u8,
    pub position: [f32; 2],
    pub end: [f32; 2],
    pub health: u32,
    pub max_health: u32,
    pub complete: bool,
    pub size_meters: f32,
    pub deck_height_mm: Option<i32>,
    pub draft_clearance_mm: u32,
    pub load_limit_kg: u32,
    pub construction_cost: Resources,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct WorkerState {
    pub id: u32,
    pub team: u8,
    pub position: [f32; 2],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkPurpose {
    Pickup,
    Deliver,
    Build,
    Mine,
    ReturnCargo,
    RecoverMaterials,
    ReturnMaterials,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct WorkerTask {
    pub worker_id: u32,
    pub destination: [f32; 2],
    pub job_id: Option<u64>,
    pub purpose: WorkPurpose,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OreDeposit {
    pub id: u32,
    pub position: [f32; 2],
    pub remaining: u64,
    pub initial: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OreCargo {
    pub team: u8,
    pub deposit_id: u32,
    pub amount: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaterialCrate {
    pub id: u64,
    pub team: u8,
    pub position: [f32; 2],
    pub resources: Resources,
    pub carrier: Option<u32>,
    pub reserved_worker: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ConstructionEvent {
    Completed {
        job: u64,
        building: Option<u64>,
        team: u8,
        kind: BuildKind,
        position: [f32; 2],
    },
    HabitatCleared {
        team: u8,
        cell: usize,
        foliage_units: u32,
    },
    TerrainChanged {
        team: u8,
        cells: Vec<usize>,
        cut_liters: u64,
        fill_liters: u64,
    },
    Mined {
        team: u8,
        deposit: u32,
        amount: u64,
    },
    Blocked {
        job: u64,
        reason: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstructionWorld {
    pub stores: [Resources; 2],
    pub initial_salvage: [Resources; 2],
    pub bases: [[f32; 2]; 2],
    pub jobs: Vec<BuildJob>,
    pub buildings: Vec<Building>,
    pub deposits: Vec<OreDeposit>,
    pub cargo: BTreeMap<u32, OreCargo>,
    pub crates: Vec<MaterialCrate>,
    pub spoil_liters: [u64; 2],
    pub cut_liters: [u64; 2],
    pub fill_liters: [u64; 2],
    pub consumed: [Resources; 2],
    pub external_inputs: [Resources; 2],
    pub last_tick: u64,
    pub next_id: u64,
    pub navigation_revision: u64,
    harvest_assignments: BTreeMap<u32, u32>,
    material_assignments: BTreeMap<u32, u64>,
    last_worker_tick: BTreeMap<u32, u64>,
}
impl ConstructionWorld {
    pub fn new(seed: u32) -> Self {
        let salvage = Resources::new(12_000, 10_000, 4_000, 20_000);
        let mut deposits = Vec::new();
        for (id, base) in TEAM_BASES.iter().enumerate() {
            let jitter = ((seed
                .wrapping_mul(1664525)
                .wrapping_add(id as u32 * 1013904223)
                >> 16)
                % 101) as f32;
            deposits.push(OreDeposit {
                id: id as u32,
                position: [
                    base[0] + if id == 0 { 180. } else { -180. },
                    base[1] - 140. - jitter,
                ],
                remaining: 50_000,
                initial: 50_000,
            });
        }
        Self {
            stores: [salvage; 2],
            initial_salvage: [salvage; 2],
            bases: TEAM_BASES,
            jobs: Vec::new(),
            buildings: Vec::new(),
            deposits,
            cargo: BTreeMap::new(),
            crates: Vec::new(),
            spoil_liters: [0; 2],
            cut_liters: [0; 2],
            fill_liters: [0; 2],
            consumed: [Resources::default(); 2],
            external_inputs: [Resources::default(); 2],
            last_tick: 0,
            next_id: 1,
            navigation_revision: 0,
            harvest_assignments: BTreeMap::new(),
            material_assignments: BTreeMap::new(),
            last_worker_tick: BTreeMap::new(),
        }
    }
    fn id(&mut self) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or("Construction ID exhausted")?;
        Ok(id)
    }
    fn validate_position(p: [f32; 2]) -> Result<usize, String> {
        if !p.iter().all(|n| n.is_finite())
            || p[0] < 0.
            || p[1] < 0.
            || p[0] >= WW as f32
            || p[1] >= HH as f32
        {
            return Err("Construction position outside map".into());
        }
        Ok(p[1] as usize / CELL_METERS * CW + p[0] as usize / CELL_METERS)
    }
    fn requires_exterior_access(kind: BuildKind) -> bool {
        kind.solid() || matches!(kind, BuildKind::Bridge | BuildKind::Dock)
    }
    fn work_access(&self, order: BuildOrder, terrain: &Terrain) -> Result<[f32; 2], String> {
        if !Self::requires_exterior_access(order.kind) {
            return Ok(order.position);
        }
        // Keep builders outside the completed56m barracks plus a16m routing
        // cell. They walk here before work; no completion-time teleport/evacuation.
        let mut candidates = [
            [order.position[0] - 48., order.position[1]],
            [order.position[0] + 48., order.position[1]],
            [order.position[0], order.position[1] - 48.],
            [order.position[0], order.position[1] + 48.],
            [order.position[0] - 48., order.position[1] - 48.],
            [order.position[0] - 48., order.position[1] + 48.],
            [order.position[0] + 48., order.position[1] - 48.],
            [order.position[0] + 48., order.position[1] + 48.],
        ];
        let base = self.bases[order.team as usize];
        candidates.sort_by(|a, b| distance(*a, base).total_cmp(&distance(*b, base)));
        for p in candidates {
            let Ok(cell) = Self::validate_position(p) else {
                continue;
            };
            let nx = cell % CW / 2 * 2;
            let ny = cell / CW / 2 * 2;
            let safe = [
                ny * CW + nx,
                ny * CW + nx + 1,
                (ny + 1) * CW + nx,
                (ny + 1) * CW + nx + 1,
            ]
            .iter()
            .all(|&i| {
                terrain.cells.get(i).is_some_and(|c| {
                    let depth = if c.flags & crate::terrain::DYNAMIC_WATER != 0 {
                        c.water as u64
                    } else {
                        c.water as u64 * 10
                    };
                    depth <= 300 && c.object == 0
                })
            });
            if !safe {
                continue;
            }
            if self.buildings.iter().any(|b| {
                b.health > 0
                    && b.kind.solid()
                    && distance(b.position, p) < (b.size_meters * 0.5 + 20.).max(32.)
            }) {
                continue;
            }
            if self.jobs.iter().any(|j| {
                j.order.kind.solid()
                    && !matches!(j.phase, JobPhase::Complete | JobPhase::Cancelled)
                    && distance(j.order.position, p) < 48.
            }) {
                continue;
            }
            return Ok(p);
        }
        Err("No dry, unobstructed exterior construction access; choose another site".into())
    }
    fn plan(order: BuildOrder, terrain: &Terrain) -> Result<Vec<TerrainEdit>, String> {
        let start = Self::validate_position(order.position)?;
        Self::validate_position(order.end)?;
        let mut edits = Vec::new();
        if order.kind == BuildKind::Trench {
            let len = (order.end[0] - order.position[0]).hypot(order.end[1] - order.position[1]);
            if len > 512. {
                return Err("Split trenches longer than512m into connected orders".into());
            }
            let n = (len / 4.).ceil().max(1.) as usize;
            let mut visited = BTreeSet::new();
            for j in 0..=n {
                let f = j as f32 / n as f32;
                let p = [
                    order.position[0] + (order.end[0] - order.position[0]) * f,
                    order.position[1] + (order.end[1] - order.position[1]) * f,
                ];
                let cell = Self::validate_position(p)?;
                if visited.insert(cell) {
                    let before = terrain.elevation_mm(cell).map_err(|e| e.to_string())?;
                    edits.push(TerrainEdit {
                        cell,
                        before_mm: before,
                        target_mm: before.checked_sub(350).ok_or("Trench elevation overflow")?,
                    });
                }
            }
        } else if matches!(order.kind, BuildKind::Foundation | BuildKind::Smooth) {
            let cx = start % CW;
            let cy = start / CW;
            let mut samples = Vec::new();
            for y in cy.saturating_sub(1)..=(cy + 1).min(CH - 1) {
                for x in cx.saturating_sub(1)..=(cx + 1).min(CW - 1) {
                    let cell = y * CW + x;
                    samples.push((cell, terrain.elevation_mm(cell).map_err(|e| e.to_string())?));
                }
            }
            let target =
                (samples.iter().map(|(_, h)| *h as i64).sum::<i64>() / samples.len() as i64) as i32;
            for (cell, before) in samples {
                let height = if order.kind == BuildKind::Smooth {
                    before + (target - before).clamp(-100, 100)
                } else {
                    target
                };
                edits.push(TerrainEdit {
                    cell,
                    before_mm: before,
                    target_mm: height,
                });
            }
        }
        if edits
            .iter()
            .any(|e| !(-1_000_000..=1_000_000).contains(&e.target_mm))
        {
            return Err("Terrain target outside supported elevation".into());
        }
        Ok(edits)
    }
    pub fn mount_host(&self, order: BuildOrder) -> Option<&Building> {
        self.buildings.iter().filter(|b| b.team==order.team && b.health>0 &&
            matches!(b.kind,BuildKind::Wall|BuildKind::Depot|BuildKind::Barracks|BuildKind::Research))
            .find(|b| distance(b.position,order.position) <= b.size_meters*0.5+12.)
    }
    pub fn enqueue(&mut self, mut order: BuildOrder, terrain: &Terrain) -> Result<u64, String> {
        if order.kind==BuildKind::MountedCamera {
            let host=self.mount_host(order).ok_or("Mounted camera requires an owned completed wall or building; place near its edge")?;
            // Snap to the closest exterior face, keeping worker access outside.
            let dx=order.position[0]-host.position[0]; let dy=order.position[1]-host.position[1];
            let r=host.size_meters*0.5+8.;
            order.position=if dx.abs()>=dy.abs() {
                [host.position[0]+if dx<0.{-r}else{r},host.position[1]+dy.clamp(-r,r)]
            }else{[host.position[0]+dx.clamp(-r,r),host.position[1]+if dy<0.{-r}else{r}]};
            order.end=order.position;
        }
        if order.team > 1 {
            return Err("Invalid construction faction".into());
        }
        let cell = Self::validate_position(order.position)?;
        Self::validate_position(order.end)?;
        if order.kind == BuildKind::Road
            && (order.end[0] - order.position[0]).hypot(order.end[1] - order.position[1]) > 512.
        {
            return Err("Split roads longer than512m into connected orders".into());
        }
        if order.kind == BuildKind::Dam
            && ((order.position[0] - 2384.).abs() > 96. || (order.position[1] - 964.).abs() > 40.)
        {
            return Err("This small map's dam must cross the managed river at y964m".into());
        }
        if order.kind == BuildKind::Dam
            && self.jobs.iter().any(|j| {
                j.order.kind == BuildKind::Dam
                    && !matches!(j.phase, JobPhase::Cancelled | JobPhase::Blocked)
            })
        {
            return Err("A dam already occupies this channel crossing".into());
        }
        let wet = terrain.cells[cell].water;
        if order.kind.solid() && order.kind != BuildKind::Dam && wet > 300 {
            return Err("Building site is submerged; use a suitable bank/foundation".into());
        }
        if order.kind == BuildKind::Dock && wet == 0 {
            let x = cell % CW;
            let y = cell / CW;
            let near = (y.saturating_sub(3)..=(y + 3).min(CH - 1)).any(|yy| {
                (x.saturating_sub(3)..=(x + 3).min(CW - 1))
                    .any(|xx| terrain.cells[yy * CW + xx].water > 0)
            });
            if !near {
                return Err("Dock requires a shoreline".into());
            }
        }
        if self.buildings.iter().any(|b| {
            b.health > 0
                && b.kind.solid()
                && order.kind.solid()
                && distance(b.position, order.position) < (b.size_meters + 32.) * 0.5
        }) {
            return Err("Building footprint overlaps a structure".into());
        }
        if order.kind.solid()
            && self.jobs.iter().any(|j| {
                j.order.kind.solid()
                    && !matches!(j.phase, JobPhase::Complete | JobPhase::Cancelled)
                    && distance(j.order.position, order.position) < 32.
            })
        {
            return Err("Building footprint overlaps an existing construction order".into());
        }
        let edits = Self::plan(order, terrain)?;
        let units = if matches!(order.kind, BuildKind::Road | BuildKind::Trench) {
            ((order.end[0] - order.position[0]).hypot(order.end[1] - order.position[1]) / 16.)
                .ceil()
                .max(1.) as u64
        } else {
            1
        };
        let base = order.kind.base_cost();
        let cost = Resources::new(
            base.metal.checked_mul(units).ok_or("Cost overflow")?,
            base.timber.checked_mul(units).ok_or("Cost overflow")?,
            base.rations.checked_mul(units).ok_or("Cost overflow")?,
            base.power.checked_mul(units).ok_or("Cost overflow")?,
        );
        let work_position = self.work_access(order, terrain)?;
        let remainder = self.stores[order.team as usize].checked_sub(cost)?;
        let id = self.id()?;
        self.stores[order.team as usize] = remainder;
        self.jobs.push(BuildJob {
            id,
            order,
            materials: cost,
            phase: JobPhase::Pickup,
            worker: None,
            delivered: false,
            pickup_position: self.bases[order.team as usize],
            work_position,
            work_done: 0,
            work_required: ((units * 20).max(60))
                .try_into()
                .map_err(|_| "Work overflow")?,
            terrain_edits: edits,
            expected_terrain_revision: terrain.revision(),
            blocked_reason: None,
            material_consumed: false,
        });
        Ok(id)
    }
    pub fn cancel(&mut self, id: u64, team: u8) -> Result<(), String> {
        let idx = self
            .jobs
            .iter()
            .position(|j| j.id == id && j.order.team == team)
            .ok_or("No owned order")?;
        let j = self.jobs[idx].clone();
        if matches!(j.phase, JobPhase::Complete | JobPhase::Cancelled) {
            return Err("Order is no longer cancellable".into());
        }
        let position = if j.delivered {
            j.work_position
        } else {
            j.pickup_position
        };
        if distance(position, self.bases[team as usize]) <= WORK_REACH_METERS {
            self.stores[team as usize] = self.stores[team as usize].checked_add(j.materials)?;
        } else if !j.materials.is_empty() {
            let id = self.id()?;
            self.crates.push(MaterialCrate {
                id,
                team,
                position,
                resources: j.materials,
                carrier: None,
                reserved_worker: None,
            });
        }
        self.jobs[idx].phase = JobPhase::Cancelled;
        self.jobs[idx].worker = None;
        self.validate()?;
        Ok(())
    }
    /// Production/care systems may transfer real output into the store. The
    /// caller owns and debits its source; the input is separately named here.
    pub fn credit_external(&mut self, team: u8, amount: Resources) -> Result<(), String> {
        let team = team as usize;
        if team > 1 {
            return Err("Invalid faction".into());
        }
        let store = self.stores[team].checked_add(amount)?;
        let input = self.external_inputs[team].checked_add(amount)?;
        self.stores[team] = store;
        self.external_inputs[team] = input;
        Ok(())
    }
    pub fn consume(&mut self, team: u8, amount: Resources) -> Result<(), String> {
        let team = team as usize;
        if team > 1 {
            return Err("Invalid faction".into());
        }
        let store = self.stores[team].checked_sub(amount)?;
        let used = self.consumed[team].checked_add(amount)?;
        self.stores[team] = store;
        self.consumed[team] = used;
        Ok(())
    }
    pub fn worker_tasks(&mut self, workers: &[WorkerState]) -> Result<Vec<WorkerTask>, String> {
        self.worker_tasks_available(workers, &BTreeSet::new())
    }
    /// Suspended workers remain alive physical owners of their jobs and loads;
    /// only automatic assignment/actions pause while manual/capture orders run.
    pub fn worker_tasks_available(
        &mut self,
        workers: &[WorkerState],
        suspended: &BTreeSet<u32>,
    ) -> Result<Vec<WorkerTask>, String> {
        let mut by_id = BTreeMap::new();
        for &w in workers {
            if w.team > 1
                || Self::validate_position(w.position).is_err()
                || by_id.insert(w.id, w).is_some()
            {
                return Err("Invalid or duplicate construction worker".into());
            }
        }
        let mut assigned = BTreeMap::new();
        let mut available: [VecDeque<usize>; 2] = [VecDeque::new(), VecDeque::new()];
        for (i, j) in self.jobs.iter_mut().enumerate() {
            if matches!(
                j.phase,
                JobPhase::Complete | JobPhase::Cancelled | JobPhase::Blocked
            ) {
                continue;
            }
            let worker = j
                .worker
                .and_then(|id| by_id.get(&id))
                .filter(|w| w.team == j.order.team);
            if j.phase == JobPhase::Hauling {
                if let Some(w) = worker {
                    j.pickup_position = w.position;
                }
            }
            if j.worker.is_some() && worker.is_none() {
                j.worker = None;
                if j.phase == JobPhase::Hauling {
                    j.phase = JobPhase::Pickup;
                }
            }
            if let Some(id) = j.worker {
                if assigned.insert(id, i).is_some() {
                    return Err("Worker assigned to multiple jobs".into());
                }
            } else {
                available[j.order.team as usize].push_back(i);
            }
        }
        let mut carried = BTreeMap::new();
        let mut reserved = BTreeMap::new();
        let mut recoverable: [VecDeque<usize>; 2] = [VecDeque::new(), VecDeque::new()];
        for (i, c) in self.crates.iter_mut().enumerate() {
            if c.resources.is_empty() {
                continue;
            }
            if let Some(id) = c.carrier {
                if let Some(w) = by_id.get(&id).filter(|w| w.team == c.team) {
                    c.position = w.position;
                } else {
                    c.carrier = None;
                    c.reserved_worker = None;
                }
            }
            if c.reserved_worker
                .is_some_and(|id| !by_id.get(&id).is_some_and(|w| w.team == c.team))
            {
                c.reserved_worker = None;
            }
            if let Some(id) = c.carrier {
                carried.insert(id, i);
            } else if let Some(id) = c.reserved_worker {
                reserved.insert(id, i);
            } else {
                recoverable[c.team as usize].push_back(i);
            }
        }
        // A mine extracts through its completed, reachable exterior workplace.
        // The deposit remains the sole finite ore owner until a worker loads;
        // its blocked underground/structure centre is never a walking goal.
        let eligible: [Vec<(usize, [f32; 2])>; 2] = std::array::from_fn(|team| {
            let mut sites = Vec::new();
            for (i, d) in self
                .deposits
                .iter()
                .enumerate()
                .filter(|(_, d)| d.remaining > 0)
            {
                for building in self.buildings.iter().filter(|b| {
                    b.kind == BuildKind::Mine
                        && b.complete
                        && b.health > 0
                        && b.team as usize == team
                        && distance(b.position, d.position) < 160.
                }) {
                    if let Some(job) = self.jobs.iter().rev().find(|j| {
                        j.phase == JobPhase::Complete
                            && j.order.kind == BuildKind::Mine
                            && j.order.team == building.team
                            && j.order.position == building.position
                    }) {
                        sites.push((i, job.work_position));
                    }
                }
            }
            sites
        });
        let mut tasks = Vec::with_capacity(workers.len());
        let mut free: [Vec<WorkerState>; 2] = [Vec::new(), Vec::new()];
        for w in by_id.values() {
            if suspended.contains(&w.id) {
                continue;
            }
            if let Some(c) = self.cargo.get(&w.id) {
                if c.team != w.team {
                    return Err("Worker cargo belongs to another faction".into());
                }
                tasks.push(WorkerTask {
                    worker_id: w.id,
                    destination: self.bases[c.team as usize],
                    job_id: None,
                    purpose: WorkPurpose::ReturnCargo,
                });
                continue;
            }
            if let Some(&i) = carried.get(&w.id) {
                self.material_assignments.insert(w.id, self.crates[i].id);
                tasks.push(WorkerTask {
                    worker_id: w.id,
                    destination: self.bases[w.team as usize],
                    job_id: None,
                    purpose: WorkPurpose::ReturnMaterials,
                });
                continue;
            }
            let existing_job = assigned.get(&w.id).copied();
            let crate_index = reserved.get(&w.id).copied().or_else(|| {
                if existing_job.is_none() {
                    recoverable[w.team as usize].pop_front()
                } else {
                    None
                }
            });
            if let Some(i) = crate_index {
                let c = &mut self.crates[i];
                c.reserved_worker = Some(w.id);
                self.material_assignments.insert(w.id, c.id);
                tasks.push(WorkerTask {
                    worker_id: w.id,
                    destination: c.position,
                    job_id: None,
                    purpose: WorkPurpose::RecoverMaterials,
                });
                continue;
            }
            if let Some(i) = existing_job {
                let j = &mut self.jobs[i];
                j.worker = Some(w.id);
                let purpose = match j.phase {
                    JobPhase::Pickup => WorkPurpose::Pickup,
                    JobPhase::Hauling => WorkPurpose::Deliver,
                    _ => WorkPurpose::Build,
                };
                tasks.push(WorkerTask {
                    worker_id: w.id,
                    destination: if j.phase == JobPhase::Pickup {
                        j.pickup_position
                    } else {
                        j.work_position
                    },
                    job_id: Some(j.id),
                    purpose,
                });
                self.harvest_assignments.remove(&w.id);
            } else {
                free[w.team as usize].push(*w);
            }
        }
        // Preserve existing job/load owners. Only newly unowned FIFO jobs pick
        // the closest eligible worker; a low ID buried in a crowd must not
        // outrank a free worker already at the physical pickup point.
        for team in 0..2 {
            while let Some(i) = available[team].pop_front() {
                let j = &mut self.jobs[i];
                let destination = if j.phase == JobPhase::Pickup {
                    j.pickup_position
                } else {
                    j.work_position
                };
                let Some(candidate) = free[team]
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        distance(a.position, destination)
                            .total_cmp(&distance(b.position, destination))
                            .then(a.id.cmp(&b.id))
                    })
                    .map(|(index, _)| index)
                else {
                    break;
                };
                let w = free[team].swap_remove(candidate);
                j.worker = Some(w.id);
                let purpose = match j.phase {
                    JobPhase::Pickup => WorkPurpose::Pickup,
                    JobPhase::Hauling => WorkPurpose::Deliver,
                    _ => WorkPurpose::Build,
                };
                tasks.push(WorkerTask {
                    worker_id: w.id,
                    destination,
                    job_id: Some(j.id),
                    purpose,
                });
                self.harvest_assignments.remove(&w.id);
            }
            for w in &free[team] {
                if let Some(&(di, access)) = eligible[team]
                    .iter()
                    .min_by(|a, b| distance(a.1, w.position).total_cmp(&distance(b.1, w.position)))
                {
                    let d = &self.deposits[di];
                    self.harvest_assignments.insert(w.id, d.id);
                    tasks.push(WorkerTask {
                        worker_id: w.id,
                        destination: access,
                        job_id: None,
                        purpose: WorkPurpose::Mine,
                    });
                }
            }
        }
        Ok(tasks)
    }
    pub fn step(
        &mut self,
        tick: u64,
        terrain: &mut Terrain,
        river: Option<&mut RiverFixture>,
        workers: &[WorkerState],
    ) -> Result<Vec<ConstructionEvent>, String> {
        self.step_available(tick, terrain, river, workers, &BTreeSet::new())
    }
    pub fn step_available(
        &mut self,
        tick: u64,
        terrain: &mut Terrain,
        mut river: Option<&mut RiverFixture>,
        workers: &[WorkerState],
        suspended: &BTreeSet<u32>,
    ) -> Result<Vec<ConstructionEvent>, String> {
        if self.last_tick.checked_add(1) != Some(tick) {
            return Err("Construction ticks must be contiguous".into());
        }
        let tasks = self.worker_tasks_available(workers, suspended)?;
        let by_id = workers
            .iter()
            .map(|w| (w.id, *w))
            .collect::<BTreeMap<_, _>>();
        let job_index = self
            .jobs
            .iter()
            .enumerate()
            .map(|(i, j)| (j.id, i))
            .collect::<BTreeMap<_, _>>();
        let mut events = Vec::new();
        for task in tasks {
            let w = by_id
                .get(&task.worker_id)
                .ok_or("Assigned worker missing")?;
            let reach = if matches!(task.purpose, WorkPurpose::Deliver | WorkPurpose::Build)
                && task
                    .job_id
                    .and_then(|id| job_index.get(&id).copied())
                    .is_some_and(|i| Self::requires_exterior_access(self.jobs[i].order.kind))
            {
                3.
            } else {
                WORK_REACH_METERS
            };
            if distance(w.position, task.destination) > reach {
                continue;
            }
            if self.last_worker_tick.get(&w.id).copied() == Some(tick) {
                return Err("Worker acted twice in one tick".into());
            }
            self.last_worker_tick.insert(w.id, tick);
            if let Some(id) = task.job_id {
                let i = *job_index.get(&id).ok_or("Assigned job missing")?;
                match self.jobs[i].phase {
                    JobPhase::Pickup => {
                        self.jobs[i].phase = JobPhase::Hauling;
                        self.jobs[i].pickup_position = w.position;
                    }
                    JobPhase::Hauling => {
                        self.jobs[i].phase = JobPhase::Working;
                        self.jobs[i].delivered = true;
                        self.jobs[i].pickup_position = self.jobs[i].work_position;
                    }
                    JobPhase::Working => {
                        self.jobs[i].work_done = self.jobs[i]
                            .work_done
                            .checked_add(1)
                            .ok_or("Work overflow")?;
                        if self.jobs[i].work_done >= self.jobs[i].work_required {
                            match self.complete(i, terrain, river.as_deref_mut()) {
                                Ok(mut e) => events.append(&mut e),
                                Err(error) => {
                                    self.jobs[i].phase = JobPhase::Blocked;
                                    self.jobs[i].blocked_reason = Some(error.clone());
                                    self.jobs[i].worker = None;
                                    events.push(ConstructionEvent::Blocked {
                                        job: id,
                                        reason: error,
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }
            } else if matches!(
                task.purpose,
                WorkPurpose::RecoverMaterials | WorkPurpose::ReturnMaterials
            ) {
                let id = *self
                    .material_assignments
                    .get(&w.id)
                    .ok_or("Material assignment missing")?;
                let i = self
                    .crates
                    .iter()
                    .position(|c| c.id == id)
                    .ok_or("Material crate missing")?;
                if task.purpose == WorkPurpose::RecoverMaterials {
                    self.crates[i].carrier = Some(w.id);
                    self.crates[i].position = w.position;
                } else {
                    let new = self.stores[w.team as usize].checked_add(self.crates[i].resources)?;
                    self.stores[w.team as usize] = new;
                    self.crates[i].resources = Resources::default();
                    self.crates[i].carrier = None;
                    self.crates[i].reserved_worker = None;
                    self.material_assignments.remove(&w.id);
                }
            } else if task.purpose == WorkPurpose::ReturnCargo {
                if let Some(c) = self.cargo.get(&w.id).cloned() {
                    let new = self.stores[c.team as usize]
                        .checked_add(Resources::new(c.amount, 0, 0, 0))?;
                    self.stores[c.team as usize] = new;
                    self.cargo.remove(&w.id);
                    events.push(ConstructionEvent::Mined {
                        team: c.team,
                        deposit: c.deposit_id,
                        amount: c.amount,
                    });
                }
            } else if task.purpose == WorkPurpose::Mine {
                let id = *self
                    .harvest_assignments
                    .get(&w.id)
                    .ok_or("Mining assignment missing")?;
                let d = self
                    .deposits
                    .iter_mut()
                    .find(|d| d.id == id)
                    .ok_or("Mine deposit missing")?;
                let amount = d.remaining.min(40);
                d.remaining -= amount;
                if amount > 0 {
                    self.cargo.insert(
                        w.id,
                        OreCargo {
                            team: w.team,
                            deposit_id: id,
                            amount,
                        },
                    );
                }
            }
        }
        // Solar input is named external energy, not fabricated minerals or
        // biological resources. Only complete, surviving arrays generate it.
        if tick % 20 == 0 {
            for team in 0..2 {
                let count = self
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.team == team && b.kind == BuildKind::Solar && b.complete && b.health > 0
                    })
                    .count() as u64;
                if count > 0 {
                    self.credit_external(
                        team,
                        Resources::new(
                            0,
                            0,
                            0,
                            count.checked_mul(10).ok_or("Solar output overflow")?,
                        ),
                    )?;
                }
            }
        }
        self.last_tick = tick;
        self.validate()?;
        Ok(events)
    }
    fn complete(
        &mut self,
        index: usize,
        terrain: &mut Terrain,
        mut river: Option<&mut RiverFixture>,
    ) -> Result<Vec<ConstructionEvent>, String> {
        self.validate()?;
        let j = self
            .jobs
            .get(index)
            .ok_or("Construction job missing")?
            .clone();
        if j.order.kind==BuildKind::MountedCamera && self.mount_host(j.order).is_none() {
            return Err("Camera mount was lost before completion; cancel or restore its host".into());
        }
        let team = j.order.team as usize;
        let next_revision = self
            .navigation_revision
            .checked_add(1)
            .ok_or("Navigation revision exhausted")?;
        // Reserve more than the maximum number of per-cell terrain revisions
        // this bounded job can issue. No public setter can overflow after this.
        terrain
            .revision()
            .checked_add((j.terrain_edits.len() + CW * CH + 1) as u64)
            .ok_or("Terrain revision exhausted")?;
        let mut cut = 0u64;
        let mut fill = 0u64;
        for e in &j.terrain_edits {
            if terrain.elevation_mm(e.cell).map_err(|e| e.to_string())? != e.before_mm {
                return Err(
                    "Terrain order is stale; cancel and reissue after reviewing the changed ground"
                        .into(),
                );
            }
            terrain
                .validate_elevation_edit(e.cell, e.target_mm)
                .map_err(|e| e.to_string())?;
            let q = (e.target_mm as i64 - e.before_mm as i64)
                .unsigned_abs()
                .checked_mul(CELL_AREA)
                .ok_or("Soil volume overflow")?;
            if e.target_mm < e.before_mm {
                cut = cut.checked_add(q).ok_or("Cut overflow")?;
            } else {
                fill = fill.checked_add(q).ok_or("Fill overflow")?;
            }
        }
        let spoil = self.spoil_liters[team]
            .checked_add(cut)
            .and_then(|n| n.checked_sub(fill))
            .ok_or("Insufficient excavated spoil for this fill; dig or choose another level")?;
        let nextcut = self.cut_liters[team]
            .checked_add(cut)
            .ok_or("Cut ledger overflow")?;
        let nextfill = self.fill_liters[team]
            .checked_add(fill)
            .ok_or("Fill ledger overflow")?;
        let consumed = self.consumed[team].checked_add(j.materials)?;
        let managed = j
            .terrain_edits
            .iter()
            .filter(|e| RiverFixture::local_index(e.cell).is_some())
            .map(|e| (e.cell, e.target_mm))
            .collect::<Vec<_>>();
        let footprint = Self::footprint_cells(j.order);
        let mut removals = Vec::with_capacity(footprint.len());
        let mut managed_removals = Vec::new();
        for cell in footprint {
            terrain
                .initial_foliage_units(cell)
                .ok_or("Invalid terrain biological cache")?;
            let cached = terrain.foliage_units(cell).map_err(|e| e.to_string())?;
            let flags = terrain
                .cells
                .get(cell)
                .ok_or("Invalid footprint cell")?
                .flags;
            let moisture = ((flags & crate::terrain::SOIL_MOISTURE_MASK)
                >> crate::terrain::SOIL_MOISTURE_SHIFT) as u16;
            terrain
                .validate_ground_surface(cell, 0, moisture)
                .map_err(|e| e.to_string())?;
            // Terrain is a derived cache. A fresh water tick may have reduced
            // the true ground-owned stock before this frame's visual sync.
            let amount = if let Some(local) = RiverFixture::local_index(cell) {
                let r = river
                    .as_deref()
                    .ok_or("Managed foliage needs its live ground owner")?;
                let n = r
                    .ground
                    .cells()
                    .get(local)
                    .ok_or("Invalid managed footprint")?
                    .foliage_units;
                managed_removals.push((cell, n));
                n
            } else {
                cached
            };
            removals.push((cell, amount, moisture));
        }
        if j.order.kind == BuildKind::Dam {
            river
                .as_deref()
                .ok_or("Dam requires live river hydrology")?
                .validate_dam_change()?;
        }
        let (building, next_id) = if !j.order.kind.is_terrain() {
            let id = self.next_id;
            let next = id.checked_add(1).ok_or("Construction ID exhausted")?;
            let cell = Self::validate_position(j.order.position)?;
            let deck = if j.order.kind == BuildKind::Bridge {
                Some(
                    i32::try_from(
                        terrain.elevation_mm(cell).map_err(|e| e.to_string())? as i64
                            + terrain.cells[cell].water as i64
                            + 3000,
                    )
                    .map_err(|_| "Bridge elevation overflow")?,
                )
            } else {
                None
            };
            self.buildings.reserve(1);
            (
                Some(Building {
                    id,
                    kind: j.order.kind,
                    team: j.order.team,
                    position: j.order.position,
                    end: j.order.end,
                    health: 1000,
                    max_health: 1000,
                    complete: true,
                    size_meters: match j.order.kind {
                        BuildKind::Camera => 20.,
                        BuildKind::FieldCamera => 12.,
                        BuildKind::CameraTower => 32.,
                        BuildKind::MountedCamera => 10.,
                        BuildKind::Wall => 24.,
                        BuildKind::Barracks => 56.,
                        _ => 40.,
                    },
                    deck_height_mm: deck,
                    draft_clearance_mm: if j.order.kind == BuildKind::Bridge {
                        2500
                    } else {
                        0
                    },
                    load_limit_kg: if j.order.kind == BuildKind::Bridge {
                        40_000
                    } else {
                        0
                    },
                    construction_cost: j.materials,
                }),
                next,
            )
        } else {
            (None, self.next_id)
        };
        let mut events = Vec::with_capacity(removals.len() + 2);
        let built = building.as_ref().map(|b| b.id);
        // FIRST owned mutation: the managed owner's combined transaction checks
        // every bed and foliage change together. Failure commits none of them.
        if !managed.is_empty() || !managed_removals.is_empty() {
            river
                .as_deref_mut()
                .ok_or("Managed edit needs live river owner")?
                .apply_construction(&managed, &managed_removals)?;
        }
        // All remaining indices, ranges, counters, cache inputs and barrier
        // links have been prevalidated; these writes have no recoverable failure
        // path. Never mark a partially committed material operation as Blocked.
        if j.order.kind == BuildKind::Dam {
            river
                .as_deref_mut()
                .expect("validated river")
                .set_dam(true)
                .expect("validated hydraulic links");
        }
        for e in &j.terrain_edits {
            terrain
                .set_elevation_mm(e.cell, e.target_mm)
                .expect("validated terrain edit");
        }
        if j.order.kind == BuildKind::Road {
            terrain.road(j.order.position, j.order.end);
        }
        for (cell, amount, moisture) in removals {
            terrain
                .set_ground_surface(cell, 0, moisture, true)
                .expect("validated finite cache removal");
            if amount > 0 {
                events.push(ConstructionEvent::HabitatCleared {
                    team: j.order.team,
                    cell,
                    foliage_units: amount,
                });
            }
        }
        self.spoil_liters[team] = spoil;
        self.cut_liters[team] = nextcut;
        self.fill_liters[team] = nextfill;
        self.consumed[team] = consumed;
        self.navigation_revision = next_revision;
        self.next_id = next_id;
        if let Some(b) = building {
            self.buildings.push(b);
        }
        self.jobs[index].phase = JobPhase::Complete;
        self.jobs[index].material_consumed = true;
        self.jobs[index].worker = None;
        if cut > 0 || fill > 0 {
            events.push(ConstructionEvent::TerrainChanged {
                team: j.order.team,
                cells: j.terrain_edits.iter().map(|e| e.cell).collect(),
                cut_liters: cut,
                fill_liters: fill,
            });
        }
        events.push(ConstructionEvent::Completed {
            job: j.id,
            building: built,
            team: j.order.team,
            kind: j.order.kind,
            position: j.order.position,
        });
        Ok(events)
    }
    pub fn footprint_cells(order: BuildOrder) -> Vec<usize> {
        let mut cells = BTreeSet::new();
        if matches!(
            order.kind,
            BuildKind::Road | BuildKind::Trench | BuildKind::Bridge
        ) {
            let length = (order.end[0] - order.position[0]).hypot(order.end[1] - order.position[1]);
            let n = (length / 4.).ceil().max(1.) as usize;
            for i in 0..=n {
                let a = i as f32 / n as f32;
                if let Ok(cell) = Self::validate_position([
                    order.position[0] + (order.end[0] - order.position[0]) * a,
                    order.position[1] + (order.end[1] - order.position[1]) * a,
                ]) {
                    cells.insert(cell);
                }
            }
        } else if let Ok(cell) = Self::validate_position(order.position) {
            let x = cell % CW;
            let y = cell / CW;
            for yy in y.saturating_sub(1)..=(y + 1).min(CH - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(CW - 1) {
                    cells.insert(yy * CW + xx);
                }
            }
        }
        cells.into_iter().collect()
    }
    pub fn bridge_at(&self, position: [f32; 2], load_kg: u32) -> Option<&Building> {
        self.buildings.iter().find(|b| {
            b.kind == BuildKind::Bridge
                && b.complete
                && b.health > 0
                && b.load_limit_kg >= load_kg
                && segment_distance(position, b.position, b.end) < 8.
        })
    }
    pub fn blocking_at(&self, position: [f32; 2]) -> bool {
        self.buildings.iter().any(|b| {
            b.complete
                && b.health > 0
                && b.kind.solid()
                && distance(position, b.position) < b.size_meters * 0.5
        })
    }
    pub fn damage_building(&mut self, id: u64, damage: u32) -> Result<bool, String> {
        let b = self
            .buildings
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or("Building not found")?;
        let before = b.health;
        b.health = b
            .health
            .checked_sub(damage.min(b.health))
            .ok_or("Building health underflow")?;
        if before > 0 && b.health == 0 {
            self.navigation_revision = self
                .navigation_revision
                .checked_add(1)
                .ok_or("Navigation revision exhausted")?;
        }
        Ok(b.health == 0)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .bases
            .iter()
            .any(|&p| Self::validate_position(p).is_err())
        {
            return Err("Invalid saved base position".into());
        }
        if self
            .last_worker_tick
            .values()
            .any(|&t| self.last_tick.checked_add(1).is_none_or(|limit| t > limit))
        {
            return Err("Worker timestamp is ahead of construction clock".into());
        }
        if self.buildings.iter().any(|b| {
            b.team > 1
                || b.health > b.max_health
                || !b.size_meters.is_finite()
                || b.size_meters <= 0.
                || Self::validate_position(b.position).is_err()
                || Self::validate_position(b.end).is_err()
        }) {
            return Err("Invalid saved building state".into());
        }
        if self
            .deposits
            .iter()
            .any(|d| Self::validate_position(d.position).is_err())
        {
            return Err("Invalid saved deposit position".into());
        }
        let mut owned = [0u128; 4];
        let mut source = [0u128; 4];
        for t in 0..2 {
            for k in 0..4 {
                owned[k] +=
                    self.stores[t].values()[k] as u128 + self.consumed[t].values()[k] as u128;
                source[k] += self.initial_salvage[t].values()[k] as u128
                    + self.external_inputs[t].values()[k] as u128;
            }
            if self.cut_liters[t].checked_sub(self.fill_liters[t]) != Some(self.spoil_liters[t]) {
                return Err("Excavated spoil ledger does not balance".into());
            }
        }
        for j in &self.jobs {
            if j.order.team > 1
                || j.work_done > j.work_required
                || Self::validate_position(j.order.position).is_err()
                || Self::validate_position(j.order.end).is_err()
                || Self::validate_position(j.pickup_position).is_err()
                || Self::validate_position(j.work_position).is_err()
                || j.terrain_edits
                    .iter()
                    .any(|e| e.cell >= CW * CH || !(-1_000_000..=1_000_000).contains(&e.target_mm))
            {
                return Err("Invalid construction job state".into());
            }
            if !matches!(j.phase, JobPhase::Complete | JobPhase::Cancelled) {
                for (k, v) in j.materials.values().iter().enumerate() {
                    owned[k] += *v as u128;
                }
            }
            if j.material_consumed != (j.phase == JobPhase::Complete) {
                return Err("Construction material consumed twice or missing".into());
            }
        }
        for d in &self.deposits {
            owned[0] += d.remaining as u128;
            source[0] += d.initial as u128;
            if d.remaining > d.initial {
                return Err("Ore deposit stock increased without input".into());
            }
        }
        for c in &self.crates {
            if c.team > 1 || Self::validate_position(c.position).is_err() {
                return Err("Invalid material crate".into());
            }
            for (k, v) in c.resources.values().iter().enumerate() {
                owned[k] += *v as u128;
            }
        }
        for c in self.cargo.values() {
            owned[0] += c.amount as u128;
            if c.team > 1 {
                return Err("Invalid cargo owner".into());
            }
        }
        if owned != source {
            return Err("Construction resource stock does not balance".into());
        }
        let ids = self
            .jobs
            .iter()
            .map(|j| j.id)
            .chain(self.buildings.iter().map(|b| b.id))
            .chain(self.crates.iter().map(|c| c.id))
            .collect::<Vec<_>>();
        if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len()
            || ids.iter().any(|&id| id >= self.next_id)
        {
            return Err("Duplicate or invalid construction identity".into());
        }
        Ok(())
    }
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let n = dx * dx + dy * dy;
    if n == 0. {
        return distance(p, a);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / n).clamp(0., 1.);
    distance(p, [a[0] + t * dx, a[1] + t * dy])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dry_access_fixture(t: &mut Terrain, p: [f32; 2]) {
        let c = ConstructionWorld::validate_position(p).unwrap();
        let x = c % CW;
        let y = c / CW;
        for yy in y.saturating_sub(12)..=(y + 12).min(CH - 1) {
            for xx in x.saturating_sub(12)..=(x + 12).min(CW - 1) {
                t.set_water_depth_mm(yy * CW + xx, 0).unwrap();
            }
        }
    }
    fn run_job(
        w: &mut ConstructionWorld,
        t: &mut Terrain,
        id: u64,
        river: Option<&mut RiverFixture>,
    ) -> Result<Vec<ConstructionEvent>, String> {
        let job = w.jobs.iter().find(|j| j.id == id).unwrap().clone();
        let mut r = river;
        let mut all = Vec::new();
        for _ in 0..job.work_required + 3 {
            let phase = w.jobs.iter().find(|j| j.id == id).unwrap().phase;
            let p = if phase == JobPhase::Pickup {
                w.bases[job.order.team as usize]
            } else {
                job.work_position
            };
            all.extend(w.step(
                w.last_tick + 1,
                t,
                r.as_deref_mut(),
                &[WorkerState {
                    id: 1,
                    team: job.order.team,
                    position: p,
                }],
            )?);
        }
        Ok(all)
    }
    #[test]
    fn jobs_require_real_pickup_delivery_and_work_then_consume_once() {
        let mut t = Terrain::new(44);
        let mut w = ConstructionWorld::new(44);
        let p = [600., 1600.];
        // Clear generated wetness for a controlled land construction fixture.
        dry_access_fixture(&mut t, p);
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Camera, p), &t)
            .unwrap();
        let initial = w.stores[0];
        for tick in 1..10 {
            w.step(
                tick,
                &mut t,
                None,
                &[WorkerState {
                    id: 1,
                    team: 0,
                    position: [2000., 2000.],
                }],
            )
            .unwrap();
        }
        assert_eq!(w.jobs[0].work_done, 0);
        assert_eq!(w.jobs[0].phase, JobPhase::Pickup);
        run_job(&mut w, &mut t, id, None).unwrap();
        assert_eq!(w.jobs[0].phase, JobPhase::Complete);
        assert_eq!(w.buildings.len(), 1);
        assert_eq!(w.stores[0], initial);
        w.validate().unwrap();
        assert!(w.cancel(id, 0).is_err());
    }
    #[test]
    fn reservation_cancel_and_save_roundtrip_preserve_exact_resources() {
        let mut t = Terrain::new(2);
        let p = [600., 1600.];
        dry_access_fixture(&mut t, p);
        let mut w = ConstructionWorld::new(2);
        let before = w.stores;
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Depot, p), &t)
            .unwrap();
        assert_ne!(w.stores, before);
        let data = serde_json::to_vec(&w).unwrap();
        let mut restored: ConstructionWorld = serde_json::from_slice(&data).unwrap();
        restored.validate().unwrap();
        restored.cancel(id, 0).unwrap();
        assert_eq!(restored.stores, before);
        restored.validate().unwrap();
        assert!(restored.cancel(id, 0).is_err());
    }
    #[test]
    fn trench_displaces_real_spoil_and_stale_jobs_do_not_overwrite() {
        let mut t = Terrain::new(22);
        let mut w = ConstructionWorld::new(22);
        let p = [600., 1600.];
        let c = ConstructionWorld::validate_position(p).unwrap();
        let h = t.elevation_mm(c).unwrap();
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Trench, p), &t)
            .unwrap();
        run_job(&mut w, &mut t, id, None).unwrap();
        assert_eq!(t.elevation_mm(c).unwrap(), h - 350);
        assert_eq!(w.spoil_liters[0], 350 * CELL_AREA);
        w.validate().unwrap();
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Trench, p), &t)
            .unwrap();
        t.set_elevation_mm(c, h - 400).unwrap();
        run_job(&mut w, &mut t, id, None).unwrap();
        assert_eq!(
            w.jobs.iter().find(|j| j.id == id).unwrap().phase,
            JobPhase::Blocked
        );
        assert_eq!(t.elevation_mm(c).unwrap(), h - 400);
        w.validate().unwrap();
    }
    #[test]
    fn managed_trench_preserves_water_and_soil_transactions_survive_ticks() {
        let mut t = Terrain::new(78421);
        let mut r = RiverFixture::new(&mut t).unwrap();
        let mut w = ConstructionWorld::new(78421);
        let p = [2384., 600.];
        let cell = ConstructionWorld::validate_position(p).unwrap();
        let before = r.water.totals().stored_liters;
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Trench, p), &t)
            .unwrap();
        run_job(&mut w, &mut t, id, Some(&mut r)).unwrap();
        assert_eq!(r.water.totals().stored_liters, before);
        assert!(r.ground.totals().construction_exported_liters > 0);
        assert_eq!(
            t.elevation_mm(cell).unwrap(),
            r.ground.cells()[RiverFixture::local_index(cell).unwrap()].bed_mm
        );
        r.advance_to(1).unwrap();
        assert!(r.ground.totals().balanced());
        w.validate().unwrap();
    }
    #[test]
    fn cancellation_at_site_needs_physical_material_recovery_before_refund() {
        let mut t = Terrain::new(44);
        let p = [600., 1600.];
        dry_access_fixture(&mut t, p);
        let mut w = ConstructionWorld::new(44);
        let initial = w.stores[0];
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Camera, p), &t)
            .unwrap();
        let reserved = w.stores[0];
        let access = w.jobs[0].work_position;
        let base = w.bases[0];
        w.step(
            1,
            &mut t,
            None,
            &[WorkerState {
                id: 1,
                team: 0,
                position: base,
            }],
        )
        .unwrap();
        w.step(
            2,
            &mut t,
            None,
            &[WorkerState {
                id: 1,
                team: 0,
                position: access,
            }],
        )
        .unwrap();
        w.cancel(id, 0).unwrap();
        assert_eq!(w.stores[0], reserved);
        assert_eq!(w.crates.len(), 1);
        w.validate().unwrap();
        w.step(
            3,
            &mut t,
            None,
            &[WorkerState {
                id: 1,
                team: 0,
                position: base,
            }],
        )
        .unwrap();
        assert_eq!(w.stores[0], reserved);
        w.step(
            4,
            &mut t,
            None,
            &[WorkerState {
                id: 1,
                team: 0,
                position: access,
            }],
        )
        .unwrap();
        assert_eq!(w.stores[0], reserved);
        w.step(
            5,
            &mut t,
            None,
            &[WorkerState {
                id: 1,
                team: 0,
                position: base,
            }],
        )
        .unwrap();
        assert_eq!(w.stores[0], initial);
        assert!(w.crates[0].resources.is_empty());
        w.validate().unwrap();
    }
    #[test]
    fn exhausted_building_identity_does_not_partially_clear_or_consume() {
        let mut t = Terrain::new(44);
        let p = [600., 1600.];
        let cell = ConstructionWorld::validate_position(p).unwrap();
        dry_access_fixture(&mut t, p);
        let mut w = ConstructionWorld::new(44);
        let id = w
            .enqueue(BuildOrder::at(0, BuildKind::Camera, p), &t)
            .unwrap();
        w.next_id = u64::MAX;
        let before = t.foliage_units(cell).unwrap();
        let spent = w.consumed;
        run_job(&mut w, &mut t, id, None).unwrap();
        assert_eq!(w.jobs[0].phase, JobPhase::Blocked);
        assert_eq!(t.foliage_units(cell).unwrap(), before);
        assert_eq!(w.consumed, spent);
        assert!(w.buildings.is_empty());
        w.validate().unwrap();
    }
    #[test]
    fn two_thousand_workers_keep_ids_and_only_assigned_reachable_workers_act() {
        let mut t = Terrain::new(44);
        let mut w = ConstructionWorld::new(44);
        for team in 0..2 {
            let p = w.bases[team];
            dry_access_fixture(&mut t, p);
            w.enqueue(BuildOrder::at(team as u8, BuildKind::Camera, p), &t)
                .unwrap();
        }
        let workers = (0..2000)
            .map(|id| WorkerState {
                id,
                team: (id % 2) as u8,
                position: w.bases[(id % 2) as usize],
            })
            .collect::<Vec<_>>();
        let tasks = w.worker_tasks(&workers).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_ne!(tasks[0].worker_id, tasks[1].worker_id);
        w.step(1, &mut t, None, &workers).unwrap();
        assert!(
            w.jobs
                .iter()
                .all(|j| j.phase == JobPhase::Hauling && j.work_done == 0)
        );
        w.validate().unwrap();
    }
    #[test]
    fn new_jobs_choose_nearby_workers_without_stealing_existing_owners() {
        let mut terrain = Terrain::new(44);
        let mut world = ConstructionWorld::new(44);
        let point = [600., 1600.];
        dry_access_fixture(&mut terrain, point);
        world
            .enqueue(BuildOrder::at(0, BuildKind::Camera, point), &terrain)
            .unwrap();
        let base = world.bases[0];
        let workers = [
            WorkerState {
                id: 1,
                team: 0,
                position: [base[0] - 80., base[1]],
            },
            WorkerState {
                id: 9,
                team: 0,
                position: base,
            },
            WorkerState {
                id: 8,
                team: 0,
                position: base,
            },
        ];
        let before = world.stores;
        let tasks = world.worker_tasks(&workers).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].worker_id, 8); // stable ID breaks equal-distance ties
        world.step(1, &mut terrain, None, &workers).unwrap();
        assert_eq!(world.jobs[0].phase, JobPhase::Hauling);
        let moved = [
            WorkerState {
                id: 8,
                team: 0,
                position: [base[0] - 100., base[1]],
            },
            workers[1],
        ];
        assert_eq!(world.worker_tasks(&moved).unwrap()[0].worker_id, 8);
        assert!(
            world
                .worker_tasks_available(&moved, &BTreeSet::from([8]))
                .unwrap()
                .is_empty()
        );
        assert_eq!(world.jobs[0].worker, Some(8));
        assert_eq!(world.stores, before);
        world.validate().unwrap();
    }
    #[test]
    fn suspended_hauler_keeps_physical_load_and_job_then_resumes_delivery() {
        let mut t = Terrain::new(44);
        let p = [600., 1600.];
        dry_access_fixture(&mut t, p);
        let mut w = ConstructionWorld::new(44);
        w.enqueue(BuildOrder::at(0, BuildKind::Camera, p), &t)
            .unwrap();
        let reserved = w.stores[0];
        let base = w.bases[0];
        w.step(
            1,
            &mut t,
            None,
            &[WorkerState {
                id: 1,
                team: 0,
                position: base,
            }],
        )
        .unwrap();
        assert_eq!(w.jobs[0].phase, JobPhase::Hauling);
        let suspended = BTreeSet::from([1]);
        let manual = [500., 1700.];
        let all = [
            WorkerState {
                id: 1,
                team: 0,
                position: manual,
            },
            WorkerState {
                id: 2,
                team: 0,
                position: base,
            },
        ];
        assert!(
            w.worker_tasks_available(&all, &suspended)
                .unwrap()
                .is_empty()
        );
        w.step_available(2, &mut t, None, &all, &suspended).unwrap();
        assert_eq!(w.jobs[0].worker, Some(1));
        assert_eq!(w.jobs[0].phase, JobPhase::Hauling);
        assert_eq!(w.jobs[0].pickup_position, manual);
        assert_eq!(w.jobs[0].work_done, 0);
        assert!(w.crates.is_empty());
        assert_eq!(w.stores[0], reserved);
        let task = w.worker_tasks(&all).unwrap();
        assert_eq!(task.len(), 1);
        assert_eq!(task[0].worker_id, 1);
        assert_eq!(task[0].purpose, WorkPurpose::Deliver);
        let access = w.jobs[0].work_position;
        w.step(
            3,
            &mut t,
            None,
            &[
                WorkerState {
                    id: 1,
                    team: 0,
                    position: access,
                },
                all[1],
            ],
        )
        .unwrap();
        assert_eq!(w.jobs[0].phase, JobPhase::Working);
        assert!(w.jobs[0].delivered);
        assert_eq!(w.stores[0], reserved);
        w.validate().unwrap();
    }
    #[test]
    fn one_worker_walks_out_after_a_solid_build_and_completes_a_second() {
        use crate::tactics::{MovementClass, NAV_CELL_MM, NavCell, NavGrid, Role, Tactics};
        let mut t = Terrain::new(44);
        let mut w = ConstructionWorld::new(44);
        let first = [600., 1600.];
        let second = [800., 1600.];
        dry_access_fixture(&mut t, first);
        dry_access_fixture(&mut t, second);
        w.bases[0] = [450., 1600.];
        w.enqueue(BuildOrder::at(0, BuildKind::Barracks, first), &t)
            .unwrap();
        w.enqueue(BuildOrder::at(0, BuildKind::Camera, second), &t)
            .unwrap();
        let mut grid =
            NavGrid::new(64, 112, NAV_CELL_MM, vec![NavCell::default(); 64 * 112]).unwrap();
        let mut pawns = Tactics::new(44);
        let pawn = pawns.spawn(0, Role::Worker, w.bases[0]).unwrap();
        let mut completed = 0;
        for tick in 1..=12_000 {
            let position = pawns.agent(pawn).unwrap().position();
            let worker = WorkerState {
                id: pawn as u32,
                team: 0,
                position,
            };
            for task in w.worker_tasks(&[worker]).unwrap() {
                pawns.assign(pawn, task.destination, task.job_id).unwrap();
            }
            pawns.step(tick, &grid).unwrap();
            let now = pawns.agent(pawn).unwrap().position();
            w.step(
                tick,
                &mut t,
                None,
                &[WorkerState {
                    position: now,
                    ..worker
                }],
            )
            .unwrap();
            if w.buildings.len() > completed {
                for b in w.buildings.iter().skip(completed) {
                    for cell in ConstructionWorld::footprint_cells(BuildOrder {
                        team: b.team,
                        kind: b.kind,
                        position: b.position,
                        end: b.end,
                    }) {
                        let p = [(cell % CW * 8 + 4) as f32, (cell / CW * 8 + 4) as f32];
                        let i = grid.index(p).unwrap();
                        let mut c = grid.cells[i];
                        c.blocked = true;
                        grid.set_cell(i, c).unwrap();
                    }
                }
                completed = w.buildings.len();
                let site = grid.index(now).unwrap();
                assert!(
                    grid.passable(site, MovementClass::Foot),
                    "Completed building entombed its builder"
                );
            }
            if completed == 2 {
                break;
            }
        }
        assert_eq!(
            completed, 2,
            "Builder must leave first structure and physically deliver/build the second"
        );
        assert!(w.jobs.iter().all(|j| j.phase == JobPhase::Complete));
        pawns.validate().unwrap();
        w.validate().unwrap();
    }
    #[test]
    fn completed_solid_mine_loads_at_exterior_and_worker_hauls_real_ore_to_base() {
        use crate::tactics::{MovementClass, NavGrid, Role, Tactics};
        let mut terrain = Terrain::new(44);
        // Controlled connected land strip; both pathfinding and construction
        // read these same physical samples, not an all-passable surrogate grid.
        for y in 184..=218 {
            for x in 40..=100 {
                let cell = y * CW + x;
                terrain.set_water_depth_mm(cell, 0).unwrap();
                terrain.set_elevation_mm(cell, 10_000).unwrap();
            }
        }
        let mut world = ConstructionWorld::new(44);
        world.bases[0] = [450., 1600.];
        world.deposits[0].position = [600., 1600.];
        let initial_ore = world.deposits[0].remaining;
        world
            .enqueue(
                BuildOrder::at(0, BuildKind::Mine, world.deposits[0].position),
                &terrain,
            )
            .unwrap();
        let paid_store = world.stores[0].metal;
        let mut nav = NavGrid::from_terrain(&terrain).unwrap();
        let mut tactics = Tactics::new(44);
        let worker = tactics.spawn(0, Role::Worker, world.bases[0]).unwrap();
        let mut footprint_applied = false;
        let mut saw_cargo = false;
        let mut delivered = 0;
        for tick in 1..=12_000 {
            let state = WorkerState {
                id: worker as u32,
                team: 0,
                position: tactics.agent(worker).unwrap().position(),
            };
            for task in world.worker_tasks(&[state]).unwrap() {
                if task.purpose == WorkPurpose::Mine {
                    assert_ne!(task.destination, world.deposits[0].position);
                    assert!(
                        nav.passable(nav.index(task.destination).unwrap(), MovementClass::Foot)
                    );
                }
                tactics
                    .assign_on_grid_within(worker, task.destination, task.job_id, &nav, 2.)
                    .unwrap();
            }
            tactics.step(tick, &nav).unwrap();
            let now = tactics.agent(worker).unwrap().position();
            let events = world
                .step(
                    tick,
                    &mut terrain,
                    None,
                    &[WorkerState {
                        position: now,
                        ..state
                    }],
                )
                .unwrap();
            if !world.buildings.is_empty() && !footprint_applied {
                for cell in ConstructionWorld::footprint_cells(BuildOrder::at(
                    0,
                    BuildKind::Mine,
                    world.deposits[0].position,
                )) {
                    terrain.set_occupancy(cell, 1).unwrap();
                }
                nav.refresh(&terrain).unwrap();
                footprint_applied = true;
                assert!(nav.passable(nav.index(now).unwrap(), MovementClass::Foot));
            }
            if let Some(cargo) = world.cargo.get(&(worker as u32)) {
                saw_cargo = true;
                assert_eq!(world.stores[0].metal, paid_store);
                assert_eq!(initial_ore - world.deposits[0].remaining, cargo.amount);
            }
            for event in events {
                if let ConstructionEvent::Mined {
                    team: 0,
                    deposit: 0,
                    amount,
                } = event
                {
                    assert!(distance(now, world.bases[0]) <= WORK_REACH_METERS);
                    delivered += amount;
                }
            }
            if delivered > 0 {
                break;
            }
        }
        assert!(
            footprint_applied && saw_cargo && delivered > 0,
            "Real worker must complete the mine, load finite ore outside it and walk the cargo home"
        );
        assert_eq!(world.stores[0].metal, paid_store + delivered);
        assert_eq!(initial_ore - world.deposits[0].remaining, delivered);
        assert!(world.cargo.is_empty());
        world.validate().unwrap();
        tactics.validate().unwrap();
    }
    #[test]
    fn invalid_orders_and_duplicate_workers_cannot_spend_or_duplicate_labor() {
        let mut t = Terrain::new(3);
        let mut w = ConstructionWorld::new(3);
        let before = w.stores;
        assert!(
            w.enqueue(BuildOrder::at(2, BuildKind::Camera, [100., 100.]), &t)
                .is_err()
        );
        assert!(
            w.enqueue(BuildOrder::at(0, BuildKind::Camera, [f32::NAN, 100.]), &t)
                .is_err()
        );
        assert_eq!(w.stores, before);
        let a = WorkerState {
            id: 4,
            team: 0,
            position: [424., 1640.],
        };
        assert!(w.step(1, &mut t, None, &[a, a]).is_err());
        assert_eq!(w.last_tick, 0);
        w.validate().unwrap();
    }
}
