//! Core-correctness run of the SAME battle scenario; no GPU and no speed claim.
//! Fixed50ms elapsed inputs intentionally unpace the rule simulation. The
//! windowed production test remains the authority for graphics/performance.
use last_world_native::campaign_benchmark;
use serde_json::json;
use std::{path::PathBuf, time::Duration};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("Pass a new owned output directory")?;
    if output.exists() {
        return Err(
            "Use a fresh output directory; correctness fixture never replays an old receipt".into(),
        );
    }
    std::fs::create_dir_all(&output)?;
    let (mut game, mut session) = campaign_benchmark::prepare("window-battle-20m-4x", &output)?;
    let mut next = 600;
    loop {
        let progress = session.step(&mut game, Duration::from_millis(50));
        if game.processed_tick >= next {
            let jobs:Vec<_>=game.construction.jobs.iter().map(|j|json!({"id":j.id,"team":j.order.team,"kind":j.order.kind.name(),"phase":j.phase,"work":j.work_done,"required":j.work_required,"worker":j.worker,"work_position":j.work_position})).collect();
            println!(
                "{}",
                json!({"kind":"core_correctness_progress_not_speed_measurement","tick":game.processed_tick,"shots":game.tactics.totals.fired_ammunition,"deaths":game.tactics.totals.deaths,"completed_jobs":game.completed_jobs,"cargo_records":game.construction.cargo.len(),"jobs":jobs})
            );
            next = game.processed_tick + 600;
        }
        match progress {
            Ok(p) if p.done => break,
            Ok(_) => {}
            Err(error) => {
                session.fail(error);
                break;
            }
        }
    }
    let mut report = session.finish(&mut game)?;
    report["kind"] =
        json!("fixed_elapsed_20minute_battle_core_correctness_not_wall_speed_measurement");
    report["performance_qualification"] = json!(false);
    report["fixed_elapsed_input_seconds"] = json!(0.05);
    report["requested_rate_sustained"] = serde_json::Value::Null;
    report["no_gpu_renderer_executed"] = json!(true);
    report["passed"] = json!(false);
    report["status"] = json!(if report["core_phase_passed"] == true {
        "core_passed_native_window_still_required"
    } else {
        "core_failed"
    });
    std::fs::write(
        output.join("battle-core-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "{}",
        json!({"kind":"battle_core_result","core_passed":report["core_phase_passed"],"tick":game.processed_tick,"failure":report["failure"],"battle_scenario":report["battle_scenario"],"graphics_and_performance_qualified":false})
    );
    if report["core_phase_passed"] != true {
        return Err("Battle core requirements failed; full report retained".into());
    }
    Ok(())
}
