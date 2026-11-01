//! Sparse ecosystem-engineering structures. These records own physical built
//! material and occupancy references, never animals, plants or a second water
//! simulation. Terrain/biomass/barrier events must be committed by the campaign
//! transaction that owns those quantities. Coordinates are millimetres.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub type HabitatId = u64;
pub type OrganismId = u64;
pub type PatchId = u32;
const MAX_CENTERLINE_POINTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point3Mm {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl Point3Mm {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Materials {
    /// Loose mineral soil, separate from in-situ terrain owned by the campaign.
    pub soil_mm3: u64,
    pub wood_units: u64,
    pub fiber_units: u64,
    pub mineral_units: u64,
}
impl Materials {
    fn can_pay(self, c: Self) -> bool {
        self.soil_mm3 >= c.soil_mm3
            && self.wood_units >= c.wood_units
            && self.fiber_units >= c.fiber_units
            && self.mineral_units >= c.mineral_units
    }
    fn subtract(&mut self, c: Self) {
        self.soil_mm3 -= c.soil_mm3;
        self.wood_units -= c.wood_units;
        self.fiber_units -= c.fiber_units;
        self.mineral_units -= c.mineral_units;
    }
    fn checked_add(self, other: Self) -> Result<Self, HabitatError> {
        Ok(Self {
            soil_mm3: self
                .soil_mm3
                .checked_add(other.soil_mm3)
                .ok_or(HabitatError::Overflow)?,
            wood_units: self
                .wood_units
                .checked_add(other.wood_units)
                .ok_or(HabitatError::Overflow)?,
            fiber_units: self
                .fiber_units
                .checked_add(other.fiber_units)
                .ok_or(HabitatError::Overflow)?,
            mineral_units: self
                .mineral_units
                .checked_add(other.mineral_units)
                .ok_or(HabitatError::Overflow)?,
        })
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkBudget {
    pub energy: u64,
    pub work_ticks: u64,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cost {
    pub materials: Materials,
    pub energy: u64,
    pub work_ticks: u64,
}
fn pay(materials: &mut Materials, effort: &mut WorkBudget, cost: Cost) -> Result<(), HabitatError> {
    if !materials.can_pay(cost.materials) {
        return Err(HabitatError::InsufficientMaterials);
    }
    if effort.energy < cost.energy || effort.work_ticks < cost.work_ticks {
        return Err(HabitatError::InsufficientWork);
    }
    materials.subtract(cost.materials);
    effort.energy -= cost.energy;
    effort.work_ticks -= cost.work_ticks;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HabitatError {
    MissingRecord,
    InvalidGeometry,
    InsufficientMaterials,
    InsufficientWork,
    IncompatibleSize,
    Occupied,
    CapacityExceeded,
    NotConnected,
    Blocked,
    InvalidWater,
    InvalidTrait,
    NoHost,
    NoLivingBuilder,
    HostDestroyed,
    AlreadyPresent,
    Overflow,
    InvalidTick,
    StaleRevision,
    Invariant,
}
impl std::fmt::Display for HabitatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for HabitatError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HabitatEvent {
    /// Requires removal from real in-situ terrain and addition to real spoil.
    TerrainExcavated {
        patch: PatchId,
        centerline: Vec<Point3Mm>,
        volume_mm3: u64,
    },
    TerrainDepressed {
        patch: PatchId,
        position: Point3Mm,
        depth_mm: u32,
        soil_mm3: u64,
    },
    TerrainBackfilled {
        patch: PatchId,
        position: Point3Mm,
        volume_mm3: u64,
    },
    SoilMixed {
        patch: PatchId,
        exchanged_mm3: u64,
        nutrients_moved: u64,
        porosity_delta: i16,
    },
    CanopyThinned {
        patch: PatchId,
        host: HabitatId,
        biomass_units: u64,
    },
    HostExcavated {
        patch: PatchId,
        host: HabitatId,
        removed_wood_units: u64,
    },
    DamBarrierChanged {
        patch: PatchId,
        dam: HabitatId,
        crest_mm: i32,
        permeability_permille: u16,
    },
    ReefClearanceChanged {
        patch: PatchId,
        reef: HabitatId,
        solid_height_mm: u32,
    },
    OccupantExposed {
        organism: OrganismId,
        patch: PatchId,
        fall_mm: u32,
        reason: ExposureReason,
    },
    OccupantFlooded {
        organism: OrganismId,
        patch: PatchId,
        depth_mm: u32,
    },
    ClutchExposed {
        clutch: u64,
        eggs: u32,
        patch: PatchId,
    },
    PreyIntercepted {
        prey: OrganismId,
        trap: HabitatId,
        patch: PatchId,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExposureReason {
    RoofCollapse,
    HostLoss,
    SupportLoss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Entrance,
    Passage,
    Chamber,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurrowNode {
    pub id: HabitatId,
    pub patch: PatchId,
    pub position: Point3Mm,
    pub kind: NodeKind,
    pub aperture_mm: u32,
    pub capacity: u32,
    pub surface_z_mm: i32,
    pub collapsed: bool,
    /// Derived from the authoritative water-level input, not an owned volume.
    pub flooded_depth_mm: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passage {
    pub id: HabitatId,
    pub from: HabitatId,
    pub to: HabitatId,
    pub centerline: Vec<Point3Mm>,
    pub aperture_mm: u32,
    pub length_mm: u64,
    pub excavated_mm3: u64,
    pub blocked: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Occupancy {
    Underground(HabitatId),
    Canopy(HabitatId),
    Shelter(HabitatId),
    Trap(HabitatId),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Occupant {
    pub organism: OrganismId,
    pub body_width_mm: u32,
    /// Reservation count; the population remains owned by the ecology ledger.
    pub group_size: u32,
    pub location: Occupancy,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShelterKind {
    Den,
    Nest,
    Cavity,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shelter {
    pub id: HabitatId,
    pub patch: PatchId,
    pub kind: ShelterKind,
    pub host: Option<HabitatId>,
    pub burrow_node: Option<HabitatId>,
    pub capacity: u32,
    pub aperture_mm: u32,
    pub material: Materials,
    pub integrity: u32,
    pub clutch: Option<(u64, u32)>,
    pub flooded: bool,
    pub flood_crest_mm: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Support {
    pub id: HabitatId,
    pub patch: PatchId,
    pub position: Point3Mm,
    pub height_mm: u32,
    pub capacity: u32,
    /// Host identity references the biological owner; this is not another tree.
    pub host_alive: bool,
    pub links: BTreeSet<HabitatId>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrapKind {
    Web,
    Pit,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trap {
    pub id: HabitatId,
    pub patch: PatchId,
    pub kind: TrapKind,
    pub max_prey_width_mm: u32,
    pub capacity: u32,
    pub integrity: u32,
    pub materials: Materials,
    pub depth_mm: u32,
    pub position: Point3Mm,
    pub excavated_mm3: u64,
    pub backfilled_mm3: u64,
    pub trapped: BTreeSet<OrganismId>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallow {
    pub id: HabitatId,
    pub patch: PatchId,
    pub position: Point3Mm,
    pub uses: u32,
    pub depth_mm: u32,
    pub max_depth_mm: u32,
    pub disturbed_area_mm2: u64,
    pub displaced_soil_mm3: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dam {
    pub id: HabitatId,
    pub patch: PatchId,
    pub crest_mm: i32,
    pub invested_wood_units: u64,
    pub required_wood_units: u64,
    pub integrity: u32,
    pub open_permeability_permille: u16,
    pub closed_permeability_permille: u16,
}
impl Dam {
    pub fn completed(&self) -> bool {
        self.invested_wood_units >= self.required_wood_units && self.integrity > 0
    }
    pub fn permeability_permille(&self) -> u16 {
        if self.integrity == 0 {
            return 1000;
        }
        let fraction = self.invested_wood_units.min(self.required_wood_units) * 1000
            / self.required_wood_units;
        let span = u64::from(self.open_permeability_permille - self.closed_permeability_permille);
        self.open_permeability_permille
            - (span * fraction * u64::from(self.integrity) / 100_000) as u16
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reef {
    pub id: HabitatId,
    pub patch: PatchId,
    pub base_depth_mm: u32,
    pub solid_height_mm: u32,
    pub mineral_units: u64,
    pub living_builder_units: u32,
    pub shelter_capacity: u32,
}
impl Reef {
    pub fn clearance_mm(&self) -> u32 {
        self.base_depth_mm.saturating_sub(self.solid_height_mm)
    }
    pub fn permits_draft(&self, draft_mm: u32) -> bool {
        draft_mm <= self.clearance_mm()
    }
}

/// Snapshot of a caller-owned site. Excavation/host damage are returned as
/// explicit events; the caller must atomically apply them to the actual owner.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Site {
    pub patch: PatchId,
    pub position_mm: [i32; 2],
    pub surface_z_mm: i32,
    pub water_depth_mm: u32,
    pub host_id: Option<HabitatId>,
    pub host_height_mm: u32,
    /// Finite source views; campaign validates and commits event withdrawals.
    pub available_substrate_mm3: u64,
    pub host_biomass_units: u64,
    pub group_size: u32,
    pub body_width_mm: u32,
}
impl Site {
    pub fn surface(self) -> Point3Mm {
        Point3Mm::new(self.position_mm[0], self.position_mm[1], self.surface_z_mm)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoilLayer {
    pub mineral_mm3: u64,
    pub nutrients: u64,
    pub porosity_permille: u16,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HabitatWorld {
    next_id: HabitatId,
    pub revision: u64,
    pub last_hazard_tick: u64,
    pub nodes: BTreeMap<HabitatId, BurrowNode>,
    pub passages: BTreeMap<HabitatId, Passage>,
    pub occupants: BTreeMap<OrganismId, Occupant>,
    pub shelters: BTreeMap<HabitatId, Shelter>,
    pub supports: BTreeMap<HabitatId, Support>,
    pub traps: BTreeMap<HabitatId, Trap>,
    pub wallows: BTreeMap<HabitatId, Wallow>,
    pub dams: BTreeMap<HabitatId, Dam>,
    pub reefs: BTreeMap<HabitatId, Reef>,
    pub debris: Materials,
}
impl Default for HabitatWorld {
    fn default() -> Self {
        Self {
            next_id: 1,
            revision: 0,
            last_hazard_tick: 0,
            nodes: BTreeMap::new(),
            passages: BTreeMap::new(),
            occupants: BTreeMap::new(),
            shelters: BTreeMap::new(),
            supports: BTreeMap::new(),
            traps: BTreeMap::new(),
            wallows: BTreeMap::new(),
            dams: BTreeMap::new(),
            reefs: BTreeMap::new(),
            debris: Materials::default(),
        }
    }
}
/// Stale preparation is rejected before replacing any live state or wallet.
#[derive(Debug, Clone)]
pub struct PreparedEngineering {
    base_revision: u64,
    before_materials: Materials,
    before_effort: WorkBudget,
    after_materials: Materials,
    after_effort: WorkBudget,
    events: Vec<HabitatEvent>,
    staged: HabitatWorld,
}
impl PreparedEngineering {
    pub fn events(&self) -> &[HabitatEvent] {
        &self.events
    }
    pub fn energy_spent(&self) -> u64 {
        self.before_effort.energy - self.after_effort.energy
    }
    pub fn work_ticks_spent(&self) -> u64 {
        self.before_effort.work_ticks - self.after_effort.work_ticks
    }
    pub fn resulting_materials(&self) -> Materials {
        self.after_materials
    }
}
fn checked_length(points: &[Point3Mm]) -> Result<u64, HabitatError> {
    if points.len() < 2 || points.len() > MAX_CENTERLINE_POINTS {
        return Err(HabitatError::InvalidGeometry);
    }
    let mut total = 0u64;
    for p in points.windows(2) {
        let dx = i64::from(p[1].x) - i64::from(p[0].x);
        let dy = i64::from(p[1].y) - i64::from(p[0].y);
        let dz = i64::from(p[1].z) - i64::from(p[0].z);
        let square = i128::from(dx) * i128::from(dx)
            + i128::from(dy) * i128::from(dy)
            + i128::from(dz) * i128::from(dz);
        // Integer square root gives deterministic geometry, no platform floats.
        let length = (square as u128).isqrt() as u64;
        if length == 0 {
            return Err(HabitatError::InvalidGeometry);
        }
        total = total.checked_add(length).ok_or(HabitatError::Overflow)?;
    }
    Ok(total)
}
impl HabitatWorld {
    fn id(&mut self) -> HabitatId {
        let n = self.next_id;
        self.next_id += 1;
        n
    }
    fn preflight_ids(&self, count: u64) -> Result<(), HabitatError> {
        self.next_id
            .checked_add(count)
            .ok_or(HabitatError::Overflow)?;
        self.revision.checked_add(1).ok_or(HabitatError::Overflow)?;
        Ok(())
    }
    fn changed(&mut self) {
        self.revision += 1;
    }
    fn occupied_node(&self, node: HabitatId) -> u64 {
        self.occupants
            .values()
            .filter(|o| match o.location {
                Occupancy::Underground(id) => id == node,
                Occupancy::Shelter(id) => self
                    .shelters
                    .get(&id)
                    .is_some_and(|s| s.burrow_node == Some(node)),
                _ => false,
            })
            .map(|o| u64::from(o.group_size))
            .sum::<u64>()
            + self
                .shelters
                .values()
                .filter(|s| s.burrow_node == Some(node))
                .filter_map(|s| s.clutch.map(|(_, count)| u64::from(count)))
                .sum::<u64>()
    }
    fn occupied(&self, location: Occupancy) -> u64 {
        self.occupants
            .values()
            .filter(|o| o.location == location)
            .map(|o| u64::from(o.group_size))
            .sum()
    }
    pub fn add_entrance(
        &mut self,
        patch: PatchId,
        pos: Point3Mm,
        aperture_mm: u32,
        capacity: u32,
    ) -> Result<HabitatId, HabitatError> {
        if aperture_mm == 0 || capacity == 0 {
            return Err(HabitatError::InvalidGeometry);
        }
        self.preflight_ids(1)?;
        let id = self.id();
        self.nodes.insert(
            id,
            BurrowNode {
                id,
                patch,
                position: pos,
                kind: NodeKind::Entrance,
                aperture_mm,
                capacity,
                surface_z_mm: pos.z,
                collapsed: false,
                flooded_depth_mm: 0,
            },
        );
        self.changed();
        Ok(id)
    }
    /// Excavates a real bent/sloped centreline. Spoil is handed to the caller's
    /// wallet exactly once; the returned event identifies the source substrate.
    pub fn excavate_passage(
        &mut self,
        from: HabitatId,
        centerline: Vec<Point3Mm>,
        aperture_mm: u32,
        chamber_capacity: u32,
        surface_z_mm: i32,
        available_substrate_mm3: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<(HabitatId, HabitatId, HabitatEvent), HabitatError> {
        let start = self.nodes.get(&from).ok_or(HabitatError::MissingRecord)?;
        if start.collapsed
            || start.position != *centerline.first().ok_or(HabitatError::InvalidGeometry)?
            || aperture_mm == 0
            || chamber_capacity == 0
        {
            return Err(HabitatError::InvalidGeometry);
        }
        let end = *centerline.last().ok_or(HabitatError::InvalidGeometry)?;
        if i64::from(surface_z_mm) - i64::from(end.z) < i64::from(aperture_mm) + 200 {
            return Err(HabitatError::InvalidGeometry);
        }
        let length = checked_length(&centerline)?;
        if self
            .passages
            .values()
            .any(|p| p.centerline == centerline || p.centerline.iter().eq(centerline.iter().rev()))
        {
            return Err(HabitatError::AlreadyPresent);
        }
        let volume = length
            .checked_mul(u64::from(aperture_mm))
            .and_then(|v| v.checked_mul(u64::from(aperture_mm)))
            .ok_or(HabitatError::Overflow)?;
        if available_substrate_mm3 < volume {
            return Err(HabitatError::InsufficientMaterials);
        }
        let spoil = materials
            .soil_mm3
            .checked_add(volume)
            .ok_or(HabitatError::Overflow)?;
        let cost = Cost {
            energy: length.div_ceil(50),
            work_ticks: length.div_ceil(10),
            ..Cost::default()
        };
        self.preflight_ids(2)?;
        let patch = start.patch;
        pay(materials, effort, cost)?;
        materials.soil_mm3 = spoil;
        let node = self.id();
        let edge = self.id();
        self.nodes.insert(
            node,
            BurrowNode {
                id: node,
                patch,
                position: end,
                kind: NodeKind::Chamber,
                aperture_mm,
                capacity: chamber_capacity,
                surface_z_mm,
                collapsed: false,
                flooded_depth_mm: 0,
            },
        );
        self.passages.insert(
            edge,
            Passage {
                id: edge,
                from,
                to: node,
                centerline: centerline.clone(),
                aperture_mm,
                length_mm: length,
                excavated_mm3: volume,
                blocked: false,
            },
        );
        self.changed();
        Ok((
            node,
            edge,
            HabitatEvent::TerrainExcavated {
                patch,
                centerline,
                volume_mm3: volume,
            },
        ))
    }
    /// Connect two already excavated nodes using an explicitly paid passage.
    pub fn connect_nodes(
        &mut self,
        from: HabitatId,
        to: HabitatId,
        centerline: Vec<Point3Mm>,
        aperture_mm: u32,
        available_substrate_mm3: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<(HabitatId, HabitatEvent), HabitatError> {
        let a = self.nodes.get(&from).ok_or(HabitatError::MissingRecord)?;
        let b = self.nodes.get(&to).ok_or(HabitatError::MissingRecord)?;
        if from == to
            || a.patch != b.patch
            || a.collapsed
            || b.collapsed
            || aperture_mm == 0
            || centerline.first() != Some(&a.position)
            || centerline.last() != Some(&b.position)
        {
            return Err(HabitatError::InvalidGeometry);
        }
        let length = checked_length(&centerline)?;
        if self
            .passages
            .values()
            .any(|p| p.centerline == centerline || p.centerline.iter().eq(centerline.iter().rev()))
        {
            return Err(HabitatError::AlreadyPresent);
        }
        let volume = length
            .checked_mul(u64::from(aperture_mm))
            .and_then(|v| v.checked_mul(u64::from(aperture_mm)))
            .ok_or(HabitatError::Overflow)?;
        if volume > available_substrate_mm3 {
            return Err(HabitatError::InsufficientMaterials);
        }
        let spoil = materials
            .soil_mm3
            .checked_add(volume)
            .ok_or(HabitatError::Overflow)?;
        self.preflight_ids(1)?;
        let patch = a.patch;
        pay(
            materials,
            effort,
            Cost {
                energy: length.div_ceil(50),
                work_ticks: length.div_ceil(10),
                ..Cost::default()
            },
        )?;
        materials.soil_mm3 = spoil;
        let id = self.id();
        self.passages.insert(
            id,
            Passage {
                id,
                from,
                to,
                centerline: centerline.clone(),
                aperture_mm,
                length_mm: length,
                excavated_mm3: volume,
                blocked: false,
            },
        );
        self.changed();
        Ok((
            id,
            HabitatEvent::TerrainExcavated {
                patch,
                centerline,
                volume_mm3: volume,
            },
        ))
    }
    pub fn route(
        &self,
        from: HabitatId,
        to: HabitatId,
        body_width_mm: u32,
    ) -> Result<Vec<HabitatId>, HabitatError> {
        let eligible = |id: HabitatId| {
            self.nodes.get(&id).is_some_and(|n| {
                !n.collapsed && n.flooded_depth_mm < body_width_mm && n.aperture_mm >= body_width_mm
            })
        };
        if body_width_mm == 0 || !eligible(from) || !eligible(to) {
            return Err(HabitatError::IncompatibleSize);
        }
        let mut queue = VecDeque::from([from]);
        let mut previous = BTreeMap::from([(from, from)]);
        while let Some(id) = queue.pop_front() {
            if id == to {
                break;
            }
            for edge in self.passages.values() {
                if edge.blocked || edge.aperture_mm < body_width_mm {
                    continue;
                }
                let next = if edge.from == id {
                    edge.to
                } else if edge.to == id {
                    edge.from
                } else {
                    continue;
                };
                if eligible(next) && !previous.contains_key(&next) {
                    previous.insert(next, id);
                    queue.push_back(next);
                }
            }
        }
        if !previous.contains_key(&to) {
            return Err(HabitatError::NotConnected);
        }
        let mut path = vec![to];
        let mut at = to;
        while at != from {
            at = previous[&at];
            path.push(at);
        }
        path.reverse();
        Ok(path)
    }
    pub fn enter_burrow(
        &mut self,
        organism: OrganismId,
        group_size: u32,
        body_width_mm: u32,
        entrance: HabitatId,
    ) -> Result<(), HabitatError> {
        if self.occupants.contains_key(&organism) {
            return Err(HabitatError::AlreadyPresent);
        }
        let node = self
            .nodes
            .get(&entrance)
            .ok_or(HabitatError::MissingRecord)?;
        if node.kind != NodeKind::Entrance
            || node.collapsed
            || node.flooded_depth_mm >= body_width_mm
        {
            return Err(HabitatError::Blocked);
        }
        if body_width_mm == 0 || body_width_mm > node.aperture_mm || group_size == 0 {
            return Err(HabitatError::IncompatibleSize);
        }
        if self.occupied_node(entrance) + u64::from(group_size) > u64::from(node.capacity) {
            return Err(HabitatError::CapacityExceeded);
        }
        self.preflight_ids(0)?;
        self.occupants.insert(
            organism,
            Occupant {
                organism,
                body_width_mm,
                group_size,
                location: Occupancy::Underground(entrance),
            },
        );
        self.changed();
        Ok(())
    }
    pub fn travel_burrow(
        &mut self,
        organism: OrganismId,
        destination: HabitatId,
        effort: &mut WorkBudget,
    ) -> Result<Vec<HabitatId>, HabitatError> {
        let o = self
            .occupants
            .get(&organism)
            .ok_or(HabitatError::MissingRecord)?;
        let Occupancy::Underground(from) = o.location else {
            return Err(HabitatError::NotConnected);
        };
        let path = self.route(from, destination, o.body_width_mm)?;
        let occupied = self.occupied_node(destination);
        let own = if from == destination {
            u64::from(o.group_size)
        } else {
            0
        };
        if occupied - own + u64::from(o.group_size) > u64::from(self.nodes[&destination].capacity) {
            return Err(HabitatError::CapacityExceeded);
        }
        let mut distance = 0u64;
        for pair in path.windows(2) {
            let length = self
                .passages
                .values()
                .filter(|e| {
                    !e.blocked
                        && e.aperture_mm >= o.body_width_mm
                        && ((e.from == pair[0] && e.to == pair[1])
                            || (e.to == pair[0] && e.from == pair[1]))
                })
                .map(|e| e.length_mm)
                .min()
                .ok_or(HabitatError::NotConnected)?;
            distance = distance.checked_add(length).ok_or(HabitatError::Overflow)?;
        }
        let energy = distance
            .div_ceil(1000)
            .checked_mul(u64::from(o.group_size))
            .ok_or(HabitatError::Overflow)?;
        let ticks = distance.div_ceil(100);
        if effort.energy < energy || effort.work_ticks < ticks {
            return Err(HabitatError::InsufficientWork);
        }
        self.preflight_ids(0)?;
        effort.energy -= energy;
        effort.work_ticks -= ticks;
        self.occupants.get_mut(&organism).unwrap().location = Occupancy::Underground(destination);
        self.changed();
        Ok(path)
    }
    pub fn exit_burrow(&mut self, organism: OrganismId) -> Result<PatchId, HabitatError> {
        let o = self
            .occupants
            .get(&organism)
            .ok_or(HabitatError::MissingRecord)?;
        let Occupancy::Underground(node) = o.location else {
            return Err(HabitatError::NotConnected);
        };
        let n = self.nodes.get(&node).ok_or(HabitatError::MissingRecord)?;
        if n.kind != NodeKind::Entrance || n.collapsed {
            return Err(HabitatError::Blocked);
        }
        self.preflight_ids(0)?;
        let patch = n.patch;
        self.occupants.remove(&organism);
        self.changed();
        Ok(patch)
    }
    pub fn set_passage_blocked(
        &mut self,
        id: HabitatId,
        blocked: bool,
    ) -> Result<(), HabitatError> {
        if !self.passages.contains_key(&id) {
            return Err(HabitatError::MissingRecord);
        }
        self.preflight_ids(0)?;
        self.passages.get_mut(&id).unwrap().blocked = blocked;
        self.changed();
        Ok(())
    }
    /// Accept water surfaces from the finite hydraulic owner. Only an open
    /// connected entrance can propagate a surface-water flood into the graph.
    pub fn update_flood_hazards(
        &mut self,
        tick: u64,
        surfaces: &[(PatchId, i32)],
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        if tick <= self.last_hazard_tick {
            return Err(HabitatError::InvalidTick);
        }
        self.preflight_ids(0)?;
        let levels: BTreeMap<PatchId, i32> = surfaces.iter().copied().collect();
        if levels.len() != surfaces.len() {
            return Err(HabitatError::InvalidWater);
        }
        let mut reached = BTreeMap::<HabitatId, i32>::new();
        let mut queue = VecDeque::new();
        for n in self.nodes.values() {
            if n.kind == NodeKind::Entrance && !n.collapsed {
                if let Some(&level) = levels.get(&n.patch) {
                    if level > n.position.z {
                        reached.insert(n.id, level);
                        queue.push_back(n.id);
                    }
                }
            }
        }
        while let Some(id) = queue.pop_front() {
            let level = reached[&id];
            for e in self.passages.values() {
                if e.blocked || e.centerline.iter().any(|p| p.z >= level) {
                    continue;
                }
                let next = if e.from == id {
                    e.to
                } else if e.to == id {
                    e.from
                } else {
                    continue;
                };
                if self.nodes[&next].collapsed
                    || reached.get(&next).is_some_and(|&old| old >= level)
                {
                    continue;
                }
                reached.insert(next, level);
                queue.push_back(next);
            }
        }
        for n in self.nodes.values_mut() {
            n.flooded_depth_mm = reached.get(&n.id).map_or(0, |&level| {
                (i64::from(level) - i64::from(n.position.z))
                    .max(0)
                    .min(i64::from(u32::MAX)) as u32
            });
        }
        let mut events = Vec::new();
        for o in self.occupants.values() {
            if let Occupancy::Underground(id) = o.location {
                let n = &self.nodes[&id];
                if n.flooded_depth_mm >= o.body_width_mm {
                    events.push(HabitatEvent::OccupantFlooded {
                        organism: o.organism,
                        patch: n.patch,
                        depth_mm: n.flooded_depth_mm,
                    });
                }
            }
        }
        for s in self.shelters.values_mut() {
            s.flooded = s
                .burrow_node
                .is_some_and(|id| self.nodes[&id].flooded_depth_mm >= s.aperture_mm)
                || s.burrow_node.is_none()
                    && levels.get(&s.patch).is_some_and(|&z| z > s.flood_crest_mm);
            if s.flooded {
                if let Some((clutch, eggs)) = s.clutch.take() {
                    events.push(HabitatEvent::ClutchExposed {
                        clutch,
                        eggs,
                        patch: s.patch,
                    });
                }
            }
        }
        for o in self.occupants.values() {
            if let Occupancy::Shelter(id) = o.location {
                let s = &self.shelters[&id];
                if s.flooded {
                    let depth = if let Some(n) = s.burrow_node {
                        self.nodes[&n].flooded_depth_mm
                    } else {
                        (i64::from(levels[&s.patch]) - i64::from(s.flood_crest_mm)).max(0) as u32
                    };
                    events.push(HabitatEvent::OccupantFlooded {
                        organism: o.organism,
                        patch: s.patch,
                        depth_mm: depth,
                    });
                }
            }
        }
        self.last_hazard_tick = tick;
        self.changed();
        Ok(events)
    }
    /// A crater/cut whose floor intersects a roof permanently collapses the
    /// affected node. Occupancy references are removed and explicit harm is
    /// returned; the caller alone decides actual animal survival/death.
    pub fn excavate_surface(
        &mut self,
        patch: PatchId,
        new_surface_z_mm: i32,
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        self.excavate_surface_at(
            patch,
            [i32::MIN, i32::MIN],
            [i32::MAX, i32::MAX],
            new_surface_z_mm,
        )
    }
    /// A local cut tests only overlapping node apertures or passage-segment
    /// bounds; another burrow elsewhere in the macro patch stays untouched.
    pub fn excavate_surface_at(
        &mut self,
        patch: PatchId,
        min_xy_mm: [i32; 2],
        max_xy_mm: [i32; 2],
        new_surface_z_mm: i32,
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        if min_xy_mm[0] > max_xy_mm[0] || min_xy_mm[1] > max_xy_mm[1] {
            return Err(HabitatError::InvalidGeometry);
        }
        self.preflight_ids(0)?;
        let ids: BTreeSet<_> = self
            .nodes
            .values()
            .filter(|n| {
                n.patch == patch
                    && i64::from(n.position.x) + i64::from(n.aperture_mm) / 2
                        >= i64::from(min_xy_mm[0])
                    && i64::from(n.position.x) - i64::from(n.aperture_mm) / 2
                        <= i64::from(max_xy_mm[0])
                    && i64::from(n.position.y) + i64::from(n.aperture_mm) / 2
                        >= i64::from(min_xy_mm[1])
                    && i64::from(n.position.y) - i64::from(n.aperture_mm) / 2
                        <= i64::from(max_xy_mm[1])
                    && n.kind != NodeKind::Entrance
                    && !n.collapsed
                    && i64::from(new_surface_z_mm)
                        <= i64::from(n.position.z) + i64::from(n.aperture_mm) + 200
            })
            .map(|n| n.id)
            .collect();
        for id in &ids {
            self.nodes.get_mut(id).unwrap().collapsed = true;
        }
        for edge in self.passages.values_mut() {
            let same_patch = self.nodes[&edge.from].patch == patch;
            let cut_roof = same_patch
                && edge.centerline.windows(2).any(|pair| {
                    let radius = i64::from(edge.aperture_mm) / 2;
                    let overlap = i64::from(pair[0].x.min(pair[1].x)) - radius
                        <= i64::from(max_xy_mm[0])
                        && i64::from(pair[0].x.max(pair[1].x)) + radius >= i64::from(min_xy_mm[0])
                        && i64::from(pair[0].y.min(pair[1].y)) - radius <= i64::from(max_xy_mm[1])
                        && i64::from(pair[0].y.max(pair[1].y)) + radius >= i64::from(min_xy_mm[1]);
                    overlap
                        && i64::from(new_surface_z_mm)
                            <= i64::from(pair[0].z.max(pair[1].z))
                                + i64::from(edge.aperture_mm)
                                + 200
                });
            if ids.contains(&edge.from) || ids.contains(&edge.to) || cut_roof {
                edge.blocked = true;
            }
        }
        let exposed: Vec<_> = self.occupants.values().filter(|o| matches!(o.location, Occupancy::Underground(id) if ids.contains(&id))
            || matches!(o.location, Occupancy::Shelter(id) if self.shelters.get(&id).is_some_and(|s| s.burrow_node.is_some_and(|n| ids.contains(&n)))))
            .map(|o| o.organism).collect();
        let mut events: Vec<_> = exposed
            .iter()
            .map(|&organism| HabitatEvent::OccupantExposed {
                organism,
                patch,
                fall_mm: 0,
                reason: ExposureReason::RoofCollapse,
            })
            .collect();
        for id in exposed {
            self.occupants.remove(&id);
        }
        for s in self.shelters.values_mut() {
            if s.burrow_node.is_some_and(|n| ids.contains(&n)) {
                s.integrity = 0;
                if let Some((clutch, eggs)) = s.clutch.take() {
                    events.push(HabitatEvent::ClutchExposed {
                        clutch,
                        eggs,
                        patch,
                    });
                }
            }
        }
        self.changed();
        Ok(events)
    }
    pub fn register_support(
        &mut self,
        host_id: HabitatId,
        patch: PatchId,
        position: Point3Mm,
        height_mm: u32,
        capacity: u32,
    ) -> Result<(), HabitatError> {
        if height_mm == 0 || capacity == 0 || self.supports.contains_key(&host_id) {
            return Err(HabitatError::InvalidGeometry);
        }
        self.preflight_ids(0)?;
        self.supports.insert(
            host_id,
            Support {
                id: host_id,
                patch,
                position,
                height_mm,
                capacity,
                host_alive: true,
                links: BTreeSet::new(),
            },
        );
        self.changed();
        Ok(())
    }
    pub fn link_supports(
        &mut self,
        a: HabitatId,
        b: HabitatId,
        max_reach_mm: u32,
    ) -> Result<(), HabitatError> {
        let sa = self.supports.get(&a).ok_or(HabitatError::NoHost)?;
        let sb = self.supports.get(&b).ok_or(HabitatError::NoHost)?;
        if a == b || !sa.host_alive || !sb.host_alive || sa.patch != sb.patch {
            return Err(HabitatError::InvalidGeometry);
        }
        let length = checked_length(&[sa.position, sb.position])?;
        if length > u64::from(max_reach_mm) {
            return Err(HabitatError::NotConnected);
        }
        self.preflight_ids(0)?;
        self.supports.get_mut(&a).unwrap().links.insert(b);
        self.supports.get_mut(&b).unwrap().links.insert(a);
        self.changed();
        Ok(())
    }
    pub fn climb(
        &mut self,
        organism: OrganismId,
        group_size: u32,
        body_width_mm: u32,
        support: HabitatId,
        effort: &mut WorkBudget,
    ) -> Result<(), HabitatError> {
        let s = self.supports.get(&support).ok_or(HabitatError::NoHost)?;
        if !s.host_alive {
            return Err(HabitatError::HostDestroyed);
        }
        if group_size == 0 || body_width_mm == 0 || body_width_mm > 1000 {
            return Err(HabitatError::IncompatibleSize);
        }
        if let Some(o) = self.occupants.get(&organism) {
            let Occupancy::Canopy(old) = o.location else {
                return Err(HabitatError::Occupied);
            };
            if old == support {
                return Err(HabitatError::AlreadyPresent);
            }
            if !self.supports[&old].links.contains(&support) {
                return Err(HabitatError::NotConnected);
            }
            if o.group_size != group_size {
                return Err(HabitatError::Invariant);
            }
        }
        if self.occupied(Occupancy::Canopy(support)) + u64::from(group_size) > u64::from(s.capacity)
        {
            return Err(HabitatError::CapacityExceeded);
        }
        let energy = u64::from(s.height_mm)
            .div_ceil(1000)
            .checked_mul(u64::from(group_size))
            .ok_or(HabitatError::Overflow)?;
        let ticks = u64::from(s.height_mm).div_ceil(100);
        if effort.energy < energy || effort.work_ticks < ticks {
            return Err(HabitatError::InsufficientWork);
        }
        self.preflight_ids(0)?;
        effort.energy -= energy;
        effort.work_ticks -= ticks;
        self.occupants.insert(
            organism,
            Occupant {
                organism,
                body_width_mm,
                group_size,
                location: Occupancy::Canopy(support),
            },
        );
        self.changed();
        Ok(())
    }
    pub fn destroy_host(&mut self, host: HabitatId) -> Result<Vec<HabitatEvent>, HabitatError> {
        let s = self.supports.get(&host).ok_or(HabitatError::NoHost)?;
        if !s.host_alive {
            return Err(HabitatError::HostDestroyed);
        }
        let patch = s.patch;
        let height = s.height_mm;
        let mut added = Materials::default();
        for shelter in self
            .shelters
            .values()
            .filter(|s| s.host == Some(host) && s.integrity > 0)
        {
            added = added.checked_add(shelter.material)?;
        }
        let debris = self.debris.checked_add(added)?;
        self.preflight_ids(0)?;
        let shelters: BTreeSet<_> = self
            .shelters
            .values()
            .filter(|s| s.host == Some(host))
            .map(|s| s.id)
            .collect();
        let ids: Vec<_> = self
            .occupants
            .values()
            .filter(|o| {
                o.location == Occupancy::Canopy(host)
                    || matches!(o.location,Occupancy::Shelter(id) if shelters.contains(&id))
            })
            .map(|o| o.organism)
            .collect();
        let mut events = Vec::new();
        for organism in ids {
            self.occupants.remove(&organism);
            events.push(HabitatEvent::OccupantExposed {
                organism,
                patch,
                fall_mm: height,
                reason: ExposureReason::HostLoss,
            });
        }
        for shelter in self.shelters.values_mut().filter(|s| s.host == Some(host)) {
            if let Some((clutch, eggs)) = shelter.clutch.take() {
                events.push(HabitatEvent::ClutchExposed {
                    clutch,
                    eggs,
                    patch,
                });
            }
            shelter.integrity = 0;
            shelter.material = Materials::default();
        }
        self.supports.get_mut(&host).unwrap().host_alive = false;
        self.supports.get_mut(&host).unwrap().links.clear();
        for support in self.supports.values_mut() {
            support.links.remove(&host);
        }
        self.debris = debris;
        self.changed();
        Ok(events)
    }
    pub fn create_shelter(
        &mut self,
        patch: PatchId,
        kind: ShelterKind,
        host: Option<HabitatId>,
        burrow_node: Option<HabitatId>,
        capacity: u32,
        aperture_mm: u32,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<(HabitatId, Vec<HabitatEvent>), HabitatError> {
        if capacity == 0 || aperture_mm == 0 {
            return Err(HabitatError::InvalidGeometry);
        }
        if let Some(host) = host {
            let s = self.supports.get(&host).ok_or(HabitatError::NoHost)?;
            if !s.host_alive || s.patch != patch {
                return Err(HabitatError::HostDestroyed);
            }
        }
        if let Some(id) = burrow_node {
            let n = self.nodes.get(&id).ok_or(HabitatError::MissingRecord)?;
            if n.patch != patch || n.collapsed || n.flooded_depth_mm >= aperture_mm {
                return Err(HabitatError::Blocked);
            }
            if capacity > n.capacity || aperture_mm > n.aperture_mm {
                return Err(HabitatError::CapacityExceeded);
            }
        }
        if kind == ShelterKind::Den && burrow_node.is_none() {
            return Err(HabitatError::InvalidGeometry);
        }
        if kind == ShelterKind::Cavity && host.is_none() {
            return Err(HabitatError::NoHost);
        }
        let material = match kind {
            ShelterKind::Den => Materials::default(),
            ShelterKind::Nest => Materials {
                wood_units: 2,
                fiber_units: 4,
                ..Materials::default()
            },
            ShelterKind::Cavity => Materials::default(),
        };
        let cost = Cost {
            materials: material,
            energy: 5 + u64::from(capacity),
            work_ticks: 40 + u64::from(capacity) * 4,
        };
        let flood_crest_mm = if let Some(node) = burrow_node {
            self.nodes[&node].position.z
        } else if let Some(host) = host {
            let s = &self.supports[&host];
            s.position
                .z
                .checked_add(i32::try_from(s.height_mm).map_err(|_| HabitatError::Overflow)?)
                .ok_or(HabitatError::Overflow)?
        } else {
            0
        };
        self.preflight_ids(1)?;
        pay(materials, effort, cost)?;
        let id = self.id();
        self.shelters.insert(
            id,
            Shelter {
                id,
                patch,
                kind,
                host,
                burrow_node,
                capacity,
                aperture_mm,
                material,
                integrity: 100,
                clutch: None,
                flooded: false,
                flood_crest_mm,
            },
        );
        self.changed();
        let events = if kind == ShelterKind::Cavity {
            vec![HabitatEvent::HostExcavated {
                patch,
                host: host.unwrap(),
                removed_wood_units: u64::from(capacity),
            }]
        } else {
            vec![]
        };
        Ok((id, events))
    }
    pub fn reserve_shelter(
        &mut self,
        id: HabitatId,
        organism: OrganismId,
        group_size: u32,
        body_width_mm: u32,
    ) -> Result<(), HabitatError> {
        let s = self.shelters.get(&id).ok_or(HabitatError::MissingRecord)?;
        if let Some(node) = s.burrow_node {
            let o = self
                .occupants
                .get(&organism)
                .ok_or(HabitatError::NotConnected)?;
            if o.location != Occupancy::Underground(node) {
                return Err(HabitatError::NotConnected);
            }
            if o.group_size != group_size || o.body_width_mm != body_width_mm {
                return Err(HabitatError::Invariant);
            }
        } else if self.occupants.contains_key(&organism) {
            return Err(HabitatError::AlreadyPresent);
        }
        if s.integrity == 0 || s.flooded {
            return Err(HabitatError::Blocked);
        }
        if group_size == 0 || body_width_mm == 0 || body_width_mm > s.aperture_mm {
            return Err(HabitatError::IncompatibleSize);
        }
        if self.occupied(Occupancy::Shelter(id)) + u64::from(group_size) > u64::from(s.capacity) {
            return Err(HabitatError::CapacityExceeded);
        }
        self.preflight_ids(0)?;
        self.occupants.insert(
            organism,
            Occupant {
                organism,
                body_width_mm,
                group_size,
                location: Occupancy::Shelter(id),
            },
        );
        self.changed();
        Ok(())
    }
    pub fn release_occupant(&mut self, organism: OrganismId) -> Result<Occupant, HabitatError> {
        if !self.occupants.contains_key(&organism) {
            return Err(HabitatError::MissingRecord);
        }
        self.preflight_ids(0)?;
        let o = self.occupants.remove(&organism).unwrap();
        if let Occupancy::Trap(id) = o.location {
            if let Some(t) = self.traps.get_mut(&id) {
                t.trapped.remove(&organism);
            }
        }
        self.changed();
        Ok(o)
    }
    pub fn attach_clutch(
        &mut self,
        nest: HabitatId,
        clutch: u64,
        eggs: u32,
    ) -> Result<(), HabitatError> {
        let s = self
            .shelters
            .get(&nest)
            .ok_or(HabitatError::MissingRecord)?;
        if s.kind != ShelterKind::Nest
            || s.integrity == 0
            || s.flooded
            || s.clutch.is_some()
            || eggs == 0
            || eggs > s.capacity
        {
            return Err(HabitatError::CapacityExceeded);
        }
        if self
            .shelters
            .values()
            .any(|s| s.clutch.is_some_and(|(id, _)| id == clutch))
        {
            return Err(HabitatError::AlreadyPresent);
        }
        if let Some(node) = s.burrow_node {
            if self.occupied_node(node) + u64::from(eggs) > u64::from(self.nodes[&node].capacity) {
                return Err(HabitatError::CapacityExceeded);
            }
        }
        self.preflight_ids(0)?;
        self.shelters.get_mut(&nest).unwrap().clutch = Some((clutch, eggs));
        self.changed();
        Ok(())
    }
    pub fn build_trap(
        &mut self,
        patch: PatchId,
        kind: TrapKind,
        position: Point3Mm,
        max_prey_width_mm: u32,
        capacity: u32,
        available_substrate_mm3: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<(HabitatId, Vec<HabitatEvent>), HabitatError> {
        if max_prey_width_mm == 0 || capacity == 0 {
            return Err(HabitatError::InvalidGeometry);
        }
        if self.traps.values().any(|t| {
            t.patch == patch && t.position == position && t.kind == kind && t.integrity > 0
        }) {
            return Err(HabitatError::AlreadyPresent);
        }
        let (material, depth, soil) = match kind {
            TrapKind::Web => (
                Materials {
                    fiber_units: 8,
                    ..Materials::default()
                },
                0,
                0,
            ),
            TrapKind::Pit => {
                let depth = 600u32;
                let width = u64::from(max_prey_width_mm) * 2;
                let soil = width
                    .checked_mul(width)
                    .and_then(|n| n.checked_mul(u64::from(depth)))
                    .ok_or(HabitatError::Overflow)?;
                if soil > available_substrate_mm3 {
                    return Err(HabitatError::InsufficientMaterials);
                }
                (Materials::default(), depth, soil)
            }
        };
        let spoil = materials
            .soil_mm3
            .checked_add(soil)
            .ok_or(HabitatError::Overflow)?;
        self.preflight_ids(1)?;
        pay(
            materials,
            effort,
            Cost {
                materials: material,
                energy: 20,
                work_ticks: 200,
            },
        )?;
        materials.soil_mm3 = spoil;
        let id = self.id();
        self.traps.insert(
            id,
            Trap {
                id,
                patch,
                kind,
                max_prey_width_mm,
                capacity,
                integrity: 100,
                materials: material,
                depth_mm: depth,
                position,
                excavated_mm3: soil,
                backfilled_mm3: 0,
                trapped: BTreeSet::new(),
            },
        );
        self.changed();
        let events = if soil > 0 {
            vec![HabitatEvent::TerrainDepressed {
                patch,
                position,
                depth_mm: depth,
                soil_mm3: soil,
            }]
        } else {
            vec![]
        };
        Ok((id, events))
    }
    pub fn intercept(
        &mut self,
        trap: HabitatId,
        prey: OrganismId,
        prey_width_mm: u32,
    ) -> Result<HabitatEvent, HabitatError> {
        if self.occupants.contains_key(&prey) {
            return Err(HabitatError::AlreadyPresent);
        }
        let t = self.traps.get(&trap).ok_or(HabitatError::MissingRecord)?;
        if t.integrity == 0 {
            return Err(HabitatError::Blocked);
        }
        if prey_width_mm == 0 || prey_width_mm > t.max_prey_width_mm {
            return Err(HabitatError::IncompatibleSize);
        }
        if t.trapped.len() >= t.capacity as usize {
            return Err(HabitatError::CapacityExceeded);
        }
        self.preflight_ids(0)?;
        let patch = t.patch;
        self.traps.get_mut(&trap).unwrap().trapped.insert(prey);
        self.occupants.insert(
            prey,
            Occupant {
                organism: prey,
                body_width_mm: prey_width_mm,
                group_size: 1,
                location: Occupancy::Trap(trap),
            },
        );
        self.changed();
        Ok(HabitatEvent::PreyIntercepted { prey, trap, patch })
    }
    pub fn destroy_trap(
        &mut self,
        id: HabitatId,
        backfill: &mut Materials,
    ) -> Result<(Vec<OrganismId>, Vec<HabitatEvent>), HabitatError> {
        let t = self.traps.get(&id).ok_or(HabitatError::MissingRecord)?;
        if t.integrity == 0 {
            return Err(HabitatError::Blocked);
        }
        // A pit's opening remains until real fill has paid its exact geometric volume.
        let fill = t
            .excavated_mm3
            .checked_sub(t.backfilled_mm3)
            .ok_or(HabitatError::Invariant)?;
        let events = if fill > 0 {
            vec![HabitatEvent::TerrainBackfilled {
                patch: t.patch,
                position: t.position,
                volume_mm3: fill,
            }]
        } else {
            vec![]
        };
        if backfill.soil_mm3 < fill {
            return Err(HabitatError::InsufficientMaterials);
        }
        let debris = self.debris.checked_add(t.materials)?;
        self.preflight_ids(0)?;
        backfill.soil_mm3 -= fill;
        let ids: Vec<_> = t.trapped.iter().copied().collect();
        for id in &ids {
            self.occupants.remove(id);
        }
        let t = self.traps.get_mut(&id).unwrap();
        t.integrity = 0;
        t.materials = Materials::default();
        t.trapped.clear();
        t.backfilled_mm3 = t.excavated_mm3;
        self.debris = debris;
        self.changed();
        Ok((ids, events))
    }
    pub fn wallow(
        &mut self,
        patch: PatchId,
        position: Point3Mm,
        wet: bool,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        if !wet {
            return Err(HabitatError::InvalidWater);
        }
        let existing = self
            .wallows
            .values()
            .find(|w| w.patch == patch)
            .map(|w| w.id);
        let (uses, depth, max_depth, area) = existing
            .map(|id| {
                let w = &self.wallows[&id];
                (w.uses, w.depth_mm, w.max_depth_mm, w.disturbed_area_mm2)
            })
            .unwrap_or((0, 0, 120, 4_000_000));
        let next_uses = uses.checked_add(1).ok_or(HabitatError::Overflow)?;
        // First contact compacts a wet patch; only repeated use excavates it.
        let increment = if next_uses % 3 == 0 && depth < max_depth {
            5.min(max_depth - depth)
        } else {
            0
        };
        let soil = area
            .checked_mul(u64::from(increment))
            .ok_or(HabitatError::Overflow)?;
        let spoil = materials
            .soil_mm3
            .checked_add(soil)
            .ok_or(HabitatError::Overflow)?;
        self.preflight_ids(u64::from(existing.is_none()))?;
        pay(
            materials,
            effort,
            Cost {
                energy: 2,
                work_ticks: 20,
                ..Cost::default()
            },
        )?;
        materials.soil_mm3 = spoil;
        let id = if let Some(id) = existing {
            id
        } else {
            let id = self.id();
            self.wallows.insert(
                id,
                Wallow {
                    id,
                    patch,
                    position,
                    uses: 0,
                    depth_mm: 0,
                    max_depth_mm: max_depth,
                    disturbed_area_mm2: area,
                    displaced_soil_mm3: 0,
                },
            );
            id
        };
        let w = self.wallows.get_mut(&id).unwrap();
        w.uses = next_uses;
        w.depth_mm += increment;
        w.displaced_soil_mm3 += soil;
        self.changed();
        Ok(if increment > 0 {
            vec![HabitatEvent::TerrainDepressed {
                patch,
                position,
                depth_mm: increment,
                soil_mm3: soil,
            }]
        } else {
            vec![]
        })
    }
    pub fn begin_dam(
        &mut self,
        patch: PatchId,
        crest_mm: i32,
        required_wood_units: u64,
    ) -> Result<HabitatId, HabitatError> {
        if required_wood_units == 0 || required_wood_units > u64::MAX / 1000 {
            return Err(HabitatError::InvalidGeometry);
        }
        self.preflight_ids(1)?;
        let id = self.id();
        self.dams.insert(
            id,
            Dam {
                id,
                patch,
                crest_mm,
                invested_wood_units: 0,
                required_wood_units,
                integrity: 100,
                open_permeability_permille: 1000,
                closed_permeability_permille: 40,
            },
        );
        self.changed();
        Ok(id)
    }
    pub fn build_dam(
        &mut self,
        id: HabitatId,
        wood_units: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<HabitatEvent, HabitatError> {
        let d = self.dams.get(&id).ok_or(HabitatError::MissingRecord)?;
        if d.integrity == 0
            || wood_units == 0
            || wood_units > d.required_wood_units - d.invested_wood_units
        {
            return Err(HabitatError::InvalidGeometry);
        }
        let energy = wood_units.checked_mul(2).ok_or(HabitatError::Overflow)?;
        let ticks = wood_units.checked_mul(10).ok_or(HabitatError::Overflow)?;
        self.preflight_ids(0)?;
        pay(
            materials,
            effort,
            Cost {
                materials: Materials {
                    wood_units,
                    ..Materials::default()
                },
                energy,
                work_ticks: ticks,
            },
        )?;
        let d = self.dams.get_mut(&id).unwrap();
        d.invested_wood_units += wood_units;
        let event = HabitatEvent::DamBarrierChanged {
            patch: d.patch,
            dam: id,
            crest_mm: d.crest_mm,
            permeability_permille: d.permeability_permille(),
        };
        self.changed();
        Ok(event)
    }
    pub fn damage_dam(&mut self, id: HabitatId, damage: u32) -> Result<HabitatEvent, HabitatError> {
        let d = self.dams.get(&id).ok_or(HabitatError::MissingRecord)?;
        let integrity = d.integrity.saturating_sub(damage);
        let debris = if integrity == 0 && d.integrity > 0 {
            self.debris.checked_add(Materials {
                wood_units: d.invested_wood_units,
                ..Materials::default()
            })?
        } else {
            self.debris
        };
        self.preflight_ids(0)?;
        let d = self.dams.get_mut(&id).unwrap();
        d.integrity = integrity;
        if integrity == 0 {
            d.invested_wood_units = 0;
        }
        let event = HabitatEvent::DamBarrierChanged {
            patch: d.patch,
            dam: id,
            crest_mm: d.crest_mm,
            permeability_permille: d.permeability_permille(),
        };
        self.debris = debris;
        self.changed();
        Ok(event)
    }
    /// Replacement wood enters the structure while the same amount of worn
    /// structural wood leaves to debris. Integrity is repaired without losing mass.
    pub fn repair_dam(
        &mut self,
        id: HabitatId,
        replacement_wood_units: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<HabitatEvent, HabitatError> {
        let dam = self.dams.get(&id).ok_or(HabitatError::MissingRecord)?;
        let missing = 100 - dam.integrity;
        if dam.integrity == 0
            || replacement_wood_units == 0
            || replacement_wood_units > u64::from(missing).div_ceil(10)
            || replacement_wood_units > dam.invested_wood_units
        {
            return Err(HabitatError::InvalidGeometry);
        }
        let debris = self.debris.checked_add(Materials {
            wood_units: replacement_wood_units,
            ..Materials::default()
        })?;
        let energy = replacement_wood_units
            .checked_mul(2)
            .ok_or(HabitatError::Overflow)?;
        let work_ticks = replacement_wood_units
            .checked_mul(10)
            .ok_or(HabitatError::Overflow)?;
        self.preflight_ids(0)?;
        pay(
            materials,
            effort,
            Cost {
                materials: Materials {
                    wood_units: replacement_wood_units,
                    ..Materials::default()
                },
                energy,
                work_ticks,
            },
        )?;
        let dam = self.dams.get_mut(&id).unwrap();
        dam.integrity += u32::try_from(replacement_wood_units * 10)
            .map_err(|_| HabitatError::Overflow)?
            .min(missing);
        let event = HabitatEvent::DamBarrierChanged {
            patch: dam.patch,
            dam: id,
            crest_mm: dam.crest_mm,
            permeability_permille: dam.permeability_permille(),
        };
        self.debris = debris;
        self.changed();
        Ok(event)
    }
    pub fn grow_reef(
        &mut self,
        patch: PatchId,
        water_depth_mm: u32,
        living_builder_units: u32,
        mineral_units: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<HabitatEvent, HabitatError> {
        if water_depth_mm < 200 || water_depth_mm > 20_000 {
            return Err(HabitatError::InvalidWater);
        }
        if living_builder_units == 0 || mineral_units == 0 {
            return Err(HabitatError::NoLivingBuilder);
        }
        let existing = self.reefs.values().find(|r| r.patch == patch).map(|r| r.id);
        let (old_height, old_mineral) = existing
            .map(|id| {
                let r = &self.reefs[&id];
                (r.solid_height_mm, r.mineral_units)
            })
            .unwrap_or((0, 0));
        let add = u32::try_from(mineral_units).map_err(|_| HabitatError::Overflow)?;
        let height = old_height.checked_add(add).ok_or(HabitatError::Overflow)?;
        if height >= water_depth_mm {
            return Err(HabitatError::InvalidWater);
        }
        let stock = old_mineral
            .checked_add(mineral_units)
            .ok_or(HabitatError::Overflow)?;
        self.preflight_ids(u64::from(existing.is_none()))?;
        pay(
            materials,
            effort,
            Cost {
                materials: Materials {
                    mineral_units,
                    ..Materials::default()
                },
                energy: mineral_units,
                work_ticks: mineral_units
                    .checked_mul(20)
                    .ok_or(HabitatError::Overflow)?,
            },
        )?;
        let id = if let Some(id) = existing {
            id
        } else {
            let id = self.id();
            self.reefs.insert(
                id,
                Reef {
                    id,
                    patch,
                    base_depth_mm: water_depth_mm,
                    solid_height_mm: 0,
                    mineral_units: 0,
                    living_builder_units: 0,
                    shelter_capacity: 0,
                },
            );
            id
        };
        let r = self.reefs.get_mut(&id).unwrap();
        r.solid_height_mm = height;
        r.base_depth_mm = water_depth_mm;
        r.mineral_units = stock;
        r.living_builder_units = living_builder_units;
        r.shelter_capacity = (height / 10).max(1);
        self.changed();
        Ok(HabitatEvent::ReefClearanceChanged {
            patch,
            reef: id,
            solid_height_mm: height,
        })
    }
    pub fn reef_builders_died(&mut self, id: HabitatId) -> Result<(), HabitatError> {
        if !self.reefs.contains_key(&id) {
            return Err(HabitatError::MissingRecord);
        }
        self.preflight_ids(0)?;
        self.reefs.get_mut(&id).unwrap().living_builder_units = 0;
        self.changed();
        Ok(())
    }
    /// Hydraulic owner updates this derived depth cache; exposed reef solid is
    /// retained when the water recedes. No water volume is created here.
    pub fn update_reef_water_depth(
        &mut self,
        patch: PatchId,
        depth_mm: u32,
    ) -> Result<(), HabitatError> {
        self.preflight_ids(0)?;
        for reef in self.reefs.values_mut().filter(|r| r.patch == patch) {
            reef.base_depth_mm = depth_mm;
        }
        self.changed();
        Ok(())
    }
    pub fn damage_reef(
        &mut self,
        id: HabitatId,
        removed_height_mm: u32,
    ) -> Result<HabitatEvent, HabitatError> {
        let reef = self.reefs.get(&id).ok_or(HabitatError::MissingRecord)?;
        if removed_height_mm == 0 || removed_height_mm > reef.solid_height_mm {
            return Err(HabitatError::InsufficientMaterials);
        }
        let debris = self.debris.checked_add(Materials {
            mineral_units: u64::from(removed_height_mm),
            ..Materials::default()
        })?;
        self.preflight_ids(0)?;
        let reef = self.reefs.get_mut(&id).unwrap();
        reef.solid_height_mm -= removed_height_mm;
        reef.mineral_units -= u64::from(removed_height_mm);
        reef.shelter_capacity = reef.solid_height_mm / 10;
        let event = HabitatEvent::ReefClearanceChanged {
            patch: reef.patch,
            reef: id,
            solid_height_mm: reef.solid_height_mm,
        };
        self.debris = debris;
        self.changed();
        Ok(event)
    }
    /// Exchanges equal finite mineral volume and its proportional nutrients.
    /// No fertility is created. Caller owns the two real substrate layers.
    pub fn mix_soil(
        &mut self,
        patch: PatchId,
        upper: &mut SoilLayer,
        lower: &mut SoilLayer,
        volume_mm3: u64,
        effort: &mut WorkBudget,
    ) -> Result<HabitatEvent, HabitatError> {
        if volume_mm3 == 0 || volume_mm3 > upper.mineral_mm3 || volume_mm3 > lower.mineral_mm3 {
            return Err(HabitatError::InsufficientMaterials);
        }
        let a = (u128::from(upper.nutrients) * u128::from(volume_mm3)
            / u128::from(upper.mineral_mm3)) as u64;
        let b = (u128::from(lower.nutrients) * u128::from(volume_mm3)
            / u128::from(lower.mineral_mm3)) as u64;
        let next_a = (upper.nutrients - a)
            .checked_add(b)
            .ok_or(HabitatError::Overflow)?;
        let next_b = (lower.nutrients - b)
            .checked_add(a)
            .ok_or(HabitatError::Overflow)?;
        let work = volume_mm3.div_ceil(1_000_000);
        let energy = work.div_ceil(5).max(1);
        if effort.energy < energy || effort.work_ticks < work {
            return Err(HabitatError::InsufficientWork);
        }
        self.preflight_ids(0)?;
        effort.energy -= energy;
        effort.work_ticks -= work;
        upper.nutrients = next_a;
        lower.nutrients = next_b;
        let old = upper.porosity_permille;
        let mean = (u32::from(upper.porosity_permille) + u32::from(lower.porosity_permille)) / 2;
        upper.porosity_permille = mean as u16;
        lower.porosity_permille = mean as u16;
        self.changed();
        Ok(HabitatEvent::SoilMixed {
            patch,
            exchanged_mm3: volume_mm3,
            nutrients_moved: a.abs_diff(b),
            porosity_delta: upper.porosity_permille as i16 - old as i16,
        })
    }
    pub fn thin_tree(
        &mut self,
        host: HabitatId,
        available_biomass_units: u64,
        removed_units: u64,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<HabitatEvent, HabitatError> {
        let s = self.supports.get(&host).ok_or(HabitatError::NoHost)?;
        if !s.host_alive {
            return Err(HabitatError::HostDestroyed);
        }
        if removed_units == 0 || removed_units > available_biomass_units {
            return Err(HabitatError::InsufficientMaterials);
        }
        let wood = materials
            .wood_units
            .checked_add(removed_units)
            .ok_or(HabitatError::Overflow)?;
        let ticks = removed_units
            .checked_mul(10)
            .ok_or(HabitatError::Overflow)?;
        self.preflight_ids(0)?;
        pay(
            materials,
            effort,
            Cost {
                energy: removed_units,
                work_ticks: ticks,
                ..Cost::default()
            },
        )?;
        materials.wood_units = wood;
        let patch = s.patch;
        self.changed();
        Ok(HabitatEvent::CanopyThinned {
            patch,
            host,
            biomass_units: removed_units,
        })
    }
    /// Prepare on a bounded sparse-world copy at the low-rate engineering decision,
    /// never per frame/organism. The campaign also stages actual source withdrawals.
    pub fn prepare_engineering(
        &self,
        trait_id: u16,
        actor: OrganismId,
        site: Site,
        materials: Materials,
        effort: WorkBudget,
    ) -> Result<PreparedEngineering, HabitatError> {
        let mut staged = self.clone();
        let mut after_materials = materials;
        let mut after_effort = effort;
        let events = staged.engineer_inner(
            trait_id,
            actor,
            site,
            &mut after_materials,
            &mut after_effort,
        )?;
        let mut excavation = 0u64;
        let mut biomass = 0u64;
        for event in &events {
            match event {
                HabitatEvent::TerrainExcavated { volume_mm3, .. }
                | HabitatEvent::TerrainDepressed {
                    soil_mm3: volume_mm3,
                    ..
                } => {
                    excavation = excavation
                        .checked_add(*volume_mm3)
                        .ok_or(HabitatError::Overflow)?
                }
                HabitatEvent::CanopyThinned { biomass_units, .. } => {
                    biomass = biomass
                        .checked_add(*biomass_units)
                        .ok_or(HabitatError::Overflow)?
                }
                HabitatEvent::HostExcavated {
                    removed_wood_units, ..
                } => {
                    biomass = biomass
                        .checked_add(*removed_wood_units)
                        .ok_or(HabitatError::Overflow)?
                }
                _ => {}
            }
        }
        if excavation > site.available_substrate_mm3 || biomass > site.host_biomass_units {
            return Err(HabitatError::InsufficientMaterials);
        }
        staged.validate()?;
        Ok(PreparedEngineering {
            base_revision: self.revision,
            before_materials: materials,
            before_effort: effort,
            after_materials,
            after_effort,
            events,
            staged,
        })
    }
    pub fn commit_engineering(
        &mut self,
        prepared: PreparedEngineering,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        if self.revision != prepared.base_revision
            || *materials != prepared.before_materials
            || *effort != prepared.before_effort
        {
            return Err(HabitatError::StaleRevision);
        }
        *self = prepared.staged;
        *materials = prepared.after_materials;
        *effort = prepared.after_effort;
        Ok(prepared.events)
    }
    /// Convenience transaction for already validated caller-owned site snapshots.
    /// A campaign must apply every returned external-owner event atomically.
    pub fn engineer(
        &mut self,
        trait_id: u16,
        actor: OrganismId,
        site: Site,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        let prepared = self.prepare_engineering(trait_id, actor, site, *materials, *effort)?;
        self.commit_engineering(prepared, materials, effort)
    }
    fn engineer_inner(
        &mut self,
        trait_id: u16,
        actor: OrganismId,
        site: Site,
        materials: &mut Materials,
        effort: &mut WorkBudget,
    ) -> Result<Vec<HabitatEvent>, HabitatError> {
        if site.group_size == 0 || site.body_width_mm == 0 {
            return Err(HabitatError::IncompatibleSize);
        }
        if matches!(trait_id, 11 | 65 | 103 | 104) {
            if let Some(host) = site.host_id {
                if !self.supports.contains_key(&host) {
                    if site.host_biomass_units == 0 || site.host_height_mm == 0 {
                        return Err(HabitatError::NoHost);
                    }
                    self.register_support(
                        host,
                        site.patch,
                        site.surface(),
                        site.host_height_mm,
                        u32::try_from(site.host_biomass_units / 10)
                            .map_err(|_| HabitatError::Overflow)?
                            .max(1),
                    )?;
                }
            }
        }
        match trait_id {
            11 => {
                let host = site.host_id.ok_or(HabitatError::NoHost)?;
                self.climb(actor, site.group_size, site.body_width_mm, host, effort)?;
                Ok(vec![])
            }
            12 => {
                let entrance = self
                    .nodes
                    .values()
                    .find(|n| n.patch == site.patch && n.kind == NodeKind::Entrance && !n.collapsed)
                    .map(|n| n.id);
                let from = if let Some(id) = entrance {
                    id
                } else {
                    self.add_entrance(
                        site.patch,
                        site.surface(),
                        site.body_width_mm
                            .checked_add(100)
                            .ok_or(HabitatError::Overflow)?
                            .max(300),
                        site.group_size,
                    )?
                };
                let start = self.nodes[&from].position;
                let mid = Point3Mm::new(
                    start.x.checked_add(900).ok_or(HabitatError::Overflow)?,
                    start.y.checked_add(350).ok_or(HabitatError::Overflow)?,
                    start.z.checked_sub(500).ok_or(HabitatError::Overflow)?,
                );
                let end = Point3Mm::new(
                    start.x.checked_add(1600).ok_or(HabitatError::Overflow)?,
                    start.y.checked_add(1100).ok_or(HabitatError::Overflow)?,
                    start.z.checked_sub(1000).ok_or(HabitatError::Overflow)?,
                );
                let (_, _, event) = self.excavate_passage(
                    from,
                    vec![start, mid, end],
                    site.body_width_mm
                        .checked_add(100)
                        .ok_or(HabitatError::Overflow)?
                        .max(300),
                    // The paid bent passage/chamber has finite refuge space
                    // for its builder and the slice's bounded two-offspring
                    // brood. Nests later share these slots; they do not add a
                    // second physical room capacity.
                    site.group_size
                        .checked_add(2)
                        .ok_or(HabitatError::Overflow)?,
                    site.surface_z_mm,
                    site.available_substrate_mm3,
                    materials,
                    effort,
                )?;
                Ok(vec![event])
            }
            46 => {
                let kind = if site.water_depth_mm == 0 {
                    TrapKind::Pit
                } else {
                    TrapKind::Web
                };
                let (_, events) = self.build_trap(
                    site.patch,
                    kind,
                    site.surface(),
                    150,
                    1,
                    site.available_substrate_mm3,
                    materials,
                    effort,
                )?;
                Ok(events)
            }
            64 => {
                let node = self
                    .nodes
                    .values()
                    .find(|n| {
                        n.patch == site.patch
                            && n.kind == NodeKind::Chamber
                            && !n.collapsed
                            && n.flooded_depth_mm == 0
                    })
                    .map(|n| n.id)
                    .ok_or(HabitatError::MissingRecord)?;
                let shelter = self
                    .shelters
                    .values()
                    .find(|s| {
                        s.burrow_node == Some(node) && s.kind == ShelterKind::Den && s.integrity > 0
                    })
                    .map(|s| s.id);
                let id = if let Some(id) = shelter {
                    id
                } else {
                    self.create_shelter(
                        site.patch,
                        ShelterKind::Den,
                        None,
                        Some(node),
                        self.nodes[&node].capacity,
                        self.nodes[&node].aperture_mm,
                        materials,
                        effort,
                    )?
                    .0
                };
                if !self.occupants.contains_key(&actor) {
                    let entrance = self
                        .nodes
                        .values()
                        .find(|n| {
                            n.patch == site.patch && n.kind == NodeKind::Entrance && !n.collapsed
                        })
                        .map(|n| n.id)
                        .ok_or(HabitatError::NotConnected)?;
                    self.enter_burrow(actor, site.group_size, site.body_width_mm, entrance)?;
                }
                self.travel_burrow(actor, node, effort)?;
                self.reserve_shelter(id, actor, site.group_size, site.body_width_mm)?;
                Ok(vec![])
            }
            65 => {
                let (id, events) = self.create_shelter(
                    site.patch,
                    ShelterKind::Nest,
                    site.host_id,
                    None,
                    8,
                    200,
                    materials,
                    effort,
                )?;
                self.shelters.get_mut(&id).unwrap().flood_crest_mm = site
                    .surface_z_mm
                    .checked_add(if site.host_id.is_some() {
                        i32::try_from(site.host_height_mm).map_err(|_| HabitatError::Overflow)?
                    } else {
                        0
                    })
                    .ok_or(HabitatError::Overflow)?;
                Ok(events)
            }
            67 => self.wallow(
                site.patch,
                site.surface(),
                site.water_depth_mm > 0,
                materials,
                effort,
            ),
            101 => {
                if site.water_depth_mm == 0 {
                    return Err(HabitatError::InvalidWater);
                }
                let existing = self
                    .dams
                    .values()
                    .find(|d| d.patch == site.patch && d.integrity > 0)
                    .map(|d| d.id);
                let id = if let Some(id) = existing {
                    id
                } else {
                    self.begin_dam(
                        site.patch,
                        site.surface_z_mm
                            .checked_add(1200)
                            .ok_or(HabitatError::Overflow)?,
                        20,
                    )?
                };
                let remaining =
                    self.dams[&id].required_wood_units - self.dams[&id].invested_wood_units;
                if remaining == 0 {
                    return Err(HabitatError::AlreadyPresent);
                }
                Ok(vec![self.build_dam(
                    id,
                    remaining.min(4),
                    materials,
                    effort,
                )?])
            }
            102 => Err(HabitatError::InsufficientMaterials), // Use mix_soil with the real layer owners.
            103 => {
                let host = site.host_id.ok_or(HabitatError::NoHost)?;
                Ok(vec![self.thin_tree(
                    host,
                    site.host_biomass_units,
                    1,
                    materials,
                    effort,
                )?])
            }
            104 => {
                let (_, events) = self.create_shelter(
                    site.patch,
                    ShelterKind::Cavity,
                    site.host_id,
                    None,
                    2,
                    150,
                    materials,
                    effort,
                )?;
                Ok(events)
            }
            105 => Ok(vec![self.grow_reef(
                site.patch,
                site.water_depth_mm,
                1,
                2,
                materials,
                effort,
            )?]),
            _ => Err(HabitatError::InvalidTrait),
        }
    }
    pub fn shelter_capacity(&self, patch: PatchId) -> u32 {
        let mut rooms = BTreeSet::new();
        let mut capacity = 0u64;
        for s in self
            .shelters
            .values()
            .filter(|s| s.patch == patch && s.integrity > 0 && !s.flooded)
        {
            if let Some(node) = s.burrow_node {
                if rooms.insert(node) {
                    capacity += u64::from(self.nodes[&node].capacity);
                }
            } else {
                capacity += u64::from(s.capacity);
            }
        }
        u32::try_from(capacity).expect("validated sparse habitat capacity exceeds u32")
    }
    pub fn validate(&self) -> Result<(), HabitatError> {
        let max_generated = self
            .nodes
            .keys()
            .chain(self.passages.keys())
            .chain(self.shelters.keys())
            .chain(self.traps.keys())
            .chain(self.wallows.keys())
            .chain(self.dams.keys())
            .chain(self.reefs.keys())
            .copied()
            .max()
            .unwrap_or(0);
        if self.next_id == 0 || self.next_id <= max_generated {
            return Err(HabitatError::Invariant);
        }
        for (id, n) in &self.nodes {
            if *id != n.id
                || n.aperture_mm == 0
                || n.capacity == 0
                || self.occupied_node(*id) > u64::from(n.capacity)
            {
                return Err(HabitatError::Invariant);
            }
        }
        for (id, p) in &self.passages {
            if *id != p.id
                || !self.nodes.contains_key(&p.from)
                || !self.nodes.contains_key(&p.to)
                || p.aperture_mm == 0
                || p.centerline.first() != Some(&self.nodes[&p.from].position)
                || p.centerline.last() != Some(&self.nodes[&p.to].position)
                || checked_length(&p.centerline)? != p.length_mm
            {
                return Err(HabitatError::Invariant);
            }
        }
        for (id, o) in &self.occupants {
            if *id != o.organism || o.group_size == 0 || o.body_width_mm == 0 {
                return Err(HabitatError::Invariant);
            }
            let capacity = match o.location {
                Occupancy::Underground(id) => {
                    let n = self.nodes.get(&id).ok_or(HabitatError::Invariant)?;
                    if n.collapsed || o.body_width_mm > n.aperture_mm {
                        return Err(HabitatError::Invariant);
                    }
                    n.capacity
                }
                Occupancy::Canopy(id) => {
                    let n = self.supports.get(&id).ok_or(HabitatError::Invariant)?;
                    if !n.host_alive {
                        return Err(HabitatError::Invariant);
                    }
                    n.capacity
                }
                Occupancy::Shelter(id) => {
                    let n = self.shelters.get(&id).ok_or(HabitatError::Invariant)?;
                    if n.integrity == 0 || o.body_width_mm > n.aperture_mm {
                        return Err(HabitatError::Invariant);
                    }
                    n.capacity
                }
                Occupancy::Trap(id) => {
                    let n = self.traps.get(&id).ok_or(HabitatError::Invariant)?;
                    if !n.trapped.contains(&o.organism) {
                        return Err(HabitatError::Invariant);
                    }
                    n.capacity
                }
            };
            if self.occupied(o.location) > u64::from(capacity) {
                return Err(HabitatError::Invariant);
            }
        }
        for s in self.shelters.values() {
            if s.capacity == 0
                || s.aperture_mm == 0
                || s.material.soil_mm3 > 0
                || s.integrity > 100
                || s.clutch
                    .is_some_and(|(_, eggs)| eggs == 0 || eggs > s.capacity)
            {
                return Err(HabitatError::Invariant);
            }
            if let Some(n) = s.burrow_node {
                let node = self.nodes.get(&n).ok_or(HabitatError::Invariant)?;
                if s.patch != node.patch
                    || s.capacity > node.capacity
                    || s.aperture_mm > node.aperture_mm
                {
                    return Err(HabitatError::Invariant);
                }
            }
            if let Some(host) = s.host {
                if !self.supports.contains_key(&host) {
                    return Err(HabitatError::Invariant);
                }
            }
        }
        for support in self.supports.values() {
            if support.capacity == 0
                || support.height_mm == 0
                || support.links.iter().any(|id| {
                    !self
                        .supports
                        .get(id)
                        .is_some_and(|s| s.host_alive && s.links.contains(&support.id))
                })
            {
                return Err(HabitatError::Invariant);
            }
        }
        for w in self.wallows.values() {
            if w.depth_mm > w.max_depth_mm
                || w.displaced_soil_mm3
                    != w.disturbed_area_mm2
                        .checked_mul(u64::from(w.depth_mm))
                        .ok_or(HabitatError::Overflow)?
            {
                return Err(HabitatError::Invariant);
            }
        }
        for r in self.reefs.values() {
            if r.mineral_units != u64::from(r.solid_height_mm) {
                return Err(HabitatError::Invariant);
            }
        }
        for t in self.traps.values() {
            let expected = if t.kind == TrapKind::Pit {
                let width = u64::from(t.max_prey_width_mm) * 2;
                width
                    .checked_mul(width)
                    .and_then(|v| v.checked_mul(u64::from(t.depth_mm)))
                    .ok_or(HabitatError::Overflow)?
            } else {
                0
            };
            if t.capacity == 0
                || t.max_prey_width_mm == 0
                || t.excavated_mm3 != expected
                || t.backfilled_mm3 > t.excavated_mm3
                || t.integrity > 100
            {
                return Err(HabitatError::Invariant);
            }
            if t.trapped.iter().any(|id| {
                !self
                    .occupants
                    .get(id)
                    .is_some_and(|o| o.location == Occupancy::Trap(t.id))
            }) {
                return Err(HabitatError::Invariant);
            }
        }
        for d in self.dams.values() {
            if d.required_wood_units == 0
                || d.invested_wood_units > d.required_wood_units
                || d.open_permeability_permille > 1000
                || d.closed_permeability_permille > d.open_permeability_permille
                || d.integrity > 100
            {
                return Err(HabitatError::Invariant);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn funds() -> (Materials, WorkBudget) {
        (
            Materials {
                soil_mm3: 0,
                wood_units: 1000,
                fiber_units: 1000,
                mineral_units: 1000,
            },
            WorkBudget {
                energy: 1_000_000,
                work_ticks: 1_000_000,
            },
        )
    }
    fn burrow() -> (HabitatWorld, HabitatId, HabitatId, HabitatId) {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let a = h
            .add_entrance(0, Point3Mm::new(0, 0, 1000), 300, 8)
            .unwrap();
        let (b, edge, _) = h
            .excavate_passage(
                a,
                vec![
                    Point3Mm::new(0, 0, 1000),
                    Point3Mm::new(500, 400, 500),
                    Point3Mm::new(1000, 1200, -100),
                ],
                200,
                4,
                1000,
                u64::MAX,
                &mut m,
                &mut e,
            )
            .unwrap();
        (h, a, b, edge)
    }
    #[test]
    fn bent_burrow_preserves_centerline_aperture_spoil_and_cost() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let initial = e;
        let a = h
            .add_entrance(7, Point3Mm::new(0, 0, 2000), 400, 4)
            .unwrap();
        let points = vec![
            Point3Mm::new(0, 0, 2000),
            Point3Mm::new(700, 300, 1300),
            Point3Mm::new(1000, 900, 900),
        ];
        let (b, id, event) = h
            .excavate_passage(a, points.clone(), 200, 3, 2000, u64::MAX, &mut m, &mut e)
            .unwrap();
        assert_eq!(h.passages[&id].centerline, points);
        assert_eq!(m.soil_mm3, h.passages[&id].excavated_mm3);
        assert!(e.energy < initial.energy && e.work_ticks < initial.work_ticks);
        assert!(matches!(
            event,
            HabitatEvent::TerrainExcavated { patch: 7, .. }
        ));
        assert_eq!(h.route(a, b, 150).unwrap(), vec![a, b]);
        assert_eq!(h.route(a, b, 250), Err(HabitatError::IncompatibleSize));
        h.validate().unwrap();
    }
    #[test]
    fn branching_routes_and_real_exit_do_not_duplicate_occupants() {
        let (mut h, a, b, _) = burrow();
        let (mut m, mut e) = funds();
        let start = h.nodes[&b].position;
        let (c, _, _) = h
            .excavate_passage(
                b,
                vec![start, Point3Mm::new(1600, 1700, -200)],
                150,
                2,
                1000,
                u64::MAX,
                &mut m,
                &mut e,
            )
            .unwrap();
        h.enter_burrow(90, 1, 100, a).unwrap();
        assert!(h.enter_burrow(90, 1, 100, a).is_err());
        h.travel_burrow(90, c, &mut e).unwrap();
        assert_eq!(h.occupants.len(), 1);
        assert_eq!(h.exit_burrow(90), Err(HabitatError::Blocked));
        h.travel_burrow(90, a, &mut e).unwrap();
        assert_eq!(h.exit_burrow(90).unwrap(), 0);
        assert!(h.occupants.is_empty());
        h.validate().unwrap();
    }
    #[test]
    fn blocked_passages_flood_only_the_open_connected_graph() {
        let (mut h, a, b, edge) = burrow();
        h.enter_burrow(9, 1, 100, a).unwrap();
        let mut effort = WorkBudget {
            energy: 100,
            work_ticks: 1000,
        };
        h.travel_burrow(9, b, &mut effort).unwrap();
        h.set_passage_blocked(edge, true).unwrap();
        let events = h.update_flood_hazards(1, &[(0, 1500)]).unwrap();
        assert_eq!(h.nodes[&b].flooded_depth_mm, 0);
        assert!(events.is_empty());
        h.set_passage_blocked(edge, false).unwrap();
        let events = h.update_flood_hazards(2, &[(0, 1500)]).unwrap();
        assert_eq!(events.len(), 1);
        assert!(h.nodes[&b].flooded_depth_mm > 100);
        assert!(h.route(a, b, 100).is_err());
        assert!(h.update_flood_hazards(2, &[]).is_err());
    }
    #[test]
    fn surface_cut_collapses_roof_and_exposes_actual_occupant_once() {
        let (mut h, a, b, edge) = burrow();
        h.enter_burrow(19, 1, 100, a).unwrap();
        let mut e = WorkBudget {
            energy: 100,
            work_ticks: 1000,
        };
        h.travel_burrow(19, b, &mut e).unwrap();
        assert!(h.excavate_surface(0, 400).unwrap().is_empty());
        assert!(!h.nodes[&b].collapsed && h.passages[&edge].blocked);
        assert_eq!(h.excavate_surface(0, 300).unwrap().len(), 1);
        assert!(h.nodes[&b].collapsed && h.passages[&edge].blocked);
        assert!(!h.occupants.contains_key(&19));
        assert!(h.excavate_surface(0, 200).unwrap().is_empty());
        h.validate().unwrap();
    }
    #[test]
    fn finite_den_capacity_and_nest_clutch_do_not_spawn_animals() {
        let (mut h, entrance, b, _) = burrow();
        let (mut m, mut e) = funds();
        let (den, _) = h
            .create_shelter(0, ShelterKind::Den, None, Some(b), 2, 150, &mut m, &mut e)
            .unwrap();
        h.enter_burrow(20, 2, 100, entrance).unwrap();
        h.travel_burrow(20, b, &mut e).unwrap();
        h.reserve_shelter(den, 20, 2, 100).unwrap();
        h.enter_burrow(21, 1, 100, entrance).unwrap();
        h.travel_burrow(21, b, &mut e).unwrap();
        assert_eq!(
            h.reserve_shelter(den, 21, 1, 100),
            Err(HabitatError::CapacityExceeded)
        );
        assert_eq!(h.occupants.len(), 2);
        let (nest, _) = h
            .create_shelter(0, ShelterKind::Nest, None, None, 4, 100, &mut m, &mut e)
            .unwrap();
        h.attach_clutch(nest, 44, 3).unwrap();
        assert!(h.attach_clutch(nest, 45, 1).is_err());
        assert_eq!(h.shelters[&nest].clutch, Some((44, 3)));
    }
    #[test]
    fn canopy_connectivity_host_loss_and_cavities_have_real_fall_outcomes() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        h.register_support(70, 0, Point3Mm::new(0, 0, 0), 4000, 3)
            .unwrap();
        h.register_support(71, 0, Point3Mm::new(1000, 0, 0), 5000, 3)
            .unwrap();
        h.climb(1, 1, 100, 70, &mut e).unwrap();
        assert_eq!(
            h.climb(1, 1, 100, 71, &mut e),
            Err(HabitatError::NotConnected)
        );
        h.link_supports(70, 71, 2000).unwrap();
        h.climb(1, 1, 100, 71, &mut e).unwrap();
        let (cavity, event) = h
            .create_shelter(
                0,
                ShelterKind::Cavity,
                Some(71),
                None,
                2,
                150,
                &mut m,
                &mut e,
            )
            .unwrap();
        assert!(matches!(
            event[0],
            HabitatEvent::HostExcavated {
                removed_wood_units: 2,
                ..
            }
        ));
        h.reserve_shelter(cavity, 2, 1, 100).unwrap();
        let events = h.destroy_host(71).unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .all(|e| matches!(e, HabitatEvent::OccupantExposed { fall_mm: 5000, .. }))
        );
        assert!(h.occupants.is_empty());
        assert_eq!(h.shelters[&cavity].integrity, 0);
        h.validate().unwrap();
    }
    #[test]
    fn webs_have_no_fake_crater_pits_need_exact_backfill_and_prey_is_unique() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let (web, event) = h
            .build_trap(
                0,
                TrapKind::Web,
                Point3Mm::new(0, 0, 0),
                100,
                1,
                0,
                &mut m,
                &mut e,
            )
            .unwrap();
        assert!(event.is_empty());
        assert_eq!(h.traps[&web].depth_mm, 0);
        assert_eq!(m.fiber_units, 992);
        h.intercept(web, 8, 50).unwrap();
        assert!(h.intercept(web, 8, 50).is_err());
        assert!(h.intercept(web, 9, 50).is_err());
        let ids = h.destroy_trap(web, &mut m).unwrap();
        assert_eq!(ids.0, vec![8]);
        assert_eq!(h.debris.fiber_units, 8);
        assert!(!h.occupants.contains_key(&8));
        let (pit, events) = h
            .build_trap(
                0,
                TrapKind::Pit,
                Point3Mm::new(0, 0, 0),
                100,
                1,
                u64::MAX,
                &mut m,
                &mut e,
            )
            .unwrap();
        let soil = m.soil_mm3;
        assert!(matches!(events[0], HabitatEvent::TerrainDepressed { .. }));
        m.soil_mm3 = 0;
        assert_eq!(
            h.destroy_trap(pit, &mut m),
            Err(HabitatError::InsufficientMaterials)
        );
        m.soil_mm3 = soil;
        h.destroy_trap(pit, &mut m).unwrap();
        assert_eq!(m.soil_mm3, 0);
        h.validate().unwrap();
    }
    #[test]
    fn repeated_wallowing_persists_and_dry_wallow_cannot_cool() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let p = Point3Mm::new(0, 0, 0);
        assert!(h.wallow(1, p, true, &mut m, &mut e).unwrap().is_empty());
        assert!(h.wallow(1, p, true, &mut m, &mut e).unwrap().is_empty());
        let event = h.wallow(1, p, true, &mut m, &mut e).unwrap();
        assert_eq!(event.len(), 1);
        assert_eq!(h.wallows.values().next().unwrap().depth_mm, 5);
        assert_eq!(m.soil_mm3, 20_000_000);
        assert_eq!(
            h.wallow(1, p, false, &mut m, &mut e),
            Err(HabitatError::InvalidWater)
        );
    }
    #[test]
    fn dams_spend_finite_material_gradually_reduce_flow_and_breach_to_debris() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let dam = h.begin_dam(0, 1000, 10).unwrap();
        assert_eq!(h.dams[&dam].permeability_permille(), 1000);
        h.build_dam(dam, 4, &mut m, &mut e).unwrap();
        assert!(!h.dams[&dam].completed());
        assert!(h.dams[&dam].permeability_permille() < 1000);
        h.build_dam(dam, 6, &mut m, &mut e).unwrap();
        assert_eq!(h.dams[&dam].permeability_permille(), 40);
        assert_eq!(m.wood_units + h.dams[&dam].invested_wood_units, 1000);
        let event = h.damage_dam(dam, 100).unwrap();
        assert!(matches!(
            event,
            HabitatEvent::DamBarrierChanged {
                permeability_permille: 1000,
                ..
            }
        ));
        assert_eq!(m.wood_units + h.debris.wood_units, 1000);
        h.damage_dam(dam, 100).unwrap();
        assert_eq!(h.debris.wood_units, 10);
        h.validate().unwrap();
    }
    #[test]
    fn dead_reef_builders_leave_physical_substrate_and_draft_limits() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let event = h.grow_reef(4, 1000, 10, 200, &mut m, &mut e).unwrap();
        let HabitatEvent::ReefClearanceChanged { reef, .. } = event else {
            panic!()
        };
        assert_eq!(h.reefs[&reef].clearance_mm(), 800);
        assert!(!h.reefs[&reef].permits_draft(900));
        h.reef_builders_died(reef).unwrap();
        assert_eq!(h.reefs[&reef].solid_height_mm, 200);
        assert_eq!(h.reefs[&reef].living_builder_units, 0);
        assert_eq!(m.mineral_units + h.reefs[&reef].mineral_units, 1000);
        assert!(h.grow_reef(4, 1000, 0, 1, &mut m, &mut e).is_err());
    }
    #[test]
    fn soil_mixing_moves_nutrients_not_free_fertility_and_costs_work() {
        let mut h = HabitatWorld::default();
        let mut a = SoilLayer {
            mineral_mm3: 1000,
            nutrients: 100,
            porosity_permille: 400,
        };
        let mut b = SoilLayer {
            mineral_mm3: 1000,
            nutrients: 20,
            porosity_permille: 600,
        };
        let mut e = WorkBudget {
            energy: 100,
            work_ticks: 100,
        };
        h.mix_soil(0, &mut a, &mut b, 500, &mut e).unwrap();
        assert_eq!(a.nutrients + b.nutrients, 120);
        assert_eq!((a.nutrients, b.nutrients), (60, 60));
        assert_eq!(a.mineral_mm3 + b.mineral_mm3, 2000);
        assert_eq!(a.porosity_permille, 500);
        assert!(e.energy < 100 && e.work_ticks < 100);
    }
    #[test]
    fn thinning_moves_real_biomass_to_wood_and_rejects_overdraw() {
        let mut h = HabitatWorld::default();
        h.register_support(12, 0, Point3Mm::new(0, 0, 0), 3000, 3)
            .unwrap();
        let (mut m, mut e) = funds();
        let before = m.wood_units;
        let event = h.thin_tree(12, 10, 4, &mut m, &mut e).unwrap();
        assert!(matches!(
            event,
            HabitatEvent::CanopyThinned {
                biomass_units: 4,
                ..
            }
        ));
        assert_eq!(m.wood_units - before, 4);
        assert!(h.thin_tree(12, 3, 4, &mut m, &mut e).is_err());
    }
    #[test]
    fn failed_cost_is_atomic_and_save_restores_real_graph_and_reservations() {
        let (mut h, a, b, _) = burrow();
        let (mut m, mut e) = funds();
        let before = serde_json::to_string(&h).unwrap();
        let (w, mut empty) = (m, WorkBudget::default());
        assert!(
            h.build_trap(
                0,
                TrapKind::Web,
                Point3Mm::new(0, 0, 0),
                100,
                1,
                0,
                &mut m,
                &mut empty
            )
            .is_err()
        );
        assert_eq!(m, w);
        assert_eq!(serde_json::to_string(&h).unwrap(), before);
        h.enter_burrow(33, 1, 100, a).unwrap();
        h.travel_burrow(33, b, &mut e).unwrap();
        let encoded = serde_json::to_string(&h).unwrap();
        let loaded: HabitatWorld = serde_json::from_str(&encoded).unwrap();
        loaded.validate().unwrap();
        assert_eq!(loaded.occupants[&33].location, Occupancy::Underground(b));
        assert_eq!(serde_json::to_string(&loaded).unwrap(), encoded);
    }
    fn site() -> Site {
        Site {
            patch: 0,
            position_mm: [0, 0],
            surface_z_mm: 1000,
            water_depth_mm: 0,
            host_id: None,
            host_height_mm: 0,
            available_substrate_mm3: 10_000_000_000,
            host_biomass_units: 0,
            group_size: 1,
            body_width_mm: 100,
        }
    }
    #[test]
    fn prepared_engineering_rejects_stale_jobs_and_never_creates_free_structures() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let before = serde_json::to_string(&h).unwrap();
        let mut no_work = WorkBudget::default();
        assert!(h.engineer(12, 1, site(), &mut m, &mut no_work).is_err());
        assert_eq!(serde_json::to_string(&h).unwrap(), before);
        let prepared = h.prepare_engineering(12, 1, site(), m, e).unwrap();
        assert!(prepared.energy_spent() > 0);
        assert!(h.nodes.is_empty());
        h.add_entrance(1, Point3Mm::new(0, 0, 0), 100, 1).unwrap();
        let wallet = m;
        let effort = e;
        assert_eq!(
            h.commit_engineering(prepared, &mut m, &mut e),
            Err(HabitatError::StaleRevision)
        );
        assert_eq!(m, wallet);
        assert_eq!(e, effort);
        let mut finite = site();
        finite.available_substrate_mm3 = 1;
        let before = serde_json::to_string(&h).unwrap();
        assert_eq!(
            h.engineer(12, 1, finite, &mut m, &mut e),
            Err(HabitatError::InsufficientMaterials)
        );
        assert_eq!(serde_json::to_string(&h).unwrap(), before);
    }
    #[test]
    fn den_reservation_requires_reachable_room_and_does_not_double_room_capacity() {
        let (mut h, a, b, _) = burrow();
        let (mut m, mut e) = funds();
        let (d1, _) = h
            .create_shelter(0, ShelterKind::Den, None, Some(b), 4, 150, &mut m, &mut e)
            .unwrap();
        let (d2, _) = h
            .create_shelter(0, ShelterKind::Den, None, Some(b), 4, 150, &mut m, &mut e)
            .unwrap();
        assert_eq!(h.shelter_capacity(0), 4);
        assert_eq!(
            h.reserve_shelter(d1, 1, 4, 100),
            Err(HabitatError::NotConnected)
        );
        h.enter_burrow(1, 4, 100, a).unwrap();
        h.travel_burrow(1, b, &mut e).unwrap();
        h.reserve_shelter(d1, 1, 4, 100).unwrap();
        h.enter_burrow(2, 1, 100, a).unwrap();
        assert_eq!(
            h.travel_burrow(2, b, &mut e),
            Err(HabitatError::CapacityExceeded)
        );
        assert_eq!(
            h.reserve_shelter(d2, 2, 1, 100),
            Err(HabitatError::NotConnected)
        );
        h.validate().unwrap();
    }
    #[test]
    fn canopy_nests_flood_only_when_real_water_reaches_their_elevation() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        h.register_support(100, 0, Point3Mm::new(0, 0, 1000), 3000, 8)
            .unwrap();
        let (nest, _) = h
            .create_shelter(
                0,
                ShelterKind::Nest,
                Some(100),
                None,
                4,
                100,
                &mut m,
                &mut e,
            )
            .unwrap();
        h.update_flood_hazards(1, &[(0, 1500)]).unwrap();
        assert!(!h.shelters[&nest].flooded);
        h.update_flood_hazards(2, &[(0, 4100)]).unwrap();
        assert!(h.shelters[&nest].flooded);
    }
    #[test]
    fn engineering_replay_across_seeds_preserves_invariants_and_failed_actions_are_atomic() {
        for seed in 1..=16u64 {
            let mut h = HabitatWorld::default();
            let (mut m, mut e) = funds();
            let mut state = seed;
            for i in 0..64u64 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let traits = [11, 12, 46, 64, 65, 67, 101, 103, 104, 105];
                let trait_id = traits[(state as usize) % traits.len()];
                let mut site = site();
                site.patch = (i % 3) as u32;
                site.position_mm = [(i % 3) as i32 * 20000, 0];
                site.water_depth_mm = if i % 2 == 0 { 1000 } else { 0 };
                site.host_id = Some(10000 + i % 3);
                site.host_height_mm = 3000;
                site.host_biomass_units = 100;
                let before = serde_json::to_string(&h).unwrap();
                let wallet = m;
                let effort = e;
                if h.engineer(trait_id, seed * 1000 + i, site, &mut m, &mut e)
                    .is_err()
                {
                    assert_eq!(serde_json::to_string(&h).unwrap(), before);
                    assert_eq!(m, wallet);
                    assert_eq!(e, effort);
                }
                h.validate().unwrap();
            }
            let json = serde_json::to_string(&h).unwrap();
            let loaded: HabitatWorld = serde_json::from_str(&json).unwrap();
            loaded.validate().unwrap();
            assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
        }
    }
    #[test]
    fn engineering_tracks_actual_group_size_and_body_width_in_refuge_capacity() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let mut s = site();
        s.group_size = 12;
        s.body_width_mm = 180;
        h.engineer(12, 7, s, &mut m, &mut e).unwrap();
        h.engineer(64, 7, s, &mut m, &mut e).unwrap();
        assert_eq!(h.occupants[&7].group_size, 12);
        assert_eq!(h.occupants[&7].body_width_mm, 180);
        assert_eq!(h.shelter_capacity(0), 14);
        // The actual twelve occupants consume twelve slots. The paid room's
        // two remaining brood slots are shared with nested reproductive shelter.
        assert_eq!(
            h.occupied_node(h.shelters.values().next().unwrap().burrow_node.unwrap()),
            12
        );
        h.validate().unwrap();
    }
    #[test]
    fn dam_damage_leaks_before_breach_and_repair_replaces_actual_wood_without_mass_loss() {
        let mut h = HabitatWorld::default();
        let (mut m, mut e) = funds();
        let id = h.begin_dam(0, 1000, 20).unwrap();
        h.build_dam(id, 20, &mut m, &mut e).unwrap();
        let closed = h.dams[&id].permeability_permille();
        h.damage_dam(id, 30).unwrap();
        assert!(h.dams[&id].permeability_permille() > closed);
        h.repair_dam(id, 3, &mut m, &mut e).unwrap();
        assert_eq!(h.dams[&id].permeability_permille(), closed);
        assert_eq!(
            m.wood_units + h.dams[&id].invested_wood_units + h.debris.wood_units,
            1000
        );
        h.validate().unwrap();
    }
    #[test]
    fn local_crater_does_not_collapse_remote_burrow_in_same_macro_patch() {
        let (mut h, a, b, edge) = burrow();
        let (mut m, mut e) = funds();
        let remote = h
            .add_entrance(0, Point3Mm::new(30000, 30000, 1000), 300, 2)
            .unwrap();
        let (r, re, _) = h
            .excavate_passage(
                remote,
                vec![
                    Point3Mm::new(30000, 30000, 1000),
                    Point3Mm::new(31500, 31000, -100),
                ],
                200,
                2,
                1000,
                u64::MAX,
                &mut m,
                &mut e,
            )
            .unwrap();
        h.enter_burrow(1, 1, 100, a).unwrap();
        h.travel_burrow(1, b, &mut e).unwrap();
        h.enter_burrow(2, 1, 100, remote).unwrap();
        h.travel_burrow(2, r, &mut e).unwrap();
        let events = h.excavate_surface_at(0, [0, 0], [8000, 8000], 200).unwrap();
        assert_eq!(events.len(), 1);
        assert!(h.nodes[&b].collapsed && h.passages[&edge].blocked);
        assert!(!h.nodes[&r].collapsed && !h.passages[&re].blocked);
        assert!(h.occupants.contains_key(&2));
        h.validate().unwrap();
    }
}
