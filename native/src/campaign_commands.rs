//! Authoritative selected-unit commands. UI selection/panels contain no owned
//! gameplay queues; Shift orders persist here with the campaign save.
use crate::{
    campaign::Campaign,
    construction::JobPhase,
    logging,
    tactics::{MovementClass, NavGrid, Role, TacticalAgent, Tactics},
    ui_interaction::UiCommand,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const MAX_UNIT_QUEUE: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum UnitOrderKind {
    Move {
        position: [f32; 2],
    },
    Attack {
        position: [f32; 2],
        target: Option<u64>,
    },
    Board {
        transport: u64,
        last_known_position: [f32; 2],
    },
    Disembark {
        position: [f32; 2],
    },
    Hold,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingUnitOrder {
    pub kind: UnitOrderKind,
    pub issued_tick: u64,
    pub started_tick: Option<u64>,
    pub formation_offset_mm: [i32; 2],
    pub resolved_goal_mm: Option<[i32; 2]>,
}
type Queues = BTreeMap<u64, VecDeque<PendingUnitOrder>>;

impl Campaign {
    pub fn apply_ui_command(&mut self, command: &UiCommand) -> Result<(), String> {
        if self.data.winner.is_some() {
            return Err("The battle has ended".into());
        }
        match command {
            UiCommand::MoveSelected {
                ids,
                position,
                queue,
            } => {
                self.check_command_destination(*position)?;
                self.queue_selected(
                    ids,
                    UnitOrderKind::Move {
                        position: *position,
                    },
                    *queue,
                )?;
            }
            UiCommand::AttackSelected {
                ids,
                position,
                target,
                queue,
            } => {
                self.check_command_destination(*position)?;
                if let Some(target) = target {
                    if !self.data.tactics.agent(*target).is_some_and(|a| {
                        a.team != 0
                            && a.alive()
                            && a.embarked_on.is_none()
                            && self.data.recon.visible(0, a.position())
                    }) {
                        return Err("Choose a currently identified opposing unit in video".into());
                    }
                }
                self.queue_selected(
                    ids,
                    UnitOrderKind::Attack {
                        position: *position,
                        target: *target,
                    },
                    *queue,
                )?;
            }
            UiCommand::BoardSelected {
                ids,
                transport,
                queue,
            } => {
                let vessel = self
                    .data
                    .tactics
                    .agent(*transport)
                    .filter(|a| {
                        a.team == 0
                            && a.alive()
                            && a.role == Role::Transport
                            && self.data.recon.visible(0, a.position())
                    })
                    .ok_or("Transport is not currently available in video")?;
                let point = vessel.position();
                self.queue_selected(
                    ids,
                    UnitOrderKind::Board {
                        transport: *transport,
                        last_known_position: point,
                    },
                    *queue,
                )?;
            }
            UiCommand::DisembarkSelected {
                transports,
                position,
                queue,
            } => {
                self.check_command_destination(*position)?;
                self.queue_selected(
                    transports,
                    UnitOrderKind::Disembark {
                        position: *position,
                    },
                    *queue,
                )?;
            }
            UiCommand::StopSelected { ids } => {
                let valid = valid_selection(&self.data.tactics, ids)?;
                let staged = stage_queues(
                    &self.data.tactics,
                    &self.data.unit_order_queues,
                    &valid,
                    UnitOrderKind::Hold,
                    false,
                    self.data.clock.tick(),
                )?;
                for &id in &valid {
                    self.data.tactics.stop(id)?;
                }
                for (id, queue) in staged {
                    self.data.unit_order_queues.insert(id, queue);
                }
                self.data.status = format!("{} selected units holding", valid.len());
            }
            UiCommand::UseTool { tool, position } => {
                if matches!(*tool, 24 | 27 | 28) {
                    return Err("Select the units or transports for this command first".into());
                }
                let previous = self.data.selected_tool;
                self.data.selected_tool = *tool;
                let result = self.click(*position, false);
                self.data.selected_tool = previous;
                result?;
            }
            UiCommand::BuildSegment { tool, from, to, .. } => {
                let previous = self.data.selected_tool;
                self.data.selected_tool = *tool;
                let result = self.place_segment(*from, *to);
                self.data.selected_tool = previous;
                result?;
            }
            UiCommand::SelectSpecies(species) => {
                if !self.data.ecology.knowledge[0]
                    .get(*species as usize)
                    .is_some_and(|k| k.observed)
                {
                    return Err("Species has not been identified in current records".into());
                }
                self.data.selected_species = Some(*species);
            }
            UiCommand::CancelOrder => {
                let job = self
                    .data
                    .construction
                    .jobs
                    .iter()
                    .rev()
                    .find(|j| {
                        j.order.team == 0
                            && !matches!(j.phase, JobPhase::Complete | JobPhase::Cancelled)
                    })
                    .ok_or("No pending order")?
                    .id;
                self.data.construction.cancel(job, 0)?;
                self.data.status =
                    "Pending construction order cancelled; owned materials remain accounted for"
                        .into();
            }
            UiCommand::RecruitCamera => self.recruit_camera()?,
            UiCommand::CycleSpecies => self.cycle_species(),
            UiCommand::Explode { position } => self.explode(*position, 0)?,
            UiCommand::CameraJump { .. }
            | UiCommand::Pause
            | UiCommand::Speed(_)
            | UiCommand::Save
            | UiCommand::Load => {
                return Err("This command is handled by the native window/session controls".into());
            }
        }
        logging::log(
            "PLAYER_COMMAND",
            &serde_json::json!({"tick":self.data.clock.tick(),"command":command}).to_string(),
        );
        Ok(())
    }

    fn check_command_destination(&self, position: [f32; 2]) -> Result<(), String> {
        if !valid_position(position, &self.data.nav) || !self.data.recon.visible(0, position) {
            return Err("Destination needs current video coverage".into());
        }
        Ok(())
    }
    fn queue_selected(
        &mut self,
        ids: &[u64],
        kind: UnitOrderKind,
        append: bool,
    ) -> Result<(), String> {
        let ids = valid_selection(&self.data.tactics, ids)?;
        let mut staged = stage_queues(
            &self.data.tactics,
            &self.data.unit_order_queues,
            &ids,
            kind.clone(),
            append,
            self.data.clock.tick(),
        )?;
        if let UnitOrderKind::Board { transport, .. } = kind {
            for &id in &ids {
                let a = self
                    .data
                    .tactics
                    .agent(id)
                    .ok_or("Selection is not available")?;
                if a.role.movement() != MovementClass::Foot
                    || a.embarked_on.is_some()
                    || id == transport
                {
                    return Err("Selection cannot board this transport".into());
                }
            }
            self.validate_seat_reservations(transport, &staged)?;
        }
        if matches!(kind, UnitOrderKind::Disembark { .. })
            && ids.iter().any(|&id| {
                !self
                    .data
                    .tactics
                    .agent(id)
                    .is_some_and(|a| a.role == Role::Transport && a.embarked_on.is_none())
            })
        {
            return Err("Select available friendly transports to disembark".into());
        }
        let start: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| {
                staged.get(id).is_some_and(|q| {
                    q.len() == 1 && q.front().is_some_and(|o| o.started_tick.is_none())
                })
            })
            .collect();
        if !start.is_empty() {
            let (point, attack) = match kind {
                UnitOrderKind::Move { position } | UnitOrderKind::Disembark { position } => {
                    (position, false)
                }
                UnitOrderKind::Attack { position, .. } => (position, true),
                UnitOrderKind::Board {
                    last_known_position,
                    ..
                } => (last_known_position, false),
                UnitOrderKind::Hold => unreachable!(),
            };
            self.data
                .tactics
                .order_selected_on_grid(0, &start, point, &self.data.nav, attack)
                .map_err(|_| "Selected units cannot currently reach that order")?;
            for id in start {
                let a = self
                    .data
                    .tactics
                    .agent(id)
                    .ok_or("Selection is not available")?;
                let q = staged.get_mut(&id).expect("staged selected queue");
                let o = q.front_mut().expect("staged active order");
                o.started_tick = Some(self.data.clock.tick());
                o.resolved_goal_mm = a.goal_mm;
            }
        }
        for (id, q) in staged {
            self.data.unit_order_queues.insert(id, q);
        }
        // Restore original group slots even when only an empty subset starts
        // immediately and other members have earlier Shift orders to finish.
        for &id in &ids {
            if let Some(o) = self
                .data
                .unit_order_queues
                .get(&id)
                .and_then(|q| q.front())
                .filter(|o| o.started_tick.is_some())
            {
                if let Some(a) = self.data.tactics.agent_mut(id) {
                    a.formation_offset_mm = o.formation_offset_mm;
                    if let UnitOrderKind::Attack { target, .. } = o.kind {
                        a.commanded_target = target;
                    } else {
                        a.commanded_target = None;
                    }
                }
            }
        }
        self.data.status = if append {
            format!("Order queued for{} selected units", ids.len())
        } else {
            format!("Order issued to{} selected units", ids.len())
        };
        Ok(())
    }

    fn validate_seat_reservations(&self, transport: u64, staged: &Queues) -> Result<(), String> {
        let vessel = self
            .data
            .tactics
            .agent(transport)
            .filter(|a| a.alive() && a.team == 0 && a.role == Role::Transport)
            .ok_or("Transport is not available")?;
        let _ = vessel;
        let mut reserved = BTreeSet::new();
        for (&id, q) in self
            .data
            .unit_order_queues
            .iter()
            .filter(|(id, _)| !staged.contains_key(id))
            .chain(staged.iter())
        {
            if self
                .data
                .tactics
                .agent(id)
                .is_some_and(|a| a.alive() && a.embarked_on.is_none())
                && q.iter()
                    .any(|o| matches!(o.kind,UnitOrderKind::Board{transport:t,..}if t==transport))
            {
                reserved.insert(id);
            }
        }
        let aboard = self
            .data
            .tactics
            .manifests
            .get(&transport)
            .ok_or("Transport manifest unavailable")?
            .len();
        if aboard + reserved.len() > 20 {
            return Err("Transport has insufficient unreserved capacity for this selection".into());
        }
        Ok(())
    }

    /// Called once before the matching Tactics tick. Completion uses physical
    /// arrival/boarding, never a UI timer or animation. Future orders remain in
    /// the saved queue until their predecessor actually finishes.
    pub fn process_unit_queues(&mut self, tick: u64) -> Result<(), String> {
        let ids: Vec<_> = self.data.unit_order_queues.keys().copied().collect();
        for id in ids {
            let Some(a) = self
                .data
                .tactics
                .agent(id)
                .copied()
                .filter(|a| a.alive() && a.team == 0)
            else {
                self.data.unit_order_queues.remove(&id);
                continue;
            };
            let Some(front) = self
                .data
                .unit_order_queues
                .get(&id)
                .and_then(|q| q.front())
                .cloned()
            else {
                self.data.unit_order_queues.remove(&id);
                continue;
            };
            let more = self.data.unit_order_queues[&id].len() > 1;
            let mut complete = false;
            match front.kind {
                UnitOrderKind::Hold => {
                    if more {
                        complete = true
                    } else {
                        self.data.tactics.stop(id)?;
                    }
                }
                UnitOrderKind::Move { .. } => {
                    complete = front.started_tick.is_some() && reached(&a, &front, &self.data.nav)
                }
                UnitOrderKind::Attack { target, .. } => {
                    let visible = target
                        .and_then(|target| self.data.tactics.agent(target))
                        .filter(|target| self.data.recon.visible(0, target.position()));
                    complete = front.started_tick.is_some()
                        && match visible {
                            Some(target) => !target.alive() || target.team == 0,
                            None => reached(&a, &front, &self.data.nav),
                        };
                }
                UnitOrderKind::Disembark { position } => {
                    let manifest = self
                        .data
                        .tactics
                        .manifests
                        .get(&id)
                        .cloned()
                        .unwrap_or_default();
                    if manifest.is_empty() {
                        complete = true;
                    } else if let Some(placements) =
                        landing_sites(&self.data.tactics, id, position, &self.data.nav)
                    {
                        match self
                            .data
                            .tactics
                            .disembark_group(&placements, &self.data.nav)
                        {
                            Ok(()) => complete = true,
                            Err(_) => {
                                self.data.status =
                                    "Selected transport is waiting for clear landing sites".into()
                            }
                        }
                    }
                }
                UnitOrderKind::Board { transport, .. } => {
                    if a.embarked_on == Some(transport) {
                        complete = true
                    } else if a.embarked_on.is_none() {
                        let near = self.data.tactics.agent(transport).is_some_and(|v| {
                            v.team == 0
                                && v.alive()
                                && v.role == Role::Transport
                                && distance_sq_mm(a.position_mm, v.position_mm) <= 12_000i64.pow(2)
                        });
                        if near {
                            match self.data.tactics.board(id, transport) {
                                Ok(()) => complete = true,
                                Err(_) => {
                                    self.data.status =
                                        "Boarding order is waiting for available transport capacity"
                                            .into()
                                }
                            }
                        }
                    }
                }
            }
            if complete {
                let q = self
                    .data
                    .unit_order_queues
                    .get_mut(&id)
                    .expect("existing queue");
                q.pop_front();
                if q.is_empty() {
                    self.data.unit_order_queues.remove(&id);
                    self.data.tactics.stop(id)?;
                }
            }
        }

        // Group equal active goals to preserve one shared route/destination
        // calculation across large explicit selections.
        let mut groups: BTreeMap<(u8, u32, u32, Option<u64>), Vec<u64>> = BTreeMap::new();
        let ids: Vec<_> = self.data.unit_order_queues.keys().copied().collect();
        for id in ids {
            let Some(mut front) = self
                .data
                .unit_order_queues
                .get(&id)
                .and_then(|q| q.front())
                .cloned()
            else {
                continue;
            };
            let Some(a) = self
                .data
                .tactics
                .agent(id)
                .copied()
                .filter(|a| a.alive() && a.team == 0)
            else {
                continue;
            };
            if matches!(front.kind, UnitOrderKind::Hold) {
                continue;
            }
            if a.embarked_on.is_some() {
                continue;
            }
            let (mode, point, target) = match &mut front.kind {
                UnitOrderKind::Move { position } => (0, *position, None),
                UnitOrderKind::Attack { position, target } => {
                    if let Some(enemy) = target
                        .and_then(|target| self.data.tactics.agent(target))
                        .filter(|target| {
                            target.team != 0
                                && target.alive()
                                && self.data.recon.visible(0, target.position())
                        })
                    {
                        *position = enemy.position();
                    }
                    (1, *position, *target)
                }
                UnitOrderKind::Board {
                    transport,
                    last_known_position,
                } => {
                    if let Some(v) = self.data.tactics.agent(*transport).filter(|v| {
                        v.team == 0 && v.alive() && self.data.recon.visible(0, v.position())
                    }) {
                        *last_known_position = v.position();
                    }
                    (2, *last_known_position, Some(*transport))
                }
                UnitOrderKind::Disembark { position } => (3, *position, None),
                UnitOrderKind::Hold => continue,
            };
            let old_kind = self.data.unit_order_queues[&id][0].kind.clone();
            self.data.unit_order_queues.get_mut(&id).unwrap()[0].kind = front.kind.clone();
            let requested = world_mm(point)?;
            let stale = front.started_tick.is_none()
                || a.requested_goal_mm != Some(requested)
                || a.formation_offset_mm != front.formation_offset_mm
                || a.resolved_topology_revision != self.data.nav.topology_revision
                || a.job.is_some()
                || old_kind != front.kind;
            if stale && (front.started_tick.is_some() || tick % 20 == 0) {
                groups
                    .entry((mode, point[0].to_bits(), point[1].to_bits(), target))
                    .or_default()
                    .push(id);
            }
            if mode == 1 {
                let priority = target.filter(|target| {
                    self.data.tactics.agent(*target).is_some_and(|enemy| {
                        enemy.team != 0
                            && enemy.alive()
                            && self.data.recon.visible(0, enemy.position())
                    })
                });
                if let Some(a) = self.data.tactics.agent_mut(id) {
                    a.commanded_target = priority;
                }
            }
        }
        for ((mode, x, y, target), ids) in groups {
            let point = [f32::from_bits(x), f32::from_bits(y)];
            if self
                .data
                .tactics
                .order_selected_on_grid(0, &ids, point, &self.data.nav, mode == 1)
                .is_err()
            {
                self.data.status = "Selected order is waiting for a usable route".into();
                continue;
            }
            let priority = if mode == 1 {
                target.filter(|target| {
                    self.data.tactics.agent(*target).is_some_and(|enemy| {
                        enemy.team != 0
                            && enemy.alive()
                            && self.data.recon.visible(0, enemy.position())
                    })
                })
            } else {
                None
            };
            for id in ids {
                let goal = self.data.tactics.agent(id).and_then(|a| a.goal_mm);
                let front = self
                    .data
                    .unit_order_queues
                    .get_mut(&id)
                    .and_then(|q| q.front_mut())
                    .ok_or("Unit order disappeared during dispatch")?;
                if front.started_tick.is_none() {
                    front.started_tick = Some(tick);
                }
                front.resolved_goal_mm = goal;
                if let Some(a) = self.data.tactics.agent_mut(id) {
                    a.formation_offset_mm = front.formation_offset_mm;
                    a.commanded_target = priority;
                }
            }
        }
        Ok(())
    }

    pub fn unit_has_manual_order(&self, id: u64) -> bool {
        self.data
            .unit_order_queues
            .get(&id)
            .is_some_and(|q| !q.is_empty())
    }

    pub fn validate_unit_order_queues(&self) -> Result<(), String> {
        for (&id, q) in &self.data.unit_order_queues {
            if q.is_empty()
                || q.len() > MAX_UNIT_QUEUE
                || !self.data.tactics.agent(id).is_some_and(|a| a.team == 0)
            {
                return Err("Invalid saved unit-order queue ownership".into());
            }
            for order in q {
                if order.issued_tick > self.data.clock.tick()
                    || order
                        .started_tick
                        .is_some_and(|t| t < order.issued_tick || t > self.data.clock.tick())
                    || order
                        .formation_offset_mm
                        .iter()
                        .any(|x| x.unsigned_abs() > 2_000_000)
                {
                    return Err("Invalid saved unit-order timing or formation".into());
                }
                match order.kind {
                    UnitOrderKind::Board { transport, .. }
                        if !self
                            .data
                            .tactics
                            .agent(transport)
                            .is_some_and(|a| a.team == 0 && a.role == Role::Transport) =>
                    {
                        return Err("Invalid saved transport order ownership".into());
                    }
                    UnitOrderKind::Attack {
                        target: Some(target),
                        ..
                    } if !self.data.tactics.agent(target).is_some_and(|a| a.team != 0) => {
                        return Err("Invalid saved target identity".into());
                    }
                    UnitOrderKind::Disembark { .. }
                        if self
                            .data
                            .tactics
                            .agent(id)
                            .is_none_or(|a| a.role != Role::Transport) =>
                    {
                        return Err("Invalid saved disembark owner".into());
                    }
                    _ => {}
                }
                let point = match order.kind {
                    UnitOrderKind::Move { position }
                    | UnitOrderKind::Attack { position, .. }
                    | UnitOrderKind::Disembark { position } => Some(position),
                    UnitOrderKind::Board {
                        last_known_position,
                        ..
                    } => Some(last_known_position),
                    UnitOrderKind::Hold => None,
                };
                if point.is_some_and(|p| !valid_position(p, &self.data.nav))
                    || order
                        .resolved_goal_mm
                        .is_some_and(|p| self.data.nav.index_mm(p).is_none())
                {
                    return Err("Saved unit order lies outside map".into());
                }
            }
        }
        Ok(())
    }
}

fn valid_selection(tactics: &Tactics, ids: &[u64]) -> Result<Vec<u64>, String> {
    if ids.is_empty() || ids.len() > tactics.agents.len() {
        return Err("Selection is not available".into());
    }
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    if sorted.windows(2).any(|w| w[0] == w[1])
        || sorted
            .iter()
            .any(|&id| !tactics.agent(id).is_some_and(|a| a.team == 0 && a.alive()))
    {
        return Err("Selection is not available".into());
    }
    Ok(sorted)
}
fn stage_queues(
    tactics: &Tactics,
    current: &Queues,
    ids: &[u64],
    kind: UnitOrderKind,
    append: bool,
    tick: u64,
) -> Result<Queues, String> {
    let valid = valid_selection(tactics, ids)?;
    let width = (valid.len() as f64).sqrt().ceil().max(1.) as i32;
    let mut staged = BTreeMap::new();
    for (slot, id) in valid.into_iter().enumerate() {
        let mut q = if append {
            current.get(&id).cloned().unwrap_or_default()
        } else {
            VecDeque::new()
        };
        if q.len() == 1
            && q.front()
                .is_some_and(|o| matches!(o.kind, UnitOrderKind::Hold))
        {
            q.clear();
        }
        if q.len() >= MAX_UNIT_QUEUE {
            return Err("Selected unit queue is full; maximum 64 orders".into());
        }
        let role = tactics.agent(id).ok_or("Selection is not available")?.role;
        let spacing = if role.movement() == MovementClass::Foot {
            6000
        } else {
            9000
        };
        let offset = if matches!(
            kind,
            UnitOrderKind::Board { .. } | UnitOrderKind::Disembark { .. } | UnitOrderKind::Hold
        ) {
            [0; 2]
        } else {
            [
                (slot as i32 % width) * spacing - (width - 1) * spacing / 2,
                (slot as i32 / width) * spacing - (width - 1) * spacing / 2,
            ]
        };
        q.push_back(PendingUnitOrder {
            kind: kind.clone(),
            issued_tick: tick,
            started_tick: None,
            formation_offset_mm: offset,
            resolved_goal_mm: None,
        });
        staged.insert(id, q);
    }
    Ok(staged)
}
fn landing_sites(
    tactics: &Tactics,
    transport: u64,
    target: [f32; 2],
    grid: &NavGrid,
) -> Option<Vec<(u64, [f32; 2])>> {
    let vessel = tactics
        .agent(transport)
        .filter(|a| a.alive() && a.team == 0 && a.role == Role::Transport)?;
    let manifest = tactics.manifests.get(&transport)?;
    let target = world_mm(target).ok()?;
    let mut placements = Vec::with_capacity(manifest.len());
    for &id in manifest {
        let a = tactics
            .agent(id)
            .filter(|a| a.alive() && a.embarked_on == Some(transport) && a.team == 0)?;
        let mut chosen = None;
        'rings: for ring in 0i32..=4 {
            for y in -ring..=ring {
                for x in -ring..=ring {
                    if ring > 0 && x.abs() != ring && y.abs() != ring {
                        continue;
                    }
                    let p = [target[0] + x * 6000, target[1] + y * 6000];
                    if distance_sq_mm(p, vessel.position_mm) > 24_000i64.pow(2)
                        || !grid
                            .index_mm(p)
                            .is_some_and(|i| grid.passable(i, MovementClass::Foot))
                    {
                        continue;
                    }
                    let occupied = tactics.agents.iter().any(|b| {
                        b.alive()
                            && b.embarked_on.is_none()
                            && b.role.movement() == MovementClass::Foot
                            && distance_sq_mm(p, b.position_mm)
                                < ((a.role.radius_mm() + b.role.radius_mm()) as i64).pow(2)
                    });
                    let reserved = placements.iter().any(|&(other, point): &(u64, [f32; 2])| {
                        let Some(b) = tactics.agent(other) else {
                            return true;
                        };
                        let Ok(other) = world_mm(point) else {
                            return true;
                        };
                        distance_sq_mm(p, other)
                            < ((a.role.radius_mm() + b.role.radius_mm()) as i64).pow(2)
                    });
                    if !occupied && !reserved {
                        chosen = Some([p[0] as f32 * 0.001, p[1] as f32 * 0.001]);
                        break 'rings;
                    }
                }
            }
        }
        placements.push((id, chosen?));
    }
    Some(placements)
}

fn reached(agent: &TacticalAgent, order: &PendingUnitOrder, grid: &NavGrid) -> bool {
    if agent.embarked_on.is_some() {
        return false;
    }
    let Some(base) = order.resolved_goal_mm else {
        return false;
    };
    let mut goal = [
        (base[0] as i64 + order.formation_offset_mm[0] as i64)
            .clamp(0, grid.width as i64 * grid.cell_mm as i64 - 1) as i32,
        (base[1] as i64 + order.formation_offset_mm[1] as i64)
            .clamp(0, grid.height as i64 * grid.cell_mm as i64 - 1) as i32,
    ];
    if !grid
        .index_mm(goal)
        .is_some_and(|i| grid.passable(i, agent.role.movement()))
    {
        goal = base;
    }
    let reach = if agent.role.movement() == MovementClass::Foot {
        1100i64
    } else {
        4500
    };
    distance_sq_mm(agent.position_mm, goal) <= reach * reach
}
fn world_mm(p: [f32; 2]) -> Result<[i32; 2], String> {
    if p.iter()
        .any(|v| !v.is_finite() || *v < 0. || *v > 2_000_000.)
    {
        return Err("Invalid order position".into());
    }
    Ok([
        (p[0] as f64 * 1000.).round() as i32,
        (p[1] as f64 * 1000.).round() as i32,
    ])
}
fn valid_position(p: [f32; 2], grid: &NavGrid) -> bool {
    world_mm(p).is_ok_and(|p| grid.index_mm(p).is_some())
}
fn distance_sq_mm(a: [i32; 2], b: [i32; 2]) -> i64 {
    let dx = a[0] as i64 - b[0] as i64;
    let dy = a[1] as i64 - b[1] as i64;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactics::NavCell;
    fn grid() -> NavGrid {
        NavGrid::new(
            20,
            8,
            16_000,
            vec![
                NavCell {
                    road: true,
                    ..Default::default()
                };
                160
            ],
        )
        .unwrap()
    }
    #[test]
    fn explicit_subset_moves_without_ordering_other_units_and_enemy_selection_is_atomic() {
        let g = grid();
        let mut t = Tactics::new(1);
        let first = t.spawn(0, Role::Soldier, [16., 16.]).unwrap();
        let second = t.spawn(0, Role::Soldier, [16., 32.]).unwrap();
        let enemy = t.spawn(1, Role::Soldier, [250., 16.]).unwrap();
        t.order_selected_on_grid(0, &[first], [100., 16.], &g, false)
            .unwrap();
        assert!(t.agent(first).unwrap().goal_mm.is_some());
        assert_eq!(t.agent(second).unwrap().goal_mm, None);
        assert_eq!(t.agent(enemy).unwrap().goal_mm, None);
        let before = serde_json::to_value(&t).unwrap();
        assert!(
            t.order_selected_on_grid(0, &[first, enemy], [80., 16.], &g, false)
                .is_err()
        );
        assert_eq!(serde_json::to_value(&t).unwrap(), before);
    }
    #[test]
    fn shift_queue_and_hold_are_owned_saved_and_bounded_without_changing_current_goal() {
        let mut t = Tactics::new(2);
        let id = t.spawn(0, Role::Worker, [16., 16.]).unwrap();
        let initial: Queues = BTreeMap::new();
        let first = stage_queues(
            &t,
            &initial,
            &[id],
            UnitOrderKind::Move {
                position: [80., 16.],
            },
            false,
            4,
        )
        .unwrap();
        let second = stage_queues(
            &t,
            &first,
            &[id],
            UnitOrderKind::Move {
                position: [140., 16.],
            },
            true,
            5,
        )
        .unwrap();
        assert_eq!(second[&id].len(), 2);
        assert_eq!(t.agent(id).unwrap().goal_mm, None);
        let saved = serde_json::to_vec(&second).unwrap();
        let restored: Queues = serde_json::from_slice(&saved).unwrap();
        assert_eq!(second, restored);
        let mut full = restored;
        while full[&id].len() < MAX_UNIT_QUEUE {
            full = stage_queues(&t, &full, &[id], UnitOrderKind::Hold, true, 6).unwrap();
        }
        assert!(stage_queues(&t, &full, &[id], UnitOrderKind::Hold, true, 7).is_err());
        assert_eq!(full[&id].len(), MAX_UNIT_QUEUE);
    }
    #[test]
    fn queue_completion_requires_actual_arrival_and_replacement_affects_only_selected_ids() {
        let g = grid();
        let mut t = Tactics::new(3);
        let a = t.spawn(0, Role::Worker, [16., 16.]).unwrap();
        let b = t.spawn(0, Role::Worker, [16., 32.]).unwrap();
        let mut queues = stage_queues(
            &t,
            &BTreeMap::new(),
            &[a, b],
            UnitOrderKind::Move {
                position: [80., 16.],
            },
            false,
            0,
        )
        .unwrap();
        t.order_selected_on_grid(0, &[a, b], [80., 16.], &g, false)
            .unwrap();
        for id in [a, b] {
            let o = queues.get_mut(&id).unwrap().front_mut().unwrap();
            o.resolved_goal_mm = t.agent(id).unwrap().goal_mm;
            o.started_tick = Some(0);
            assert!(!reached(t.agent(id).unwrap(), o, &g));
        }
        for tick in 1..=250 {
            t.step(tick, &g).unwrap();
        }
        assert!(reached(t.agent(a).unwrap(), &queues[&a][0], &g));
        assert!(reached(t.agent(b).unwrap(), &queues[&b][0], &g));
        let staged = stage_queues(&t, &queues, &[a], UnitOrderKind::Hold, false, 250).unwrap();
        queues.extend(staged);
        assert!(matches!(queues[&a][0].kind, UnitOrderKind::Hold));
        assert!(matches!(queues[&b][0].kind, UnitOrderKind::Move { .. }));
        t.validate().unwrap();
    }
}
