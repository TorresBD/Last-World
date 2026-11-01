//! Native campaign input/render loop. Gameplay remains in the Campaign resource;
//! the existing GPU device receives only display copies, never another authority.
#![cfg(not(target_arch = "wasm32"))]
use crate::{
    campaign::Campaign,
    clock::{SimulationSpeed, TACTICAL_HZ, TICK_NANOS},
    frame_profile::{FrameProfile, FrameWork},
    gpu::{Gpu, hud},
    logging,
    metrics::FrameTimings,
    tactics::Role,
    terrain::{HH, WW},
    ui_interaction::{
        CommandCategory, Layer, Modifiers, PanelKind, UiAction, UiCommand, UiState, selectables,
    },
};
use bevy::prelude::*;
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use web_time::Instant;
use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

const MILITARY: u32 = 10_000;
const WORKERS: u32 = 2_000;
const CHECK_FINISH_TICK: u64 = 2_400;
const SAVE_PATH: &str = "saves/last-world.json";
type Outcome = Arc<Mutex<Option<Result<(), String>>>>;

fn milliseconds(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() * 1000.
}
fn position(camera: [f32; 3], cursor: [f64; 2], width: u32, height: u32) -> [f32; 2] {
    [
        camera[0] + (cursor[0] as f32 - width as f32 * 0.5) / camera[2],
        camera[1] + (cursor[1] as f32 - height as f32 * 0.5) / camera[2],
    ]
}
fn in_world(p: [f32; 2]) -> bool {
    p.iter().all(|v| v.is_finite())
        && p[0] >= 0.
        && p[1] >= 0.
        && p[0] < WW as f32
        && p[1] < HH as f32
}

fn ui_point(cursor: [f64; 2], width: u32, height: u32) -> [f32; 2] {
    let (w, h) = hud::canvas_size(width, height);
    [
        cursor[0] as f32 * w as f32 / width.max(1) as f32,
        cursor[1] as f32 * h as f32 / height.max(1) as f32,
    ]
}
fn campaign_hud(
    c: &Campaign,
    ui: &mut UiState,
    width: u32,
    height: u32,
    now_ms: u64,
) -> hud::HudModel {
    let mut model = c.hud();
    let (w, h) = hud::canvas_size(width, height);
    model.ui = Some(ui.presentation(c, [w, h], now_ms));
    model
}
fn ui_evidence_frames(c: &Campaign, ui: &UiState, now_ms: u64) -> Vec<crate::gpu::UiEvidenceFrame> {
    let mut frames = Vec::new();
    for [width, height] in [[1366, 768], [1920, 1080], [2560, 1440]] {
        for (label, presentation) in ui.capture_presentations(c, [width, height], now_ms) {
            let mut model = c.hud();
            model.ui = Some(presentation);
            frames.push(crate::gpu::UiEvidenceFrame {
                width,
                height,
                label,
                model,
            });
        }
    }
    frames
}
fn apply_commands(c: &mut Campaign, commands: Vec<UiCommand>, last_clock: &mut Instant) -> bool {
    let mut loaded = false;
    for command in commands {
        if matches!(
            command,
            UiCommand::Pause | UiCommand::Speed(_) | UiCommand::Save | UiCommand::Load
        ) {
            if let Err(error) = settle_input(c, last_clock) {
                reject(c, "Clock input", error);
                continue;
            }
        }
        let result = match &command {
            UiCommand::CameraJump { position, zoom } => {
                c.camera[0] = position[0].clamp(0., WW as f32);
                c.camera[1] = position[1].clamp(0., HH as f32);
                if let Some(zoom) = zoom {
                    c.camera[2] = zoom.clamp(0.12, 6.);
                }
                Ok(())
            }
            UiCommand::Pause => {
                c.paused = !c.paused;
                Ok(())
            }
            UiCommand::Speed(value) => SimulationSpeed::try_from(*value)
                .map(|speed| c.clock.set_speed(speed))
                .map_err(|e| e.to_string()),
            UiCommand::Save => c
                .save(Path::new(SAVE_PATH))
                .map(|_| c.status = "World saved".into()),
            UiCommand::Load => Campaign::load(Path::new(SAVE_PATH)).map(|game| {
                *c = game;
                *last_clock = Instant::now();
                c.status = "Saved world restored".into();
                loaded = true;
            }),
            UiCommand::RecruitCamera => c.recruit_camera(),
            UiCommand::CycleSpecies => {
                c.cycle_species();
                Ok(())
            }
            UiCommand::Explode { position } => c.explode(*position, 0),
            _ => c.apply_ui_command(&command),
        };
        match result {
            Ok(()) => logging::log(
                "UI_COMMAND",
                &format!("tick={}; {:?}", c.clock.tick(), command),
            ),
            Err(error) => reject(c, "Command", error),
        }
    }
    loaded
}

/// Accrue old-state wall time without committing unexecuted CPU ticks. The
/// Campaign advance then consumes debt exactly once in its normal fixed path.
fn settle_input(c: &mut Campaign, last: &mut Instant) -> Result<(), String> {
    let now = Instant::now();
    let paused = c.paused || c.winner.is_some();
    c.clock
        .advance(now - *last, paused, 0)
        .map_err(|e| e.to_string())?;
    *last = now;
    Ok(())
}
fn reject(c: &mut Campaign, action: &str, error: String) {
    c.status = format!("{action}: {error}");
    logging::log(
        "ACTION_REJECTED",
        &format!("{action}; tick={}; {error}", c.clock.tick()),
    );
}
fn stop(outcome: &Outcome, elwt: &winit::event_loop::ActiveEventLoop, error: String) {
    *outcome.lock().expect("campaign outcome lock") = Some(Err(error));
    elwt.exit();
}
fn save_check_report(path: &str, report: &serde_json::Value) -> Result<(), String> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[allow(deprecated)]
pub async fn windowed(
    backends: Option<wgpu::Backends>,
    speed: SimulationSpeed,
    automated: bool,
) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Last World · generating native world")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.)),
            )
            .map_err(|e| e.to_string())?,
    );
    let size = window.inner_size();
    let seed = if automated {
        78421
    } else {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        (nonce ^ nonce.rotate_left(37)) as u32
    };
    let (mut gpu, surface, terrain) = Gpu::new_campaign(
        Some(window.clone()),
        seed,
        size.width.max(1),
        size.height.max(1),
        backends,
    )
    .await?;
    let surface = surface.ok_or("Native campaign surface missing")?;
    let mut config = surface
        .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
        .ok_or("No campaign surface configuration")?;
    config.format = gpu.format;
    config.present_mode = wgpu::PresentMode::AutoVsync;
    surface.configure(&gpu.device, &config);
    let mut campaign = if automated {
        Campaign::new_from_terrain(terrain, MILITARY, WORKERS)?
    } else { Campaign::new_crash_landing(terrain)? };
    campaign.clock.set_speed(speed);
    if automated {
        campaign.automation()?;
    }
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).insert_resource(campaign);
    let ui_started = Instant::now();
    let mut ui =
        UiState::load_layout(Path::new("settings/ui-layout.json")).unwrap_or_else(|error| {
            logging::log("UI_LAYOUT_ERROR", &error);
            UiState::default()
        });
    let (ui_w, ui_h) = hud::canvas_size(config.width, config.height);
    ui.resize_layout([ui_w, ui_h]);
    if !automated { ui.begin_landing(&app.world().resource::<Campaign>()); }
    let mut saved_prefs = ui.prefs.clone();
    let mut next_ui_save = Instant::now();
    let mut hud_model = campaign_hud(
        app.world().resource::<Campaign>(),
        &mut ui,
        config.width,
        config.height,
        0,
    );
    gpu.set_hud(&hud_model, config.width, config.height)?;
    logging::log("CAMPAIGN_READY",&serde_json::json!({"mode":if automated{"automated_campaign_check"}else{"playable_campaign"},"seed":seed,"initial_military":MILITARY,"initial_workers":WORKERS,"map_meters":[WW,HH],"cpu_authority":true,"gpu_agent_simulation_enabled":false,"requested_speed":speed.multiplier(),"backend":format!("{:?}",gpu.info.backend),"adapter":gpu.info.name,"controls":"Drag roads/trenches/bridges; left HUD/tool + world click; right move; WASD/arrows pan; wheel zoom; Space pause; F1/F2/F3 speeds; F5 save; F9 load; Tab observed species; R radar overlay; Q recruit camera trooper; X local explosion","automated_inputs_locked":automated}).to_string());
    let prefix = format!(
        "campaign-check-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let report_path = format!("{prefix}-report.json");
    let report_after_loop = report_path.clone();
    let outcome: Outcome = Arc::new(Mutex::new(None));
    let outcome_after_loop = outcome.clone();
    let mut last_clock = Instant::now();
    let mut last_work_end = last_clock;
    let mut last_frame = last_clock;
    let mut last_pan = last_clock;
    let mut last_flash_wall = last_clock;
    let mut measured = last_clock;
    let mut next_hud = last_clock;
    let mut hud_dirty = true;
    let mut previous_tick = 0;
    let mut frame_id = 0u64;
    let mut accepted_frames = 0u64;
    let mut measured_frames = 0u64;
    let mut interval_samples = FrameTimings::new(8192);
    let mut all_intervals = FrameTimings::new(8192);
    let mut stage_samples = FrameProfile::default();
    let mut all_stages = FrameProfile::default();
    let mut input_ms = 0.;
    let mut cursor = [config.width as f64 * 0.5, config.height as f64 * 0.5];
    let mut pan_keys = [false; 4];
    let mut input_units = selectables(app.world().resource::<Campaign>());
    let mut modifiers = Modifiers::default();
    let mut focused = true;
    let mut load_reset = false;
    let mut minimized = false;
    let mut captured = [false; 3];
    let mut ui_captures: Vec<String> = Vec::new();
    event_loop.run(move |event,elwt| match event {
        Event::WindowEvent{event,..}=>match event {
            WindowEvent::CloseRequested=>{
                ui.cancel_pointer();
                if !automated {if let Err(error)=ui.save_layout(Path::new("settings/ui-layout.json")){logging::log("UI_LAYOUT_ERROR",&error);}}
                logging::log("SESSION_END",if automated{"Campaign check window closed before automatic completion"}else{"Campaign window closed"});
                *outcome.lock().expect("campaign outcome lock")=Some(if automated{Err("Campaign check window closed before all scheduled steps/captures completed".into())}else{Ok(())});elwt.exit();
            }
            WindowEvent::Resized(size)=>{minimized=size.width==0||size.height==0;if !minimized{config.width=size.width;config.height=size.height;surface.configure(&gpu.device,&config);let(w,h)=hud::canvas_size(config.width,config.height);ui.resize_layout([w,h]);hud_dirty=true;logging::log("RESIZE",&format!("campaign{}x{}physicalpixels",size.width,size.height));}}
            WindowEvent::ModifiersChanged(value)=>{let value=value.state();modifiers=Modifiers{shift:value.shift_key(),control:value.control_key(),alt:value.alt_key()};}
            WindowEvent::Focused(value)=>{focused=value;if !value{pan_keys=[false;4];ui.cancel_pointer();modifiers=Modifiers::default();}}
            WindowEvent::CursorMoved{position:point,..} if !automated=>{
                let start=Instant::now();cursor=[point.x,point.y];let hit=hud::hit_test(&hud_model,config.width,config.height,cursor[0],cursor[1]);let screen=ui_point(cursor,config.width,config.height);let(w,h)=hud::canvas_size(config.width,config.height);let mut c=app.world_mut().resource_mut::<Campaign>();let world=position(c.camera,cursor,config.width,config.height);let visible=&input_units;let commands=ui.pointer_move(hit,screen,world,c.camera,[w as f32,h as f32],visible);load_reset|=apply_commands(&mut c,commands,&mut last_clock);hud_dirty|=ui.dirty;input_ms+=milliseconds(start.elapsed());
            }
            WindowEvent::MouseWheel{delta,..} if !automated=>{
                let input_start=Instant::now();let dy=match delta{MouseScrollDelta::LineDelta(_,y)=>y,MouseScrollDelta::PixelDelta(p)=>p.y as f32/80.};let hit=hud::hit_test(&hud_model,config.width,config.height,cursor[0],cursor[1]);
                if !ui.wheel(hit,dy){ui.cancel_pointer();let mut c=app.world_mut().resource_mut::<Campaign>();let before=position(c.camera,cursor,config.width,config.height);c.camera[2]=(c.camera[2]*(dy*0.15).exp()).clamp(0.12,6.);let after=position(c.camera,cursor,config.width,config.height);c.camera[0]=(c.camera[0]+before[0]-after[0]).clamp(0.,WW as f32);c.camera[1]=(c.camera[1]+before[1]-after[1]).clamp(0.,HH as f32);}
                hud_dirty=true;input_ms+=milliseconds(input_start.elapsed());
            }
            WindowEvent::MouseInput{state,button,..} if !automated=>{
                if button!=MouseButton::Left&&button!=MouseButton::Right{return}let start=Instant::now();let now_ms=ui_started.elapsed().as_millis() as u64;let hit=hud::hit_test(&hud_model,config.width,config.height,cursor[0],cursor[1]);let screen=ui_point(cursor,config.width,config.height);let(w,h)=hud::canvas_size(config.width,config.height);let mut c=app.world_mut().resource_mut::<Campaign>();let world=position(c.camera,cursor,config.width,config.height);let visible=selectables(&c);
                let commands=if state==ElementState::Pressed{ui.pointer_down(hit,screen,world,button==MouseButton::Right,modifiers,c.camera,[w as f32,h as f32],&visible,now_ms)}else if button==MouseButton::Left{ui.pointer_up(hit,screen,world,modifiers,c.camera,[w as f32,h as f32],&visible,now_ms)}else{vec![]};
                load_reset|=apply_commands(&mut c,commands,&mut last_clock);hud_dirty=true;input_ms+=milliseconds(start.elapsed());
            }
            WindowEvent::KeyboardInput{event,..} if !automated=>{
                let start=Instant::now();if let PhysicalKey::Code(key)=event.physical_key{let down=event.state==ElementState::Pressed;
                    if ui.modal.is_some() {if down&&!event.repeat&&matches!(key,KeyCode::Escape|KeyCode::Enter){ui.dismiss_modal();}pan_keys=[false;4];hud_dirty=true;input_ms+=milliseconds(start.elapsed());return;}
                    if ui.help_search_focused&&down {match key{KeyCode::Escape=>ui.escape(),KeyCode::Backspace=>ui.backspace(),_=>{if !modifiers.control&&!modifiers.alt{if let Some(text)=&event.text{ui.text_input(text);}}}}hud_dirty=true;input_ms+=milliseconds(start.elapsed());return;}
                    let pan=if !down||(!modifiers.control&&!modifiers.alt){match key{KeyCode::KeyW|KeyCode::ArrowUp=>Some(0),KeyCode::KeyS|KeyCode::ArrowDown=>Some(1),KeyCode::KeyA|KeyCode::ArrowLeft=>Some(2),KeyCode::KeyD|KeyCode::ArrowRight=>Some(3),_=>None}}else{None};if let Some(i)=pan{pan_keys[i]=down;}
                    if down&&!event.repeat{let now_ms=ui_started.elapsed().as_millis() as u64;let mut c=app.world_mut().resource_mut::<Campaign>();let mut commands=Vec::new();let digit=match key{KeyCode::Digit1=>Some(1),KeyCode::Digit2=>Some(2),KeyCode::Digit3=>Some(3),KeyCode::Digit4=>Some(4),KeyCode::Digit5=>Some(5),KeyCode::Digit6=>Some(6),KeyCode::Digit7=>Some(7),KeyCode::Digit8=>Some(8),KeyCode::Digit9=>Some(9),_=>None};
                        if let Some(digit)=digit{commands=ui.group_key(digit,modifiers,c.camera,now_ms,&selectables(&c));}
                        else{let action=match key{
                            KeyCode::F4 if !modifiers.alt=>Some(UiAction::ChooseTool(0)),KeyCode::F6 if !modifiers.alt=>Some(UiAction::ChooseTool(1)),KeyCode::F7 if !modifiers.alt=>Some(UiAction::ChooseTool(10)),
                            KeyCode::Escape=>{ui.escape();None},KeyCode::Space=>{commands.push(UiCommand::Pause);None},KeyCode::F1=>{commands.push(UiCommand::Speed(1));None},KeyCode::F2=>{commands.push(UiCommand::Speed(2));None},KeyCode::F3=>{commands.push(UiCommand::Speed(4));None},KeyCode::F5=>{commands.push(UiCommand::Save);None},KeyCode::F9=>{commands.push(UiCommand::Load);None},KeyCode::Tab=>{commands.push(UiCommand::CycleSpecies);None},KeyCode::KeyQ=>{commands.push(UiCommand::RecruitCamera);None},KeyCode::KeyX=>{let at=position(c.camera,cursor,config.width,config.height);if in_world(at){commands.push(UiCommand::Explode{position:at});}None},
                            KeyCode::Delete=>Some(UiAction::Stop),KeyCode::KeyB=>Some(UiAction::ChooseCategory(CommandCategory::Build)),KeyCode::KeyN=>Some(UiAction::ChooseCategory(CommandCategory::Naval)),KeyCode::KeyD if modifiers.control=>Some(UiAction::ChooseCategory(CommandCategory::Defense)),KeyCode::KeyE if modifiers.control=>Some(UiAction::ChooseCategory(CommandCategory::Economy)),KeyCode::KeyR if modifiers.control=>Some(UiAction::ChooseCategory(CommandCategory::Recon)),KeyCode::KeyE=>Some(UiAction::OpenPanel(PanelKind::Ecology)),KeyCode::KeyF=>Some(UiAction::OpenPanel(PanelKind::FoodWeb)),KeyCode::KeyI=>Some(UiAction::OpenPanel(PanelKind::Logistics)),KeyCode::KeyJ=>Some(UiAction::OpenPanel(PanelKind::Damage)),KeyCode::KeyH=>Some(UiAction::OpenPanel(PanelKind::Help)),KeyCode::KeyP=>Some(UiAction::OpenPanel(PanelKind::Settings)),KeyCode::KeyR=>Some(UiAction::ToggleLayer(Layer::Radar)),KeyCode::KeyC=>Some(UiAction::ToggleLayer(Layer::Cameras)),KeyCode::KeyO=>Some(UiAction::ToggleLayer(Layer::Roads)),KeyCode::KeyL=>Some(UiAction::ToggleLayer(Layer::Supply)),KeyCode::KeyU=>Some(UiAction::ToggleLayer(Layer::Resources)),KeyCode::KeyV=>Some(UiAction::ToggleLayer(Layer::Ecology)),KeyCode::KeyG=>Some(UiAction::ToggleLayer(Layer::Damage)),KeyCode::KeyK=>Some(UiAction::ToggleLayer(Layer::Fauna)),KeyCode::KeyT=>Some(UiAction::ToggleLayer(Layer::Territory)),_=>None};if let Some(action)=action{commands.extend(ui.activate(action,c.camera,now_ms));}}
                        load_reset|=apply_commands(&mut c,commands,&mut last_clock);hud_dirty=true;
                    }
                }input_ms+=milliseconds(start.elapsed());
            }
            WindowEvent::RedrawRequested=>{
                let begin=Instant::now();
                frame_id=frame_id.checked_add(1).expect("campaign frame diagnostic counter overflow");
                let mut work=FrameWork{frame:frame_id,event_gap_ms:milliseconds(begin-last_work_end),input_ms:std::mem::take(&mut input_ms),..Default::default()};
                let mut c=app.world_mut().resource_mut::<Campaign>();
                let flash_elapsed=(begin-last_flash_wall).as_secs_f32();
                last_flash_wall=begin;
                if c.flash>=0. {c.flash+=flash_elapsed;if c.flash>=0.44{c.flash=-1.;}}
                if load_reset{load_reset=false;ui.reset_after_load();last_flash_wall=begin;previous_tick=c.clock.tick();measured=begin;measured_frames=0;interval_samples.clear();stage_samples.clear();hud_dirty=true;}
                if !automated&&focused&&ui.modal.is_none()&&!ui.world_dragging()&&!ui.help_search_focused{let dt=(begin-last_pan).as_secs_f32().min(0.25);let distance=600.*dt/c.camera[2];let(w,h)=hud::canvas_size(config.width,config.height);let edge=ui.edge_pan(ui_point(cursor,config.width,config.height),[w as f32,h as f32]);c.camera[0]=(c.camera[0]+(pan_keys[3] as u8 as f32-pan_keys[2] as u8 as f32+edge[0])*distance).clamp(0.,WW as f32);c.camera[1]=(c.camera[1]+(pan_keys[1] as u8 as f32-pan_keys[0] as u8 as f32+edge[1])*distance).clamp(0.,HH as f32);}
                last_pan=begin;
                if !automated&&begin>=next_ui_save&&ui.prefs!=saved_prefs {match ui.save_layout(Path::new("settings/ui-layout.json")){Ok(())=>saved_prefs=ui.prefs.clone(),Err(error)=>logging::log("UI_LAYOUT_ERROR",&error)}next_ui_save=begin+Duration::from_secs(2);}
                let sim_begin=Instant::now();
                let elapsed=sim_begin-last_clock;
                last_clock=sim_begin;
                let steps=match c.advance(elapsed){Ok(n)=>n,Err(error)=>{stop(&outcome,elwt,format!("Campaign fixed-tick update: {error}"));return;}};
                if steps>0||hud_dirty {input_units=selectables(&c);}
                work.simulation_ms=milliseconds(sim_begin.elapsed());
                work.tick=c.clock.tick();
                // The CPU world progresses independently of GPU-owned display.
                // A failed surface acquisition never discards committed CPU ticks.
                if minimized {
                    work.total_work_ms=milliseconds(begin.elapsed());
                    stage_samples.record(&work);all_stages.record(&work);
                    if work.slow(){logging::log("SLOW_FRAME",&serde_json::to_string(&work).unwrap());}
                    last_work_end=Instant::now();
                    return;
                }
                let acquire=Instant::now();
                let frame=match surface.get_current_texture(){
                    Ok(frame)=>frame,
                    Err(wgpu::SurfaceError::Lost|wgpu::SurfaceError::Outdated)=>{
                        surface.configure(&gpu.device,&config);
                        work.surface_acquire_ms=milliseconds(acquire.elapsed());
                        work.total_work_ms=milliseconds(begin.elapsed());
                        stage_samples.record(&work);all_stages.record(&work);
                        logging::log("SURFACE_RECOVERY","Campaign surface reconfigured; CPU ticks retained");
                        if work.slow(){logging::log("SLOW_FRAME",&serde_json::to_string(&work).unwrap());}
                        last_work_end=Instant::now();return;
                    }
                    Err(error)=>{logging::log("SURFACE_ERROR",&error.to_string());stop(&outcome,elwt,error.to_string());return;}
                };
                work.surface_acquire_ms=milliseconds(acquire.elapsed());
                let interval=(begin-last_frame).as_secs_f64();last_frame=begin;
                interval_samples.record_seconds(interval);all_intervals.record_seconds(interval);
                let upload=Instant::now();
                let flash_origin=c.flash_position.unwrap_or([c.camera[0],c.camera[1]]);
                gpu.params.view=[c.camera[0],c.camera[1],flash_origin[0],flash_origin[1]];
                gpu.params.screen=[config.width as f32,config.height as f32,c.camera[2],c.clock.seconds() as f32];
                gpu.params.misc[1]=1.;
                gpu.params.misc[2]=if ui.prefs.layers.contains(&Layer::Radar){1.}else{0.};
                gpu.params.misc[3]=c.flash;
                let alpha=c.clock.remainder_nanos() as f32/TICK_NANOS as f32;
                let agents=c.draw_agents(alpha);
                let render_count=agents.len();
                let upload_result=(||{
                    let mut bytes=gpu.upload_campaign_terrain(&mut c)?;
                    let objects = c.data.visual_memory.as_ref().map(|m| m.objects.iter().flatten().copied().collect::<Vec<_>>()).map(Ok).unwrap_or_else(|| c.draw_objects())?;
                    bytes+=gpu.upload_campaign_agents(&agents)?;
                    bytes+=gpu.upload_campaign_objects(&objects,objects.len())?;
                    bytes+=gpu.upload_campaign_intel(c.intel())?;
                    if hud_dirty||begin>=next_hud {
                        hud_model=campaign_hud(&c,&mut ui,config.width,config.height,ui_started.elapsed().as_millis() as u64);
                        bytes+=gpu.set_hud(&hud_model,config.width,config.height)?;
                        next_hud=begin+Duration::from_millis(200);hud_dirty=false;
                    }
                    Ok::<u64,String>(bytes)
                })();
                let uploaded=match upload_result{Ok(bytes)=>bytes,Err(error)=>{stop(&outcome,elwt,error);return;}};
                work.upload_ms=milliseconds(upload.elapsed());
                let encode=Instant::now();
                gpu.render(&frame.texture.create_view(&Default::default()),0);
                work.encode_submit_ms=milliseconds(encode.elapsed());
                let present=Instant::now();frame.present();work.present_ms=milliseconds(present.elapsed());
                accepted_frames+=1;measured_frames+=1;
                if automated {
                    let capture_begin=Instant::now();
                    for (i,(tick,label)) in [(1,"start"),(1200,"built"),(CHECK_FINISH_TICK,"ecology")].iter().enumerate(){
                        if !captured[i]&&c.clock.tick()>=*tick {
                            hud_model=campaign_hud(&c,&mut ui,config.width,config.height,ui_started.elapsed().as_millis() as u64);
                            if let Err(error)=gpu.set_hud(&hud_model,config.width,config.height){stop(&outcome,elwt,error);return;}
                            let image=gpu.target(config.width,config.height);
                            gpu.render(&image.create_view(&Default::default()),0);
                            let path=format!("{prefix}-{label}.png");
                            if let Err(error)=gpu.capture(&image,Path::new(&path)){stop(&outcome,elwt,format!("Campaign capture: {error}"));return;}
                            logging::log("CAPTURE",&path);captured[i]=true;
                        }
                    }
                    if c.clock.tick()>=CHECK_FINISH_TICK&&ui_captures.is_empty() {
                        let variants=ui_evidence_frames(&c,&ui,ui_started.elapsed().as_millis() as u64);
                        match gpu.capture_ui_evidence(&variants,&prefix) {
                            Ok(files)=>ui_captures=files,
                            Err(error)=>{stop(&outcome,elwt,format!("Native UI evidence capture: {error}"));return;}
                        }
                    }
                    work.capture_ms=milliseconds(capture_begin.elapsed());
                }
                let reporting=Instant::now();
                let report_elapsed=measured.elapsed().as_secs_f64();
                let finishing=automated&&c.clock.tick()>=CHECK_FINISH_TICK;
                if report_elapsed>=1.||finishing {
                    let committed_speed=(c.clock.tick().saturating_sub(previous_tick)) as f64/TACTICAL_HZ as f64/report_elapsed.max(f64::EPSILON);
                    let report=serde_json::json!({"workload_kind":"small_native_campaign_CPU_authoritative","run_mode":"windowed_vsync","backend":format!("{:?}",gpu.info.backend),"device":gpu.info.name,"authoritative_tick":c.clock.tick(),"pending_ticks":c.clock.pending_ticks(),"agents_rendered":render_count,"committed_ticks_this_frame":steps,"accepted_frames_in_window":measured_frames,"accepted_frames_per_wall_second":measured_frames as f64/report_elapsed.max(f64::EPSILON),"initial_military":MILITARY,"initial_workers":WORKERS,"requested_speed":c.clock.speed().multiplier(),"cpu_committed_simulation_seconds_per_wall_second":committed_speed,"gpu_completed_simulation_speed":null,"gpu_execution_ms":null,"frame_intervals":interval_samples.summary(),"stage_cpu_wall":stage_samples.summary(),"last_frame_upload_payload_bytes":uploaded,"quantiles":"nearest_rank_recent_retained_samples","max_and_mean":"all_intervals_in_reporting_window","no_hidden_population_reduction":true});
                    logging::log("PERF_DETAIL",&report.to_string());
                    logging::log("CAMPAIGN_STATE",&c.report().to_string());
                    window.set_title(&format!("Last World · tick {} · {:0.1} committed/requested {}× · {} pending ticks",c.clock.tick(),committed_speed,c.clock.speed().multiplier(),c.clock.pending_ticks()));
                    measured=Instant::now();measured_frames=0;previous_tick=c.clock.tick();interval_samples.clear();stage_samples.clear();
                }
                work.reporting_ms=milliseconds(reporting.elapsed());
                work.total_work_ms=milliseconds(begin.elapsed());
                stage_samples.record(&work);all_stages.record(&work);
                if work.slow(){logging::log("SLOW_FRAME",&serde_json::to_string(&work).unwrap());}
                if finishing {
                    let final_begin=Instant::now();
                    let checked=c.validate();
                    let passed=checked.is_ok()&&captured.iter().all(|v|*v)&&ui_captures.len()==15;
                    let report=serde_json::json!({"status":if passed{"passed"}else{"failed"},"conserved":passed,"kind":"automated_small_native_campaign_check_not20km_full160_release","tick":c.clock.tick(),"requested_speed":c.clock.speed().multiplier(),"captures":captured,"native_ui_captures":ui_captures,"native_ui":gpu.hud_diagnostics(),"accepted_frames":accepted_frames,"frame_intervals":all_intervals.summary(),"stage_cpu_wall":all_stages.summary(),"quantiles":"nearest_rank_latest8192intervals","max_and_mean":"all_accepted_intervals","gpu_execution_ms":null,"gpu_completed_simulation_speed":null,"validation_error":checked.err(),"state":c.report()});
                    if let Err(error)=save_check_report(&report_path,&report){stop(&outcome,elwt,format!("Writing campaign report: {error}"));return;}
                    logging::log("CAMPAIGN_CHECK_RESULT",&report.to_string());
                    logging::log("CAMPAIGN_FINAL_TIMING",&serde_json::json!({"frame":frame_id,"tick":c.clock.tick(),"final_validation_report_write_log_ms":milliseconds(final_begin.elapsed()),"final_frame_including_capture_ms":milliseconds(begin.elapsed()),"capture_ms":work.capture_ms,"all_capture_cost_included_in_stage_profile":true,"GPU_timestamps":false}).to_string());
                    *outcome.lock().expect("campaign outcome lock")=Some(if passed{Ok(())}else{Err("Campaign final validation/capture check failed".into())});
                    elwt.exit();
                }
                last_work_end=Instant::now();
            }
            _=>{}
        },
        Event::NewEvents(winit::event::StartCause::ResumeTimeReached { .. }) if minimized => window.request_redraw(),
        Event::AboutToWait=>{
            if minimized {
                elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(std::time::Instant::now()+Duration::from_millis(20)));
            }else{
                elwt.set_control_flow(winit::event_loop::ControlFlow::Poll);
                window.request_redraw();
            }
        },
        _=>{}
    }).map_err(|e|e.to_string())?;
    let result = outcome_after_loop
        .lock()
        .expect("campaign outcome lock")
        .take()
        .unwrap_or_else(|| Err("Campaign event loop ended without a completion result".into()));
    if automated && result.is_err() && !Path::new(&report_after_loop).exists() {
        save_check_report(
            &report_after_loop,
            &serde_json::json!({"status":"failed","conserved":false,"reason":result.as_ref().err(),"kind":"automated_campaign_incomplete"}),
        )?;
    }
    result
}

/// Native presentation of one shared campaign benchmark case. The session owns
/// the controller/checkpoint contract; this loop adds the real surface, renderer
/// and read-only commander UI without replacing any simulation system.
#[allow(deprecated)]
pub async fn benchmark_windowed(
    case_id: &str,
    output: &Path,
    backends: Option<wgpu::Backends>,
) -> Result<(), String> {
    use crate::campaign_benchmark::{self, BenchmarkMode};
    let requested = campaign_benchmark::case(case_id)
        .ok_or_else(|| format!("Unknown campaign benchmark case: {case_id}"))?;
    if requested.mode != BenchmarkMode::Window {
        return Err(format!(
            "Benchmark case {case_id} is not a native window case"
        ));
    }
    let setup_started = Instant::now();
    let requested_tactical = requested.military as u64 + requested.workers as u64;
    if requested_tactical > crate::gpu::CAPACITY as u64 {
        let error = format!(
            "Native renderer supports at most {} tactical display agents; this case requests {} military + workers. The declared capacity is unsupported; no population is constructed or reduced.",
            crate::gpu::CAPACITY,
            requested_tactical
        );
        let mut report = campaign_benchmark::failed_prepare_report(case_id, output, &error)?;
        report["native_window"] = serde_json::json!({"render_started":false,"capacity_rejection":true,
            "requested_tactical_agents":requested_tactical,"gpu_agent_capacity":crate::gpu::CAPACITY,
            "actual_simulated_agents":null,"no_count_clamp":true,"world_construction_skipped":true,
            "setup_ms":milliseconds(setup_started.elapsed()),"gpu_execution_ms":null,
            "scope":"declared unsupported native renderer population; no hardware saturation or FPS measurement"});
        let session_id = report["session_id"]
            .as_str()
            .ok_or("Missing unsupported native report session identity")?;
        save_native_benchmark_report(&output.join(format!("{session_id}-report.json")), &report)?;
        logging::log("NATIVE_BENCHMARK_CAPACITY_REJECTED", &report.to_string());
        return Err(error);
    }
    let (mut campaign, mut session) = match campaign_benchmark::prepare(case_id, output) {
        Ok(value) => value,
        Err(error) => {
            campaign_benchmark::failed_prepare_report(case_id, output, &error)?;
            return Err(error);
        }
    };
    let final_path = session.report_path.clone();
    let progress_path = session.progress_path.clone();
    let fallback_progress = progress_path.clone();
    let capture_path = session.capture_path.clone();
    // Separate read-only diagnostic role; never committed to Campaign.recon.
    let observer_masks = requested.battle_scenario.then(|| {
        vec![
            crate::campaign_support::VISIBLE | crate::campaign_support::EXPLORED;
            crate::gpu::CAMPAIGN_MASK_CELLS
        ]
    });
    if session.recovered_completed_case() {
        let report = session.finish(&mut campaign)?;
        logging::log("NATIVE_BENCHMARK_RECOVERED_VALID_RECEIPT",&serde_json::json!({
            "report_path":session.report_path,"replayed_simulation":false,
            "fresh_native_gpu_measurement":false,"receipt":report.get("recovered_previous_success"),
        }).to_string());
        return if report["passed"].as_bool() == Some(true) {
            Ok(())
        } else {
            Err("Recovered benchmark receipt is not a validated pass".into())
        };
    }
    let backend = match requested
        .backend
        .as_deref()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("vulkan") => Some(wgpu::Backends::VULKAN),
        Some("dx12" | "d3d12") => Some(wgpu::Backends::DX12),
        Some(other) => {
            let error = format!("Unsupported native benchmark backend {other}");
            session.fail(error.clone());
            let _ = session.finish(&mut campaign);
            return Err(error);
        }
        None => backends,
    };
    if let (Some(case_backend), Some(cli_backend)) = (backend, backends) {
        if !case_backend.intersects(cli_backend) {
            let error =
                "Benchmark case backend conflicts with the explicitly requested backend".to_owned();
            session.fail(error.clone());
            let _ = session.finish(&mut campaign);
            return Err(error);
        }
    }
    let setup = async {
        let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
        let size = requested.window_size;
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(format!("Last World · native benchmark {case_id}"))
                        .with_inner_size(winit::dpi::PhysicalSize::new(
                            size[0].max(1),
                            size[1].max(1),
                        )),
                )
                .map_err(|e| e.to_string())?,
        );
        let actual = window.inner_size();
        let (mut gpu, surface, initial_terrain) = Gpu::new_campaign(
            Some(window.clone()),
            requested.seed,
            actual.width.max(1),
            actual.height.max(1),
            backend,
        )
        .await?;
        // This is the renderer's procedural initial upload, not another running
        // campaign. Restored/modified authoritative terrain is uploaded below.
        drop(initial_terrain);
        let surface = surface.ok_or("Native benchmark surface missing")?;
        let mut config = surface
            .get_default_config(&gpu.adapter, actual.width.max(1), actual.height.max(1))
            .ok_or("No native benchmark surface configuration")?;
        config.format = gpu.format;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&gpu.device, &config);
        let mut ui = UiState::default();
        let (w, h) = hud::canvas_size(config.width, config.height);
        ui.resize_layout([w, h]);
        let mut model = campaign_hud(&campaign, &mut ui, config.width, config.height, 0);
        benchmark_hud_status(
            &mut model,
            session.current_requested_rate(),
            0.,
            campaign.clock.pending_ticks(),
            session.tick_budget_per_call(),
        );
        if requested.battle_scenario {
            battle_observer_hud(&mut model, campaign.clock.tick());
        }
        gpu.upload_terrain(&mut campaign.terrain);
        gpu.set_hud(&model, config.width, config.height)?;
        Ok::<_, String>((event_loop, window, gpu, surface, config, ui))
    }
    .await;
    let (event_loop, window, mut gpu, surface, mut config, mut ui) = match setup {
        Ok(value) => value,
        Err(error) => {
            session.fail(format!("Native window/GPU setup: {error}"));
            let mut report = session
                .finish(&mut campaign)
                .unwrap_or_else(|finish_error| {
                    serde_json::json!({
                        "status":"failed","reason":error,"report_error":finish_error,
                    })
                });
            report["native_window"] = serde_json::json!({
                "setup_ms":milliseconds(setup_started.elapsed()),"render_started":false,
                "gpu_execution_ms":null,"capture_paths":[],
            });
            save_check_report(
                final_path
                    .to_str()
                    .ok_or("Non-UTF8 benchmark report path")?,
                &report,
            )?;
            return Err(error);
        }
    };
    let setup_ms = milliseconds(setup_started.elapsed());
    let gpu_error = Arc::new(Mutex::new(None::<String>));
    let errors = gpu_error.clone();
    gpu.device.on_uncaptured_error(Arc::new(move |error| {
        let text = error.to_string();
        logging::log("GPU_ERROR", &text);
        *errors.lock().expect("native benchmark GPU error lock") = Some(text);
    }));
    let errors = gpu_error.clone();
    gpu.device.set_device_lost_callback(move |reason, message| {
        if reason == wgpu::DeviceLostReason::Destroyed {
            logging::log(
                "GPU_DEVICE_DESTROYED",
                "Native benchmark device destruction callback",
            );
            return;
        }
        let text = format!("Device lost: {reason:?}; {message}");
        logging::log("GPU_ERROR", &text);
        *errors.lock().expect("native benchmark GPU error lock") = Some(text);
    });
    let backend_name = format!("{:?}", gpu.info.backend);
    let adapter_name = gpu.info.name.clone();
    let outcome: Outcome = Arc::new(Mutex::new(None));
    let after_loop = outcome.clone();
    let fallback_report = final_path.clone();
    let ready = Instant::now();
    let mut last_step = ready;
    let mut last_end = ready;
    let mut first_step = true;
    let mut last_flash = ready;
    let mut next_hud = ready;
    let mut next_progress = ready;
    let mut minimized = false;
    let mut stats = NativeBenchmarkStats::default();
    stats.tick_budget_per_call = session.tick_budget_per_call();
    let mut input_ms = 0.;
    let mut finalizing = false;
    logging::log("NATIVE_BENCHMARK_READY",&serde_json::json!({
        "case":case_id,"requested_rate":session.current_requested_rate(),"military":requested.military,
        "workers":requested.workers,"backend":backend_name,"adapter":adapter_name,
        "physical_window":[config.width,config.height],"setup_ms":setup_ms,
        "input_policy":"scenario input locked; real OS event handling measured; no input throughput qualification",
        "readback_policy":"no per-frame readbacks; terminal evidence only",
    }).to_string());
    let run_error = event_loop.run(move |event,elwt| {
        match event {
            Event::WindowEvent { event,.. } => match event {
                WindowEvent::CloseRequested => {
                    if finalizing { return; }
                    finalizing = true;
                    let reason = "Native benchmark window closed before automatic completion".to_owned();
                    session.fail(reason.clone());
                    let report = finish_native_benchmark(&mut session,&mut campaign,&gpu,&mut stats,
                        setup_ms,&backend_name,&adapter_name,&[],0.,Some(&reason),Some(last_end));
                    *outcome.lock().expect("native benchmark outcome lock") = Some(report.and(Err(reason)));
                    elwt.exit();
                }
                WindowEvent::Resized(size) => {
                    let started=Instant::now(); minimized=size.width==0||size.height==0;
                    stats.resize_events+=1;
                    if !minimized {
                        config.width=size.width;config.height=size.height;
                        surface.configure(&gpu.device,&config);
                        let(w,h)=hud::canvas_size(config.width,config.height);ui.resize_layout([w,h]);next_hud=Instant::now();
                    }
                    input_ms+=milliseconds(started.elapsed());
                }
                WindowEvent::Focused(_) | WindowEvent::ModifiersChanged(_) |
                WindowEvent::CursorMoved {..} | WindowEvent::MouseInput {..} |
                WindowEvent::MouseWheel {..} | WindowEvent::KeyboardInput {..} => {
                    let started=Instant::now();stats.input_events+=1;
                    // Benchmark scenarios keep authoritative orders identical
                    // across CPU/window/backend cases. Ordinary play retains its
                    // full input path in windowed(), unchanged above.
                    input_ms+=milliseconds(started.elapsed());
                }
                WindowEvent::RedrawRequested => {
                    if finalizing { return; }
                    let begin=Instant::now();
                    stats.attempted_frames+=1;
                    let mut work=FrameWork {frame:stats.attempted_frames,
                        event_gap_ms:milliseconds(begin-last_end),input_ms:std::mem::take(&mut input_ms),..Default::default()};
                    let mut frame_recorded=false;
                    let mut finalization_started=false;
                    let result=(|| -> Result<bool,String> {
                        if let Some(error)=gpu_error.lock().expect("native benchmark GPU error lock").take() {
                            return Err(format!("Native benchmark GPU failure: {error}"));
                        }
                        let elapsed=if first_step {first_step=false;Duration::ZERO} else {begin-last_step};
                        last_step=begin;
                        let flash_elapsed=(begin-last_flash).as_secs_f32();last_flash=begin;
                        if campaign.flash>=0. {campaign.flash+=flash_elapsed;if campaign.flash>=0.44 {campaign.flash=-1.;}}
                        let sim=Instant::now();
                        let before_tick=campaign.clock.tick();
                        let progress=session.step(&mut campaign,elapsed)?;
                        let committed=campaign.clock.tick().saturating_sub(before_tick);
                        stats.simulation_calls+=1;stats.committed_ticks+=committed;
                        stats.max_committed_ticks_per_call=stats.max_committed_ticks_per_call.max(committed);
                        work.simulation_ms=milliseconds(sim.elapsed());work.tick=campaign.clock.tick();
                        if requested.battle_scenario {
                            campaign.data.camera=battle_observer_camera(&campaign);
                        }
                        let mut presented=false;
                        if minimized {
                            stats.minimized_frames+=1;
                        } else {
                            let acquire=Instant::now();
                            let frame=match surface.get_current_texture() {
                                Ok(frame)=>Some(frame),
                                Err(wgpu::SurfaceError::Lost|wgpu::SurfaceError::Outdated)=>{
                                    surface.configure(&gpu.device,&config);stats.surface_recoveries+=1;
                                    logging::log("SURFACE_RECOVERY","Native benchmark surface reconfigured; authoritative tick debt retained");None
                                }
                                Err(wgpu::SurfaceError::Timeout)=>{stats.surface_timeouts+=1;None}
                                Err(error)=>return Err(format!("Native benchmark surface: {error}")),
                            };
                            work.surface_acquire_ms=milliseconds(acquire.elapsed());
                            if let Some(frame)=frame {
                                let upload=Instant::now();
                                let flash_origin=campaign.flash_position.unwrap_or([campaign.camera[0],campaign.camera[1]]);
                                gpu.params.view=[campaign.camera[0],campaign.camera[1],flash_origin[0],flash_origin[1]];
                                gpu.params.screen=[config.width as f32,config.height as f32,campaign.camera[2],campaign.clock.seconds() as f32];
                                gpu.params.misc[1]=1.;gpu.params.misc[2]=if ui.prefs.layers.contains(&Layer::Radar){1.}else{0.};gpu.params.misc[3]=campaign.flash;
                                let alpha=campaign.clock.remainder_nanos() as f32/TICK_NANOS as f32;
                                let agents=if requested.battle_scenario {
                                    crate::battle_ai::observer_agents(&campaign,alpha)
                                } else {campaign.draw_agents(alpha)};
                                let objects=campaign.draw_objects()?;
                                stats.max_display_agents=stats.max_display_agents.max(agents.len());
                                stats.max_display_objects=stats.max_display_objects.max(objects.len());
                                let mut bytes=gpu.upload_terrain(&mut campaign.terrain);
                                bytes+=gpu.upload_campaign_agents(&agents)?;
                                bytes+=gpu.upload_campaign_objects(&objects,objects.len())?;
                                bytes+=gpu.upload_campaign_intel(observer_masks.as_deref().unwrap_or_else(|| campaign.intel()))?;
                                if begin>=next_hud {
                                    let hud_begin=Instant::now();
                                    let mut model=campaign_hud(&campaign,&mut ui,config.width,config.height,ready.elapsed().as_millis() as u64);
                                    benchmark_hud_status(&mut model,session.current_requested_rate(),progress.achieved_simulation_rate,progress.pending_ticks,session.tick_budget_per_call());
                                    if requested.battle_scenario {battle_observer_hud(&mut model,campaign.clock.tick());}
                                    bytes+=gpu.set_hud(&model,config.width,config.height)?;
                                    stats.hud_samples.record_seconds(hud_begin.elapsed().as_secs_f64());
                                    stats.hud_updates+=1;next_hud=begin+Duration::from_millis(200);
                                }
                                stats.uploaded_bytes=stats.uploaded_bytes.saturating_add(bytes);
                                stats.max_upload_bytes=stats.max_upload_bytes.max(bytes);
                                work.upload_ms=milliseconds(upload.elapsed());
                                let encode=Instant::now();gpu.render(&frame.texture.create_view(&Default::default()),0);
                                work.encode_submit_ms=milliseconds(encode.elapsed());
                                let present=Instant::now();frame.present();work.present_ms=milliseconds(present.elapsed());
                                stats.presented_frames+=1;
                                if campaign.flash>=0. && campaign.flash<0.44 {stats.light_uniform_active_frames+=1;}
                                presented=true;
                            }
                        }
                        session.note_native_presentation(&mut campaign,presented)?;
                        let progress=session.progress(&campaign,progress.last_advanced_ticks);
                        stats.max_pending_ticks=stats.max_pending_ticks.max(progress.pending_ticks);
                        stats.last_pending_ticks=progress.pending_ticks;
                        stats.last_achieved_rate=progress.achieved_simulation_rate;
                        if !progress.done {
                            let reporting=Instant::now();
                            if begin>=next_progress {
                                let snapshot=serde_json::json!({"kind":"native_window_progress","status":"running",
                                    "case":requested.id,"tick":campaign.clock.tick(),"requested_rate":session.current_requested_rate(),
                                    "live_loop_started_utc_ms":session.live_loop_started_utc_ms(),
                                    "simulated_ticks":progress.simulated_ticks,"achieved_rate":progress.achieved_simulation_rate,
                                    "pending_ticks":progress.pending_ticks,"elapsed_wall_seconds":progress.elapsed_wall_seconds,
                                    "speed_ramp":session.speed_ramp_progress(),
                                    "presented_frames":stats.presented_frames,"attempted_frames":stats.attempted_frames,
                                    "frame_intervals":stats.normal_intervals.summary(),"gpu_execution_ms":null});
                                save_check_report(progress_path.to_str().ok_or("Non-UTF8 benchmark progress path")?,&snapshot)?;
                                logging::log("NATIVE_BENCHMARK_PROGRESS",&snapshot.to_string());
                                let ramp_progress=session.speed_ramp_progress();
                                let phase=if requested.battle_scenario {"DIAGNOSTIC OBSERVER /20minute AI battle"}else{ramp_progress["phase"].as_str().unwrap_or("measurement")};
                                window.set_title(&format!("Last World · {} · {} · requested {}× / achieved {:.2}× · {} pending · {} presented",
                                    requested.id,phase,session.current_requested_rate(),progress.achieved_simulation_rate,progress.pending_ticks,stats.presented_frames));
                                next_progress=begin+Duration::from_secs(1);
                            }
                            work.reporting_ms=milliseconds(reporting.elapsed());
                            work.total_work_ms=milliseconds(begin.elapsed());
                            if work.slow(){logging::log("SLOW_FRAME",&serde_json::to_string(&work).unwrap());}
                            work.total_work_ms=milliseconds(begin.elapsed());
                            stats.record_stages(&work);frame_recorded=true;
                            let complete_end=Instant::now();
                            let interval=complete_end-last_end;
                            stats.normal_intervals.record_seconds(interval.as_secs_f64());
                            stats.all_intervals.record_seconds(interval.as_secs_f64());
                            session.record_frame(interval);
                            last_end=complete_end;
                            return Ok(false);
                        }
                        let normal_interval=Instant::now()-last_end;
                        stats.normal_intervals.record_seconds(normal_interval.as_secs_f64());
                        session.record_frame(normal_interval);
                        // Intentional terminal readback. It is excluded from
                        // normal FPS and reported as a separate evidence burst.
                        let capture_begin=Instant::now();
                        let mut captures=Vec::new();
                        if requested.capture_last_frame && !presented {
                            return Err("Native benchmark completed without an acquired/presented terminal frame; evidence not qualified".into());
                        }
                        if requested.capture_last_frame {
                            let mut model=campaign_hud(&campaign,&mut ui,config.width,config.height,ready.elapsed().as_millis() as u64);
                            benchmark_hud_status(&mut model,session.current_requested_rate(),progress.achieved_simulation_rate,progress.pending_ticks,session.tick_budget_per_call());
                            if requested.battle_scenario {battle_observer_hud(&mut model,campaign.clock.tick());}
                            gpu.set_hud(&model,config.width,config.height)?;
                            let image=gpu.target(config.width,config.height);
                            gpu.render(&image.create_view(&Default::default()),0);
                            gpu.capture(&image,&capture_path)?;
                            captures.push(capture_path.to_string_lossy().into_owned());
                            let stem=capture_path.with_extension("");
                            let prefix=stem.file_name().and_then(|name|name.to_str()).ok_or("Non-UTF8 benchmark capture prefix")?;
                            let directory=stem.parent().unwrap_or_else(||std::path::Path::new(""));
                            let mut variants:Vec<_>=ui_evidence_frames(&campaign,&ui,ready.elapsed().as_millis() as u64)
                                .into_iter().filter(|frame|frame.label=="calm").collect();
                            for frame in &mut variants {
                                benchmark_hud_status(&mut frame.model,session.current_requested_rate(),progress.achieved_simulation_rate,progress.pending_ticks,session.tick_budget_per_call());
                                if requested.battle_scenario {battle_observer_hud(&mut frame.model,campaign.clock.tick());}
                            }
                            captures.extend(gpu.capture_ui_evidence_in(&variants,directory,prefix)?);
                        }
                        work.capture_ms=milliseconds(capture_begin.elapsed());
                        if let Some(error)=gpu_error.lock().expect("native benchmark GPU error lock").take() {
                            return Err(format!("Native benchmark GPU failure during terminal evidence: {error}"));
                        }
                        work.total_work_ms=milliseconds(begin.elapsed());stats.record_stages(&work);frame_recorded=true;
                        finalization_started=true;
                        let passed=finish_native_benchmark(&mut session,&mut campaign,&gpu,&mut stats,setup_ms,
                            &backend_name,&adapter_name,&captures,work.capture_ms,None,Some(last_end))?;
                        if !passed {return Err("Shared benchmark scenario validation failed; native report retained".into());}
                        logging::log("NATIVE_BENCHMARK_FINAL_FRAME",&serde_json::json!({
                            "normal_complete_frame_ms":milliseconds(normal_interval),
                            "terminal_frame_including_evidence_and_report_ms":milliseconds(begin.elapsed()),
                            "capture_burst_ms":work.capture_ms,"gpu_execution_ms":null,
                        }).to_string());
                        Ok(true)
                    })();
                    match result {
                        Ok(false)=>{},
                        Ok(true)=>{finalizing=true;*outcome.lock().expect("native benchmark outcome lock")=Some(Ok(()));elwt.exit();},
                        Err(error)=>{
                            finalizing=true;
                            work.total_work_ms=milliseconds(begin.elapsed());if !frame_recorded {stats.record_stages(&work);}
                            if !finalization_started {
                                session.fail(error.clone());
                                if let Err(report_error)=finish_native_benchmark(&mut session,&mut campaign,&gpu,&mut stats,setup_ms,
                                    &backend_name,&adapter_name,&[],work.capture_ms,Some(&error),Some(last_end)) {
                                    logging::log("BENCHMARK_REPORT_ERROR",&report_error);
                                }
                            } else if !session.report_path.exists() {
                                let fallback=serde_json::json!({"status":"failed","reason":error,
                                    "native_finalization_incomplete":true,"frames":stats.report(),"state":campaign.report()});
                                if let Err(report_error)=save_check_report(session.report_path.to_str().unwrap_or("native-benchmark-report-failed.json"),&fallback) {
                                    logging::log("BENCHMARK_REPORT_ERROR",&report_error);
                                }
                            }
                            logging::log("NATIVE_BENCHMARK_FAILED",&error);
                            *outcome.lock().expect("native benchmark outcome lock")=Some(Err(error));elwt.exit();
                        }
                    }
                }
                _=>{},
            },
            Event::AboutToWait=>{
                if minimized {elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(std::time::Instant::now()+Duration::from_millis(20)));}
                else {elwt.set_control_flow(winit::event_loop::ControlFlow::Poll);window.request_redraw();}
            }
            Event::NewEvents(winit::event::StartCause::ResumeTimeReached{..}) if minimized=>window.request_redraw(),
            _=>{},
        }
    }).map_err(|error| error.to_string());
    if let Err(error) = run_error {
        // Per-second progress remains if the platform aborts before the event
        // handler can flush the richer partial report.
        save_check_report(
            fallback_report
                .to_str()
                .ok_or("Non-UTF8 benchmark report path")?,
            &serde_json::json!({"status":"failed","reason":error,"kind":"native_event_loop_failure","partial_progress_file":fallback_progress}),
        )?;
        return Err(error);
    }
    let result = after_loop
        .lock()
        .expect("native benchmark outcome lock")
        .take()
        .unwrap_or_else(|| Err("Native benchmark event loop ended without a report".into()));
    result
}

/// Only the automated diagnostic camera uses unfiltered physical locations.
/// Commander decisions never read this result and no recon masks are changed.
fn battle_observer_camera(game: &Campaign) -> [f32; 3] {
    let shot = game.clock.tick() / 1200 % 4;
    match shot {
        1 => [
            crate::construction::TEAM_BASES[0][0] + 100.,
            crate::construction::TEAM_BASES[0][1],
            0.75,
        ],
        2 => [2400., 1600., 0.30],
        3 => [
            crate::construction::TEAM_BASES[1][0] - 100.,
            crate::construction::TEAM_BASES[1][1],
            0.75,
        ],
        _ => {
            let mut center = [0., 0.];
            let mut count = 0u32;
            for a in game
                .tactics
                .agents
                .iter()
                .filter(|a| a.alive() && a.role == Role::Soldier && a.target.is_some())
                .take(128)
            {
                let p = a.position();
                center[0] += p[0];
                center[1] += p[1];
                count += 1;
            }
            if count > 0 {
                [center[0] / count as f32, center[1] / count as f32, 0.85]
            } else {
                [2208., 1580., 0.65]
            }
        }
    }
}
fn battle_observer_hud(model: &mut hud::HudModel, tick: u64) {
    let seconds = tick / 20;
    let label = format!(
        "DIAGNOSTIC OBSERVER | TWO ACTIVE AIs | {:02}:{:02} / 20:00 game time | requested4x",
        seconds / 60,
        seconds % 60
    );
    if let Some(view) = &mut model.ui {
        view.prefs.hints = true;
        view.hints.push(label.clone());
        view.selection.title = "BATTLE OBSERVER".into();
    }
    model.footer.push(label);
}

fn benchmark_hud_status(
    model: &mut hud::HudModel,
    requested: u32,
    achieved: f64,
    pending: u64,
    budget: u32,
) {
    let status = format!(
        "BENCH {requested}x requested / {achieved:.2}x achieved / {pending} debt / {budget} ticks per call"
    );
    if model.lines.len() < 3 {
        model.lines.resize(3, String::new());
    }
    // The ordinary clock retains its save-compatible 1/2/4 setting. This
    // benchmark override is explicit and never relabels it as 100x gameplay.
    model.lines[2] = status.clone();
    if let Some(view) = &mut model.ui {
        view.prefs.hints = true;
        view.hints.push(status);
    }
}

struct NativeBenchmarkStats {
    tick_budget_per_call: u32,
    attempted_frames: u64,
    presented_frames: u64,
    input_events: u64,
    resize_events: u64,
    minimized_frames: u64,
    surface_recoveries: u64,
    surface_timeouts: u64,
    max_pending_ticks: u64,
    last_pending_ticks: u64,
    last_achieved_rate: f64,
    max_display_agents: usize,
    max_display_objects: usize,
    uploaded_bytes: u64,
    max_upload_bytes: u64,
    hud_updates: u64,
    light_uniform_active_frames: u64,
    simulation_calls: u64,
    committed_ticks: u64,
    max_committed_ticks_per_call: u64,
    normal_intervals: FrameTimings,
    all_intervals: FrameTimings,
    hud_samples: FrameTimings,
    stage_samples: Vec<(&'static str, FrameTimings)>,
    stage_profile: FrameProfile,
}
impl Default for NativeBenchmarkStats {
    fn default() -> Self {
        Self {
            tick_budget_per_call: crate::campaign_benchmark::BENCHMARK_STEPS_PER_CALL,
            attempted_frames: 0,
            presented_frames: 0,
            input_events: 0,
            resize_events: 0,
            minimized_frames: 0,
            surface_recoveries: 0,
            surface_timeouts: 0,
            max_pending_ticks: 0,
            last_pending_ticks: 0,
            last_achieved_rate: 0.,
            max_display_agents: 0,
            max_display_objects: 0,
            uploaded_bytes: 0,
            max_upload_bytes: 0,
            hud_updates: 0,
            light_uniform_active_frames: 0,
            simulation_calls: 0,
            committed_ticks: 0,
            max_committed_ticks_per_call: 0,
            normal_intervals: FrameTimings::new(8192),
            all_intervals: FrameTimings::new(8192),
            hud_samples: FrameTimings::new(8192),
            stage_samples: [
                "input",
                "surface_acquire",
                "simulation",
                "upload",
                "encode_submit",
                "present",
                "capture",
                "reporting",
                "total_work",
            ]
            .into_iter()
            .map(|name| (name, FrameTimings::new(8192)))
            .collect(),
            stage_profile: FrameProfile::default(),
        }
    }
}
impl NativeBenchmarkStats {
    fn record_stages(&mut self, work: &FrameWork) {
        self.stage_profile.record(work);
        let stages = [
            work.input_ms,
            work.surface_acquire_ms,
            work.simulation_ms,
            work.upload_ms,
            work.encode_submit_ms,
            work.present_ms,
            work.capture_ms,
            work.reporting_ms,
            work.total_work_ms,
        ];
        for ((_, samples), milliseconds) in self.stage_samples.iter_mut().zip(stages) {
            samples.record_seconds(milliseconds / 1000.);
        }
    }
    fn report(&self) -> serde_json::Value {
        let stages: serde_json::Map<String, serde_json::Value> = self
            .stage_samples
            .iter()
            .map(|(name, samples)| ((*name).to_owned(), serde_json::json!(samples.summary())))
            .collect();
        let normal_wall = self
            .normal_intervals
            .summary()
            .map(|s| s.mean_ms * s.total_samples as f64 / 1000.)
            .unwrap_or(0.);
        serde_json::json!({"attempted_frames":self.attempted_frames,"presented_frames":self.presented_frames,
            "input_events":self.input_events,"resize_events":self.resize_events,"minimized_frames":self.minimized_frames,
            "surface_recoveries":self.surface_recoveries,"surface_timeouts":self.surface_timeouts,
            "normal_complete_frame_intervals":self.normal_intervals.summary(),
            "all_frame_intervals_including_terminal_evidence":self.all_intervals.summary(),
            "stage_cpu_wall":self.stage_profile.summary(),"stage_cpu_wall_quantiles":stages,
            "ui_projection_composition_upload_cpu_wall":self.hud_samples.summary(),"ui_updates":self.hud_updates,
            "last_pending_ticks":self.last_pending_ticks,"max_pending_ticks":self.max_pending_ticks,
            "last_achieved_simulation_rate":self.last_achieved_rate,
            "explosion_light_uniform_active_presented_frames":self.light_uniform_active_frames,
            "explosion_light_evidence_scope":"actual flash uniform enabled for native rendered frames; not a proof every shadow pixel was visible",
            "simulation_calls":self.simulation_calls,"committed_ticks":self.committed_ticks,
            "max_committed_ticks_per_call":self.max_committed_ticks_per_call,
            "benchmark_step_budget_per_call":self.tick_budget_per_call,"normal_game_step_budget_per_call":16,
            "measured_call_budget_speed_ceiling":if normal_wall>0. {Some(self.simulation_calls as f64*self.tick_budget_per_call as f64/TACTICAL_HZ as f64/normal_wall)}else{None},
            "budget_ceiling_scope":"hypothetical actual case tick budget at observed call rate; not claimed achieved hardware throughput",
            "max_current_intel_display_agents":self.max_display_agents,"max_current_intel_display_objects":self.max_display_objects,
            "total_upload_payload_bytes":self.uploaded_bytes,"max_frame_upload_payload_bytes":self.max_upload_bytes,
            "quantiles":"nearest rank latest 8192; maximum and mean cover every recorded frame",
            "render_filter":"case-specific diagnostic observer, or normal commander recon; see native_window.observer_role; population never truncated",
            "gpu_execution_ms":null,"gpu_ui_execution_ms":null,"gpu_timestamps_enabled":false,
            "timing_scope":"CPU wall + surface/present waits; GPU work is submitted, not GPU timestamp measured"})
    }
}
fn finish_native_benchmark(
    session: &mut crate::campaign_benchmark::BenchmarkSession,
    campaign: &mut Campaign,
    gpu: &Gpu,
    stats: &mut NativeBenchmarkStats,
    setup_ms: f64,
    backend: &str,
    adapter: &str,
    captures: &[String],
    capture_ms: f64,
    error: Option<&str>,
    terminal_start: Option<Instant>,
) -> Result<bool, String> {
    let finishing = Instant::now();
    let mut report = session.finish(campaign).unwrap_or_else(|finish_error| {
        serde_json::json!({
            "status":"failed","passed":false,"core_phase_passed":false,
            "reason":finish_error,"partial_campaign_state":campaign.report(),
            "checkpoint_commit":"not qualified by this native error report",
        })
    });
    let captures_valid = !session.case.capture_last_frame
        || (captures.len() >= 4
            && captures.iter().all(|path| {
                std::fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() > 24)
            }));
    let native_passed = error.is_none() && stats.presented_frames > 0 && captures_valid;
    let passed = report["core_phase_passed"].as_bool() == Some(true) && native_passed;
    report["native_window"] = serde_json::json!({"backend":backend,"adapter":adapter,
        "setup_ms":setup_ms,"render_started":true,"captures":captures,"capture_burst_ms":capture_ms,
        "physical_window":[gpu.params.screen[0] as u32,gpu.params.screen[1] as u32],
        "present_mode":"AutoVsync","native_validation_passed":native_passed,"captures_valid":captures_valid,
        "frames":stats.report(),"native_ui":gpu.hud_diagnostics(),"error":error,
        "screenshot_scope":"actual existing GPU world renderer; ordinary UI knowledge remains filtered",
        "observer_role":if session.case.battle_scenario {"explicit diagnostic full-surface observer; only renderer masks/actor copies differ; AIs and campaign recon/research unchanged"}else{"normal commander video/radar knowledge"},
        "evidence_policy":"terminal readbacks; excluded from normal FPS, explicitly included in all-frame diagnostics",
        "all_implemented_systems_use_shared_session":true,
        "whole_window_diagnostics_include_first_presentation_loading_live_warmup_and_settling":session.case.speed_ramp.is_some(),
        "per_rate_timing_location":"speed_ramp.completed_stages when adaptive; whole-window frames are not per-rate measurements",
        "input_policy":"benchmark orders fixed by shared scenario; ordinary gameplay input remains in windowed"});
    if !passed {
        let failure =
            error.unwrap_or("Native/core benchmark validation or terminal capture failed");
        session.fail(failure.to_owned());
        report["status"] = serde_json::json!("failed");
        report["passed"] = serde_json::json!(false);
        report["native_failure"] = serde_json::json!(failure);
    }
    report["native_final_validation_report_prewrite_ms"] =
        serde_json::json!(milliseconds(finishing.elapsed()));
    // Persist every validation fact while the core report is still pending.
    // A process failure here must leave the group's previous success intact.
    save_native_benchmark_report(&session.report_path, &report)?;
    logging::log("NATIVE_BENCHMARK_VALIDATION", &report.to_string());
    if passed {
        report["passed"] = serde_json::json!(true);
        report["status"] = serde_json::json!("passed");
        session.commit_success(campaign, &mut report)?;
    }
    let before_diagnostic_append = Instant::now();
    if let Some(start) = terminal_start {
        stats
            .all_intervals
            .record_seconds((before_diagnostic_append - start).as_secs_f64());
    }
    report["native_window"]["frames"] = stats.report();
    report["native_finalization_through_validation_checkpoint_and_report_write_ms"] =
        serde_json::json!(milliseconds(before_diagnostic_append - finishing));
    report["native_final_diagnostic_append_scope"] = serde_json::json!(
        "All-frame timing includes evidence, validation, pending native report write/log and checkpoint/full report commit; this final timing append and result log are outside the measured interval. The committed receipt contains all validation facts and normal FPS; its own save duration is appended to this report."
    );
    save_native_benchmark_report(&session.report_path, &report)?;
    logging::log("NATIVE_BENCHMARK_RESULT", &report.to_string());
    Ok(passed)
}
fn save_native_benchmark_report(path: &Path, report: &serde_json::Value) -> Result<(), String> {
    let temp = path.with_extension(format!("native-{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?;
    std::fs::write(&temp, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, path).map_err(|error| error.to_string())
}
