//! Stateful ordinary-command workload for the exhaustive native benchmark.
//! Placement/selection is an explicitly omniscient test harness for both factions;
//! production player UI intelligence filters and physical simulation stay intact.
//! This driver never advances clocks, supplies free stock, completes jobs, moves
//! bodies directly or manufactures wildlife. Controller and world save together.
use crate::{
    campaign::Campaign,
    construction::{BuildKind, BuildOrder, JobPhase, Resources, TEAM_BASES},
    logging,
    tactics::{MovementClass, Role},
    ui_interaction::UiCommand,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const BUILD_KINDS: [BuildKind; 19] = [
    BuildKind::Trench,
    BuildKind::Mine,
    BuildKind::Solar,
    BuildKind::Road,
    BuildKind::Depot,
    BuildKind::Mill,
    BuildKind::Barracks,
    BuildKind::Farm,
    BuildKind::Camera,
    BuildKind::Radar,
    BuildKind::Wall,
    BuildKind::Turret,
    BuildKind::Pen,
    BuildKind::Research,
    BuildKind::Foundation,
    BuildKind::Smooth,
    BuildKind::Bridge,
    BuildKind::Dock,
    BuildKind::Dam,
];
const COMMAND_PERIOD: u64 = 20;
const ACTIVITY_PERIOD: u64 = 200;
const MAX_SITE_ATTEMPTS: u32 = 64;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommandCoverage {
    pub attempted: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub last_tick: u64,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BuildExercise {
    pub team: u8,
    pub kind: BuildKind,
    pub job_ids: Vec<u64>,
    pub site_cursor: u32,
    pub placement_attempts: u32,
    pub next_retry_tick: u64,
    pub last_error: Option<String>,
    pub retry_count: u8,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QueueWatch {
    pub issued_tick: u64,
    pub maximum_depth: usize,
    pub started_observed: bool,
    pub completed_observed: bool,
    pub died: bool,
    pub origin: Option<[f32; 2]>,
    pub final_goal: Option<[f32; 2]>,
    pub moved_away_observed: bool,
    pub ended_observed: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Observations {
    pub seasons: BTreeSet<String>,
    pub satellite_active_samples: [u64; 2],
    pub explored_cells: [u32; 2],
    pub video_cells_max: [u32; 2],
    pub radar_cells_max: [u32; 2],
    pub dam_closed_observed: bool,
    pub dam_reopened_observed: bool,
    pub maximum_upstream_liters: u128,
    pub minimum_upstream_liters: Option<u128>,
    pub maximum_surface_water_liters: u128,
    pub eroded_liters: u128,
    pub deposited_liters: u128,
    pub infiltrated_liters: u128,
    pub evaporated_liters: u128,
    pub camera_minimum_battery: Option<u32>,
    pub camera_battery_depleted: bool,
    pub camera_recharged: bool,
    pub transport_loaded_observed: bool,
    pub transport_disembarked_observed: bool,
    pub queued_order_completions: u64,
    pub flooded_road_cells_max: u32,
    pub flooded_road_blocked_cells_max: u32,
    pub bridge_deck_cells_max: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderExercise {
    pub version: u32,
    pub initialized: bool,
    pub bootstrap_attempted: bool,
    pub bootstrap_error: Option<String>,
    pub last_drive_tick: Option<u64>,
    pub next_build_tick: u64,
    pub next_activity_tick: u64,
    pub next_observation_tick: u64,
    pub build_cursor: usize,
    pub activity_cursor: u8,
    pub requests: Vec<BuildExercise>,
    pub commands: BTreeMap<String, CommandCoverage>,
    pub observations: Observations,
    pub queue_watch: BTreeMap<u64, QueueWatch>,
    pub scout: Option<u64>,
    pub scout_sent_out: bool,
    pub scout_return_requested: bool,
    pub scout_recruit_attempts: u32,
    pub boats: [Option<u64>; 2],
    pub transports: [Option<u64>; 2],
    pub naval_spawn_cursor: u8,
    pub naval_action_cursor: u8,
    pub naval_rendezvous: Option<[f32; 2]>,
    pub transport_board_requested: bool,
    pub transport_disembark_requested: bool,
    pub boarded_passengers: BTreeSet<u64>,
    pub disembark_target: Option<[f32; 2]>,
    pub last_context_tick: u64,
    pub artillery_attempts: u32,
    pub initial_spawned: Option<[u64; 2]>,
    pub initial_deposits: Option<u64>,
    pub initial_ecological_damage: Option<[u64; 2]>,
    #[serde(skip)]
    reachable: Reachability,
}
impl Default for OrderExercise {
    fn default() -> Self {
        Self {
            version: 1,
            initialized: false,
            bootstrap_attempted: false,
            bootstrap_error: None,
            last_drive_tick: None,
            next_build_tick: 0,
            next_activity_tick: 40,
            next_observation_tick: 0,
            build_cursor: 0,
            activity_cursor: 0,
            requests: BUILD_KINDS
                .iter()
                .flat_map(|&kind| {
                    (0..2).map(move |team| BuildExercise {
                        team,
                        kind,
                        job_ids: Vec::new(),
                        site_cursor: 0,
                        placement_attempts: 0,
                        next_retry_tick: 0,
                        last_error: None,
                        retry_count: 0,
                    })
                })
                .collect(),
            commands: BTreeMap::new(),
            observations: Observations::default(),
            queue_watch: BTreeMap::new(),
            scout: None,
            scout_sent_out: false,
            scout_return_requested: false,
            scout_recruit_attempts: 0,
            boats: [None; 2],
            transports: [None; 2],
            naval_spawn_cursor: 0,
            naval_action_cursor: 0,
            naval_rendezvous: None,
            transport_board_requested: false,
            transport_disembark_requested: false,
            boarded_passengers: BTreeSet::new(),
            disembark_target: None,
            last_context_tick: 0,
            artillery_attempts: 0,
            initial_spawned: None,
            initial_deposits: None,
            initial_ecological_damage: None,
            reachable: Reachability::default(),
        }
    }
}
#[derive(Clone, Debug, Default)]
struct Reachability {
    revision: Option<u64>,
    width: usize,
    height: usize,
    labels: Vec<u32>,
    worker_components: [BTreeSet<u32>; 2],
    cells: [Vec<u8>; 2],
}
impl Reachability {
    fn ensure(&mut self, game: &Campaign) -> Result<(), String> {
        let changed = self.revision != Some(game.nav.topology_revision)
            || self.width != game.nav.width
            || self.height != game.nav.height;
        if changed {
            self.width = game.nav.width;
            self.height = game.nav.height;
            self.labels.clear();
            self.labels.resize(game.nav.cells.len(), u32::MAX);
            let mut label = 0u32;
            let mut queue = VecDeque::new();
            for seed in 0..game.nav.cells.len() {
                if self.labels[seed] != u32::MAX || !game.nav.passable(seed, MovementClass::Foot) {
                    continue;
                }
                self.labels[seed] = label;
                queue.push_back(seed);
                while let Some(i) = queue.pop_front() {
                    let x = i % self.width;
                    let y = i / self.width;
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let xx = x as i32 + dx;
                            let yy = y as i32 + dy;
                            if xx < 0
                                || yy < 0
                                || xx >= self.width as i32
                                || yy >= self.height as i32
                            {
                                continue;
                            }
                            let next = yy as usize * self.width + xx as usize;
                            if self.labels[next] == u32::MAX
                                && game.nav.adjacent_reachable(i, next, MovementClass::Foot)
                            {
                                self.labels[next] = label;
                                queue.push_back(next);
                            }
                        }
                    }
                }
                label = label
                    .checked_add(1)
                    .ok_or("Benchmark navigation component identity overflow")?;
            }
        }
        // Moving inside the same component does not invalidate this coarse cache.
        // Boarding/disembarking or the last worker's death in a component does.
        let mut components: [BTreeSet<u32>; 2] = [BTreeSet::new(), BTreeSet::new()];
        for a in game
            .tactics
            .agents
            .iter()
            .filter(|a| a.alive() && a.role == Role::Worker && a.embarked_on.is_none())
        {
            if let Some(i) = game.nav.index_mm(a.position_mm) {
                if let Some(&label) = self.labels.get(i).filter(|&&n| n != u32::MAX) {
                    components[a.team as usize].insert(label);
                }
            }
        }
        if changed || components != self.worker_components {
            for team in 0..2 {
                self.cells[team].clear();
                self.cells[team].extend(
                    self.labels
                        .iter()
                        .map(|n| u8::from(components[team].contains(n))),
                );
            }
            self.worker_components = components;
        }
        self.revision = Some(game.nav.topology_revision);
        Ok(())
    }
    fn at(&self, game: &Campaign, team: u8, p: [f32; 2]) -> bool {
        game.nav
            .index(p)
            .is_some_and(|i| self.cells[team as usize].get(i) == Some(&1))
    }
}

impl OrderExercise {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn drive(&mut self, game: &mut Campaign) -> Result<(), String> {
        let tick = game.processed_tick;
        if self.last_drive_tick.is_some_and(|last| tick < last) {
            return Err(
                "OrderExercise checkpoint is ahead of its campaign; restore both together".into(),
            );
        }
        if self.last_drive_tick == Some(tick) {
            return Ok(());
        }
        self.last_drive_tick = Some(tick);
        if !self.initialized {
            self.initial_spawned = Some(game.tactics.totals.spawned);
            self.initial_deposits =
                Some(game.construction.deposits.iter().map(|d| d.remaining).sum());
            self.initial_ecological_damage = Some(game.ecological_damage);
            self.initialized = true;
        }
        if !self.bootstrap_attempted {
            self.bootstrap_attempted = true;
            if game.construction.jobs.is_empty() && game.construction.buildings.is_empty() {
                let result = game.automation();
                // Disable only the separate visual fixture's2400tick stop. The
                // same production precedent is used by the strict CPU scenario.
                game.automated = false;
                if let Err(e) = &result {
                    self.bootstrap_error = Some(e.clone());
                }
                self.record("ordinary_automation_bootstrap", tick, result);
            } else {
                self.record("existing_paid_bootstrap_adopted", tick, Ok(()));
            }
            self.adopt_jobs(game);
            self.next_build_tick = tick.saturating_add(COMMAND_PERIOD);
            return Ok(());
        }
        if tick >= self.next_observation_tick {
            self.observe(game);
            self.next_observation_tick = tick.saturating_add(200);
        }
        if game.winner.is_some() {
            return Ok(());
        }
        // At most two construction commands (enqueue plus immediate cancellation
        // of an unreachable reserved workpoint), plus at most two activity
        // commands per invocation. The six-order bootstrap is separately named.
        // Deadlines never dump missed commands when a caller skips ticks.
        if tick >= self.next_build_tick {
            self.build_one(game)?;
            self.next_build_tick = tick.saturating_add(COMMAND_PERIOD);
        }
        if tick >= self.next_activity_tick {
            match self.activity_cursor % 6 {
                0 => self.camera_action(game),
                1 => self.context_action(game),
                2 => self.naval_spawn(game),
                3 => self.naval_action(game),
                4 => self.artillery_action(game),
                _ => self.combat_action(game),
            };
            self.activity_cursor = self.activity_cursor.wrapping_add(1);
            self.next_activity_tick = tick.saturating_add(ACTIVITY_PERIOD);
        }
        Ok(())
    }
    fn record(&mut self, key: &str, tick: u64, result: Result<(), String>) {
        let c = self.commands.entry(key.into()).or_default();
        c.attempted = c
            .attempted
            .checked_add(1)
            .expect("bounded benchmark attempt counter");
        c.last_tick = tick;
        match result {
            Ok(()) => {
                c.accepted = c
                    .accepted
                    .checked_add(1)
                    .expect("bounded benchmark acceptance counter")
            }
            Err(e) => {
                c.rejected = c
                    .rejected
                    .checked_add(1)
                    .expect("bounded benchmark rejection counter");
                c.last_error = Some(e);
            }
        }
        logging::log("BENCHMARK_ORDER",&serde_json::json!({"key":key,"tick":tick,"coverage":c,"scope":"ordinary_commands_from_declared_omniscient_test_harness;no_free_stocks_or_actor_teleport"}).to_string());
    }
    fn adopt_jobs(&mut self, game: &Campaign) {
        for r in &mut self.requests {
            if r.job_ids.is_empty() {
                if let Some(j) = game.construction.jobs.iter().find(|j| {
                    j.order.team == r.team
                        && j.order.kind == r.kind
                        && j.phase != JobPhase::Cancelled
                }) {
                    r.job_ids.push(j.id);
                }
            }
        }
    }
    fn build_one(&mut self, game: &mut Campaign) -> Result<(), String> {
        self.adopt_jobs(game);
        let tick = game.processed_tick;
        for _ in 0..self.requests.len() {
            let i = self.build_cursor % self.requests.len();
            self.build_cursor = (self.build_cursor + 1) % self.requests.len();
            let r = &self.requests[i];
            if tick < r.next_retry_tick || r.placement_attempts >= MAX_SITE_ATTEMPTS {
                continue;
            }
            if let Some(&id) = r.job_ids.last() {
                if let Some(job) = game.construction.jobs.iter().find(|j| j.id == id) {
                    if job.phase == JobPhase::Complete {
                        continue;
                    }
                    if job.phase != JobPhase::Blocked && job.phase != JobPhase::Cancelled {
                        continue;
                    }
                    if job.phase == JobPhase::Blocked && r.retry_count < 3 {
                        let result = game.construction.cancel(id, r.team);
                        self.record(
                            &format!("cancel_blocked/{}/{}", r.team, r.kind.name()),
                            tick,
                            result,
                        );
                        self.requests[i].retry_count += 1;
                        self.requests[i].next_retry_tick = tick.saturating_add(200);
                        return Ok(());
                    }
                    if job.phase == JobPhase::Blocked {
                        continue;
                    }
                }
            }
            self.reachable.ensure(game)?;
            let team = r.team;
            let kind = r.kind;
            let attempt = r.site_cursor;
            let order = self.placement(game, team, kind, attempt);
            self.requests[i].site_cursor = self.requests[i].site_cursor.saturating_add(1);
            self.requests[i].placement_attempts =
                self.requests[i].placement_attempts.saturating_add(1);
            let result =
                order.and_then(|order| game.data.construction.enqueue(order, &game.terrain));
            let key = format!("build/{}/{}", team, kind.name());
            match result {
                Ok(id) => {
                    self.requests[i].job_ids.push(id);
                    self.requests[i].last_error = None;
                    self.record(&key, tick, Ok(()));
                    // Construction chooses and persists the actual exterior
                    // workpoint. A different exterior point being reachable
                    // does not establish that this chosen one is reachable.
                    // Cancel before any worker receives the order so the real
                    // reservation returns through the ordinary ledger once.
                    let work_position = game
                        .construction
                        .jobs
                        .iter()
                        .find(|job| job.id == id)
                        .ok_or("Accepted benchmark order has no persisted job")?
                        .work_position;
                    if !self.reachable.at(game, team, work_position) {
                        let reason = "Chosen production workpoint is outside every actual faction worker's connected land";
                        self.requests[i].last_error = Some(reason.into());
                        self.requests[i].next_retry_tick = tick.saturating_add(200);
                        let cancelled = game.construction.cancel(id, team);
                        let success = cancelled.is_ok();
                        self.record(
                            &format!("cancel_unreachable_workpoint/{team}/{}", kind.name()),
                            tick,
                            cancelled,
                        );
                        if !success {
                            return Err("Unable to cancel an unreachable benchmark order through its real reservation ledger".into());
                        }
                    }
                }
                Err(e) => {
                    self.requests[i].last_error = Some(e.clone());
                    self.requests[i].next_retry_tick = tick.saturating_add(200);
                    self.record(&key, tick, Err(e));
                }
            }
            return Ok(());
        }
        Ok(())
    }
    fn placement(
        &self,
        game: &Campaign,
        team: u8,
        kind: BuildKind,
        attempt: u32,
    ) -> Result<BuildOrder, String> {
        if kind == BuildKind::Mine {
            let mut candidates = game
                .construction
                .deposits
                .iter()
                .filter(|d| d.remaining > 0)
                .filter(|d| {
                    self.check_access(game, BuildOrder::at(team, kind, d.position))
                        .is_ok()
                })
                .collect::<Vec<_>>();
            candidates.sort_by(|a, b| {
                distance(a.position, TEAM_BASES[team as usize])
                    .total_cmp(&distance(b.position, TEAM_BASES[team as usize]))
            });
            let d = candidates
                .get(attempt as usize % candidates.len().max(1))
                .ok_or("No finite ore deposit has reachable exterior access")?;
            return Ok(BuildOrder::at(team, kind, d.position));
        }
        if kind == BuildKind::Dock {
            return self.shore_order(game, team, kind, attempt);
        }
        if kind == BuildKind::Bridge {
            return self.bridge_order(game, team, attempt);
        }
        if kind == BuildKind::Dam {
            let order = BuildOrder::at(
                team,
                kind,
                [2384. + if team == 0 { -32. } else { 32. }, 964.],
            );
            self.check_access(game, order)?;
            return Ok(order);
        }
        let base = TEAM_BASES[team as usize];
        let index = BUILD_KINDS.iter().position(|&k| k == kind).unwrap_or(0) as u32;
        let slot = index * 7 + attempt * 5;
        let dirs = [
            [-1., 0.],
            [1., 0.],
            [0., -1.],
            [0., 1.],
            [-1., -1.],
            [1., -1.],
            [-1., 1.],
            [1., 1.],
        ];
        let d = dirs[(slot % 8) as usize];
        let radius = 96. + ((slot / 8) % 5) as f32 * 80.;
        let p = [
            (base[0] + d[0] * radius).clamp(40., 4760.),
            (base[1] + d[1] * radius).clamp(40., 3160.),
        ];
        let point = game
            .nav
            .nearest_valid_goal(p, MovementClass::Foot, 3)
            .ok_or("No nearby dry construction site")?;
        if !self.reachable.at(game, team, point) {
            return Err("Candidate site is not connected to any actual faction worker".into());
        }
        let end = if matches!(kind, BuildKind::Road | BuildKind::Trench) {
            [point[0] + if team == 0 { 64. } else { -64. }, point[1]]
        } else {
            point
        };
        let order = BuildOrder {
            team,
            kind,
            position: point,
            end,
        };
        self.check_access(game, order)?;
        Ok(order)
    }
    fn check_access(&self, game: &Campaign, order: BuildOrder) -> Result<(), String> {
        if !order.kind.solid() && !matches!(order.kind, BuildKind::Bridge | BuildKind::Dock) {
            if self.reachable.at(game, order.team, order.position) {
                return Ok(());
            }
            return Err("Terrain worksite has no worker route".into());
        }
        let p = order.position;
        let possible = [
            [-48., 0.],
            [48., 0.],
            [0., -48.],
            [0., 48.],
            [-48., -48.],
            [-48., 48.],
            [48., -48.],
            [48., 48.],
        ]
        .iter()
        .any(|d| {
            self.reachable
                .at(game, order.team, [p[0] + d[0], p[1] + d[1]])
        });
        if possible {
            Ok(())
        } else {
            Err("No exterior worker access in the faction's real connected land".into())
        }
    }
    fn shore_order(
        &self,
        game: &Campaign,
        team: u8,
        kind: BuildKind,
        attempt: u32,
    ) -> Result<BuildOrder, String> {
        let mut candidates = Vec::new();
        for i in 0..game.nav.cells.len() {
            if !game.nav.passable(i, Role::Transport.movement()) {
                continue;
            }
            let x = i % game.nav.width;
            let y = i / game.nav.width;
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let xx = x as i32 + dx;
                let yy = y as i32 + dy;
                if xx < 0 || yy < 0 || xx >= game.nav.width as i32 || yy >= game.nav.height as i32 {
                    continue;
                }
                let land = yy as usize * game.nav.width + xx as usize;
                if !game.nav.passable(land, MovementClass::Foot) {
                    continue;
                }
                let p = game.nav.center_mm(land).map(|n| n as f32 * 0.001);
                if self.reachable.at(game, team, p) {
                    candidates.push(p);
                }
            }
        }
        candidates.sort_by(|a, b| {
            distance(*a, TEAM_BASES[team as usize])
                .total_cmp(&distance(*b, TEAM_BASES[team as usize]))
        });
        candidates.dedup();
        let point = candidates
            .get(attempt as usize % candidates.len().max(1))
            .copied()
            .ok_or("No physically reachable shoreline with navigable vessel depth")?;
        let order = BuildOrder::at(team, kind, point);
        self.check_access(game, order)?;
        Ok(order)
    }
    fn bridge_order(&self, game: &Campaign, team: u8, attempt: u32) -> Result<BuildOrder, String> {
        let row = (964. / 16.) as usize + ((attempt as usize % 17) * 3);
        let y = row.min(game.nav.height - 1);
        for x in 1..game.nav.width - 1 {
            let wet = y * game.nav.width + x;
            if !game.nav.passable(wet, Role::Transport.movement()) {
                continue;
            }
            let left = (1..=16).filter_map(|d| x.checked_sub(d)).find(|&xx| {
                game.nav
                    .passable(y * game.nav.width + xx, MovementClass::Foot)
            });
            let right = (x + 1..=(x + 16).min(game.nav.width - 1)).find(|&xx| {
                game.nav
                    .passable(y * game.nav.width + xx, MovementClass::Foot)
            });
            if let (Some(a), Some(b)) = (left, right) {
                let pa = game
                    .nav
                    .center_mm(y * game.nav.width + a)
                    .map(|n| n as f32 * 0.001);
                let pb = game
                    .nav
                    .center_mm(y * game.nav.width + b)
                    .map(|n| n as f32 * 0.001);
                let (position, end) = if self.reachable.at(game, team, pa) {
                    (pa, pb)
                } else if self.reachable.at(game, team, pb) {
                    (pb, pa)
                } else {
                    continue;
                };
                if distance(position, end) > 512. {
                    continue;
                }
                let order = BuildOrder {
                    team,
                    kind: BuildKind::Bridge,
                    position,
                    end,
                };
                self.check_access(game, order)?;
                return Ok(order);
            }
        }
        Err("No reachable two-bank bridge crossing on this surveyed row".into())
    }
    fn camera_action(&mut self, game: &mut Campaign) {
        let tick = game.processed_tick;
        if self.scout.is_none() {
            if self.scout_recruit_attempts >= 8 {
                return;
            }
            self.scout_recruit_attempts += 1;
            let previous = game.tactics.agents.len();
            let result = game.apply_ui_command(&UiCommand::RecruitCamera);
            if result.is_ok() {
                self.scout = game
                    .tactics
                    .agents
                    .iter()
                    .skip(previous)
                    .find(|a| a.team == 0 && a.role == Role::Scout)
                    .map(|a| a.id);
            }
            self.record("camera_trooper_paid_recruitment", tick, result);
            return;
        }
        let id = self.scout.unwrap();
        let Some(a) = game.tactics.agent(id).copied().filter(|a| a.alive()) else {
            return;
        };
        if !self.scout_sent_out {
            let goal = visible_goal(game, a.position(), 110., 320.);
            let result = goal
                .ok_or_else(|| {
                    "No currently monitored dry outpost beyond the100m recharge zone".to_string()
                })
                .and_then(|position| {
                    game.apply_ui_command(&UiCommand::MoveSelected {
                        ids: vec![id],
                        position,
                        queue: false,
                    })
                });
            if result.is_ok() {
                self.scout_sent_out = true;
            }
            self.record("camera_trooper_actual_outbound_order", tick, result);
        } else if self.observations.camera_battery_depleted && !self.scout_return_requested {
            let result = game.apply_ui_command(&UiCommand::MoveSelected {
                ids: vec![id],
                position: TEAM_BASES[0],
                queue: false,
            });
            if result.is_ok() {
                self.scout_return_requested = true;
            }
            self.record("camera_trooper_actual_recharge_return", tick, result);
        }
    }
    fn context_action(&mut self, game: &mut Campaign) {
        let tick = game.processed_tick;
        if tick < self.last_context_tick.saturating_add(1200) && self.last_context_tick > 0 {
            return;
        }
        let ids = game
            .tactics
            .agents
            .iter()
            .filter(|a| {
                a.team == 0
                    && a.alive()
                    && a.role == Role::Soldier
                    && a.embarked_on.is_none()
                    && game.recon.visible(0, a.position())
                    && !game.unit_has_manual_order(a.id)
            })
            .take(4)
            .map(|a| a.id)
            .collect::<Vec<_>>();
        let Some(first) = ids.first().and_then(|&id| game.tactics.agent(id)).copied() else {
            self.record(
                "selected_unit_queue",
                tick,
                Err("No available observed soldiers for the ordinary selection exercise".into()),
            );
            return;
        };
        let start = first.position();
        let Some(goal) = visible_goal(game, start, 32., 128.) else {
            self.record(
                "selected_unit_queue",
                tick,
                Err("No currently observed passable waypoint for selected units".into()),
            );
            return;
        };
        let result = game.apply_ui_command(&UiCommand::MoveSelected {
            ids: ids.clone(),
            position: goal,
            queue: false,
        });
        let moved = result.is_ok();
        self.record("selected_move_command", tick, result);
        if moved {
            let result = game.apply_ui_command(&UiCommand::AttackSelected {
                ids: ids.clone(),
                position: start,
                target: None,
                queue: true,
            });
            self.record("shift_queued_attack_move", tick, result);
            for id in ids {
                let depth = game.unit_order_queues.get(&id).map_or(0, |q| q.len());
                self.queue_watch.insert(
                    id,
                    QueueWatch {
                        issued_tick: tick,
                        maximum_depth: depth,
                        origin: game.tactics.agent(id).map(|a| a.position()),
                        final_goal: Some(start),
                        ..Default::default()
                    },
                );
            }
        }
        self.last_context_tick = tick;
    }
    fn naval_spawn(&mut self, game: &mut Campaign) {
        let tick = game.processed_tick;
        let slot = self.naval_spawn_cursor % 4;
        self.naval_spawn_cursor = self.naval_spawn_cursor.wrapping_add(1);
        let team = slot / 2;
        let role = if slot % 2 == 0 {
            Role::Boat
        } else {
            Role::Transport
        };
        let previous = if role == Role::Boat {
            self.boats[team as usize]
        } else {
            self.transports[team as usize]
        };
        if previous.is_some() {
            return;
        }
        let dock = game
            .construction
            .buildings
            .iter()
            .find(|b| b.team == team && b.kind == BuildKind::Dock && b.complete && b.health > 0)
            .cloned();
        let result = (|| -> Result<u64, String> {
            let dock =
                dock.ok_or("No completed dock; vessels cannot launch from an invented facility")?;
            let launch = game
                .nav
                .cells
                .iter()
                .enumerate()
                .filter(|(i, _)| game.nav.passable(*i, role.movement()))
                .map(|(i, _)| game.nav.center_mm(i).map(|n| n as f32 * 0.001))
                .filter(|p| distance(*p, dock.position) < 120.)
                .min_by(|a, b| distance(*a, dock.position).total_cmp(&distance(*b, dock.position)))
                .ok_or("No suitable draft/overhead clearance near this actual dock")?;
            let before = game.tactics.agents.len();
            if team == 0 {
                game.apply_ui_command(&UiCommand::UseTool {
                    tool: if role == Role::Boat { 25 } else { 26 },
                    position: launch,
                })?;
            } else {
                // Faction1 has no player UI. Use the same ordinary paid launch
                // transaction as Campaign::click, labelled as harness control.
                let cost = Resources::new(500, 200, 20, 30);
                game.construction.stores[team as usize].checked_sub(cost)?;
                let id = game
                    .data
                    .tactics
                    .spawn_formation(team, role, launch, 1, &game.data.nav)?
                    .into_iter()
                    .next()
                    .ok_or("Vessel launch produced no identity")?;
                game.construction.consume(team, cost)?;
                return Ok(id);
            }
            game.tactics
                .agents
                .iter()
                .skip(before)
                .find(|a| a.team == team && a.role == role)
                .map(|a| a.id)
                .ok_or_else(|| "Paid vessel command returned without an actual vessel".to_owned())
        })();
        match result {
            Ok(id) => {
                if role == Role::Boat {
                    self.boats[team as usize] = Some(id);
                } else {
                    self.transports[team as usize] = Some(id);
                }
                self.record(&format!("paid_naval_launch/{team}/{role:?}"), tick, Ok(()));
            }
            Err(e) => self.record(&format!("paid_naval_launch/{team}/{role:?}"), tick, Err(e)),
        }
    }
    fn naval_action(&mut self, game: &mut Campaign) {
        let tick = game.processed_tick;
        if let Some(vessel) = self.transports[0]
            .and_then(|id| game.tactics.agent(id))
            .copied()
            .filter(|a| a.alive())
        {
            let manifest = game
                .tactics
                .manifests
                .get(&vessel.id)
                .cloned()
                .unwrap_or_default();
            if !manifest.is_empty() {
                self.observations.transport_loaded_observed = true;
                self.boarded_passengers.extend(manifest.iter().copied());
            }
            if !self.transport_board_requested {
                let bank = game
                    .nav
                    .nearest_valid_goal(vessel.position(), MovementClass::Foot, 4);
                let Some(bank) = bank else {
                    self.record(
                        "transport_rendezvous",
                        tick,
                        Err("No real dry boarding bank within reach".into()),
                    );
                    return;
                };
                let ids = game
                    .tactics
                    .agents
                    .iter()
                    .filter(|a| {
                        a.team == 0
                            && a.alive()
                            && a.role == Role::Soldier
                            && a.embarked_on.is_none()
                            && game.recon.visible(0, a.position())
                            && !game.unit_has_manual_order(a.id)
                    })
                    .take(4)
                    .map(|a| a.id)
                    .collect::<Vec<_>>();
                if ids.is_empty() {
                    self.record(
                        "transport_rendezvous",
                        tick,
                        Err(
                            "No observed soldiers available to walk to the actual transport".into(),
                        ),
                    );
                    return;
                }
                let result = game.apply_ui_command(&UiCommand::MoveSelected {
                    ids: ids.clone(),
                    position: bank,
                    queue: false,
                });
                let success = result.is_ok();
                self.record("transport_rendezvous", tick, result);
                if success {
                    let result = game.apply_ui_command(&UiCommand::BoardSelected {
                        ids,
                        transport: vessel.id,
                        queue: true,
                    });
                    if result.is_ok() {
                        self.transport_board_requested = true;
                        self.naval_rendezvous = Some(bank);
                    }
                    self.record("queued_actual_transport_boarding", tick, result);
                }
                return;
            }
            if self.observations.transport_loaded_observed && !self.transport_disembark_requested {
                let bank = self.naval_rendezvous.and_then(|p| {
                    game.nav
                        .nearest_valid_goal([p[0], p[1] + 64.], MovementClass::Foot, 4)
                });
                let result = bank
                    .ok_or_else(|| "No clear real disembarkation bank".to_string())
                    .and_then(|position| {
                        game.apply_ui_command(&UiCommand::DisembarkSelected {
                            transports: vec![vessel.id],
                            position,
                            queue: true,
                        })
                    });
                if result.is_ok() {
                    self.transport_disembark_requested = true;
                    self.disembark_target = bank;
                }
                self.record("queued_actual_transport_disembarkation", tick, result);
            }
            if self.transport_disembark_requested
                && self.observations.transport_loaded_observed
                && manifest.is_empty()
            {
                self.observations.transport_disembarked_observed =
                    self.boarded_passengers.iter().any(|&id| {
                        game.tactics.agent(id).is_some_and(|a| {
                            a.alive()
                                && a.embarked_on.is_none()
                                && self
                                    .disembark_target
                                    .is_some_and(|p| distance(a.position(), p) <= 32.)
                        })
                    });
            }
        }
        // Actual naval movement/attack orders, never a visual-only vessel path.
        for team in [self.naval_action_cursor % 2] {
            if let Some(id) = self.boats[team as usize] {
                if let Some(a) = game.tactics.agent(id).copied().filter(|a| a.alive()) {
                    let point = game.nav.nearest_valid_goal(
                        [
                            a.position()[0],
                            (a.position()[1]
                                + if self.naval_action_cursor % 2 == 0 {
                                    160.
                                } else {
                                    -160.
                                })
                            .clamp(16., 3184.),
                        ],
                        a.role.movement(),
                        8,
                    );
                    if let Some(goal) = point {
                        let result = game
                            .data
                            .tactics
                            .order_selected_on_grid(team, &[id], goal, &game.data.nav, true)
                            .map(|_| ());
                        self.record(&format!("actual_vessel_movement/{team}"), tick, result);
                    }
                }
            }
        }
        self.naval_action_cursor = self.naval_action_cursor.wrapping_add(1);
    }
    fn artillery_action(&mut self, game: &mut Campaign) {
        if self.artillery_attempts >= 8 {
            return;
        }
        let tick = game.processed_tick;
        let team = (self.artillery_attempts % 2) as u8;
        let base = TEAM_BASES[team as usize];
        let point = [
            base[0] + if team == 0 { 200. } else { -200. },
            base[1] + 240.,
        ];
        if !game.recon.visible(team, point) {
            self.record(
                "paid_crater_and_attributed_damage",
                tick,
                Err("Strike area lacks required current video; no blind free crater".into()),
            );
            return;
        }
        self.artillery_attempts += 1;
        let result = game.explode(point, team);
        self.record("paid_crater_and_attributed_damage", tick, result);
    }
    fn combat_action(&mut self, game: &mut Campaign) {
        let tick = game.processed_tick;
        for team in 0..2u8 {
            let target = game
                .construction
                .buildings
                .iter()
                .find(|b| b.team != team && b.health > 0 && b.kind == BuildKind::Dam)
                .map(|b| b.position)
                .unwrap_or([2200., 1580.]);
            let ids = game
                .tactics
                .agents
                .iter()
                .filter(|a| {
                    a.team == team
                        && a.alive()
                        && a.role == Role::Soldier
                        && a.embarked_on.is_none()
                        && (team == 1 || !game.unit_has_manual_order(a.id))
                })
                .take(8)
                .map(|a| a.id)
                .collect::<Vec<_>>();
            if ids.is_empty() {
                continue;
            }
            let result = game
                .data
                .tactics
                .order_selected_on_grid(team, &ids, target, &game.data.nav, true)
                .map(|_| ());
            self.record(&format!("ordinary_attack_order/{team}"), tick, result);
        }
    }
    fn observe(&mut self, game: &Campaign) {
        let tick = game.processed_tick;
        self.observations.seasons.insert(
            crate::climate::SeasonState::at_tick(tick, game.seed, 0)
                .season_name()
                .into(),
        );
        for team in 0..2u8 {
            if game.recon.satellite_circle(tick, team).is_some() {
                self.observations.satellite_active_samples[team as usize] += 1;
            }
            let mask = game.recon.masks(team);
            let explored = mask
                .iter()
                .filter(|&&m| m & crate::campaign_support::EXPLORED != 0)
                .count() as u32;
            let video = mask
                .iter()
                .filter(|&&m| m & crate::campaign_support::VISIBLE != 0)
                .count() as u32;
            let radar = mask
                .iter()
                .filter(|&&m| m & crate::campaign_support::RADAR_COVERAGE != 0)
                .count() as u32;
            self.observations.explored_cells[team as usize] = explored;
            self.observations.video_cells_max[team as usize] =
                self.observations.video_cells_max[team as usize].max(video);
            self.observations.radar_cells_max[team as usize] =
                self.observations.radar_cells_max[team as usize].max(radar);
        }
        if game.river.dam_closed {
            self.observations.dam_closed_observed = true;
        } else if self.observations.dam_closed_observed {
            self.observations.dam_reopened_observed = true;
        }
        let upstream = game.river.water.cells()[..81 * crate::river::WIDTH]
            .iter()
            .map(|c| c.volume_liters as u128)
            .sum::<u128>();
        self.observations.maximum_upstream_liters =
            self.observations.maximum_upstream_liters.max(upstream);
        self.observations.minimum_upstream_liters = Some(
            self.observations
                .minimum_upstream_liters
                .map_or(upstream, |n| n.min(upstream)),
        );
        self.observations.maximum_surface_water_liters = self
            .observations
            .maximum_surface_water_liters
            .max(game.river.water.totals().stored_liters);
        let g = game.river.ground.totals();
        self.observations.eroded_liters = g.cumulative_eroded_liters;
        self.observations.deposited_liters = g.cumulative_deposited_liters;
        self.observations.infiltrated_liters = g.infiltrated_liters;
        self.observations.evaporated_liters = g.evaporated_liters;
        let mut flooded_roads = 0;
        let mut blocked_flooded_roads = 0;
        let mut bridge_decks = 0;
        for (i, cell) in game.nav.cells.iter().enumerate() {
            if cell.road && cell.effective_depth_mm() > 0 {
                flooded_roads += 1;
                if !game.nav.passable(i, MovementClass::Foot) {
                    blocked_flooded_roads += 1;
                }
            }
            bridge_decks += u32::from(cell.bridge_deck_mm.is_some());
        }
        self.observations.flooded_road_cells_max =
            self.observations.flooded_road_cells_max.max(flooded_roads);
        self.observations.flooded_road_blocked_cells_max = self
            .observations
            .flooded_road_blocked_cells_max
            .max(blocked_flooded_roads);
        self.observations.bridge_deck_cells_max =
            self.observations.bridge_deck_cells_max.max(bridge_decks);
        if let Some(id) = self.scout {
            if let Some(b) = game.recon.battery_status(0, id + 2_000_000_000) {
                let old = self.observations.camera_minimum_battery;
                self.observations.camera_minimum_battery =
                    Some(old.map_or(b.remaining_ticks, |n| n.min(b.remaining_ticks)));
                if b.remaining_ticks == 0 {
                    self.observations.camera_battery_depleted = true;
                }
                if self.observations.camera_battery_depleted
                    && b.remaining_ticks > 0
                    && game
                        .tactics
                        .agent(id)
                        .is_some_and(|a| distance(a.position(), TEAM_BASES[0]) < 100.)
                {
                    self.observations.camera_recharged = true;
                }
            }
        }
        for (&id, w) in &mut self.queue_watch {
            let Some(actor) = game.tactics.agent(id).filter(|a| a.alive()) else {
                w.died = true;
                continue;
            };
            w.moved_away_observed |= w
                .origin
                .is_some_and(|p| distance(actor.position(), p) > 12.);
            if let Some(q) = game.unit_order_queues.get(&id) {
                w.maximum_depth = w.maximum_depth.max(q.len());
                w.started_observed |= q.front().is_some_and(|o| o.started_tick.is_some());
            } else {
                w.ended_observed = true;
                if w.started_observed
                    && w.moved_away_observed
                    && !w.completed_observed
                    && w.final_goal
                        .is_some_and(|p| distance(actor.position(), p) <= 20.)
                {
                    w.completed_observed = true;
                    self.observations.queued_order_completions += 1;
                }
            }
        }
    }
    pub fn report(&self, game: &Campaign) -> serde_json::Value {
        let builds = self.requests.iter().map(|r| {
            let jobs = r.job_ids.iter().filter_map(|id| game.construction.jobs.iter().find(|j| j.id == *id)).collect::<Vec<_>>();
            serde_json::json!({
                "team": r.team,
                "kind": r.kind.name(),
                "placement_attempts": r.placement_attempts,
                "accepted_job_ids": r.job_ids,
                "completed": jobs.iter().filter(|j| j.phase == JobPhase::Complete).count(),
                "waiting": jobs.iter().filter(|j| matches!(j.phase, JobPhase::Pickup | JobPhase::Hauling | JobPhase::Working)).count(),
                "blocked": jobs.iter().filter(|j| j.phase == JobPhase::Blocked).map(|j| serde_json::json!({"job": j.id, "reason": j.blocked_reason, "work": j.work_done, "required": j.work_required})).collect::<Vec<_>>(),
                "cancelled": jobs.iter().filter(|j| j.phase == JobPhase::Cancelled).count(),
                "site_search_exhausted": r.placement_attempts >= MAX_SITE_ATTEMPTS,
                "last_placement_error": r.last_error
            })
        }).collect::<Vec<_>>();
        let unfinished = builds
            .iter()
            .filter(|b| b["completed"].as_u64() == Some(0))
            .map(|b| {
                format!(
                    "faction{}:{}",
                    b["team"],
                    b["kind"].as_str().unwrap_or("unknown")
                )
            })
            .collect::<Vec<_>>();
        let reservations = game.construction.jobs.iter().filter(|j| !matches!(j.phase, JobPhase::Complete | JobPhase::Cancelled)).map(|j| {
            serde_json::json!({"job":j.id,"team":j.order.team,"phase":j.phase,"resources":j.materials,"worker":j.worker,"pickup_position":j.pickup_position,"work_position":j.work_position,"delivered":j.delivered})
        }).collect::<Vec<_>>();
        let materials = serde_json::json!({
            "stores": game.construction.stores,
            "remaining_ore": game.construction.deposits,
            "reserved_job_materials": reservations,
            "cargo": game.construction.cargo,
            "recovery_crates": game.construction.crates,
            "spoil_liters": game.construction.spoil_liters,
            "cut_liters": game.construction.cut_liters,
            "fill_liters": game.construction.fill_liters,
            "consumed": game.construction.consumed,
            "named_external_production": game.construction.external_inputs,
            "ledger_valid": game.construction.validate().is_ok(),
            "ore_delivered": game.ore_delivered,
            "ownership_note": "Reserved job materials remain owned exactly once by their job; a worker reference or delivered flag does not create another copy. Ore cargo and cancellation recovery crates are separate finite stocks."
        });
        let metrics_scope = serde_json::json!({
            "water": "Canonical PLAYER fixture-dam flag. Upstream means the first81managedriverrows, not upstream of every animal dam; natural dam/physical counters are reported separately by Campaign.physical.",
            "satellite": "Active samples at the10second observation cadence, not a distinct pass count.",
            "queues": "Completion evidence means queue ended after observed physical departure and return to its final waypoint. Cancellations without this physical evidence only count as ended.",
            "controller": "Declared omniscient test-harness placement and both-faction core orders; production player UI and ConservationDriver capture/release remain recon-filtered."
        });
        serde_json::json!({
            "kind": "stateful_ordinary_benchmark_orders",
            "version": self.version,
            "last_drive_tick": self.last_drive_tick,
            "bootstrap_attempted": self.bootstrap_attempted,
            "bootstrap_error": self.bootstrap_error,
            "builds": builds,
            "unfinished_build_coverage": unfinished,
            "commands": self.commands,
            "observations": self.observations,
            "selected_queue_watches": self.queue_watch,
            "scout": self.scout,
            "boats": self.boats,
            "transports": self.transports,
            "actual_unit_queues": game.unit_order_queues.len(),
            "actual_boarded_passenger_ids": self.boarded_passengers,
            "disembark_target": self.disembark_target,
            "finite_materials": materials,
            "actual_combat": game.tactics.totals,
            "actual_field_support": game.support.report(),
            "attributed_ecological_damage": game.ecological_damage,
            "crater_spoil_liters": game.blast_spoil_liters,
            "initial_spawned": self.initial_spawned,
            "initial_ore_deposits": self.initial_deposits,
            "initial_ecological_damage": self.initial_ecological_damage,
            "metric_scope": metrics_scope,
            "explicit_limits": [
                "This is observed coverage, not a guarantee every order completes or every emergent trait occurs.",
                "Sanctuary is not an implemented separate building: the supported equivalent is Pen plus handlers, care, research, breeding, release and habitat protection.",
                "No paid demolition command exists; dam breach/destruction must come from actual opposing weapons or ecological physical events.",
                "Production20km streaming and the remaining130traits are not implemented by this small-map test.",
                "Naval faction1 launch uses the ordinary core paid transaction because there is no faction1 player UI.",
                "Dedicated invariant/mechanism fixtures remain separately labelled from this ordinary long run."
            ],
            "driver_coarse_cache_bytes": self.reachable.cells.iter().map(|v| v.capacity()).sum::<usize>() + self.reachable.labels.capacity() * std::mem::size_of::<u32>(),
            "command_budget": "Maximum four ordinary commands per drive: at most enqueue+immediate cancellation, and two activity commands. The once-only bootstrap separately queues up to six ordinary paid build orders. No missed-deadline catch-up dump.",
            "resume_contract": "Serialize OrderExercise with Campaign; restoring only one is an invalid checkpoint."
        })
    }
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn visible_goal(game: &Campaign, origin: [f32; 2], minimum: f32, maximum: f32) -> Option<[f32; 2]> {
    let start = game.nav.index(origin)?;
    let x = start % game.nav.width;
    let y = start / game.nav.width;
    let r = (maximum / 16.).ceil() as usize;
    let mut best = None;
    let mut score = f32::MAX;
    for yy in y.saturating_sub(r)..=(y + r).min(game.nav.height - 1) {
        for xx in x.saturating_sub(r)..=(x + r).min(game.nav.width - 1) {
            let i = yy * game.nav.width + xx;
            if !game.nav.passable(i, MovementClass::Foot) {
                continue;
            }
            let p = game.nav.center_mm(i).map(|n| n as f32 * 0.001);
            let d = distance(origin, p);
            if d >= minimum && d <= maximum && game.recon.visible(0, p) && d < score {
                best = Some(p);
                score = d;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checkpoint_roundtrip_preserves_each_order_cursor_and_accepted_identity() {
        let mut x = OrderExercise::new();
        x.bootstrap_attempted = true;
        x.last_drive_tick = Some(1200);
        x.build_cursor = 12;
        x.requests[7].job_ids = vec![19, 42];
        x.requests[7].site_cursor = 8;
        x.requests[7].placement_attempts = 9;
        x.scout = Some(55);
        x.scout_sent_out = true;
        x.observations.camera_battery_depleted = true;
        x.boarded_passengers.insert(91);
        x.disembark_target = Some([144., 288.]);
        x.queue_watch.insert(
            89,
            QueueWatch {
                issued_tick: 1180,
                origin: Some([144., 144.]),
                final_goal: Some([160., 144.]),
                moved_away_observed: true,
                ..Default::default()
            },
        );
        let restored: OrderExercise =
            serde_json::from_slice(&serde_json::to_vec(&x).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&x).unwrap(),
            serde_json::to_value(restored).unwrap()
        );
    }
    #[test]
    fn recipe_has_all_supported_kinds_for_both_sides_without_sanctuary_fiction() {
        let x = OrderExercise::new();
        assert_eq!(x.requests.len(), 38);
        for kind in BUILD_KINDS {
            assert_eq!(x.requests.iter().filter(|r| r.kind == kind).count(), 2);
        }
        assert!(!x.requests.iter().any(|r| r.kind.name() == "sanctuary"));
    }
    #[test]
    fn repeated_drive_and_restored_cursor_do_not_reserve_bootstrap_materials_twice() {
        let mut game = Campaign::new_for_benchmark(78421, 8, 8).unwrap();
        let mut driver = OrderExercise::new();
        driver.drive(&mut game).unwrap();
        assert!(driver.bootstrap_attempted);
        assert!(!game.construction.jobs.is_empty());
        let stores = game.construction.stores;
        let jobs = game.construction.jobs.len();
        let commands = serde_json::to_value(&driver.commands).unwrap();
        driver.drive(&mut game).unwrap();
        let mut restored: OrderExercise =
            serde_json::from_slice(&serde_json::to_vec(&driver).unwrap()).unwrap();
        restored.drive(&mut game).unwrap();
        assert_eq!(game.construction.stores, stores);
        assert_eq!(game.construction.jobs.len(), jobs);
        assert_eq!(serde_json::to_value(&restored.commands).unwrap(), commands);
        game.construction.validate().unwrap();
        restored.last_drive_tick = Some(game.processed_tick + 1);
        assert!(restored.drive(&mut game).is_err());
        assert_eq!(game.construction.stores, stores);
        assert_eq!(game.construction.jobs.len(), jobs);
    }
}
