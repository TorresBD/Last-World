//! CPU-authoritative fixed-tick movement, shared navigation and tactical combat.
//!
//! Positions use integer millimeters. GPU records are derived render copies.
//! Tile data is a flat resource, not entities. Route frontiers and cost-refresh
//! deadlines serialize; derived spatial scratch rebuilds each tick. Bounded
//! Dijkstra work delays routes without dropping agents or crossing stale barriers.

use crate::terrain::{
    CELL_METERS, CH, CW, DYNAMIC_WATER, SOIL_MOISTURE_MASK, SOIL_MOISTURE_SHIFT, Terrain,
    WALK_WATER_LIMIT_MM,
};
use serde::{Deserialize, Serialize};
use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque},
};

pub type AgentId = u64;
pub const NAV_CELL_MM: i32 = 16_000;
const UNREACHED: u32 = u32::MAX;
const MAX_ROUTES: usize = 32;
const ROUTE_EXPANSIONS_PER_TICK: usize = 4096;
const COST_REFRESH_TICKS: u64 = 200;
const FOOT_CONNECTIVITY: u32 = 0x1ff;
const NAVAL_CONNECTIVITY: u32 = 0x1ff0000;
const MAX_SLOPE_PERMILLE: i64 = 500;
const DIRS: [(i32, i32); 8] = [
    (-1, 0),
    (1, 0),
    (0, -1),
    (0, 1),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
    Soldier,
    Worker,
    Scout,
    Boat,
    Transport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MovementClass {
    Foot,
    Naval { draft_mm: u32, air_draft_mm: u32 },
}

impl Role {
    pub fn movement(self) -> MovementClass {
        match self {
            Self::Boat | Self::Transport => MovementClass::Naval {
                draft_mm: 350,
                air_draft_mm: 1800,
            },
            _ => MovementClass::Foot,
        }
    }
    pub fn radius_mm(self) -> i32 {
        match self {
            Self::Soldier => 2800,
            Self::Worker | Self::Scout => 2200,
            Self::Boat | Self::Transport => 3500,
        }
    }
    fn max_health(self) -> u16 {
        if matches!(self, Self::Boat | Self::Transport) {
            600
        } else {
            100
        }
    }
    fn initial_ammo(self) -> u16 {
        match self {
            Self::Soldier => 40,
            Self::Boat => 80,
            _ => 0,
        }
    }
    fn range_mm(self) -> i32 {
        match self {
            Self::Soldier => 72_000,
            Self::Boat => 220_000,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavCell {
    pub elevation_mm: i32,
    pub water_depth_mm: u32,
    /// Conservative vessel clearance across the coarse cell. None is used by
    /// uniform custom/test grids; terrain aggregation stores the shallowest
    /// of its four8m samples so one wet corner never permits sailing on land.
    #[serde(default)]
    pub minimum_water_depth_mm: Option<u32>,
    /// Physical reef solid is an overlay, not a second water-volume owner.
    #[serde(default)]
    pub reef_height_mm: u32,
    pub cover: u8,
    #[serde(default)]
    pub soil_moisture_permille: u16,
    pub road: bool,
    pub blocked: bool,
    /// Absolute top-of-deck elevation. A road alone never bridges water.
    pub bridge_deck_mm: Option<i32>,
    /// Deck underside elevation for vessels; zero means no overhead structure.
    pub bridge_underside_mm: Option<i32>,
}

impl NavCell {
    pub fn effective_depth_mm(self) -> u32 {
        self.water_depth_mm.saturating_sub(self.reef_height_mm)
    }
    pub fn keel_depth_mm(self) -> u32 {
        self.minimum_water_depth_mm
            .unwrap_or(self.water_depth_mm)
            .min(self.water_depth_mm)
            .saturating_sub(self.reef_height_mm)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DirtyBatch {
    revision: u64,
    cells: Vec<usize>,
    #[serde(default)]
    topology_changes: Vec<(usize, u32)>,
}

#[derive(Clone, Debug, Default)]
struct OverlayIndices {
    bridges: BTreeSet<usize>,
    reefs: BTreeSet<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NavGrid {
    pub width: usize,
    pub height: usize,
    pub cell_mm: i32,
    /// External edits must use `set_cell` so revision history, connectivity and
    /// derived overlay indices remain synchronized. Direct access is for reads.
    pub cells: Vec<NavCell>,
    pub revision: u64,
    #[serde(default = "default_revision")]
    pub topology_revision: u64,
    changes: VecDeque<DirtyBatch>,
    #[serde(default)]
    connectivity_masks: Vec<u32>,
    // Derived from the authoritative cells once after construction/load. Keep
    // full replacement semantics without inspecting the whole grid each tick.
    #[serde(skip)]
    overlay_indices: Option<OverlayIndices>,
}

impl NavGrid {
    pub fn new(
        width: usize,
        height: usize,
        cell_mm: i32,
        cells: Vec<NavCell>,
    ) -> Result<Self, String> {
        if width == 0
            || height == 0
            || cell_mm <= 0
            || width.checked_mul(height) != Some(cells.len())
            || width > i32::MAX as usize / cell_mm as usize
            || height > i32::MAX as usize / cell_mm as usize
            || cells
                .iter()
                .any(|c| c.reef_height_mm > 1_000_000 || c.soil_moisture_permille > 1000)
        {
            return Err("Invalid navigation grid dimensions".into());
        }
        let mut grid = Self {
            width,
            height,
            cell_mm,
            cells,
            revision: 1,
            topology_revision: 1,
            changes: VecDeque::new(),
            connectivity_masks: Vec::new(),
            overlay_indices: None,
        };
        grid.ensure_connectivity_masks();
        grid.ensure_overlay_indices();
        Ok(grid)
    }

    pub fn from_terrain(terrain: &Terrain) -> Result<Self, String> {
        let mut grid = Self::new(
            CW / 2,
            CH / 2,
            NAV_CELL_MM,
            vec![NavCell::default(); CW / 2 * (CH / 2)],
        )?;
        grid.refresh(terrain)?;
        Ok(grid)
    }

    /// The caller refreshes after each authoritative physical transaction.
    /// Water-only changes matter even when the terrain geometry revision did
    /// not change. Existing explicit bridge metadata is retained.
    pub fn refresh(&mut self, terrain: &Terrain) -> Result<usize, String> {
        if self.width != CW / 2
            || self.height != CH / 2
            || self.cell_mm != (CELL_METERS * 2000) as i32
        {
            return Err("Terrain/navigation dimensions do not match".into());
        }
        self.ensure_connectivity_masks();
        self.revision
            .checked_add(1)
            .ok_or("Navigation revision overflow")?;
        self.topology_revision
            .checked_add(1)
            .ok_or("Navigation topology revision overflow")?;
        let mut dirty = Vec::new();
        let mut topology_candidates = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let index = y * self.width + x;
                let base = y * 2 * CW + x * 2;
                let indices = [base, base + 1, base + CW, base + CW + 1];
                let mut elevation = 0i64;
                let mut depth = 0u32;
                let mut minimum_depth = u32::MAX;
                let mut cover = 0u16;
                let mut soil_moisture = 0u32;
                let mut road = false;
                let mut blocked = false;
                for i in indices {
                    let c = terrain.cells[i];
                    elevation += terrain.elevation_mm(i).map_err(|e| e.to_string())? as i64;
                    let sample_depth = if c.flags & DYNAMIC_WATER != 0 {
                        c.water
                    } else {
                        c.water
                            .checked_mul(10)
                            .ok_or("Legacy water depth overflow")?
                    };
                    depth = depth.max(sample_depth);
                    minimum_depth = minimum_depth.min(sample_depth);
                    cover += c.cover.clamp(0., 255.) as u16;
                    soil_moisture +=
                        ((c.flags & SOIL_MOISTURE_MASK) >> SOIL_MOISTURE_SHIFT).min(1000);
                    road |= c.flags & 1 != 0;
                    blocked |= c.object != 0;
                }
                let previous = self.cells[index];
                let next = NavCell {
                    elevation_mm: (elevation / 4) as i32,
                    water_depth_mm: depth,
                    minimum_water_depth_mm: Some(minimum_depth),
                    reef_height_mm: previous.reef_height_mm,
                    cover: (cover / 4) as u8,
                    soil_moisture_permille: (soil_moisture / 4) as u16,
                    road,
                    blocked,
                    bridge_deck_mm: previous.bridge_deck_mm,
                    bridge_underside_mm: previous.bridge_underside_mm,
                };
                if previous != next {
                    self.cells[index] = next;
                    if nav_signature(previous) != nav_signature(next) {
                        dirty.push(index);
                        if topology_signature(previous) != topology_signature(next) {
                            topology_candidates.push(index);
                        }
                    }
                }
            }
        }
        let count = dirty.len();
        self.commit_dirty(dirty, topology_candidates)?;
        Ok(count)
    }

    pub fn set_cell(&mut self, index: usize, cell: NavCell) -> Result<bool, String> {
        if cell.reef_height_mm > 1_000_000 || cell.soil_moisture_permille > 1000 {
            return Err("Reef structural height outside navigation range".into());
        }
        self.ensure_connectivity_masks();
        let old = self.cells.get(index).ok_or("Navigation cell outside map")?;
        if *old == cell {
            return Ok(false);
        }
        let changed = nav_signature(*old) != nav_signature(cell);
        let topology_changed = topology_signature(*old) != topology_signature(cell);
        if changed {
            self.revision
                .checked_add(1)
                .ok_or("Navigation revision overflow")?;
            self.topology_revision
                .checked_add(1)
                .ok_or("Navigation topology revision overflow")?;
        }
        self.cells[index] = cell;
        if let Some(indices) = &mut self.overlay_indices {
            if cell.bridge_deck_mm.is_some() || cell.bridge_underside_mm.is_some() {
                indices.bridges.insert(index);
            } else {
                indices.bridges.remove(&index);
            }
            if cell.reef_height_mm != 0 {
                indices.reefs.insert(index);
            } else {
                indices.reefs.remove(&index);
            }
        }
        if changed {
            self.commit_dirty(
                vec![index],
                if topology_changed {
                    vec![index]
                } else {
                    Vec::new()
                },
            )?;
        }
        Ok(true)
    }

    /// Replace the complete authoritative bridge overlay as one semantic
    /// revision. Destroyed spans are removed; unchanged spans do not churn
    /// fields on every hydrology tick. Each tuple is cell/deck/underside.
    pub fn replace_bridges(&mut self, spans: &[(usize, i32, i32)]) -> Result<usize, String> {
        let mut desired = BTreeMap::new();
        for &(index, deck, underside) in spans {
            if index >= self.cells.len() || underside > deck {
                return Err("Invalid bridge navigation overlay".into());
            }
            if desired
                .insert(index, (deck, underside))
                .is_some_and(|old| old != (deck, underside))
            {
                return Err("Conflicting bridge decks in one navigation cell".into());
            }
        }
        self.ensure_overlay_indices();
        let desired_indices: BTreeSet<_> = desired.keys().copied().collect();
        let inspect: Vec<_> = self
            .overlay_indices
            .as_ref()
            .unwrap()
            .bridges
            .union(&desired_indices)
            .copied()
            .collect();
        let mut changes = Vec::new();
        for index in inspect {
            let c = self.cells[index];
            let (deck, underside) = desired
                .get(&index)
                .map_or((None, None), |&(d, u)| (Some(d), Some(u)));
            if c.bridge_deck_mm != deck || c.bridge_underside_mm != underside {
                let mut next = c;
                next.bridge_deck_mm = deck;
                next.bridge_underside_mm = underside;
                changes.push((index, next));
            }
        }
        if changes.is_empty() {
            return Ok(0);
        }
        self.revision
            .checked_add(1)
            .ok_or("Navigation revision overflow")?;
        self.ensure_connectivity_masks();
        let count = changes.len();
        let mut dirty = Vec::with_capacity(count);
        let mut topology_candidates = Vec::new();
        self.topology_revision
            .checked_add(1)
            .ok_or("Navigation topology revision overflow")?;
        for (index, next) in changes {
            if nav_signature(self.cells[index]) != nav_signature(next) {
                dirty.push(index);
                if topology_signature(self.cells[index]) != topology_signature(next) {
                    topology_candidates.push(index);
                }
            }
            self.cells[index] = next;
        }
        self.overlay_indices.as_mut().unwrap().bridges = desired_indices;
        self.commit_dirty(dirty, topology_candidates)?;
        Ok(count)
    }

    /// Replace every current physical reef clearance modifier. Raw water
    /// samples stay intact, so refresh followed by this operation never flips
    /// static reefs off/on and invalidates routes twice per tactical tick.
    pub fn replace_reefs(&mut self, reefs: &[(usize, u32)]) -> Result<usize, String> {
        let mut desired = BTreeMap::new();
        for &(index, height) in reefs {
            if index >= self.cells.len() || height > 1_000_000 {
                return Err("Invalid reef navigation overlay".into());
            }
            let entry = desired.entry(index).or_insert(0u32);
            *entry = (*entry).max(height);
        }
        self.ensure_overlay_indices();
        let desired_indices: BTreeSet<_> = desired
            .iter()
            .filter_map(|(&index, &height)| (height != 0).then_some(index))
            .collect();
        let inspect: Vec<_> = self
            .overlay_indices
            .as_ref()
            .unwrap()
            .reefs
            .union(&desired_indices)
            .copied()
            .collect();
        let changes: Vec<_> = inspect
            .into_iter()
            .filter_map(|i| {
                let c = self.cells[i];
                let height = desired.get(&i).copied().unwrap_or(0);
                if c.reef_height_mm != height {
                    let mut next = c;
                    next.reef_height_mm = height;
                    Some((i, next))
                } else {
                    None
                }
            })
            .collect();
        if changes.is_empty() {
            return Ok(0);
        }
        self.revision
            .checked_add(1)
            .ok_or("Navigation revision overflow")?;
        self.ensure_connectivity_masks();
        let count = changes.len();
        let mut dirty = Vec::with_capacity(count);
        let mut topology_candidates = Vec::new();
        self.topology_revision
            .checked_add(1)
            .ok_or("Navigation topology revision overflow")?;
        for (index, next) in changes {
            if nav_signature(self.cells[index]) != nav_signature(next) {
                dirty.push(index);
                if topology_signature(self.cells[index]) != topology_signature(next) {
                    topology_candidates.push(index);
                }
            }
            self.cells[index] = next;
        }
        self.overlay_indices.as_mut().unwrap().reefs = desired_indices;
        self.commit_dirty(dirty, topology_candidates)?;
        Ok(count)
    }

    fn ensure_overlay_indices(&mut self) {
        if self.overlay_indices.is_none() {
            let mut indices = OverlayIndices::default();
            for (index, cell) in self.cells.iter().enumerate() {
                if cell.bridge_deck_mm.is_some() || cell.bridge_underside_mm.is_some() {
                    indices.bridges.insert(index);
                }
                if cell.reef_height_mm != 0 {
                    indices.reefs.insert(index);
                }
            }
            self.overlay_indices = Some(indices);
        }
    }

    fn ensure_connectivity_masks(&mut self) {
        if self.connectivity_masks.len() != self.cells.len() {
            self.connectivity_masks = (0..self.cells.len())
                .map(|i| self.connectivity_mask(i))
                .collect();
        }
    }
    fn connectivity_mask(&self, index: usize) -> u32 {
        let foot = MovementClass::Foot;
        let naval = Role::Boat.movement();
        let mut mask = if self.passable(index, foot) {
            1 << 8
        } else {
            0
        };
        if self.passable(index, naval) {
            mask |= 1 << 24;
        }
        for (bit, (dx, dy)) in DIRS.iter().copied().enumerate() {
            if let Some(n) = self.neighbor(index, dx, dy) {
                if self.edge(index, n, foot).is_some() {
                    mask |= 1 << bit;
                }
                if self.edge(index, n, naval).is_some() {
                    mask |= 1 << (bit + 16);
                }
            }
        }
        mask
    }
    fn commit_dirty(
        &mut self,
        cells: Vec<usize>,
        topology_candidates: Vec<usize>,
    ) -> Result<(), String> {
        if cells.is_empty() {
            return Ok(());
        }
        // Cost and topology are separate. Only actual node/edge eligibility
        // changes invalidate path work; a 100 mm height/cost change alone does not.
        let mut inspect = Vec::with_capacity(topology_candidates.len() * 9);
        for i in topology_candidates {
            inspect.push(i);
            for (dx, dy) in DIRS {
                if let Some(n) = self.neighbor(i, dx, dy) {
                    inspect.push(n);
                }
            }
        }
        inspect.sort_unstable();
        inspect.dedup();
        let mut topology_changes = Vec::new();
        for i in inspect {
            let next = self.connectivity_mask(i);
            let changed = self.connectivity_masks[i] ^ next;
            if changed != 0 {
                topology_changes.push((i, changed));
                self.connectivity_masks[i] = next;
            }
        }
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or("Navigation revision overflow")?;
        if !topology_changes.is_empty() {
            self.topology_revision = self
                .topology_revision
                .checked_add(1)
                .ok_or("Navigation topology revision overflow")?;
        }
        self.changes.push_back(DirtyBatch {
            revision: self.revision,
            cells,
            topology_changes,
        });
        while self.changes.len() > 64 {
            self.changes.pop_front();
        }
        Ok(())
    }

    /// Resolve a target to a nearby compatible point, preserving an already
    /// passable exact point. This query does not teleport or move any pawn.
    pub fn nearest_valid_goal(
        &self,
        position: [f32; 2],
        movement: MovementClass,
        max_radius_cells: usize,
    ) -> Option<[f32; 2]> {
        let point = to_mm(position).ok()?;
        self.nearest_goal_mm(point, movement, max_radius_cells, None)
            .map(|p| [p[0] as f32 * 0.001, p[1] as f32 * 0.001])
    }
    fn nearest_goal_mm(
        &self,
        point: [i32; 2],
        movement: MovementClass,
        max_radius_cells: usize,
        component: Option<(&[u32], u32)>,
    ) -> Option<[i32; 2]> {
        let origin = self.index_mm(point)?;
        let belongs = |i: usize| {
            component.is_none_or(|(labels, wanted)| labels.get(i).copied() == Some(wanted))
        };
        if self.passable(origin, movement) && belongs(origin) {
            return Some(point);
        }
        let x = origin % self.width;
        let y = origin / self.width;
        let pad = match movement {
            MovementClass::Foot => 600,
            MovementClass::Naval { .. } => 3500,
        }
        .min(self.cell_mm / 3);
        let mut best: Option<(i64, usize, [i32; 2])> = None;
        for cy in y.saturating_sub(max_radius_cells)
            ..=y.saturating_add(max_radius_cells).min(self.height - 1)
        {
            for cx in x.saturating_sub(max_radius_cells)
                ..=x.saturating_add(max_radius_cells).min(self.width - 1)
            {
                let i = cy * self.width + cx;
                if !self.passable(i, movement) || !belongs(i) {
                    continue;
                }
                let min = [
                    cx as i32 * self.cell_mm + pad,
                    cy as i32 * self.cell_mm + pad,
                ];
                let max = [
                    (cx + 1) as i32 * self.cell_mm - pad - 1,
                    (cy + 1) as i32 * self.cell_mm - pad - 1,
                ];
                let p = [
                    point[0].clamp(min[0], max[0]),
                    point[1].clamp(min[1], max[1]),
                ];
                let candidate = (distance_sq(p, point), i, p);
                if best.as_ref().is_none_or(|old| candidate < *old) {
                    best = Some(candidate);
                }
            }
        }
        best.map(|(_, _, p)| p)
    }

    pub fn index_mm(&self, position: [i32; 2]) -> Option<usize> {
        if position[0] < 0 || position[1] < 0 {
            return None;
        }
        let x = position[0] as usize / self.cell_mm as usize;
        let y = position[1] as usize / self.cell_mm as usize;
        (x < self.width && y < self.height).then_some(y * self.width + x)
    }
    pub fn index(&self, position: [f32; 2]) -> Option<usize> {
        self.index_mm(to_mm(position).ok()?)
    }
    pub fn center_mm(&self, index: usize) -> [i32; 2] {
        [
            (index % self.width) as i32 * self.cell_mm + self.cell_mm / 2,
            (index / self.width) as i32 * self.cell_mm + self.cell_mm / 2,
        ]
    }
    fn neighbor(&self, index: usize, dx: i32, dy: i32) -> Option<usize> {
        let x = index % self.width;
        let y = index / self.width;
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        (nx >= 0 && ny >= 0 && nx < self.width as i32 && ny < self.height as i32)
            .then_some(ny.max(0) as usize * self.width + nx.max(0) as usize)
    }

    pub fn passable(&self, index: usize, movement: MovementClass) -> bool {
        let Some(c) = self.cells.get(index) else {
            return false;
        };
        match movement {
            MovementClass::Foot => {
                !c.blocked && (c.effective_depth_mm() < WALK_WATER_LIMIT_MM || bridge_dry(*c))
            }
            MovementClass::Naval {
                draft_mm,
                air_draft_mm,
            } => {
                !c.blocked
                    && c.keel_depth_mm() >= draft_mm
                    && c.bridge_underside_mm.is_none_or(|underside| {
                        let water_surface = c.elevation_mm as i64 + c.water_depth_mm as i64;
                        underside as i64 - water_surface >= air_draft_mm as i64
                    })
            }
        }
    }

    pub fn adjacent_reachable(&self, from: usize, to: usize, movement: MovementClass) -> bool {
        if from >= self.cells.len() || to >= self.cells.len() || from == to {
            return false;
        }
        let dx = (from % self.width).abs_diff(to % self.width);
        let dy = (from / self.width).abs_diff(to / self.width);
        dx <= 1 && dy <= 1 && self.edge(from, to, movement).is_some()
    }

    fn effective_height(&self, index: usize) -> i32 {
        self.cells[index]
            .bridge_deck_mm
            .unwrap_or(self.cells[index].elevation_mm + self.cells[index].reef_height_mm as i32)
    }
    fn edge(&self, from: usize, to: usize, movement: MovementClass) -> Option<u32> {
        if !self.passable(from, movement) || !self.passable(to, movement) {
            return None;
        }
        let fx = from % self.width;
        let fy = from / self.width;
        let tx = to % self.width;
        let ty = to / self.width;
        let diagonal = fx != tx && fy != ty;
        if diagonal {
            // Do not cut through a blocked shore/building corner.
            if !self.passable(fy * self.width + tx, movement)
                || !self.passable(ty * self.width + fx, movement)
            {
                return None;
            }
        }
        let mut slope = 0;
        if movement == MovementClass::Foot {
            slope = (self.effective_height(from).div_euclid(100) as i64 * 100
                - self.effective_height(to).div_euclid(100) as i64 * 100)
                .abs()
                * 1000
                / self.cell_mm as i64;
            if slope > MAX_SLOPE_PERMILLE {
                return None;
            }
        }
        let c = self.cells[to];
        let base = match movement {
            MovementClass::Foot if c.road || c.bridge_deck_mm.is_some() => {
                100u32 + traction_band(c) as u32 * 2
            }
            MovementClass::Foot => {
                350 + (c.cover as u32 / 8) * 24 + slope as u32 + traction_band(c) as u32 * 80
            }
            MovementClass::Naval { .. } => 130,
        };
        Some(if diagonal { base * 1414 / 1000 } else { base })
    }

    pub fn line_of_fire(&self, from_mm: [i32; 2], to_mm: [i32; 2]) -> bool {
        let (Some(from), Some(to)) = (self.index_mm(from_mm), self.index_mm(to_mm)) else {
            return false;
        };
        let dx = to_mm[0] as i64 - from_mm[0] as i64;
        let dy = to_mm[1] as i64 - from_mm[1] as i64;
        let steps = ((dx.abs().max(dy.abs()) / (self.cell_mm as i64 / 2).max(1)) + 1).min(4096);
        let h0 = self.effective_height(from) as i64 + 1600;
        let h1 = self.effective_height(to) as i64 + 1600;
        for s in 1..steps {
            let p = [
                (from_mm[0] as i64 + dx * s / steps) as i32,
                (from_mm[1] as i64 + dy * s / steps) as i32,
            ];
            let Some(i) = self.index_mm(p) else {
                return false;
            };
            if i == from || i == to {
                continue;
            }
            if self.cells[i].blocked
                || self.cells[i].elevation_mm as i64 > h0 + (h1 - h0) * s / steps
            {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TacticalAgent {
    pub id: AgentId,
    pub team: u8,
    pub role: Role,
    pub position_mm: [i32; 2],
    pub previous_position_mm: [i32; 2],
    pub health: u16,
    pub ammunition: u16,
    pub goal_mm: Option<[i32; 2]>,
    #[serde(default)]
    pub requested_goal_mm: Option<[i32; 2]>,
    #[serde(default)]
    pub resolved_topology_revision: u64,
    pub formation_offset_mm: [i32; 2],
    pub job: Option<u64>,
    pub target: Option<AgentId>,
    #[serde(default = "default_attack_move")]
    pub attack_move: bool,
    #[serde(default)]
    pub commanded_target: Option<AgentId>,
    pub next_shot_tick: u64,
    pub embarked_on: Option<AgentId>,
    pub arrived: bool,
    /// Continuous deep-water exposure on the authoritative clock.
    #[serde(default)]
    pub water_exposure_ticks: u32,
}
impl TacticalAgent {
    pub fn position(&self) -> [f32; 2] {
        [
            self.position_mm[0] as f32 * 0.001,
            self.position_mm[1] as f32 * 0.001,
        ]
    }
    pub fn alive(&self) -> bool {
        self.health != 0
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct AgentView {
    pub id: AgentId,
    pub team: u8,
    pub role: Role,
    pub position: [f32; 2],
    pub facing: f32,
    pub health: u16,
    pub max_health: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TacticalEvent {
    Arrived {
        agent: AgentId,
        job: Option<u64>,
    },
    Work {
        worker: AgentId,
        job: u64,
        units: u32,
    },
    Shot {
        source: AgentId,
        target: AgentId,
        damage: u16,
    },
    Died {
        agent: AgentId,
        team: u8,
        killer: Option<AgentId>,
    },
    StructureHit {
        source: AgentId,
        structure: u64,
        damage: u16,
    },
    NeedsSupply {
        agent: AgentId,
        team: u8,
    },
    WaterDamage {
        agent: AgentId,
        damage: u16,
        water_depth_mm: u32,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct StructureTarget {
    pub id: u64,
    pub team: u8,
    pub position_mm: [i32; 2],
    pub active: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TacticalTotals {
    pub spawned: [u64; 2],
    pub deaths: [u64; 2],
    pub initial_ammunition: u64,
    pub supplied_ammunition: u64,
    pub fired_ammunition: u64,
    pub lost_ammunition: u64,
    pub route_builds: u64,
    pub route_cache_hits: u64,
    pub route_invalidations: u64,
    pub route_node_expansions: u64,
    #[serde(default)]
    pub route_cost_refreshes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct RouteKey {
    goal: usize,
    movement: MovementClass,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Flow {
    key: RouteKey,
    revision: u64,
    #[serde(default)]
    topology_revision: u64,
    distance: Vec<u32>,
    settled: Vec<u8>,
    required: BTreeSet<usize>,
    heap: BinaryHeap<Reverse<(u32, usize)>>,
    complete: bool,
    last_requested_tick: u64,
    ready_tick: Option<u64>,
    #[serde(default)]
    cost_dirty: bool,
    #[serde(default)]
    last_cost_refresh_tick: u64,
}

impl Flow {
    fn new(key: RouteKey, grid: &NavGrid, tick: u64) -> Self {
        let mut distance = vec![UNREACHED; grid.cells.len()];
        let mut heap = BinaryHeap::new();
        if grid.passable(key.goal, key.movement) {
            distance[key.goal] = 0;
            heap.push(Reverse((0, key.goal)));
        } else {
            // Structures may occupy the requested point. Reach an adjacent
            // perimeter; they do not become passable just to satisfy an order.
            for (dx, dy) in DIRS {
                if let Some(n) = grid.neighbor(key.goal, dx, dy) {
                    if grid.passable(n, key.movement) {
                        distance[n] = 100;
                        heap.push(Reverse((100, n)));
                    }
                }
            }
        }
        Self {
            key,
            revision: grid.revision,
            topology_revision: grid.topology_revision,
            distance,
            settled: vec![0; grid.cells.len()],
            required: BTreeSet::new(),
            heap,
            complete: false,
            last_requested_tick: tick,
            ready_tick: None,
            cost_dirty: false,
            last_cost_refresh_tick: tick,
        }
    }
    fn stale(&self, grid: &NavGrid) -> bool {
        if self.revision == grid.revision || self.topology_revision == grid.topology_revision {
            return false;
        }
        let Some(first) = grid.changes.front() else {
            return true;
        };
        if self.revision + 1 < first.revision {
            return true;
        }
        grid.changes
            .iter()
            .filter(|d| d.revision > self.revision)
            .any(|d| {
                let relevant = if self.key.movement == MovementClass::Foot {
                    FOOT_CONNECTIVITY
                } else {
                    NAVAL_CONNECTIVITY
                };
                d.topology_changes.iter().any(|&(i, changed)| {
                    changed & relevant != 0
                        && (self.distance[i] != UNREACHED
                            || DIRS.iter().any(|&(dx, dy)| {
                                grid.neighbor(i, dx, dy)
                                    .is_some_and(|n| self.distance[n] != UNREACHED)
                            }))
                })
            })
    }
    fn expand(&mut self, grid: &NavGrid, budget: usize) -> usize {
        let mut expanded = 0;
        while expanded < budget {
            let Some(Reverse((cost, i))) = self.heap.pop() else {
                self.complete = true;
                break;
            };
            if cost != self.distance[i] || self.settled[i] != 0 {
                continue;
            }
            self.settled[i] = 1;
            expanded += 1;
            for (dx, dy) in DIRS {
                let Some(n) = grid.neighbor(i, dx, dy) else {
                    continue;
                };
                // Reverse graph: cost is the actual n->i traversal cost.
                let Some(edge) = grid.edge(n, i, self.key.movement) else {
                    continue;
                };
                if let Some(next) = cost.checked_add(edge) {
                    if next < self.distance[n] {
                        self.distance[n] = next;
                        self.heap.push(Reverse((next, n)));
                    }
                }
            }
            if self.required.iter().all(|i| self.settled[*i] != 0) {
                self.complete = true;
                break;
            }
        }
        if self.heap.is_empty() {
            self.complete = true;
        }
        expanded
    }
    fn next_cell(&self, current: usize, grid: &NavGrid) -> Option<usize> {
        if self.settled[current] == 0 || self.distance[current] == UNREACHED {
            return None;
        }
        let mut best = (UNREACHED, current);
        for (dx, dy) in DIRS {
            let Some(n) = grid.neighbor(current, dx, dy) else {
                continue;
            };
            let Some(edge) = grid.edge(current, n, self.key.movement) else {
                continue;
            };
            if self.settled[n] != 0 && self.distance[n] < self.distance[current] {
                if let Some(cost) = self.distance[n].checked_add(edge) {
                    if (cost, n) < best {
                        best = (cost, n);
                    }
                }
            }
        }
        Some(best.1)
    }
}

/// Reused hashed flat bins. Memory scales with active records, not world area.
#[derive(Clone, Debug, Default)]
struct SpatialIndex {
    heads: [Vec<i32>; 2],
    next: Vec<i32>,
    bins: Vec<[i32; 2]>,
    cell_mm: i32,
}
impl SpatialIndex {
    fn bucket(&self, bin: [i32; 2]) -> usize {
        let h = (bin[0] as u32).wrapping_mul(73_856_093) ^ (bin[1] as u32).wrapping_mul(19_349_663);
        h as usize & (self.heads[0].len() - 1)
    }
    fn build(&mut self, agents: &[TacticalAgent], cell_mm: i32) {
        let capacity = (agents.len().saturating_mul(2)).max(16).next_power_of_two();
        self.cell_mm = cell_mm;
        for heads in &mut self.heads {
            heads.resize(capacity, -1);
            heads.fill(-1);
        }
        self.next.resize(agents.len(), -1);
        self.next.fill(-1);
        self.bins.resize(agents.len(), [0; 2]);
        for (i, a) in agents.iter().enumerate() {
            if !a.alive() || a.embarked_on.is_some() {
                continue;
            }
            let bin = [a.position_mm[0] / cell_mm, a.position_mm[1] / cell_mm];
            self.bins[i] = bin;
            let bucket = self.bucket(bin);
            self.next[i] = self.heads[a.team as usize][bucket];
            self.heads[a.team as usize][bucket] = i as i32;
        }
    }
    fn visit_bin(&self, team: usize, bin: [i32; 2], mut f: impl FnMut(usize) -> bool) -> bool {
        let mut i = self.heads[team][self.bucket(bin)];
        while i >= 0 {
            let index = i as usize;
            if self.bins[index] == bin && !f(index) {
                return false;
            }
            i = self.next[index];
        }
        true
    }
}

#[derive(Clone, Debug)]
struct Components {
    revision: u64,
    labels: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tactics {
    pub seed: u32,
    pub last_tick: u64,
    pub agents: Vec<TacticalAgent>,
    pub totals: TacticalTotals,
    pub manifests: BTreeMap<AgentId, Vec<AgentId>>,
    pub structures: Vec<StructureTarget>,
    next_id: u64,
    #[serde(with = "route_serde")]
    routes: BTreeMap<RouteKey, Flow>,
    #[serde(default, with = "route_serde")]
    cost_refreshes: BTreeMap<RouteKey, Flow>,
    #[serde(default)]
    cost_refresh_cursor: usize,
    #[serde(skip)]
    crowd: SpatialIndex,
    #[serde(skip)]
    combat_index: SpatialIndex,
    #[serde(skip)]
    yielded_idle: BTreeSet<usize>,
    #[serde(skip)]
    pending_damage: Vec<u32>,
    #[serde(skip)]
    pending_killers: Vec<Option<AgentId>>,
    route_cursor: usize,
    request_cursor: usize,
    #[serde(skip)]
    components: BTreeMap<MovementClass, Components>,
}

impl Tactics {
    pub fn new(seed: u32) -> Self {
        Self {
            seed,
            last_tick: 0,
            agents: Vec::new(),
            totals: TacticalTotals::default(),
            manifests: BTreeMap::new(),
            structures: Vec::new(),
            next_id: 1,
            routes: BTreeMap::new(),
            cost_refreshes: BTreeMap::new(),
            cost_refresh_cursor: 0,
            crowd: SpatialIndex::default(),
            combat_index: SpatialIndex::default(),
            yielded_idle: BTreeSet::new(),
            pending_damage: Vec::new(),
            pending_killers: Vec::new(),
            route_cursor: 0,
            request_cursor: 0,
            components: BTreeMap::new(),
        }
    }
    pub fn spawn(&mut self, team: u8, role: Role, position: [f32; 2]) -> Result<AgentId, String> {
        if team > 1 {
            return Err("Only the two warring factions are valid".into());
        }
        if self.agents.len() >= i32::MAX as usize {
            return Err("Tactical index capacity exhausted".into());
        }
        let position_mm = to_mm(position)?;
        let id = self.next_id;
        let next = id.checked_add(1).ok_or("Agent identity exhausted")?;
        let spawned = self.totals.spawned[team as usize]
            .checked_add(1)
            .ok_or("Spawn ledger overflow")?;
        let ammunition = self
            .totals
            .initial_ammunition
            .checked_add(role.initial_ammo() as u64)
            .ok_or("Ammunition ledger overflow")?;
        self.agents.push(TacticalAgent {
            id,
            team,
            role,
            position_mm,
            previous_position_mm: position_mm,
            health: role.max_health(),
            ammunition: role.initial_ammo(),
            goal_mm: None,
            requested_goal_mm: None,
            resolved_topology_revision: 0,
            formation_offset_mm: [0; 2],
            job: None,
            target: None,
            attack_move: true,
            commanded_target: None,
            next_shot_tick: 0,
            embarked_on: None,
            arrived: false,
            water_exposure_ticks: 0,
        });
        self.next_id = next;
        self.totals.spawned[team as usize] = spawned;
        self.totals.initial_ammunition = ammunition;
        if role == Role::Transport {
            self.manifests.insert(id, Vec::new());
        }
        Ok(id)
    }
    /// Place the exact requested count on passable, nonoverlapping sites near
    /// the requested origin. Every location is checked before any pawn is
    /// created; insufficient land returns an error and never shrinks an army.
    pub fn spawn_formation(
        &mut self,
        team: u8,
        role: Role,
        center: [f32; 2],
        count: usize,
        grid: &NavGrid,
    ) -> Result<Vec<AgentId>, String> {
        if team > 1 {
            return Err("Invalid faction".into());
        }
        let origin = to_mm(center)?;
        if grid.index_mm(origin).is_none() {
            return Err("Spawn origin outside navigation grid".into());
        }
        if count == 0 {
            return Ok(Vec::new());
        }
        let final_len = self
            .agents
            .len()
            .checked_add(count)
            .ok_or("Agent capacity overflow")?;
        if final_len >= i32::MAX as usize {
            return Err("Tactical index capacity exhausted".into());
        }
        self.next_id
            .checked_add(count as u64)
            .ok_or("Agent identity exhausted")?;
        self.totals.spawned[team as usize]
            .checked_add(count as u64)
            .ok_or("Spawn ledger overflow")?;
        self.totals
            .initial_ammunition
            .checked_add(
                (role.initial_ammo() as u64)
                    .checked_mul(count as u64)
                    .ok_or("Ammunition ledger overflow")?,
            )
            .ok_or("Ammunition ledger overflow")?;
        let movement = role.movement();
        let spacing = if movement == MovementClass::Foot {
            4000
        } else {
            9000
        };
        let radius = role.radius_mm();
        let mut occupied: BTreeMap<[i32; 2], Vec<([i32; 2], MovementClass, i32)>> = BTreeMap::new();
        for a in &self.agents {
            if a.alive() && a.embarked_on.is_none() {
                occupied
                    .entry([a.position_mm[0] / 8000, a.position_mm[1] / 8000])
                    .or_default()
                    .push((a.position_mm, a.role.movement(), a.role.radius_mm()));
            }
        }
        let mut proposed = Vec::with_capacity(count);
        let max_ring =
            (grid.width.max(grid.height) as i64 * grid.cell_mm as i64 / spacing as i64 + 2) as i32;
        'rings: for ring in 0..=max_ring {
            for y in -ring..=ring {
                for x in -ring..=ring {
                    if ring > 0 && x.abs() != ring && y.abs() != ring {
                        continue;
                    }
                    let px = origin[0] as i64 + x as i64 * spacing as i64;
                    let py = origin[1] as i64 + y as i64 * spacing as i64;
                    if px < 0
                        || py < 0
                        || px >= grid.width as i64 * grid.cell_mm as i64
                        || py >= grid.height as i64 * grid.cell_mm as i64
                    {
                        continue;
                    }
                    let position = [px as i32, py as i32];
                    if !grid
                        .index_mm(position)
                        .is_some_and(|i| grid.passable(i, movement))
                    {
                        continue;
                    }
                    let bin = [position[0] / 8000, position[1] / 8000];
                    let mut clear = true;
                    'near: for by in -1..=1 {
                        for bx in -1..=1 {
                            if let Some(bodies) = occupied.get(&[bin[0] + bx, bin[1] + by]) {
                                for &(other, other_movement, other_radius) in bodies {
                                    if other_movement == movement
                                        && distance_sq(position, other)
                                            < ((radius + other_radius) as i64).pow(2)
                                    {
                                        clear = false;
                                        break 'near;
                                    }
                                }
                            }
                        }
                    }
                    if clear {
                        proposed.push(position);
                        occupied
                            .entry(bin)
                            .or_default()
                            .push((position, movement, radius));
                        if proposed.len() == count {
                            break 'rings;
                        }
                    }
                }
            }
        }
        if proposed.len() != count {
            return Err(format!(
                "Requested{count} pawns but map has only{} available compatible spawn sites; no pawns spawned",
                proposed.len()
            ));
        }
        let mut ids = Vec::with_capacity(count);
        for p in proposed {
            ids.push(self.spawn(team, role, [p[0] as f32 * 0.001, p[1] as f32 * 0.001])?);
        }
        Ok(ids)
    }

    fn idx(&self, id: AgentId) -> Result<usize, String> {
        let i = id
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or("Invalid agent identity")?;
        if self.agents.get(i).is_some_and(|a| a.id == id) {
            Ok(i)
        } else {
            Err("Unknown agent identity".into())
        }
    }
    pub fn agent(&self, id: AgentId) -> Option<&TacticalAgent> {
        self.idx(id).ok().map(|i| &self.agents[i])
    }
    pub fn agent_mut(&mut self, id: AgentId) -> Option<&mut TacticalAgent> {
        let index = self.idx(id).ok()?;
        self.agents.get_mut(index)
    }

    pub fn assign(&mut self, id: AgentId, goal: [f32; 2], job: Option<u64>) -> Result<(), String> {
        let goal = to_mm(goal)?;
        let i = self.idx(id)?;
        let a = &mut self.agents[i];
        if !a.alive() || a.embarked_on.is_some() {
            return Err("Cannot order a dead or embarked pawn".into());
        }
        if a.goal_mm == Some(goal) && a.job == job && a.formation_offset_mm == [0; 2] {
            return Ok(());
        }
        a.goal_mm = Some(goal);
        a.requested_goal_mm = Some(goal);
        a.resolved_topology_revision = 0;
        a.job = job;
        a.formation_offset_mm = [0; 2];
        a.arrived = false;
        Ok(())
    }
    /// A group shares its route goal; offsets apply only near that goal.
    pub fn order(&mut self, team: u8, goal: [f32; 2]) -> Result<usize, String> {
        if team > 1 {
            return Err("Invalid faction".into());
        }
        let goal = to_mm(goal)?;
        let count = self
            .agents
            .iter()
            .filter(|a| {
                a.team == team && a.alive() && a.embarked_on.is_none() && a.role != Role::Worker
            })
            .count();
        let width = (count as f64).sqrt().ceil().max(1.) as usize;
        let mut slot = 0usize;
        for a in &mut self.agents {
            if a.team != team || !a.alive() || a.embarked_on.is_some() || a.role == Role::Worker {
                continue;
            }
            a.goal_mm = Some(goal);
            a.requested_goal_mm = Some(goal);
            a.resolved_topology_revision = 0;
            a.job = None;
            a.arrived = false;
            let spacing = if a.role.movement() == MovementClass::Foot {
                6000
            } else {
                9000
            };
            a.formation_offset_mm = [
                (slot % width) as i32 * spacing - (width as i32 - 1) * spacing / 2,
                (slot / width) as i32 * spacing - (width as i32 - 1) * spacing / 2,
            ];
            slot += 1;
        }
        Ok(count)
    }
    /// Validate every requested destination before changing an army order.
    /// Foot and naval classes resolve separately; disconnected armies receive
    /// an explicit error instead of a permanently unfinished water-center goal.
    pub fn order_on_grid(
        &mut self,
        team: u8,
        goal: [f32; 2],
        grid: &NavGrid,
    ) -> Result<usize, String> {
        let ids: Vec<_> = self
            .agents
            .iter()
            .filter(|a| {
                a.team == team && a.alive() && a.embarked_on.is_none() && a.role != Role::Worker
            })
            .map(|a| a.id)
            .collect();
        self.order_selected_on_grid(team, &ids, goal, grid, true)
    }
    /// Explicit selection never expands into a whole-faction order. Every
    /// identity/destination is checked before any authoritative order changes.
    pub fn order_selected_on_grid(
        &mut self,
        team: u8,
        ids: &[AgentId],
        goal: [f32; 2],
        grid: &NavGrid,
        attack_move: bool,
    ) -> Result<usize, String> {
        if team > 1 {
            return Err("Selection is not available".into());
        }
        let requested = to_mm(goal)?;
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        if sorted.windows(2).any(|w| w[0] == w[1]) {
            return Err("Selection is not available".into());
        }
        let mut indices = Vec::with_capacity(sorted.len());
        for id in sorted {
            let index = self.idx(id).map_err(|_| "Selection is not available")?;
            let a = self.agents[index];
            if a.team != team || !a.alive() || a.embarked_on.is_some() {
                return Err("Selection is not available".into());
            }
            indices.push(index);
        }
        let mut plans = Vec::with_capacity(indices.len());
        let mut resolved = BTreeMap::new();
        for &index in &indices {
            let a = self.agents[index];
            let movement = a.role.movement();
            let source = grid
                .index_mm(a.position_mm)
                .ok_or("Selection cannot reach that destination")?;
            let cached_component = self
                .components
                .get(&movement)
                .filter(|c| {
                    c.revision == grid.topology_revision && c.labels.len() == grid.cells.len()
                })
                .map(|c| c.labels[source]);
            let point = if let Some(point) = cached_component
                .and_then(|label| resolved.get(&(movement, label)))
                .copied()
            {
                point
            } else {
                let point = self.resolve_for_agent(index, requested, grid, 32)?;
                let component = self.components.get(&movement).unwrap().labels[source];
                resolved.insert((movement, component), point);
                point
            };
            plans.push(point);
        }
        let count = indices.len();
        let width = (count as f64).sqrt().ceil().max(1.) as i32;
        for (slot, (index, point)) in indices.into_iter().zip(plans).enumerate() {
            let a = &mut self.agents[index];
            let spacing = if a.role.movement() == MovementClass::Foot {
                6000
            } else {
                9000
            };
            let offset = [
                (slot as i32 % width) * spacing - (width - 1) * spacing / 2,
                (slot as i32 / width) * spacing - (width - 1) * spacing / 2,
            ];
            let unchanged = a.goal_mm == Some(point)
                && a.requested_goal_mm == Some(requested)
                && a.formation_offset_mm == offset
                && a.attack_move == attack_move
                && a.job.is_none();
            a.goal_mm = Some(point);
            a.requested_goal_mm = Some(requested);
            a.resolved_topology_revision = grid.topology_revision;
            a.formation_offset_mm = offset;
            a.attack_move = attack_move;
            a.commanded_target = None;
            a.job = None;
            if !unchanged {
                a.arrived = false;
            }
        }
        Ok(count)
    }
    pub fn assign_on_grid(
        &mut self,
        id: AgentId,
        goal: [f32; 2],
        job: Option<u64>,
        grid: &NavGrid,
    ) -> Result<[f32; 2], String> {
        self.assign_on_grid_within(id, goal, job, grid, 512.)
    }
    /// Physical jobs use their real reach here, so a snapped route cannot
    /// falsely complete construction or capture from a distant opposite bank.
    pub fn assign_on_grid_within(
        &mut self,
        id: AgentId,
        goal: [f32; 2],
        job: Option<u64>,
        grid: &NavGrid,
        maximum_offset_m: f32,
    ) -> Result<[f32; 2], String> {
        if !maximum_offset_m.is_finite() || maximum_offset_m < 0. || maximum_offset_m > 512. {
            return Err("Invalid goal resolution reach".into());
        }
        let requested = to_mm(goal)?;
        let index = self.idx(id)?;
        let a = self.agents[index];
        if !a.alive() || a.embarked_on.is_some() {
            return Err("Cannot order a dead or embarked pawn".into());
        }
        if a.requested_goal_mm == Some(requested)
            && a.job == job
            && a.resolved_topology_revision == grid.topology_revision
        {
            if let Some(point) = a.goal_mm {
                return Ok([point[0] as f32 * 0.001, point[1] as f32 * 0.001]);
            }
        }
        let radius = ((maximum_offset_m * 1000. / grid.cell_mm as f32).ceil() as usize).max(1);
        let point = self.resolve_for_agent(index, requested, grid, radius)?;
        if distance_sq(point, requested) as f64 > (maximum_offset_m as f64 * 1000.).powi(2) {
            return Err("Nearest reachable stand point exceeds this job's physical reach".into());
        }
        let resolved = [point[0] as f32 * 0.001, point[1] as f32 * 0.001];
        self.assign(id, resolved, job)?;
        let a = &mut self.agents[index];
        a.requested_goal_mm = Some(requested);
        a.resolved_topology_revision = grid.topology_revision;
        Ok(resolved)
    }
    fn resolve_for_agent(
        &mut self,
        index: usize,
        requested: [i32; 2],
        grid: &NavGrid,
        radius: usize,
    ) -> Result<[i32; 2], String> {
        let a = self.agents[index];
        let movement = a.role.movement();
        let source = grid
            .index_mm(a.position_mm)
            .ok_or("Pawn outside navigation grid")?;
        if !grid.passable(source, movement) {
            return Err("Pawn currently has no standable route origin; evacuation or restored clearance is required".into());
        }
        if self.components.get(&movement).is_none_or(|c| {
            c.revision != grid.topology_revision || c.labels.len() != grid.cells.len()
        }) {
            let mut labels = vec![0u32; grid.cells.len()];
            let mut queue = VecDeque::new();
            let mut component = 0u32;
            for start in 0..grid.cells.len() {
                if labels[start] != 0 || !grid.passable(start, movement) {
                    continue;
                }
                component = component
                    .checked_add(1)
                    .ok_or("Navigation component capacity exhausted")?;
                labels[start] = component;
                queue.push_back(start);
                while let Some(i) = queue.pop_front() {
                    for (dx, dy) in DIRS {
                        if let Some(n) = grid.neighbor(i, dx, dy) {
                            if labels[n] == 0 && grid.edge(i, n, movement).is_some() {
                                labels[n] = component;
                                queue.push_back(n);
                            }
                        }
                    }
                }
            }
            self.components.insert(
                movement,
                Components {
                    revision: grid.topology_revision,
                    labels,
                },
            );
        }
        let components = self.components.get(&movement).unwrap();
        let label = components.labels[source];
        grid.nearest_goal_mm(requested,movement,radius,Some((&components.labels,label))).ok_or("No reachable compatible destination near that order; a bridge or transport may be required".into())
    }

    pub fn stop(&mut self, id: AgentId) -> Result<(), String> {
        let i = self.idx(id)?;
        self.agents[i].goal_mm = None;
        self.agents[i].requested_goal_mm = None;
        self.agents[i].resolved_topology_revision = 0;
        self.agents[i].job = None;
        self.agents[i].commanded_target = None;
        self.agents[i].arrived = false;
        Ok(())
    }
    pub fn set_structure_targets(
        &mut self,
        structures: Vec<StructureTarget>,
    ) -> Result<(), String> {
        if structures.iter().any(|s| s.team > 1) {
            return Err("Invalid structure faction".into());
        }
        self.structures = structures;
        Ok(())
    }
    /// Caller removes exactly the accepted amount from its economy stock.
    pub fn give_ammunition(&mut self, id: AgentId, offered: u16) -> Result<u16, String> {
        let i = self.idx(id)?;
        let a = &self.agents[i];
        if !a.alive() || a.role.range_mm() == 0 {
            return Ok(0);
        }
        let accepted = offered.min(100 - a.ammunition);
        let total = self
            .totals
            .supplied_ammunition
            .checked_add(accepted as u64)
            .ok_or("Ammo supply ledger overflow")?;
        self.agents[i].ammunition += accepted;
        self.totals.supplied_ammunition = total;
        Ok(accepted)
    }
    pub fn board(&mut self, passenger: AgentId, transport: AgentId) -> Result<(), String> {
        let p = self.idx(passenger)?;
        let t = self.idx(transport)?;
        let a = &self.agents[p];
        let b = &self.agents[t];
        if p == t
            || !a.alive()
            || !b.alive()
            || a.team != b.team
            || a.role.movement() != MovementClass::Foot
            || a.embarked_on.is_some()
            || b.role != Role::Transport
            || distance_sq(a.position_mm, b.position_mm) > 12_000i64.pow(2)
        {
            return Err("Passenger cannot board this transport".into());
        }
        let manifest = self
            .manifests
            .get_mut(&transport)
            .ok_or("Transport manifest missing")?;
        if manifest.len() >= 20 {
            return Err("Transport capacity reached".into());
        }
        manifest.push(passenger);
        self.agents[p].embarked_on = Some(transport);
        self.agents[p].water_exposure_ticks = 0;
        self.agents[p].job = None;
        self.agents[p].goal_mm = None;
        Ok(())
    }
    pub fn disembark(
        &mut self,
        passenger: AgentId,
        position: [f32; 2],
        grid: &NavGrid,
    ) -> Result<(), String> {
        let pos = to_mm(position)?;
        let p = self.idx(passenger)?;
        let a = &self.agents[p];
        let vessel = a.embarked_on.ok_or("Passenger is not embarked")?;
        let t = self.idx(vessel)?;
        if !a.alive()
            || !self.agents[t].alive()
            || distance_sq(pos, self.agents[t].position_mm) > 24_000i64.pow(2)
            || !grid
                .index_mm(pos)
                .is_some_and(|i| grid.passable(i, MovementClass::Foot))
        {
            return Err("No reachable dry disembark point".into());
        }
        if self.agents.iter().any(|b| {
            b.alive()
                && b.embarked_on.is_none()
                && b.role.movement() == MovementClass::Foot
                && distance_sq(pos, b.position_mm)
                    < ((a.role.radius_mm() + b.role.radius_mm()) as i64).pow(2)
        }) {
            return Err("Disembark point is occupied".into());
        }
        let manifest = self
            .manifests
            .get_mut(&vessel)
            .ok_or("Transport manifest missing")?;
        let slot = manifest
            .iter()
            .position(|id| *id == passenger)
            .ok_or("Passenger ownership mismatch")?;
        manifest.remove(slot);
        self.agents[p].embarked_on = None;
        self.agents[p].position_mm = pos;
        self.agents[p].previous_position_mm = pos;
        Ok(())
    }
    /// Prevalidate an entire real manifest handoff before moving any passenger.
    pub fn disembark_group(
        &mut self,
        placements: &[(AgentId, [f32; 2])],
        grid: &NavGrid,
    ) -> Result<(), String> {
        let mut seen = BTreeSet::new();
        let mut planned = Vec::with_capacity(placements.len());
        for &(id, point) in placements {
            if !seen.insert(id) {
                return Err("Duplicate disembark passenger".into());
            }
            let p = self.idx(id)?;
            let a = self.agents[p];
            let vessel = a.embarked_on.ok_or("Passenger is not embarked")?;
            let t = self.idx(vessel)?;
            let v = self.agents[t];
            let pos = to_mm(point)?;
            if !a.alive()
                || !v.alive()
                || a.team != v.team
                || v.role != Role::Transport
                || a.role.movement() != MovementClass::Foot
                || distance_sq(pos, v.position_mm) > 24_000i64.pow(2)
                || !grid
                    .index_mm(pos)
                    .is_some_and(|i| grid.passable(i, MovementClass::Foot))
                || !self
                    .manifests
                    .get(&vessel)
                    .is_some_and(|m| m.iter().filter(|&&other| other == id).count() == 1)
            {
                return Err("Manifest cannot reach those landing sites".into());
            }
            if self.agents.iter().any(|b| {
                b.alive()
                    && b.embarked_on.is_none()
                    && b.role.movement() == MovementClass::Foot
                    && distance_sq(pos, b.position_mm)
                        < ((a.role.radius_mm() + b.role.radius_mm()) as i64).pow(2)
            }) || planned
                .iter()
                .any(|&(other, other_pos): &(usize, [i32; 2])| {
                    distance_sq(pos, other_pos)
                        < ((a.role.radius_mm() + self.agents[other].role.radius_mm()) as i64).pow(2)
                })
            {
                return Err("Disembark site is occupied".into());
            }
            planned.push((p, pos));
        }
        for (p, pos) in planned {
            let vessel = self.agents[p]
                .embarked_on
                .expect("validated manifest owner");
            let id = self.agents[p].id;
            self.manifests
                .get_mut(&vessel)
                .expect("validated manifest")
                .retain(|other| *other != id);
            let a = &mut self.agents[p];
            a.embarked_on = None;
            a.position_mm = pos;
            a.previous_position_mm = pos;
            a.goal_mm = None;
            a.requested_goal_mm = None;
            a.commanded_target = None;
            a.arrived = false;
        }
        Ok(())
    }

    pub fn views(&self) -> Vec<AgentView> {
        self.agents
            .iter()
            .filter(|a| a.alive() && a.embarked_on.is_none())
            .map(|a| {
                let dx = a.position_mm[0] - a.previous_position_mm[0];
                let dy = a.position_mm[1] - a.previous_position_mm[1];
                AgentView {
                    id: a.id,
                    team: a.team,
                    role: a.role,
                    position: a.position(),
                    facing: (dy as f32).atan2(dx as f32),
                    health: a.health,
                    max_health: a.role.max_health(),
                }
            })
            .collect()
    }
    pub fn alive_count(&self, team: u8) -> usize {
        self.agents
            .iter()
            .filter(|a| a.team == team && a.alive())
            .count()
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.next_id != self.agents.len() as u64 + 1 {
            return Err("Agent identity sequence mismatch".into());
        }
        let mut living = [0u64; 2];
        let mut dead = [0u64; 2];
        let mut ammo = 0u64;
        let mut carried = BTreeSet::new();
        for (i, a) in self.agents.iter().enumerate() {
            if a.id != i as u64 + 1
                || a.team > 1
                || a.health > a.role.max_health()
                || a.ammunition > 100
            {
                return Err("Invalid tactical record".into());
            }
            if a.alive() {
                living[a.team as usize] += 1
            } else {
                dead[a.team as usize] += 1;
            }
            ammo = ammo
                .checked_add(a.ammunition as u64)
                .ok_or("Ammo stock overflow")?;
            if a.position_mm.iter().any(|p| *p < 0) {
                return Err("Negative tactical position".into());
            }
        }
        for team in 0..2 {
            if self.totals.spawned[team] != living[team] + dead[team]
                || self.totals.deaths[team] != dead[team]
            {
                return Err("Tactical population is not conserved".into());
            }
        }
        if self
            .totals
            .initial_ammunition
            .checked_add(self.totals.supplied_ammunition)
            != ammo
                .checked_add(self.totals.fired_ammunition)
                .and_then(|n| n.checked_add(self.totals.lost_ammunition))
        {
            return Err("Tactical ammunition is not conserved".into());
        }
        for (&vessel, manifest) in &self.manifests {
            let v = self.agent(vessel).ok_or("Manifest vessel missing")?;
            if v.role != Role::Transport || manifest.len() > 20 {
                return Err("Invalid transport manifest".into());
            }
            for &id in manifest {
                let a = self.agent(id).ok_or("Manifest passenger missing")?;
                if !carried.insert(id)
                    || !a.alive()
                    || !v.alive()
                    || a.embarked_on != Some(vessel)
                    || a.team != v.team
                {
                    return Err("Duplicated/invalid passenger ownership".into());
                }
            }
        }
        for a in &self.agents {
            if a.embarked_on.is_some() != carried.contains(&a.id) {
                return Err("Passenger missing from owning manifest".into());
            }
        }
        Ok(())
    }

    pub fn step(&mut self, tick: u64, grid: &NavGrid) -> Result<Vec<TacticalEvent>, String> {
        if self.last_tick.checked_add(1) != Some(tick) {
            return Err("Tactics requires each authoritative20Hz tick exactly once".into());
        }
        if self.agents.iter().any(|a| {
            a.alive()
                && a.embarked_on.is_none()
                && (grid.index_mm(a.position_mm).is_none()
                    || a.goal_mm.is_some_and(|g| grid.index_mm(g).is_none()))
        }) {
            return Err("Tactical pawn/order outside navigation grid".into());
        }
        if self
            .routes
            .values()
            .chain(self.cost_refreshes.values())
            .any(|f| {
                f.distance.len() != grid.cells.len()
                    || f.settled.len() != grid.cells.len()
                    || f.key.goal >= grid.cells.len()
            })
        {
            return Err("Saved shared route dimensions differ from navigation grid".into());
        }
        let mut events = Vec::new();
        self.apply_water_hazards(tick, grid, &mut events)?;
        self.refresh_routes(tick, grid)?;
        self.crowd.build(&self.agents, 4000);
        self.yielded_idle.clear();
        self.combat_index.build(&self.agents, 16_000);
        for i in 0..self.agents.len() {
            if !self.agents[i].alive() || self.agents[i].embarked_on.is_some() {
                continue;
            }
            if grid.index_mm(self.agents[i].position_mm).is_none() {
                return Err("Tactical pawn lies outside navigation grid".into());
            }
            if (tick + self.agents[i].id) % 20 == 0 && self.agents[i].role.range_mm() > 0 {
                self.agents[i].target = self.acquire_target(i, grid).map(|j| self.agents[j].id);
            }
            if !self.evacuate_flood(i, grid)? {
                self.move_agent(i, grid, &mut events)?;
            }
        }
        // Positions of passengers are a derived copy of their vessel location;
        // each identity remains owned once and is absent from spatial queries.
        for i in 0..self.agents.len() {
            if let Some(vessel) = self.agents[i].embarked_on {
                let t = self.idx(vessel)?;
                self.agents[i].previous_position_mm = self.agents[i].position_mm;
                self.agents[i].position_mm = self.agents[t].position_mm;
            }
        }
        self.pending_damage.resize(self.agents.len(), 0);
        self.pending_damage.fill(0);
        self.pending_killers.resize(self.agents.len(), None);
        self.pending_killers.fill(None);
        // Commit damage after every shooter inspected the same living snapshot.
        for i in 0..self.agents.len() {
            self.fire(i, tick, grid, &mut events)?;
        }
        for i in 0..self.agents.len() {
            let damage = self.pending_damage[i];
            if damage == 0 || !self.agents[i].alive() {
                continue;
            }
            if damage >= self.agents[i].health as u32 {
                self.kill(i, self.pending_killers[i], &mut events)?;
            } else {
                self.agents[i].health -= damage as u16;
            }
        }
        self.last_tick = tick;
        Ok(events)
    }

    /// External combat/environment transactions call this exactly once at an
    /// authoritative tick boundary. Repeated damage to an existing corpse has
    /// no extra death/ammunition effects.
    pub fn damage(
        &mut self,
        id: AgentId,
        amount: u16,
        killer: Option<AgentId>,
    ) -> Result<Vec<TacticalEvent>, String> {
        let i = self.idx(id)?;
        let mut events = Vec::new();
        if !self.agents[i].alive() || amount == 0 {
            return Ok(events);
        }
        if amount >= self.agents[i].health {
            self.kill(i, killer, &mut events)?;
        } else {
            self.agents[i].health -= amount;
        }
        Ok(events)
    }
    fn apply_water_hazards(
        &mut self,
        tick: u64,
        grid: &NavGrid,
        events: &mut Vec<TacticalEvent>,
    ) -> Result<(), String> {
        for i in 0..self.agents.len() {
            let a = self.agents[i];
            if !a.alive() || a.embarked_on.is_some() || a.role.movement() != MovementClass::Foot {
                continue;
            }
            let cell = grid.cells[grid
                .index_mm(a.position_mm)
                .ok_or("Pawn outside water-hazard grid")?];
            if cell.effective_depth_mm() < 1500 || bridge_dry(cell) {
                self.agents[i].water_exposure_ticks = 0;
                continue;
            }
            let exposure = a
                .water_exposure_ticks
                .checked_add(1)
                .ok_or("Water exposure clock overflow")?;
            self.agents[i].water_exposure_ticks = exposure;
            // A finite grace period allows neighboring-bank evacuation. Deep
            // water is still harmful when stationary/unseen/without orders.
            if exposure > 200 && exposure % 20 == 0 {
                let damage = 3.min(a.health);
                events.push(TacticalEvent::WaterDamage {
                    agent: a.id,
                    damage,
                    water_depth_mm: cell.effective_depth_mm(),
                });
                if damage == a.health {
                    self.kill(i, None, events)?;
                } else {
                    self.agents[i].health -= damage;
                }
            }
        }
        let _ = tick; // cadence derives from continuous exposure, not frame rate.
        Ok(())
    }
    fn evacuate_flood(&mut self, index: usize, grid: &NavGrid) -> Result<bool, String> {
        let a = self.agents[index];
        if a.role.movement() != MovementClass::Foot {
            return Ok(false);
        }
        let current = grid
            .index_mm(a.position_mm)
            .ok_or("Flooded pawn outside map")?;
        if grid.cells[current].effective_depth_mm() < WALK_WATER_LIMIT_MM
            || bridge_dry(grid.cells[current])
        {
            return Ok(false);
        }
        self.agents[index].previous_position_mm = a.position_mm;
        self.agents[index].arrived = false;
        let escape = DIRS
            .iter()
            .filter_map(|&(dx, dy)| grid.neighbor(current, dx, dy))
            .filter(|&n| {
                grid.passable(n, MovementClass::Foot)
                    && (grid.effective_height(n) as i64 - grid.effective_height(current) as i64)
                        .abs()
                        * 1000
                        <= grid.cell_mm as i64 * MAX_SLOPE_PERMILLE
            })
            .min_by_key(|&n| (distance_sq(a.position_mm, grid.center_mm(n)), n));
        let Some(escape) = escape else {
            return Ok(true);
        };
        let to = grid.center_mm(escape);
        let distance = integer_sqrt(distance_sq(a.position_mm, to) as u64) as i64;
        if distance == 0 {
            return Ok(true);
        }
        let stride = 90.min(distance);
        let candidate = [
            a.position_mm[0] + ((to[0] - a.position_mm[0]) as i64 * stride / distance) as i32,
            a.position_mm[1] + ((to[1] - a.position_mm[1]) as i64 * stride / distance) as i32,
        ];
        let cell = grid.index_mm(candidate).ok_or("Evacuation outside map")?;
        if (cell == current || cell == escape) && self.clear_of_others(index, candidate) {
            self.agents[index].position_mm = candidate;
        }
        Ok(true)
    }

    fn refresh_routes(&mut self, tick: u64, grid: &NavGrid) -> Result<(), String> {
        let mut sources: BTreeMap<RouteKey, BTreeSet<usize>> = BTreeMap::new();
        for a in &self.agents {
            if !a.alive() || a.embarked_on.is_some() || a.water_exposure_ticks > 0 {
                continue;
            }
            if let Some(goal) = a.goal_mm {
                let goal = grid
                    .index_mm(goal)
                    .ok_or("Ordered destination outside map")?;
                let current = grid
                    .index_mm(a.position_mm)
                    .ok_or("Pawn outside navigation grid")?;
                sources
                    .entry(RouteKey {
                        goal,
                        movement: a.role.movement(),
                    })
                    .or_default()
                    .insert(current);
            }
        }
        let requested: BTreeSet<_> = sources.keys().copied().collect();
        // Only changed traversable nodes/edges invalidate an in-flight field.
        // Roads, cover, mud and sub-threshold bed changes are advisory cost
        // updates. Restarting on those starves bounded searches in a living world.
        let stale: Vec<_> = self
            .routes
            .iter()
            .filter(|(_, f)| f.stale(grid))
            .map(|(k, _)| *k)
            .collect();
        for key in stale {
            self.routes.remove(&key);
            self.cost_refreshes.remove(&key);
            self.totals.route_invalidations = self
                .totals
                .route_invalidations
                .checked_add(1)
                .ok_or("Route counter overflow")?;
        }
        self.cost_refreshes.retain(|key, f| {
            requested.contains(key) && self.routes.contains_key(key) && !f.stale(grid)
        });
        for f in self
            .routes
            .values_mut()
            .chain(self.cost_refreshes.values_mut())
        {
            if f.revision != grid.revision {
                // Naval edges have constant costs. Their clearance changes are
                // handled immediately by topology invalidation above.
                f.cost_dirty |= f.key.movement == MovementClass::Foot;
                f.revision = grid.revision;
                f.topology_revision = grid.topology_revision;
            }
        }
        let requested_keys: Vec<_> = requested.iter().copied().collect();
        for offset in 0..requested_keys.len() {
            let key = requested_keys[(self.request_cursor + offset) % requested_keys.len()];
            if let Some(f) = self.routes.get_mut(&key) {
                f.last_requested_tick = tick;
                self.totals.route_cache_hits = self
                    .totals
                    .route_cache_hits
                    .checked_add(1)
                    .ok_or("Route counter overflow")?;
                continue;
            }
            if self.routes.len() >= MAX_ROUTES {
                // Excess distinct destinations wait fairly; no pawn is removed.
                let victim = self
                    .routes
                    .iter()
                    .filter(|(k, f)| {
                        !requested.contains(k)
                            || (f.complete
                                && f.ready_tick
                                    .is_some_and(|ready| tick.saturating_sub(ready) >= 8))
                    })
                    .min_by_key(|(k, f)| (f.ready_tick.unwrap_or(f.last_requested_tick), **k))
                    .map(|(k, _)| *k);
                if let Some(victim) = victim {
                    self.routes.remove(&victim);
                    self.cost_refreshes.remove(&victim);
                } else {
                    continue;
                }
            }
            self.routes.insert(key, Flow::new(key, grid, tick));
            self.totals.route_builds = self
                .totals
                .route_builds
                .checked_add(1)
                .ok_or("Route counter overflow")?;
        }
        if !requested_keys.is_empty() {
            self.request_cursor = (self.request_cursor + 1) % requested_keys.len();
        }
        for (key, f) in self.routes.iter_mut().chain(self.cost_refreshes.iter_mut()) {
            f.required = sources.get(key).cloned().unwrap_or_default();
            f.complete = f.heap.is_empty() || f.required.iter().all(|i| f.settled[*i] != 0);
            if f.complete && f.ready_tick.is_none() {
                f.ready_tick = Some(tick);
            }
        }
        // Allocate only to unfinished jobs. Ready fields used to dilute the
        // budget, even though they consumed no expansions themselves.
        let foreground: Vec<_> = self
            .routes
            .iter()
            .filter(|(k, f)| requested.contains(k) && !f.complete)
            .map(|(k, _)| *k)
            .collect();
        let mut budget = ROUTE_EXPANSIONS_PER_TICK;
        let foreground_limit = if self.cost_refreshes.is_empty() {
            budget
        } else {
            budget * 3 / 4
        };
        let mut foreground_budget = foreground_limit;
        for offset in 0..foreground.len() {
            let key = foreground[(self.route_cursor + offset) % foreground.len()];
            let share = foreground_budget.div_ceil(foreground.len() - offset);
            let flow = self.routes.get_mut(&key).unwrap();
            let used = flow.expand(grid, share);
            if flow.complete && flow.ready_tick.is_none() {
                flow.ready_tick = Some(tick);
            }
            foreground_budget -= used;
            budget -= used;
        }
        if !foreground.is_empty() {
            self.route_cursor = (self.route_cursor + 1) % foreground.len();
        }
        // Rebuild costs beside the live field, never in its place. Even if
        // moisture changes every tick, this replacement finishes and commits.
        // Its own dirty marker schedules another bounded refresh later.
        for (&key, flow) in &self.routes {
            if requested.contains(&key)
                && flow.complete
                && flow.cost_dirty
                && tick.saturating_sub(flow.last_cost_refresh_tick) >= COST_REFRESH_TICKS
                && (tick % 20 + key.goal as u64 % 20) % 20 == 0
                && !self.cost_refreshes.contains_key(&key)
            {
                let mut replacement = Flow::new(key, grid, tick);
                replacement.required = sources[&key].clone();
                self.cost_refreshes.insert(key, replacement);
            }
        }
        let background: Vec<_> = self
            .cost_refreshes
            .iter()
            .filter(|(_, f)| !f.complete)
            .map(|(k, _)| *k)
            .collect();
        for offset in 0..background.len() {
            let key = background[(self.cost_refresh_cursor + offset) % background.len()];
            let share = budget.div_ceil(background.len() - offset);
            let used = self
                .cost_refreshes
                .get_mut(&key)
                .unwrap()
                .expand(grid, share);
            budget -= used;
        }
        if !background.is_empty() {
            self.cost_refresh_cursor = (self.cost_refresh_cursor + 1) % background.len();
        }
        let ready: Vec<_> = self
            .cost_refreshes
            .iter()
            .filter(|(_, f)| f.complete)
            .map(|(k, _)| *k)
            .collect();
        for key in ready {
            let mut replacement = self.cost_refreshes.remove(&key).unwrap();
            replacement.last_cost_refresh_tick = tick;
            replacement.ready_tick = Some(tick);
            self.routes.insert(key, replacement);
            self.totals.route_cost_refreshes = self
                .totals
                .route_cost_refreshes
                .checked_add(1)
                .ok_or("Route counter overflow")?;
        }
        self.totals.route_node_expansions = self
            .totals
            .route_node_expansions
            .checked_add((ROUTE_EXPANSIONS_PER_TICK - budget) as u64)
            .ok_or("Route counter overflow")?;
        Ok(())
    }
    fn acquire_target(&self, index: usize, grid: &NavGrid) -> Option<usize> {
        let a = &self.agents[index];
        let range = a.role.range_mm();
        if let Some(i) = a.commanded_target.and_then(|id| self.idx(id).ok()) {
            let target = &self.agents[i];
            if target.alive()
                && target.team != a.team
                && target.embarked_on.is_none()
                && distance_sq(a.position_mm, target.position_mm) <= (range as i64).pow(2)
                && grid.line_of_fire(a.position_mm, target.position_mm)
            {
                return Some(i);
            }
        }

        let origin = [a.position_mm[0] / 16_000, a.position_mm[1] / 16_000];
        let rings = (range + 15_999) / 16_000;
        let mut best = None;
        let mut best_distance = range as i64 * range as i64 + 1;
        let mut candidates = 0;
        for ring in 0..=rings {
            for y in -ring..=ring {
                for x in -ring..=ring {
                    if ring > 0 && x.abs() != ring && y.abs() != ring {
                        continue;
                    }
                    let keep = self.combat_index.visit_bin(
                        1 - a.team as usize,
                        [origin[0] + x, origin[1] + y],
                        |j| {
                            let b = &self.agents[j];
                            if !b.alive() || b.embarked_on.is_some() {
                                return true;
                            }
                            candidates += 1;
                            let d = distance_sq(a.position_mm, b.position_mm);
                            if d < best_distance && grid.line_of_fire(a.position_mm, b.position_mm)
                            {
                                best_distance = d;
                                best = Some(j);
                            }
                            candidates < 256
                        },
                    );
                    if !keep {
                        return best;
                    }
                }
            }
        }
        best
    }
    fn move_agent(
        &mut self,
        index: usize,
        grid: &NavGrid,
        events: &mut Vec<TacticalEvent>,
    ) -> Result<(), String> {
        if !self.yielded_idle.contains(&index) {
            self.agents[index].previous_position_mm = self.agents[index].position_mm;
        }
        let a = self.agents[index];
        let Some(goal) = a.goal_mm else {
            return Ok(());
        };
        // Autonomous soldiers stop to engage an in-range known target.
        if a.attack_move
            && let Some(target) = a.target.and_then(|id| self.agent(id))
        {
            let to_enemy = [
                target.position_mm[0] as i64 - a.position_mm[0] as i64,
                target.position_mm[1] as i64 - a.position_mm[1] as i64,
            ];
            let to_goal = [
                goal[0] as i64 - a.position_mm[0] as i64,
                goal[1] as i64 - a.position_mm[1] as i64,
            ];
            let approaches_enemy = to_enemy[0] * to_goal[0] + to_enemy[1] * to_goal[1] > 0;
            if target.alive()
                && a.ammunition > 0
                && approaches_enemy
                && distance_sq(a.position_mm, target.position_mm)
                    <= (a.role.range_mm() as i64).pow(2)
                && grid.line_of_fire(a.position_mm, target.position_mm)
            {
                return Ok(());
            }
        }
        let goal_index = grid
            .index_mm(goal)
            .ok_or("Destination outside navigation grid")?;
        let current = grid
            .index_mm(a.position_mm)
            .ok_or("Pawn outside navigation grid")?;
        let key = RouteKey {
            goal: goal_index,
            movement: a.role.movement(),
        };
        let Some(flow) = self.routes.get(&key) else {
            return Ok(());
        };
        let Some(next) = flow.next_cell(current, grid) else {
            return Ok(());
        };
        let formation_reach = 160_000i64
            + a.formation_offset_mm[0]
                .abs()
                .max(a.formation_offset_mm[1].abs()) as i64;
        let near = distance_sq(a.position_mm, goal) < formation_reach.pow(2);
        let mut local_goal = goal;
        if near && a.job.is_none() {
            local_goal = [
                (goal[0] as i64 + a.formation_offset_mm[0] as i64)
                    .clamp(0, grid.width as i64 * grid.cell_mm as i64 - 1) as i32,
                (goal[1] as i64 + a.formation_offset_mm[1] as i64)
                    .clamp(0, grid.height as i64 * grid.cell_mm as i64 - 1) as i32,
            ];
            if !grid
                .index_mm(local_goal)
                .is_some_and(|i| grid.passable(i, key.movement))
            {
                local_goal = goal;
            }
        }
        let formation_direct = near
            && local_goal != goal
            && segment_passable(grid, a.position_mm, local_goal, key.movement);
        let in_goal = current == goal_index || next == current || formation_direct;
        let target = if in_goal {
            if grid.passable(goal_index, key.movement) {
                local_goal
            } else {
                grid.center_mm(current)
            }
        } else {
            route_lane_target(grid, current, next, a.position_mm, a.role.radius_mm())
        };
        let delta = [target[0] - a.position_mm[0], target[1] - a.position_mm[1]];
        let distance = integer_sqrt(distance_sq(target, a.position_mm) as u64) as i64;
        let arrival = if a.role.movement() == MovementClass::Foot {
            1100
        } else {
            4500
        };
        if in_goal && distance <= arrival {
            if !a.arrived {
                events.push(TacticalEvent::Arrived {
                    agent: a.id,
                    job: a.job,
                });
            }
            if let Some(job) = a.job {
                if a.role == Role::Worker {
                    events.push(TacticalEvent::Work {
                        worker: a.id,
                        job,
                        units: 1,
                    });
                }
            }
            self.agents[index].arrived = true;
            return Ok(());
        }
        self.agents[index].arrived = false;
        if distance == 0 {
            return Ok(());
        }
        let c = grid.cells[current];
        let speed = movement_speed_mm(key.movement, c);
        let stride = speed.min(distance);
        let dx = delta[0] as i64 * stride / distance;
        let dy = delta[1] as i64 * stride / distance;
        let from = a.position_mm;
        let candidates = [
            [from[0] + dx as i32, from[1] + dy as i32],
            [from[0] + dy as i32, from[1] - dx as i32],
            [from[0] - dy as i32, from[1] + dx as i32],
        ];
        if a.role == Role::Worker && !self.can_move(index, candidates[0], current, grid) {
            self.yield_idle_friend(index, candidates[0], grid);
        }
        let chosen = candidates
            .into_iter()
            .find(|&p| self.can_move(index, p, current, grid));
        if let Some(p) = chosen {
            self.agents[index].position_mm = p;
        }
        Ok(())
    }
    /// Open a physical lane through a nearby idle formation. Outer bodies
    /// yield first, making room for the interior bodies on following ticks.
    /// Requests inspect at most 128 bodies in fixed spatial bins, move at most
    /// four idle allies and share a 64 body per tick ceiling. Active jobs and
    /// enemies never receive these movements; every step checks real collision.
    fn yield_idle_friend(&mut self, worker: usize, desired: [i32; 2], grid: &NavGrid) {
        if self.yielded_idle.len() >= 64 || self.clear_of_others(worker, desired) {
            return;
        }
        let a = self.agents[worker];
        let bin = [a.position_mm[0] / 4000, a.position_mm[1] / 4000];
        let direction = [
            desired[0] as i64 - a.position_mm[0] as i64,
            desired[1] as i64 - a.position_mm[1] as i64,
        ];
        let perpendicular = [-direction[1], direction[0]];
        let length = integer_sqrt(
            (perpendicular[0] * perpendicular[0] + perpendicular[1] * perpendicular[1]) as u64,
        )
        .max(1) as i64;
        let mut candidates = Vec::new();
        let mut examined = 0;
        'bins: for y in -4..=4 {
            for x in -4..=4 {
                self.crowd
                    .visit_bin(a.team as usize, [bin[0] + x, bin[1] + y], |j| {
                        examined += 1;
                        let b = self.agents[j];
                        if j != worker
                            && b.alive()
                            && b.embarked_on.is_none()
                            && b.role.movement() == a.role.movement()
                            && b.goal_mm.is_none()
                            && b.target.is_none()
                            && !self.yielded_idle.contains(&j)
                        {
                            let lateral = (b.position_mm[0] as i64 - a.position_mm[0] as i64)
                                * perpendicular[0]
                                + (b.position_mm[1] as i64 - a.position_mm[1] as i64)
                                    * perpendicular[1];
                            candidates.push((Reverse(lateral.abs()), b.id, j, lateral));
                        }
                        examined < 128
                    });
                if examined >= 128 {
                    break 'bins;
                }
            }
        }
        candidates.sort_unstable();
        let mut moved = 0;
        for (_, _, j, lateral) in candidates {
            if moved == 4 || self.yielded_idle.len() >= 64 {
                break;
            }
            let b = self.agents[j];
            let from = b.position_mm;
            let Some(current) = grid.index_mm(from) else {
                continue;
            };
            let speed = movement_speed_mm(b.role.movement(), grid.cells[current]).min(140);
            let sign = if lateral < 0 {
                -1
            } else if lateral > 0 {
                1
            } else if b.id % 2 == 0 {
                -1
            } else {
                1
            };
            let delta = [
                (perpendicular[0] * speed * sign / length) as i32,
                (perpendicular[1] * speed * sign / length) as i32,
            ];
            let position = [from[0] + delta[0], from[1] + delta[1]];
            if position != from && self.can_move(j, position, current, grid) {
                self.agents[j].previous_position_mm = from;
                self.agents[j].position_mm = position;
                self.yielded_idle.insert(j);
                moved += 1;
            }
        }
    }
    fn can_move(&self, index: usize, position: [i32; 2], current: usize, grid: &NavGrid) -> bool {
        let a = &self.agents[index];
        let Some(next) = grid.index_mm(position) else {
            return false;
        };
        if !grid.passable(next, a.role.movement())
            || (next != current && grid.edge(current, next, a.role.movement()).is_none())
        {
            return false;
        }
        self.clear_of_others(index, position)
    }
    fn clear_of_others(&self, index: usize, position: [i32; 2]) -> bool {
        let a = &self.agents[index];
        let bin = [position[0] / 4000, position[1] / 4000];
        let rings = 2;
        for team in 0..2 {
            for y in -rings..=rings {
                for x in -rings..=rings {
                    if !self.crowd.visit_bin(team, [bin[0] + x, bin[1] + y], |j| {
                        if j == index {
                            return true;
                        }
                        let b = &self.agents[j];
                        if !b.alive()
                            || b.embarked_on.is_some()
                            || b.role.movement() != a.role.movement()
                        {
                            return true;
                        }
                        let minimum = (a.role.radius_mm() + b.role.radius_mm()) as i64;
                        distance_sq(position, b.position_mm) >= minimum * minimum
                    }) {
                        return false;
                    }
                }
            }
        }
        true
    }
    fn fire(
        &mut self,
        index: usize,
        tick: u64,
        grid: &NavGrid,
        events: &mut Vec<TacticalEvent>,
    ) -> Result<(), String> {
        let a = &self.agents[index];
        if !a.alive()
            || a.embarked_on.is_some()
            || a.role.range_mm() == 0
            || tick < a.next_shot_tick
        {
            return Ok(());
        }
        if a.ammunition == 0 {
            if (tick + a.id) % 100 == 0 {
                events.push(TacticalEvent::NeedsSupply {
                    agent: a.id,
                    team: a.team,
                });
            }
            return Ok(());
        }
        let target = a.target.and_then(|id| self.idx(id).ok()).filter(|&i| {
            let b = &self.agents[i];
            b.alive()
                && b.embarked_on.is_none()
                && b.team != a.team
                && distance_sq(a.position_mm, b.position_mm) <= (a.role.range_mm() as i64).pow(2)
                && grid.line_of_fire(a.position_mm, b.position_mm)
        });
        if let Some(j) = target {
            let target_cell = grid
                .index_mm(self.agents[j].position_mm)
                .ok_or("Target outside map")?;
            let base = if a.role == Role::Boat { 70 } else { 22 };
            let damage = (base * (300 - grid.cells[target_cell].cover as u32) / 300).max(4) as u16;
            self.pending_damage[j] = self.pending_damage[j]
                .checked_add(damage as u32)
                .ok_or("Queued combat damage overflow")?;
            self.pending_killers[j] = Some(a.id);
            events.push(TacticalEvent::Shot {
                source: a.id,
                target: self.agents[j].id,
                damage,
            });
        } else {
            let target = self
                .structures
                .iter()
                .filter(|s| {
                    s.active
                        && s.team != a.team
                        && distance_sq(a.position_mm, s.position_mm)
                            <= (a.role.range_mm() as i64).pow(2)
                        && grid.line_of_fire(a.position_mm, s.position_mm)
                })
                .min_by_key(|s| (distance_sq(a.position_mm, s.position_mm), s.id));
            let Some(s) = target else {
                return Ok(());
            };
            events.push(TacticalEvent::StructureHit {
                source: a.id,
                structure: s.id,
                damage: if a.role == Role::Boat { 40 } else { 10 },
            });
        }
        self.totals.fired_ammunition = self
            .totals
            .fired_ammunition
            .checked_add(1)
            .ok_or("Shot ledger overflow")?;
        let a = &mut self.agents[index];
        a.ammunition -= 1;
        a.next_shot_tick = tick
            .checked_add(if a.role == Role::Boat { 40 } else { 20 })
            .ok_or("Shot deadline overflow")?;
        Ok(())
    }
    fn kill(
        &mut self,
        index: usize,
        killer: Option<AgentId>,
        events: &mut Vec<TacticalEvent>,
    ) -> Result<(), String> {
        if !self.agents[index].alive() {
            return Ok(());
        }
        let a = &self.agents[index];
        let id = a.id;
        let team = a.team;
        let deaths = self.totals.deaths[team as usize]
            .checked_add(1)
            .ok_or("Death ledger overflow")?;
        let lost = self
            .totals
            .lost_ammunition
            .checked_add(a.ammunition as u64)
            .ok_or("Lost ammo ledger overflow")?;
        self.agents[index].health = 0;
        self.agents[index].ammunition = 0;
        self.agents[index].target = None;
        self.agents[index].goal_mm = None;
        self.agents[index].job = None;
        self.totals.deaths[team as usize] = deaths;
        self.totals.lost_ammunition = lost;
        events.push(TacticalEvent::Died {
            agent: id,
            team,
            killer,
        });
        if let Some(passengers) = self.manifests.get_mut(&id).map(std::mem::take) {
            for passenger in passengers {
                let p = self.idx(passenger)?;
                self.agents[p].embarked_on = None;
                self.kill(p, killer, events)?;
            }
        }
        if let Some(vessel) = self.agents[index].embarked_on.take() {
            if let Some(manifest) = self.manifests.get_mut(&vessel) {
                manifest.retain(|p| *p != id);
            }
        }
        Ok(())
    }
}

mod route_serde {
    use super::*;
    use serde::{Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        routes: &BTreeMap<RouteKey, Flow>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        routes.values().collect::<Vec<_>>().serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<RouteKey, Flow>, D::Error> {
        let flows = Vec::<Flow>::deserialize(deserializer)?;
        let mut result = BTreeMap::new();
        for flow in flows {
            let key = flow.key;
            if result.insert(key, flow).is_some() {
                return Err(serde::de::Error::custom(
                    "Duplicate serialized shared route",
                ));
            }
        }
        Ok(result)
    }
}

fn default_attack_move() -> bool {
    true
}
fn default_revision() -> u64 {
    1
}
fn movement_speed_mm(movement: MovementClass, c: NavCell) -> i64 {
    match movement {
        MovementClass::Foot if c.road || c.bridge_deck_mm.is_some() => {
            400 * 100 / (100 + traction_band(c) as i64 * 2)
        }
        MovementClass::Foot => {
            (140 * (320 - c.cover as i64) / 320).max(25) * 100
                / (100 + traction_band(c) as i64 * 12)
        }
        MovementClass::Naval { .. } => 600,
    }
}
fn topology_signature(c: NavCell) -> (i64, bool, Option<i32>, bool, bool) {
    let s = nav_signature(c);
    (s.0, s.3, s.4, s.5, s.6)
}

fn bridge_dry(c: NavCell) -> bool {
    c.bridge_deck_mm
        .is_some_and(|deck| deck as i64 >= c.elevation_mm as i64 + c.water_depth_mm as i64 + 100)
}
/// A fine water-depth change always updates the query cache, but only a route
/// cost/passability change invalidates fields. The only movement classes used
/// by Tactical Role are Foot and the common350mm/1800mm vessel profile.
fn traction_band(c: NavCell) -> u8 {
    if c.bridge_deck_mm.is_some() {
        0
    } else {
        (c.soil_moisture_permille / 125) as u8
    }
}
fn nav_signature(c: NavCell) -> (i64, u8, bool, bool, Option<i32>, bool, bool, u8) {
    let foot = !c.blocked && (c.effective_depth_mm() < WALK_WATER_LIMIT_MM || bridge_dry(c));
    let vessel = !c.blocked
        && c.keel_depth_mm() >= 350
        && c.bridge_underside_mm.is_none_or(|underside| {
            underside as i64 - (c.elevation_mm as i64 + c.water_depth_mm as i64) >= 1800
        });
    (
        (c.elevation_mm as i64 + c.reef_height_mm as i64).div_euclid(100),
        if c.road || c.bridge_deck_mm.is_some() {
            0
        } else {
            c.cover / 8
        },
        c.road,
        c.blocked,
        c.bridge_deck_mm.map(|d| d.div_euclid(100)),
        foot,
        vessel,
        traction_band(c),
    )
}
/// Preserve each body's lateral position through the shared cell corridor.
/// Steering every agent to each cell center funnels an otherwise spaced group
/// into a permanent crowd knot. The lane stays inside the actual next cell;
/// can_move still verifies its live edge, passability and body clearance.
fn route_lane_target(
    grid: &NavGrid,
    current: usize,
    next: usize,
    position: [i32; 2],
    radius: i32,
) -> [i32; 2] {
    let origin = grid.center_mm(current);
    let center = grid.center_mm(next);
    let direction = [
        (center[0] - origin[0]).signum(),
        (center[1] - origin[1]).signum(),
    ];
    let perpendicular = [-direction[1], direction[0]];
    let divisor =
        (perpendicular[0] * perpendicular[0] + perpendicular[1] * perpendicular[1]).max(1);
    let lane = ((position[0] as i64 - origin[0] as i64) * perpendicular[0] as i64
        + (position[1] as i64 - origin[1] as i64) * perpendicular[1] as i64)
        / divisor as i64;
    let limit = (grid.cell_mm / 2 - radius - 100).max(0) as i64;
    [
        center[0] + (lane * perpendicular[0] as i64).clamp(-limit, limit) as i32,
        center[1] + (lane * perpendicular[1] as i64).clamp(-limit, limit) as i32,
    ]
}
fn segment_passable(grid: &NavGrid, from: [i32; 2], to: [i32; 2], movement: MovementClass) -> bool {
    let Some(mut previous) = grid.index_mm(from) else {
        return false;
    };
    let dx = to[0] as i64 - from[0] as i64;
    let dy = to[1] as i64 - from[1] as i64;
    let steps = (dx.abs().max(dy.abs()) / (grid.cell_mm as i64 / 2).max(1) + 1).min(4096);
    for step in 1..=steps {
        let point = [
            (from[0] as i64 + dx * step / steps) as i32,
            (from[1] as i64 + dy * step / steps) as i32,
        ];
        let Some(next) = grid.index_mm(point) else {
            return false;
        };
        if !grid.passable(next, movement)
            || (next != previous && grid.edge(previous, next, movement).is_none())
        {
            return false;
        }
        previous = next;
    }
    true
}

fn to_mm(position: [f32; 2]) -> Result<[i32; 2], String> {
    if position
        .iter()
        .any(|v| !v.is_finite() || *v < 0. || *v > 2_000_000.)
    {
        return Err("Position must be finite, nonnegative and representable in millimeters".into());
    }
    Ok([
        (position[0] as f64 * 1000.).round() as i32,
        (position[1] as f64 * 1000.).round() as i32,
    ])
}
fn distance_sq(a: [i32; 2], b: [i32; 2]) -> i64 {
    let dx = a[0] as i64 - b[0] as i64;
    let dy = a[1] as i64 - b[1] as i64;
    dx * dx + dy * dy
}
fn integer_sqrt(n: u64) -> u64 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + n / x) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    fn grid(width: usize, height: usize) -> NavGrid {
        NavGrid::new(
            width,
            height,
            16_000,
            vec![
                NavCell {
                    road: true,
                    ..Default::default()
                };
                width * height
            ],
        )
        .unwrap()
    }
    #[test]
    fn foot_paths_do_not_treat_roads_as_bridges_and_naval_checks_clearance() {
        let mut g = grid(3, 1);
        let mut c = g.cells[1];
        c.water_depth_mm = 1000;
        g.set_cell(1, c).unwrap();
        assert!(!g.passable(1, MovementClass::Foot));
        assert!(g.passable(1, Role::Boat.movement()));
        c.minimum_water_depth_mm = Some(0);
        g.set_cell(1, c).unwrap();
        assert!(
            !g.passable(1, Role::Boat.movement()),
            "One wet corner cannot admit a vessel"
        );
        c.minimum_water_depth_mm = None;
        c.bridge_deck_mm = Some(4000);
        c.bridge_underside_mm = Some(3500);
        g.set_cell(1, c).unwrap();
        assert!(g.passable(1, MovementClass::Foot));
        assert!(g.passable(1, Role::Boat.movement()));
        c.bridge_underside_mm = Some(2000);
        g.set_cell(1, c).unwrap();
        assert!(!g.passable(1, Role::Boat.movement()));
    }
    #[test]
    fn shared_route_has_one_build_and_flood_invalidates_it() {
        let mut g = grid(10, 3);
        let mut t = Tactics::new(1);
        for y in 0..8 {
            let id = t.spawn(0, Role::Worker, [4., 4. + y as f32 * 1.5]).unwrap();
            t.assign(id, [150., 8.], Some(9)).unwrap();
        }
        for tick in 1..=20 {
            t.step(tick, &g).unwrap();
        }
        assert_eq!(t.totals.route_builds, 1);
        assert!(t.totals.route_cache_hits > 1);
        let mut c = g.cells[5];
        c.water_depth_mm = 2000;
        g.set_cell(5, c).unwrap();
        t.step(21, &g).unwrap();
        assert_eq!(t.totals.route_invalidations, 1);
        assert_eq!(t.totals.route_builds, 2);
        t.validate().unwrap();
    }
    #[test]
    fn worker_labor_requires_real_arrival_and_duplicate_ticks_fail() {
        let g = grid(8, 1);
        let mut t = Tactics::new(2);
        let id = t.spawn(0, Role::Worker, [8., 8.]).unwrap();
        t.assign(id, [100., 8.], Some(17)).unwrap();
        let mut worked = false;
        for tick in 1..=400 {
            let e = t.step(tick, &g).unwrap();
            if tick < 200 {
                assert!(!e.iter().any(|e| matches!(e, TacticalEvent::Work { .. })));
            }
            worked |= e
                .iter()
                .any(|e| matches!(e, TacticalEvent::Work { job: 17, .. }));
        }
        assert!(worked);
        assert!(t.step(400, &g).is_err());
        t.validate().unwrap();
    }
    #[test]
    fn crowd_does_not_compress_two_walkers_into_same_goal() {
        let g = grid(6, 3);
        let mut t = Tactics::new(3);
        let a = t.spawn(0, Role::Worker, [8., 20.]).unwrap();
        let b = t.spawn(0, Role::Worker, [8., 26.]).unwrap();
        t.assign(a, [80., 24.], Some(1)).unwrap();
        t.assign(b, [80., 24.], Some(1)).unwrap();
        for tick in 1..=500 {
            t.step(tick, &g).unwrap();
            assert!(
                distance_sq(
                    t.agent(a).unwrap().position_mm,
                    t.agent(b).unwrap().position_mm
                ) >= 4400i64.pow(2)
            );
        }
    }
    #[test]
    fn combat_commits_real_casualties_and_conserves_ammunition() {
        let g = grid(8, 3);
        let mut t = Tactics::new(4);
        let a = t.spawn(0, Role::Soldier, [30., 24.]).unwrap();
        let b = t.spawn(1, Role::Soldier, [65., 24.]).unwrap();
        let mut shots = 0;
        for tick in 1..=300 {
            shots += t
                .step(tick, &g)
                .unwrap()
                .iter()
                .filter(|e| matches!(e, TacticalEvent::Shot { .. }))
                .count();
        }
        assert!(shots > 0);
        assert!(!t.agent(a).unwrap().alive() || !t.agent(b).unwrap().alive());
        assert!(t.totals.deaths.iter().sum::<u64>() > 0);
        t.validate().unwrap();
    }
    #[test]
    fn boarding_and_release_keep_every_identity_owned_once_through_save() {
        let mut g = grid(5, 2);
        for i in 0..5 {
            let mut c = g.cells[i];
            c.water_depth_mm = 1000;
            g.set_cell(i, c).unwrap();
        }
        let mut t = Tactics::new(5);
        let vessel = t.spawn(0, Role::Transport, [24., 8.]).unwrap();
        let passenger = t.spawn(0, Role::Soldier, [24., 19.]).unwrap();
        t.board(passenger, vessel).unwrap();
        assert!(t.board(passenger, vessel).is_err());
        assert_eq!(t.alive_count(0), 2);
        assert_eq!(t.views().len(), 1);
        t.validate().unwrap();
        let raw = serde_json::to_vec(&t).unwrap();
        let mut restored: Tactics = serde_json::from_slice(&raw).unwrap();
        restored.validate().unwrap();
        restored.disembark(passenger, [24., 24.], &g).unwrap();
        assert_eq!(restored.views().len(), 2);
        assert_eq!(restored.alive_count(0), 2);
        restored.validate().unwrap();
    }
    #[test]
    fn discrete_route_budget_is_bounded_and_dead_pawns_do_not_work() {
        let g = grid(100, 100);
        let mut t = Tactics::new(6);
        let worker = t.spawn(0, Role::Worker, [8., 8.]).unwrap();
        t.assign(worker, [1500., 1500.], Some(1)).unwrap();
        t.step(1, &g).unwrap();
        assert!(t.totals.route_node_expansions <= ROUTE_EXPANSIONS_PER_TICK as u64);
        let mut events = Vec::new();
        t.kill(0, None, &mut events).unwrap();
        for tick in 2..=3 {
            assert!(
                !t.step(tick, &g)
                    .unwrap()
                    .iter()
                    .any(|e| matches!(e, TacticalEvent::Work { .. }))
            );
        }
        t.validate().unwrap();
    }
    #[test]
    fn harmless_water_changes_do_not_starve_a_partially_built_route() {
        let mut g = grid(100, 100);
        let mut t = Tactics::new(8);
        let worker = t.spawn(0, Role::Worker, [8., 8.]).unwrap();
        t.assign(worker, [1500., 1500.], Some(4)).unwrap();
        for tick in 1..=80 {
            let mut c = g.cells[50 * 100 + 50];
            c.water_depth_mm = 10 + (tick % 20) as u32;
            g.set_cell(50 * 100 + 50, c).unwrap();
            t.step(tick, &g).unwrap();
        }
        assert_eq!(t.totals.route_builds, 1);
        assert_eq!(t.totals.route_invalidations, 0);
        assert_ne!(t.agent(worker).unwrap().position_mm, [8000, 8000]);
    }
    #[test]
    fn partial_navigation_jobs_survive_save_without_changing_next_ticks() {
        let mut g = grid(120, 120);
        let mut original = Tactics::new(9);
        let worker = original.spawn(0, Role::Worker, [8., 8.]).unwrap();
        original.assign(worker, [1800., 1800.], Some(8)).unwrap();
        original.step(1, &g).unwrap();
        // A remote construction transaction can occur after tactics but before
        // the save. Its bounded dirty history must survive with the frontier.
        let mut remote = g.cells[2];
        remote.blocked = true;
        g.set_cell(2, remote).unwrap();
        let raw = serde_json::to_vec(&original).unwrap();
        let mut restored: Tactics = serde_json::from_slice(&raw).unwrap();
        let grid_raw = serde_json::to_vec(&g).unwrap();
        let mut restored_grid: NavGrid = serde_json::from_slice(&grid_raw).unwrap();
        for tick in 2..=100 {
            if tick == 30 {
                let mut c = g.cells[40];
                c.blocked = true;
                g.set_cell(40, c).unwrap();
                restored_grid.set_cell(40, c).unwrap();
            }
            assert_eq!(
                original.step(tick, &g).unwrap(),
                restored.step(tick, &restored_grid).unwrap()
            );
            assert_eq!(
                serde_json::to_value(&original).unwrap(),
                serde_json::to_value(&restored).unwrap()
            );
        }
        original.validate().unwrap();
        restored.validate().unwrap();
    }
    #[test]
    fn exact_formation_spawn_uses_only_free_passable_sites() {
        let mut g = grid(10, 10);
        let mut t = Tactics::new(10);
        let mut c = g.cells[55];
        c.water_depth_mm = 1000;
        g.set_cell(55, c).unwrap();
        let ids = t
            .spawn_formation(0, Role::Soldier, [80., 80.], 100, &g)
            .unwrap();
        assert_eq!(ids.len(), 100);
        for (i, a) in t.agents.iter().enumerate() {
            assert!(g.passable(g.index_mm(a.position_mm).unwrap(), MovementClass::Foot));
            for b in &t.agents[i + 1..] {
                assert!(distance_sq(a.position_mm, b.position_mm) >= 1200i64.pow(2));
            }
        }
        let before = t.agents.len();
        assert!(
            t.spawn_formation(1, Role::Boat, [80., 80.], 100, &g)
                .is_err()
        );
        assert_eq!(t.agents.len(), before);
        t.validate().unwrap();
    }
    #[test]
    fn flood_evacuation_and_drowning_are_real_saved_finite_state() {
        let mut g = grid(3, 3);
        let mut t = Tactics::new(12);
        let pawn = t.spawn(0, Role::Soldier, [24., 24.]).unwrap();
        let mut c = g.cells[4];
        c.water_depth_mm = 1000;
        g.set_cell(4, c).unwrap();
        for tick in 1..=150 {
            t.step(tick, &g).unwrap();
        }
        assert!(g.passable(
            g.index_mm(t.agent(pawn).unwrap().position_mm).unwrap(),
            MovementClass::Foot
        ));
        assert_eq!(t.agent(pawn).unwrap().health, 100);
        assert_eq!(t.agent(pawn).unwrap().water_exposure_ticks, 0);
        t.validate().unwrap();
        for i in 0..g.cells.len() {
            let mut c = g.cells[i];
            c.water_depth_mm = 2000;
            g.set_cell(i, c).unwrap();
        }
        for tick in 151..=400 {
            t.step(tick, &g).unwrap();
        }
        assert!(t.agent(pawn).unwrap().water_exposure_ticks > 200);
        assert!(t.agent(pawn).unwrap().health < 100);
        let raw = serde_json::to_vec(&t).unwrap();
        let mut restored: Tactics = serde_json::from_slice(&raw).unwrap();
        for tick in 401..=1200 {
            assert_eq!(t.step(tick, &g).unwrap(), restored.step(tick, &g).unwrap());
        }
        assert!(!t.agent(pawn).unwrap().alive());
        assert_eq!(t.totals.deaths, [1, 0]);
        assert_eq!(t.totals.lost_ammunition, 40);
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
        t.validate().unwrap();
    }
    #[test]
    fn sinking_transport_and_repeat_damage_do_not_duplicate_passenger_deaths() {
        let g = grid(3, 3);
        let mut t = Tactics::new(13);
        let vessel = t.spawn(0, Role::Transport, [24., 20.]).unwrap();
        let passenger = t.spawn(0, Role::Soldier, [24., 28.]).unwrap();
        t.board(passenger, vessel).unwrap();
        let first = t.damage(vessel, 1000, None).unwrap();
        assert_eq!(
            first
                .iter()
                .filter(|e| matches!(e, TacticalEvent::Died { .. }))
                .count(),
            2
        );
        assert!(t.damage(vessel, 1000, None).unwrap().is_empty());
        assert_eq!(t.alive_count(0), 0);
        assert_eq!(t.totals.deaths, [2, 0]);
        t.validate().unwrap();
        let _ = g;
    }
    #[test]
    fn more_distinct_jobs_than_cache_slots_are_eventually_serviced() {
        let g = grid(12, 12);
        let mut t = Tactics::new(14);
        for i in 0..40 {
            let id = t
                .spawn(
                    0,
                    Role::Worker,
                    [8. + (i % 6) as f32 * 2., 8. + (i / 6) as f32 * 2.],
                )
                .unwrap();
            let cell = 80 + i;
            t.assign(
                id,
                [
                    g.center_mm(cell)[0] as f32 * 0.001,
                    g.center_mm(cell)[1] as f32 * 0.001,
                ],
                Some(i as u64),
            )
            .unwrap();
        }
        let mut ready = BTreeSet::new();
        for tick in 1..=100 {
            t.step(tick, &g).unwrap();
            for (key, flow) in &t.routes {
                if flow.complete {
                    ready.insert(key.goal);
                }
            }
        }
        assert_eq!(ready.len(), 40);
        assert!(t.routes.len() <= MAX_ROUTES);
        t.validate().unwrap();
    }
    #[test]
    fn ten_thousand_military_plus_two_thousand_workers_keep_exact_counts() {
        let g = grid(300, 200);
        let mut t = Tactics::new(15);
        t.spawn_formation(0, Role::Soldier, [500., 1600.], 5000, &g)
            .unwrap();
        t.spawn_formation(1, Role::Soldier, [4300., 1600.], 5000, &g)
            .unwrap();
        t.spawn_formation(0, Role::Worker, [800., 1600.], 1000, &g)
            .unwrap();
        t.spawn_formation(1, Role::Worker, [4000., 1600.], 1000, &g)
            .unwrap();
        for tick in 1..=20 {
            t.step(tick, &g).unwrap();
        }
        assert_eq!(
            t.agents
                .iter()
                .filter(|a| a.alive() && a.role == Role::Soldier)
                .count(),
            10_000
        );
        assert_eq!(
            t.agents
                .iter()
                .filter(|a| a.alive() && a.role == Role::Worker)
                .count(),
            2000
        );
        assert_eq!(t.totals.spawned, [6000, 6000]);
        assert_eq!(t.totals.initial_ammunition, 400_000);
        t.validate().unwrap();
        // Hashed scratch scales with records rather than world-meter area.
        assert!(t.crowd.heads[0].len() <= t.agents.len() * 4);
    }
    #[test]
    fn bridge_destruction_removes_ghost_crossing_without_unchanged_overlay_churn() {
        let mut g = grid(3, 1);
        let mut c = g.cells[1];
        c.water_depth_mm = 1000;
        g.set_cell(1, c).unwrap();
        assert_eq!(g.replace_bridges(&[(1, 4000, 3500)]).unwrap(), 1);
        assert!(g.passable(1, MovementClass::Foot));
        let revision = g.revision;
        assert_eq!(g.replace_bridges(&[(1, 4000, 3500)]).unwrap(), 0);
        assert_eq!(g.revision, revision);
        assert_eq!(g.replace_bridges(&[]).unwrap(), 1);
        assert!(!g.passable(1, MovementClass::Foot));
        assert_eq!(g.revision, revision + 1);
    }
    #[test]
    fn reef_overlay_changes_clearance_once_without_depth_restore_churn() {
        let mut g = grid(3, 1);
        let mut c = g.cells[1];
        c.water_depth_mm = 1000;
        g.set_cell(1, c).unwrap();
        assert!(g.passable(1, Role::Boat.movement()));
        assert_eq!(g.replace_reefs(&[(1, 800)]).unwrap(), 1);
        assert!(!g.passable(1, Role::Boat.movement()));
        assert!(g.passable(1, MovementClass::Foot));
        assert_eq!(g.cells[1].water_depth_mm, 1000);
        let revision = g.revision;
        assert_eq!(g.replace_reefs(&[(1, 800)]).unwrap(), 0);
        assert_eq!(g.revision, revision);
        let mut current = g.cells[1];
        current.water_depth_mm = 1001;
        g.set_cell(1, current).unwrap();
        assert_eq!(g.revision, revision);
        assert_eq!(g.replace_reefs(&[(1, 800)]).unwrap(), 0);
        assert_eq!(g.revision, revision);
        g.replace_reefs(&[]).unwrap();
        assert!(g.passable(1, Role::Boat.movement()));
        assert!(!g.passable(1, MovementClass::Foot));
    }

    // Keep a dense reference independent of the sparse index cache. Comparing
    // serialized grids includes route cost/topology revisions and dirty history,
    // not just the visible overlay values.
    fn dense_replace_overlays(
        grid: &mut NavGrid,
        bridges: Option<&[(usize, i32, i32)]>,
        reefs: Option<&[(usize, u32)]>,
    ) -> usize {
        let desired_bridges: BTreeMap<_, _> = bridges
            .unwrap_or_default()
            .iter()
            .map(|&(index, deck, underside)| (index, (deck, underside)))
            .collect();
        let mut desired_reefs = BTreeMap::new();
        for &(index, height) in reefs.unwrap_or_default() {
            let entry = desired_reefs.entry(index).or_insert(0u32);
            *entry = (*entry).max(height);
        }
        let changes: Vec<_> = grid
            .cells
            .iter()
            .enumerate()
            .filter_map(|(index, old)| {
                let mut next = *old;
                if bridges.is_some() {
                    let span = desired_bridges.get(&index).copied();
                    next.bridge_deck_mm = span.map(|(deck, _)| deck);
                    next.bridge_underside_mm = span.map(|(_, underside)| underside);
                }
                if reefs.is_some() {
                    next.reef_height_mm = desired_reefs.get(&index).copied().unwrap_or(0);
                }
                (*old != next).then_some((index, next))
            })
            .collect();
        if changes.is_empty() {
            return 0;
        }
        grid.ensure_connectivity_masks();
        let count = changes.len();
        let mut dirty = Vec::new();
        let mut topology_candidates = Vec::new();
        for (index, next) in changes {
            if nav_signature(grid.cells[index]) != nav_signature(next) {
                dirty.push(index);
                if topology_signature(grid.cells[index]) != topology_signature(next) {
                    topology_candidates.push(index);
                }
            }
            grid.cells[index] = next;
        }
        grid.commit_dirty(dirty, topology_candidates).unwrap();
        count
    }

    fn assert_same_navigation(actual: &NavGrid, expected: &NavGrid) {
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }

    #[test]
    fn sparse_overlay_replacement_matches_dense_changes_overlap_and_removal() {
        let cells = (0..31 * 11)
            .map(|i| NavCell {
                elevation_mm: (i % 5) as i32 * 100,
                water_depth_mm: (i % 7) as u32 * 300,
                minimum_water_depth_mm: Some((i % 7) as u32 * 200),
                cover: (i % 255) as u8,
                soil_moisture_permille: (i % 1001) as u16,
                road: i % 2 == 0,
                blocked: i % 17 == 0,
                // Constructor-supplied overlays must also be removed by a
                // later complete replacement, including an underside alone.
                bridge_underside_mm: (i == 0).then_some(500),
                reef_height_mm: if i == 0 { 20 } else { 0 },
                ..Default::default()
            })
            .collect();
        let mut actual = NavGrid::new(31, 11, NAV_CELL_MM, cells).unwrap();
        let mut expected = actual.clone();
        let replacements: &[(&[(usize, i32, i32)], &[(usize, u32)])] = &[
            (
                &[(32, 4000, 3500), (300, 2000, 1400), (32, 4000, 3500)],
                &[(32, 800), (300, 300), (32, 200), (19, 0)],
            ),
            (
                &[(300, 2000, 1400), (32, 4000, 3500)],
                &[(19, 0), (300, 300), (32, 200), (32, 800)],
            ),
            (
                &[(32, 3800, 3100), (101, 5000, 4500)],
                &[(101, 900), (32, 0), (101, 700)],
            ),
            (
                &[(101, 5000, 4500), (32, 3800, 3100), (101, 5000, 4500)],
                &[(101, 700), (101, 900), (32, 0)],
            ),
            (&[], &[]),
            (&[], &[(4, 0)]),
            (&[(250, 1000, 500)], &[(250, 2000)]),
        ];
        for &(bridges, reefs) in replacements {
            assert_eq!(
                actual.replace_bridges(bridges).unwrap(),
                dense_replace_overlays(&mut expected, Some(bridges), None)
            );
            assert_same_navigation(&actual, &expected);
            assert_eq!(
                actual.replace_reefs(reefs).unwrap(),
                dense_replace_overlays(&mut expected, None, Some(reefs))
            );
            assert_same_navigation(&actual, &expected);
        }
        let before = serde_json::to_value(&actual).unwrap();
        assert!(
            actual
                .replace_bridges(&[(2, 2000, 1500), (2, 3000, 1500)])
                .is_err()
        );
        assert!(
            actual
                .replace_bridges(&[(actual.cells.len(), 2000, 1500)])
                .is_err()
        );
        assert!(actual.replace_bridges(&[(2, 1000, 1500)]).is_err());
        assert!(actual.replace_reefs(&[(2, 1_000_001)]).is_err());
        assert_eq!(serde_json::to_value(&actual).unwrap(), before);
    }

    #[test]
    fn sparse_overlay_indices_follow_cell_edits_and_rebuild_after_load() {
        let mut actual = grid(8, 4);
        let mut cell = actual.cells[10];
        cell.bridge_deck_mm = Some(2000);
        cell.bridge_underside_mm = Some(1500);
        cell.reef_height_mm = 300;
        actual.set_cell(10, cell).unwrap();
        let mut expected = actual.clone();
        assert_eq!(
            actual.replace_bridges(&[]).unwrap(),
            dense_replace_overlays(&mut expected, Some(&[]), None)
        );
        assert_same_navigation(&actual, &expected);
        assert_eq!(actual.cells[10].reef_height_mm, 300);
        actual.replace_bridges(&[(10, 3000, 2500)]).unwrap();
        let serialized = serde_json::to_vec(&actual).unwrap();
        assert!(
            serde_json::from_slice::<serde_json::Value>(&serialized)
                .unwrap()
                .get("overlay_indices")
                .is_none()
        );
        actual = serde_json::from_slice(&serialized).unwrap();
        assert!(actual.overlay_indices.is_none());
        let mut rebuilt = actual.clone();
        rebuilt.ensure_overlay_indices();
        assert_eq!(serde_json::to_vec(&rebuilt).unwrap(), serialized);
        // Edits made before the lazy cache rebuild must also be discovered.
        let mut new_cell = actual.cells[12];
        new_cell.bridge_underside_mm = Some(400);
        new_cell.reef_height_mm = 800;
        actual.set_cell(12, new_cell).unwrap();
        let mut expected = actual.clone();
        assert_eq!(
            actual.replace_reefs(&[(10, 100)]).unwrap(),
            dense_replace_overlays(&mut expected, None, Some(&[(10, 100)]))
        );
        assert_same_navigation(&actual, &expected);
        assert_eq!(
            actual.replace_bridges(&[]).unwrap(),
            dense_replace_overlays(&mut expected, Some(&[]), None)
        );
        assert_same_navigation(&actual, &expected);
        assert!(actual.overlay_indices.as_ref().unwrap().bridges.is_empty());
        assert_eq!(
            actual.overlay_indices.as_ref().unwrap().reefs,
            BTreeSet::from([10])
        );
    }

    #[test]
    fn ordinary_flood_water_below_breathing_height_does_not_drown_stationary_troops() {
        let mut g = grid(1, 1);
        let mut c = g.cells[0];
        c.water_depth_mm = 500;
        g.set_cell(0, c).unwrap();
        let mut t = Tactics::new(16);
        let id = t.spawn(0, Role::Soldier, [8., 8.]).unwrap();
        for tick in 1..=1000 {
            t.step(tick, &g).unwrap();
        }
        assert_eq!(t.agent(id).unwrap().health, 100);
        assert_eq!(t.agent(id).unwrap().water_exposure_ticks, 0);
        t.validate().unwrap();
    }
    #[test]
    fn retreat_and_new_solid_obstruction_do_not_leave_soldiers_frozen_on_old_targets() {
        let mut g = grid(8, 3);
        let mut t = Tactics::new(17);
        let id = t.spawn(0, Role::Soldier, [60., 24.]).unwrap();
        let enemy = t.spawn(1, Role::Soldier, [90., 24.]).unwrap();
        for tick in 1..=20 {
            t.step(tick, &g).unwrap();
        }
        assert_eq!(t.agent(id).unwrap().target, Some(enemy));
        t.assign(id, [8., 24.], None).unwrap();
        let before = t.agent(id).unwrap().position_mm;
        t.step(21, &g).unwrap();
        assert!(t.agent(id).unwrap().position_mm[0] < before[0]);
        t.assign(id, [120., 24.], None).unwrap();
        let mut wall = g.cells[1 * 8 + 4];
        wall.blocked = true;
        g.set_cell(1 * 8 + 4, wall).unwrap();
        let before = t.agent(id).unwrap().position_mm;
        t.step(22, &g).unwrap();
        assert_ne!(t.agent(id).unwrap().position_mm, before);
        t.validate().unwrap();
    }
    #[test]
    fn grid_orders_resolve_reachable_banks_without_teleporting_or_faking_work_reach() {
        let mut g = grid(8, 3);
        for y in 0..3 {
            let i = y * 8 + 3;
            let mut c = g.cells[i];
            c.water_depth_mm = 1000;
            g.set_cell(i, c).unwrap();
        }
        let mut t = Tactics::new(21);
        let id = t.spawn(0, Role::Worker, [8., 24.]).unwrap();
        let before = t.agent(id).unwrap().position_mm;
        assert!(
            t.assign_on_grid_within(id, [56., 24.], Some(1), &g, 3.)
                .is_err()
        );
        assert_eq!(t.agent(id).unwrap().goal_mm, None);
        let resolved = t.assign_on_grid(id, [56., 24.], Some(1), &g).unwrap();
        assert!(resolved[0] < 48.);
        assert_eq!(t.agent(id).unwrap().position_mm, before);
        let otherbank = t.assign_on_grid(id, [104., 24.], None, &g).unwrap();
        assert!(otherbank[0] < 48.);
        for tick in 1..=150 {
            t.step(tick, &g).unwrap();
            assert!(t.agent(id).unwrap().position_mm[0] < 48_000);
        }
        t.validate().unwrap();
    }
    #[test]
    fn cost_only_updates_preserve_cached_topology_and_idempotent_assignments() {
        let mut g = grid(6, 3);
        let mut t = Tactics::new(22);
        let id = t.spawn(0, Role::Worker, [8., 24.]).unwrap();
        t.assign_on_grid(id, [80., 24.], Some(2), &g).unwrap();
        let topology = g.topology_revision;
        let mut cell = g.cells[1];
        cell.cover = 200;
        cell.road = false;
        g.set_cell(1, cell).unwrap();
        assert_eq!(g.topology_revision, topology);
        let original = t.agent(id).unwrap().goal_mm;
        t.agents[0].arrived = true;
        t.assign_on_grid(id, [80., 24.], Some(2), &g).unwrap();
        assert!(t.agent(id).unwrap().arrived);
        assert_eq!(t.agent(id).unwrap().goal_mm, original);
        let raw = serde_json::to_vec(&t).unwrap();
        let mut restored: Tactics = serde_json::from_slice(&raw).unwrap();
        restored
            .assign_on_grid(id, [80., 24.], Some(2), &g)
            .unwrap();
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
    }
    #[test]
    fn wet_soil_reduces_offroad_traction_while_roads_limit_the_mud_cost() {
        let mut dry = grid(20, 1);
        for c in &mut dry.cells {
            c.road = false;
        }
        let mut wet = dry.clone();
        for i in 0..wet.cells.len() {
            let mut c = wet.cells[i];
            c.soil_moisture_permille = 875;
            wet.set_cell(i, c).unwrap();
        }
        let mut road = wet.clone();
        for i in 0..road.cells.len() {
            let mut c = road.cells[i];
            c.road = true;
            road.set_cell(i, c).unwrap();
        }
        let mut walkers = Vec::new();
        for terrain in [&dry, &wet, &road] {
            let mut t = Tactics::new(33);
            let id = t.spawn(0, Role::Worker, [8., 8.]).unwrap();
            t.assign(id, [300., 8.], None).unwrap();
            for tick in 1..=100 {
                t.step(tick, terrain).unwrap();
            }
            walkers.push(t.agent(id).unwrap().position_mm[0]);
            t.validate().unwrap();
        }
        assert!(walkers[1] < walkers[0]);
        assert!(walkers[2] > walkers[0]);
    }
    #[test]
    fn soil_moisture_updates_only_revise_route_cost_when_a_traction_band_changes() {
        let mut g = grid(3, 1);
        let mut c = g.cells[1];
        c.road = false;
        g.set_cell(1, c).unwrap();
        let topology = g.topology_revision;
        let revision = g.revision;
        c.soil_moisture_permille = 124;
        g.set_cell(1, c).unwrap();
        assert_eq!(g.cells[1].soil_moisture_permille, 124);
        assert_eq!(g.revision, revision);
        c.soil_moisture_permille = 125;
        g.set_cell(1, c).unwrap();
        assert_eq!(g.revision, revision + 1);
        assert_eq!(g.topology_revision, topology);
        c.soil_moisture_permille = 126;
        g.set_cell(1, c).unwrap();
        assert_eq!(g.revision, revision + 1);
    }
    #[test]
    fn changing_costs_do_not_starve_search_or_interrupt_live_cost_refresh() {
        let mut g = grid(160, 160);
        for c in &mut g.cells {
            c.road = false;
        }
        let mut tactics = Tactics::new(41);
        let id = tactics.spawn(0, Role::Worker, [8., 8.]).unwrap();
        tactics.assign(id, [2488., 2488.], Some(1)).unwrap();
        let mut saw_refresh_in_flight = false;
        let mut moved_during_refresh = false;
        for tick in 1..=260 {
            // A living world changes costs on every tick, including enough
            // separate transactions to exhaust the bounded dirty history.
            for i in 0..70 {
                let index = 80 * g.width + i;
                let mut cell = g.cells[index];
                cell.cover = if tick % 2 == 0 { 200 } else { 0 };
                cell.soil_moisture_permille = if tick % 2 == 0 { 875 } else { 0 };
                g.set_cell(index, cell).unwrap();
            }
            let before = tactics.agent(id).unwrap().position_mm;
            let expansions = tactics.totals.route_node_expansions;
            let refreshing = !tactics.cost_refreshes.is_empty();
            tactics.step(tick, &g).unwrap();
            assert!(
                tactics.totals.route_node_expansions - expansions
                    <= ROUTE_EXPANSIONS_PER_TICK as u64
            );
            saw_refresh_in_flight |= refreshing || !tactics.cost_refreshes.is_empty();
            moved_during_refresh |= refreshing && tactics.agent(id).unwrap().position_mm != before;
        }
        assert_eq!(tactics.totals.route_builds, 1);
        assert_eq!(tactics.totals.route_invalidations, 0);
        assert!(tactics.totals.route_cost_refreshes > 0);
        assert!(saw_refresh_in_flight && moved_during_refresh);
        assert_ne!(tactics.agent(id).unwrap().position_mm, [8000, 8000]);
        tactics.validate().unwrap();
    }

    #[test]
    fn topology_masks_distinguish_cost_changes_and_foot_from_naval_clearance() {
        let mut g = grid(4, 2);
        for i in 0..g.cells.len() {
            let mut cell = g.cells[i];
            cell.water_depth_mm = 299;
            g.set_cell(i, cell).unwrap();
        }
        let foot_key = RouteKey {
            goal: 3,
            movement: MovementClass::Foot,
        };
        let mut foot = Flow::new(foot_key, &g, 1);
        foot.required.insert(0);
        foot.expand(&g, 100);
        let topology = g.topology_revision;
        let mut cell = g.cells[1];
        cell.elevation_mm = 100;
        g.set_cell(1, cell).unwrap();
        assert_eq!(
            g.topology_revision, topology,
            "A small slope cost change does not alter reachability"
        );
        assert!(!foot.stale(&g));
        // A bridge keeps Foot access while a change in overhead clearance
        // closes the vessel route. Only the Naval connectivity bits change.
        cell.bridge_deck_mm = Some(5000);
        cell.bridge_underside_mm = Some(4000);
        cell.water_depth_mm = 1000;
        g.set_cell(1, cell).unwrap();
        let mut foot = Flow::new(foot_key, &g, 2);
        foot.required.insert(0);
        foot.expand(&g, 100);
        cell.bridge_underside_mm = Some(1500);
        g.set_cell(1, cell).unwrap();
        assert!(!foot.stale(&g));
        assert!(
            g.changes
                .back()
                .unwrap()
                .topology_changes
                .iter()
                .any(|&(_, mask)| mask & NAVAL_CONNECTIVITY != 0)
        );
        assert!(
            g.changes
                .back()
                .unwrap()
                .topology_changes
                .iter()
                .all(|&(_, mask)| mask & FOOT_CONNECTIVITY == 0)
        );
        cell.elevation_mm = 20_000;
        cell.water_depth_mm = 0;
        cell.bridge_deck_mm = None;
        cell.bridge_underside_mm = None;
        g.set_cell(2, cell).unwrap();
        assert!(
            foot.stale(&g),
            "A real slope barrier invalidates the affected Foot field"
        );
    }

    #[test]
    fn cost_refresh_frontier_and_deadline_survive_save_exactly() {
        let mut g = grid(160, 160);
        let mut original = Tactics::new(42);
        let id = original.spawn(0, Role::Worker, [8., 8.]).unwrap();
        original.assign(id, [2488., 2488.], Some(2)).unwrap();
        let mut checkpoint = None;
        for tick in 1..=230 {
            let mut cell = g.cells[120 * 160 + 120];
            cell.road = tick % 2 == 0;
            g.set_cell(120 * 160 + 120, cell).unwrap();
            original.step(tick, &g).unwrap();
            if !original.cost_refreshes.is_empty() {
                checkpoint = Some(tick);
                break;
            }
        }
        let tick = checkpoint.expect("A cost replacement should be in flight");
        let mut restored: Tactics =
            serde_json::from_slice(&serde_json::to_vec(&original).unwrap()).unwrap();
        let mut saved_grid: NavGrid =
            serde_json::from_slice(&serde_json::to_vec(&g).unwrap()).unwrap();
        for next in tick + 1..=tick + 35 {
            let mut cell = g.cells[1];
            cell.cover = (next % 25) as u8 * 8;
            g.set_cell(1, cell).unwrap();
            saved_grid.set_cell(1, cell).unwrap();
            original.step(next, &g).unwrap();
            restored.step(next, &saved_grid).unwrap();
        }
        assert_eq!(
            serde_json::to_value(&original).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            serde_json::to_value(&saved_grid).unwrap()
        );
    }

    #[test]
    fn idle_friend_yields_without_ghosting_enemies_or_other_workers() {
        let g = grid(20, 4);
        let mut tactics = Tactics::new(43);
        let worker = tactics.spawn(0, Role::Worker, [8., 24.]).unwrap();
        let guard = tactics.spawn(0, Role::Soldier, [14., 24.]).unwrap();
        tactics.assign(worker, [140., 24.], Some(1)).unwrap();
        let guard_start = tactics.agent(guard).unwrap().position_mm;
        for tick in 1..=600 {
            tactics.step(tick, &g).unwrap();
            assert!(
                distance_sq(
                    tactics.agent(worker).unwrap().position_mm,
                    tactics.agent(guard).unwrap().position_mm
                ) >= 5000i64.pow(2)
            );
        }
        assert_ne!(tactics.agent(guard).unwrap().position_mm, guard_start);
        assert!(tactics.agent(worker).unwrap().position_mm[0] > guard_start[0] + 10_000);
        let mut hostile = Tactics::new(44);
        let worker = hostile.spawn(0, Role::Worker, [8., 24.]).unwrap();
        let enemy = hostile.spawn(1, Role::Scout, [12.5, 24.]).unwrap();
        hostile.assign(worker, [140., 24.], Some(1)).unwrap();
        let enemy_start = hostile.agent(enemy).unwrap().position_mm;
        for tick in 1..=60 {
            hostile.step(tick, &g).unwrap();
        }
        assert_eq!(hostile.agent(enemy).unwrap().position_mm, enemy_start);
        hostile.validate().unwrap();
    }
    #[test]
    fn a_new_road_eventually_changes_preference_without_discarding_live_field() {
        let mut g = grid(10, 3);
        for cell in &mut g.cells {
            cell.road = false;
        }
        let mut tactics = Tactics::new(45);
        let id = tactics.spawn(0, Role::Worker, [8., 24.]).unwrap();
        tactics.assign(id, [152., 24.], Some(3)).unwrap();
        tactics.refresh_routes(1, &g).unwrap();
        let key = RouteKey {
            goal: 19,
            movement: MovementClass::Foot,
        };
        assert_eq!(tactics.routes[&key].next_cell(10, &g), Some(11));
        for i in 0..10 {
            let mut cell = g.cells[i];
            cell.road = true;
            g.set_cell(i, cell).unwrap();
        }
        for tick in 2..=230 {
            tactics.refresh_routes(tick, &g).unwrap();
        }
        assert_eq!(tactics.totals.route_invalidations, 0);
        assert_eq!(tactics.totals.route_builds, 1);
        assert!(tactics.totals.route_cost_refreshes > 0);
        assert_eq!(tactics.routes[&key].next_cell(10, &g), Some(1));
    }
    #[test]
    fn shared_corridor_keeps_six_haulers_moving_through_a_dense_worker_spawn() {
        let g = grid(80, 140);
        let mut tactics = Tactics::new(46);
        let workers = tactics
            .spawn_formation(0, Role::Worker, [424., 1720.], 32, &g)
            .unwrap();
        let mut picked_up = [false; 6];
        for &id in &workers[..6] {
            tactics
                .assign_on_grid_within(id, [424., 1640.], Some(id), &g, 2.)
                .unwrap();
        }
        for tick in 1..=1200 {
            tactics.step(tick, &g).unwrap();
            for (i, &id) in workers[..6].iter().enumerate() {
                if !picked_up[i]
                    && distance_sq(tactics.agent(id).unwrap().position_mm, [424_000, 1_640_000])
                        <= 22_000i64.pow(2)
                {
                    picked_up[i] = true;
                    // A real pickup hands the hauler its next physical job.
                    tactics
                        .assign_on_grid_within(id, [380. + i as f32 * 18., 1540.], Some(id), &g, 2.)
                        .unwrap();
                }
            }
            for (i, &id) in workers.iter().enumerate() {
                for &other in &workers[i + 1..] {
                    assert!(
                        distance_sq(
                            tactics.agent(id).unwrap().position_mm,
                            tactics.agent(other).unwrap().position_mm
                        ) >= 4400i64.pow(2)
                    );
                }
            }
            if picked_up.iter().all(|done| *done) {
                break;
            }
        }
        assert_eq!(
            picked_up, [true; 6],
            "Every physical hauler must escape its initial crowd and reach the base pickup"
        );
        assert_eq!(tactics.agents.len(), 32);
        tactics.validate().unwrap();
    }
    #[test]
    fn terrain_derived_wet_jungle_corridor_keeps_physical_haulers_progressing() {
        let mut terrain = Terrain::new(78_421);
        let mut grid = NavGrid::from_terrain(&terrain).unwrap();
        let mut tactics = Tactics::new(78_421);
        let workers = tactics
            .spawn_formation(0, Role::Worker, [424., 1720.], 32, &grid)
            .unwrap();
        let mut picked_up = [false; 6];
        for &id in &workers[..6] {
            tactics
                .assign_on_grid_within(id, [424., 1640.], Some(id), &grid, 2.)
                .unwrap();
        }
        for tick in 1..=6000 {
            if tick % 20 == 1 {
                // The normal Terrain -> NavGrid path, using actual generated
                // jungle/height/depth and changing authoritative moisture bits.
                let moisture = if tick % 80 < 40 { 250 } else { 625 };
                for y in 200..220 {
                    for x in 48..58 {
                        let cell = &mut terrain.cells[y * CW + x];
                        cell.flags =
                            (cell.flags & !SOIL_MOISTURE_MASK) | (moisture << SOIL_MOISTURE_SHIFT);
                    }
                }
                grid.refresh(&terrain).unwrap();
            }
            tactics.step(tick, &grid).unwrap();
            for (i, &id) in workers[..6].iter().enumerate() {
                if !picked_up[i]
                    && distance_sq(tactics.agent(id).unwrap().position_mm, [424_000, 1_640_000])
                        <= 22_000i64.pow(2)
                {
                    picked_up[i] = true;
                    tactics.stop(id).unwrap();
                }
            }
            for (i, &id) in workers.iter().enumerate() {
                for &other in &workers[i + 1..] {
                    assert!(
                        distance_sq(
                            tactics.agent(id).unwrap().position_mm,
                            tactics.agent(other).unwrap().position_mm
                        ) >= 4400i64.pow(2)
                    );
                }
            }
            if picked_up.iter().all(|done| *done) {
                break;
            }
        }
        assert_eq!(
            picked_up, [true; 6],
            "Generated wet jungle must permit every real hauler to reach pickup"
        );
        assert_eq!(tactics.agents.len(), 32);
        tactics.validate().unwrap();
    }
}
