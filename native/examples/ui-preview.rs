//! Diagnostic only: actual native UI composition over transparency, without a
//! graphics device, world rendering or native screenshot/performance claims.
use last_world_native::{campaign::Campaign, gpu::hud, ui_interaction::UiState};
use std::{
    fs::File,
    path::PathBuf,
    time::{Duration, Instant},
};

fn main() {
    last_world_native::logging::install();
    if let Err(error) = run() {
        last_world_native::logging::log("UI_PREVIEW_ERROR", &error);
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("Usage: cargo run --example ui-preview -- OUTPUT_DIRECTORY [TICKS]")?;
    let ticks = std::env::args()
        .nth(2)
        .map(|s| s.parse::<u64>().map_err(|e| e.to_string()))
        .transpose()?
        .unwrap_or(40);
    if ticks > 2400 {
        return Err(
            "UI composition diagnostic supports0..2400ticks; it is not a long-war qualification"
                .into(),
        );
    }
    std::fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    // Explicit diagnostic population only. The playable game and combined
    // military/civilian qualification retain their own full selected counts.
    let mut campaign = Campaign::new(78421, 64, 12)?;
    campaign.automation()?;
    for _ in 0..ticks {
        campaign.advance(Duration::from_millis(50))?;
    }
    campaign.validate()?;
    campaign.cycle_species();
    let mut ui = UiState::default();
    ui.selection.extend(
        campaign
            .tactics
            .agents
            .iter()
            .filter(|a| a.team == 0 && a.alive() && campaign.recon.visible(0, a.position()))
            .take(8)
            .map(|a| a.id),
    );
    let mut files = Vec::new();
    for size in [[1366, 768], [1920, 1080], [2560, 1440]] {
        for (label, presentation) in ui.capture_presentations(&campaign, size, 2000) {
            let mut model = campaign.hud();
            model.ui = Some(presentation);
            let begin = Instant::now();
            let image = hud::rasterize(&model, size[0], size[1]);
            let composition_ms = begin.elapsed().as_secs_f64() * 1000.;
            let file_name = format!("cpu-ui-{}x{}-{}.png", size[0], size[1], label);
            let path = output.join(&file_name);
            let mut encoder = png::Encoder::new(
                File::create(&path).map_err(|e| e.to_string())?,
                image.width,
                image.height,
            );
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .map_err(|e| e.to_string())?
                .write_image_data(&image.rgba)
                .map_err(|e| e.to_string())?;
            let painted = image.rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
            files.push(serde_json::json!({"file":file_name,"size":size,"scene":label,"scope":"CPU_UI_composition_not_native_world_screenshot","alpha_painted_percent":painted as f64*100./(image.width as f64*image.height as f64),"cold_diagnostic_composition_ms":composition_ms,"not_steady_UI_budget_measurement":true,"GPU_device_created":false,"world_overlay_instances_not_rasterized_here":hud::world_quads(&model,size[0],size[1]).len()}));
        }
    }
    let report = serde_json::json!({"kind":"CPU_UI_composition_not_native_world_screenshot","graphics_device_created":false,"source":"actual Campaign/UiState/hud::rasterize; no fabricated gameplay stocks","diagnostic_initial_military":64,"diagnostic_initial_workers":12,"seed":78421,"ticks":campaign.clock.tick(),"UI_knowledge_filter":"ordinary UiState.presentation", "gameplay_performance_qualification":false,"files":files});
    std::fs::write(
        output.join("cpu-ui-composition-manifest.json"),
        serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}", report);
    Ok(())
}
