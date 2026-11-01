//! Campaign persistence and information ownership. GPU masks and dots are
//! derived from these CPU records; ordinary soldiers do not grant vision.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const VISIBLE: u32 = 1;
pub const RADAR_SWEEP: u32 = 2;
pub const RADAR_COVERAGE: u32 = 4;
pub const EXPLORED: u32 = 8;
pub const SAVE_SCHEMA: u32 = 2;
const SAVE_FORMAT: &str = "last-world-campaign";
const MAX_SAVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_INTEL_CELLS: usize = 4 * 1024 * 1024;
const CACHE_BUDGET_CELLS: usize = 4 * 1024 * 1024;

/// Accidental-corruption checksum, not a cryptographic authentication scheme.
fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ *b as u64).wrapping_mul(0x100000001b3)
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SaveEnvelope {
    format: String,
    schema_version: u32,
    payload_checksum: String,
    // Preserve the exact serialized bytes through JSON parsing. Reconstructing
    // a parsed Value before checksumming can change floating-point spellings.
    payload: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SaveReport {
    pub bytes: u64,
    pub schema_version: u32,
    pub previous_retained: bool,
    pub payload_checksum: String,
}

fn decode_envelope(bytes: &[u8]) -> Result<SaveEnvelope, String> {
    let value: SaveEnvelope = serde_json::from_slice(bytes)
        .map_err(|e| format!("Invalid campaign save envelope: {e}"))?;
    if value.format != SAVE_FORMAT {
        return Err("This is not a Last World campaign save".into());
    }
    if value.schema_version != SAVE_SCHEMA {
        return Err(format!(
            "Save schema {} needs an explicit migration; current schema is {SAVE_SCHEMA}",
            value.schema_version
        ));
    }
    let actual = format!("{:016x}", checksum(value.payload.as_bytes()));
    if value.payload_checksum != actual {
        return Err("Campaign save checksum mismatch; current file was not modified".into());
    }
    Ok(value)
}

#[cfg(not(target_arch = "wasm32"))]
mod files {
    use super::*;
    use std::{
        ffi::OsString,
        fs::{File, OpenOptions},
        io::{Read, Write},
        path::{Path, PathBuf},
        sync::atomic::{AtomicU64, Ordering},
    };
    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn suffix(path: &Path, extra: &str) -> PathBuf {
        let mut name: OsString = path.as_os_str().to_owned();
        name.push(extra);
        PathBuf::from(name)
    }
    pub fn previous_path(path: impl AsRef<Path>) -> PathBuf {
        suffix(path.as_ref(), ".previous")
    }
    fn temporary_path(path: &Path) -> PathBuf {
        suffix(
            path,
            &format!(
                ".pending-{}-{}",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ),
        )
    }
    struct Pending(PathBuf);
    impl Drop for Pending {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    /// OS-held cooperative lock: a crash closes its file handle automatically.
    /// The empty lock file can remain without permanently disabling saves.
    struct SaveLock {
        _file: File,
    }
    fn lock(path: &Path) -> Result<SaveLock, String> {
        let path = suffix(path, ".save-lock");
        let mut options = OpenOptions::new();
        options.create(true).read(true).write(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let file = options
            .open(&path)
            .map_err(|e| format!("Cannot acquire save lock {}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe extern "C" {
                fn flock(fd: i32, operation: i32) -> i32;
            }
            // LOCK_EX | LOCK_NB. File ownership keeps the lock until drop.
            if unsafe { flock(file.as_raw_fd(), 2 | 4) } != 0 {
                return Err(format!(
                    "Another process is writing this save: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Ok(SaveLock { _file: file })
    }

    fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .map_err(|e| format!("Creating {}: {e}", path.display()))?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| format!("Writing/syncing {}: {e}", path.display()))
    }
    fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
        let file = File::open(path).map_err(|e| format!("Opening {}: {e}", path.display()))?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_SAVE_BYTES {
            return Err("Campaign save exceeds the explicit512MiB safety limit".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_SAVE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SAVE_BYTES {
            return Err("Campaign save grew past the explicit512MiB safety limit".into());
        }
        Ok(bytes)
    }
    #[cfg(not(windows))]
    fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
        std::fs::rename(from, to)
            .map_err(|e| format!("Atomic rename {} -> {}: {e}", from.display(), to.display()))
    }
    #[cfg(windows)]
    fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
        }
        let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
        // MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH. Both files are
        // siblings on one filesystem; never remove the current save first.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 1 | 8) } == 0 {
            return Err(format!(
                "Atomic Windows save replacement: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
    fn sync_parent(path: &Path) -> Result<(), String> {
        #[cfg(unix)]
        File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("Save directory sync: {e}"))?;
        Ok(())
    }

    /// Serialize exact authoritative state, durably stage it, retain a verified
    /// previous snapshot, then atomically replace the current save. Never read
    /// pending files as saves. Callers perform their own semantic validation.
    pub fn save_atomic<T: Serialize>(
        path: impl AsRef<Path>,
        state: &T,
    ) -> Result<SaveReport, String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let _lock = lock(path)?;
        let payload = serde_json::to_string(state).map_err(|e| e.to_string())?;
        let payload_checksum = format!("{:016x}", checksum(payload.as_bytes()));
        let envelope = SaveEnvelope {
            format: SAVE_FORMAT.into(),
            schema_version: SAVE_SCHEMA,
            payload_checksum: payload_checksum.clone(),
            payload,
        };
        let bytes = serde_json::to_vec(&envelope).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SAVE_BYTES {
            return Err("Campaign save exceeds the explicit512MiB safety limit".into());
        }
        let pending = Pending(temporary_path(path));
        write_new(&pending.0, &bytes)?;
        // Validate the serialized representation before replacing any file.
        decode_envelope(&read_bounded(&pending.0)?)?;
        let previous_retained = path.exists();
        if previous_retained {
            let old = read_bounded(path)?;
            decode_envelope(&old).map_err(|e| {
                format!("Refusing to replace a corrupt/unsupported current save: {e}. Recover explicitly from .previous first")
            })?;
            let previous = previous_path(path);
            let backup_pending = Pending(temporary_path(&previous));
            write_new(&backup_pending.0, &old)?;
            replace_file(&backup_pending.0, &previous)?;
            sync_parent(path)?;
        }
        replace_file(&pending.0, path)?;
        sync_parent(path)?;
        Ok(SaveReport {
            bytes: bytes.len() as u64,
            schema_version: SAVE_SCHEMA,
            previous_retained,
            payload_checksum,
        })
    }

    pub fn load<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, String> {
        let envelope = decode_envelope(&read_bounded(path.as_ref())?)?;
        serde_json::from_str(&envelope.payload).map_err(|e| format!("Campaign state schema: {e}"))
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum LoadSource {
        Current,
        Previous { current_error: String },
    }
    /// Recovery is explicit in the return value; it does not overwrite either
    /// file or silently pretend the latest progress survived corruption.
    pub fn load_with_backup<T: serde::de::DeserializeOwned>(
        path: impl AsRef<Path>,
    ) -> Result<(T, LoadSource), String> {
        let path = path.as_ref();
        match load(path) {
            Ok(state) => Ok((state, LoadSource::Current)),
            Err(current_error) => load(previous_path(path))
                .map(|state| {
                    (
                        state,
                        LoadSource::Previous {
                            current_error: current_error.clone(),
                        },
                    )
                })
                .map_err(|previous_error| {
                    format!("Current save: {current_error}; previous save: {previous_error}")
                }),
        }
    }

    /// The only accepted legacy-v1 envelope is explicitly named and migrated to
    /// a separate destination. No existing current/previous file is deleted.
    pub fn migrate_v1<T: Serialize + serde::de::DeserializeOwned>(
        legacy: impl AsRef<Path>,
        destination: impl AsRef<Path>,
        validate: impl FnOnce(&T) -> Result<(), String>,
    ) -> Result<SaveReport, String> {
        #[derive(Deserialize)]
        struct Legacy {
            format: String,
            schema_version: u32,
            state: serde_json::Value,
        }
        let legacy = legacy.as_ref();
        let destination = destination.as_ref();
        if legacy == destination {
            return Err(
                "V1 migration requires a separate destination to preserve the original".into(),
            );
        }
        let value: Legacy = serde_json::from_slice(&read_bounded(legacy)?)
            .map_err(|e| format!("Legacy v1 envelope: {e}"))?;
        if value.format != SAVE_FORMAT || value.schema_version != 1 {
            return Err("Expected explicit Last World legacy-v1 envelope".into());
        }
        let state: T = serde_json::from_value(value.state).map_err(|e| e.to_string())?;
        validate(&state)?;
        save_atomic(destination, &state)
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use files::{LoadSource, load, load_with_backup, migrate_v1, previous_path, save_atomic};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SensorKind {
    ShipCamera,
    Camera,
    CameraTrooper,
    Radar,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sensor {
    pub id: u64,
    pub team: u8,
    pub kind: SensorKind,
    pub position: [f32; 2],
    /// Height above local ground, not an absolute world elevation.
    pub height_mm: u32,
    pub range_m: f32,
    pub enabled: bool,
    /// Parent must establish actual arrival in an owned recharge facility.
    pub at_base: bool,
    pub battery_capacity_ticks: u32,
    pub recharge_ticks_per_tick: u32,
    pub sweep_period_ticks: u32,
}
impl Sensor {
    pub fn new(id: u64, team: u8, kind: SensorKind, position: [f32; 2]) -> Self {
        Self {
            id,
            team,
            kind,
            position,
            height_mm: match kind {
                SensorKind::Radar => 12_000,
                SensorKind::CameraTrooper => 1_700,
                SensorKind::ShipCamera | SensorKind::Camera => 8_000,
            },
            range_m: if kind == SensorKind::Radar {
                950.
            } else {
                420.
            },
            enabled: true,
            at_base: false,
            battery_capacity_ticks: 12_000,
            recharge_ticks_per_tick: 4,
            sweep_period_ticks: 160,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Battery {
    sensor_id: u64,
    team: u8,
    remaining_ticks: u32,
    capacity_ticks: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatteryStatus {
    pub remaining_ticks: u32,
    pub capacity_ticks: u32,
    pub active: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SatelliteConfig {
    pub enabled: bool,
    pub period_ticks: u32,
    pub duration_ticks: u32,
    pub team_phase_offsets: [u32; 2],
    pub radius_m: f32,
    pub prewarm_ticks: u32,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SatelliteCircle {
    pub center: [f32; 2],
    pub radius_m: f32,
}

/// No faction, species, health or role is present in the player-facing echo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadarEcho {
    pub opaque_track: u64,
    pub position: [f32; 2],
    pub last_detected_tick: u64,
}
#[derive(Clone, Copy, Debug)]
pub struct ContactCandidate {
    pub opaque_track: u64,
    pub position: [f32; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct TeamIntel {
    masks: Vec<u32>,
    last_step: Option<u64>,
    echoes: Vec<RadarEcho>,
    #[serde(skip)]
    prewarm: Vec<u32>,
}
#[derive(Clone, Debug, Default)]
struct SensorCache {
    team: u8,
    id: u64,
    signature: Option<u64>,
    cells: Vec<u32>,
}

/// Coarse recon is a flat CPU resource, never one ECS entity per cell or troop.
/// Static LOS footprints are reused until sensor geometry or blockers change.
#[derive(bevy::prelude::Resource, Clone, Debug, Serialize, Deserialize)]
pub struct Recon {
    pub width: usize,
    pub height: usize,
    pub cell_meters: f32,
    pub satellite: SatelliteConfig,
    pub echo_lifetime_ticks: u32,
    teams: [TeamIntel; 2],
    batteries: Vec<Battery>,
    #[serde(skip)]
    caches: Vec<SensorCache>,
}

impl Recon {
    pub fn new(width: usize, height: usize, cell_meters: f32) -> Result<Self, String> {
        let len = width
            .checked_mul(height)
            .ok_or("Recon dimensions overflow")?;
        if width == 0
            || height == 0
            || len > MAX_INTEL_CELLS
            || !cell_meters.is_finite()
            || cell_meters <= 0.
        {
            return Err("Invalid/beyond-budget recon grid dimensions".into());
        }
        let team = || TeamIntel {
            masks: vec![0; len],
            last_step: None,
            echoes: Vec::new(),
            prewarm: Vec::new(),
        };
        Ok(Self {
            width,
            height,
            cell_meters,
            satellite: SatelliteConfig {
                enabled: true,
                period_ticks: 3_600,
                duration_ticks: 600,
                team_phase_offsets: [0, 1_800],
                // One moving circular footprint reaches both map edges in Y.
                radius_m: height as f32 * cell_meters * 0.62,
                prewarm_ticks: 120,
            },
            echo_lifetime_ticks: 180,
            teams: [team(), team()],
            batteries: Vec::new(),
            caches: Vec::new(),
        })
    }
    pub fn validate(&self) -> Result<(), String> {
        let len = self
            .width
            .checked_mul(self.height)
            .ok_or("Recon dimensions overflow")?;
        if self.width == 0
            || self.height == 0
            || len > MAX_INTEL_CELLS
            || !self.cell_meters.is_finite()
            || self.cell_meters <= 0.
        {
            return Err("Invalid recon grid".into());
        }
        let sat = &self.satellite;
        if sat.period_ticks == 0
            || sat.duration_ticks == 0
            || sat.duration_ticks >= sat.period_ticks
            || sat.prewarm_ticks >= sat.period_ticks
            || !sat.radius_m.is_finite()
            || sat.radius_m <= 0.
        {
            return Err("Invalid satellite schedule/footprint".into());
        }
        for team in &self.teams {
            if team.masks.len() != len || team.masks.iter().any(|m| *m & !15 != 0) {
                return Err("Invalid saved recon mask dimensions/bits".into());
            }
            let mut tracks = BTreeSet::new();
            for echo in &team.echoes {
                if !tracks.insert(echo.opaque_track)
                    || self.index(echo.position).is_none()
                    || team
                        .last_step
                        .is_some_and(|tick| echo.last_detected_tick > tick)
                {
                    return Err("Invalid saved radar echo".into());
                }
            }
        }
        let mut battery_ids = BTreeSet::new();
        for battery in &self.batteries {
            if battery.team > 1
                || battery.capacity_ticks == 0
                || battery.remaining_ticks > battery.capacity_ticks
                || !battery_ids.insert((battery.team, battery.sensor_id))
            {
                return Err("Invalid/duplicate saved camera battery".into());
            }
        }
        Ok(())
    }
    fn index(&self, position: [f32; 2]) -> Option<usize> {
        if !position[0].is_finite()
            || !position[1].is_finite()
            || position[0] < 0.
            || position[1] < 0.
        {
            return None;
        }
        let x = (position[0] / self.cell_meters) as usize;
        let y = (position[1] / self.cell_meters) as usize;
        (x < self.width && y < self.height).then_some(x + y * self.width)
    }
    pub fn visible(&self, team: u8, position: [f32; 2]) -> bool {
        self.teams
            .get(team as usize)
            .and_then(|t| self.index(position).map(|i| t.masks[i] & VISIBLE != 0))
            .unwrap_or(false)
    }
    /// True only in the current unobstructed rotating beam. This does not reveal
    /// a current target's position; render persistent RadarEcho snapshots.
    pub fn radar_contact(&self, team: u8, position: [f32; 2]) -> bool {
        self.teams
            .get(team as usize)
            .and_then(|t| self.index(position).map(|i| t.masks[i] & RADAR_SWEEP != 0))
            .unwrap_or(false)
    }
    pub fn explored(&self, team: u8, position: [f32; 2]) -> bool {
        self.teams
            .get(team as usize)
            .and_then(|t| self.index(position).map(|i| t.masks[i] & EXPLORED != 0))
            .unwrap_or(false)
    }
    pub fn masks(&self, team: u8) -> &[u32] {
        self.teams
            .get(team as usize)
            .map(|t| t.masks.as_slice())
            .unwrap_or(&[])
    }
    pub fn prewarm_cells(&self, team: u8) -> &[u32] {
        self.teams
            .get(team as usize)
            .map(|t| t.prewarm.as_slice())
            .unwrap_or(&[])
    }
    pub fn echoes(&self, team: u8) -> &[RadarEcho] {
        self.teams
            .get(team as usize)
            .map(|t| t.echoes.as_slice())
            .unwrap_or(&[])
    }
    pub fn battery_status(&self, team: u8, sensor_id: u64) -> Option<BatteryStatus> {
        self.batteries
            .iter()
            .find(|b| b.team == team && b.sensor_id == sensor_id)
            .map(|b| BatteryStatus {
                remaining_ticks: b.remaining_ticks,
                capacity_ticks: b.capacity_ticks,
                active: b.remaining_ticks > 0,
            })
    }
    pub fn satellite_circle(&self, tick: u64, team: u8) -> Option<SatelliteCircle> {
        if team > 1 || !self.satellite.enabled {
            return None;
        }
        let sat = &self.satellite;
        if sat.period_ticks == 0
            || sat.duration_ticks == 0
            || sat.duration_ticks >= sat.period_ticks
            || !sat.radius_m.is_finite()
            || sat.radius_m <= 0.
        {
            return None;
        }
        let phase = phase(
            tick,
            sat.period_ticks,
            sat.team_phase_offsets[team as usize],
        );
        if phase >= sat.duration_ticks {
            return None;
        }
        Some(SatelliteCircle {
            center: [
                self.satellite_x(phase),
                self.height as f32 * self.cell_meters * 0.5,
            ],
            radius_m: sat.radius_m,
        })
    }
    fn satellite_x(&self, phase: u32) -> f32 {
        let sat = &self.satellite;
        -sat.radius_m
            + (self.width as f32 * self.cell_meters + sat.radius_m * 2.) * phase as f32
                / sat.duration_ticks as f32
    }
    fn satellite_segments(&self, start: u64, end: u64, team: u8) -> Vec<[f32; 2]> {
        if !self.satellite.enabled {
            return Vec::new();
        }
        let sat = &self.satellite;
        if end - start >= sat.period_ticks as u64 {
            return vec![[
                -sat.radius_m,
                self.width as f32 * self.cell_meters + sat.radius_m,
            ]];
        }
        let a = phase(
            start,
            sat.period_ticks,
            sat.team_phase_offsets[team as usize],
        );
        let b = phase(end, sat.period_ticks, sat.team_phase_offsets[team as usize]);
        let mut result = Vec::with_capacity(2);
        let mut add = |lo: u32, hi: u32| {
            if lo <= sat.duration_ticks {
                result.push([
                    self.satellite_x(lo),
                    self.satellite_x(hi.min(sat.duration_ticks)),
                ]);
            }
        };
        if a <= b {
            add(a, b);
        } else {
            add(a, sat.period_ticks);
            add(0, b);
        }
        result
    }
    pub fn step(
        &mut self,
        tick: u64,
        team: u8,
        sensors: &[Sensor],
        ground_mm: &[i32],
        solid: &[bool],
    ) -> Result<(), String> {
        self.step_internal(tick, team, sensors, ground_mm, None, solid)
    }
    /// Object offsets above ground are separate so a pole's own ground height
    /// is never counted twice. Terrain is solid even where `solid` is false;
    /// `solid` controls whether an object offset blocks this ray.
    pub fn step_with_obstructions(
        &mut self,
        tick: u64,
        team: u8,
        sensors: &[Sensor],
        ground_mm: &[i32],
        object_height_mm: &[u32],
        solid: &[bool],
    ) -> Result<(), String> {
        self.step_internal(
            tick,
            team,
            sensors,
            ground_mm,
            Some(object_height_mm),
            solid,
        )
    }
    fn step_internal(
        &mut self,
        tick: u64,
        team: u8,
        sensors: &[Sensor],
        ground_mm: &[i32],
        objects: Option<&[u32]>,
        solid: &[bool],
    ) -> Result<(), String> {
        self.validate()?;
        let len = self.width * self.height;
        if team > 1
            || ground_mm.len() != len
            || solid.len() != len
            || objects.is_some_and(|o| o.len() != len)
        {
            return Err("Recon team/grid shape mismatch".into());
        }
        let slot = team as usize;
        if self.teams[slot].last_step.is_some_and(|last| tick <= last) {
            return Err("Duplicate/stale recon tick rejected".into());
        }
        let future = tick
            .checked_add(self.satellite.prewarm_ticks as u64)
            .ok_or("Satellite prewarm tick overflow")?;
        let mut sensor_ids = BTreeSet::new();
        for sensor in sensors {
            if sensor.team > 1
                || self.index(sensor.position).is_none()
                || !sensor.range_m.is_finite()
                || sensor.range_m <= 0.
                || sensor.battery_capacity_ticks == 0
                || sensor.recharge_ticks_per_tick == 0
                || sensor.sweep_period_ticks == 0
                || !sensor_ids.insert((sensor.team, sensor.id))
            {
                return Err("Invalid/duplicate recon sensor".into());
            }
            if let Some(battery) = self
                .batteries
                .iter()
                .find(|b| b.team == sensor.team && b.sensor_id == sensor.id)
            {
                if sensor.kind == SensorKind::CameraTrooper
                    && battery.capacity_ticks != sensor.battery_capacity_ticks
                {
                    return Err(
                        "Camera capacity change needs an explicit battery upgrade transaction"
                            .into(),
                    );
                }
            }
        }
        let elapsed = self.teams[slot]
            .last_step
            .map(|last| tick - last)
            .unwrap_or(0);
        let old_tick = self.teams[slot].last_step.unwrap_or(tick);
        let explored_segments = self.satellite_segments(old_tick, tick, team);
        let prewarm_segments = self.satellite_segments(tick, future, team);
        let circle = self.satellite_circle(tick, team);
        let width = self.width;
        let height = self.height;
        let cell_meters = self.cell_meters;
        let team_intel = &mut self.teams[slot];
        for bits in &mut team_intel.masks {
            *bits &= EXPLORED;
        }
        team_intel.prewarm.clear();
        for sensor in sensors.iter().filter(|s| s.team == team) {
            let mut powered = sensor.enabled;
            if sensor.kind == SensorKind::CameraTrooper {
                let mut battery_elapsed = elapsed;
                let battery_index = match self
                    .batteries
                    .iter()
                    .position(|b| b.team == team && b.sensor_id == sensor.id)
                {
                    Some(i) => i,
                    None => {
                        // No charge is consumed before this sensor exists.
                        battery_elapsed = 0;
                        self.batteries.push(Battery {
                            sensor_id: sensor.id,
                            team,
                            remaining_ticks: sensor.battery_capacity_ticks,
                            capacity_ticks: sensor.battery_capacity_ticks,
                        });
                        self.batteries.len() - 1
                    }
                };
                let battery = &mut self.batteries[battery_index];
                if sensor.at_base {
                    let gain =
                        battery_elapsed.saturating_mul(sensor.recharge_ticks_per_tick as u64);
                    battery.remaining_ticks =
                        (battery.remaining_ticks as u64 + gain.min(battery.capacity_ticks as u64))
                            .min(battery.capacity_ticks as u64) as u32;
                } else if sensor.enabled {
                    battery.remaining_ticks = battery
                        .remaining_ticks
                        .saturating_sub(battery_elapsed.min(u32::MAX as u64) as u32);
                }
                powered &= battery.remaining_ticks > 0;
            }
            if !powered {
                continue;
            }
            // A distant crater must not invalidate every camera on the map.
            let geometry_signature = geometry_hash_local(
                sensor,
                width,
                height,
                cell_meters,
                ground_mm,
                objects,
                solid,
            );
            let signature = sensor_hash(sensor, geometry_signature);
            let cache_index = match self
                .caches
                .iter()
                .position(|c| c.team == team && c.id == sensor.id)
            {
                Some(i) => i,
                None => {
                    self.caches.push(SensorCache {
                        team,
                        id: sensor.id,
                        ..SensorCache::default()
                    });
                    self.caches.len() - 1
                }
            };
            if self.caches[cache_index].signature != Some(signature) {
                self.caches[cache_index].cells = sensor_footprint(
                    sensor,
                    width,
                    height,
                    cell_meters,
                    ground_mm,
                    objects,
                    solid,
                );
                self.caches[cache_index].signature = Some(signature);
            }
            for &cell in &self.caches[cache_index].cells {
                let bits = &mut team_intel.masks[cell as usize];
                if sensor.kind == SensorKind::Radar {
                    *bits |= RADAR_COVERAGE;
                    let position = [
                        (cell as usize % width) as f32 * cell_meters + cell_meters * 0.5,
                        (cell as usize / width) as f32 * cell_meters + cell_meters * 0.5,
                    ];
                    if in_beam(sensor, tick, position) {
                        *bits |= RADAR_SWEEP;
                    }
                } else {
                    *bits |= VISIBLE | EXPLORED;
                }
            }
        }
        let y_mid = height as f32 * cell_meters * 0.5;
        let radius2 = self.satellite.radius_m * self.satellite.radius_m;
        for i in 0..len {
            let p = [
                (i % width) as f32 * cell_meters + cell_meters * 0.5,
                (i / width) as f32 * cell_meters + cell_meters * 0.5,
            ];
            if circle.is_some_and(|c| distance2(p, c.center) <= radius2) {
                team_intel.masks[i] |= VISIBLE | EXPLORED;
            }
            if explored_segments
                .iter()
                .any(|s| segment_distance2(p, *s, y_mid) <= radius2)
            {
                team_intel.masks[i] |= EXPLORED;
            }
            if prewarm_segments
                .iter()
                .any(|s| segment_distance2(p, *s, y_mid) <= radius2)
            {
                team_intel.prewarm.push(i as u32);
            }
        }
        team_intel.last_step = Some(tick);
        // Masks no longer need old caches. Bound this derived cache without
        // removing sensors or changing their gameplay visibility semantics.
        self.caches
            .retain(|cache| cache.team != team || sensor_ids.contains(&(cache.team, cache.id)));
        if self.caches.iter().map(|c| c.cells.len()).sum::<usize>() > CACHE_BUDGET_CELLS {
            self.caches.clear();
        }
        Ok(())
    }

    /// Call after this team's step with real candidate positions at the SAME
    /// tick. A hidden target's new position is copied only under the beam. Old
    /// dots persist briefly; returning current true positions would leak intel.
    pub fn sweep_contacts(
        &mut self,
        tick: u64,
        team: u8,
        candidates: &[ContactCandidate],
    ) -> Result<(), String> {
        if team > 1 || self.teams[team as usize].last_step != Some(tick) {
            return Err("Radar contacts need the matching accepted recon tick".into());
        }
        let mut candidate_ids = BTreeSet::new();
        for candidate in candidates {
            if self.index(candidate.position).is_none()
                || !candidate_ids.insert(candidate.opaque_track)
            {
                return Err("Invalid/duplicate radar contact candidate".into());
            }
        }
        let width = self.width;
        let cell_meters = self.cell_meters;
        let intel = &mut self.teams[team as usize];
        let mut echoes: BTreeMap<u64, RadarEcho> = std::mem::take(&mut intel.echoes)
            .into_iter()
            .filter(|e| {
                let i = (e.position[0] / cell_meters) as usize
                    + (e.position[1] / cell_meters) as usize * width;
                tick.checked_sub(e.last_detected_tick)
                    .is_some_and(|age| age <= self.echo_lifetime_ticks as u64)
                    && intel.masks[i] & (VISIBLE | RADAR_SWEEP) == 0
            })
            .map(|e| (e.opaque_track, e))
            .collect();
        for candidate in candidates {
            let index = (candidate.position[0] / cell_meters) as usize
                + (candidate.position[1] / cell_meters) as usize * width;
            if intel.masks[index] & RADAR_SWEEP != 0 && intel.masks[index] & VISIBLE == 0 {
                let echo = RadarEcho {
                    opaque_track: candidate.opaque_track,
                    position: candidate.position,
                    last_detected_tick: tick,
                };
                echoes.insert(candidate.opaque_track, echo);
            }
        }
        intel.echoes = echoes.into_values().collect();
        Ok(())
    }
}

fn distance2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let d = [a[0] - b[0], a[1] - b[1]];
    d[0] * d[0] + d[1] * d[1]
}
fn segment_distance2(p: [f32; 2], segment: [f32; 2], y: f32) -> f32 {
    let x = p[0].clamp(segment[0], segment[1]);
    distance2(p, [x, y])
}
fn phase(tick: u64, period: u32, offset: u32) -> u32 {
    ((tick % period as u64 + offset as u64 % period as u64) % period as u64) as u32
}
fn geometry_hash_local(
    sensor: &Sensor,
    width: usize,
    height: usize,
    meters: f32,
    ground: &[i32],
    objects: Option<&[u32]>,
    solid: &[bool],
) -> u64 {
    let sx = (sensor.position[0] / meters) as usize;
    let sy = (sensor.position[1] / meters) as usize;
    let radius = (sensor.range_m / meters).ceil() as usize;
    let mut h = 0xcbf29ce484222325u64;
    for y in sy.saturating_sub(radius)..=sy.saturating_add(radius).min(height - 1) {
        for x in sx.saturating_sub(radius)..=sx.saturating_add(radius).min(width - 1) {
            let i = x + y * width;
            h = (h ^ ground[i] as u32 as u64).wrapping_mul(0x100000001b3);
            if solid[i] {
                h = (h ^ objects.map_or(0, |o| o[i]) as u64).wrapping_mul(0x100000001b3);
            }
        }
    }
    h
}

fn sensor_hash(s: &Sensor, geometry: u64) -> u64 {
    [
        s.position[0].to_bits() as u64,
        s.position[1].to_bits() as u64,
        s.height_mm as u64,
        s.range_m.to_bits() as u64,
        s.kind as u64,
    ]
    .into_iter()
    .fold(geometry, |h, v| (h ^ v).wrapping_mul(0x100000001b3))
}
fn in_beam(sensor: &Sensor, tick: u64, position: [f32; 2]) -> bool {
    let phase = (tick % sensor.sweep_period_ticks as u64) as f32 / sensor.sweep_period_ticks as f32;
    let angle = phase * std::f32::consts::TAU + (sensor.id % 8) as f32 * 0.73;
    let d = [
        position[0] - sensor.position[0],
        position[1] - sensor.position[1],
    ];
    let delta = (d[1].atan2(d[0]) - angle)
        .sin()
        .atan2((d[1].atan2(d[0]) - angle).cos());
    delta.abs() <= std::f32::consts::PI / 12.
}
fn sensor_footprint(
    sensor: &Sensor,
    width: usize,
    height: usize,
    meters: f32,
    ground: &[i32],
    objects: Option<&[u32]>,
    solid: &[bool],
) -> Vec<u32> {
    let sx = (sensor.position[0] / meters) as usize;
    let sy = (sensor.position[1] / meters) as usize;
    let radius = (sensor.range_m / meters).ceil() as usize;
    let eye = ground[sx + sy * width] as i64 + sensor.height_mm as i64;
    let mut result = Vec::new();
    for y in sy.saturating_sub(radius)..=sy.saturating_add(radius).min(height - 1) {
        for x in sx.saturating_sub(radius)..=sx.saturating_add(radius).min(width - 1) {
            let target = [
                x as f32 * meters + meters * 0.5,
                y as f32 * meters + meters * 0.5,
            ];
            if distance2(target, sensor.position) > sensor.range_m * sensor.range_m {
                continue;
            }
            let steps = sx.abs_diff(x).max(sy.abs_diff(y));
            let finish = ground[x + y * width] as i64 + 1_400;
            let mut clear = true;
            for n in 1..steps {
                let ratio = n as f64 / steps as f64;
                let ix = (sx as f64 + (x as f64 - sx as f64) * ratio).round() as usize;
                let iy = (sy as f64 + (y as f64 - sy as f64) * ratio).round() as usize;
                let i = ix + iy * width;
                let top = ground[i] as i64
                    + if solid[i] {
                        objects.map(|o| o[i] as i64).unwrap_or(0)
                    } else {
                        0
                    };
                let limit = if sensor.kind == SensorKind::Radar {
                    eye as f64
                } else {
                    eye as f64 + (finish - eye) as f64 * ratio
                };
                if top as f64 > limit {
                    clear = false;
                    break;
                }
            }
            if clear {
                result.push((x + y * width) as u32);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn no_satellite(width: usize, height: usize) -> Recon {
        let mut r = Recon::new(width, height, 10.).unwrap();
        r.satellite.enabled = false;
        r
    }
    fn camera() -> Sensor {
        Sensor::new(0, 0, SensorKind::Camera, [15., 25.])
    }
    #[test]
    fn ordinary_units_give_no_vision_and_saved_exploration_is_not_current_vision() {
        let mut r = no_satellite(12, 6);
        let heights = vec![0; 72];
        let solids = vec![false; 72];
        r.step(0, 0, &[], &heights, &solids).unwrap();
        assert!(!r.visible(0, [35., 25.]));
        r.step(1, 0, &[camera()], &heights, &solids).unwrap();
        assert!(r.visible(0, [35., 25.]));
        r.step(2, 0, &[], &heights, &solids).unwrap();
        assert!(!r.visible(0, [35., 25.]));
        assert!(r.explored(0, [35., 25.]));
    }
    #[test]
    fn taller_solid_blocks_radar_and_camera_but_short_or_nonsolid_objects_do_not() {
        let mut r = no_satellite(12, 6);
        let heights = vec![0; 72];
        let mut solids = vec![false; 72];
        let mut objects = vec![0; 72];
        for y in 0..6 {
            solids[5 + y * 12] = true;
            objects[5 + y * 12] = 20_000;
        }
        let mut s = camera();
        s.kind = SensorKind::Radar;
        s.height_mm = 8_000;
        r.step_with_obstructions(0, 0, &[s.clone()], &heights, &objects, &solids)
            .unwrap();
        assert_eq!(r.masks(0)[9 + 2 * 12] & RADAR_COVERAGE, 0);
        for o in &mut objects {
            *o = 4_000;
        }
        r.step_with_obstructions(1, 0, &[s.clone()], &heights, &objects, &solids)
            .unwrap();
        assert_ne!(r.masks(0)[9 + 2 * 12] & RADAR_COVERAGE, 0);
        for o in &mut objects {
            *o = 20_000;
        }
        solids.fill(false);
        r.step_with_obstructions(2, 0, &[s], &heights, &objects, &solids)
            .unwrap();
        assert_ne!(r.masks(0)[9 + 2 * 12] & RADAR_COVERAGE, 0);
        solids.fill(true);
        r.step_with_obstructions(3, 0, &[camera()], &heights, &objects, &solids)
            .unwrap();
        assert!(!r.visible(0, [95., 25.]));
    }
    #[test]
    fn battery_runs_out_recharges_at_real_base_and_duplicate_tick_cannot_charge_it_twice() {
        let mut r = no_satellite(12, 6);
        let heights = vec![0; 72];
        let solids = vec![false; 72];
        let mut s = camera();
        s.kind = SensorKind::CameraTrooper;
        s.battery_capacity_ticks = 20;
        r.step(0, 0, &[s.clone()], &heights, &solids).unwrap();
        r.step(20, 0, &[s.clone()], &heights, &solids).unwrap();
        assert!(!r.visible(0, [35., 25.]));
        assert_eq!(r.battery_status(0, 0).unwrap().remaining_ticks, 0);
        s.at_base = true;
        r.step(21, 0, &[s.clone()], &heights, &solids).unwrap();
        assert_eq!(r.battery_status(0, 0).unwrap().remaining_ticks, 4);
        assert!(r.step(21, 0, &[s], &heights, &solids).is_err());
        assert_eq!(r.battery_status(0, 0).unwrap().remaining_ticks, 4);
    }
    #[test]
    fn newly_spawned_camera_is_not_charged_for_time_before_it_existed() {
        let mut r = no_satellite(12, 6);
        let heights = vec![0; 72];
        let solids = vec![false; 72];
        r.step(0, 0, &[], &heights, &solids).unwrap();
        let mut s = camera();
        s.kind = SensorKind::CameraTrooper;
        s.battery_capacity_ticks = 20;
        r.step(300, 0, &[s], &heights, &solids).unwrap();
        assert_eq!(r.battery_status(0, 0).unwrap().remaining_ticks, 20);
        assert!(r.visible(0, [35., 25.]));
    }
    #[test]
    fn remote_terrain_edits_preserve_local_los_cache_but_local_edits_invalidate_it() {
        let mut r = no_satellite(60, 20);
        let mut heights = vec![0; 1200];
        let solids = vec![false; 1200];
        let mut s = camera();
        s.range_m = 100.;
        r.step(0, 0, &[s.clone()], &heights, &solids).unwrap();
        let signature = r.caches[0].signature;
        heights[59 + 19 * 60] = 90_000;
        r.step(1, 0, &[s.clone()], &heights, &solids).unwrap();
        assert_eq!(r.caches[0].signature, signature);
        heights[5 + 2 * 60] = 90_000;
        r.step(2, 0, &[s], &heights, &solids).unwrap();
        assert_ne!(r.caches[0].signature, signature);
        assert!(!r.visible(0, [95., 25.]));
    }
    #[test]
    fn radar_echo_is_a_snapshot_not_hidden_live_tracking_and_expires() {
        let mut r = no_satellite(12, 12);
        let heights = vec![0; 144];
        let solids = vec![false; 144];
        let mut s = camera();
        s.kind = SensorKind::Radar;
        s.position = [25., 25.];
        r.step(0, 0, &[s.clone()], &heights, &solids).unwrap();
        r.sweep_contacts(
            0,
            0,
            &[ContactCandidate {
                opaque_track: 19,
                position: [85., 25.],
            }],
        )
        .unwrap();
        assert_eq!(r.echoes(0).len(), 1);
        r.step(1, 0, &[s.clone()], &heights, &solids).unwrap();
        r.sweep_contacts(
            1,
            0,
            &[ContactCandidate {
                opaque_track: 19,
                position: [85., 85.],
            }],
        )
        .unwrap();
        // Beam rescanned the old empty point, so that old echo disappears.
        assert!(r.echoes(0).is_empty());
        r.step(40, 0, &[s.clone()], &heights, &solids).unwrap();
        r.sweep_contacts(
            40,
            0,
            &[ContactCandidate {
                opaque_track: 19,
                position: [25., 85.],
            }],
        )
        .unwrap();
        assert_eq!(r.echoes(0).len(), 1);
        r.step(60, 0, &[s.clone()], &heights, &solids).unwrap();
        r.sweep_contacts(
            60,
            0,
            &[ContactCandidate {
                opaque_track: 19,
                position: [85., 85.],
            }],
        )
        .unwrap();
        assert_eq!(r.echoes(0)[0].position, [25., 85.]);
        r.step(221, 0, &[s], &heights, &solids).unwrap();
        r.sweep_contacts(221, 0, &[]).unwrap();
        assert!(r.echoes(0).is_empty());
    }
    #[test]
    fn circular_satellite_prewarm_precedes_visibility_and_team_passes_are_offset() {
        let mut r = Recon::new(30, 20, 10.).unwrap();
        let heights = vec![0; 600];
        let solids = vec![false; 600];
        assert!(r.satellite_circle(0, 0).is_some());
        assert!(r.satellite_circle(0, 1).is_none());
        r.step(0, 0, &[], &heights, &solids).unwrap();
        assert!(!r.prewarm_cells(0).is_empty());
        let cell = *r.prewarm_cells(0).last().unwrap() as usize;
        let p = [(cell % 30) as f32 * 10. + 5., (cell / 30) as f32 * 10. + 5.];
        assert!(!r.visible(0, p));
        // A whole pass must reach both top/bottom corners, not a narrow strip.
        for tick in (20..=620).step_by(20) {
            r.step(tick, 0, &[], &heights, &solids).unwrap();
        }
        assert!(r.masks(0).iter().all(|m| m & EXPLORED != 0));
        assert!(r.masks(0).iter().all(|m| m & VISIBLE == 0));
        assert!(r.satellite_circle(1800, 1).is_some());
    }
    #[test]
    fn recon_save_roundtrip_preserves_batteries_explored_and_unidentified_echoes() {
        let mut r = no_satellite(12, 12);
        let heights = vec![0; 144];
        let solids = vec![false; 144];
        let mut s = camera();
        s.kind = SensorKind::CameraTrooper;
        r.step(0, 0, &[s.clone()], &heights, &solids).unwrap();
        r.step(13, 0, &[s], &heights, &solids).unwrap();
        let encoded = serde_json::to_vec(&r).unwrap();
        let copy: Recon = serde_json::from_slice(&encoded).unwrap();
        copy.validate().unwrap();
        assert_eq!(r.masks(0), copy.masks(0));
        assert_eq!(r.battery_status(0, 0), copy.battery_status(0, 0));
    }
    #[cfg(not(target_arch = "wasm32"))]
    mod saves {
        use super::*;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        struct Dir(PathBuf);
        impl Dir {
            fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "last-world-save-check-{}-{}",
                    std::process::id(),
                    SEQ.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&p).unwrap();
                Self(p)
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        #[test]
        fn atomic_save_preserves_previous_and_ignores_interrupted_pending_files() {
            let dir = Dir::new();
            let path = dir.0.join("war.json");
            let first = serde_json::json!({"tick":41,"organisms":[1,2,3],"eggs":2});
            let second = serde_json::json!({"tick":87,"organisms":[1,3],"eggs":3});
            let r = save_atomic(&path, &first).unwrap();
            assert!(!r.previous_retained);
            std::fs::write(dir.0.join("war.json.pending-interrupted"), b"partial").unwrap();
            assert_eq!(load::<serde_json::Value>(&path).unwrap(), first);
            let r = save_atomic(&path, &second).unwrap();
            assert!(r.previous_retained);
            assert_eq!(
                load::<serde_json::Value>(previous_path(&path)).unwrap(),
                first
            );
            assert_eq!(load::<serde_json::Value>(&path).unwrap(), second);
        }
        #[test]
        fn noninteger_state_checksum_uses_saved_bytes_without_float_reformatting() {
            let dir = Dir::new();
            let path = dir.0.join("positions.json");
            let state = vec![
                [0.19999999999999998f64, 3176.183837890625],
                [1.2345678901234567, 0.000000000000000014],
            ];
            save_atomic(&path, &state).unwrap();
            let restored: Vec<[f64; 2]> = load(&path).unwrap();
            assert_eq!(restored.len(), state.len());
            assert!((restored[0][1] - state[0][1]).abs() < 0.0000001);
        }
        #[test]
        fn corrupt_current_requires_explicit_recovery_and_cannot_poison_previous() {
            let dir = Dir::new();
            let path = dir.0.join("war.json");
            save_atomic(&path, &vec![11u32, 23]).unwrap();
            save_atomic(&path, &vec![11u32, 23, 44]).unwrap();
            let mut envelope: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            envelope["payload"] = "[99,23,44]".into();
            std::fs::write(&path, serde_json::to_vec(&envelope).unwrap()).unwrap();
            assert!(load::<Vec<u32>>(&path).is_err());
            assert!(save_atomic(&path, &vec![5u32]).is_err());
            let (recovered, source) = load_with_backup::<Vec<u32>>(&path).unwrap();
            assert_eq!(recovered, vec![11, 23]);
            assert!(matches!(source, LoadSource::Previous { .. }));
            assert_eq!(
                load::<Vec<u32>>(previous_path(&path)).unwrap(),
                vec![11, 23]
            );
        }
        #[test]
        fn unknown_schema_and_invalid_v1_migration_leave_original_untouched() {
            let dir = Dir::new();
            let path = dir.0.join("legacy.json");
            let dest = dir.0.join("v2.json");
            let bytes = serde_json::to_vec(
                &serde_json::json!({"format":SAVE_FORMAT,"schema_version":1,"state":{"tick":42}}),
            )
            .unwrap();
            std::fs::write(&path, &bytes).unwrap();
            assert!(load::<serde_json::Value>(&path).is_err());
            assert!(
                migrate_v1::<serde_json::Value>(&path, &dest, |_| Err(
                    "Invalid species state".into()
                ))
                .is_err()
            );
            assert!(!dest.exists());
            migrate_v1::<serde_json::Value>(&path, &dest, |s| {
                if s["tick"] == 42 {
                    Ok(())
                } else {
                    Err("tick".into())
                }
            })
            .unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(load::<serde_json::Value>(&dest).unwrap()["tick"], 42);
        }
    }
}
