use crate::{
    clock::{GameClock, SimulationSpeed, TACTICAL_HZ},
    gpu::Gpu,
    frame_profile::{FrameProfile, FrameWork},
    logging,
    metrics::FrameTimings,
    river::RiverFixture,
    terrain::Terrain,
};
use bevy::prelude::*;
use std::{sync::Arc, time::Duration};
use web_time::Instant;
use winit::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};
#[derive(Resource, Clone)]
pub struct Control {
    pub frame_elapsed: Duration,
    pub clock: GameClock,
    pub clock_error: Option<String>,
    pub steps: u32,
    pub paused: bool,
    pub count: u32,
    pub camera: [f32; 3],
    pub flash: f32,
    pub fog: bool,
    pub radar: bool,
    pub benchmark: bool,
    pub phase: usize,
    pub phase_time: f32,
    pub keys: [bool; 4],
    pub river_fixture: bool,
    pub river_actions: Vec<bool>,
}
impl Default for Control {
    fn default() -> Self {
        Self {
            frame_elapsed: Duration::ZERO,
            clock: GameClock::default(),
            clock_error: None,
            steps: 0,
            paused: false,
            count: 5000,
            camera: [2000., 1640., 0.9],
            flash: -1.,
            fog: false,
            radar: true,
            benchmark: false,
            phase: 0,
            phase_time: 0.,
            keys: [false; 4],
            river_fixture: false,
            river_actions: Vec::new(),
        }
    }
}
#[derive(Resource)]
pub struct WorldSurface {
    pub terrain: Terrain,
    pub river: Option<RiverFixture>,
}

fn evolve_water(mut c: ResMut<Control>, surface: Option<ResMut<WorldSurface>>) {
    let Some(mut surface) = surface else {
        return;
    };
    let WorldSurface { terrain, river } = &mut *surface;
    let Some(river) = river else {
        return;
    };
    let result = (|| {
        river.refresh_geometry(terrain)?;
        for closed in c.river_actions.drain(..) {
            let before = river.water.totals().stored_liters;
            river.set_dam(closed)?;
            logging::log(
                "WATER_ACTION",
                &format!(
                    "dam_closed={closed}; retained {before} liters; no reservoir volume created"
                ),
            );
        }
        if c.clock.tick() > river.last_processed_tick {
            river.advance_to(c.clock.tick())?;
            river.sync_terrain(terrain)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        c.clock_error = Some(error);
    }
}
const COUNTS: [u32; 6] = [1000, 5000, 10000, 25000, 50000, 100000];
// Bound one render frame's work without dropping owed gameplay ticks.
const MAX_STEPS_PER_FRAME: u32 = 16;
fn fixed_clock(mut s: ResMut<Control>) {
    let elapsed = std::mem::take(&mut s.frame_elapsed);
    // This clamp is only for cosmetic camera/flash/benchmark UI updates.
    let dt = elapsed.as_secs_f32().min(0.25);
    let pan = 600. * dt / s.camera[2];
    for (i, dir) in [[0., -1.], [0., 1.], [-1., 0.], [1., 0.]]
        .iter()
        .enumerate()
    {
        if s.keys[i] {
            s.camera[0] = (s.camera[0] + dir[0] * pan).clamp(0., 4800.);
            s.camera[1] = (s.camera[1] + dir[1] * pan).clamp(0., 3200.);
        }
    }
    s.steps = 0;
    if s.flash >= 0. {
        s.flash += dt;
        if s.flash > 0.44 {
            s.flash = -1.;
        }
    }
    let paused = s.paused;
    match s.clock.advance(elapsed, paused, MAX_STEPS_PER_FRAME) {
        Ok(steps) => s.steps = steps,
        Err(error) => s.clock_error = Some(error.to_string()),
    }
    if s.benchmark {
        s.phase_time += dt;
        if s.phase_time >= 15. {
            s.phase_time = 0.;
            s.phase += 1;
            if s.phase >= COUNTS.len() {
                s.benchmark = false;
                logging::log(
                    "BENCHMARK",
                    "Population sweep complete; console.txt retains the session.",
                );
            } else {
                s.count = COUNTS[s.phase];
                logging::log("BENCHMARK_PHASE", &s.count.to_string());
            }
        }
    }
}
// Charge elapsed time at the old pause/speed state before changing an input.
// It becomes debt only; no movement batch is committed before surface success.
fn settle_clock_input(c: &mut Control, last: &mut Instant) -> Result<(), String> {
    let now = Instant::now();
    c.clock
        .advance(now - *last, c.paused, 0)
        .map_err(|error| error.to_string())?;
    *last = now;
    Ok(())
}
pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(Control::default())
        .add_systems(Update, (fixed_clock, evolve_water).chain());
    app
}
fn apply(g: &mut Gpu, c: &Control) {
    g.params.view[0] = c.camera[0];
    g.params.view[1] = c.camera[1];
    g.params.screen[2] = c.camera[2];
    // Float time is a derived rendering uniform, never the authority for rules.
    g.params.screen[3] = c.clock.seconds() as f32;
    g.params.sim[0] = c.count;
    g.params.misc[1] = if c.fog { 1. } else { 0. };
    g.params.misc[2] = if c.radar { 1. } else { 0. };
    g.params.misc[3] = c.flash;
}
#[allow(deprecated)]
pub async fn windowed(backends: Option<wgpu::Backends>) -> Result<(), String> {
    windowed_with_speed(backends, SimulationSpeed::One).await
}
#[allow(deprecated)]
pub async fn windowed_with_speed(
    backends: Option<wgpu::Backends>,
    speed: SimulationSpeed,
) -> Result<(), String> {
    windowed_options(backends, speed, false).await
}
#[allow(deprecated)]
pub async fn windowed_options(
    backends: Option<wgpu::Backends>,
    speed: SimulationSpeed,
    river_fixture: bool,
) -> Result<(), String> {
    windowed_run(backends, speed, river_fixture, false).await
}
pub async fn river_check(
    backends: Option<wgpu::Backends>,
    speed: SimulationSpeed,
) -> Result<(), String> {
    windowed_run(backends, speed, true, true).await
}
#[allow(deprecated)]
async fn windowed_run(
    backends: Option<wgpu::Backends>,
    speed: SimulationSpeed,
    river_fixture: bool,
    automated: bool,
) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title("Last World · Bevy + GPU · generating world")
                    .with_inner_size(winit::dpi::LogicalSize::new(1280., 800.)),
            )
            .map_err(|e| e.to_string())?,
    );
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowExtWebSys;
        let browser = web_sys::window().ok_or("Missing browser")?;
        let document = browser.document().ok_or("Missing document")?;
        let canvas = window.canvas().ok_or("Missing canvas")?;
        document
            .get_element_by_id("native-canvas")
            .ok_or("Missing canvas host")?
            .append_child(&canvas)
            .map_err(|_| "Canvas attach failed")?;
        let w = browser.inner_width().unwrap().as_f64().unwrap_or(1280.);
        let h = browser.inner_height().unwrap().as_f64().unwrap_or(800.);
        let _ = window.request_inner_size(winit::dpi::LogicalSize::new(w, h));
    }
    let size = window.inner_size();
    let (mut gpu, surface, mut terrain) = Gpu::new(
        Some(window.clone()),
        78421,
        5000,
        size.width.max(1),
        size.height.max(1),
        backends,
    )
    .await?;
    let surface = surface.ok_or("Missing surface")?;
    let mut config = surface
        .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
        .ok_or("No surface configuration")?;
    config.format = gpu.format;
    config.present_mode = wgpu::PresentMode::AutoVsync;
    surface.configure(&gpu.device, &config);
    logging::log(
        "READY",
        &format!(
            "{}; startup {:0.0} ms; WASD pan, scroll zoom, Space pause, 1-6 agents, F1/F2/F3 select 1x/2x/4x, +/- speed, B benchmark, F feeds, R radar, E explosion",
            crate::engine_identity(),
            gpu.generated_ms
        ),
    );
    let mut app = app();
    let river = if river_fixture {
        let mut river = RiverFixture::new(&mut terrain)?;
        if automated {
            river.enable_automated_check();
        }
        Some(river)
    } else {
        None
    };
    if river_fixture {
        let mut c = app.world_mut().resource_mut::<Control>();
        // A separate water-only interaction fixture, not a reduced army or a
        // performance qualification of military/civilian/ecological gameplay.
        c.river_fixture = true;
        c.count = 0;
        c.camera = [2380., 950., 0.85];
        c.radar = false;
        logging::log(
            "MODE",
            "Isolated river fixture; no military/civilian/wildlife agents. G closes hydraulic barrier, J breaches/opens. F1/F2/F3=1x/2x/4x; no construction/ecology/naval gameplay yet.",
        );
    }
    app.insert_resource(WorldSurface { terrain, river });
    app.world_mut()
        .resource_mut::<Control>()
        .clock
        .set_speed(speed);
    let mut last = Instant::now();
    let mut last_frame = last;
    let mut measured = last;
    let mut frames = 0u32;
    let mut previous_ticks = 0u64;
    let mut frame_timings = FrameTimings::new(8192);
    let mut submit_ms = 0.;
    let mut frame_profile = FrameProfile::default();
    let mut frame_number = 0u64;
    let mut last_finished = last;
    let mut touch: Option<(f64, f64)> = None;
    #[cfg(not(target_arch = "wasm32"))]
    let check_prefix = format!(
        "river-check-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    #[cfg(not(target_arch = "wasm32"))]
    let report_path = format!("{check_prefix}-report.json");
    let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let completed_in_loop = completed.clone();
    let mut captured = [false; 3];
    #[cfg(not(target_arch = "wasm32"))]
    let report_in_loop = report_path.clone();
    let handler = move |event: Event<()>, elwt: &winit::event_loop::ActiveEventLoop| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => {
                logging::log("SESSION_END", "Window closed");
                elwt.exit();
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    config.width = size.width;
                    config.height = size.height;
                    surface.configure(&gpu.device, &config);
                    gpu.params.screen[0] = size.width as f32;
                    gpu.params.screen[1] = size.height as f32;
                    logging::log(
                        "RESIZE",
                        &format!("{} x {} physical pixels", size.width, size.height),
                    );
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    let down = event.state == ElementState::Pressed;
                    let mut c = app.world_mut().resource_mut::<Control>();
                    let k = match key {
                        KeyCode::KeyW | KeyCode::ArrowUp => Some(0),
                        KeyCode::KeyS | KeyCode::ArrowDown => Some(1),
                        KeyCode::KeyA | KeyCode::ArrowLeft => Some(2),
                        KeyCode::KeyD | KeyCode::ArrowRight => Some(3),
                        _ => None,
                    };
                    if let Some(k) = k {
                        c.keys[k] = down;
                    }
                    if down && !event.repeat {
                        if matches!(
                            key,
                            KeyCode::Space
                                | KeyCode::F1
                                | KeyCode::F2
                                | KeyCode::F3
                                | KeyCode::Equal
                                | KeyCode::NumpadAdd
                                | KeyCode::Minus
                                | KeyCode::NumpadSubtract
                        ) {
                            if let Err(error) = settle_clock_input(&mut c, &mut last) {
                                logging::log("FATAL", &error);
                                elwt.exit();
                                return;
                            }
                        }
                        match key {
                            KeyCode::Escape => elwt.exit(),
                            KeyCode::Space => c.paused = !c.paused,
                            KeyCode::KeyB if !c.river_fixture => {
                                c.benchmark = true;
                                c.phase = 0;
                                c.phase_time = 0.;
                                c.count = COUNTS[0];
                            }
                            KeyCode::KeyF => c.fog = !c.fog,
                            KeyCode::KeyR => c.radar = !c.radar,
                            KeyCode::KeyE => c.flash = 0.,
                            KeyCode::KeyG if c.river_fixture => c.river_actions.push(true),
                            KeyCode::KeyJ if c.river_fixture => c.river_actions.push(false),
                            KeyCode::F1 => c.clock.set_speed(SimulationSpeed::One),
                            KeyCode::F2 => c.clock.set_speed(SimulationSpeed::Two),
                            KeyCode::F3 => c.clock.set_speed(SimulationSpeed::Four),
                            KeyCode::Equal | KeyCode::NumpadAdd => {
                                let speed = if c.clock.speed() == SimulationSpeed::One {
                                    SimulationSpeed::Two
                                } else {
                                    SimulationSpeed::Four
                                };
                                c.clock.set_speed(speed);
                            }
                            KeyCode::Minus | KeyCode::NumpadSubtract => {
                                let speed = if c.clock.speed() == SimulationSpeed::Four {
                                    SimulationSpeed::Two
                                } else {
                                    SimulationSpeed::One
                                };
                                c.clock.set_speed(speed);
                            }
                            KeyCode::Digit1 if !c.river_fixture => c.count = COUNTS[0],
                            KeyCode::Digit2 if !c.river_fixture => c.count = COUNTS[1],
                            KeyCode::Digit3 if !c.river_fixture => c.count = COUNTS[2],
                            KeyCode::Digit4 if !c.river_fixture => c.count = COUNTS[3],
                            KeyCode::Digit5 if !c.river_fixture => c.count = COUNTS[4],
                            KeyCode::Digit6 if !c.river_fixture => c.count = COUNTS[5],
                            _ => {}
                        }
                        logging::log(
                            "ACTION",
                            &format!(
                                "{key:?}; {} agents, {}x, paused {}",
                                c.count,
                                c.clock.speed().multiplier(),
                                c.paused
                            ),
                        );
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 * 0.02,
                };
                let mut c = app.world_mut().resource_mut::<Control>();
                c.camera[2] = (c.camera[2] * (d * 0.1).exp()).clamp(0.12, 6.);
                logging::log("ACTION", &format!("Zoom {:.3}", c.camera[2]));
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                app.world_mut().resource_mut::<Control>().flash = 0.;
                logging::log("ACTION", "Explosion flash");
            }
            WindowEvent::Touch(event) => {
                use winit::event::TouchPhase;
                let now = (event.location.x, event.location.y);
                let mut c = app.world_mut().resource_mut::<Control>();
                match event.phase {
                    TouchPhase::Started => touch = Some(now),
                    TouchPhase::Moved => {
                        if let Some(old) = touch {
                            c.camera[0] -= (now.0 - old.0) as f32 / c.camera[2];
                            c.camera[1] -= (now.1 - old.1) as f32 / c.camera[2];
                        }
                        touch = Some(now);
                    }
                    _ => {
                        touch = None;
                        logging::log(
                            "ACTION",
                            &format!("Touch pan to {:.0}, {:.0}", c.camera[0], c.camera[1]),
                        );
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_started = Instant::now();
                frame_number += 1;
                let mut work = FrameWork { frame: frame_number, event_gap_ms: (frame_started-last_finished).as_secs_f64()*1000., ..Default::default() };
                #[cfg(target_arch = "wasm32")]
                WEB_COMMANDS.with(|q| {
                    let commands = std::mem::take(&mut *q.borrow_mut());
                    let mut c = app.world_mut().resource_mut::<Control>();
                    for command in commands {
                        if matches!(command.as_str(), "pause" | "speed+" | "speed-") {
                            if let Err(error) = settle_clock_input(&mut c, &mut last) {
                                c.clock_error = Some(error);
                                return;
                            }
                        }
                        match command.as_str() {
                            "pause" => c.paused = !c.paused,
                            "benchmark" => {
                                c.benchmark = true;
                                c.phase = 0;
                                c.phase_time = 0.;
                                c.count = COUNTS[0];
                            }
                            "feed" => c.fog = !c.fog,
                            "radar" => c.radar = !c.radar,
                            "explode" => c.flash = 0.,
                            "zoom+" => c.camera[2] = (c.camera[2] * 1.3).min(6.),
                            "zoom-" => c.camera[2] = (c.camera[2] / 1.3).max(0.12),
                            "speed+" => {
                                let speed = if c.clock.speed() == SimulationSpeed::One {
                                    SimulationSpeed::Two
                                } else {
                                    SimulationSpeed::Four
                                };
                                c.clock.set_speed(speed);
                            }
                            "speed-" => {
                                let speed = if c.clock.speed() == SimulationSpeed::Four {
                                    SimulationSpeed::Two
                                } else {
                                    SimulationSpeed::One
                                };
                                c.clock.set_speed(speed);
                            }
                            value => {
                                if let Ok(n) = value.parse::<u32>() {
                                    c.count = n.clamp(1, 100000);
                                }
                            }
                        }
                        logging::log("ACTION", &command);
                    }
                });
                work.input_ms = frame_started.elapsed().as_secs_f64()*1000.;
                let now = Instant::now();
                let elapsed = now - last;
                let acquire_started = Instant::now();
                let frame = match surface.get_current_texture() {
                    Ok(frame) => frame,
                    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                        surface.configure(&gpu.device, &config);
                        return;
                    }
                    Err(error) => {
                        logging::log("SURFACE_ERROR", &error.to_string());
                        return;
                    }
                };
                // A failed surface acquisition must not advance a simulation
                // batch that the GPU never receives. Retain its elapsed wall
                // time for the next accepted frame instead of discarding it.
                work.surface_acquire_ms = acquire_started.elapsed().as_secs_f64()*1000.;
                last = now;
                frame_timings.record_seconds((now - last_frame).as_secs_f64());
                last_frame = now;
                app.world_mut().resource_mut::<Control>().frame_elapsed = elapsed;
                let simulation_started = Instant::now();
                app.update();
                work.simulation_ms = simulation_started.elapsed().as_secs_f64()*1000.;
                let c = app.world().resource::<Control>().clone();
                if let Some(error) = &c.clock_error {
                    logging::log("FATAL", error);
                    elwt.exit();
                    return;
                }
                let upload_started = Instant::now();
                apply(&mut gpu, &c);
                let upload_bytes =
                    gpu.upload_terrain(&mut app.world_mut().resource_mut::<WorldSurface>().terrain);
                work.upload_ms = upload_started.elapsed().as_secs_f64()*1000.;
                let start = Instant::now();
                gpu.render(&frame.texture.create_view(&Default::default()), c.steps);
                work.encode_submit_ms = start.elapsed().as_secs_f64()*1000.;
                submit_ms += work.encode_submit_ms;
                let present_started = Instant::now();
                frame.present();
                work.present_ms = present_started.elapsed().as_secs_f64()*1000.;
                work.tick = c.clock.tick();
                frames += 1;
                #[cfg(not(target_arch = "wasm32"))]
                if automated {
                    for (i, (tick, label)) in [(1u64, "start"), (1200, "dam"), (3600, "drained")]
                        .iter()
                        .enumerate()
                    {
                        if !captured[i] && c.clock.tick() >= *tick {
                            let capture_started = Instant::now();
                            let image = gpu.target(config.width, config.height);
                            gpu.render(&image.create_view(&Default::default()), 0);
                            let path = format!("{check_prefix}-{label}.png");
                            if let Err(error) = gpu.capture(&image, std::path::Path::new(&path)) {
                                logging::log(
                                    "FATAL",
                                    &format!("Automated river capture failed: {error}"),
                                );
                                elwt.exit();
                                return;
                            }
                            work.capture_ms += capture_started.elapsed().as_secs_f64()*1000.;
                            logging::log("CAPTURE", &path);
                            logging::log("CAPTURE_TIMING", &serde_json::json!({"frame":work.frame,"tick":work.tick,"label":label,"cpu_wall_ms":capture_started.elapsed().as_secs_f64()*1000.}).to_string());
                            captured[i] = true;
                        }
                    }
                    if c.clock.tick() >= 3600 {
                        let surface = app.world().resource::<WorldSurface>();
                        let state = surface
                            .river
                            .as_ref()
                            .expect("automated river initialized")
                            .describe();
                        let conserved = state["conserved"].as_bool().unwrap_or(false);
                        let report = serde_json::json!({"status":if conserved {"passed"} else {"failed"},"conserved":conserved,"kind":"automated_ground_water_visual_check_not_full_game", "tick":c.clock.tick(), "requested_speed":c.clock.speed().multiplier(),"captures":captured,"ground_configuration":crate::ground::GroundConfig::default(),"state":state});
                        if let Err(error) = std::fs::write(
                            &report_in_loop,
                            serde_json::to_vec_pretty(&report).unwrap(),
                        ) {
                            logging::log("FATAL", &format!("Writing river report: {error}"));
                            elwt.exit();
                            return;
                        }
                        logging::log("RIVER_CHECK_RESULT", &report.to_string());
                        work.total_work_ms = frame_started.elapsed().as_secs_f64()*1000.;
                        frame_profile.record(&work);
                        logging::log("FRAME_FINAL", &serde_json::to_string(&work).unwrap());
                        logging::log("FRAME_STAGES", &frame_profile.summary().to_string());
                        completed_in_loop.store(conserved, std::sync::atomic::Ordering::Relaxed);
                        elwt.exit();
                        return;
                    }
                }
                let reporting_started = Instant::now();
                let elapsed = measured.elapsed().as_secs_f64();
                let report_due = elapsed >= 1.;
                if report_due {
                    let fps = frames as f64 / elapsed;
                    let actual =
                        (c.clock.tick() - previous_ticks) as f64 / TACTICAL_HZ as f64 / elapsed;
                    let timing = frame_timings
                        .summary()
                        .expect("at least one frame interval");
                    let text = format!(
                        "{} agents · {:0.0} FPS · submitted {:0.2}x/requested {}x · backlog {} ticks · season {:.1}% · frame median/p95/p99/max {:.2}/{:.2}/{:.2}/{:.2} ms · CPU encode+submit {:.2} ms · {}",
                        c.count,
                        fps,
                        actual,
                        c.clock.speed().multiplier(),
                        c.clock.pending_ticks(),
                        c.clock.season_phase() * 100.,
                        timing.median_ms,
                        timing.p95_ms,
                        timing.p99_ms,
                        timing.max_ms,
                        submit_ms / frames as f64,
                        gpu.info.name
                    );
                    let river_text = if c.river_fixture {
                        let surface = app.world().resource::<WorldSurface>();
                        let river = surface.river.as_ref().expect("river fixture initialized");
                        format!(
                            " · RIVER {} · {:.2}ML upstream · G dam / J breach",
                            if river.dam_closed {
                                "dam closed"
                            } else {
                                "open"
                            },
                            river.describe()["upstream_liters"].as_f64().unwrap_or(0.) / 1_000_000.
                        )
                    } else {
                        String::new()
                    };
                    window.set_title(&format!("Last World · Bevy GPU · {text}{river_text}"));
                    logging::log("PERF", &text);
                    if let Some(river) = &app.world().resource::<WorldSurface>().river {
                        logging::log("WATER", &river.describe().to_string());
                    }
                    logging::log(
                        "PERF_DETAIL",
                        &serde_json::json!({
                            "workload_kind": if c.river_fixture { "isolated_river_water_fixture_not_campaign" } else { "isolated_movement_renderer_not_campaign" },
                            "run_mode": "windowed_vsync",
                            "agents": c.count, "requested_speed": c.clock.speed().multiplier(),
                            "authoritative_tick": c.clock.tick(),
                            "pending_ticks": c.clock.pending_ticks(),
                            "season_cycle": c.clock.season_cycle_index(),
                            "season_tick": c.clock.season_tick(),
                            "season_environment_effects_implemented": false,
                            "submitted_simulation_seconds_per_wall_second": actual,
                            "gpu_completed_simulation_speed": null, "frame_intervals": timing,
                            "cpu_encode_submit_mean_ms": submit_ms / frames as f64,
                            "gpu_execution_ms": null,
                            "quantiles": "nearest_rank_recent_retained_samples",
                            "max_and_mean": "all_intervals_in_reporting_window",
                            "device": gpu.info.name,
                            "backend": format!("{:?}", gpu.info.backend)
                            ,"last_frame_terrain_upload_payload_bytes": upload_bytes,
                            "water_fixture": c.river_fixture,
                            "coupled_military_water_rules_present": false
                        })
                        .to_string(),
                    );
                    #[cfg(target_arch = "wasm32")]
                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        if let Some(status) = document.get_element_by_id("native-status") {
                            status.set_text_content(Some(&text));
                        }
                    }
                    measured = Instant::now();
                    frames = 0;
                    previous_ticks = c.clock.tick();
                    frame_timings.clear();
                    submit_ms = 0.;
                }
                work.reporting_ms = reporting_started.elapsed().as_secs_f64()*1000.;
                work.total_work_ms = frame_started.elapsed().as_secs_f64()*1000.;
                frame_profile.record(&work);
                if work.slow() { logging::log("SLOW_FRAME", &serde_json::to_string(&work).unwrap()); }
                if report_due { logging::log("FRAME_STAGES", &frame_profile.summary().to_string()); frame_profile.clear(); }
                last_finished = Instant::now();
            }
            _ => {}
        },
        Event::AboutToWait => window.request_redraw(),
        _ => {}
    };
    #[cfg(not(target_arch = "wasm32"))]
    event_loop.run(handler).map_err(|e| e.to_string())?;
    #[cfg(not(target_arch = "wasm32"))]
    if automated && !completed.load(std::sync::atomic::Ordering::Relaxed) {
        let failed = serde_json::json!({"status":"failed","conserved":false,"reason":"Automated window ended before all scheduled checks/captures completed; inspect console.txt"});
        if !std::path::Path::new(&report_path).exists() {
            let _ = std::fs::write(&report_path, failed.to_string());
        }
        return Err("Automated river check did not reach its completion tick".into());
    }
    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        event_loop.spawn(handler);
    }
    Ok(())
}
#[cfg(not(target_arch = "wasm32"))]
pub async fn headless(
    counts: Vec<u32>,
    seconds: f64,
    capture: Option<String>,
    check: bool,
    backends: Option<wgpu::Backends>,
    speed: SimulationSpeed,
) -> Result<(), String> {
    if counts.is_empty()
        || counts
            .iter()
            .any(|&n| n == 0 || n as usize > crate::gpu::CAPACITY)
    {
        return Err(
            "Headless counts must be nonempty and inside the allocated agent capacity".into(),
        );
    }
    if !seconds.is_finite() || seconds <= 0. {
        return Err("Headless duration must be finite and positive".into());
    }
    let run_id = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let (mut gpu, _, terrain) = Gpu::new(None, 78421, counts[0], 1280, 800, backends).await?;
    let target = gpu.target(1280, 800);
    let view = target.create_view(&Default::default());
    let mut app = app();
    app.insert_resource(WorldSurface {
        terrain,
        river: None,
    });
    app.world_mut()
        .resource_mut::<Control>()
        .clock
        .set_speed(speed);
    let mut reports = Vec::new();
    logging::log(
        "MODE",
        &format!(
            "Synchronized isolated headless benchmark; {:?}. Synthetic minimum 50ms frame delta: FPS is not real-time simulation speed. Software adapters are proxy checks, not hardware performance evidence.",
            gpu.info.device_type
        ),
    );
    for count in counts {
        app.world_mut().resource_mut::<Control>().count = count;
        append_benchmark_history(&serde_json::json!({
            "event":"phase_started", "run_id":run_id, "agents":count,
            "device":gpu.info.name, "backend":format!("{:?}", gpu.info.backend)
        }))?;
        let first = gpu.read_buffer(3, 32);
        let phase_start_ticks = app.world().resource::<Control>().clock.tick();
        let start = Instant::now();
        let mut last = start;
        let mut frames = 0;
        let mut timings = FrameTimings::new(8192);
        loop {
            let begin = Instant::now();
            let elapsed = (begin - last).max(Duration::from_millis(50));
            last = begin;
            app.world_mut().resource_mut::<Control>().frame_elapsed = elapsed;
            app.update();
            let c = app.world().resource::<Control>().clone();
            if let Some(error) = &c.clock_error {
                return Err(error.clone());
            }
            apply(&mut gpu, &c);
            gpu.render(&view, c.steps);
            gpu.device
                .poll(wgpu::PollType::wait_indefinitely())
                .map_err(|e| e.to_string())?;
            frames += 1;
            timings.record_seconds(begin.elapsed().as_secs_f64());
            if start.elapsed().as_secs_f64() >= seconds && frames >= 2 {
                break;
            }
        }
        let elapsed = start.elapsed().as_secs_f64();
        let stats = timings
            .summary()
            .ok_or("No valid frame durations recorded")?;
        let control = app.world().resource::<Control>();
        let simulated_ticks = control.clock.tick() - phase_start_ticks;
        let mut report = serde_json::json!({
            "schema_version": 3, "event":"phase_result", "run_id": run_id,
            "workload_kind": "isolated_gpu_movement_and_rendering_benchmark",
            "is_integrated_campaign": false,
            "qualification": if gpu.info.device_type == wgpu::DeviceType::Cpu {"proxy_check"} else {"isolated_benchmark_only"},
            "agents":count, "military_rules_present":false,
            "civilian_and_ecology_rules_present":false,
            "map_meters":[crate::terrain::WW,crate::terrain::HH],
            "seed":78421, "frames":frames,"wall_seconds":elapsed,
            "frames_per_second":frames as f64/elapsed,
            "frame_median_ms":stats.median_ms,"frame_p95_ms":stats.p95_ms,
            "frame_p99_ms":stats.p99_ms,"frame_max_ms":stats.max_ms,
            "frame_durations":stats,
            "quantiles":"nearest_rank_recent_retained_samples",
            "max_and_mean":"all_frames_in_phase",
            "simulated_ticks":simulated_ticks,
            "simulated_seconds":simulated_ticks as f64 / TACTICAL_HZ as f64,
            "synthetic_simulation_seconds_per_wall_second":simulated_ticks as f64 / TACTICAL_HZ as f64 / elapsed,
            "time_policy":"minimum_50ms_synthetic_frame_delta_not_real_time_qualification",
            "device":gpu.info.name,"driver":gpu.info.driver,"driver_info":gpu.info.driver_info,
            "device_type":format!("{:?}",gpu.info.device_type),
            "backend":format!("{:?}",gpu.info.backend),"synchronized_headless":true,
            "known_gpu_buffer_payload_bytes":gpu.buffers.iter().map(wgpu::Buffer::size).sum::<u64>(),
            "textures_driver_overhead_and_staging_excluded":true,
            "process_ram_bytes":null,"driver_vram_usage_bytes":null,
            "host":crate::diagnostics::host(),
            "correctness":if check {"pending"} else {"not_requested"}
        });
        report["simulation_clock"] = serde_json::json!({
            "requested_speed": speed.multiplier(),
            "pending_ticks": control.clock.pending_ticks(),
            "season_cycle": control.clock.season_cycle_index(),
            "season_tick": control.clock.season_tick(),
            "season_environment_effects_implemented": false
        });
        let checked: Result<(), String> = (|| {
            if check {
                let data = gpu.read_buffer(3, count as u64 * 32);
                let agents = bytemuck::cast_slice::<u8, crate::gpu::Agent>(&data);
                if agents
                    .iter()
                    .any(|a| !a.pos[0].is_finite() || !a.pos[1].is_finite())
                {
                    return Err("GPU wrote non-finite coordinates".into());
                }
                if data[..8] == first[..8] {
                    return Err("GPU movement did not change the first agent".into());
                }
                let heads = gpu.read_buffer(4, 60000 * 4);
                let heads = bytemuck::cast_slice::<u8, i32>(&heads);
                let mut seen = vec![false; count as usize];
                for head in heads {
                    let mut i = *head;
                    let mut chain = 0;
                    while i >= 0 {
                        let id = i as usize;
                        if id >= agents.len() || seen[id] || chain > count {
                            return Err("GPU spatial index is corrupt".into());
                        }
                        seen[id] = true;
                        chain += 1;
                        i = agents[id].next;
                    }
                }
                if seen.iter().filter(|x| **x).count() != count as usize {
                    return Err("GPU spatial index dropped an agent".into());
                }
                logging::log(
                    "GPU_CHECK",
                    &format!(
                        "{count} finite agents, first agent movement checked, every spatial link checked exactly once"
                    ),
                );
            }
            Ok(())
        })();
        report["correctness"] = if check {
            if checked.is_ok() { "passed" } else { "failed" }
        } else {
            "not_requested"
        }
        .into();
        if let Err(error) = &checked {
            report["failure"] = error.clone().into();
        }
        logging::log("BENCHMARK_RESULT", &report.to_string());
        append_benchmark_history(&report)?;
        checked?;
        reports.push(report);
    }
    if let Some(path) = capture {
        gpu.capture(&target, std::path::Path::new(&path))?;
        logging::log("CAPTURE", &path);
    }
    std::fs::write(
        "benchmark-results.json.part",
        serde_json::to_vec_pretty(&reports).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename("benchmark-results.json.part", "benchmark-results.json")
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
fn append_benchmark_history(report: &serde_json::Value) -> Result<(), String> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("benchmark-history.jsonl")
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut file, report).map_err(|e| e.to_string())?;
    file.write_all(b"\n").map_err(|e| e.to_string())
}

#[cfg(target_arch = "wasm32")]
thread_local! {static WEB_COMMANDS:std::cell::RefCell<Vec<String>>=const{std::cell::RefCell::new(Vec::new())};}
#[cfg(target_arch = "wasm32")]
pub fn web_command(command: &str) {
    WEB_COMMANDS.with(|q| q.borrow_mut().push(command.to_owned()));
}

#[cfg(test)]
mod clock_integration_tests {
    use super::*;

    #[test]
    fn control_consumes_elapsed_once_and_drains_only_remaining_tick_debt() {
        let mut app = app();
        app.world_mut().resource_mut::<Control>().frame_elapsed = Duration::from_secs(1);
        app.update();
        {
            let control = app.world().resource::<Control>();
            assert_eq!(control.steps, MAX_STEPS_PER_FRAME);
            assert_eq!(control.clock.tick(), 16);
            assert_eq!(control.clock.pending_ticks(), 4);
            assert_eq!(control.frame_elapsed, Duration::ZERO);
            assert!(control.clock_error.is_none());
        }
        // No new elapsed input: execute owed ticks without charging a frame twice.
        app.update();
        {
            let control = app.world().resource::<Control>();
            assert_eq!(control.steps, 4);
            assert_eq!(control.clock.tick(), 20);
            assert_eq!(control.clock.pending_ticks(), 0);
        }
        app.update();
        let control = app.world().resource::<Control>();
        assert_eq!(control.steps, 0);
        assert_eq!(control.clock.tick(), 20);
    }

    #[test]
    fn four_x_control_retains_unclamped_time_and_pause_preserves_owed_ticks() {
        let mut app = app();
        {
            let mut control = app.world_mut().resource_mut::<Control>();
            control.clock.set_speed(SimulationSpeed::Four);
            control.frame_elapsed = Duration::from_secs(1);
        }
        app.update();
        {
            let control = app.world().resource::<Control>();
            assert_eq!(control.steps, 16);
            assert_eq!(control.clock.tick(), 16);
            assert_eq!(control.clock.pending_ticks(), 64);
        }
        {
            let mut control = app.world_mut().resource_mut::<Control>();
            control.paused = true;
            control.frame_elapsed = Duration::from_secs(10);
        }
        app.update();
        {
            let control = app.world().resource::<Control>();
            assert_eq!(control.steps, 0);
            assert_eq!(control.clock.tick(), 16);
            assert_eq!(control.clock.pending_ticks(), 64);
            assert_eq!(control.frame_elapsed, Duration::ZERO);
        }
        app.world_mut().resource_mut::<Control>().paused = false;
        app.update();
        let control = app.world().resource::<Control>();
        assert_eq!(control.steps, 16);
        assert_eq!(control.clock.tick(), 32);
        assert_eq!(control.clock.pending_ticks(), 48);
    }
}
