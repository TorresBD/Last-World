//! Bounded crowd regression, not full battle or performance qualification.
use last_world_native::{battle_ai::BattleDirector, campaign::Campaign};
use serde_json::json;
use std::time::Duration;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut game = Campaign::new_battle_benchmark(78421, 512, 2000)?;
    let mut director = BattleDirector::default();
    while game.processed_tick < 6000 {
        director.drive(&mut game)?;
        game.advance_benchmark(Duration::from_millis(50), 4)?;
        if game.processed_tick % 600 == 0 {
            let jobs:Vec<_>=game.construction.jobs.iter().map(|j| {
                let worker=j.worker.and_then(|id|game.tactics.agents.iter().find(|a| a.id==id as u64));
                json!({"id":j.id,"team":j.order.team,"kind":j.order.kind.name(),"phase":j.phase,"work":j.work_done,"worker":j.worker,"position":worker.map(|a|a.position()),"pickup":j.pickup_position,"work_position":j.work_position})
            }).collect();
            println!(
                "{}",
                json!({"kind":"crowd_regression_only_reduced_military_not_battle_qualification","tick":game.processed_tick,"military":512,"workers":2000,"completed_jobs":game.completed_jobs,"jobs":jobs})
            );
        }
    }
    game.validate()?;
    let report = director.report(&game);
    println!(
        "{}",
        json!({"kind":"construction_crowd_regression_result_not_performance","battle_report":report})
    );
    for team in 0..2 {
        if report["paid_completed_jobs"][team].as_u64().unwrap_or(0) < 2 {
            return Err(format!("Faction{team} failed paid construction crowd regression").into());
        }
    }
    Ok(())
}
