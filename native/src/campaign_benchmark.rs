//! Native coupled stress harness. Commands and elapsed wall time enter the
//! same authoritative campaign; this module never credits stocks or animals.
use crate::{
    battle_ai::BattleDirector,
    benchmark_orders::OrderExercise,
    campaign::{Campaign, CampaignSave, DetailedAnimal},
    campaign_scenarios::{capture_candidate, visible_release_point},
    campaign_support,
    construction::{BuildKind, JobPhase, TEAM_BASES},
    ecology::{Sex, Stage},
    logging,
    tactics::Role,
    terrain::Terrain,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const BENCHMARK_STEPS_PER_CALL: u32 = 64;
const MAX_SAMPLES: usize = 1_000_000;
const MEMORY_GUARD_BYTES: u64 = 6 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BenchmarkMode {
    Cpu,
    Window,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FaultProbe {
    AfterCommitCrashOnce,
    AfterCommitCrashAlways,
    ContinueAfterSkippedProbe,
}
const FAILED_PROBE_CASE: &str = "probe-after-commit-crash-always";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkCase {
    pub id: String,
    pub mode: BenchmarkMode,
    pub requested_rate: u32,
    pub military: u32,
    pub workers: u32,
    pub seed: u32,
    pub wall_seconds: u64,
    pub target_ticks: Option<u64>,
    pub timeout_seconds: u64,
    pub checkpoint_group: String,
    pub backend: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub window_size: [u32; 2],
    pub capture_last_frame: bool,
    pub materialized_wildlife_target: u32,
    #[serde(default)]
    pub fault_probe: Option<FaultProbe>,
    #[serde(default)]
    pub harness_control_probe: bool,
    #[serde(default)]
    pub speed_ramp: Option<SpeedRampConfig>,
    #[serde(default)]
    pub battle_scenario: bool,
}

/// One world/device load, followed by bounded measured stages. These values
/// come from the compiled plan rather than changing normal gameplay speeds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpeedRampConfig {
    pub rates: Vec<u32>,
    pub initial_warmup_seconds: u64,
    pub settle_seconds: u64,
    pub window_seconds: u64,
    pub minimum_windows: u32,
    pub maximum_windows: u32,
}
impl SpeedRampConfig {
    fn valid(&self) -> bool {
        self.rates == [1, 2, 4, 8, 16, 32, 64, 100]
            && (15..=60).contains(&self.initial_warmup_seconds)
            && (5..=30).contains(&self.settle_seconds)
            && (10..=60).contains(&self.window_seconds)
            && self.minimum_windows >= 2
            && self.minimum_windows <= self.maximum_windows
            && self.maximum_windows <= 4
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RampPhase {
    Loading,
    Warmup,
    Settling,
    Measuring,
    Complete,
}
impl RampPhase {
    fn label(self) -> &'static str {
        match self {
            Self::Loading => "waiting_for_first_native_presentation",
            Self::Warmup => "initial_live_warmup",
            Self::Settling => "live_rate_settling",
            Self::Measuring => "measurement",
            Self::Complete => "complete",
        }
    }
}

#[derive(Clone, Copy, Default)]
struct RampMark {
    wall: Duration,
    tick: u64,
    pending: u64,
}

struct SpeedRamp {
    config: SpeedRampConfig,
    index: usize,
    phase: RampPhase,
    phase_started: Duration,
    measurement: RampMark,
    window: RampMark,
    windows: Vec<Value>,
    consecutive_overloaded: u32,
    stages: Vec<Value>,
    warmup_intervals: Vec<Value>,
    highest_sustained: Option<u32>,
    completion_reason: Option<&'static str>,
    ticks: Samples,
    frames: Samples,
    omit_next_frame: bool,
    omit_next_presentation: bool,
    loading_unpresented: u64,
    window_presented: u64,
    window_unpresented: u64,
    stage_presented: u64,
    stage_unpresented: u64,
}
impl SpeedRamp {
    fn new(config: SpeedRampConfig) -> Self {
        Self {
            config,
            index: 0,
            phase: RampPhase::Loading,
            phase_started: Duration::ZERO,
            measurement: RampMark::default(),
            window: RampMark::default(),
            windows: Vec::new(),
            consecutive_overloaded: 0,
            stages: Vec::new(),
            warmup_intervals: Vec::new(),
            highest_sustained: None,
            completion_reason: None,
            ticks: Samples::default(),
            frames: Samples::default(),
            omit_next_frame: true,
            omit_next_presentation: false,
            loading_unpresented: 0,
            window_presented: 0,
            window_unpresented: 0,
            stage_presented: 0,
            stage_unpresented: 0,
        }
    }
    fn rate(&self) -> u32 {
        self.config.rates[self.index]
    }
    fn boundary_due(&self, wall: Duration) -> bool {
        let seconds = match self.phase {
            RampPhase::Loading => return false,
            RampPhase::Warmup => self.config.initial_warmup_seconds,
            RampPhase::Settling => self.config.settle_seconds,
            RampPhase::Measuring => self.config.window_seconds,
            RampPhase::Complete => return false,
        };
        wall.saturating_sub(self.phase_started) >= Duration::from_secs(seconds)
    }
    fn record_ticks(&mut self, times: &[u64]) {
        if self.phase == RampPhase::Measuring {
            for &ns in times {
                self.ticks.push(ns);
            }
        }
    }
    fn record_frame(&mut self, elapsed: Duration) {
        if std::mem::take(&mut self.omit_next_frame) {
            return;
        }
        if self.phase == RampPhase::Measuring {
            self.frames
                .push(elapsed.as_nanos().min(u64::MAX as u128) as u64);
        }
    }
    fn begin_live_warmup(&mut self, mark: RampMark) {
        if self.phase != RampPhase::Loading {
            return;
        }
        self.warmup_intervals.push(json!({
            "kind":"first_native_presentation_loading","wall_seconds":mark.wall.as_secs_f64(),
            "start_wall_seconds":0.,"end_wall_seconds":mark.wall.as_secs_f64(),
            "end_tick":mark.tick,"pending_ticks_end":mark.pending,
            "unpresented_attempts":self.loading_unpresented,
            "first_successful_presentation":true,"world_kept_running":true,
            "included_in_stage_performance":false
        }));
        self.phase = RampPhase::Warmup;
        self.phase_started = mark.wall;
        self.omit_next_frame = true;
    }
    fn record_presentation(&mut self, presented: bool) {
        if self.phase == RampPhase::Loading {
            self.loading_unpresented += u64::from(!presented);
            return;
        }
        if std::mem::take(&mut self.omit_next_presentation) {
            return;
        }
        if self.phase == RampPhase::Measuring {
            if presented {
                self.window_presented += 1;
                self.stage_presented += 1;
            } else {
                self.window_unpresented += 1;
                self.stage_unpresented += 1;
            }
        }
    }
    /// Called at a boundary only after the session has charged its processing
    /// tail at the OLD rate. The next caller subtracts that precharged tail.
    fn observe_boundary(&mut self, mark: RampMark) {
        if !self.boundary_due(mark.wall) {
            return;
        }
        self.omit_next_frame = true;
        self.omit_next_presentation = true;
        if matches!(self.phase, RampPhase::Warmup | RampPhase::Settling) {
            self.warmup_intervals.push(json!({
                "kind":self.phase.label(),"requested_rate":self.rate(),
                "wall_seconds":mark.wall.saturating_sub(self.phase_started).as_secs_f64(),
                "start_wall_seconds":self.phase_started.as_secs_f64(),
                "end_wall_seconds":mark.wall.as_secs_f64(),
                "end_tick":mark.tick,"pending_ticks_end":mark.pending,
                "world_kept_running":true,"included_in_stage_performance":false
            }));
            self.phase = RampPhase::Measuring;
            self.phase_started = mark.wall;
            self.measurement = mark;
            self.window = mark;
            return;
        }
        let seconds = mark.wall.saturating_sub(self.window.wall).as_secs_f64();
        let achieved = mark.tick.saturating_sub(self.window.tick) as f64 / 20. / seconds;
        let debt_growth = mark.pending.saturating_sub(self.window.pending);
        let presentation_valid = self.window_presented > 0 && self.window_unpresented == 0;
        let overloaded =
            presentation_valid && ramp_window_overloaded(self.rate(), achieved, debt_growth);
        self.consecutive_overloaded = if overloaded {
            self.consecutive_overloaded + 1
        } else {
            0
        };
        self.windows.push(json!({
            "window":self.windows.len()+1,"wall_seconds":seconds,
            "start_wall_seconds":self.window.wall.as_secs_f64(),"end_wall_seconds":mark.wall.as_secs_f64(),
            "start_tick":self.window.tick,"end_tick":mark.tick,
            "requested_rate":self.rate(),"achieved_rate":achieved,
            "pending_ticks_start":self.window.pending,"pending_ticks_end":mark.pending,
            "debt_growth_ticks":debt_growth,"capacity_limited":overloaded,
            "presented_frames":self.window_presented,"unpresented_attempts":self.window_unpresented,
            "native_presentation_evidence_valid":presentation_valid
        }));
        self.window_presented = 0;
        self.window_unpresented = 0;
        self.window = mark;
        self.phase_started = mark.wall;
        let elapsed = mark
            .wall
            .saturating_sub(self.measurement.wall)
            .as_secs_f64();
        let total_ticks = mark.tick.saturating_sub(self.measurement.tick);
        let achieved = total_ticks as f64 / 20. / elapsed;
        let debt_growth = mark.pending.saturating_sub(self.measurement.pending);
        let presentation_valid = self.stage_presented > 0
            && self.stage_unpresented == 0
            && self
                .windows
                .iter()
                .all(|w| w["native_presentation_evidence_valid"].as_bool() == Some(true));
        let sustained = achieved >= self.rate() as f64 * 0.98
            && debt_growth <= 4
            && self.measurement.pending <= 4
            && mark.pending <= 4
            && presentation_valid;
        let enough = self.windows.len() >= self.config.minimum_windows as usize;
        let clearly_limited = enough && self.consecutive_overloaded >= 2;
        let confirmation_exhausted = self.windows.len() >= self.config.maximum_windows as usize;
        if !enough
            || (!sustained && !clearly_limited && !confirmation_exhausted && presentation_valid)
        {
            return;
        }
        let rate = self.rate();
        let reason = if !presentation_valid {
            "native_presentation_interrupted_not_qualified"
        } else if sustained {
            "sustained"
        } else if clearly_limited {
            "two_consecutive_capacity_limited_windows"
        } else {
            "bounded_confirmation_did_not_qualify_rate"
        };
        self.stages.push(json!({
            "requested_rate":rate,"achieved_simulation_rate":achieved,
            "requested_rate_sustained":sustained,"decision":reason,
            "wall_seconds":elapsed,"start_tick":self.measurement.tick,"end_tick":mark.tick,
            "measurement_start_wall_seconds":self.measurement.wall.as_secs_f64(),
            "measurement_end_wall_seconds":mark.wall.as_secs_f64(),
            "committed_ticks":total_ticks,"pending_ticks_start":self.measurement.pending,
            "pending_ticks_end":mark.pending,"debt_growth_ticks":debt_growth,
            "presented_frames":self.stage_presented,"unpresented_attempts":self.stage_unpresented,
            "native_presentation_evidence_valid":presentation_valid,
            "actual_fixed_20hz_tick":self.ticks.report(),
            "complete_frame_intervals":self.frames.report(),"windows":self.windows
        }));
        if sustained {
            self.highest_sustained = Some(rate);
        }
        if !sustained || self.index + 1 == self.config.rates.len() {
            self.phase = RampPhase::Complete;
            self.completion_reason = Some(if !presentation_valid {
                "higher_rates_not_run_without_valid_native_presentation"
            } else if sustained {
                "all_planned_rates_measured_and_sustained"
            } else if clearly_limited {
                "higher_rates_not_run_after_clear_capacity_limit"
            } else {
                "higher_rates_not_run_after_inconclusive_capacity_confirmation"
            });
        } else {
            self.index += 1;
            self.phase = RampPhase::Settling;
            self.windows.clear();
            self.consecutive_overloaded = 0;
            self.ticks = Samples::default();
            self.frames = Samples::default();
            self.stage_presented = 0;
            self.stage_unpresented = 0;
        }
    }
    fn progress_report(&self) -> Value {
        json!({
            "phase":self.phase.label(),"current_requested_rate":self.rate(),
            "highest_sustained_rate":self.highest_sustained,
            "completed_stages":self.stages,"measurement_windows_completed":self.windows.len(),
            "current_measurement_windows":self.windows,
            "completion_reason":self.completion_reason,
            "remaining_rates_not_run":if self.phase == RampPhase::Complete {self.config.rates[self.index+1..].to_vec()}else{Vec::new()},
            "initial_warmup_seconds":self.config.initial_warmup_seconds,
            "settle_seconds_per_increase":self.config.settle_seconds,
            "measurement_window_seconds":self.config.window_seconds,
            "minimum_measurement_windows":self.config.minimum_windows,
            "maximum_measurement_windows":self.config.maximum_windows,
            "warmup_intervals":self.warmup_intervals,
            "loading_unpresented_attempts":self.loading_unpresented,
            "current_window_presented_frames":self.window_presented,
            "current_window_unpresented_attempts":self.window_unpresented,
            "native_presentation_policy":"After first successful GPU presentation, run the full live warm-up. Each measured window requires at least one successful native presentation and zero unpresented attempts; boundary-crossing frames are excluded. Interrupted presentation is unqualified evidence, never a GPU-capacity result.",
            "capacity_rule":"Stop higher requests after two consecutive measured windows with <98% achieved and >4 ticks growing debt, or <90% achieved. Mixed results receive bounded confirmation; no higher rate without qualification.",
            "sustained_rule":"At least two full measured windows; >=98% requested rate; <=4 ticks total debt growth and <=4 pending ticks at both measurement endpoints. Inherited backlog cannot qualify a faster rate.",
            "sample_scope":"Stage samples exclude load, live warm-up, live settling, boundary-crossing frames and terminal captures; whole-session diagnostics include first native presentation loading and live warm-up/settling.",
            "world_and_device_loaded_once":true,"rules_and_population_unchanged":true,
            "clock_debt_preserved":true,"unrun_higher_rates_are_not_passes":true
        })
    }
}
fn ramp_window_overloaded(requested: u32, achieved: f64, debt_growth: u64) -> bool {
    achieved < requested as f64 * 0.90 || (achieved < requested as f64 * 0.98 && debt_growth > 4)
}
fn consume_precharged_elapsed(elapsed: Duration, credit: &mut Duration) -> Duration {
    let consumed = elapsed.min(*credit);
    *credit = credit.saturating_sub(consumed);
    elapsed.saturating_sub(consumed)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkPlan {
    pub schema_version: u32,
    pub name: String,
    pub scope: String,
    pub tick_hz: u32,
    pub benchmark_steps_per_call: u32,
    pub cases: Vec<BenchmarkCase>,
    #[serde(flatten)]
    pub metadata: BTreeMap<String, Value>,
}
pub fn plan() -> BenchmarkPlan {
    serde_json::from_str(include_str!("../qualification/benchmark-plan.json"))
        .expect("Compiled benchmark plan must be valid")
}
pub fn case(id: &str) -> Option<BenchmarkCase> {
    plan().cases.into_iter().find(|c| c.id == id).map(|mut c| {
        c.window_size = [
            c.width.unwrap_or(c.window_size[0]),
            c.height.unwrap_or(c.window_size[1]),
        ];
        c
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkProgress {
    pub done: bool,
    pub elapsed_wall_seconds: f64,
    pub simulated_ticks: u64,
    pub achieved_simulation_rate: f64,
    pub pending_ticks: u64,
    pub stop_reason: Option<String>,
    pub last_advanced_ticks: u32,
}

#[derive(Default)]
struct Samples {
    count: u64,
    total_ns: u128,
    max_ns: u64,
    values: Vec<u64>,
}
impl Samples {
    fn push(&mut self, ns: u64) {
        self.count += 1;
        self.total_ns += ns as u128;
        self.max_ns = self.max_ns.max(ns);
        if self.values.len() < MAX_SAMPLES {
            self.values.push(ns);
        }
    }
    fn report(&self) -> Value {
        let mut sorted = self.values.clone();
        sorted.sort_unstable();
        let percentile = |p: usize| {
            sorted
                .get((sorted.len() * p).div_ceil(100).max(1) - 1)
                .map(|x| *x as f64 / 1_000_000.)
        };
        json!({"count":self.count,"mean_ms":if self.count>0 {Some(self.total_ns as f64/self.count as f64/1_000_000.)}else{None},"p50_ms":if self.count as usize==self.values.len(){percentile(50)}else{None},"p95_ms":if self.count as usize==self.values.len(){percentile(95)}else{None},"p99_ms":if self.count as usize==self.values.len(){percentile(99)}else{None},"retained_subset_p95_ms":percentile(95),"quantile_method":"nearest_rank","quantile_scope":if self.count as usize==self.values.len(){"whole_case"}else{"first_1000000_samples_only; whole_case_quantiles_unavailable"},"max_ms":self.max_ns as f64/1_000_000.,"distribution_complete":self.count as usize==self.values.len(),"sample_limit":MAX_SAMPLES})
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct ConservationDriver {
    selected: Option<u32>,
    next_action_tick: u64,
    adult_ids: BTreeSet<u64>,
    pair_delivered: bool,
    research: bool,
    breeding: bool,
    offspring: bool,
    released: bool,
    release_request: Option<(u64, u32, [f32; 2])>,
    milestones: Vec<Value>,
    errors: Vec<Value>,
}
#[derive(Clone, Serialize, Deserialize)]
struct ControllerState {
    orders: OrderExercise,
    #[serde(default)]
    battle: BattleDirector,
    conservation: ConservationDriver,
    tracked_wildlife: Vec<u64>,
    initial_materialized: u32,
    completed_cases: Vec<String>,
}
#[derive(Serialize, Deserialize)]
struct BenchmarkCheckpoint {
    version: u32,
    #[serde(default = "first_attempt")]
    attempt: u8,
    checkpoint_group: String,
    seed: u32,
    military: u32,
    workers: u32,
    source_identity: Value,
    campaign: CampaignSave,
    controller: ControllerState,
    last_case: String,
    last_report: Value,
}

pub struct BenchmarkSession {
    pub case: BenchmarkCase,
    pub process_id: u32,
    pub session_id: String,
    pub started_utc_ms: u64,
    pub attempt: u8,
    pub replay_origin: String,
    pub rejected_checkpoint: Option<Value>,
    pub report_path: PathBuf,
    pub progress_path: PathBuf,
    pub capture_path: PathBuf,
    checkpoint_path: PathBuf,
    _group_lock: GroupLock,
    controller: ControllerState,
    source_identity: Value,
    start_tick: u64,
    start_pending: u64,
    initial: Value,
    initial_physical: Value,
    initial_habitat: Value,
    initial_eco_commits: [u64; 30],
    started: Option<Instant>,
    live_loop_started_utc_ms: Option<u64>,
    measured_wall: Option<Duration>,
    preparation_seconds: f64,
    elapsed_charged: Duration,
    precharged_tail: Duration,
    speed_ramp: Option<SpeedRamp>,
    advance_calls: u64,
    max_pending: u64,
    tick_samples: Samples,
    batch_samples: Samples,
    frame_samples: Samples,
    controller_samples: Samples,
    validation_samples: Samples,
    progress_io_samples: Samples,
    last_pin_tick: Option<u64>,
    last_progress: Duration,
    last_validation_tick: u64,
    validation_count: u64,
    failure: Option<String>,
    done: bool,
    stop_reason: Option<String>,
    rss_max: Option<u64>,
    recovered_success: Option<Value>,
    finished: Option<Value>,
}

struct GroupLock {
    _file: File,
}
impl GroupLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let file = options
            .open(path)
            .map_err(|e| format!("Checkpoint group is in use or cannot be locked: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe extern "C" {
                fn flock(fd: i32, operation: i32) -> i32;
            }
            if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
                return Err("Another native benchmark owns this checkpoint group".into());
            }
        }
        Ok(Self { _file: file })
    }
}
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
fn source_identity() -> Value {
    let executable = std::env::current_exe().ok();
    if let Some(stamp) = executable
        .as_ref()
        .and_then(|p| p.parent())
        .map(|p| p.join("native-build-stamp.json"))
    {
        if let Ok(bytes) = std::fs::read(stamp) {
            if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                return value;
            }
        }
    }
    let metadata = executable.as_ref().and_then(|p| std::fs::metadata(p).ok());
    json!({"status":"build_stamp_unavailable","package_version":env!("CARGO_PKG_VERSION"),"executable":executable,"bytes":metadata.as_ref().map(|m|m.len()),"modified_utc_ms":metadata.and_then(|m|m.modified().ok()).and_then(|t|t.duration_since(UNIX_EPOCH).ok()).map(|d|d.as_millis())})
}
fn report_object(fields: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn atomic_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temporary = path.with_extension(format!("pending-{}", std::process::id()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| format!("Committing {}: {e}", path.display()))
}
fn rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let text = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = text.lines().find(|l| l.starts_with("VmRSS:"))?;
        return line
            .split_whitespace()
            .nth(1)?
            .parse::<u64>()
            .ok()?
            .checked_mul(1024);
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
fn artifact_paths(
    c: &BenchmarkCase,
    output: &Path,
) -> (u32, u64, String, PathBuf, PathBuf, PathBuf) {
    let pid = std::process::id();
    let started = now_ms();
    let session = format!("{}-{pid}-{started}", c.id);
    (
        pid,
        started,
        session.clone(),
        output.join(format!("{session}-report.json")),
        output.join(format!("{session}-progress.json")),
        output.join("captures").join(format!("{session}-final.png")),
    )
}
fn materialize_existing(game: &mut Campaign, target: u32) -> Result<Vec<u64>, String> {
    let mut ids = game.detailed.keys().copied().collect::<Vec<_>>();
    let candidates = game
        .ecology
        .cohorts
        .iter()
        .filter(|c| {
            c.count > 0
                && c.surface_observable
                && c.location_height_mm >= 0
                && c.engineering_busy_until <= game.processed_tick
                && c.breeding_refuge.is_none()
        })
        .map(|c| c.id)
        .collect::<Vec<_>>();
    for owner in candidates {
        loop {
            if ids.len() >= target as usize {
                game.validate()?;
                return Ok(ids);
            }
            let Some(cohort) = game
                .ecology
                .cohorts
                .iter()
                .find(|c| c.id == owner && c.count > 0)
            else {
                break;
            };
            if cohort.count == 1 && game.detailed.contains_key(&owner) {
                break;
            }
            let position = game.cohort_position(cohort);
            let id = game.ecology.materialize_one(owner)?;
            game.physical.observe_position(
                id,
                [(position[0] * 1000.) as i32, (position[1] * 1000.) as i32],
            )?;
            let tick = game.processed_tick;
            game.detailed.insert(
                id,
                DetailedAnimal {
                    position,
                    previous: position,
                    last_seen: tick,
                },
            );
            ids.push(id);
            if id == owner {
                break;
            }
        }
    }
    game.validate()?;
    Ok(ids)
}

/// `output` is one run-owned root shared by cases. Only this group's lock is
/// held; CPU and window groups can coexist without overwriting their states.
pub fn prepare(id: &str, output: &Path) -> Result<(Campaign, BenchmarkSession), String> {
    prepare_for_attempt(id, output, runtime_attempt()?)
}
fn first_attempt() -> u8 {
    1
}
#[derive(Serialize, Deserialize)]
struct RejectedBuild {
    native_source_sha256: String,
    executable_sha256: String,
}
#[derive(Serialize, Deserialize)]
struct RejectedAttempt {
    case_id: String,
    attempt_index: u8,
    source_identity: RejectedBuild,
    reason: String,
}
#[derive(Serialize, Deserialize)]
struct RejectionLedger {
    schema_version: u32,
    rejections: Vec<RejectedAttempt>,
}
fn rejection_ledger(output: &Path) -> Result<RejectionLedger, String> {
    let path = output.join("rejected-attempts.json");
    if !path.exists() {
        return Ok(RejectionLedger {
            schema_version: 1,
            rejections: Vec::new(),
        });
    }
    if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("Rejected-attempt ledger exceeds bounded benchmark metadata size".into());
    }
    let ledger: RejectionLedger =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Invalid rejected-attempt ledger: {e}"))?;
    if ledger.schema_version != 1
        || ledger.rejections.iter().any(|r| {
            !matches!(r.attempt_index, 1 | 2)
                || r.source_identity.native_source_sha256.len() != 64
                || r.source_identity.executable_sha256.len() != 64
                || !r
                    .source_identity
                    .native_source_sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
                || !r
                    .source_identity
                    .executable_sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
        })
    {
        return Err("Invalid build-bound rejected-attempt schema".into());
    }
    Ok(ledger)
}
fn rejected<'a>(
    saved: &BenchmarkCheckpoint,
    ledger: &'a RejectionLedger,
) -> Option<&'a RejectedAttempt> {
    ledger.rejections.iter().find(|r| {
        r.case_id == saved.last_case
            && r.attempt_index == saved.attempt
            && saved.source_identity["native_source_sha256"]
                .as_str()
                .is_some_and(|hash| {
                    hash.eq_ignore_ascii_case(&r.source_identity.native_source_sha256)
                })
            && saved.source_identity["executable_sha256"]
                .as_str()
                .is_some_and(|hash| hash.eq_ignore_ascii_case(&r.source_identity.executable_sha256))
    })
}

fn runtime_attempt() -> Result<u8, String> {
    match std::env::var("LAST_WORLD_BENCHMARK_ATTEMPT") {
        Err(std::env::VarError::NotPresent) => Ok(1),
        Ok(value) if value == "1" => Ok(1),
        Ok(value) if value == "2" => Ok(2),
        _ => Err("LAST_WORLD_BENCHMARK_ATTEMPT must be 1 or 2 when supplied".into()),
    }
}
fn validate_checkpoint_identity(
    saved: &BenchmarkCheckpoint,
    c: &BenchmarkCase,
    identity: &Value,
) -> Result<(), String> {
    if saved.version != 1
        || saved.checkpoint_group != c.checkpoint_group
        || saved.seed != c.seed
        || saved.military != c.military
        || saved.workers != c.workers
        || &saved.source_identity != identity
    {
        return Err(
            "Owned checkpoint identity/build/workload differs; choose a new run output root".into(),
        );
    }
    if saved.campaign.version != 1 {
        return Err("Unsupported benchmark campaign version".into());
    }
    Ok(())
}
/// Explicit attempt parameter keeps retry tests independent of process-global
/// environment mutation. Attempt2 must rerun the phase and native rendering.
pub fn prepare_for_attempt(
    id: &str,
    output: &Path,
    attempt: u8,
) -> Result<(Campaign, BenchmarkSession), String> {
    prepare_with_identity(id, output, attempt, source_identity())
}
fn prepare_with_identity(
    id: &str,
    output: &Path,
    attempt: u8,
    identity: Value,
) -> Result<(Campaign, BenchmarkSession), String> {
    if !matches!(attempt, 1 | 2) {
        return Err("Benchmark attempt must be 1 or 2".into());
    }
    let prepared = Instant::now();
    let c = case(id).ok_or_else(|| format!("Unknown benchmark case {id}"))?;
    if !(1..=100).contains(&c.requested_rate)
        || c.military < 2
        || c.military > 200000
        || c.workers < 2
        || c.workers > 40000
        || c.materialized_wildlife_target > 5000
        || c.checkpoint_group
            .chars()
            .any(|x| !x.is_ascii_alphanumeric() && x != '-' && x != '_')
    {
        return Err("Invalid explicit benchmark plan limits".into());
    }
    if c.battle_scenario
        && (c.mode != BenchmarkMode::Window
            || c.requested_rate != 4
            || c.target_ticks != Some(crate::battle_ai::BATTLE_TICKS)
            || c.speed_ramp.is_some())
    {
        return Err("Invalid explicit20minute4x battle scenario".into());
    }
    if let Some(ramp) = &c.speed_ramp {
        if !ramp.valid()
            || c.mode != BenchmarkMode::Window
            || c.requested_rate != 1
            || c.target_ticks.is_some()
        {
            return Err("Invalid single native adaptive speed-ramp plan".into());
        }
    }
    std::fs::create_dir_all(output.join("checkpoints")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(output.join("captures")).map_err(|e| e.to_string())?;
    let checkpoint_path = output
        .join("checkpoints")
        .join(format!("{}.save", c.checkpoint_group));
    let lock = GroupLock::acquire(&checkpoint_path.with_extension("run-lock"))?;
    let ledger = rejection_ledger(output)?;
    let mut selected = if checkpoint_path.exists() {
        let saved: BenchmarkCheckpoint = campaign_support::load(&checkpoint_path)?;
        validate_checkpoint_identity(&saved, &c, &identity)?;
        Some(saved)
    } else {
        None
    };
    let mut origin = if attempt == 2 {
        "retry_continued_prephase_group"
    } else {
        "continued_last_good_group"
    };
    let mut rewind = None;
    let mut discard_failed_first_phase = false;
    let rejection = selected
        .as_ref()
        .and_then(|saved| rejected(saved, &ledger))
        .map(|r| json!({"case_id":r.case_id,"attempt_index":r.attempt_index,"reason":r.reason}));
    let rejected_checkpoint = rejection.clone();
    let force_retry = attempt == 2
        && selected
            .as_ref()
            .is_some_and(|saved| saved.last_case == c.id);
    if rejection.is_some() || force_retry {
        let failed_case = selected.as_ref().unwrap().last_case.clone();
        let previous_path = campaign_support::previous_path(&checkpoint_path);
        if previous_path.exists() {
            let previous: BenchmarkCheckpoint = campaign_support::load(&previous_path)?;
            validate_checkpoint_identity(&previous, &c, &identity)?;
            if previous.last_case == failed_case
                || previous.controller.completed_cases.contains(&failed_case)
                || rejected(&previous, &ledger).is_some()
            {
                return Err(
                    "Retry previous checkpoint is not an accepted prephase snapshot".into(),
                );
            }
            selected = Some(previous);
            rewind = Some(previous_path);
            origin = if rejection.is_some() && failed_case != c.id {
                "continued_after_rejected_attempt_rollback"
            } else {
                "retry_previous_phase_snapshot"
            };
        } else {
            let current = selected.as_ref().unwrap();
            if current
                .controller
                .completed_cases
                .iter()
                .any(|done| done != &failed_case)
            {
                return Err("Rejected continued phase has no prephase checkpoint; existing world will not be regenerated".into());
            }
            selected = None;
            discard_failed_first_phase = true;
            origin = if rejection.is_some() && failed_case != c.id {
                "fresh_after_rejected_first_phase"
            } else {
                "retry_fresh_named_initial_world"
            };
        }
    }
    let (game, controller, recovered) = if let Some(saved) = selected {
        if saved.controller.completed_cases.contains(&c.id) && saved.last_case != c.id {
            return Err("This case completed before a later group phase; the supervisor must skip completed IDs or use a new run root".into());
        }
        let recovered = (attempt == 1
            && saved.last_case == c.id
            && saved.last_report["passed"].as_bool() == Some(true))
        .then_some(saved.last_report);
        if recovered.is_some() {
            origin = "recovered_last_completed_receipt";
        }
        let game = Campaign {
            terrain: Terrain::restore(&saved.campaign.terrain)?,
            data: saved.campaign.data,
        };
        game.validate()?;
        (game, saved.controller, recovered)
    } else {
        if !discard_failed_first_phase {
            origin = if attempt == 2 {
                "retry_fresh_named_initial_world"
            } else {
                "fresh_named_initial_world"
            };
        }
        let mut game = if c.battle_scenario {
            Campaign::new_battle_benchmark(c.seed, c.military, c.workers)?
        } else {
            Campaign::new_for_benchmark(c.seed, c.military, c.workers)?
        };
        let tracked = materialize_existing(&mut game, c.materialized_wildlife_target)?;
        let controller = ControllerState {
            orders: OrderExercise::new(),
            battle: BattleDirector::default(),
            conservation: ConservationDriver::default(),
            initial_materialized: tracked.len() as u32,
            tracked_wildlife: tracked,
            completed_cases: Vec::new(),
        };
        (game, controller, None)
    };
    if c.fault_probe == Some(FaultProbe::ContinueAfterSkippedProbe)
        && controller
            .completed_cases
            .iter()
            .any(|id| id == FAILED_PROBE_CASE)
    {
        return Err(
            "Recovery control found a skipped failed phase in the continued controller checkpoint"
                .into(),
        );
    }
    // Roll back the failed committed image while holding the group lock. This
    // preserves the genuine prephase .previous for the next successful commit,
    // rather than backing up another failed same-case image over it.
    let rewound = rewind.is_some();
    if let Some(previous_path) = rewind {
        let pending =
            checkpoint_path.with_extension(format!("retry-rewind-{}", std::process::id()));
        std::fs::copy(previous_path, &pending).map_err(|e| e.to_string())?;
        std::fs::rename(pending, &checkpoint_path).map_err(|e| e.to_string())?;
    } else if discard_failed_first_phase {
        std::fs::remove_file(&checkpoint_path).map_err(|e| e.to_string())?;
    }
    if rewound || discard_failed_first_phase {
        logging::log("BENCHMARK_RETRY_RESTORE",&json!({"case_id":c.id,"attempt":attempt,"replay_origin":origin,"rejected_checkpoint":rejected_checkpoint,"restored_tick":game.processed_tick,"restored_pending_ticks":game.clock.pending_ticks()}).to_string());
    }
    let (pid, started_utc_ms, session_id, report_path, progress_path, capture_path) =
        artifact_paths(&c, output);
    let initial = game.report();
    let initial_physical = game.physical.report();
    let initial_habitat = habitat_snapshot(&game);
    let initial_eco_commits = game.ecology.physical_effects_committed;
    let rss = rss_bytes();
    let speed_ramp = c.speed_ramp.clone().map(SpeedRamp::new);
    let session = BenchmarkSession {
        case: c,
        process_id: pid,
        session_id,
        started_utc_ms,
        attempt,
        replay_origin: origin.into(),
        rejected_checkpoint,
        report_path,
        progress_path,
        capture_path,
        checkpoint_path,
        _group_lock: lock,
        controller,
        source_identity: identity,
        start_tick: game.processed_tick,
        start_pending: game.clock.pending_ticks(),
        initial,
        initial_physical,
        initial_habitat,
        initial_eco_commits,
        started: None,
        live_loop_started_utc_ms: None,
        measured_wall: None,
        preparation_seconds: prepared.elapsed().as_secs_f64(),
        elapsed_charged: Duration::ZERO,
        precharged_tail: Duration::ZERO,
        speed_ramp,
        advance_calls: 0,
        max_pending: game.clock.pending_ticks(),
        tick_samples: Samples::default(),
        batch_samples: Samples::default(),
        frame_samples: Samples::default(),
        controller_samples: Samples::default(),
        validation_samples: Samples::default(),
        progress_io_samples: Samples::default(),
        last_pin_tick: None,
        last_progress: Duration::ZERO,
        last_validation_tick: game.processed_tick,
        validation_count: 1,
        failure: None,
        done: recovered.is_some(),
        stop_reason: recovered
            .as_ref()
            .map(|_| "recovered_previous_success".into()),
        rss_max: rss,
        recovered_success: recovered,
        finished: None,
    };
    Ok((game, session))
}

/// One advance may end at a decision boundary, never jump across it. Elapsed
/// input is still charged in full by GameClock and remaining debt is retained.
fn battle_tick_budget(director: &BattleDirector, tick: u64, target: u64) -> Result<u32, String> {
    let until_decision = director
        .next_decision_tick
        .checked_sub(tick)
        .filter(|remaining| (1..=20).contains(remaining))
        .ok_or("Battle commander did not schedule its next clock boundary")?;
    let until_target = target
        .checked_sub(tick)
        .filter(|remaining| *remaining > 0)
        .ok_or("Battle advance attempted beyond its exact target")?;
    Ok(until_decision.min(until_target) as u32)
}

impl BenchmarkSession {
    pub fn live_loop_started_utc_ms(&self) -> Option<u64> {
        self.live_loop_started_utc_ms
    }
    pub fn current_requested_rate(&self) -> u32 {
        self.speed_ramp
            .as_ref()
            .map_or(self.case.requested_rate, SpeedRamp::rate)
    }
    pub fn tick_budget_per_call(&self) -> u32 {
        if self.case.battle_scenario {
            20
        } else {
            BENCHMARK_STEPS_PER_CALL
        }
    }
    pub fn speed_ramp_progress(&self) -> Value {
        self.speed_ramp
            .as_ref()
            .map_or(Value::Null, SpeedRamp::progress_report)
    }
    /// Presentation is observed from the existing renderer, not inferred from
    /// a simulation call. Loading never qualifies a measured stage.
    pub fn note_native_presentation(
        &mut self,
        game: &mut Campaign,
        presented: bool,
    ) -> Result<(), String> {
        if self
            .speed_ramp
            .as_ref()
            .is_some_and(|r| r.phase == RampPhase::Loading)
            && presented
        {
            let wall = self.wall();
            self.charge_processing_tail(game, wall)?;
            let mark = RampMark {
                wall,
                tick: game.processed_tick,
                pending: game.clock.pending_ticks(),
            };
            if let Some(ramp) = &mut self.speed_ramp {
                ramp.begin_live_warmup(mark);
            }
        } else if let Some(ramp) = &mut self.speed_ramp {
            ramp.record_presentation(presented);
        }
        Ok(())
    }
    pub fn fail(&mut self, reason: impl Into<String>) {
        if self.measured_wall.is_none() {
            self.measured_wall = Some(self.wall());
        }
        if self.failure.is_none() {
            self.failure = Some(reason.into());
        }
        self.done = true;
        self.stop_reason = Some("failed_or_aborted".into());
    }
    pub fn abort(&mut self, reason: impl Into<String>) {
        self.fail(reason);
    }
    pub fn record_frame(&mut self, elapsed: Duration) {
        self.frame_samples
            .push(elapsed.as_nanos().min(u64::MAX as u128) as u64);
        if let Some(ramp) = &mut self.speed_ramp {
            ramp.record_frame(elapsed);
        }
    }
    fn wall(&self) -> Duration {
        self.measured_wall
            .unwrap_or_else(|| self.started.map_or(Duration::ZERO, |s| s.elapsed()))
    }
    pub(crate) fn progress(&self, game: &Campaign, steps: u32) -> BenchmarkProgress {
        let wall = self.wall().as_secs_f64();
        let ticks = game.processed_tick.saturating_sub(self.start_tick);
        let current_achieved = self.speed_ramp.as_ref().and_then(|ramp| {
            if ramp.phase == RampPhase::Measuring {
                let measured = self
                    .wall()
                    .saturating_sub(ramp.measurement.wall)
                    .as_secs_f64();
                (measured > 0.).then(|| {
                    game.processed_tick.saturating_sub(ramp.measurement.tick) as f64
                        / 20.
                        / measured
                })
            } else if ramp.phase == RampPhase::Complete {
                ramp.stages
                    .last()
                    .and_then(|s| s["achieved_simulation_rate"].as_f64())
            } else {
                None
            }
        });
        BenchmarkProgress {
            done: self.done,
            elapsed_wall_seconds: wall,
            simulated_ticks: ticks,
            achieved_simulation_rate: current_achieved.unwrap_or_else(|| {
                if wall > 0. {
                    ticks as f64 / 20. / wall
                } else {
                    0.
                }
            }),
            pending_ticks: game.clock.pending_ticks(),
            stop_reason: self.stop_reason.clone(),
            last_advanced_ticks: steps,
        }
    }
    pub fn step(
        &mut self,
        game: &mut Campaign,
        elapsed: Duration,
    ) -> Result<BenchmarkProgress, String> {
        if self.done {
            return Ok(self.progress(game, 0));
        }
        let elapsed = if self.started.is_none() {
            self.started = Some(Instant::now());
            // UTC is correlation metadata only. Instant continues to own all
            // durations and no system-clock change can alter the simulation.
            self.live_loop_started_utc_ms = Some(now_ms());
            Duration::ZERO
        } else {
            consume_precharged_elapsed(elapsed, &mut self.precharged_tail)
        };
        if game.paused || game.winner.is_some() {
            self.fail(format!(
                "Campaign paused or naturally ended: {}",
                game.outcome
            ));
            return Ok(self.progress(game, 0));
        }
        let controller_started = Instant::now();
        let tick = game.processed_tick;
        if self.last_pin_tick != Some(tick) {
            for id in &self.controller.tracked_wildlife {
                if let Some(detail) = game.detailed.get_mut(id) {
                    detail.last_seen = tick;
                }
            }
            self.last_pin_tick = Some(tick);
        }
        let controller_result = if self.case.battle_scenario {
            self.controller.battle.drive(game)
        } else {
            self.controller
                .orders
                .drive(game)
                .and_then(|()| self.controller.conservation.drive(game))
        };
        self.controller_samples.push(
            controller_started
                .elapsed()
                .as_nanos()
                .min(u64::MAX as u128) as u64,
        );
        if let Err(error) = controller_result {
            self.fail(error.clone());
            return Err(error);
        }
        let before = Instant::now();
        let tick_budget = if self.case.battle_scenario {
            battle_tick_budget(
                &self.controller.battle,
                game.processed_tick,
                self.start_tick.saturating_add(
                    self.case
                        .target_ticks
                        .unwrap_or(crate::battle_ai::BATTLE_TICKS),
                ),
            )?
        } else {
            self.tick_budget_per_call()
        };
        let (steps, times) = match game.advance_benchmark_profiled_budgeted(
            elapsed,
            self.current_requested_rate(),
            tick_budget,
        ) {
            Ok(value) => value,
            Err(e) => {
                self.fail(e.clone());
                return Err(e);
            }
        };
        if steps > tick_budget {
            self.fail("Profiled advance exceeded its declared tick budget");
            return Err("Invalid benchmark tick budget".into());
        }
        self.advance_calls += 1;
        self.elapsed_charged = self.elapsed_charged.saturating_add(elapsed);
        if steps > 0 {
            self.batch_samples
                .push(before.elapsed().as_nanos().min(u64::MAX as u128) as u64);
            for ns in &times[..steps as usize] {
                self.tick_samples.push(*ns);
            }
            if let Some(ramp) = &mut self.speed_ramp {
                ramp.record_ticks(&times[..steps as usize]);
            }
        }
        self.max_pending = self.max_pending.max(game.clock.pending_ticks());
        if game.paused || game.winner.is_some() {
            self.fail(format!(
                "Campaign paused or naturally ended during measurement: {}",
                game.outcome
            ));
            return Ok(self.progress(game, steps));
        }
        if game.processed_tick >= self.last_validation_tick.saturating_add(200) {
            let validation_started = Instant::now();
            let validation = game.validate();
            self.validation_samples.push(
                validation_started
                    .elapsed()
                    .as_nanos()
                    .min(u64::MAX as u128) as u64,
            );
            if let Err(e) = validation {
                self.fail(e.clone());
                return Err(e);
            }
            self.last_validation_tick = game.processed_tick;
            self.validation_count += 1;
        }
        let wall = self.wall();
        if wall.saturating_sub(self.last_progress) >= Duration::from_secs(2) {
            let progress_io_started = Instant::now();
            self.last_progress = wall;
            if let Some(rss) = rss_bytes() {
                self.rss_max = Some(self.rss_max.unwrap_or(0).max(rss));
                if rss > MEMORY_GUARD_BYTES {
                    self.fail("Native RSS exceeded the explicit6GiB benchmark guard");
                }
            }
            let progress = self.progress(game, steps);
            let progress_write = atomic_json(
                &self.progress_path,
                &json!({"schema_version":1,"case_id":self.case.id,"process_id":self.process_id,"session_id":self.session_id,"started_utc_ms":self.started_utc_ms,"live_loop_started_utc_ms":self.live_loop_started_utc_ms,"attempt":self.attempt,"replay_origin":self.replay_origin,"status":if self.done{"stopped"}else{"running"},"requested_rate":self.current_requested_rate(),"progress":progress,"speed_ramp":self.speed_ramp_progress(),"actual_counts":actual_counts(game,&self.case),"memory_rss_bytes":self.rss_max}),
            );
            self.progress_io_samples.push(
                progress_io_started
                    .elapsed()
                    .as_nanos()
                    .min(u64::MAX as u128) as u64,
            );
            progress_write?;
        }
        let wall = self.wall();
        if self
            .speed_ramp
            .as_ref()
            .is_some_and(|r| r.boundary_due(wall))
        {
            // Settle every last old-rate nanosecond before changing rate.
            // The next caller's delta includes this processing tail, so retain
            // a credit and subtract it exactly once from that input.
            self.charge_processing_tail(game, wall)?;
            let mark = RampMark {
                wall,
                tick: game.processed_tick,
                pending: game.clock.pending_ticks(),
            };
            if let Some(ramp) = &mut self.speed_ramp {
                ramp.observe_boundary(mark);
            }
        }
        let completed = if let Some(ramp) = &self.speed_ramp {
            ramp.phase == RampPhase::Complete
        } else if let Some(target) = self.case.target_ticks {
            game.processed_tick.saturating_sub(self.start_tick) >= target
        } else {
            wall >= Duration::from_secs(self.case.wall_seconds)
        };
        if completed && !self.done {
            // The final processing tail has not yet reached the next caller's
            // elapsed delta. Charge it once as debt, execute zero extra ticks,
            // then freeze phase timing before capture/save work.
            self.charge_processing_tail(game, wall)?;
            self.measured_wall = Some(wall);
            self.done = true;
            self.stop_reason = Some("measurement_complete".into());
        }
        if wall >= Duration::from_secs(self.case.timeout_seconds) && !self.done {
            self.fail("Native case wall timeout before its required phase completed");
        }
        Ok(self.progress(game, steps))
    }
    fn charge_processing_tail(
        &mut self,
        game: &mut Campaign,
        wall: Duration,
    ) -> Result<(), String> {
        let tail = wall.saturating_sub(self.elapsed_charged);
        game.clock
            .advance_benchmark(tail, false, 0, self.current_requested_rate())
            .map_err(|e| e.to_string())?;
        self.elapsed_charged = self.elapsed_charged.saturating_add(tail);
        self.precharged_tail = self.precharged_tail.saturating_add(tail);
        self.max_pending = self.max_pending.max(game.clock.pending_ticks());
        Ok(())
    }
    pub fn recovered_completed_case(&self) -> bool {
        self.recovered_success.is_some()
    }
    /// Window callers invoke this only after the real renderer, captures and
    /// final annotations validate. A pending core report is never a good save.
    pub fn commit_success(&mut self, game: &Campaign, report: &mut Value) -> Result<(), String> {
        if self.failure.is_some()
            || report["passed"].as_bool() != Some(true)
            || report["core_phase_passed"].as_bool() != Some(true)
        {
            return Err("Only a fully validated successful phase may commit its checkpoint".into());
        }
        let snapshot_started = Instant::now();
        let mut controller = self.controller.clone();
        if !controller.completed_cases.contains(&self.case.id) {
            controller.completed_cases.push(self.case.id.clone());
        }
        report["checkpoint"]["committed"] = json!(true);
        report["checkpoint"]["awaiting_window_validation"] = json!(false);
        let checkpoint = BenchmarkCheckpoint {
            version: 1,
            attempt: self.attempt,
            checkpoint_group: self.case.checkpoint_group.clone(),
            seed: self.case.seed,
            military: self.case.military,
            workers: self.case.workers,
            source_identity: self.source_identity.clone(),
            campaign: CampaignSave {
                version: 1,
                terrain: game.terrain.snapshot(),
                data: game.data.clone(),
            },
            controller: controller.clone(),
            last_case: self.case.id.clone(),
            last_report: report.clone(),
        };
        match campaign_support::save_atomic(&self.checkpoint_path, &checkpoint) {
            Ok(saved) => {
                self.controller = controller;
                report["checkpoint"]["bytes"] = json!(saved.bytes);
                report["checkpoint"]["write_seconds"] =
                    json!(snapshot_started.elapsed().as_secs_f64());
            }
            Err(error) => {
                self.fail(error.clone());
                report["status"] = json!("failed");
                report["passed"] = json!(false);
                report["failure"] = json!(format!("Phase checkpoint commit failed: {error}"));
                report["checkpoint"]["committed"] = json!(false);
                let _ = atomic_json(&self.report_path, report);
                self.finished = Some(report.clone());
                return Err(error);
            }
        }
        atomic_json(&self.report_path, report)?;
        self.finished = Some(report.clone());
        Ok(())
    }
    pub fn finish(&mut self, game: &mut Campaign) -> Result<Value, String> {
        if let Some(report) = &self.finished {
            return Ok(report.clone());
        }
        if let Some(mut previous) = self.recovered_success.take() {
            let original = previous["session_id"].clone();
            previous["process_id"] = json!(self.process_id);
            previous["session_id"] = json!(self.session_id);
            previous["started_utc_ms"] = json!(self.started_utc_ms);
            previous["attempt"] = json!(self.attempt);
            previous["replay_origin"] = json!(self.replay_origin);
            previous["recovered_previous_success"] = json!({"original_session_id":original,"replayed_simulation":false,"checkpoint":self.checkpoint_path});
            atomic_json(&self.report_path, &previous)?;
            self.finished = Some(previous.clone());
            return Ok(previous);
        }
        if !self.done {
            self.fail("Session ended before its measurement requirement");
        }
        let final_validation_started = Instant::now();
        if let Err(e) = game.validate() {
            self.fail(e);
        }
        let final_validation_seconds = final_validation_started.elapsed().as_secs_f64();
        if actual_counts(game, &self.case)["population_counts_were_not_reduced"].as_bool()
            != Some(true)
        {
            self.fail("Requested initial tactical identities are missing from the actual role/death ledger");
        }
        if self.case.battle_scenario {
            if let Some(error) = self.controller.battle.qualification_error(game) {
                self.fail(error);
            }
        }
        self.validation_count += 1;
        let wall = self.wall().as_secs_f64();
        let ticks = game.processed_tick.saturating_sub(self.start_tick);
        let achieved = if wall > 0. {
            ticks as f64 / 20. / wall
        } else {
            0.
        };
        let debt = game.clock.pending_ticks();
        let debt_growth = debt.saturating_sub(self.start_pending);
        let requested = self.current_requested_rate();
        let rate_sustained = self.speed_ramp.as_ref().map_or_else(
            || {
                achieved >= requested as f64 * 0.95
                    && debt_growth <= self.tick_budget_per_call() as u64
            },
            |ramp| {
                ramp.stages
                    .last()
                    .is_some_and(|stage| stage["requested_rate_sustained"].as_bool() == Some(true))
            },
        );
        let passed =
            self.failure.is_none() && self.stop_reason.as_deref() == Some("measurement_complete");
        let coverage = coverage(self, game);
        let metrics = report_object([
            ("preparation_seconds", json!(self.preparation_seconds)),
            (
                "live_loop_started_utc_ms",
                json!(self.live_loop_started_utc_ms),
            ),
            (
                "utc_correlation_scope",
                json!(
                    "Annotation for joining host telemetry to monotonic wall offsets; UTC is not a simulation clock and may move if the OS clock is adjusted"
                ),
            ),
            ("wall_seconds", json!(wall)),
            (
                "elapsed_charged_seconds",
                json!(self.elapsed_charged.as_secs_f64()),
            ),
            ("start_tick", json!(self.start_tick)),
            ("end_tick", json!(game.processed_tick)),
            ("committed_ticks", json!(ticks)),
            ("achieved_simulation_rate", json!(achieved)),
            (
                "requested_simulation_rate",
                if self.speed_ramp.is_some() {
                    Value::Null
                } else {
                    json!(requested)
                },
            ),
            (
                "rate_scope",
                json!(if self.speed_ramp.is_some() {
                    "Whole-session aggregate mixes first native presentation loading, live warm-up, settling and requested rates; use speed_ramp.completed_stages for rate comparisons"
                } else {
                    "single requested rate"
                }),
            ),
            ("pending_ticks_start", json!(self.start_pending)),
            ("pending_ticks_end", json!(debt)),
            ("pending_ticks_max", json!(self.max_pending)),
            ("advance_calls", json!(self.advance_calls)),
            (
                "advance_calls_per_second",
                json!(if wall > 0. {
                    self.advance_calls as f64 / wall
                } else {
                    0.
                }),
            ),
            ("tick_budget_per_call", json!(self.tick_budget_per_call())),
            (
                "scheduler_ceiling_simulation_rate",
                json!(if wall > 0. {
                    self.tick_budget_per_call() as f64 * self.advance_calls as f64 / 20. / wall
                } else {
                    0.
                }),
            ),
            ("actual_fixed_20hz_tick", json!(self.tick_samples.report())),
            ("advance_batches", json!(self.batch_samples.report())),
            (
                "complete_frame_intervals",
                json!(self.frame_samples.report()),
            ),
            ("rss_bytes_max", json!(self.rss_max)),
            ("memory_guard_bytes", json!(MEMORY_GUARD_BYTES)),
            (
                "memory_guard_scope",
                json!(
                    "Linux native RSS if available; Windows external supervisor also guards the process"
                ),
            ),
            (
                "controller_and_tracking_overhead",
                self.controller_samples.report(),
            ),
            (
                "periodic_validation_overhead",
                self.validation_samples.report(),
            ),
            (
                "progress_io_and_memory_sampling_overhead",
                self.progress_io_samples.report(),
            ),
            (
                "final_validation_seconds_outside_phase",
                json!(final_validation_seconds),
            ),
            (
                "harness_overhead_scope",
                json!(
                    "Controller/tracking, periodic validation and progress I/O are inside measured phase wall but outside actual fixed-rule tick timers"
                ),
            ),
            ("validation_calls", json!(self.validation_count)),
            ("save_and_setup_excluded_from_phase_sampling", json!(true)),
        ]);
        let checkpoint = report_object([
            ("path", json!(self.checkpoint_path)),
            ("committed", json!(false)),
            ("clock_debt_preserved", json!(true)),
            ("phase_controller_and_world_one_envelope", json!(true)),
            (
                "awaiting_window_validation",
                json!(passed && self.case.mode == BenchmarkMode::Window),
            ),
        ]);
        let scope_limits = report_object([
            ("map_meters", json!([4800, 3200])),
            ("full_20km_campaign_qualified", json!(false)),
            ("million_wildlife_implemented", json!(false)),
            ("all160traits_implemented", json!(false)),
            ("mutation_implemented", json!(false)),
            (
                "speed_target_met_is_separate_from_valid_phase_completion",
                json!(true),
            ),
            (
                "sequential_evolving_world_ramp_not_same_state_apples_to_apples",
                json!(true),
            ),
        ]);
        let mut report = report_object([
            ("schema_version", json!(1)),
            ("kind", json!("native_coupled_campaign_stress_measurement")),
            ("case_id", json!(self.case.id)),
            ("mode", json!(self.case.mode)),
            ("process_id", json!(self.process_id)),
            ("session_id", json!(self.session_id)),
            ("started_utc_ms", json!(self.started_utc_ms)),
            ("attempt", json!(self.attempt)),
            ("replay_origin", json!(self.replay_origin)),
            ("rejected_checkpoint", json!(self.rejected_checkpoint)),
            (
                "harness_control_probe",
                json!(self.case.harness_control_probe),
            ),
            (
                "status",
                json!(if passed {
                    if self.case.mode == BenchmarkMode::Window {
                        "pending_window_validation"
                    } else {
                        "passed"
                    }
                } else {
                    "failed"
                }),
            ),
            (
                "passed",
                json!(passed && self.case.mode == BenchmarkMode::Cpu),
            ),
            ("core_phase_passed", json!(passed)),
            ("requested_rate", json!(requested)),
            ("requested_rate_sustained", json!(rate_sustained)),
            ("speed_ramp", self.speed_ramp_progress()),
            (
                "battle_scenario",
                if self.case.battle_scenario {
                    self.controller.battle.report(game)
                } else {
                    Value::Null
                },
            ),
            ("case", json!(self.case)),
            ("source_identity", json!(self.source_identity)),
            ("actual_counts", json!(actual_counts(game, &self.case))),
            ("failure", json!(self.failure)),
            ("stop_reason", json!(self.stop_reason)),
            (
                "controller_scope",
                json!(
                    "explicit qualification harness; ordinary paid commands and currently observed capture/release; no player UI knowledge bypass"
                ),
            ),
            ("metrics", json!(metrics)),
            ("coverage", json!(coverage)),
            ("initial_state", json!(self.initial)),
            ("final_state", json!(game.report())),
            ("physical_initial", json!(self.initial_physical)),
            ("physical_final", json!(game.physical.report())),
            ("habitats_initial", json!(self.initial_habitat)),
            ("habitats_final", json!(habitat_snapshot(game))),
            (
                "conservation_controller",
                json!(self.controller.conservation.report(game)),
            ),
            ("order_exercise", json!(self.controller.orders.report(game))),
            ("checkpoint", json!(checkpoint)),
            ("scope_limits", json!(scope_limits)),
        ]);
        if passed && self.case.mode == BenchmarkMode::Cpu {
            let _ = self.commit_success(game, &mut report);
        }
        atomic_json(&self.report_path, &report)?;
        logging::log("BENCHMARK_CASE_RESULT",&json!({"session_id":self.session_id,"case_id":self.case.id,"passed":report["passed"],"requested_rate":requested,"achieved_rate":achieved,"speed_ramp":self.speed_ramp_progress(),"debt":debt,"report":self.report_path}).to_string());
        self.finished = Some(report.clone());
        Ok(report)
    }
}

pub fn failed_prepare_report(id: &str, output: &Path, error: &str) -> Result<Value, String> {
    let c = case(id).ok_or("Unknown benchmark case")?;
    let (pid, time, session, path, progress, _) = artifact_paths(&c, output);
    let report = json!({"schema_version":1,"kind":"native_coupled_campaign_stress_measurement","case_id":id,"process_id":pid,"session_id":session,"started_utc_ms":time,"attempt":runtime_attempt().ok(),"replay_origin":"preparation_failed","status":"failed","passed":false,"requested_rate":c.requested_rate,"case":c,"actual_counts":null,"coverage":{"status":"not_reached","reason":"world or checkpoint preparation failed"},"metrics":{},"source_identity":source_identity(),"failure":error});
    atomic_json(&path, &report)?;
    atomic_json(&progress, &report)?;
    Ok(report)
}
pub fn run_cpu_case(id: &str, output: &Path) -> Result<Value, String> {
    let (mut game, mut session) = match prepare(id, output) {
        Ok(value) => value,
        Err(error) => return failed_prepare_report(id, output, &error),
    };
    if session.case.mode != BenchmarkMode::Cpu {
        session.fail("Window case requires the native renderer");
        return session.finish(&mut game);
    }
    let mut previous = Instant::now();
    loop {
        let now = Instant::now();
        let elapsed = now.duration_since(previous);
        previous = now;
        match session.step(&mut game, elapsed) {
            Ok(progress) => {
                if progress.done {
                    break;
                }
                if progress.last_advanced_ticks == 0 {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            Err(error) => {
                session.fail(error);
                break;
            }
        }
    }
    let report = session.finish(&mut game)?;
    if report["passed"].as_bool() == Some(true)
        && report["checkpoint"]["committed"].as_bool() == Some(true)
    {
        let abort = match session.case.fault_probe {
            Some(FaultProbe::AfterCommitCrashOnce) => session.attempt == 1,
            Some(FaultProbe::AfterCommitCrashAlways) => true,
            _ => false,
        };
        if abort {
            logging::log("BENCHMARK_INTENTIONAL_RECOVERY_PROBE",&json!({"case_id":session.case.id,"attempt":session.attempt,"after_valid_checkpoint_and_report_commit":true,"scope":"supervisor recovery control, not gameplay/performance qualification"}).to_string());
            std::process::abort();
        }
    }
    Ok(report)
}

impl ConservationDriver {
    fn note(&mut self, tick: u64, stage: &str, details: Value) {
        let event = json!({"tick":tick,"stage":stage,"details":details});
        logging::log("BENCHMARK_GAMEPLAY", &event.to_string());
        if self.milestones.len() < 128 {
            self.milestones.push(event);
        }
    }
    fn command(
        &mut self,
        game: &mut Campaign,
        tool: usize,
        species: u32,
        point: [f32; 2],
        name: &str,
    ) -> bool {
        let previous_tool = game.selected_tool;
        let previous_species = game.selected_species;
        game.selected_tool = tool;
        game.selected_species = Some(species);
        let result = game.click(point, false);
        game.selected_tool = previous_tool;
        game.selected_species = previous_species;
        match result {
            Ok(()) => {
                self.note(
                    game.processed_tick,
                    name,
                    json!({"species":species,"point":point}),
                );
                true
            }
            Err(error) => {
                if self.errors.len() == 10 {
                    self.errors.remove(0);
                }
                self.errors
                    .push(json!({"tick":game.processed_tick,"action":name,"rejected":error}));
                false
            }
        }
    }
    fn drive(&mut self, game: &mut Campaign) -> Result<(), String> {
        let tick = game.processed_tick;
        if tick < self.next_action_tick || self.released {
            return Ok(());
        }
        self.next_action_tick = tick.saturating_add(20);
        let pen = game
            .construction
            .buildings
            .iter()
            .any(|b| b.team == 0 && b.kind == BuildKind::Pen && b.complete && b.health > 0);
        if self.selected.is_none() && pen {
            if let Some((species, _, cohort, point)) = capture_candidate(game, None) {
                if self.command(game, 19, species, point, "inspect_observed_species") {
                    self.selected = Some(species);
                    self.note(
                        tick,
                        "selected_species",
                        json!({"species":species,"cohort":cohort}),
                    );
                }
            }
        }
        let Some(species) = self.selected else {
            return Ok(());
        };
        let adults = game
            .ecology
            .captives
            .iter()
            .filter(|c| {
                c.team == 0
                    && c.animal.species_id == species
                    && c.animal.stage == Stage::Adult
                    && game.ecology.is_housed(c)
            })
            .map(|c| (c.animal.id, c.animal.sex))
            .collect::<Vec<_>>();
        let pair = adults.len() >= 2
            && adults.iter().any(|(_, s)| *s != Sex::Male)
            && adults.iter().any(|(_, s)| *s != Sex::Female);
        let pending = game
            .capture_orders
            .iter()
            .any(|o| o.team == 0 && o.species == species);
        if !self.pair_delivered && !pair && !pending {
            if let Some((_, _, _, point)) = capture_candidate(game, Some(species)) {
                self.command(game, 20, species, point, "capture_handler_dispatched");
            }
        }
        if pair && !pending && !self.pair_delivered {
            self.adult_ids = adults.iter().map(|(id, _)| *id).collect();
            self.pair_delivered = true;
            self.note(
                tick,
                "compatible_pair_delivered",
                json!({"ids":self.adult_ids,"species":species}),
            );
        }
        let housed =
            game.ecology.captives.iter().any(|c| {
                c.team == 0 && c.animal.species_id == species && game.ecology.is_housed(c)
            });
        let lab =
            game.construction.buildings.iter().any(|b| {
                b.team == 0 && b.kind == BuildKind::Research && b.complete && b.health > 0
            });
        if housed && lab && !game.ecology.knowledge[0][species as usize].enabled {
            self.command(game, 21, species, TEAM_BASES[0], "research_enabled");
        }
        self.research |= game.ecology.knowledge[0][species as usize].study_ticks >= 800;
        if self.research && !self.breeding {
            self.breeding = self.command(
                game,
                22,
                species,
                TEAM_BASES[0],
                "compatible_breeding_enabled",
            );
        }
        if !self.offspring {
            let children = game
                .ecology
                .captives
                .iter()
                .filter(|c| {
                    c.team == 0
                        && c.animal.species_id == species
                        && c.animal
                            .parent_id
                            .is_some_and(|p| self.adult_ids.contains(&p))
                })
                .map(|c| c.animal.id)
                .collect::<Vec<_>>();
            if !children.is_empty() {
                self.offspring = true;
                self.note(tick, "real_captive_offspring", json!({"ids":children}));
            }
        }
        if let Some((id, patch, point)) = self.release_request {
            let pending = game
                .capture_orders
                .iter()
                .any(|o| o.releasing && o.captured == Some(id));
            let captive = game.ecology.captives.iter().any(|c| c.animal.id == id);
            let released = game
                .ecology
                .cohorts
                .iter()
                .find(|c| c.id == id && c.count == 1 && c.patch == patch);
            if !pending
                && !captive
                && released.is_some_and(|c| {
                    let p = game.cohort_position(c);
                    (p[0] - point[0]).hypot(p[1] - point[1]) <= 5.
                })
            {
                self.released = true;
                self.note(
                    tick,
                    "existing_animal_released_at_clicked_point",
                    json!({"id":id,"patch":patch,"point":point}),
                );
            } else if !pending {
                self.release_request = None;
            }
        }
        if self.offspring && !self.released && self.release_request.is_none() {
            if let Some((patch, point)) = visible_release_point(game, species) {
                if self.command(game, 23, species, point, "release_handler_dispatched") {
                    if let Some(order) = game
                        .capture_orders
                        .iter()
                        .rev()
                        .find(|o| o.releasing && o.team == 0 && o.species == species)
                    {
                        if let Some(id) = order.captured {
                            self.release_request = Some((id, patch, point));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn report(&self, game: &Campaign) -> Value {
        json!({"selected_species":self.selected,"actual_pair_delivered":self.pair_delivered,"study800ticks":self.research,"breeding_enabled":self.breeding,"actual_offspring":self.offspring,"actual_existing_animal_released":self.released,"water_given_to_care":game.water_given_to_care,"pending_release":self.release_request,"pending_handlers":game.capture_orders,"milestones":self.milestones,"recent_command_rejections":self.errors,"next_action_tick":self.next_action_tick,"no_species_or_stock_rescue":true})
    }
}
fn actual_counts(game: &Campaign, c: &BenchmarkCase) -> Value {
    let mut roles = BTreeMap::<String, (u64, u64)>::new();
    for a in &game.tactics.agents {
        let entry = roles.entry(format!("{:?}", a.role)).or_default();
        entry.0 += 1;
        entry.1 += u64::from(a.alive());
    }
    let role_ledger = roles
        .iter()
        .map(|(role, (total, alive))| {
            (
                role.clone(),
                json!({"spawned_records":total,"alive":alive,"dead":total-alive}),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let eco = game.ecology.census();
    json!({"role_ledger":role_ledger,"requested_initial_military":c.military,"requested_initial_workers":c.workers,"actual_tactical_records":game.tactics.agents.len(),"roles_total_and_alive":roles,"spawned_by_faction":game.tactics.totals.spawned,"deaths_by_faction":game.tactics.totals.deaths,"wild_animals":eco.wild_animals,"transit_animals":eco.transit_animals,"captive_animals":eco.captive_animals,"fine_wildlife_records":game.detailed.len(),"living_plants":eco.living_plants,"cohort_struct_records":game.ecology.cohorts.len(),"species_profiles":game.ecology.species.len(),"population_counts_were_not_reduced":game.tactics.agents.iter().filter(|a|a.role==Role::Soldier).count()>=c.military as usize&&game.tactics.agents.iter().filter(|a|a.role==Role::Worker).count()>=c.workers as usize})
}
fn habitat_snapshot(game: &Campaign) -> Value {
    let h = &game.habitats;
    let mut degree = BTreeMap::<u64, u32>::new();
    for passage in h.passages.values() {
        *degree.entry(passage.from).or_default() += 1;
        *degree.entry(passage.to).or_default() += 1;
    }
    let bent = h
        .passages
        .values()
        .filter(|p| {
            p.centerline.windows(3).any(|w| {
                let a = [
                    w[1].x as i64 - w[0].x as i64,
                    w[1].y as i64 - w[0].y as i64,
                    w[1].z as i64 - w[0].z as i64,
                ];
                let b = [
                    w[2].x as i64 - w[1].x as i64,
                    w[2].y as i64 - w[1].y as i64,
                    w[2].z as i64 - w[1].z as i64,
                ];
                a[0] * b[1] != a[1] * b[0]
                    || a[0] * b[2] != a[2] * b[0]
                    || a[1] * b[2] != a[2] * b[1]
            })
        })
        .count();
    report_object([
("collapsed_nodes",json!(h.nodes.values().filter(|n|n.collapsed).count())),
("distinct_entrances",json!(h.nodes.values().filter(|n|n.kind==crate::habitat::NodeKind::Entrance).count())),
("branch_nodes_degree_above_two",json!(degree.values().filter(|v|**v>2).count())),
("bent_passages",json!(bent)),
("passages_with_horizontal_displacement",json!(h.passages.values().filter(|p|p.centerline.windows(2).any(|w|w[0].x!=w[1].x||w[0].y!=w[1].y)).count())),
("live_canopy_supports",json!(h.supports.values().filter(|s|s.host_alive).count())),
("dead_canopy_supports",json!(h.supports.values().filter(|s|!s.host_alive).count())),
("canopy_occupant_groups",json!(h.occupants.values().filter(|o|matches!(o.location,crate::habitat::Occupancy::Canopy(_))).count())),
("dens",json!(h.shelters.values().filter(|s|s.kind==crate::habitat::ShelterKind::Den).count())),
("nests",json!(h.shelters.values().filter(|s|s.kind==crate::habitat::ShelterKind::Nest).count())),
("cavities",json!(h.shelters.values().filter(|s|s.kind==crate::habitat::ShelterKind::Cavity).count())),
("underground_nested_nests",json!(h.shelters.values().filter(|s|s.kind==crate::habitat::ShelterKind::Nest&&s.burrow_node.is_some()).count())),
("attached_clutches",json!(h.shelters.values().filter(|s|s.clutch.is_some()).count())),
("mapped_animal_dam_links",json!(game.physical.dam_links.values().map(Vec::len).sum::<usize>())),
("reef_navigation_overlay_cells",json!(game.nav.cells.iter().filter(|c|c.reef_height_mm>0).count())),
("revision",json!(h.revision)),
("nodes",json!(h.nodes.len())),
("passages",json!(h.passages.len())),
("passage_length_mm",json!(h.passages.values().map(|p|p.length_mm as u128).sum::<u128>())),
("excavated_passage_mm3",json!(h.passages.values().map(|p|p.excavated_mm3 as u128).sum::<u128>())),
("blocked_passages",json!(h.passages.values().filter(|p|p.blocked).count())),
("flooded_nodes",json!(h.nodes.values().filter(|n|n.flooded_depth_mm>0).count())),
("occupant_groups",json!(h.occupants.len())),
("reserved_real_organisms",json!(h.occupants.values().map(|o|o.group_size as u64).sum::<u64>())),
("shelters",json!(h.shelters.len())),
("shelter_wood",json!(h.shelters.values().map(|s|s.material.wood_units as u128).sum::<u128>())),
("shelter_fiber",json!(h.shelters.values().map(|s|s.material.fiber_units as u128).sum::<u128>())),
("dams",json!(h.dams.values().map(|d|json!({"id":d.id,"integrity":d.integrity,"invested_wood_units":d.invested_wood_units,"required_wood_units":d.required_wood_units,"crest_mm":d.crest_mm,"permeability_permille":d.permeability_permille()})).collect::<Vec<_>>())),
("reefs",json!(h.reefs.values().map(|r|json!({"id":r.id,"solid_height_mm":r.solid_height_mm,"mineral_units":r.mineral_units,"living_builder_units":r.living_builder_units,"clearance_mm":r.clearance_mm()})).collect::<Vec<_>>())),
("wallows",json!(h.wallows.len())),
("wallow_uses",json!(h.wallows.values().map(|w|w.uses as u64).sum::<u64>())),
("wallow_displaced_mm3",json!(h.wallows.values().map(|w|w.displaced_soil_mm3 as u128).sum::<u128>())),
("traps",json!(h.traps.len())),
("trapped_organism_ids",json!(h.traps.values().map(|t|t.trapped.len()).sum::<usize>())),
("trap_excavated_mm3",json!(h.traps.values().map(|t|t.excavated_mm3 as u128).sum::<u128>())),
("debris",json!(h.debris)),
("conserved",json!(h.validate().is_ok())),
])
}
fn state_entry(observed: bool, evidence: Value) -> Value {
    json!({"status":if observed{"observed"}else{"not_reached"},"scope":"actual continued world; phase deltas where available","evidence":evidence})
}
fn coverage(session: &BenchmarkSession, game: &Campaign) -> Value {
    let initial = &session.initial;
    let final_state = game.report();
    let tick_delta = game.processed_tick.saturating_sub(session.start_tick);
    let shots_start = initial["tactics"]["fired_ammunition"].as_u64().unwrap_or(0);
    let jobs_start = initial["completed_jobs"].as_u64().unwrap_or(0);
    let physical = game.physical.report();
    let mut trait_rows = Vec::new();
    for (index, id) in crate::ecology::SELECTED_TRAITS.iter().copied().enumerate() {
        let physical_trait = [11, 12, 46, 64, 65, 67, 101, 102, 103, 104, 105].contains(&id);
        let committed = game.ecology.physical_effects_committed[index];
        let delta = committed.saturating_sub(session.initial_eco_commits[index]);
        trait_rows.push(json!({"trait_id":id,"present_in_species_profiles":game.ecology.species.iter().filter(|s|s.traits.has(id)).map(|s|s.id).collect::<Vec<_>>(),"natural_commits_cumulative":if physical_trait{Some(committed)}else{None},"natural_commits_phase_delta":if physical_trait{Some(delta)}else{None},"counter_scope":"continued-world cumulative; this phase delta is separate","status":if physical_trait{if committed>0{"observed"}else{"not_reached"}}else{"not_instrumented"},"note":if physical_trait{"Actual physical transaction counters; mechanism fixtures remain separate"}else{"Trait presence and aggregate biology events do not establish this trait's consequences; inspect state samples and dedicated mechanism fixtures"}}));
    }
    let all_animals = game
        .ecology
        .cohorts
        .iter()
        .chain(game.ecology.transits.iter().map(|t| &t.organism))
        .chain(game.ecology.captives.iter().map(|c| &c.animal));
    let mut pollen = 0u64;
    let mut seeds_carried = 0u64;
    let mut sperm = 0u64;
    let mut dormant = 0u64;
    for c in all_animals {
        pollen += c.pollen.as_ref().map_or(0, |p| p.units);
        seeds_carried += c.carried_seeds.iter().map(|s| s.units).sum::<u64>();
        sperm += c.sperm.as_ref().map_or(0, |s| s.doses as u64);
        if c.dormant_until > game.processed_tick {
            dormant += c.count as u64;
        }
    }
    let recon=(0..2).map(|team|{let masks=game.recon.masks(team);json!({"team":team,"visual_cells":masks.iter().filter(|m|**m&crate::campaign_support::VISIBLE!=0).count(),"radar_coverage_cells":masks.iter().filter(|m|**m&crate::campaign_support::RADAR_COVERAGE!=0).count(),"radar_sweep_cells":masks.iter().filter(|m|**m&crate::campaign_support::RADAR_SWEEP!=0).count(),"explored_cells":masks.iter().filter(|m|**m&crate::campaign_support::EXPLORED!=0).count()})}).collect::<Vec<_>>();
    json!({
        "military":state_entry(game.tactics.totals.fired_ammunition>0,json!({"fired_ammunition":game.tactics.totals.fired_ammunition,"phase_shots":game.tactics.totals.fired_ammunition.saturating_sub(shots_start),"deaths":game.tactics.totals.deaths,"ship_health":game.ship_health,"route_builds":game.tactics.totals.route_builds,"route_invalidations":game.tactics.totals.route_invalidations,"route_cost_refreshes":game.tactics.totals.route_cost_refreshes,"route_node_expansions":game.tactics.totals.route_node_expansions,"queued_units":game.unit_order_queues.len()})),
        "construction":state_entry(game.completed_jobs>0,json!({"completed_jobs":game.completed_jobs,"phase_completed_jobs":game.completed_jobs.saturating_sub(jobs_start),"unfinished_jobs":game.construction.jobs.iter().filter(|j|!matches!(j.phase,JobPhase::Complete|JobPhase::Cancelled)).count(),"buildings":game.construction.buildings.len()})),
        "logistics_economy":state_entry(game.ore_delivered.iter().any(|v|*v>0),json!({"ore_delivered":game.ore_delivered,"actual_ore_cargo":game.construction.cargo,"finite_deposits":game.construction.deposits,"stores":game.construction.stores,"external_inputs":game.construction.external_inputs})),
        "conservation":state_entry(session.controller.conservation.released,session.controller.conservation.report(game)),
        "water_ground":state_entry(tick_delta>0,json!({"initial":initial["water"],"final":final_state["water"],"scope":"Actual coupled managed river; remaining generated water outside managed region is static"})),
        "physical_ecology":state_entry(game.ecology.physical_effects_committed.iter().any(|v|*v>0),json!({"initial":session.initial_physical,"final":physical,"phase_commit_deltas":game.ecology.physical_effects_committed.iter().zip(session.initial_eco_commits).map(|(end,start)|end.saturating_sub(start)).collect::<Vec<_>>(),"habitats_initial":session.initial_habitat,"habitats_final":habitat_snapshot(game),"mechanism_fixtures_are_separate":true})),
        "recon":state_entry(tick_delta>0,json!({"actual_masks":recon,"satellite_period_ticks":game.recon.satellite.period_ticks,"no_troop_vision_was_enabled":true})),
        "seasons":state_entry(tick_delta>=72000,json!({"start_tick":session.start_tick,"end_tick":game.processed_tick,"phase_committed_ticks":tick_delta,"full_cycles_executed_in_this_phase":tick_delta/72000,"season_boundaries_crossed":game.processed_tick/18000-session.start_tick/18000,"one_full_cycle_requires":72000,"partial_run_is_not_full_season_qualification":true})),
        "saves":state_entry(!session.controller.completed_cases.is_empty(),json!({"restored_completed_cases":session.controller.completed_cases,"clock_pending_ticks":game.clock.pending_ticks(),"checkpoint_group":session.case.checkpoint_group,"interrupted_case_resume":"last successful complete phase, never partial failed state","completed_earlier_case_ids":"Supervisor must skip completed cases; only the most recently committed case is recovered idempotently by native prepare"})),
        "traits":{"status":"partially_instrumented","rows":trait_rows,"actual_state_samples":{"pollen_carried_units":pollen,"seed_carried_units":seeds_carried,"stored_sperm_doses":sperm,"dormant_animals":dormant,"viable_seeds":game.ecology.seeds.iter().map(|s|s.units).sum::<u64>(),"afloat_seed_units":game.ecology.seeds.iter().filter(|s|s.afloat).map(|s|s.units).sum::<u64>(),"plant_nectar_units":game.ecology.plants.iter().map(|p|p.nectar).sum::<u64>(),"serviced_pollen_units":game.ecology.plants.iter().map(|p|p.serviced_pollen).sum::<u64>(),"guarded_clutches":game.ecology.clutches.iter().filter(|c|c.guarded).count(),"temperature_sampled_clutches":game.ecology.clutches.iter().filter(|c|c.temperature_samples>0).count(),"extinct_species":game.ecology.census().extinct_species},"all30consequences_qualified":false,"all160_implemented":false},
        "mutation":{"status":"unimplemented","reason":"Seed-generated profiles are not heritable mutation events"},
        "sanctuary":{"status":"unimplemented","reason":"Holding pens do not implement a sanctuary game system"},
        "wildlife_detail":state_entry(session.controller.initial_materialized>0,json!({"requested_existing_materializations":session.case.materialized_wildlife_target,"initial_existing_identities_materialized":session.controller.initial_materialized,"requested_detail_target_fully_met":session.controller.initial_materialized>=session.case.materialized_wildlife_target,"unmet_requested_existing_detail_count":session.case.materialized_wildlife_target.saturating_sub(session.controller.initial_materialized),"tracked_identity_count":session.controller.tracked_wildlife.len(),"current_fine_records":game.detailed.len(),"population_multiplication":false,"keepalive_scope":"Explicit benchmark tracking retains existing fine-position records, does not grant player vision or replace deaths"})),
        "ecological_score":state_entry(game.ecological_damage.iter().any(|v|*v>0),json!({"damage_attribution":game.ecological_damage,"outcome":game.outcome,"winner":game.winner})),
        "order_exercise":session.controller.orders.report(game)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn battle_commander_cadence_is_identical_for_smooth_frames_and_a_backlog() {
        fn run(inputs: &[Duration]) -> (Vec<u64>, Value) {
            let mut game = Campaign::new_battle_benchmark(78421, 128, 160).unwrap();
            let mut director = BattleDirector::default();
            let mut trace = Vec::new();
            let target = 120;
            for elapsed in inputs
                .iter()
                .copied()
                .chain(std::iter::repeat(Duration::ZERO))
            {
                let previous = director.last_tick;
                director.drive(&mut game).unwrap();
                if previous != director.last_tick {
                    trace.push(game.processed_tick);
                }
                let budget = battle_tick_budget(&director, game.processed_tick, target).unwrap();
                let (steps, _) = game
                    .advance_benchmark_profiled_budgeted(elapsed, 4, budget)
                    .unwrap();
                assert!(steps <= budget && budget <= 20);
                if game.processed_tick == target {
                    break;
                }
            }
            assert_eq!(game.clock.pending_ticks(), 0);
            assert_eq!(game.clock.remainder_nanos(), 0);
            let state = json!({"director":director,"orders":game.construction.jobs,"tactical_totals":game.tactics.totals});
            (trace, state)
        }
        let smooth = run(&vec![Duration::from_millis(50); 30]);
        let backlog = run(&[Duration::from_millis(1500)]);
        assert_eq!(smooth.0, vec![0, 20, 40, 60, 80, 100]);
        assert_eq!(smooth, backlog);
    }
    #[test]
    fn battle_budget_ends_at_both_decision_and_exact_target_without_discarding_debt() {
        let mut game = Campaign::new_battle_benchmark(78421, 128, 16).unwrap();
        let mut director = BattleDirector::default();
        director.drive(&mut game).unwrap();
        let budget = battle_tick_budget(&director, 0, 7).unwrap();
        assert_eq!(budget, 7);
        let (steps, _) = game
            .advance_benchmark_profiled_budgeted(Duration::from_secs(1), 4, budget)
            .unwrap();
        assert_eq!(steps, 7);
        assert_eq!(game.processed_tick, 7);
        assert_eq!(game.clock.pending_ticks(), 73);
        assert!(battle_tick_budget(&director, 7, 7).is_err());
    }
    #[test]
    fn plan_keeps_real_baseline_rates_and_explicit_safe_loads() {
        let p = plan();
        let mut names = BTreeSet::new();
        for c in &p.cases {
            assert!(names.insert(c.id.clone()));
            assert!((1..=100).contains(&c.requested_rate));
            assert!(c.military <= 200000 && c.workers <= 40000);
            if c.id.contains("rate-") {
                assert_eq!((c.military, c.workers), (10000, 2000));
            }
        }
        assert_eq!(case("cpu-season-78421").unwrap().target_ticks, Some(72000));
        assert_eq!(p.cases.iter().filter(|c| c.speed_ramp.is_some()).count(), 1);
        assert!(
            !p.cases
                .iter()
                .any(|c| c.id.starts_with("cpu-rate-") || c.id.starts_with("window-rate-"))
        );
        let ramp = case("window-speed-ramp").unwrap();
        assert_eq!(ramp.mode, BenchmarkMode::Window);
        assert_eq!(ramp.requested_rate, 1);
        assert_eq!((ramp.military, ramp.workers), (10000, 2000));
        assert!(ramp.speed_ramp.as_ref().is_some_and(SpeedRampConfig::valid));
        assert_ne!(
            ramp.checkpoint_group,
            case("cpu-season-78421").unwrap().checkpoint_group
        );
        assert_eq!(case("cpu-pop-200000-40000").unwrap().military, 200000);
        assert!(
            case("window-pop-100000-20000").unwrap().military
                + case("window-pop-100000-20000").unwrap().workers
                > 100000
        );
        let battle = case("window-battle-20m-4x").unwrap();
        assert!(battle.battle_scenario);
        assert_eq!(battle.mode, BenchmarkMode::Window);
        assert_eq!(battle.target_ticks, Some(24_000));
        assert_eq!(battle.requested_rate, 4);
        assert_eq!((battle.military, battle.workers), (10_000, 2_000));
        assert!(battle.speed_ramp.is_none());
        assert!(case("missing").is_none());
    }
    fn ramp_fixture() -> SpeedRamp {
        let mut ramp = SpeedRamp::new(case("window-speed-ramp").unwrap().speed_ramp.unwrap());
        ramp.begin_live_warmup(ramp_mark(0, 0, 0));
        ramp
    }
    fn ramp_mark(seconds: u64, tick: u64, pending: u64) -> RampMark {
        RampMark {
            wall: Duration::from_secs(seconds),
            tick,
            pending,
        }
    }
    fn ramp_presented_boundary(ramp: &mut SpeedRamp, mark: RampMark) {
        ramp.record_presentation(true);
        ramp.record_presentation(true);
        ramp.observe_boundary(mark);
    }
    #[test]
    fn adaptive_ramp_warms_one_world_and_stops_before_obviously_faster_failures() {
        let mut ramp = ramp_fixture();
        ramp_presented_boundary(&mut ramp, ramp_mark(14, 280, 0));
        assert_eq!(ramp.phase, RampPhase::Warmup);
        ramp_presented_boundary(&mut ramp, ramp_mark(15, 300, 0));
        assert_eq!(ramp.phase, RampPhase::Measuring);
        ramp_presented_boundary(&mut ramp, ramp_mark(25, 500, 0));
        assert_eq!(ramp.rate(), 1);
        assert!(ramp.stages.is_empty());
        ramp_presented_boundary(&mut ramp, ramp_mark(35, 700, 0));
        assert_eq!(ramp.rate(), 2);
        assert_eq!(ramp.phase, RampPhase::Settling);
        ramp_presented_boundary(&mut ramp, ramp_mark(40, 900, 0));
        ramp_presented_boundary(&mut ramp, ramp_mark(50, 1300, 0));
        ramp_presented_boundary(&mut ramp, ramp_mark(60, 1700, 0));
        assert_eq!(ramp.highest_sustained, Some(2));
        assert_eq!(ramp.rate(), 4);
        ramp_presented_boundary(&mut ramp, ramp_mark(65, 2100, 0));
        ramp_presented_boundary(&mut ramp, ramp_mark(75, 2780, 120));
        assert_eq!(ramp.phase, RampPhase::Measuring);
        ramp_presented_boundary(&mut ramp, ramp_mark(85, 3460, 240));
        assert_eq!(ramp.phase, RampPhase::Complete);
        let report = ramp.progress_report();
        assert_eq!(report["highest_sustained_rate"], 2);
        assert_eq!(
            report["completed_stages"][0]["measurement_start_wall_seconds"],
            15.
        );
        assert_eq!(
            report["completed_stages"][0]["measurement_end_wall_seconds"],
            35.
        );
        assert_eq!(
            report["completed_stages"][0]["windows"][0]["start_wall_seconds"],
            15.
        );
        assert_eq!(
            report["completed_stages"][0]["windows"][0]["end_wall_seconds"],
            25.
        );
        assert_eq!(
            report["remaining_rates_not_run"],
            json!([8, 16, 32, 64, 100])
        );
        assert_eq!(
            report["completed_stages"][2]["requested_rate_sustained"],
            false
        );
        assert_eq!(
            report["completed_stages"][2]["decision"],
            "two_consecutive_capacity_limited_windows"
        );
    }
    #[test]
    fn adaptive_ramp_never_qualifies_inherited_backlog_as_new_rate_capacity() {
        let mut ramp = ramp_fixture();
        ramp_presented_boundary(&mut ramp, ramp_mark(15, 300, 1000));
        // Catching old debt produces >1x ticks, but is not a clean1x stage.
        for (seconds, tick) in [(25, 1500), (35, 1700), (45, 1900), (55, 2100)] {
            ramp_presented_boundary(&mut ramp, ramp_mark(seconds, tick, 0));
        }
        assert_eq!(ramp.phase, RampPhase::Complete);
        assert_eq!(ramp.highest_sustained, None);
        assert_eq!(ramp.rate(), 1);
        assert_eq!(
            ramp.completion_reason,
            Some("higher_rates_not_run_after_inconclusive_capacity_confirmation")
        );
    }
    #[test]
    fn rate_boundary_tail_is_charged_once_even_when_next_delta_is_smaller() {
        let mut credit = Duration::from_millis(18);
        assert_eq!(
            consume_precharged_elapsed(Duration::from_millis(7), &mut credit),
            Duration::ZERO
        );
        assert_eq!(credit, Duration::from_millis(11));
        assert_eq!(
            consume_precharged_elapsed(Duration::from_millis(20), &mut credit),
            Duration::from_millis(9)
        );
        assert_eq!(credit, Duration::ZERO);
        assert_eq!(
            consume_precharged_elapsed(Duration::from_millis(4), &mut credit),
            Duration::from_millis(4)
        );
    }
    #[test]
    fn adaptive_ramp_runs_full_warmup_after_a_slow_first_native_presentation() {
        let mut ramp = SpeedRamp::new(case("window-speed-ramp").unwrap().speed_ramp.unwrap());
        ramp.record_presentation(false);
        assert!(!ramp.boundary_due(Duration::from_secs(60)));
        assert_eq!(ramp.phase, RampPhase::Loading);
        ramp.begin_live_warmup(ramp_mark(60, 1200, 0));
        assert_eq!(ramp.phase, RampPhase::Warmup);
        ramp.observe_boundary(ramp_mark(74, 1480, 0));
        assert_eq!(ramp.phase, RampPhase::Warmup);
        ramp.observe_boundary(ramp_mark(75, 1500, 0));
        assert_eq!(ramp.phase, RampPhase::Measuring);
        assert_eq!(ramp.measurement.wall, Duration::from_secs(75));
        assert!(ramp.stages.is_empty());
        let report = ramp.progress_report();
        assert_eq!(report["warmup_intervals"][0]["wall_seconds"], 60.);
        assert_eq!(report["warmup_intervals"][1]["wall_seconds"], 15.);
        assert_eq!(report["warmup_intervals"][0]["start_wall_seconds"], 0.);
        assert_eq!(report["warmup_intervals"][0]["end_wall_seconds"], 60.);
        assert_eq!(report["warmup_intervals"][1]["start_wall_seconds"], 60.);
        assert_eq!(report["warmup_intervals"][1]["end_wall_seconds"], 75.);
        assert_eq!(report["loading_unpresented_attempts"], 1);
    }
    #[test]
    fn adaptive_ramp_never_qualifies_minimized_or_unpresented_windows_as_gpu_capacity() {
        let mut ramp = ramp_fixture();
        ramp.observe_boundary(ramp_mark(15, 300, 0));
        for (seconds, tick) in [(25, 500), (35, 700)] {
            // The first call consumes the deliberately excluded boundary frame;
            // the next call represents a genuinely unpresented measured frame.
            ramp.record_presentation(false);
            ramp.record_presentation(false);
            ramp.observe_boundary(ramp_mark(seconds, tick, 0));
        }
        assert_eq!(ramp.phase, RampPhase::Complete);
        assert_eq!(ramp.highest_sustained, None);
        let report = ramp.progress_report();
        assert_eq!(
            report["completed_stages"][0]["requested_rate_sustained"],
            false
        );
        assert_eq!(
            report["completed_stages"][0]["native_presentation_evidence_valid"],
            false
        );
        assert_eq!(
            report["completed_stages"][0]["decision"],
            "native_presentation_interrupted_not_qualified"
        );
        assert_eq!(
            report["completed_stages"][0]["windows"][0]["capacity_limited"],
            false
        );
        assert_eq!(
            report["remaining_rates_not_run"],
            json!([2, 4, 8, 16, 32, 64, 100])
        );
    }
    #[test]
    fn timing_samples_distinguish_actual_tick_cost_from_requested_speed() {
        let mut samples = Samples::default();
        for n in [1_000_000, 2_000_000, 50_000_000, 3_000_000] {
            samples.push(n);
        }
        let report = samples.report();
        assert_eq!(report["count"], 4);
        assert_eq!(report["max_ms"], 50.);
        assert_eq!(report["p95_ms"], 50.);
        assert_eq!(report["distribution_complete"], true);
    }
    #[test]
    fn real_smoke_checkpoint_recovers_without_replaying_or_resetting_debt() {
        let output = std::env::temp_dir().join(format!(
            "last-world-benchmark-smoke-{}-{}",
            std::process::id(),
            now_ms()
        ));
        std::fs::create_dir_all(&output).unwrap();
        let report = run_cpu_case("smoke-cpu", &output).unwrap();
        assert_eq!(report["passed"], true);
        assert!(
            report["metrics"]["live_loop_started_utc_ms"]
                .as_u64()
                .is_some()
        );
        assert_eq!(report["checkpoint"]["committed"], true);
        assert_eq!(
            report["actual_counts"]["population_counts_were_not_reduced"],
            true
        );
        let (mut game, mut recovered) = prepare("smoke-cpu", &output).unwrap();
        let tick = game.processed_tick;
        let debt = game.clock.pending_ticks();
        assert_eq!(Some(tick), report["metrics"]["end_tick"].as_u64());
        assert_eq!(Some(debt), report["metrics"]["pending_ticks_end"].as_u64());
        assert!(recovered.recovered_completed_case());
        assert!(
            recovered
                .step(&mut game, Duration::from_secs(1))
                .unwrap()
                .done
        );
        assert_eq!(game.processed_tick, tick);
        assert_eq!(game.clock.pending_ticks(), debt);
        let replay = recovered.finish(&mut game).unwrap();
        assert_eq!(replay["passed"], true);
        assert_eq!(
            replay["recovered_previous_success"]["replayed_simulation"],
            false
        );
        drop(recovered);
        drop(game);
        let (fresh, retry) = prepare_for_attempt("smoke-cpu", &output, 2).unwrap();
        assert!(!retry.recovered_completed_case());
        assert_eq!(retry.replay_origin, "retry_fresh_named_initial_world");
        assert_eq!(fresh.processed_tick, 0);
        assert!(fresh.construction.jobs.is_empty());
        assert!(retry.controller.completed_cases.is_empty());
        assert!(
            !retry.checkpoint_path.exists(),
            "A failed first-phase image must be discarded before a real retry"
        );
        drop(retry);
        drop(fresh);
        std::fs::remove_dir_all(output).unwrap();
    }
    #[test]
    fn retry_rollback_promotes_prephase_preserves_debt_and_attempt_identity() {
        let output = std::env::temp_dir().join(format!(
            "last-world-benchmark-rollback-{}-{}",
            std::process::id(),
            now_ms()
        ));
        let identity = json!({"native_source_sha256":"0".repeat(64),"executable_sha256":"1".repeat(64),"scope":"unit-only checkpoint fixture, not a performance receipt"});
        let (mut game, mut session) =
            prepare_with_identity("smoke-cpu", &output, 1, identity.clone()).unwrap();
        game.clock
            .advance_benchmark(Duration::from_secs(1), false, 0, 100)
            .unwrap();
        let pre_tick = game.processed_tick;
        let pre_debt = game.clock.pending_ticks();
        session.controller.completed_cases = vec!["prephase-fixture".into()];
        let checkpoint_path = session.checkpoint_path.clone();
        let previous = BenchmarkCheckpoint {
            version: 1,
            attempt: 1,
            checkpoint_group: session.case.checkpoint_group.clone(),
            seed: session.case.seed,
            military: session.case.military,
            workers: session.case.workers,
            source_identity: identity.clone(),
            campaign: CampaignSave {
                version: 1,
                terrain: game.terrain.snapshot(),
                data: game.data.clone(),
            },
            controller: session.controller.clone(),
            last_case: "prephase-fixture".into(),
            last_report: json!({"passed":true,"scope":"unit prephase fixture"}),
        };
        campaign_support::save_atomic(campaign_support::previous_path(&checkpoint_path), &previous)
            .unwrap();
        drop(previous);
        game.advance_benchmark_profiled(Duration::ZERO, 4).unwrap();
        assert!(game.processed_tick > pre_tick);
        session.controller.completed_cases.push("smoke-cpu".into());
        let failed = BenchmarkCheckpoint {
            version: 1,
            attempt: 1,
            checkpoint_group: session.case.checkpoint_group.clone(),
            seed: session.case.seed,
            military: session.case.military,
            workers: session.case.workers,
            source_identity: identity.clone(),
            campaign: CampaignSave {
                version: 1,
                terrain: game.terrain.snapshot(),
                data: game.data.clone(),
            },
            controller: session.controller.clone(),
            last_case: "smoke-cpu".into(),
            last_report: json!({"passed":true,"scope":"unit simulated postcommit teardown rejection"}),
        };
        campaign_support::save_atomic(&checkpoint_path, &failed).unwrap();
        drop(failed);
        drop(session);
        drop(game);
        let ledger = RejectionLedger {
            schema_version: 1,
            rejections: vec![RejectedAttempt {
                case_id: "smoke-cpu".into(),
                attempt_index: 1,
                source_identity: RejectedBuild {
                    native_source_sha256: "0".repeat(64),
                    executable_sha256: "1".repeat(64),
                },
                reason: "unit postcommit failure".into(),
            }],
        };
        atomic_json(
            &output.join("rejected-attempts.json"),
            &serde_json::to_value(&ledger).unwrap(),
        )
        .unwrap();
        let (game, retry) = prepare_with_identity("smoke-cpu", &output, 2, identity).unwrap();
        assert!(!retry.recovered_completed_case());
        assert_eq!(retry.attempt, 2);
        assert_eq!(retry.replay_origin, "retry_previous_phase_snapshot");
        assert_eq!(game.processed_tick, pre_tick);
        assert_eq!(game.clock.pending_ticks(), pre_debt);
        assert_eq!(retry.controller.completed_cases, vec!["prephase-fixture"]);
        let mut promoted: BenchmarkCheckpoint = campaign_support::load(&checkpoint_path).unwrap();
        assert_eq!(
            promoted.last_case, "prephase-fixture",
            "Retry must promote the true prephase before any later successful commit"
        );
        promoted.last_case = "smoke-cpu".into();
        promoted.attempt = 2;
        assert!(
            rejected(&promoted, &ledger).is_none(),
            "An attempt1 rejection must never reject attempt2"
        );
        drop(retry);
        drop(game);
        std::fs::remove_dir_all(output).unwrap();
    }
}
