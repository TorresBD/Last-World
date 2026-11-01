//! Real campaign + the production renderer. Read-only opening evidence, no speed suite.
use last_world_native::{campaign::Campaign,gpu::{Gpu,hud},ui_interaction::{UiState,UiAction,CommandCategory}};
use std::path::Path;
fn main()->Result<(),String> {pollster::block_on(run())}
async fn run()->Result<(),String>{
    last_world_native::logging::install();
    let directory=Path::new("evidence/opening-hud");
    std::fs::create_dir_all(directory).map_err(|e|e.to_string())?;
    let (mut gpu,_,terrain)=Gpu::new_campaign(None,78421,1920,1080,Some(wgpu::Backends::DX12)).await?;
    let mut campaign=Campaign::new_crash_landing(terrain)?;
    let mut ui=UiState::default();ui.begin_landing(&campaign);
    gpu.params.view=[campaign.camera[0],campaign.camera[1],0.,0.];
    gpu.params.screen=[1920.,1080.,campaign.camera[2],0.];
    gpu.params.misc[1]=1.;gpu.params.misc[3]=-1.;
    gpu.upload_campaign_terrain(&mut campaign)?;
    gpu.upload_campaign_agents(&campaign.draw_agents(0.))?;
    let objects=campaign.visual_memory.as_ref().unwrap().objects.iter().flatten().copied().collect::<Vec<_>>();
    gpu.upload_campaign_objects(&objects,objects.len())?;
    gpu.upload_campaign_intel(campaign.intel())?;
    let mut captures=Vec::new();
    for stage in 0..4 {
        if stage==1 {ui.dismiss_modal();}
        if stage==2 {ui.dismiss_modal();ui.presentation(&campaign,[1920,1080],0);ui.activate(UiAction::SelectBuilders,campaign.camera,0);}
        if stage==3 {ui.activate(UiAction::ChooseCategory(CommandCategory::Recon),campaign.camera,0);}
        for (width,height) in [(1366,768),(1920,1080),(2560,1440)] {
            gpu.params.screen[0]=width as f32;gpu.params.screen[1]=height as f32;
            let mut model=campaign.hud();model.ui=Some(ui.presentation(&campaign,[width,height],0));
            gpu.set_hud(&model,width,height)?;
            let target=gpu.target(width,height);
            gpu.render(&target.create_view(&Default::default()),0);
            let file=directory.join(format!("opening-{stage}-{width}x{height}.png"));
            gpu.capture(&target,&file)?;
            captures.push(file.to_string_lossy().into_owned());
            // The same layout/hit-test model is used by the actual input loop.
            let _=hud::layout(&model,width,height);
        }
    }
    if !campaign.paused||campaign.clock.tick()!=0{return Err("Opening evidence advanced the clock".into());}
    std::fs::write(directory.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({
        "source":"production Campaign and Gpu renderer","adapter":format!("{:?}",gpu.info),
        "tick":campaign.clock.tick(),"paused":campaign.paused,"captures":captures,
        "performance_qualification":false,"player_PC_visual_review":false
    })).unwrap()).map_err(|e|e.to_string())?;
    Ok(())
}
