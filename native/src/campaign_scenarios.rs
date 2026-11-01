//! Connected CPU qualification scenes. These exercise actual campaign commands
//! and fixed ticks; they never complete jobs, refill stocks or create animals
//! on behalf of the player. The explicit smaller fixture is not a reduction of
//! the game's default10,000military +2,000worker configuration.

use crate::{
    campaign::{Campaign, CampaignSave, patch_at},
    construction::{BuildKind, JobPhase, TEAM_BASES, WorkPurpose, WorkerState},
    ecology::{Custody, Habitat, Sex, Stage, Taxon},
    logging,
};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const SEED: u32 = 78_421;
// Twenty simulation minutes is a correctness-test ceiling, not a change to
// match duration/speed. The 10 minute run proved that the second actual handler
// was still returning through wet jungle; care, gestation and release follow.
const MAX_TICKS: u64 = 24_000;
const INSPECT: usize = 19;
const CAPTURE: usize = 20;
const RESEARCH: usize = 21;
const BREED: usize = 22;
const RELEASE: usize = 23;

#[derive(Default, Serialize)]
struct Gates {
    generated_habitat_food_web: bool,
    discovery: bool,
    paid_worker_construction: bool,
    mining_and_delivery: bool,
    actual_combat: bool,
    capture_and_handler_return: bool,
    continuing_care: bool,
    research: bool,
    compatible_breeding: bool,
    real_captive_offspring: bool,
    existing_animal_release: bool,
    save_load_continuity: bool,
    exact_invariants: bool,
    cross_seed_smoke: bool,
}
impl Gates {
    fn passed(&self) -> bool {
        self.gameplay_passed() && self.cross_seed_smoke
    }
    fn gameplay_passed(&self) -> bool {
        self.generated_habitat_food_web
            && self.discovery
            && self.paid_worker_construction
            && self.mining_and_delivery
            && self.actual_combat
            && self.capture_and_handler_return
            && self.continuing_care
            && self.research
            && self.compatible_breeding
            && self.real_captive_offspring
            && self.existing_animal_release
            && self.save_load_continuity
            && self.exact_invariants
    }
}

/// Returns a report even if a gameplay gate is not reached. The launcher must
/// treat passed=false as failure while retaining this diagnostic report.
pub fn integration_report() -> Result<serde_json::Value, String> {
    let mut game = Campaign::new(SEED, 512, 64)?;
    let initial = game.report();
    let initial_tactical_diagnostics = tactical_diagnostics(&game);
    let initial_deposit: u64 = game.construction.deposits.iter().map(|d| d.remaining).sum();
    let mut gates = Gates {
        generated_habitat_food_web: !game.ecology.plants.is_empty()
            && game.ecology.species.iter().any(|s| {
                !matches!(s.taxon, Taxon::Plant | Taxon::Fungus) && !s.food_species.is_empty()
            })
            && game.ecology.census().wild_animals > 0,
        discovery: game.ecology.knowledge[0].iter().any(|k| k.observed),
        ..Default::default()
    };
    game.automation()?;
    // The window fixture's2400tick exit limit is not a gameplay rule. This CPU
    // scene continues ordinary ticks while real handlers/care/gestation run.
    game.automated = false;
    let mut selected = None;
    let mut adult_ids = BTreeSet::new();
    let mut released = None;
    let mut release_request: Option<(u64, u32, u64, [f32; 2])> = None;
    let mut milestones = Vec::new();
    let mut checkpoints = vec![serde_json::json!({"stage":"initial","state":initial})];
    let mut recent_command_errors = Vec::new();
    let mut save_receipt = None;
    let mut last_checkpoint = 0;
    let mut breeding_requested = false;
    let mut previous_handler_signature = String::new();

    while game.processed_tick < MAX_TICKS {
        let before = game.processed_tick;
        game.advance(Duration::from_millis(50))
            .map_err(|e| format!("Connected campaign failed after tick{before}: {e}"))?;
        if game.processed_tick <= before {
            return Err(format!(
                "Connected scene stopped before its gates at tick{}: {}",
                game.processed_tick, game.outcome
            ));
        }
        let tick = game.processed_tick;
        if tick % 20 != 0 {
            continue;
        }
        game.validate()?;
        gates.exact_invariants = true;
        let completed_player = game
            .construction
            .jobs
            .iter()
            .filter(|j| j.order.team == 0 && j.phase == JobPhase::Complete && j.material_consumed)
            .count();
        gates.paid_worker_construction = completed_player >= 2;
        gates.mining_and_delivery = game
            .construction
            .deposits
            .iter()
            .map(|d| d.remaining)
            .sum::<u64>()
            < initial_deposit
            && game.ore_delivered[0] > 0;
        gates.actual_combat = game.tactics.totals.fired_ammunition > 0
            && game.tactics.totals.deaths.iter().sum::<u64>() > 0;
        let signature = serde_json::to_string(&(
            game.capture_orders
                .iter()
                .map(|o| (o.worker, o.cohort, o.captured, o.releasing, o.picked_up))
                .collect::<Vec<_>>(),
            game.ecology
                .captives
                .iter()
                .filter(|c| c.team == 0)
                .map(|c| (c.animal.id, c.housing))
                .collect::<Vec<_>>(),
        ))
        .map_err(|e| e.to_string())?;
        if signature != previous_handler_signature {
            if !game.capture_orders.is_empty()
                || !game.ecology.captives.is_empty()
                || !previous_handler_signature.is_empty()
            {
                milestone(
                    &mut milestones,
                    tick,
                    "handler_ownership_or_housing_changed",
                    capture_diagnostics(&game),
                );
            }
            previous_handler_signature = signature;
        }

        let pen = game
            .construction
            .buildings
            .iter()
            .any(|b| b.team == 0 && b.complete && b.health > 0 && b.kind == BuildKind::Pen);
        let lab =
            game.construction.buildings.iter().any(|b| {
                b.team == 0 && b.complete && b.health > 0 && b.kind == BuildKind::Research
            });
        if pen && selected.is_none() {
            if let Some((species, patch, cohort, point)) = capture_candidate(&game, None) {
                game.selected_tool = INSPECT;
                game.click(point, false)?;
                // Select only a currently observed compatible species. Inspect
                // may select another visible resident of the same patch first.
                game.selected_species = Some(species);
                selected = Some(species);
                milestone(
                    &mut milestones,
                    tick,
                    "observed_species_selected",
                    serde_json::json!({"species":species,"patch":patch,"cohort":cohort,"point":point}),
                );
            }
        }

        if let Some(species) = selected {
            let (female, male, current_adults) = {
                let adults: Vec<_> = game
                    .ecology
                    .captives
                    .iter()
                    .filter(|c| {
                        c.team == 0
                            && c.animal.species_id == species
                            && c.animal.stage == Stage::Adult
                            && c.housing.is_some()
                    })
                    .collect();
                (
                    adults.iter().any(|c| c.animal.sex != Sex::Male),
                    adults.iter().any(|c| c.animal.sex != Sex::Female),
                    adults.iter().map(|c| c.animal.id).collect::<BTreeSet<_>>(),
                )
            };
            let pair = female && male && current_adults.len() >= 2;
            let pending = game
                .capture_orders
                .iter()
                .filter(|o| o.team == 0 && o.species == species)
                .count();
            if !gates.capture_and_handler_return && !pair && pending == 0 {
                if let Some((_, patch, cohort, point)) = capture_candidate(&game, Some(species)) {
                    game.selected_tool = CAPTURE;
                    game.selected_species = Some(species);
                    match game.click(point, false) {
                        Ok(()) => milestone(
                            &mut milestones,
                            tick,
                            "handler_dispatched",
                            serde_json::json!({"species":species,"patch":patch,"desired_cohort":cohort,"point":point,"actual_orders":capture_diagnostics(&game)}),
                        ),
                        Err(e) => keep_error(&mut recent_command_errors, tick, "capture", e),
                    }
                }
            }

            let pending_delivery = game
                .capture_orders
                .iter()
                .any(|o| o.team == 0 && o.species == species);
            if pair && !pending_delivery && !gates.capture_and_handler_return {
                adult_ids = current_adults;
                gates.capture_and_handler_return = true;
                milestone(
                    &mut milestones,
                    tick,
                    "compatible_pair_delivered",
                    serde_json::json!({"species":species,"organisms":adult_ids}),
                );
            }
            if game
                .ecology
                .captives
                .iter()
                .any(|c| c.team == 0 && c.animal.species_id == species && game.ecology.is_housed(c))
                && lab
                && !game.ecology.knowledge[0][species as usize].enabled
            {
                game.selected_tool = RESEARCH;
                game.selected_species = Some(species);
                match game.click(TEAM_BASES[0], false) {
                    Ok(()) => milestone(
                        &mut milestones,
                        tick,
                        "research_started",
                        serde_json::json!({"species":species}),
                    ),
                    Err(e) => keep_error(&mut recent_command_errors, tick, "research", e),
                }
            }
            gates.continuing_care =
                gates.capture_and_handler_return && game.water_given_to_care > 0;
            gates.research = game.ecology.knowledge[0][species as usize].study_ticks >= 800;
            if gates.research && !breeding_requested {
                game.selected_tool = BREED;
                game.selected_species = Some(species);
                match game.click(TEAM_BASES[0], false) {
                    Ok(()) => {
                        breeding_requested = true;
                        milestone(
                            &mut milestones,
                            tick,
                            "breeding_enabled",
                            serde_json::json!({"species":species}),
                        );
                    }
                    Err(e) => keep_error(&mut recent_command_errors, tick, "breed", e),
                }
            }
            if game.ecology.clutches.iter().any(|c| {
                c.species_id == species
                    && c.custody == Custody::Captive(0)
                    && adult_ids.contains(&c.mother_id)
            }) {
                gates.compatible_breeding = true;
            }
            let children: Vec<_> = game
                .ecology
                .captives
                .iter()
                .filter(|c| {
                    c.team == 0
                        && c.animal.species_id == species
                        && !adult_ids.contains(&c.animal.id)
                        && c.animal.parent_id.is_some_and(|id| adult_ids.contains(&id))
                })
                .map(|c| c.animal.id)
                .collect();
            if !children.is_empty() && !gates.real_captive_offspring {
                gates.real_captive_offspring = true;
                gates.compatible_breeding = true;
                milestone(
                    &mut milestones,
                    tick,
                    "finite_captive_offspring_born",
                    serde_json::json!({"species":species,"children":children}),
                );
            }
            if let Some((id, patch, requested_tick, requested_point)) = release_request {
                let pending = game
                    .capture_orders
                    .iter()
                    .any(|o| o.releasing && o.captured == Some(id));
                let owned = game.ecology.captives.iter().any(|c| c.animal.id == id);
                let wild = game
                    .ecology
                    .cohorts
                    .iter()
                    .find(|c| c.id == id && c.count > 0);
                if !pending
                    && !owned
                    && wild.is_some_and(|c| {
                        let actual = game.cohort_position(c);
                        c.patch == patch
                            && c.count == 1
                            && (actual[0] - requested_point[0])
                                .hypot(actual[1] - requested_point[1])
                                <= 5.
                    })
                {
                    released = Some(id);
                    gates.existing_animal_release = true;
                    milestone(
                        &mut milestones,
                        tick,
                        "existing_captive_delivered_and_released",
                        serde_json::json!({"species":species,"organism":id,"patch":patch,"requested_point":requested_point,"actual_point":wild.map(|c|game.cohort_position(c)),"handler_requested_tick":requested_tick}),
                    );
                } else if !pending && owned {
                    keep_error(&mut recent_command_errors,tick,"release","Handler returned or stopped before reaching release habitat; specimen remains owned".into());
                    release_request = None;
                }
            }
            if gates.real_captive_offspring
                && !gates.existing_animal_release
                && release_request.is_none()
            {
                if let Some((patch, point)) = visible_release_point(&game, species) {
                    game.selected_tool = RELEASE;
                    game.selected_species = Some(species);
                    match game.click(point, false) {
                        Ok(()) => {
                            let order = game
                                .capture_orders
                                .iter()
                                .rev()
                                .find(|o| o.releasing && o.team == 0 && o.species == species)
                                .ok_or("Release command did not create a real handler order")?;
                            let id = order
                                .captured
                                .ok_or("Release handler has no finite specimen identity")?;
                            release_request = Some((id, patch, tick, point));
                            milestone(
                                &mut milestones,
                                tick,
                                "release_handler_dispatched",
                                serde_json::json!({"species":species,"organism":id,"patch":patch,"point":point,"worker":order.worker}),
                            );
                        }
                        Err(e) => keep_error(&mut recent_command_errors, tick, "release", e),
                    }
                }
            }
        }

        if tick >= 1200 && save_receipt.is_none() {
            let receipt = verify_save_continuity(&mut game)?;
            gates.save_load_continuity = true;
            milestone(
                &mut milestones,
                game.processed_tick,
                "save_load_and_next_ticks_match",
                receipt.clone(),
            );
            save_receipt = Some(receipt);
        }
        if game.processed_tick >= last_checkpoint + 600 {
            last_checkpoint = game.processed_tick;
            let checkpoint = serde_json::json!({"stage":"ordinary_connected_ticks","gates":gates,"state":game.report(),"capture_diagnostics":capture_diagnostics(&game),"mining_diagnostics":mining_diagnostics(&game)});
            logging::log("CAMPAIGN_CHECK_PROGRESS", &checkpoint.to_string());
            checkpoints.push(checkpoint);
        }
        if gates.gameplay_passed() {
            break;
        }
    }

    // The early checkpoint covers an unfinished campaign. A second checkpoint
    // must preserve the integrated state after every gameplay gate has passed:
    // paid construction, finite ore, housed offspring and the released identity.
    let final_state_save_receipt = if gates.gameplay_passed() {
        let receipt = verify_save_continuity(&mut game)?;
        milestone(
            &mut milestones,
            game.processed_tick,
            "late_integrated_state_save_load_and_next_ticks_match",
            receipt.clone(),
        );
        Some(receipt)
    } else {
        None
    };

    let mut seed_checks = Vec::new();
    for seed in [78_422, 78_423] {
        let mut smoke = Campaign::new(seed, 128, 16)?;
        for _ in 0..200 {
            smoke.advance(Duration::from_millis(50))?;
        }
        smoke.validate()?;
        seed_checks.push(serde_json::json!({"seed":seed,"military":128,"workers":16,"ticks":smoke.processed_tick,"exact_invariants":true,"state":smoke.report()}));
    }
    gates.cross_seed_smoke = seed_checks.len() == 2;
    game.validate()?;
    gates.exact_invariants = true;
    let passed = gates.passed();
    let report = serde_json::json!({
        "schema_version":1,
        "kind":"connected_campaign_cpu_correctness_not_performance",
        "seed":SEED,
        "fixture":{"military":512,"workers":64,"map_meters":[4800,3200],"live_wildlife":true,"default_game_counts_unchanged":[10000,2000]},
        "passed":passed,
        "gates":gates,
        "ticks":game.processed_tick,
        "maximum_ticks":MAX_TICKS,
        "maximum_simulation_minutes":20,
        "horizon_reason":"The second real handler was still returning at the former 12000 tick ceiling. This 24000 tick test window includes actual wet-jungle travel, care, study, gestation and release; it exits after the gameplay gates pass. Match speed, counts and all qualification gates stay unchanged.",
        "selected_observed_species":selected,
        "released_existing_animal":released,
        "pending_release_handler":release_request,
        "milestones":milestones,
        "checkpoints":checkpoints,
        "recent_command_errors":recent_command_errors,
        "save_receipt":save_receipt,
        "final_state_save_receipt":final_state_save_receipt,
        "post_gameplay_save_comparison_ticks":40,
        "cross_seed_checks":seed_checks,
        "final":game.report(),
        "initial_tactical_diagnostics":initial_tactical_diagnostics,
        "final_tactical_diagnostics":tactical_diagnostics(&game),
        "final_capture_diagnostics":capture_diagnostics(&game),
        "final_mining_diagnostics":mining_diagnostics(&game),
        "selected_species_profile":selected.and_then(|id|game.ecology.species.get(id as usize)),
        "unfinished_job_diagnostics":game.construction.jobs.iter().filter(|j|!matches!(j.phase,JobPhase::Complete|JobPhase::Cancelled)).map(|j|{
            let worker=j.worker.and_then(|id|game.tactics.agent(id as u64));
            serde_json::json!({"job":j.id,"team":j.order.team,"kind":j.order.kind,"phase":j.phase,"worker":j.worker,"worker_position":worker.map(|a|a.position()),"goal":worker.and_then(|a|a.goal_mm),"requested_goal":worker.and_then(|a|a.requested_goal_mm),"work_position":j.work_position,"pickup_position":j.pickup_position,"work_done":j.work_done,"work_required":j.work_required,"blocked_reason":j.blocked_reason})
        }).collect::<Vec<_>>(),
        "full_20km_campaign_or_hardware_performance_qualified":false,
        "remaining130traits_implemented":false
    });
    logging::log("CAMPAIGN_CHECK_RESULT", &report.to_string());
    Ok(report)
}

fn tactical_diagnostics(game: &Campaign) -> serde_json::Value {
    let mut samples = Vec::new();
    for team in 0..2 {
        let mut soldiers: Vec<_> = game
            .tactics
            .agents
            .iter()
            .filter(|a| a.alive() && a.team == team && a.role == crate::tactics::Role::Soldier)
            .collect();
        soldiers.sort_by_key(|a| {
            let p = a.position();
            ((p[0] - 2200.).abs() + (p[1] - 1580.).abs()) as u32
        });
        for a in soldiers.into_iter().take(8) {
            let enemy = game
                .tactics
                .agents
                .iter()
                .filter(|b| b.alive() && b.team != team && b.embarked_on.is_none())
                .min_by_key(|b| {
                    let dx = a.position_mm[0] as i64 - b.position_mm[0] as i64;
                    let dy = a.position_mm[1] as i64 - b.position_mm[1] as i64;
                    dx * dx + dy * dy
                });
            let closest=enemy.map(|b|serde_json::json!({"id":b.id,"position":b.position(),"distance_m":(a.position()[0]-b.position()[0]).hypot(a.position()[1]-b.position()[1]),"line_of_fire":game.nav.line_of_fire(a.position_mm,b.position_mm)}));
            samples.push(serde_json::json!({"id":a.id,"team":team,"position":a.position(),"goal":a.goal_mm,"requested_goal":a.requested_goal_mm,"health":a.health,"ammo":a.ammunition,"target":a.target,"nav_cell":game.nav.index_mm(a.position_mm).map(|i|game.nav.cells[i]),"closest_enemy":closest}));
        }
    }
    serde_json::json!({"scope":"CPUdiagnostics_only_not_player_intel","front_nearest_samples":samples})
}

/// Report-only ground truth. None of these hidden positions enter player UI,
/// target acquisition or gameplay decisions.
fn worker_diagnostic(game: &Campaign, id: u64) -> serde_json::Value {
    let Some(worker) = game.tactics.agent(id) else {
        return serde_json::json!({"id":id,"missing":true});
    };
    let point = worker.position();
    let cell = game.nav.index(point);
    let nearby = game.tactics.agents.iter().filter(|a| a.id != id && a.alive() && a.embarked_on.is_none())
        .filter_map(|a| {
            let position = a.position();
            let distance = (point[0] - position[0]).hypot(point[1] - position[1]);
            (distance < 12.).then(||serde_json::json!({"id":a.id,"team":a.team,"role":a.role,"position":position,"distance_m":distance,"goal_mm":a.goal_mm,"job":a.job,"arrived":a.arrived}))
        }).take(12).collect::<Vec<_>>();
    serde_json::json!({"id":id,"position":point,"alive":worker.alive(),"goal_mm":worker.goal_mm,"requested_goal_mm":worker.requested_goal_mm,"arrived":worker.arrived,"job":worker.job,"nav_cell":cell.map(|i|game.nav.cells[i]),"nearby_bodies_12m":nearby})
}
fn capture_diagnostics(game: &Campaign) -> serde_json::Value {
    let orders = game.capture_orders.iter().map(|order| {
        let cohort = game.ecology.cohorts.iter().find(|c|c.id == order.cohort && c.count > 0);
        let pen = game.construction.buildings.iter().find(|b|b.id == order.pen);
        let access = pen.and_then(|pen|game.construction.jobs.iter().find(|j|j.order.team == pen.team && j.order.kind == BuildKind::Pen && j.order.position == pen.position && j.phase == JobPhase::Complete)).map(|j|j.work_position);
        let target = if order.releasing && order.picked_up { order.last_known_position }
            else if order.releasing || order.captured.is_some() { access }
            else { order.last_known_position };
        let distance = game.tactics.agent(order.worker).zip(target).map(|(a,p)|(a.position()[0]-p[0]).hypot(a.position()[1]-p[1]));
        let observed = cohort.map(|c|serde_json::json!({"id":c.id,"species":c.species_id,"sex":c.sex,"stage":c.stage,"count":c.count,"position":game.cohort_position(c),"height_mm":c.location_height_mm,"surface_observable":c.surface_observable,"currently_visible":game.recon.visible(order.team,game.cohort_position(c)),"locomotion":game.ecology.species[c.species_id as usize].locomotion_for(c.stage)}));
        serde_json::json!({"order":order,"worker":worker_diagnostic(game,order.worker),"physical_target":target,"distance_to_target_m":distance,"target_nav_cell":target.and_then(|p|game.nav.index(p)).map(|i|game.nav.cells[i]),"pen_access":access,"pen_available":game.ecology.housing_available(order.team,order.pen).ok(),"target_cohort":observed,"required_handling_ticks":game.ecology.capture_work_ticks(order.cohort).ok()})
    }).collect::<Vec<_>>();
    let captives = game.ecology.captives.iter().filter(|c|c.team == 0).map(|c|serde_json::json!({"animal":c.animal,"housing":c.housing,"is_housed":game.ecology.is_housed(c),"care_food":c.care_food,"care_water":c.care_water,"breed_enabled":c.breed_enabled,"profile":game.ecology.species[c.animal.species_id as usize]})).collect::<Vec<_>>();
    serde_json::json!({"scope":"CPU_report_only_not_player_intel","tick":game.processed_tick,"latest_status":game.status,"orders":orders,"captives":captives,"housing":game.ecology.housing[0],"water_given_to_care":game.water_given_to_care,"current_season":(game.processed_tick/18000)%4,"captive_breeding_season_gate":false})
}
fn mining_diagnostics(game: &Campaign) -> serde_json::Value {
    let workers = game
        .tactics
        .agents
        .iter()
        .filter(|a| a.alive() && a.role == crate::tactics::Role::Worker && a.embarked_on.is_none())
        .filter_map(|a| {
            u32::try_from(a.id).ok().map(|id| WorkerState {
                id,
                team: a.team,
                position: a.position(),
            })
        })
        .collect::<Vec<_>>();
    let suspended = game
        .capture_orders
        .iter()
        .map(|o| o.worker)
        .chain(game.unit_order_queues.keys().copied())
        .filter_map(|id| u32::try_from(id).ok())
        .collect::<BTreeSet<_>>();
    // Task generation can assign work, so derive this preview on a clone. The
    // campaign's actual assignments/stocks are never mutated by diagnostics.
    let mut world = game.construction.clone();
    let tasks = world.worker_tasks_available(&workers, &suspended);
    let (tasks, error) = match tasks {
        Ok(tasks) => (tasks, None),
        Err(error) => (Vec::new(), Some(error)),
    };
    let miners = tasks.into_iter().filter(|task|matches!(task.purpose,WorkPurpose::Mine|WorkPurpose::ReturnCargo)).map(|task|{
        let target = game.nav.index(task.destination);
        let worker = game.tactics.agent(task.worker_id as u64);
        let distance = worker.map(|a|(a.position()[0]-task.destination[0]).hypot(a.position()[1]-task.destination[1]));
        serde_json::json!({"task":task,"worker":worker_diagnostic(game,task.worker_id as u64),"distance_to_task_m":distance,"target_nav_cell":target.map(|i|game.nav.cells[i]),"target_foot_passable":target.is_some_and(|i|game.nav.passable(i,crate::tactics::MovementClass::Foot))})
    }).collect::<Vec<_>>();
    serde_json::json!({"scope":"CPU_report_only_derived_tasks_on_clone","ore_delivered":game.ore_delivered,"deposits":game.construction.deposits,"cargo":game.construction.cargo,"mines":game.construction.buildings.iter().filter(|b|b.kind==BuildKind::Mine).collect::<Vec<_>>(),"next_task_preview":miners,"preview_error":error})
}
pub(crate) fn capture_candidate(
    game: &Campaign,
    selected: Option<u32>,
) -> Option<(u32, u32, u64, [f32; 2])> {
    let female = selected.is_some_and(|sid| {
        game.ecology
            .captives
            .iter()
            .any(|c| c.team == 0 && c.animal.species_id == sid && c.animal.sex != Sex::Male)
    });
    let male = selected.is_some_and(|sid| {
        game.ecology
            .captives
            .iter()
            .any(|c| c.team == 0 && c.animal.species_id == sid && c.animal.sex != Sex::Female)
    });
    let candidates: Vec<_> = game
        .ecology
        .cohorts
        .iter()
        .filter_map(|c| {
            let species = &game.ecology.species[c.species_id as usize];
            let point = game.cohort_position(c);
            (c.count > 0
                && c.stage == Stage::Adult
                && c.surface_observable
                && c.location_height_mm >= 0
                && !matches!(species.taxon, Taxon::Plant | Taxon::Fungus)
                && species.habitat != Habitat::Water
                && selected.is_none_or(|sid| sid == c.species_id)
                && game.ecology.knowledge[0][c.species_id as usize].observed
                && game.recon.visible(0, point)
                && game
                    .nav
                    .index(point)
                    .is_some_and(|i| game.nav.passable(i, crate::tactics::MovementClass::Foot)))
            .then_some((c, point))
        })
        .collect();
    let mut roles = std::collections::BTreeMap::<u32, (bool, bool)>::new();
    for (c, _) in &candidates {
        let role = roles.entry(c.species_id).or_default();
        role.0 |= c.sex != Sex::Male;
        role.1 |= c.sex != Sex::Female;
    }
    candidates
        .into_iter()
        .filter(|(c, _)| selected.is_some() || roles.get(&c.species_id) == Some(&(true, true)))
        .filter(|(c, _)| {
            !female && !male
                || female && male
                || female && c.sex != Sex::Female
                || male && c.sex != Sex::Male
        })
        .min_by_key(|(c, p)| {
            let distance = (p[0] - TEAM_BASES[0][0]).abs() + (p[1] - TEAM_BASES[0][1]).abs();
            (
                distance as u32,
                game.ecology.species[c.species_id as usize].gestation_ticks,
                c.species_id,
                c.patch,
                c.id,
            )
        })
        .map(|(c, p)| (c.species_id, c.patch, c.id, p))
}
/// Choose a real currently visible dry land point with an existing food web,
/// outside the pen's access radius. This drives the same ordinary click path
/// as a player and gives the handler a short, actual release journey.
pub(crate) fn visible_release_point(game: &Campaign, species: u32) -> Option<(u32, [f32; 2])> {
    let profile = game.ecology.species.get(species as usize)?;
    if profile.habitat == Habitat::Water {
        return None;
    }
    let captive = game
        .ecology
        .captives
        .iter()
        .find(|c| c.team == 0 && c.animal.species_id == species && game.ecology.is_housed(c))?;
    let pen = game.construction.buildings.iter().find(|b| {
        Some(b.id) == captive.housing && b.complete && b.health > 0 && b.kind == BuildKind::Pen
    })?;
    let origin = game
        .construction
        .jobs
        .iter()
        .find(|j| {
            j.order.kind == BuildKind::Pen
                && j.order.team == pen.team
                && j.order.position == pen.position
                && j.phase == JobPhase::Complete
        })
        .map(|j| j.work_position)?;
    let mut food_patches = BTreeSet::new();
    for plant in &game.ecology.plants {
        if plant.live_units() > 0 && profile.food_species.contains(&plant.species_id) {
            food_patches.insert(plant.patch);
        }
    }
    for animal in &game.ecology.cohorts {
        if animal.count > 0 && profile.food_species.contains(&animal.species_id) {
            food_patches.insert(animal.patch);
        }
    }
    let source = game.nav.index(origin)?;
    let source_x = source % game.nav.width;
    let source_y = source / game.nav.width;
    let mut best = None;
    for y in source_y.saturating_sub(10)..=(source_y + 10).min(game.nav.height - 1) {
        for x in source_x.saturating_sub(10)..=(source_x + 10).min(game.nav.width - 1) {
            let cell = y * game.nav.width + x;
            if !game.nav.passable(cell, crate::tactics::MovementClass::Foot)
                || game.nav.cells[cell].effective_depth_mm() > 100
            {
                continue;
            }
            let center = game.nav.center_mm(cell);
            let point = [center[0] as f32 * 0.001, center[1] as f32 * 0.001];
            let distance = (point[0] - origin[0]).hypot(point[1] - origin[1]);
            let patch = patch_at(point) as u32;
            if !(24. ..=160.).contains(&distance)
                || !food_patches.contains(&patch)
                || !game.ecology.visible_patches[0][patch as usize]
                || !game.recon.visible(0, point)
            {
                continue;
            }
            let candidate = ((distance * 1000.) as u32, cell, patch, point);
            if best
                .as_ref()
                .is_none_or(|old: &(u32, usize, u32, [f32; 2])| {
                    (candidate.0, candidate.1) < (old.0, old.1)
                })
            {
                best = Some(candidate);
            }
        }
    }
    best.map(|(_, _, patch, point)| (patch, point))
}
fn milestone(
    records: &mut Vec<serde_json::Value>,
    tick: u64,
    name: &str,
    details: serde_json::Value,
) {
    let record = serde_json::json!({"tick":tick,"stage":name,"details":details});
    logging::log("CAMPAIGN_CHECK_STAGE", &record.to_string());
    records.push(record);
}
fn keep_error(records: &mut Vec<serde_json::Value>, tick: u64, action: &str, error: String) {
    if records.len() >= 10 {
        records.remove(0);
    }
    records.push(serde_json::json!({"tick":tick,"action":action,"error":error}));
}

fn verify_save_continuity(game: &mut Campaign) -> Result<serde_json::Value, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let folder: PathBuf = std::env::temp_dir().join(format!(
        "last-world-campaign-check-{}-{timestamp}",
        std::process::id()
    ));
    std::fs::create_dir(&folder).map_err(|e| e.to_string())?;
    let path = folder.join("campaign.json");
    let before_tick = game.processed_tick;
    game.save(&path)?;
    let bytes = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
    let mut restored = Campaign::load(&path)?;
    let original = CampaignSave {
        version: 1,
        terrain: game.terrain.snapshot(),
        data: game.data.clone(),
    };
    let loaded = CampaignSave {
        version: 1,
        terrain: restored.terrain.snapshot(),
        data: restored.data.clone(),
    };
    if serde_json::to_vec(&original).map_err(|e| e.to_string())?
        != serde_json::to_vec(&loaded).map_err(|e| e.to_string())?
    {
        return Err(format!(
            "Campaign save/load state differs at tick{before_tick}; evidence retained at{}",
            folder.display()
        ));
    }
    for _ in 0..40 {
        game.advance(Duration::from_millis(50))?;
        restored.advance(Duration::from_millis(50))?;
    }
    game.validate()?;
    restored.validate()?;
    let original = CampaignSave {
        version: 1,
        terrain: game.terrain.snapshot(),
        data: game.data.clone(),
    };
    let loaded = CampaignSave {
        version: 1,
        terrain: restored.terrain.snapshot(),
        data: restored.data.clone(),
    };
    if serde_json::to_vec(&original).map_err(|e| e.to_string())?
        != serde_json::to_vec(&loaded).map_err(|e| e.to_string())?
    {
        return Err(format!(
            "Campaign next40ticks differ after reload at tick{before_tick}; evidence retained at{}",
            folder.display()
        ));
    }
    std::fs::remove_dir_all(&folder).map_err(|e| e.to_string())?;
    Ok(
        serde_json::json!({"saved_tick":before_tick,"compared_through_tick":game.processed_tick,"save_bytes":bytes,"exact_serialized_state_match":true,"fixture_files_removed_after_verification":true}),
    )
}
