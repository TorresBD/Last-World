//! Two finite, knowledge-limited commanders for the explicit visible battle check.
//! This is an initial-world scenario/controller, never a second simulation.
use crate::{
    campaign::Campaign,
    construction::{BuildKind, BuildOrder, JobPhase, TEAM_BASES},
    gpu::Agent,
    tactics::Role,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const BATTLE_TICKS: u64 = 20 * 60 * 20;
const DECISION_TICKS: u64 = 20;
const BUILD_TICKS: u64 = 100;
const ATTACK_TICKS: u64 = 200;
const MAX_PENDING: usize = 4;
const FRONT: [f32; 2] = [2208., 1580.];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkProgress {
    phase: JobPhase,
    work_done: u32,
    best_distance_m: f32,
    last_progress_tick: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TeamBrain {
    pub decisions: u64,
    pub accepted_build_orders: u64,
    pub accepted_attack_orders: u64,
    pub parked_idle_workers: BTreeSet<u64>,
    pub minimum_observed_fired_rounds: u64,
    pub minimum_observed_late_fired_rounds: u64,
    pub last_actual_shot_tick: Option<u64>,
    pub late_combat_observation_windows: u64,
    pub visible_enemy_reactions: u64,
    pub rejected_commands: u64,
    pub last_error: Option<String>,
    pub next_build_tick: u64,
    pub next_attack_tick: u64,
    pub attack_squad: usize,
    pub build_site_cursor: u32,
    pub work_progress: BTreeMap<u64, WorkProgress>,
    pub stalled_orders_cancelled: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BattleDirector {
    pub initialized: bool,
    pub start_tick: u64,
    pub next_decision_tick: u64,
    pub last_tick: Option<u64>,
    pub teams: [TeamBrain; 2],
    pub initial_biology_events: u64,
    pub initial_fired_rounds: u64,
    pub initial_water_tick: u64,
    pub initial_terrain_revision: u64,
    pub initial_ecological_damage: [u64; 2],
    pub last_live_ammunition: Vec<Option<u16>>,
    pub requested_initial_counts: Option<[u64; 2]>,
    pub timeline: Vec<Value>,
    pub next_timeline_tick: u64,
    pub timeline_previous_rounds: [u64; 2],
}
impl BattleDirector {
    /// Controllers run once per game second, never once per rendered frame.
    pub fn drive(&mut self, game: &mut Campaign) -> Result<(), String> {
        let tick = game.processed_tick;
        if self.last_tick.is_some_and(|previous| tick < previous) {
            return Err("Battle commander is ahead of its saved world".into());
        }
        if self.last_tick == Some(tick) || tick < self.next_decision_tick {
            return Ok(());
        }
        if !self.initialized {
            self.initialized = true;
            self.start_tick = tick;
            self.initial_biology_events = game.biological_events;
            self.initial_fired_rounds = game.tactics.totals.fired_ammunition;
            self.initial_water_tick = game.river.last_processed_tick;
            self.initial_terrain_revision = game.terrain.revision();
            self.initial_ecological_damage = game.ecological_damage;
            self.requested_initial_counts = Some(game.tactics.totals.spawned);
            // This scenario has two commanders. Suppress the ordinary single
            // enemy's six-item periodic script, not any tactical/ecology rule.
            game.next_enemy_order = u64::MAX;
            game.automated = false;
        }
        self.last_tick = Some(tick);
        self.next_decision_tick = tick.saturating_add(DECISION_TICKS);
        self.observe_real_shots(game);
        if tick >= self.next_timeline_tick {
            let rounds = std::array::from_fn::<_, 2, _>(|team| {
                self.teams[team].minimum_observed_fired_rounds
            });
            let deltas = std::array::from_fn::<_, 2, _>(|team| {
                rounds[team].saturating_sub(self.timeline_previous_rounds[team])
            });
            for team in 0..2 {
                if tick.saturating_sub(self.start_tick) >= 12_000 && deltas[team] > 0 {
                    self.teams[team].late_combat_observation_windows += 1;
                }
            }
            self.timeline.push(json!({"tick":tick,"minimum_observed_round_deltas":deltas,
                "global_actual_fired_rounds":game.tactics.totals.fired_ammunition,
                "last_actual_shot_tick":[self.teams[0].last_actual_shot_tick,self.teams[1].last_actual_shot_tick],
                "paid_completed_jobs":[paid_jobs(game,0),paid_jobs(game,1)],
                "military_deaths":game.tactics.totals.deaths,"biology_events":game.biological_events,
                "ecological_damage":game.ecological_damage,"water_tick":game.river.last_processed_tick}));
            self.timeline_previous_rounds = rounds;
            self.next_timeline_tick = tick.saturating_add(600);
        }
        for team in 0..2u8 {
            self.teams[team as usize].decisions += 1;
            self.disperse_idle_workers(game, team);
            if tick >= self.teams[team as usize].next_attack_tick {
                self.attack(game, team);
                self.teams[team as usize].next_attack_tick = tick.saturating_add(ATTACK_TICKS);
            }
            if tick >= self.teams[team as usize].next_build_tick {
                self.build(game, team);
                self.teams[team as usize].next_build_tick = tick.saturating_add(BUILD_TICKS);
            }
        }
        Ok(())
    }
    fn observe_real_shots(&mut self, game: &Campaign) {
        if self.last_live_ammunition.len() < game.tactics.agents.len() {
            self.last_live_ammunition
                .resize(game.tactics.agents.len(), None);
        }
        for (i, a) in game.tactics.agents.iter().enumerate() {
            if a.alive() {
                if let Some(old) = self.last_live_ammunition[i] {
                    // A living pawn's ammunition drops only when its actual
                    // weapon fires. Death-lost rounds cannot count as shots.
                    let rounds = old.saturating_sub(a.ammunition) as u64;
                    let team = &mut self.teams[a.team as usize];
                    team.minimum_observed_fired_rounds += rounds;
                    if rounds > 0 {
                        team.last_actual_shot_tick = Some(game.processed_tick);
                        if game.processed_tick.saturating_sub(self.start_tick) >= 12_000 {
                            team.minimum_observed_late_fired_rounds += rounds;
                        }
                    }
                }
                self.last_live_ammunition[i] = Some(a.ammunition);
            } else {
                self.last_live_ammunition[i] = None;
            }
        }
    }
    fn reject(&mut self, team: u8, error: String) {
        let brain = &mut self.teams[team as usize];
        brain.rejected_commands += 1;
        brain.last_error = Some(error);
    }
    fn disperse_idle_workers(&mut self, game: &mut Campaign, team: u8) {
        // Keep8 nearby workers available; move the remaining idle formation in
        // bounded groups through real collision/pathfinding. No pawn vanishes
        // and a subsequently assigned hauling job overrides this parking goal.
        let parked = &self.teams[team as usize].parked_idle_workers;
        let ids: Vec<_> = game
            .tactics
            .agents
            .iter()
            .filter(|a| a.team == team && a.role == Role::Worker && a.alive())
            .skip(8)
            .filter(|a| {
                a.job.is_none()
                    && !parked.contains(&a.id)
                    && !game.construction.cargo.contains_key(&(a.id as u32))
            })
            .take(96)
            .map(|a| a.id)
            .collect();
        if ids.is_empty() {
            return;
        }
        let base = TEAM_BASES[team as usize];
        let slot = self.teams[team as usize].parked_idle_workers.len() / 96;
        let point = [
            base[0] + if team == 0 { -192. } else { 192. },
            (base[1] + 192. + slot as f32 * 96.).clamp(128., 3072.),
        ];
        match game
            .data
            .tactics
            .order_selected_on_grid(team, &ids, point, &game.data.nav, false)
        {
            Ok(_) => self.teams[team as usize].parked_idle_workers.extend(ids),
            Err(error) => self.reject(team, error),
        }
    }
    fn attack(&mut self, game: &mut Campaign, team: u8) {
        let enemy = known_enemy_goal(game, team);
        let squad = self.teams[team as usize].attack_squad % 4;
        self.teams[team as usize].attack_squad += 1;
        let point = enemy.unwrap_or([
            FRONT[0] + if team == 0 { 48. } else { -48. },
            FRONT[1] + [0., -128., 128., 0.][squad],
        ]);
        let ids: Vec<_> = game
            .tactics
            .agents
            .iter()
            .filter(|a| {
                a.team == team
                    && a.role == Role::Soldier
                    && a.alive()
                    && a.embarked_on.is_none()
                    && a.id as usize % 4 == squad
            })
            .map(|a| a.id)
            .collect();
        if ids.is_empty() {
            return;
        }
        match game
            .data
            .tactics
            .order_selected_on_grid(team, &ids, point, &game.data.nav, true)
        {
            Ok(_) => {
                self.teams[team as usize].accepted_attack_orders += 1;
                self.teams[team as usize].visible_enemy_reactions += u64::from(enemy.is_some());
            }
            Err(error) => self.reject(team, error),
        }
    }
    fn build(&mut self, game: &mut Campaign, team: u8) {
        // Cancel a genuinely blocked construction order through the normal
        // refund/ownership path. Work and materials are never completed here.
        if let Some(id) = game
            .construction
            .jobs
            .iter()
            .find(|j| j.order.team == team && j.phase == JobPhase::Blocked)
            .map(|j| j.id)
        {
            if let Err(error) = game.construction.cancel(id, team) {
                self.reject(team, error);
            }
            return;
        }
        let tick = game.processed_tick;
        let stalled = game
            .construction
            .jobs
            .iter()
            .filter(|j| {
                j.order.team == team
                    && !matches!(
                        j.phase,
                        JobPhase::Complete | JobPhase::Cancelled | JobPhase::Blocked
                    )
            })
            .find_map(|j| {
                let point = if j.phase == JobPhase::Pickup {
                    j.pickup_position
                } else {
                    j.work_position
                };
                let distance = j
                    .worker
                    .and_then(|id| game.tactics.agent(id as u64))
                    .map(|a| distance(a.position(), point))
                    .unwrap_or(f32::MAX);
                let progress = self.teams[team as usize]
                    .work_progress
                    .entry(j.id)
                    .or_insert(WorkProgress {
                        phase: j.phase,
                        work_done: j.work_done,
                        best_distance_m: distance,
                        last_progress_tick: tick,
                    });
                if progress.phase != j.phase
                    || progress.work_done != j.work_done
                    || distance + 1. < progress.best_distance_m
                {
                    progress.phase = j.phase;
                    progress.work_done = j.work_done;
                    progress.best_distance_m = distance;
                    progress.last_progress_tick = tick;
                }
                (tick.saturating_sub(progress.last_progress_tick) >= 2400).then_some(j.id)
            });
        if let Some(id) = stalled {
            match game.construction.cancel(id, team) {
                Ok(()) => self.teams[team as usize].stalled_orders_cancelled += 1,
                Err(error) => self.reject(team, error),
            }
            return;
        }
        let pending = game
            .construction
            .jobs
            .iter()
            .filter(|j| {
                j.order.team == team
                    && !matches!(
                        j.phase,
                        JobPhase::Complete | JobPhase::Cancelled | JobPhase::Blocked
                    )
            })
            .count();
        if pending >= MAX_PENDING {
            return;
        }
        let kinds = [
            BuildKind::Camera,
            BuildKind::Road,
            BuildKind::Solar,
            BuildKind::Mine,
            BuildKind::Barracks,
            BuildKind::Radar,
            BuildKind::Trench,
            BuildKind::Turret,
            BuildKind::Mill,
            BuildKind::Farm,
            BuildKind::Depot,
        ];
        let wanted = kinds.into_iter().find(|&kind| {
            !game.construction.jobs.iter().any(|j| {
                j.order.team == team && j.order.kind == kind && j.phase != JobPhase::Cancelled
            })
        });
        let kind = wanted.unwrap_or_else(|| {
            if game.construction.jobs.iter().any(|j| {
                j.order.team == team
                    && j.order.kind == BuildKind::Road
                    && !matches!(
                        j.phase,
                        JobPhase::Complete | JobPhase::Cancelled | JobPhase::Blocked
                    )
            }) {
                BuildKind::Turret
            } else {
                BuildKind::Road
            }
        });
        let base = TEAM_BASES[team as usize];
        let inward = if team == 0 { 1. } else { -1. };
        let cursor = self.teams[team as usize].build_site_cursor;
        let mut order = if kind == BuildKind::Mine {
            let point = game
                .construction
                .deposits
                .iter()
                .filter(|d| game.recon.visible(team, d.position) && d.remaining > 0)
                .min_by(|a, b| distance(a.position, base).total_cmp(&distance(b.position, base)))
                .map(|d| d.position)
                .unwrap_or(base);
            BuildOrder::at(team, kind, point)
        } else if kind == BuildKind::Road {
            let progress = game
                .construction
                .jobs
                .iter()
                .filter(|j| {
                    j.order.team == team
                        && j.order.kind == BuildKind::Road
                        && j.phase == JobPhase::Complete
                })
                .map(|j| (j.order.end[0] - base[0]) * inward)
                .fold(16f32, f32::max)
                .min(1400.);
            let from = [base[0] + inward * progress, base[1] + 24.];
            let mut o = BuildOrder::at(team, kind, from);
            o.end = [from[0] + inward * 128., from[1]];
            o
        } else if kind == BuildKind::Trench {
            let point = [base[0] + inward * 160., base[1] - 96.];
            let mut o = BuildOrder::at(team, kind, point);
            o.end = [point[0], point[1] + 96.];
            o
        } else {
            // A bounded spiral of actual sites beside the supply corridor.
            let band = (cursor / 8) % 4;
            let direction = [
                [1., -1.],
                [1., 1.],
                [-1., -1.],
                [-1., 1.],
                [0., -1.],
                [0., 1.],
                [1., 0.],
                [-1., 0.],
            ][cursor as usize % 8];
            let radius = 80. + band as f32 * 48.;
            BuildOrder::at(
                team,
                kind,
                [
                    base[0] + direction[0] * radius,
                    base[1] + direction[1] * radius,
                ],
            )
        };
        order.position[0] = order.position[0].clamp(40., 4760.);
        order.position[1] = order.position[1].clamp(40., 3160.);
        order.end[0] = order.end[0].clamp(40., 4760.);
        order.end[1] = order.end[1].clamp(40., 3160.);
        self.teams[team as usize].build_site_cursor += 1;
        match game.data.construction.enqueue(order, &game.terrain) {
            Ok(_) => self.teams[team as usize].accepted_build_orders += 1,
            Err(error) => self.reject(team, error),
        }
    }
    pub fn report(&self, game: &Campaign) -> Value {
        let completed: [u64; 2] = std::array::from_fn(|team| {
            game.construction
                .jobs
                .iter()
                .filter(|j| {
                    j.order.team as usize == team
                        && j.phase == JobPhase::Complete
                        && j.material_consumed
                })
                .count() as u64
        });
        json!({"kind":"two_knowledge_limited_aggressive_commanders", "decision_hz":1,
            "named_initial_deployment":"staged_frontline_clash; military bulk origins1750m/2666m before first clock tick; normal ship opening unchanged",
            "initial_counts_and_resources":"same10000military+2000workers and finite crashed-ship salvage; no runtime relocation or extra forces",
            "timeline":self.timeline,
            "required_actual_ticks":BATTLE_TICKS,"actual_ticks":game.processed_tick.saturating_sub(self.start_tick),
            "teams":self.teams,"paid_completed_jobs":completed,
            "stock_consumed":game.construction.consumed,"remaining_stocks":game.construction.stores,
            "fired_rounds":game.tactics.totals.fired_ammunition.saturating_sub(self.initial_fired_rounds),
            "military_deaths":game.tactics.totals.deaths,"biology_events":game.biological_events.saturating_sub(self.initial_biology_events),
            "water_tick":game.river.last_processed_tick,"ecological_damage":game.ecological_damage,
            "terrain_changed":game.terrain.revision()!=self.initial_terrain_revision,
            "initial_tactical_records":self.requested_initial_counts,"current_tactical_records":game.tactics.agents.len(),
            "no_free_stock_no_teleport_no_rule_skips":true,
            "enemy_intelligence":"current own-team video only; midfield objectives are declared strategy, never hidden enemy state",
            "observer_rendering":"diagnostic-only full surface visibility; no recon, discovery, research or AI knowledge changes"})
    }
    pub fn qualification_error(&self, game: &Campaign) -> Option<String> {
        if game.processed_tick.saturating_sub(self.start_tick) < BATTLE_TICKS {
            return Some("Battle did not simulate all24000authoritative ticks".into());
        }
        for team in 0..2 {
            if self.teams[team].accepted_attack_orders == 0
                || self.teams[team].minimum_observed_fired_rounds == 0
            {
                return Some(format!(
                    "Faction{team}did not demonstrate real commanded combat"
                ));
            }
            if self.teams[team].minimum_observed_late_fired_rounds < 20
                || self.teams[team].late_combat_observation_windows < 3
            {
                return Some(format!(
                    "Faction{team}did not demonstrate meaningful combat during the later10minutes"
                ));
            }
            let kinds: BTreeSet<_> = game
                .construction
                .jobs
                .iter()
                .filter(|j| {
                    j.order.team as usize == team
                        && j.phase == JobPhase::Complete
                        && j.material_consumed
                })
                .map(|j| j.order.kind.name())
                .collect();
            if paid_jobs(game, team as u8) < 2 || kinds.len() < 2 {
                return Some(format!(
                    "Faction{team}did not complete2paid jobs of2different kinds"
                ));
            }
        }
        if game.biological_events <= self.initial_biology_events
            || game.river.last_processed_tick <= self.initial_water_tick
        {
            return Some("Battle ecology/water did not continue".into());
        }
        None
    }
}
fn paid_jobs(game: &Campaign, team: u8) -> u64 {
    game.construction
        .jobs
        .iter()
        .filter(|j| j.order.team == team && j.phase == JobPhase::Complete && j.material_consumed)
        .count() as u64
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn known_enemy_goal(game: &Campaign, team: u8) -> Option<[f32; 2]> {
    game.tactics
        .agents
        .iter()
        .filter(|a| {
            a.team != team
                && a.alive()
                && a.embarked_on.is_none()
                && game.recon.visible(team, a.position())
        })
        .min_by(|a, b| distance(a.position(), FRONT).total_cmp(&distance(b.position(), FRONT)))
        .map(|a| a.position())
}
/// Pure display copy of existing surface identities. The campaign's recon and
/// species knowledge are never modified and underground occupants stay hidden.
pub fn observer_agents(game: &Campaign, alpha: f32) -> Vec<Agent> {
    let alpha = alpha.clamp(0., 1.);
    let mut output: Vec<_> = game
        .tactics
        .agents
        .iter()
        .filter(|a| {
            a.alive() && a.embarked_on.is_none() && !matches!(a.role, Role::Boat | Role::Transport)
        })
        .map(|a| Agent {
            pos: [
                (a.previous_position_mm[0] as f32 * (1. - alpha) + a.position_mm[0] as f32 * alpha)
                    * 0.001,
                (a.previous_position_mm[1] as f32 * (1. - alpha) + a.position_mm[1] as f32 * alpha)
                    * 0.001,
            ],
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
        })
        .collect();
    let allowed: BTreeMap<_, _> = game
        .ecology
        .cohorts
        .iter()
        .filter(|c| c.count == 1 && c.surface_observable && c.location_height_mm >= 0)
        .map(|c| (c.id, c.species_id))
        .collect();
    for (id, d) in &game.detailed {
        let Some(&species) = allowed.get(id) else {
            continue;
        };
        output.push(Agent {
            pos: [
                d.previous[0] * (1. - alpha) + d.position[0] * alpha,
                d.previous[1] * (1. - alpha) + d.position[1] * alpha,
            ],
            facing: 0.,
            id: *id as u32,
            goal: 0,
            team: game.terrain.fauna_appearance(species),
            role: 4,
            next: -1,
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_commanders_reserve_real_material_and_do_not_order_twice_on_one_tick() {
        let mut game = Campaign::new(78421, 128, 160).unwrap();
        let before = game.construction.stores;
        let count = game.tactics.agents.len();
        let mut director = BattleDirector::default();
        director.drive(&mut game).unwrap();
        assert_eq!(game.tactics.agents.len(), count);
        for team in 0..2 {
            assert_eq!(director.teams[team].decisions, 1);
            assert!(director.teams[team].accepted_build_orders > 0);
            assert!(game.construction.stores[team].metal < before[team].metal);
            assert!(director.teams[team].accepted_attack_orders > 0);
        }
        let bytes = serde_json::to_vec(&(&game.data, &director)).unwrap();
        director.drive(&mut game).unwrap();
        assert_eq!(serde_json::to_vec(&(&game.data, &director)).unwrap(), bytes);
        game.validate().unwrap();
    }
    #[test]
    fn named_frontline_start_preserves_counts_salvage_ecology_and_legal_spawn_ownership() {
        let normal = Campaign::new(78421, 128, 16).unwrap();
        let staged = Campaign::new_battle_benchmark(78421, 128, 16).unwrap();
        assert_eq!(normal.tactics.totals.spawned, staged.tactics.totals.spawned);
        assert_eq!(
            normal.tactics.totals.initial_ammunition,
            staged.tactics.totals.initial_ammunition
        );
        assert_eq!(normal.construction.stores, staged.construction.stores);
        assert_eq!(
            serde_json::to_value(normal.ecology.census()).unwrap(),
            serde_json::to_value(staged.ecology.census()).unwrap()
        );
        assert_eq!(staged.clock.tick(), 0);
        assert_eq!(staged.processed_tick, 0);
        assert_eq!(normal.tactics.agents.len(), staged.tactics.agents.len());
        assert!(
            normal
                .tactics
                .agents
                .iter()
                .zip(&staged.tactics.agents)
                .any(|(a, b)| a.role == Role::Soldier && a.position_mm != b.position_mm)
        );
        for (a, b) in normal.tactics.agents.iter().zip(&staged.tactics.agents) {
            assert_eq!(
                (a.id, a.role, a.team, a.ammunition),
                (b.id, b.role, b.team, b.ammunition)
            );
            assert_eq!(b.position_mm, b.previous_position_mm);
            assert!(b.goal_mm.is_none());
            let cell = staged.nav.index_mm(b.position_mm).unwrap();
            assert!(staged.nav.passable(cell, b.role.movement()));
            if a.role == Role::Worker {
                assert_eq!(a.position_mm, b.position_mm);
            }
        }
        staged.validate().unwrap();
    }
    #[test]
    fn hidden_enemy_changes_cannot_change_the_commanders_known_target() {
        let mut game = Campaign::new(78421, 128, 16).unwrap();
        assert!(known_enemy_goal(&game, 0).is_none());
        let id = game.tactics.agents.iter().find(|a| a.team == 1).unwrap().id;
        game.data.tactics.agent_mut(id).unwrap().position_mm = [3_000_000, 2_000_000];
        assert!(known_enemy_goal(&game, 0).is_none());
    }
    #[test]
    fn diagnostic_draw_does_not_grant_video_or_research_or_duplicate_pawns() {
        let game = Campaign::new(78421, 128, 16).unwrap();
        let before = serde_json::to_vec(&game.data.recon).unwrap();
        let view = observer_agents(&game, 0.5);
        let ids: BTreeSet<_> = view.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), view.len());
        assert_eq!(view.len(), game.tactics.agents.len());
        assert_eq!(serde_json::to_vec(&game.data.recon).unwrap(), before);
    }
}
