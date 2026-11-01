//! Finite ammunition built from already-paid construction material. These are
//! real local turret/store stocks, never a global or automatically refilled bank.
use crate::{
    construction::{BuildKind, Building, ConstructionWorld},
    tactics::{AgentId, NavGrid, TacticalEvent, Tactics},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const TARGET_INTERVAL: u64 = 20;
const SHOT_COOLDOWN: u64 = 10;
const TURRET_RANGE_MM: i64 = 144_000;
const TURRET_DAMAGE: u16 = 25;
const SUPPLY_EDGE_RANGE_MM: i64 = 32_000;
const CANDIDATE_BUDGET: usize = 512;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SupportStock {
    pub building_id: u64,
    pub team: u8,
    pub kind: BuildKind,
    pub position_mm: [i32; 2],
    pub size_mm: u32,
    pub construction_metal_paid: u64,
    pub converted_metal_units: u64,
    pub initial_rounds: u64,
    pub rounds: u64,
    pub supplied_rounds: u64,
    pub fired_rounds: u64,
    pub destroyed_rounds: u64,
    pub decommissioned: bool,
    pub target: Option<AgentId>,
    pub next_shot_tick: u64,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SupportTotals {
    pub imported_from_paid_construction: [u64; 2],
    pub converted_metal_units: [u64; 2],
    pub supplied_rounds: [u64; 2],
    pub fired_rounds: [u64; 2],
    pub destroyed_rounds: [u64; 2],
    pub targets_killed: [u64; 2],
    pub acquisitions: u64,
    pub candidates_examined: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct SupportShot {
    pub building_id: u64,
    pub team: u8,
    pub target: AgentId,
    pub target_position: [f32; 2],
    pub damage: u16,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FactionSupport {
    pub last_tick: u64,
    pub stocks: BTreeMap<u64, SupportStock>,
    pub totals: SupportTotals,
    /// Transient committed effects for campaign collateral/combat-deadline processing.
    #[serde(skip)]
    pub recent_shots: Vec<SupportShot>,
    #[serde(skip)]
    spatial: SupportSpatial,
    #[serde(skip)]
    candidates: Vec<usize>,
}
#[derive(Clone, Debug, Default)]
struct SupportSpatial {
    heads: [Vec<i32>; 2],
    touched: [Vec<usize>; 2],
    next: Vec<i32>,
}
impl SupportSpatial {
    fn rebuild(&mut self, tactics: &Tactics, grid: &NavGrid, tick: u64) -> Result<(), String> {
        for team in 0..2 {
            if self.heads[team].len() != grid.cells.len() {
                self.heads[team] = vec![-1; grid.cells.len()];
                self.touched[team].clear();
            } else {
                for i in self.touched[team].drain(..) {
                    self.heads[team][i] = -1;
                }
            }
        }
        self.next.resize(tactics.agents.len(), -1);
        // Rotate insertion order to prevent a dense, stable bucket from hiding
        // the same late IDs forever behind the per-decision candidate budget.
        let n = tactics.agents.len();
        let start = if n == 0 {
            0
        } else {
            ((tick as u128 * 251) % n as u128) as usize
        };
        for offset in 0..n {
            let i = (start + offset) % n;
            let a = &tactics.agents[i];
            if !a.alive() || a.embarked_on.is_some() {
                continue;
            }
            if a.team > 1 {
                return Err("Invalid tactical faction for support index".into());
            }
            let cell = grid
                .index_mm(a.position_mm)
                .ok_or("Tactical agent outside support navigation grid")?;
            let team = a.team as usize;
            if self.heads[team][cell] < 0 {
                self.touched[team].push(cell);
            }
            self.next[i] = self.heads[team][cell];
            self.heads[team][cell] =
                i32::try_from(i).map_err(|_| "Support spatial index capacity exhausted")?;
        }
        Ok(())
    }
    fn collect(
        &self,
        team: u8,
        position: [i32; 2],
        radius: i64,
        grid: &NavGrid,
        out: &mut Vec<usize>,
    ) {
        out.clear();
        let x = position[0] as i64 / grid.cell_mm as i64;
        let y = position[1] as i64 / grid.cell_mm as i64;
        let r = (radius + grid.cell_mm as i64 - 1) / grid.cell_mm as i64;
        'rows: for yy in (y - r).max(0)..=(y + r).min(grid.height as i64 - 1) {
            for xx in (x - r).max(0)..=(x + r).min(grid.width as i64 - 1) {
                let mut i = self.heads[team as usize][yy as usize * grid.width + xx as usize];
                while i >= 0 {
                    if out.len() == CANDIDATE_BUDGET {
                        break 'rows;
                    }
                    out.push(i as usize);
                    i = self.next[i as usize];
                }
            }
        }
    }
}
impl FactionSupport {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn step(
        &mut self,
        tick: u64,
        construction: &ConstructionWorld,
        tactics: &mut Tactics,
        nav: &NavGrid,
    ) -> Result<(), String> {
        if self.last_tick.checked_add(1) != Some(tick) {
            return Err("Faction support requires contiguous authoritative ticks".into());
        }
        if tactics.last_tick != tick || construction.last_tick != tick {
            return Err(
                "Faction support must run after tactics and construction on the same tick".into(),
            );
        }
        self.validate()?;
        self.recent_shots.clear();
        if nav.cell_mm <= 0
            || nav.width == 0
            || nav.height == 0
            || nav.width.checked_mul(nav.height) != Some(nav.cells.len())
        {
            return Err("Invalid support navigation grid".into());
        }
        self.spatial.rebuild(tactics, nav, tick)?;
        let by_id = construction
            .buildings
            .iter()
            .map(|b| (b.id, b))
            .collect::<BTreeMap<_, _>>();
        if by_id.len() != construction.buildings.len() {
            return Err("Duplicate construction identity in faction support".into());
        }
        for (&id, s) in &self.stocks {
            if let Some(b) = by_id.get(&id) {
                if s.team != b.team
                    || s.kind != b.kind
                    || s.position_mm != position_mm(b.position)?
                    || s.construction_metal_paid != b.construction_cost.metal
                    || s.size_mm != size_mm(b)?
                {
                    return Err("Support source identity changed or was reused".into());
                }
            }
        }
        let mut pending = Vec::new();
        let mut total = self.totals;
        for b in &construction.buildings {
            let Some(rounds) = initial_rounds(b.kind) else {
                continue;
            };
            if !b.complete {
                continue;
            }
            if b.team > 1 || b.construction_cost.metal < rounds {
                return Err("Support ammunition lacks its paid construction-metal source".into());
            }
            let position_mm = position_mm(b.position)?;
            if nav.index_mm(position_mm).is_none() {
                return Err("Support building outside navigation grid".into());
            }
            if let Some(s) = self.stocks.get(&b.id) {
                if s.team != b.team
                    || s.kind != b.kind
                    || s.position_mm != position_mm
                    || s.construction_metal_paid != b.construction_cost.metal
                {
                    return Err("Support source identity changed or was reused".into());
                }
            } else {
                let team = b.team as usize;
                total.imported_from_paid_construction[team] = checked(
                    total.imported_from_paid_construction[team],
                    rounds,
                    "support import",
                )?;
                total.converted_metal_units[team] = checked(
                    total.converted_metal_units[team],
                    rounds,
                    "metal conversion",
                )?;
                pending.push(SupportStock {
                    building_id: b.id,
                    team: b.team,
                    kind: b.kind,
                    position_mm,
                    size_mm: size_mm(b)?,
                    construction_metal_paid: b.construction_cost.metal,
                    converted_metal_units: rounds,
                    initial_rounds: rounds,
                    rounds,
                    supplied_rounds: 0,
                    fired_rounds: 0,
                    destroyed_rounds: 0,
                    decommissioned: false,
                    target: None,
                    next_shot_tick: tick,
                });
            }
        }
        for team in 0..2 {
            if total.converted_metal_units[team] > construction.consumed[team].metal {
                return Err("Support converted more metal than was actually paid/consumed".into());
            }
        }
        self.totals = total;
        for s in pending {
            self.stocks.insert(s.building_id, s);
        }
        self.recent_shots.reserve(self.stocks.len());
        let ids = self.stocks.keys().copied().collect::<Vec<_>>();
        for id in ids {
            let alive = by_id.get(&id).is_some_and(|b| b.complete && b.health > 0);
            if !alive {
                let s = self.stocks.get_mut(&id).expect("known support stock");
                if !s.decommissioned {
                    let team = s.team as usize;
                    let lost = checked(
                        self.totals.destroyed_rounds[team],
                        s.rounds,
                        "destroyed ammunition",
                    )?;
                    s.destroyed_rounds =
                        checked(s.destroyed_rounds, s.rounds, "building ammunition loss")?;
                    s.rounds = 0;
                    s.target = None;
                    s.decommissioned = true;
                    self.totals.destroyed_rounds[team] = lost;
                }
                continue;
            }
            let s = &self.stocks[&id];
            if s.decommissioned || s.rounds == 0 {
                continue;
            }
            if s.kind == BuildKind::Turret {
                if tick % TARGET_INTERVAL == id % TARGET_INTERVAL {
                    self.spatial.collect(
                        1 - s.team,
                        s.position_mm,
                        TURRET_RANGE_MM,
                        nav,
                        &mut self.candidates,
                    );
                    let mut target = None;
                    let mut nearest = i64::MAX;
                    for &i in &self.candidates {
                        let a = &tactics.agents[i];
                        let d = distance_sq(s.position_mm, a.position_mm);
                        if d > TURRET_RANGE_MM * TURRET_RANGE_MM
                            || !clear_line(s, a.position_mm, nav)
                        {
                            continue;
                        }
                        if d < nearest || d == nearest && target.is_none_or(|id| a.id < id) {
                            target = Some(a.id);
                            nearest = d;
                        }
                    }
                    self.totals.acquisitions =
                        checked(self.totals.acquisitions, 1, "target acquisitions")?;
                    self.totals.candidates_examined = checked(
                        self.totals.candidates_examined,
                        self.candidates.len() as u64,
                        "candidate budget",
                    )?;
                    self.stocks.get_mut(&id).unwrap().target = target;
                }
                let s = &self.stocks[&id];
                if tick < s.next_shot_tick {
                    continue;
                }
                let Some(target) = s
                    .target
                    .and_then(|target| tactics.agent(target))
                    .filter(|a| a.alive() && a.embarked_on.is_none() && a.team != s.team)
                else {
                    continue;
                };
                if distance_sq(s.position_mm, target.position_mm)
                    > TURRET_RANGE_MM * TURRET_RANGE_MM
                    || !clear_line(s, target.position_mm, nav)
                {
                    continue;
                }
                let target_id = target.id;
                let target_position = target.position();
                let target_was_alive = target.alive();
                let team = s.team as usize;
                let next = tick
                    .checked_add(SHOT_COOLDOWN)
                    .ok_or("Turret cooldown overflow")?;
                let total_fired = checked(self.totals.fired_rounds[team], 1, "turret firing")?;
                let stock_fired = checked(s.fired_rounds, 1, "local turret firing")?;
                let possible_kills = if target.health <= TURRET_DAMAGE {
                    checked(self.totals.targets_killed[team], 1, "turret kills")?
                } else {
                    self.totals.targets_killed[team]
                };
                // The shot belongs to this turret ledger; it is not deducted
                // from an unrelated pawn or counted as that pawn's ammunition.
                let events = tactics.damage(target_id, TURRET_DAMAGE, None)?;
                let s = self.stocks.get_mut(&id).unwrap();
                s.rounds -= 1;
                s.fired_rounds = stock_fired;
                s.next_shot_tick = next;
                self.totals.fired_rounds[team] = total_fired;
                self.recent_shots.push(SupportShot {
                    building_id: id,
                    team: team as u8,
                    target: target_id,
                    target_position,
                    damage: TURRET_DAMAGE,
                });
                if target_was_alive
                    && events
                        .iter()
                        .any(|e| matches!(e,TacticalEvent::Died{agent,..}if *agent==target_id))
                {
                    self.totals.targets_killed[team] = possible_kills;
                }
            } else if tick % TARGET_INTERVAL == id % TARGET_INTERVAL {
                let radius = s.size_mm as i64 / 2;
                self.spatial.collect(
                    s.team,
                    s.position_mm,
                    radius + SUPPLY_EDGE_RANGE_MM,
                    nav,
                    &mut self.candidates,
                );
                for &i in &self.candidates {
                    let s = &self.stocks[&id];
                    if s.rounds == 0 {
                        break;
                    }
                    let a = &tactics.agents[i];
                    let d = distance_sq(s.position_mm, a.position_mm);
                    if !a.alive()
                        || a.team != s.team
                        || a.embarked_on.is_some()
                        || d < radius * radius
                        || d > (radius + SUPPLY_EDGE_RANGE_MM).pow(2)
                        || !clear_line(s, a.position_mm, nav)
                    {
                        continue;
                    }
                    let offer =
                        u16::try_from(s.rounds.min(100)).map_err(|_| "Support offer overflow")?;
                    // All local/global counters are checked before the external
                    // owner accepts anything, so no late failure duplicates ammo.
                    checked(
                        self.totals.supplied_rounds[s.team as usize],
                        offer as u64,
                        "supply total",
                    )?;
                    checked(s.supplied_rounds, offer as u64, "local supply total")?;
                    let accepted = tactics.give_ammunition(a.id, offer)? as u64;
                    let s = self.stocks.get_mut(&id).unwrap();
                    s.rounds -= accepted;
                    s.supplied_rounds += accepted;
                    self.totals.supplied_rounds[s.team as usize] += accepted;
                }
            }
        }
        self.last_tick = tick;
        self.validate()?;
        Ok(())
    }
    pub fn validate(&self) -> Result<(), String> {
        let mut imported = [0u64; 2];
        let mut converted = [0u64; 2];
        let mut fired = [0u64; 2];
        let mut supplied = [0u64; 2];
        let mut lost = [0u64; 2];
        for (&id, s) in &self.stocks {
            if id != s.building_id
                || s.team > 1
                || s.size_mm == 0
                || initial_rounds(s.kind) != Some(s.initial_rounds)
                || s.converted_metal_units != s.initial_rounds
                || s.construction_metal_paid < s.converted_metal_units
            {
                return Err("Invalid paid support source".into());
            }
            if s.rounds
                .checked_add(s.fired_rounds)
                .and_then(|n| n.checked_add(s.supplied_rounds))
                .and_then(|n| n.checked_add(s.destroyed_rounds))
                != Some(s.initial_rounds)
            {
                return Err("Local support ammunition does not balance".into());
            }
            if s.decommissioned && s.rounds != 0
                || s.kind == BuildKind::Turret && s.supplied_rounds != 0
                || s.kind != BuildKind::Turret && s.fired_rounds != 0
            {
                return Err("Invalid support ammunition role/loss state".into());
            }
            let team = s.team as usize;
            imported[team] = checked(imported[team], s.initial_rounds, "import census")?;
            converted[team] = checked(converted[team], s.converted_metal_units, "metal census")?;
            fired[team] = checked(fired[team], s.fired_rounds, "firing census")?;
            supplied[team] = checked(supplied[team], s.supplied_rounds, "supply census")?;
            lost[team] = checked(lost[team], s.destroyed_rounds, "loss census")?;
        }
        if self
            .totals
            .targets_killed
            .iter()
            .zip(self.totals.fired_rounds)
            .any(|(&k, f)| k > f)
        {
            return Err("Turret kill count exceeds actual shots".into());
        }
        if self.totals.candidates_examined as u128
            > self.totals.acquisitions as u128 * CANDIDATE_BUDGET as u128
        {
            return Err("Support target scan exceeded its budget".into());
        }
        if imported != self.totals.imported_from_paid_construction
            || converted != self.totals.converted_metal_units
            || fired != self.totals.fired_rounds
            || supplied != self.totals.supplied_rounds
            || lost != self.totals.destroyed_rounds
        {
            return Err("Global support ammunition does not balance".into());
        }
        Ok(())
    }
    pub fn report(&self) -> serde_json::Value {
        serde_json::json!({"kind":"finite_paid_structure_ammunition_and_local_support","tick":self.last_tick,"totals":self.totals,"stocks":self.stocks,"conserved":self.validate().is_ok(),"automatic_refill":false,"candidate_limit_per_acquisition":CANDIDATE_BUDGET})
    }
}
fn initial_rounds(kind: BuildKind) -> Option<u64> {
    match kind {
        BuildKind::Turret => Some(50),
        BuildKind::Depot => Some(100),
        BuildKind::Barracks => Some(60),
        _ => None,
    }
}
fn checked(a: u64, b: u64, context: &str) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| format!("{context} overflow"))
}
fn position_mm(p: [f32; 2]) -> Result<[i32; 2], String> {
    if p.iter()
        .any(|n| !n.is_finite() || *n < 0. || *n as f64 * 1000. > i32::MAX as f64)
    {
        return Err("Invalid support position".into());
    }
    Ok(p.map(|n| (n as f64 * 1000.).round() as i32))
}
fn size_mm(b: &Building) -> Result<u32, String> {
    if !b.size_meters.is_finite() || b.size_meters <= 0. || b.size_meters > 1000. {
        return Err("Invalid support building size".into());
    }
    Ok((b.size_meters as f64 * 1000.).round() as u32)
}
fn distance_sq(a: [i32; 2], b: [i32; 2]) -> i64 {
    let x = a[0] as i64 - b[0] as i64;
    let y = a[1] as i64 - b[1] as i64;
    x * x + y * y
}
fn clear_line(s: &SupportStock, target: [i32; 2], nav: &NavGrid) -> bool {
    let (Some(from), Some(to)) = (nav.index_mm(s.position_mm), nav.index_mm(target)) else {
        return false;
    };
    let dx = target[0] as i64 - s.position_mm[0] as i64;
    let dy = target[1] as i64 - s.position_mm[1] as i64;
    let steps = ((dx.abs().max(dy.abs()) / (nav.cell_mm as i64 / 2).max(1)) + 1).min(4096);
    let elevation = |i: usize| {
        let c = nav.cells[i];
        c.bridge_deck_mm
            .map(|h| h as i64)
            .unwrap_or(c.elevation_mm as i64 + c.reef_height_mm as i64)
    };
    let h0 = elevation(from) + 1600;
    let h1 = elevation(to) + 1600;
    // Construction currently occupies its exact3×3 eight-metre sample footprint.
    // Ignore only that building's coarse occupied cells, never a blanket36m
    // muzzle jump that could skip an unrelated ridge or neighbouring wall.
    let sx = s.position_mm[0] as i64 / 8000;
    let sy = s.position_mm[1] as i64 / 8000;
    let left = (sx - 1).max(0) * 8000;
    let top = (sy - 1).max(0) * 8000;
    let right = (sx + 2).min(crate::terrain::CW as i64) * 8000;
    let bottom = (sy + 2).min(crate::terrain::CH as i64) * 8000;
    for step in 1..steps {
        let p = [
            (s.position_mm[0] as i64 + dx * step / steps) as i32,
            (s.position_mm[1] as i64 + dy * step / steps) as i32,
        ];
        let Some(i) = nav.index_mm(p) else {
            return false;
        };
        if i == from || i == to {
            continue;
        }
        let x = (i % nav.width) as i64 * nav.cell_mm as i64;
        let y = (i / nav.width) as i64 * nav.cell_mm as i64;
        let own = x < right
            && x + nav.cell_mm as i64 > left
            && y < bottom
            && y + nav.cell_mm as i64 > top;
        if nav.cells[i].blocked && !own || elevation(i) > h0 + (h1 - h0) * step / steps {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        construction::Resources,
        tactics::{NAV_CELL_MM, NavCell, Role},
    };
    fn grid() -> NavGrid {
        NavGrid::new(32, 24, NAV_CELL_MM, vec![NavCell::default(); 32 * 24]).unwrap()
    }
    fn building(c: &mut ConstructionWorld, id: u64, kind: BuildKind, p: [f32; 2]) {
        let cost = kind.base_cost();
        c.consume(0, cost).unwrap();
        c.buildings.push(Building {
            id,
            kind,
            team: 0,
            position: p,
            end: p,
            health: 1000,
            max_health: 1000,
            complete: true,
            size_meters: 40.,
            deck_height_mm: None,
            draft_clearance_mm: 0,
            load_limit_kg: 0,
            construction_cost: cost,
        });
        c.next_id = c.next_id.max(id + 1);
    }
    fn advance(
        s: &mut FactionSupport,
        c: &mut ConstructionWorld,
        t: &mut Tactics,
        n: &NavGrid,
        tick: u64,
    ) {
        t.step(tick, n).unwrap();
        c.last_tick = tick;
        s.step(tick, c, t, n).unwrap();
        t.validate().unwrap();
        s.validate().unwrap();
    }
    #[test]
    fn turret_hits_real_target_on_cooldown_with_finite_paid_rounds() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Turret, [100., 100.]);
        let mut t = Tactics::new(1);
        let enemy = t.spawn(1, Role::Worker, [200., 100.]).unwrap();
        let n = grid();
        let mut s = FactionSupport::new();
        advance(&mut s, &mut c, &mut t, &n, 1);
        assert_eq!(t.agent(enemy).unwrap().health, 75);
        assert_eq!(s.stocks[&1].rounds, 49);
        for tick in 2..=10 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert_eq!(t.agent(enemy).unwrap().health, 75);
        assert_eq!(s.totals.fired_rounds[0], 1);
        for tick in 11..=45 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert!(!t.agent(enemy).unwrap().alive());
        assert_eq!(s.totals.targets_killed[0], 1);
        assert_eq!(s.totals.fired_rounds[0], 4);
        assert_eq!(s.stocks[&1].rounds, 46);
        assert_eq!(s.totals.converted_metal_units[0], 50);
    }
    #[test]
    fn exhausted_turret_does_not_receive_invisible_replacement_rounds() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Turret, [100., 100.]);
        let stores = c.stores;
        let mut t = Tactics::new(1);
        for i in 0..15 {
            t.spawn(
                1,
                Role::Worker,
                [180. + (i % 8) as f32 * 6., 100. + (i / 8) as f32 * 10.],
            )
            .unwrap();
        }
        let n = grid();
        let mut s = FactionSupport::new();
        for tick in 1..=900 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert_eq!(s.totals.fired_rounds[0], 50);
        assert_eq!(s.stocks[&1].rounds, 0);
        assert_eq!(s.totals.imported_from_paid_construction[0], 50);
        assert_eq!(c.stores, stores);
        assert!(t.alive_count(1) > 0);
    }
    #[test]
    fn blocked_line_spends_nothing_and_opening_wall_allows_fire() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Turret, [100., 100.]);
        let mut t = Tactics::new(1);
        let enemy = t.spawn(1, Role::Worker, [200., 100.]).unwrap();
        let mut n = grid();
        let wall = n.index([160., 100.]).unwrap();
        let mut cell = n.cells[wall];
        cell.blocked = true;
        n.set_cell(wall, cell).unwrap();
        let mut s = FactionSupport::new();
        for tick in 1..=21 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert_eq!(s.stocks[&1].rounds, 50);
        assert_eq!(t.agent(enemy).unwrap().health, 100);
        cell.blocked = false;
        n.set_cell(wall, cell).unwrap();
        for tick in 22..=41 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert!(s.totals.fired_rounds[0] > 0);
    }
    #[test]
    fn own_building_footprint_does_not_block_but_nearby_ridge_still_does() {
        use crate::construction::BuildOrder;
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Turret, [100., 100.]);
        let mut t = Tactics::new(1);
        let target = t.spawn(1, Role::Worker, [200., 100.]).unwrap();
        let mut n = grid();
        for cell in
            ConstructionWorld::footprint_cells(BuildOrder::at(0, BuildKind::Turret, [100., 100.]))
        {
            let p = [
                (cell % crate::terrain::CW * 8 + 4) as f32,
                (cell / crate::terrain::CW * 8 + 4) as f32,
            ];
            let i = n.index(p).unwrap();
            let mut v = n.cells[i];
            v.blocked = true;
            n.set_cell(i, v).unwrap();
        }
        let mut s = FactionSupport::new();
        advance(&mut s, &mut c, &mut t, &n, 1);
        assert_eq!(t.agent(target).unwrap().health, 75);
        let ridge = n.index([120., 100.]).unwrap();
        let mut v = n.cells[ridge];
        v.elevation_mm = 10_000;
        n.set_cell(ridge, v).unwrap();
        for tick in 2..=21 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert_eq!(s.totals.fired_rounds[0], 1);
        assert_eq!(s.stocks[&1].rounds, 49);
    }
    #[test]
    fn supply_requires_local_physical_arrival_and_caps_actual_acceptance() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Depot, [100., 100.]);
        let mut t = Tactics::new(1);
        let soldier = t.spawn(0, Role::Soldier, [250., 100.]).unwrap();
        let n = grid();
        let mut s = FactionSupport::new();
        for tick in 1..=21 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert_eq!(t.agent(soldier).unwrap().ammunition, 40);
        assert_eq!(s.stocks[&1].rounds, 100);
        t.assign(soldier, [140., 100.], None).unwrap();
        let mut reached = false;
        for tick in 22..=2500 {
            advance(&mut s, &mut c, &mut t, &n, tick);
            if t.agent(soldier).unwrap().ammunition == 100 {
                reached = true;
                break;
            }
        }
        assert!(reached);
        assert_eq!(s.stocks[&1].rounds, 40);
        assert_eq!(s.totals.supplied_rounds[0], 60);
        assert_eq!(t.totals.supplied_ammunition, 60);
        let before = s.stocks[&1].rounds;
        for tick in s.last_tick + 1..=s.last_tick + 20 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        assert_eq!(s.stocks[&1].rounds, before);
    }
    #[test]
    fn remaining_stock_is_lost_once_on_destruction_and_cannot_respawn() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Barracks, [100., 100.]);
        let mut t = Tactics::new(1);
        let n = grid();
        let mut s = FactionSupport::new();
        advance(&mut s, &mut c, &mut t, &n, 1);
        c.buildings[0].health = 0;
        advance(&mut s, &mut c, &mut t, &n, 2);
        advance(&mut s, &mut c, &mut t, &n, 3);
        assert_eq!(s.totals.destroyed_rounds[0], 60);
        assert_eq!(s.stocks[&1].rounds, 0);
        c.buildings[0].health = 1000;
        advance(&mut s, &mut c, &mut t, &n, 4);
        assert_eq!(s.stocks[&1].rounds, 0);
        assert_eq!(s.totals.imported_from_paid_construction[0], 60);
    }
    #[test]
    fn save_reload_keeps_cooldown_target_ammo_and_exact_continuation() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Turret, [100., 100.]);
        let mut t = Tactics::new(1);
        t.spawn(1, Role::Worker, [200., 100.]).unwrap();
        let n = grid();
        let mut s = FactionSupport::new();
        for tick in 1..=7 {
            advance(&mut s, &mut c, &mut t, &n, tick);
        }
        let mut loaded: FactionSupport =
            serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        let mut t2 = t.clone();
        let mut c2 = c.clone();
        for tick in 8..=45 {
            advance(&mut s, &mut c, &mut t, &n, tick);
            advance(&mut loaded, &mut c2, &mut t2, &n, tick);
        }
        assert_eq!(s.report(), loaded.report());
        assert_eq!(
            serde_json::to_value(t.totals).unwrap(),
            serde_json::to_value(t2.totals).unwrap()
        );
    }
    #[test]
    fn unsupported_or_unpaid_sources_and_reused_ids_are_rejected() {
        let mut c = ConstructionWorld::new(1);
        building(&mut c, 1, BuildKind::Depot, [100., 100.]);
        c.buildings[0].construction_cost = Resources::default();
        let mut t = Tactics::new(1);
        let n = grid();
        t.step(1, &n).unwrap();
        c.last_tick = 1;
        let mut s = FactionSupport::new();
        assert!(s.step(1, &c, &mut t, &n).is_err());
        assert!(s.stocks.is_empty());
        assert_eq!(s.last_tick, 0);
        c.buildings[0].construction_cost = BuildKind::Depot.base_cost();
        s.step(1, &c, &mut t, &n).unwrap();
        c.buildings[0].team = 1;
        t.step(2, &n).unwrap();
        c.last_tick = 2;
        assert!(s.step(2, &c, &mut t, &n).is_err());
    }
}
