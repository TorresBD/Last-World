//! Player UI state and read-only projection. Gameplay quantities remain in
//! Campaign; outbound commands always revalidate against that authority.
use crate::{
    campaign::{Campaign, patch_at},
    campaign_support::{self, EXPLORED, VISIBLE},
    construction::{BuildKind, Resources},
    tactics::Role,
    terrain::{CW, HH, SOIL_MOISTURE_MASK, SOIL_MOISTURE_SHIFT, WW},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UiRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl UiRect {
    pub fn contains(self, p: [f32; 2]) -> bool {
        p[0] >= self.x
            && p[1] >= self.y
            && p[0] < self.x + self.width
            && p[1] < self.y + self.height
    }
    pub fn normalized(a: [f32; 2], b: [f32; 2]) -> Self {
        Self {
            x: a[0].min(b[0]),
            y: a[1].min(b[1]),
            width: (a[0] - b[0]).abs(),
            height: (a[1] - b[1]).abs(),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommandCategory {
    #[default]
    Build,
    Economy,
    Defense,
    Recon,
    Ecology,
    Naval,
}
impl CommandCategory {
    pub const ALL: [Self; 6] = [
        Self::Build,
        Self::Economy,
        Self::Defense,
        Self::Recon,
        Self::Ecology,
        Self::Naval,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Build => "Build",
            Self::Economy => "Economy",
            Self::Defense => "Defense",
            Self::Recon => "Recon",
            Self::Ecology => "Ecology",
            Self::Naval => "Naval",
        }
    }
    pub fn tools(self) -> &'static [usize] {
        match self {
            Self::Build => &[0, 1, 2, 3, 4, 14],
            Self::Economy => &[5, 6, 7, 8, 9, 29],
            Self::Defense => &[12, 13, 24, 29],
            Self::Recon => &[30, 10, 31, 32, 11, 19],
            Self::Ecology => &[17, 18, 19, 20, 21, 22, 23],
            Self::Naval => &[15, 16, 25, 26, 27, 28],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PanelKind {
    Selection,
    Ecology,
    FoodWeb,
    Logistics,
    Damage,
    Help,
    Settings,
}
impl PanelKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Selection => "Selection",
            Self::Ecology => "Ecology",
            Self::FoodWeb => "Known food web",
            Self::Logistics => "Logistics",
            Self::Damage => "Ecological damage",
            Self::Help => "Field manual",
            Self::Settings => "Settings",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Layer {
    Radar,
    Cameras,
    Roads,
    Supply,
    Resources,
    Ecology,
    Damage,
    Fauna,
    Territory,
}
impl Layer {
    pub const ALL: [Self; 9] = [
        Self::Radar,
        Self::Cameras,
        Self::Roads,
        Self::Supply,
        Self::Resources,
        Self::Ecology,
        Self::Damage,
        Self::Fauna,
        Self::Territory,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Radar => "Radar",
            Self::Cameras => "Video",
            Self::Roads => "Roads",
            Self::Supply => "Supply",
            Self::Resources => "Resources",
            Self::Ecology => "Ecology",
            Self::Damage => "Damage",
            Self::Fauna => "Fauna",
            Self::Territory => "Territory",
        }
    }
    pub fn hotkey(self) -> &'static str {
        match self {
            Self::Radar => "R",
            Self::Cameras => "C",
            Self::Roads => "O",
            Self::Supply => "L",
            Self::Resources => "U",
            Self::Ecology => "V",
            Self::Damage => "G",
            Self::Fauna => "K",
            Self::Territory => "T",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelLayout {
    pub kind: PanelKind,
    pub rect: UiRect,
    pub open: bool,
    pub pinned: bool,
    pub collapsed: bool,
    pub scroll: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    pub version: u32,
    pub viewport: [u32; 2],
    pub ui_scale_permille: u16,
    pub edge_scroll: bool,
    pub reduce_motion: bool,
    pub hints: bool,
    pub minimap_size: [u32; 2],
    pub minimap_zoom_permille: u16,
    pub minimap_center: [f32; 2],
    pub panels: Vec<PanelLayout>,
    pub layers: Vec<Layer>,
    pub seen_hints: BTreeSet<String>,
}
impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            version: 1,
            viewport: [1280, 800],
            ui_scale_permille: 1000,
            edge_scroll: false,
            reduce_motion: false,
            hints: true,
            minimap_size: [224, 160],
            minimap_zoom_permille: 1000,
            minimap_center: [0.5, 0.5],
            panels: [
                PanelKind::Selection,
                PanelKind::Ecology,
                PanelKind::FoodWeb,
                PanelKind::Logistics,
                PanelKind::Damage,
                PanelKind::Help,
                PanelKind::Settings,
            ]
            .into_iter()
            .map(|kind| PanelLayout {
                kind,
                rect: panel_default(kind),
                open: false,
                pinned: false,
                collapsed: false,
                scroll: 0,
            })
            .collect(),
            layers: vec![Layer::Radar],
            seen_hints: BTreeSet::new(),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ResourceView {
    pub name: String,
    pub icon: String,
    pub value: String,
    pub trend: String,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueItemView {
    pub id: u64,
    pub icon: String,
    pub label: String,
    pub progress_permille: u16,
    pub warning: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SelectionView {
    pub actions: Vec<(String, UiAction, bool)>,
    pub title: String,
    pub subtitle: String,
    pub portrait: String,
    pub health_permille: Option<u16>,
    pub stats: Vec<String>,
    pub queue: Vec<QueueItemView>,
    pub selected_count: usize,
    pub observed_count: usize,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MinimapMarker {
    pub position: [f32; 2],
    pub team: Option<u8>,
    pub kind: String,
    pub selected: bool,
    pub stale: bool,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MinimapView {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub markers: Vec<MinimapMarker>,
    pub camera: UiRect,
    pub satellite: Option<[f32; 3]>,
    pub zoom_permille: u16,
    pub center: [f32; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub caption: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PanelEntry {
    pub id: u64,
    pub title: String,
    pub subtitle: String,
    pub icon: String,
    pub progress_permille: Option<u16>,
    pub details: Vec<String>,
    pub action: Option<UiAction>,
    pub relations: Vec<u64>,
    pub preview: Option<PreviewImage>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PanelView {
    pub kind: PanelKind,
    pub title: String,
    pub rect: UiRect,
    pub pinned: bool,
    pub collapsed: bool,
    pub scroll: u32,
    pub lines: Vec<String>,
    pub entries: Vec<PanelEntry>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct AlertView {
    pub id: u64,
    pub priority: u8,
    pub icon: String,
    pub text: String,
    pub repeats: u32,
    pub position: Option<[f32; 2]>,
    pub created_ms: u64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct TooltipView {
    pub title: String,
    pub lines: Vec<String>,
    pub anchor: [f32; 2],
}
#[derive(Clone, Debug, PartialEq)]
pub struct WorldMarker {
    pub screen: [f32; 2],
    pub radius: f32,
    pub health_permille: Option<u16>,
    pub selected: bool,
    pub hovered: bool,
    pub team: u8,
    pub kind: String,
}
#[derive(Clone, Debug, PartialEq)]
pub struct LayerCell {
    pub rect: UiRect,
    pub color: [u8; 4],
}
#[derive(Clone, Debug, PartialEq)]
pub struct ModalView { pub title: String, pub lines: Vec<String>, pub button: String }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiPresentation {
    pub modal: Option<ModalView>,
    pub resources: Vec<ResourceView>,
    pub selection: SelectionView,
    pub minimap: MinimapView,
    pub category: CommandCategory,
    pub active_tool: Option<usize>,
    pub panels: Vec<PanelView>,
    pub alerts: Vec<AlertView>,
    pub prefs: UiPrefs,
    pub layers: Vec<Layer>,
    pub tooltip: Option<TooltipView>,
    pub selection_box: Option<UiRect>,
    pub world_marks: Vec<WorldMarker>,
    pub overlay_cells: Vec<LayerCell>,
    pub hints: Vec<String>,
    pub help_query: String,
    pub selected_species: Option<u32>,
    pub food_web_pan: [f32; 2],
    pub food_web_zoom: f32,
    pub disabled_layers: Vec<(Layer, String)>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    DismissModal,
    Loadout,
    History,
    SelectBuilders,
    SelectDefenders,
    SelectCameras,
    ChooseCategory(CommandCategory),
    ChooseTool(usize),
    OpenPanel(PanelKind),
    ClosePanel(PanelKind),
    PinPanel(PanelKind),
    CollapsePanel(PanelKind),
    ToggleLayer(Layer),
    JumpAlert(u64),
    DismissAlert(u64),
    ToggleEdgeScroll,
    ToggleHints,
    ToggleReduceMotion,
    SetUiScale(u16),
    SelectMode,
    CancelOrder,
    Stop,
    Save,
    Load,
    Speed(u32),
    Pause,
    SelectSpecies(u32),
    FocusHelpSearch,
    ClearHelpSearch,
    FoodWebZoom(i8),
    FoodWebPan([f32; 2]),
}
#[derive(Clone, Debug, PartialEq)]
pub enum UiHit {
    World,
    Blocked,
    Action(UiAction),
    Minimap(UiRect),
    MinimapResize(UiRect),
    PanelHeader(PanelKind),
    PanelBody(PanelKind),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum UiCommand {
    MoveSelected {
        ids: Vec<u64>,
        position: [f32; 2],
        queue: bool,
    },
    AttackSelected {
        ids: Vec<u64>,
        position: [f32; 2],
        target: Option<u64>,
        queue: bool,
    },
    BoardSelected {
        ids: Vec<u64>,
        transport: u64,
        queue: bool,
    },
    DisembarkSelected {
        transports: Vec<u64>,
        position: [f32; 2],
        queue: bool,
    },
    StopSelected {
        ids: Vec<u64>,
    },
    UseTool {
        tool: usize,
        position: [f32; 2],
    },
    BuildSegment {
        tool: usize,
        from: [f32; 2],
        to: [f32; 2],
        queue: bool,
    },
    CameraJump {
        position: [f32; 2],
        zoom: Option<f32>,
    },
    SelectSpecies(u32),
    CancelOrder,
    Pause,
    Speed(u32),
    Save,
    Load,
    RecruitCamera,
    CycleSpecies,
    Explode {
        position: [f32; 2],
    },
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}
#[derive(Clone, Debug)]
pub struct Selectable {
    pub id: u64,
    pub team: u8,
    pub role: Role,
    pub position: [f32; 2],
    pub visible: bool,
    pub alive: bool,
    pub health: u16,
    pub max_health: u16,
}
#[derive(Clone, Debug)]
enum Drag {
    Box {
        screen: [f32; 2],
        world: [f32; 2],
        additive: bool,
    },
    Build {
        screen: [f32; 2],
        world: [f32; 2],
        tool: usize,
    },
    Panel {
        kind: PanelKind,
        start: [f32; 2],
        rect: UiRect,
    },
    Minimap {
        rect: UiRect,
    },
    ResizeMinimap {
        start: [f32; 2],
        size: [u32; 2],
    },
    FoodWeb {
        start: [f32; 2],
        pan: [f32; 2],
    },
}
#[derive(Clone, Debug)]
struct KnownUnit {
    position: [f32; 2],
    team: u8,
    role: Role,
    health: u16,
    seen_tick: u64,
}
#[derive(Clone, Debug)]
pub struct UiState {
    pub prefs: UiPrefs,
    pub modal: Option<ModalView>,
    landing_stage: u8,
    pub selection: BTreeSet<u64>,
    pub groups: [BTreeSet<u64>; 9],
    pub bookmarks: [Option<[f32; 3]>; 4],
    pub category: CommandCategory,
    pub active_tool: Option<usize>,
    pub cursor: [f32; 2],
    pub help_query: String,
    pub help_search_focused: bool,
    pub hovered: Option<u64>,
    pub food_web_pan: [f32; 2],
    pub food_web_zoom: f32,
    pub dirty: bool,
    drag: Option<Drag>,
    hover_action: Option<UiAction>,
    last_click: Option<(u64, u64)>,
    last_group: Option<(u8, u64)>,
    alerts: Vec<AlertView>,
    next_alert: u64,
    known_units: BTreeMap<u64, KnownUnit>,
    known_cells: Vec<[u8; 4]>,
    world_seed: Option<u32>,
    last_world_tick: u64,
    last_status: String,
    last_resources: Option<(u64, Resources)>,
    resource_trends: [String; 4],
    hint_until_ms: u64,
    hint_text: String,
}
impl Default for UiState {
    fn default() -> Self {
        Self {
            prefs: UiPrefs::default(),
            modal: None,
            landing_stage: 0,
            selection: BTreeSet::new(),
            groups: std::array::from_fn(|_| BTreeSet::new()),
            bookmarks: [None; 4],
            category: CommandCategory::Build,
            active_tool: None,
            cursor: [0.; 2],
            help_query: String::new(),
            help_search_focused: false,
            hovered: None,
            food_web_pan: [0.; 2],
            food_web_zoom: 1.,
            dirty: true,
            drag: None,
            hover_action: None,
            last_click: None,
            last_group: None,
            alerts: Vec::new(),
            next_alert: 1,
            known_units: BTreeMap::new(),
            known_cells: vec![[9, 17, 14, 255]; 15000],
            world_seed: None,
            last_world_tick: 0,
            last_status: String::new(),
            last_resources: None,
            resource_trends: std::array::from_fn(|_| String::new()),
            hint_until_ms: 0,
            hint_text: String::new(),
        }
    }
}

impl UiPrefs {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1
            || self.viewport.iter().any(|v| *v == 0 || *v > 16384)
            || !(750..=2000).contains(&self.ui_scale_permille)
            || !(160..=640).contains(&self.minimap_size[0])
            || !(120..=480).contains(&self.minimap_size[1])
            || !(1000..=6000).contains(&self.minimap_zoom_permille)
            || self
                .minimap_center
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err("Invalid UI layout preferences".into());
        }
        let mut kinds = BTreeSet::new();
        for panel in &self.panels {
            let r = panel.rect;
            if !kinds.insert(panel.kind)
                || [r.x, r.y, r.width, r.height].iter().any(|v| !v.is_finite())
                || r.width < 200.
                || r.height < 100.
                || r.width > 2400.
                || r.height > 2400.
            {
                return Err("Invalid/duplicate UI panel layout".into());
            }
        }
        Ok(())
    }
}
impl UiState {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_layout(path: &Path) -> Result<Self, String> {
        if !path.exists() && !campaign_support::previous_path(path).exists() {
            return Ok(Self::default());
        }
        let (prefs, source) = campaign_support::load_with_backup::<UiPrefs>(path)?;
        prefs.validate()?;
        if matches!(source, campaign_support::LoadSource::Previous { .. }) {
            let quarantine = if path.exists() {
                Some(quarantine_ui_layout(path)?)
            } else {
                None
            };
            // With the invalid UI file safely quarantined, a fresh current does
            // not rotate or replace the verified previous settings snapshot.
            campaign_support::save_atomic(path, &prefs)?;
            crate::logging::log(
                "UI_LAYOUT_RECOVERED",
                &format!(
                    "Restored verified previous UI preferences; corrupt settings preserved at {}",
                    quarantine
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "none (current was absent)".into())
                ),
            );
        }
        Ok(Self {
            prefs,
            ..Default::default()
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save_layout(&self, path: &Path) -> Result<(), String> {
        self.prefs.validate()?;
        campaign_support::save_atomic(path, &self.prefs)?;
        Ok(())
    }
    pub fn resize_layout(&mut self, viewport: [u32; 2]) {
        if viewport[0] == 0 || viewport[1] == 0 || viewport == self.prefs.viewport {
            return;
        }
        let old = self.prefs.viewport;
        let responsive = |height: u32| ((height as f32 / 1080.).powf(0.75)).clamp(0.875, 1.7);
        let factor = responsive(viewport[1]) / responsive(old[1].max(1));
        for p in &mut self.prefs.panels {
            let right = (old[0] as f32 - p.rect.x - p.rect.width).abs() < 40.;
            p.rect.width = (p.rect.width * factor).clamp(240., 1600.);
            p.rect.height = (p.rect.height * factor).clamp(160., 1600.);
            p.rect.x = if right {
                viewport[0] as f32 - p.rect.width - 12.
            } else {
                p.rect.x * viewport[0] as f32 / old[0].max(1) as f32
            };
            p.rect.y *= viewport[1] as f32 / old[1].max(1) as f32;
            p.rect = clamp_panel(p.rect, [viewport[0] as f32, viewport[1] as f32]);
        }
        self.prefs.minimap_size = [
            (self.prefs.minimap_size[0] as f32 * factor).clamp(160., 640.) as u32,
            (self.prefs.minimap_size[1] as f32 * factor).clamp(120., 480.) as u32,
        ];
        self.prefs.viewport = viewport;
        self.dirty = true;
    }
    pub fn world_dragging(&self) -> bool {
        matches!(self.drag, Some(Drag::Box { .. } | Drag::Build { .. }))
    }
    pub fn capture_presentations(
        &self,
        c: &Campaign,
        size: [u32; 2],
        now_ms: u64,
    ) -> Vec<(String, UiPresentation)> {
        let mut output = Vec::new();
        for (name, panel, category) in [
            ("calm", None, CommandCategory::Build),
            ("build", None, CommandCategory::Economy),
            (
                "ecology",
                Some(PanelKind::Ecology),
                CommandCategory::Ecology,
            ),
            (
                "food-web",
                Some(PanelKind::FoodWeb),
                CommandCategory::Ecology,
            ),
            (
                "logistics",
                Some(PanelKind::Logistics),
                CommandCategory::Economy,
            ),
        ] {
            let mut copy = self.clone();
            for p in &mut copy.prefs.panels {
                p.open = false;
            }
            copy.category = category;
            copy.resize_layout(size);
            if let Some(kind) = panel {
                copy.panel_mut(kind).open = true;
            }
            output.push((name.into(), copy.presentation(c, size, now_ms)));
        }
        output
    }
    pub fn cancel_pointer(&mut self) {
        self.drag = None;
        self.dirty = true;
    }
    pub fn selection_box(&self) -> Option<UiRect> {
        if let Some(Drag::Box { screen, .. }) = &self.drag {
            Some(UiRect::normalized(*screen, self.cursor))
        } else {
            None
        }
    }
    pub fn panel_mut(&mut self, kind: PanelKind) -> &mut PanelLayout {
        if let Some(i) = self.prefs.panels.iter().position(|p| p.kind == kind) {
            return &mut self.prefs.panels[i];
        }
        self.prefs.panels.push(PanelLayout {
            kind,
            rect: UiRect {
                x: 880.,
                y: 90.,
                width: 360.,
                height: 450.,
            },
            open: false,
            pinned: false,
            collapsed: false,
            scroll: 0,
        });
        self.prefs.panels.last_mut().unwrap()
    }
    pub fn panel_open(&self, kind: PanelKind) -> bool {
        self.prefs.panels.iter().any(|p| p.kind == kind && p.open)
    }
    pub fn toggle_panel(&mut self, kind: PanelKind) {
        let p = self.panel_mut(kind);
        p.open = !p.open;
        self.help_search_focused = kind == PanelKind::Help && p.open;
        self.dirty = true;
    }
    pub fn notice(
        &mut self,
        key: &str,
        priority: u8,
        text: String,
        position: Option<[f32; 2]>,
        now_ms: u64,
    ) {
        if let Some(a) = self
            .alerts
            .iter_mut()
            .find(|a| a.icon == key && now_ms.saturating_sub(a.created_ms) < 20_000)
        {
            a.text = text;
            a.position = position.or(a.position);
            a.priority = a.priority.max(priority);
            a.repeats = a.repeats.saturating_add(1);
            a.created_ms = now_ms;
        } else {
            let id = self.next_alert;
            self.next_alert = self.next_alert.saturating_add(1);
            self.alerts.push(AlertView {
                id,
                priority,
                icon: key.into(),
                text,
                repeats: 1,
                position,
                created_ms: now_ms,
            });
        }
        self.alerts.sort_by_key(|a| {
            (
                std::cmp::Reverse(a.priority),
                std::cmp::Reverse(a.created_ms),
                a.id,
            )
        });
        self.alerts.truncate(8);
        self.dirty = true;
    }
    pub fn hint(&mut self, key: &str, text: &str, now_ms: u64) {
        if self.prefs.hints && self.prefs.seen_hints.insert(key.into()) {
            self.hint_text = text.into();
            self.hint_until_ms = now_ms.saturating_add(9_000);
            self.dirty = true;
        }
    }
    pub fn begin_landing(&mut self,c:&Campaign) {
        let wet=c.terrain.cells.iter().filter(|x|x.water>0).count()*100/c.terrain.cells.len();
        let low=c.terrain.cells.iter().map(|x|x.height as i32).min().unwrap_or(0);
        let high=c.terrain.cells.iter().map(|x|x.height as i32).max().unwrap_or(0);
        let climate=crate::climate::SeasonState::at_tick(0,c.seed,0);
        self.landing_stage=1;
        self.modal=Some(ModalView { title:format!("Landing survey / planet {:08X}",c.seed),
            lines:vec![
                format!("Surveyed sector: {} x {} metres. Jungle canopy, clearings and river lowlands.",WW,HH),
                format!("Surface water: {wet}% of survey cells. Relief: {}m.",(high-low)/100),
                format!("Current climate: {}. Dry / arriving rains / wet / retreating rains.",climate.season_name()),
                format!("Climate cycle: {} game minutes at 1x.",crate::climate::SEASON_CYCLE_TICKS/20/60),
                format!("Daylight: 12h of the 24h biological day; compressed to {}s daylight per {}s day at 1x.",crate::climate::DAYLIGHT_TICKS/20,crate::climate::BIOLOGICAL_DAY_TICKS/20),
                "Species, population counts, food webs and biological traits require discovery and research.".into(),
                if c.recon.satellite_circle(0,0).is_some() {"Satellite in orbit and currently overhead. Video updates only within its moving footprint.".into()}else{"Satellite in orbit; next pass pending.".into()},
                "Crew: 6 builders, 4 defenders, 1 unarmed camera trooper. The crashed ship holds your supplies.".into()
            ],button:"Continue to planning".into() });
        self.drag=None;self.help_search_focused=false;self.dirty=true;
    }
    pub fn dismiss_modal(&mut self) {
        self.drag=None;
        if self.landing_stage==1 {
            self.landing_stage=2;
            self.modal=Some(ModalView {title:"What is the build plan immediately after landing?".into(),
                lines:vec![
                    "The game stays paused. Select builders and plan roads, supply buildings and camera coverage.".into(),
                    "Builders collect and deliver real supplies when time runs. Defenders protect the landing site.".into(),
                    "Press Play / Space when setup is finished. Pause stops time but still allows planning.".into(),
                    "1x is normal speed; 2x and 4x request faster simulation. Actual speed is shown beside the request.".into(),
                    "Changing speed or closing either notice never starts time.".into()
                ],button:"Start planning (stay paused)".into()});
        }else{self.landing_stage=0;self.modal=None;}
        self.dirty=true;
    }
    pub fn activate(&mut self, action: UiAction, _camera: [f32; 3], now_ms: u64) -> Vec<UiCommand> {
        if self.modal.is_some() {
            if action==UiAction::DismissModal {self.dismiss_modal();}
            return vec![];
        }
        self.dirty = true;
        match action {
            UiAction::DismissModal => { self.dismiss_modal(); vec![] }
            UiAction::Loadout | UiAction::History => {
                let history=action==UiAction::History;
                self.modal=Some(ModalView {title:if history {"Unit event history".into()}else{"Unit loadout".into()},lines:vec![],button:"Return to command".into()});vec![]
            }
            UiAction::SelectBuilders | UiAction::SelectDefenders | UiAction::SelectCameras => {
                let role=match action {UiAction::SelectBuilders=>Role::Worker,UiAction::SelectDefenders=>Role::Soldier,_=>Role::Scout};
                self.selection=self.known_units.iter().filter(|(_,a)|a.team==0&&a.role==role&&a.health>0).map(|(&id,_)|id).collect();vec![]
            }
            UiAction::ChooseCategory(c) => {
                self.category = c;
                self.active_tool = None;
                vec![]
            }
            UiAction::ChooseTool(tool) => {
                if !self.category.tools().contains(&tool) {
                    if let Some(category) = CommandCategory::ALL
                        .into_iter()
                        .find(|c| c.tools().contains(&tool))
                    {
                        self.category = category;
                    }
                }
                self.active_tool = Some(tool);
                self.drag = None;
                vec![]
            }
            UiAction::OpenPanel(k) => {
                self.panel_mut(k).open = true;
                if k == PanelKind::Help {
                    self.help_search_focused = true;
                }
                vec![]
            }
            UiAction::ClosePanel(k) => {
                self.panel_mut(k).open = false;
                self.help_search_focused = false;
                vec![]
            }
            UiAction::PinPanel(k) => {
                let p = self.panel_mut(k);
                p.pinned = !p.pinned;
                vec![]
            }
            UiAction::CollapsePanel(k) => {
                let p = self.panel_mut(k);
                p.collapsed = !p.collapsed;
                vec![]
            }
            UiAction::ToggleLayer(layer) => {
                if layer == Layer::Territory {
                    self.notice(
                        "territory",
                        1,
                        "Territory claims are unavailable in this build.".into(),
                        None,
                        now_ms,
                    );
                    return vec![];
                }
                if let Some(i) = self.prefs.layers.iter().position(|l| *l == layer) {
                    self.prefs.layers.remove(i);
                } else {
                    self.prefs.layers.push(layer);
                }
                vec![]
            }
            UiAction::JumpAlert(id) => self
                .alerts
                .iter()
                .find(|a| a.id == id)
                .and_then(|a| a.position)
                .map(|p| {
                    vec![UiCommand::CameraJump {
                        position: p,
                        zoom: None,
                    }]
                })
                .unwrap_or_default(),
            UiAction::DismissAlert(id) => {
                self.alerts.retain(|a| a.id != id);
                vec![]
            }
            UiAction::ToggleEdgeScroll => {
                self.prefs.edge_scroll = !self.prefs.edge_scroll;
                vec![]
            }
            UiAction::ToggleHints => {
                self.prefs.hints = !self.prefs.hints;
                vec![]
            }
            UiAction::ToggleReduceMotion => {
                self.prefs.reduce_motion = !self.prefs.reduce_motion;
                vec![]
            }
            UiAction::SetUiScale(value) => {
                let value = value.clamp(750, 2000);
                let ratio = value as f32 / self.prefs.ui_scale_permille as f32;
                for p in &mut self.prefs.panels {
                    p.rect.width = (p.rect.width * ratio).clamp(240., 1600.);
                    p.rect.height = (p.rect.height * ratio).clamp(160., 1600.);
                }
                self.prefs.ui_scale_permille = value;
                vec![]
            }
            UiAction::SelectMode => {
                self.active_tool = None;
                self.drag = None;
                vec![]
            }
            UiAction::CancelOrder => vec![UiCommand::CancelOrder],
            UiAction::Stop => vec![UiCommand::StopSelected {
                ids: self.selection.iter().copied().collect(),
            }],
            UiAction::Save => vec![UiCommand::Save],
            UiAction::Load => vec![UiCommand::Load],
            UiAction::Pause => vec![UiCommand::Pause],
            UiAction::Speed(v) => vec![UiCommand::Speed(v)],
            UiAction::SelectSpecies(id) => vec![UiCommand::SelectSpecies(id)],
            UiAction::FocusHelpSearch => {
                self.help_search_focused = true;
                vec![]
            }
            UiAction::ClearHelpSearch => {
                self.help_query.clear();
                vec![]
            }
            UiAction::FoodWebZoom(delta) => {
                self.food_web_zoom =
                    (self.food_web_zoom * (delta as f32 * 0.12).exp()).clamp(0.5, 3.);
                vec![]
            }
            UiAction::FoodWebPan(delta) => {
                self.food_web_pan[0] += delta[0];
                self.food_web_pan[1] += delta[1];
                vec![]
            }
        }
    }
    pub fn group_key(
        &mut self,
        digit: u8,
        modifiers: Modifiers,
        camera: [f32; 3],
        now_ms: u64,
        visible: &[Selectable],
    ) -> Vec<UiCommand> {
        self.dirty = true;
        if !(1..=9).contains(&digit) {
            return vec![];
        }
        if modifiers.alt && digit <= 4 {
            let index = digit as usize - 1;
            if modifiers.shift {
                self.bookmarks[index] = Some(camera);
                return vec![];
            }
            return self.bookmarks[index]
                .map(|c| {
                    vec![UiCommand::CameraJump {
                        position: [c[0], c[1]],
                        zoom: Some(c[2]),
                    }]
                })
                .unwrap_or_default();
        }
        let group = &mut self.groups[digit as usize - 1];
        if modifiers.control {
            if modifiers.shift {
                group.extend(self.selection.iter().copied());
            } else {
                *group = self.selection.clone();
            }
            return vec![];
        }
        if !modifiers.shift {
            self.selection.clear();
        }
        self.selection.extend(group.iter().copied());
        let twice = self
            .last_group
            .is_some_and(|(d, t)| d == digit && now_ms.saturating_sub(t) < 350);
        self.last_group = Some((digit, now_ms));
        if twice {
            let known: Vec<_> = visible
                .iter()
                .filter(|a| a.visible && a.alive && a.team == 0 && self.selection.contains(&a.id))
                .collect();
            if !known.is_empty() {
                let n = known.len() as f32;
                return vec![UiCommand::CameraJump {
                    position: [
                        known.iter().map(|a| a.position[0]).sum::<f32>() / n,
                        known.iter().map(|a| a.position[1]).sum::<f32>() / n,
                    ],
                    zoom: None,
                }];
            }
        }
        vec![]
    }
    pub fn escape(&mut self) {
        if self.modal.is_some() {self.dismiss_modal(); return;}
        self.drag = None;
        if self.active_tool.take().is_none() {
            if let Some(p) = self
                .prefs
                .panels
                .iter_mut()
                .rev()
                .find(|p| p.open && !p.pinned)
            {
                p.open = false;
            } else {
                self.selection.clear();
            }
        }
        self.help_search_focused = false;
        self.dirty = true;
    }
    pub fn text_input(&mut self, text: &str) {
        if self.help_search_focused {
            for ch in text.chars().filter(|c| !c.is_control()) {
                if self.help_query.chars().count() < 64 {
                    self.help_query.push(ch);
                }
            }
            self.dirty = true;
        }
    }
    pub fn backspace(&mut self) {
        if self.help_search_focused {
            self.help_query.pop();
            self.dirty = true;
        }
    }
    pub fn pointer_down(
        &mut self,
        hit: UiHit,
        screen: [f32; 2],
        world: [f32; 2],
        right: bool,
        modifiers: Modifiers,
        camera: [f32; 3],
        canvas: [f32; 2],
        visible: &[Selectable],
        now_ms: u64,
    ) -> Vec<UiCommand> {
        self.cursor = screen;
        self.drag = None;
        self.dirty = true;
        if self.modal.is_some() && hit!=UiHit::Action(UiAction::DismissModal) {return vec![];}
        if !matches!(
            &hit,
            UiHit::Action(
                UiAction::FocusHelpSearch
                    | UiAction::ClearHelpSearch
                    | UiAction::OpenPanel(PanelKind::Help)
            )
        ) {
            self.help_search_focused = false;
        }
        match hit {
            UiHit::Action(action) => {
                if right {
                    vec![]
                } else {
                    self.activate(action, camera, now_ms)
                }
            }
            UiHit::Blocked => vec![],
            UiHit::PanelBody(PanelKind::FoodWeb) => {
                if !right {
                    self.drag = Some(Drag::FoodWeb {
                        start: screen,
                        pan: self.food_web_pan,
                    });
                }
                vec![]
            }
            UiHit::PanelBody(_) => vec![],
            UiHit::PanelHeader(kind) => {
                if !right {
                    let rect = clamp_panel(self.panel_mut(kind).rect, canvas);
                    self.drag = Some(Drag::Panel {
                        kind,
                        start: screen,
                        rect,
                    });
                }
                vec![]
            }
            UiHit::Minimap(rect) => {
                if !right {
                    self.drag = Some(Drag::Minimap { rect });
                    vec![self.minimap_jump(screen, rect)]
                } else {
                    vec![]
                }
            }
            UiHit::MinimapResize(_) => {
                if !right {
                    self.drag = Some(Drag::ResizeMinimap {
                        start: screen,
                        size: self.prefs.minimap_size,
                    });
                }
                vec![]
            }
            UiHit::World => {
                if !in_world(world) {
                    return vec![];
                }
                if right {
                    if self.active_tool.take().is_some() {
                        return vec![];
                    }
                    if self.selection.is_empty() {
                        self.hint(
                            "select_first",
                            "Box-select troops in video, then right-click to order them.",
                            now_ms,
                        );
                        return vec![];
                    }
                    self.hint(
                        "shift_orders",
                        "Hold Shift when ordering to queue routes. Ctrl+1..9 stores a group.",
                        now_ms,
                    );
                    let ids = self.selection.iter().copied().collect();
                    let target = nearest(visible, world, 12. / camera[2].max(0.12), false);
                    return match target {
                        Some(a) if a.team != 0 => vec![UiCommand::AttackSelected {
                            ids,
                            position: world,
                            target: Some(a.id),
                            queue: modifiers.shift,
                        }],
                        Some(a) if a.role == Role::Transport => vec![UiCommand::BoardSelected {
                            ids,
                            transport: a.id,
                            queue: modifiers.shift,
                        }],
                        _ => vec![UiCommand::MoveSelected {
                            ids,
                            position: world,
                            queue: modifiers.shift,
                        }],
                    };
                }
                if let Some(tool) = self.active_tool {
                    self.drag = Some(Drag::Build {
                        screen,
                        world,
                        tool,
                    });
                } else {
                    self.drag = Some(Drag::Box {
                        screen,
                        world,
                        additive: modifiers.shift,
                    });
                }
                vec![]
            }
        }
    }
    pub fn pointer_move(
        &mut self,
        hit: UiHit,
        screen: [f32; 2],
        world: [f32; 2],
        camera: [f32; 3],
        canvas: [f32; 2],
        visible: &[Selectable],
    ) -> Vec<UiCommand> {
        self.cursor = screen;
        let previous_hover = self.hovered;
        let previous_action = self.hover_action.clone();
        self.hovered = if matches!(hit, UiHit::World) {
            nearest(visible, world, 10. / camera[2].max(0.12), false).map(|a| a.id)
        } else {
            None
        };
        self.hover_action = if let UiHit::Action(action) = hit {
            Some(action)
        } else {
            None
        };
        if previous_hover != self.hovered || previous_action != self.hover_action {
            self.dirty = true;
        }
        let mut command = Vec::new();
        match self.drag.clone() {
            Some(Drag::Panel { kind, start, rect }) => {
                let panel = self.panel_mut(kind);
                panel.rect.x =
                    (rect.x + screen[0] - start[0]).clamp(0., (canvas[0] - rect.width).max(0.));
                panel.rect.y =
                    (rect.y + screen[1] - start[1]).clamp(48., (canvas[1] - 56.).max(48.));
                self.dirty = true;
            }
            Some(Drag::Minimap { rect }) => command.push(self.minimap_jump(screen, rect)),
            Some(Drag::ResizeMinimap { start, size }) => {
                self.prefs.minimap_size = [
                    (size[0] as f32 + screen[0] - start[0]).clamp(160., 640.) as u32,
                    (size[1] as f32 + screen[1] - start[1]).clamp(120., 480.) as u32,
                ];
                self.dirty = true;
            }
            Some(Drag::FoodWeb { start, pan }) => {
                self.food_web_pan = [pan[0] + screen[0] - start[0], pan[1] + screen[1] - start[1]];
                self.dirty = true;
            }
            Some(_) => self.dirty = true,
            None => {}
        }
        command
    }
    pub fn pointer_up(
        &mut self,
        hit: UiHit,
        screen: [f32; 2],
        world: [f32; 2],
        modifiers: Modifiers,
        camera: [f32; 3],
        canvas: [f32; 2],
        visible: &[Selectable],
        now_ms: u64,
    ) -> Vec<UiCommand> {
        self.cursor = screen;
        self.dirty = true;
        let Some(drag) = self.drag.take() else {
            return vec![];
        };
        match drag {
            Drag::Build {
                screen: start,
                world: from,
                tool,
            } => {
                if !matches!(hit, UiHit::World) || !in_world(world) {
                    return vec![];
                }
                let is_segment = matches!(tool, 0 | 1 | 15);
                let selected = || self.selection.iter().copied().collect();
                let command = if tool == 24 {
                    UiCommand::MoveSelected {
                        ids: selected(),
                        position: world,
                        queue: modifiers.shift,
                    }
                } else if tool == 27 {
                    let Some(transport) = visible
                        .iter()
                        .filter(|a| {
                            a.visible
                                && a.alive
                                && a.team == 0
                                && a.role == Role::Transport
                                && distance(a.position, world) <= 12. / camera[2].max(0.12)
                        })
                        .min_by(|a, b| {
                            distance(a.position, world).total_cmp(&distance(b.position, world))
                        })
                    else {
                        self.notice(
                            "boarding",
                            1,
                            "Choose a friendly transport currently in video".into(),
                            None,
                            now_ms,
                        );
                        return vec![];
                    };
                    UiCommand::BoardSelected {
                        ids: selected(),
                        transport: transport.id,
                        queue: modifiers.shift,
                    }
                } else if tool == 28 {
                    UiCommand::DisembarkSelected {
                        transports: selected(),
                        position: world,
                        queue: modifiers.shift,
                    }
                } else if tool == 29 {
                    UiCommand::CancelOrder
                } else if is_segment {
                    let to = if (screen[0] - start[0]).hypot(screen[1] - start[1]) < 5. {
                        [from[0] + 32., from[1]]
                    } else {
                        world
                    };
                    UiCommand::BuildSegment {
                        tool,
                        from,
                        to,
                        queue: modifiers.shift,
                    }
                } else {
                    UiCommand::UseTool {
                        tool,
                        position: world,
                    }
                };
                if !modifiers.shift {
                    self.active_tool = None;
                }
                vec![command]
            }
            Drag::Box {
                screen: start,
                world: from,
                additive,
            } => {
                if !matches!(hit, UiHit::World) {
                    return vec![];
                }
                if !additive {
                    self.selection.clear();
                }
                if (screen[0] - start[0]).hypot(screen[1] - start[1]) > 6. {
                    let rect = UiRect::normalized(from, world);
                    for a in visible.iter().filter(|a| {
                        a.visible && a.alive && a.team == 0 && rect.contains(a.position)
                    }) {
                        self.selection.insert(a.id);
                    }
                    self.last_click = None;
                } else if let Some(a) = nearest(visible, world, 12. / camera[2].max(0.12), true) {
                    let double = self
                        .last_click
                        .is_some_and(|(id, time)| id == a.id && now_ms.saturating_sub(time) < 350);
                    if double {
                        for other in visible.iter().filter(|other| {
                            other.visible
                                && other.alive
                                && other.team == 0
                                && other.role == a.role
                                && on_screen(other.position, camera, canvas)
                        }) {
                            self.selection.insert(other.id);
                        }
                    } else if additive && self.selection.contains(&a.id) {
                        self.selection.remove(&a.id);
                    } else {
                        self.selection.insert(a.id);
                    }
                    self.last_click = Some((a.id, now_ms));
                }
                if !self.selection.is_empty() {
                    self.hint("select_orders","Right-click moves selected troops; click an enemy to attack. Shift queues orders.",now_ms);
                }
                vec![]
            }
            _ => vec![],
        }
    }
    fn minimap_jump(&self, screen: [f32; 2], rect: UiRect) -> UiCommand {
        let zoom = self.prefs.minimap_zoom_permille as f32 / 1000.;
        let q = [
            ((screen[0] - rect.x) / rect.width.max(1.)).clamp(0., 1.),
            ((screen[1] - rect.y) / rect.height.max(1.)).clamp(0., 1.),
        ];
        let p = [
            (self.prefs.minimap_center[0] + (q[0] - 0.5) / zoom).clamp(0., 1.),
            (self.prefs.minimap_center[1] + (q[1] - 0.5) / zoom).clamp(0., 1.),
        ];
        UiCommand::CameraJump {
            position: [p[0] * (WW as f32 - 1.), p[1] * (HH as f32 - 1.)],
            zoom: None,
        }
    }
    pub fn wheel(&mut self, hit: UiHit, dy: f32) -> bool {
        if self.modal.is_some() {return true;}
        match hit {
            UiHit::Minimap(rect) | UiHit::MinimapResize(rect) => {
                let old = self.prefs.minimap_zoom_permille as f32 / 1000.;
                let q = [
                    ((self.cursor[0] - rect.x) / rect.width.max(1.)).clamp(0., 1.) - 0.5,
                    ((self.cursor[1] - rect.y) / rect.height.max(1.)).clamp(0., 1.) - 0.5,
                ];
                let anchor = [
                    self.prefs.minimap_center[0] + q[0] / old,
                    self.prefs.minimap_center[1] + q[1] / old,
                ];
                self.prefs.minimap_zoom_permille = (self.prefs.minimap_zoom_permille as f32
                    * (dy * 0.15).exp())
                .clamp(1000., 6000.) as u16;
                let new = self.prefs.minimap_zoom_permille as f32 / 1000.;
                self.prefs.minimap_center = [
                    (anchor[0] - q[0] / new).clamp(0., 1.),
                    (anchor[1] - q[1] / new).clamp(0., 1.),
                ];
                self.dirty = true;
                true
            }
            UiHit::PanelBody(kind) => {
                if kind == PanelKind::FoodWeb {
                    self.food_web_zoom = (self.food_web_zoom * (dy * 0.12).exp()).clamp(0.5, 3.);
                } else {
                    let p = self.panel_mut(kind);
                    p.scroll = (p.scroll as i64 - (dy * 3.) as i64).max(0).min(100_000) as u32;
                }
                self.dirty = true;
                true
            }
            UiHit::Action(_) | UiHit::Blocked | UiHit::PanelHeader(_) => true,
            UiHit::World => false,
        }
    }
    pub fn edge_pan(&self, screen: [f32; 2], canvas: [f32; 2]) -> [f32; 2] {
        if !self.prefs.edge_scroll || self.drag.is_some() {
            return [0.; 2];
        }
        let edge = 12.;
        [
            (if screen[0] < edge { -1. } else { 0. })
                + (if screen[0] > canvas[0] - edge { 1. } else { 0. }),
            (if screen[1] < edge { -1. } else { 0. })
                + (if screen[1] > canvas[1] - edge { 1. } else { 0. }),
        ]
    }
    pub fn observe_world(&mut self, c: &Campaign, now_ms: u64) {
        if self.world_seed != Some(c.seed) || c.processed_tick < self.last_world_tick {
            self.selection.clear();
            self.groups = std::array::from_fn(|_| BTreeSet::new());
            self.known_units.clear();
            for i in 0..15000 {
                self.known_cells[i]=c.visual_memory.as_ref().map(|m|m.color(i)).unwrap_or([9,17,14,255]);
            }
            self.alerts.clear();
            self.bookmarks = [None; 4];
            self.last_click = None;
            self.last_group = None;
            self.drag = None;
            self.world_seed = Some(c.seed);
            self.last_resources = None;
            self.last_status.clear();
        }
        let mut notices = Vec::new();
        for a in &c.tactics.agents {
            let p = a.position();
            if !c.recon.visible(0, p) {
                continue;
            }
            if a.team == 0 {
                if let Some(old) = self.known_units.get(&a.id) {
                    if old.health > a.health.saturating_add(15) {
                        notices.push((
                            format!("combat/{}", patch_at(p)),
                            3,
                            "Troops under fire in video".to_owned(),
                            Some(p),
                        ));
                    }
                }
            }
            if a.alive() {
                self.known_units.insert(
                    a.id,
                    KnownUnit {
                        position: p,
                        team: a.team,
                        role: a.role,
                        health: a.health,
                        seen_tick: c.processed_tick,
                    },
                );
            } else {
                self.known_units.remove(&a.id);
                self.selection.remove(&a.id);
            }
        }
        for (key, priority, text, position) in notices {
            self.notice(&key, priority, text, position, now_ms);
        }
        for i in 0..15000 {
            if let Some(memory)=&c.visual_memory {self.known_cells[i]=memory.color(i); continue;}
            if c.recon.masks(0)[i] & VISIBLE != 0 {
                let x = i % 150 * 16 + 8;
                let y = i / 150 * 16 + 8;
                let pixel = (y * (WW / 2) + x) * 4;
                if pixel + 3 < c.terrain.rgba.len() {
                    self.known_cells[i] = [
                        c.terrain.rgba[pixel],
                        c.terrain.rgba[pixel + 1],
                        c.terrain.rgba[pixel + 2],
                        255,
                    ];
                }
            }
        }
        if self.last_status != c.status {
            let lower = c.status.to_ascii_lowercase();
            let warning = lower.contains("insufficient")
                || lower.contains("blocked")
                || lower.contains("shortage")
                || lower.contains("command:")
                || lower.contains("unavailable")
                || lower.contains("cannot");
            if !c.status.is_empty() {
                self.notice(
                    if warning { "supply" } else { "feedback" },
                    if warning { 2 } else { 0 },
                    c.status.clone(),
                    None,
                    now_ms,
                );
            }
            self.last_status = c.status.clone();
        }
        if let Some((tick, previous)) = self.last_resources {
            let delta = c.processed_tick.saturating_sub(tick);
            if delta >= 40 {
                let now = resource_values(c.construction.stores[0]);
                let before = resource_values(previous);
                for i in 0..4 {
                    let rate = (now[i] as i128 - before[i] as i128) * 20 / delta as i128;
                    self.resource_trends[i] = if rate > 0 {
                        format!("+{rate}/s")
                    } else if rate < 0 {
                        format!("{rate}/s")
                    } else {
                        String::new()
                    };
                }
                self.last_resources = Some((c.processed_tick, c.construction.stores[0]));
            }
        } else {
            self.last_resources = Some((c.processed_tick, c.construction.stores[0]));
        }
        self.alerts.retain(|a| {
            now_ms.saturating_sub(a.created_ms) < if a.priority >= 3 { 60_000 } else { 25_000 }
        });
        self.last_world_tick = c.processed_tick;
        self.hint("welcome","F4 Road · F6 Trench · F7 Camera. Box-select troops; right-click orders them. E Ecology.",now_ms);
    }
    pub fn presentation(&mut self, c: &Campaign, canvas: [u32; 2], now_ms: u64) -> UiPresentation {
        self.observe_world(c, now_ms);
        let canvas_f = [canvas[0] as f32, canvas[1] as f32];
        let mut out = UiPresentation {
            modal: self.modal.clone(),
            category: self.category,
            active_tool: self.active_tool,
            prefs: self.prefs.clone(),
            layers: self.prefs.layers.clone(),
            selection_box: self.selection_box(),
            alerts: self.alerts.clone(),
            help_query: self.help_query.clone(),
            selected_species: c.selected_species,
            food_web_pan: self.food_web_pan,
            food_web_zoom: self.food_web_zoom,
            disabled_layers: vec![(
                Layer::Territory,
                "Territory claims are not recorded in this build".into(),
            )],
            ..Default::default()
        };
        let resources = c.construction.stores[0];
        for (i, (name, icon)) in [
            ("Metal", "mine"),
            ("Timber", "mill"),
            ("Food", "farm"),
            ("Power", "solar"),
        ]
        .iter()
        .enumerate()
        {
            let value = resource_values(resources)[i];
            out.resources.push(ResourceView {
                name: (*name).into(),
                icon: (*icon).into(),
                value: value.to_string(),
                trend: self.resource_trends[i].clone(),
                warning: (value < 40).then(|| "Reserve is low; inspect logistics".into()),
            });
        }
        let selected: Vec<_> = c
            .tactics
            .agents
            .iter()
            .filter(|a| {
                self.selection.contains(&a.id) && a.alive() && c.recon.visible(0, a.position())
            })
            .collect();
        let roles: BTreeSet<_> = selected.iter().map(|a| a.role).collect();
        let health = selected.iter().map(|a| a.health as u64).sum::<u64>();
        let max = selected
            .iter()
            .map(|a| {
                if matches!(a.role, Role::Boat | Role::Transport) {
                    600u64
                } else {
                    100
                }
            })
            .sum::<u64>();
        out.selection = SelectionView {
            actions: vec![("Loadout".into(),UiAction::Loadout,!selected.is_empty()),("History".into(),UiAction::History,!selected.is_empty()),("Hold".into(),UiAction::Stop,!selected.is_empty()),
                ("Build".into(),UiAction::ChooseCategory(CommandCategory::Build),roles.contains(&Role::Worker)),
                ("Recon".into(),UiAction::ChooseCategory(CommandCategory::Recon),roles.contains(&Role::Scout)||roles.contains(&Role::Worker))],
            title: if self.selection.is_empty() {
                "Landing command".into()
            } else if let [a] = selected.as_slice() {
                format!("{} #{}",role_name(a.role),a.id)
            } else {
                format!("{} selected", self.selection.len())
            },
            subtitle: {
                let groups = self
                    .groups
                    .iter()
                    .enumerate()
                    .filter(|(_, g)| !g.is_empty() && **g == self.selection)
                    .map(|(i, _)| format!("G{}", i + 1))
                    .collect::<Vec<_>>()
                    .join(" · ");
                if groups.is_empty() {
                    if self.selection.is_empty() {"Select crew below; plan, then press Play.".into()} else {format!("{} currently in video", selected.len())}
                } else {
                    format!("{groups} · {} currently in video", selected.len())
                }
            },
            portrait: roles
                .first()
                .map(|r| role_icon(*r).to_owned())
                .unwrap_or_else(|| "ship".into()),
            health_permille: (max > 0).then(|| (health * 1000 / max) as u16),
            stats: {
                let mut stats = vec![];
                if let [a] = selected.as_slice() {
                    let class = match a.role {
                        Role::Soldier => "Soldier",
                        Role::Worker => "Worker",
                        Role::Scout => "Camera trooper",
                        Role::Boat => "Gunboat",
                        Role::Transport => "Transport",
                    };
                    stats.push(format!("{class} · {} health", a.health));
                    if matches!(a.role, Role::Soldier | Role::Boat) {
                        stats.push(format!("{} rounds carried", a.ammunition));
                    }
                    if a.role == Role::Scout {
                        if let Some(battery) = c.recon.battery_status(0, a.id + 2_000_000_000) {
                            stats.push(format!(
                                "Battery {}% · recharges at ship",
                                battery.remaining_ticks.saturating_mul(100)
                                    / battery.capacity_ticks.max(1)
                            ));
                        }
                    }
                    if a.role == Role::Transport {
                        stats.push(format!(
                            "{} / 20 passengers",
                            c.tactics.manifests.get(&a.id).map_or(0, |m| m.len())
                        ));
                    }
                }
                if let [a]=selected.as_slice() {
                    stats.insert(0,format!("Task: {}",unit_task(c,a)));
                }else{
                    for role in &roles {
                        stats.push(format!("{} {}",selected.iter().filter(|a|a.role==*role).count(),role_name(*role)));
                    }
                }
                stats.push("Right-click: move / attack / board".into());
                stats.push("Shift: queue | Ctrl+1..9: store group".into());
                stats
            },
            queue: c
                .construction
                .jobs
                .iter()
                .filter(|j| {
                    j.order.team == 0
                        && !matches!(
                            j.phase,
                            crate::construction::JobPhase::Complete
                                | crate::construction::JobPhase::Cancelled
                        )
                })
                .take(8)
                .map(|j| QueueItemView {
                    id: j.id,
                    icon: j.order.kind.atlas_kind().into(),
                    label: j.order.kind.name().into(),
                    progress_permille: (j.work_done as u64 * 1000 / j.work_required.max(1) as u64)
                        as u16,
                    warning: j.blocked_reason.clone(),
                })
                .collect(),
            selected_count: self.selection.len(),
            observed_count: selected.len(),
        };
        if self.selection.is_empty() {
            out.selection.actions=vec![("Builders".into(),UiAction::SelectBuilders,true),("Defenders".into(),UiAction::SelectDefenders,true),("Camera crew".into(),UiAction::SelectCameras,true)];
        }
        let unit_orders = selected
            .iter()
            .flat_map(|a| {
                c.unit_order_queues
                    .get(&a.id)
                    .into_iter()
                    .flat_map(move |q| {
                        q.iter().enumerate().map(move |(i, o)| {
                            use crate::campaign_commands::UnitOrderKind;
                            let (icon, label) = match o.kind {
                                UnitOrderKind::Move { .. } => ("rally", "Move"),
                                UnitOrderKind::Attack { .. } => ("turret", "Attack"),
                                UnitOrderKind::Board { .. } => ("transport", "Embark"),
                                UnitOrderKind::Disembark { .. } => ("dock", "Disembark"),
                                UnitOrderKind::Hold => ("wall", "Hold"),
                            };
                            QueueItemView {
                                id: (1u64 << 63) | (a.id << 8) | i as u64,
                                icon: icon.into(),
                                label: label.into(),
                                progress_permille: 0,
                                warning: None,
                            }
                        })
                    })
            })
            .take(4)
            .collect::<Vec<_>>();
        if !unit_orders.is_empty() {
            out.selection.queue.splice(0..0, unit_orders);
            out.selection.queue.truncate(8);
        }
        let mut map = MinimapView {
            width: 150,
            height: 100,
            rgba: Vec::with_capacity(60_000),
            zoom_permille: self.prefs.minimap_zoom_permille,
            center: self.prefs.minimap_center,
            ..Default::default()
        };
        for (i, color) in self.known_cells.iter().enumerate() {
            let bits = c.recon.masks(0)[i];
            let mut rgba = *color;
            if bits & VISIBLE == 0 {
                if bits & EXPLORED == 0 && c.visual_memory.is_none() {
                    rgba = [7, 12, 10, 255];
                } else {
                    for v in &mut rgba[..3] {
                        *v = (*v as u16 * 72 / 100) as u8;
                    }
                }
            }
            map.rgba.extend_from_slice(&rgba);
        }
        for (&id, a) in &self.known_units {
            let current = c.recon.visible(0, a.position) && a.seen_tick == c.processed_tick;
            if !current && c.processed_tick.saturating_sub(a.seen_tick) > 600 {
                continue;
            }
            map.markers.push(MinimapMarker {
                position: [a.position[0] / WW as f32, a.position[1] / HH as f32],
                team: Some(a.team),
                kind: role_icon(a.role).into(),
                selected: self.selection.contains(&id),
                stale: !current,
            });
        }
        for e in c.recon.echoes(0) {
            map.markers.push(MinimapMarker {
                position: [e.position[0] / WW as f32, e.position[1] / HH as f32],
                team: None,
                kind: "radar_contact".into(),
                selected: false,
                stale: true,
            });
        }
        for alert in &self.alerts {
            if let Some(p) = alert.position {
                map.markers.push(MinimapMarker {
                    position: [p[0] / WW as f32, p[1] / HH as f32],
                    team: None,
                    kind: "alert".into(),
                    selected: false,
                    stale: false,
                });
            }
        }
        map.camera = UiRect {
            x: (c.camera[0] - canvas[0] as f32 / c.camera[2] * 0.5) / WW as f32,
            y: (c.camera[1] - canvas[1] as f32 / c.camera[2] * 0.5) / HH as f32,
            width: canvas[0] as f32 / c.camera[2] / WW as f32,
            height: canvas[1] as f32 / c.camera[2] / HH as f32,
        };
        map.satellite = c.recon.satellite_circle(c.processed_tick, 0).map(|s| {
            [
                s.center[0] / WW as f32,
                s.center[1] / HH as f32,
                s.radius_m / WW as f32,
            ]
        });
        out.minimap = map;
        for p in self.prefs.panels.iter().filter(|p| p.open) {
            out.panels.push(panel_projection(c, p, &self.help_query));
        }
        for p in &mut out.panels {
            p.rect = clamp_panel(p.rect, canvas_f);
        }
        for a in c.tactics.agents.iter().filter(|a| {
            a.alive()
                && c.recon.visible(0, a.position())
                && (self.selection.contains(&a.id) || self.hovered == Some(a.id))
        }) {
            if on_screen(a.position(), c.camera, canvas_f) {
                let screen = world_to_screen(a.position(), c.camera, canvas_f);
                out.world_marks.push(WorldMarker {
                    screen,
                    radius: 9.,
                    health_permille: Some(
                        (a.health as u32 * 1000
                            / if matches!(a.role, Role::Boat | Role::Transport) {
                                600
                            } else {
                                100
                            }) as u16,
                    ),
                    selected: self.selection.contains(&a.id),
                    hovered: self.hovered == Some(a.id),
                    team: a.team,
                    kind: role_icon(a.role).into(),
                });
            }
        }
        if self.prefs.layers.contains(&Layer::Fauna) {
            let animals = c
                .ecology
                .cohorts
                .iter()
                .filter(|a| a.count == 1 && a.surface_observable && a.location_height_mm >= 0)
                .map(|a| (a.id, a))
                .collect::<BTreeMap<_, _>>();
            for (id, detail) in &c.detailed {
                let Some(animal) = animals.get(id) else {
                    continue;
                };
                if !c.recon.visible(0, detail.position)
                    || c.ecology.observed_inspect(0, animal.species_id).is_none()
                {
                    continue;
                }
                out.minimap.markers.push(MinimapMarker {
                    position: [
                        detail.position[0] / WW as f32,
                        detail.position[1] / HH as f32,
                    ],
                    team: None,
                    kind: "fauna".into(),
                    selected: false,
                    stale: false,
                });
                if on_screen(detail.position, c.camera, canvas_f) {
                    out.world_marks.push(WorldMarker {
                        screen: world_to_screen(detail.position, c.camera, canvas_f),
                        radius: 8.,
                        health_permille: None,
                        selected: false,
                        hovered: true,
                        team: 2,
                        kind: "fauna".into(),
                    });
                }
            }
        }
        let layer = self
            .prefs
            .layers
            .iter()
            .rev()
            .find(|l| !matches!(l, Layer::Radar | Layer::Fauna | Layer::Territory))
            .copied();
        if let Some(layer) = layer {
            for i in 0..15000 {
                if c.recon.masks(0)[i] & VISIBLE == 0 {
                    continue;
                }
                let world = [(i % 150) as f32 * 32., (i / 150) as f32 * 32.];
                let screen = world_to_screen(world, c.camera, canvas_f);
                let size = 32. * c.camera[2];
                if screen[0] + size < 0.
                    || screen[1] + size < 0.
                    || screen[0] > canvas_f[0]
                    || screen[1] > canvas_f[1]
                {
                    continue;
                }
                let cell = (i / 150 * 4 + 2) * CW + i % 150 * 4 + 2;
                let p = patch_at(world);
                let tint = match layer {
                    Layer::Cameras => Some([85, 170, 158, 32]),
                    Layer::Roads if c.terrain.cells[cell].flags & 1 != 0 => {
                        Some([232, 187, 81, 90])
                    }
                    Layer::Supply
                        if c.construction.buildings.iter().any(|b| {
                            b.team == 0
                                && b.health > 0
                                && matches!(b.kind, BuildKind::Depot | BuildKind::Mine)
                                && distance(world, b.position) < b.size_meters.max(32.)
                        }) =>
                    {
                        Some([90, 170, 215, 45])
                    }
                    Layer::Resources
                        if c.construction
                            .deposits
                            .iter()
                            .any(|d| distance(world, d.position) < 32.) =>
                    {
                        Some([210, 171, 73, 100])
                    }
                    Layer::Ecology => {
                        let m = ((c.terrain.cells[cell].flags & SOIL_MOISTURE_MASK)
                            >> SOIL_MOISTURE_SHIFT) as u16;
                        Some([
                            80,
                            (90 + c.ecology.plant_fraction_permille(p as u32) / 10).min(200) as u8,
                            (70 + m / 20).min(150) as u8,
                            35,
                        ])
                    }
                    Layer::Damage if c.terrain.cells[cell].flags & 4 != 0 => {
                        Some([215, 100, 70, 60])
                    }
                    _ => None,
                };
                if let Some(color) = tint {
                    out.overlay_cells.push(LayerCell {
                        rect: UiRect {
                            x: screen[0],
                            y: screen[1],
                            width: size,
                            height: size,
                        },
                        color,
                    });
                }
            }
        }
        if let Some(tool) = self
            .hover_action
            .as_ref()
            .and_then(|a| {
                if let UiAction::ChooseTool(t) = a {
                    Some(*t)
                } else {
                    None
                }
            })
            .or(self.active_tool)
        {
            let mut lines = Vec::new();
            if let Some(hotkey) = match tool {
                0 => Some("F4"),
                1 => Some("F6"),
                10 => Some("F7"),
                _ => None,
            } {
                lines.push(format!("Hotkey: {hotkey}"));
            }
            if let Some(kind) = build_kind(tool) {
                let cost = kind.base_cost();
                lines.push(format!(
                    "{} metal | {} timber | {} food | {} power",
                    cost.metal, cost.timber, cost.rations, cost.power
                ));
                if matches!(kind, BuildKind::Road | BuildKind::Trench) {
                    lines.push("Cost is per segment; final length is priced when ordered.".into());
                }
                if let Some((height, range))=kind.camera_spec() {
                    lines.push(format!("{}m high | {}m optical range | blocked by terrain and canopy",height/1000,range));
                    if kind==BuildKind::MountedCamera { lines.push("Snaps to an owned completed wall/building; requires its host.".into()); }
                }
                lines.push("A worker collects materials, travels and performs the work.".into());
            } else {
                lines.push(match tool{20=>"Capture requires observed reachable wildlife, a handler and a suitable pen.",21=>"Research requires a delivered specimen, care and a research building.",22=>"Breeding requires researched compatible specimens and real care.",23=>"A handler collects and releases an existing captive.",25|26=>"Launch requires a completed dock, paid materials and sufficient depth.",27|28=>"Use an actual nearby transport and reachable shoreline.",_=>"Click the world to use this command."}.into());
            }
            out.tooltip = Some(TooltipView {
                title: tool_label(tool).into(),
                lines,
                anchor: self.cursor,
            });
        }
        if self.prefs.hints && now_ms < self.hint_until_ms {
            out.hints.push(self.hint_text.clone());
        }
        if let Some(modal)=out.modal.as_mut() {
            if modal.lines.is_empty() {
                for a in &selected {
                    if a.team!=0 {continue;}
                    modal.lines.push(format!("{} #{} / {}",role_name(a.role),a.id,unit_task(c,a)));
                    if modal.title=="Unit event history" {
                        if let Some(events)=c.unit_history.get(&a.id) {
                            for (tick,text) in events.iter().rev().take(6) {modal.lines.push(format!("t{tick}: {text}"));}
                        }else{modal.lines.push("No recorded events for this unit yet.".into());}
                    }else{
                        modal.lines.push(format!("Health: {} / {}. Ammunition carried: {}.",a.health,if matches!(a.role,Role::Boat|Role::Transport){600}else{100},a.ammunition));
                        modal.lines.push(match a.role {
                            Role::Worker=>"Construction tools; collects, hauls and builds real materials. No weapon.",
                            Role::Soldier=>"Rifle; move, hold, attack visible enemies and board transports. No camera.",
                            Role::Scout=>"Unarmed camera; move to extend video, return to ship to recharge.",
                            Role::Boat=>"Armed gunboat; requires navigable water.",
                            Role::Transport=>"Troop transport; board and disembark at reachable shore."
                        }.into());
                    }
                    if modal.lines.len()>10 {break;}
                }
                if modal.lines.is_empty(){modal.lines.push("Select one of your units in current video to inspect its real state.".into());}
            }
        }
        self.dirty = false;
        out
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn quarantine_ui_layout(path: &Path) -> Result<std::path::PathBuf, String> {
    use std::{
        fs::OpenOptions,
        io,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    for _ in 0..8 {
        let mut name = path.as_os_str().to_owned();
        name.push(format!(
            ".corrupt-{}-{timestamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let quarantine = std::path::PathBuf::from(name);
        let mut target = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&quarantine)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Preserving corrupt UI preferences: {error}")),
        };
        let copied = (|| {
            let mut source = std::fs::File::open(path)?;
            io::copy(&mut source, &mut target)?;
            target.sync_all()
        })();
        drop(target);
        if let Err(error) = copied {
            let _ = std::fs::remove_file(&quarantine);
            return Err(format!("Preserving corrupt UI preferences: {error}"));
        }
        #[cfg(unix)]
        std::fs::File::open(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )
        .and_then(|directory| directory.sync_all())
        .map_err(|e| format!("Syncing preserved UI preferences directory: {e}"))?;
        // create_new prevents overwriting any prior quarantine. Remove only
        // this UI current, after its complete byte-for-byte copy is synced.
        std::fs::remove_file(path).map_err(|e| {
            format!(
                "UI preferences preserved at {}; removing corrupt current failed: {e}",
                quarantine.display()
            )
        })?;
        return Ok(quarantine);
    }
    Err("Could not allocate a unique corrupt UI preferences quarantine".into())
}

fn role_name(role: Role) -> &'static str {
    match role {Role::Soldier=>"Defender",Role::Worker=>"Builder",Role::Scout=>"Camera trooper",Role::Boat=>"Gunboat",Role::Transport=>"Transport"}
}
fn unit_task(c:&Campaign,a:&crate::tactics::TacticalAgent)->String {
    if let Some(job)=a.job.and_then(|id|c.construction.jobs.iter().find(|j|j.id==id)) {
        return format!("{} {}{}",job_status(job.phase),job.order.kind.name(),job.blocked_reason.as_ref().map(|s|format!(" / {s}")).unwrap_or_default());
    }
    if a.embarked_on.is_some(){return "Embarked".into();}
    if a.commanded_target.is_some()||a.target.is_some(){return "Engaging observed target".into();}
    if a.goal_mm.is_some()&&!a.arrived{return "Moving to assigned destination".into();}
    if c.unit_order_queues.get(&a.id).is_some_and(|q|!q.is_empty()){return "Following command queue".into();}
    match a.role {Role::Worker=>"Idle / ready to collect and build",Role::Soldier=>"Guarding / ready to move",Role::Scout=>"Video watch / ready to scout",_=>"Ready for orders"}.into()
}
fn role_icon(role: Role) -> &'static str {
    match role {
        Role::Soldier => "soldier",
        Role::Worker => "worker",
        Role::Scout => "camera_trooper",
        Role::Boat => "boat",
        Role::Transport => "transport",
    }
}
fn resource_values(r: Resources) -> [u64; 4] {
    [r.metal, r.timber, r.rations, r.power]
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn in_world(p: [f32; 2]) -> bool {
    p.iter().all(|v| v.is_finite())
        && p[0] >= 0.
        && p[1] >= 0.
        && p[0] < WW as f32
        && p[1] < HH as f32
}
fn nearest(items: &[Selectable], p: [f32; 2], radius: f32, friendly: bool) -> Option<&Selectable> {
    items
        .iter()
        .filter(|a| {
            a.visible && a.alive && (!friendly || a.team == 0) && distance(a.position, p) <= radius
        })
        .min_by(|a, b| distance(a.position, p).total_cmp(&distance(b.position, p)))
}
fn on_screen(position: [f32; 2], camera: [f32; 3], canvas: [f32; 2]) -> bool {
    let p = world_to_screen(position, camera, canvas);
    p[0] >= 0. && p[1] >= 0. && p[0] < canvas[0] && p[1] < canvas[1]
}
fn world_to_screen(position: [f32; 2], camera: [f32; 3], canvas: [f32; 2]) -> [f32; 2] {
    [
        (position[0] - camera[0]) * camera[2] + canvas[0] * 0.5,
        (position[1] - camera[1]) * camera[2] + canvas[1] * 0.5,
    ]
}
fn clamp_panel(mut r: UiRect, canvas: [f32; 2]) -> UiRect {
    r.width = r.width.min((canvas[0] - 24.).max(200.));
    r.height = r.height.min((canvas[1] - 120.).max(120.));
    r.x = r.x.clamp(8., (canvas[0] - r.width - 8.).max(8.));
    r.y = r.y.clamp(54., (canvas[1] - r.height - 54.).max(54.));
    r
}

fn job_status(phase: crate::construction::JobPhase) -> &'static str {
    use crate::construction::JobPhase::*;
    match phase {
        Pickup => "Collecting materials",
        Hauling => "Delivering materials",
        Working => "Under construction",
        Complete => "Complete",
        Cancelled => "Cancelled",
        Blocked => "Needs attention",
    }
}
fn habitat_name(habitat: crate::ecology::Habitat) -> &'static str {
    use crate::ecology::Habitat::*;
    match habitat {
        Land => "Land",
        Water => "Aquatic",
        Amphibious => "Land and water",
    }
}
fn locomotion_name(locomotion: crate::ecology::Locomotion) -> &'static str {
    use crate::ecology::Locomotion::*;
    match locomotion {
        Rooted => "Rooted",
        Walking => "Walking",
        Climbing => "Climbing",
        Flying => "Flying",
        Swimming => "Swimming",
        Burrowing => "Burrowing",
    }
}
fn trait_description(id: u16) -> (&'static str, &'static str) {
    match id {
        11 => (
            "Tree climbing",
            "Reaches connected living canopy; climbing costs energy.",
        ),
        12 => (
            "Burrowing",
            "Excavates connected shelter; body clearance, soil and flooding limit access.",
        ),
        46 => (
            "Trap construction",
            "Builds persistent traps using gathered materials and work.",
        ),
        61 => (
            "Seasonal migration",
            "Travels to seasonal habitat while consuming reserves.",
        ),
        64 => (
            "Den dependence",
            "Needs accessible shelter with room for its actual group.",
        ),
        65 => (
            "Nest construction",
            "Spends materials and energy on reproductive shelter.",
        ),
        67 => (
            "Wallowing",
            "Repeated wet-ground use compacts soil and disturbs vegetation.",
        ),
        72 => (
            "Seasonal dormancy",
            "Reduces activity while consuming stored reserves in shelter.",
        ),
        75 => (
            "Dehydration tolerance",
            "Survives temporary shortages longer; still requires water.",
        ),
        84 => (
            "Sperm storage",
            "Retains viable reproductive material for a limited time.",
        ),
        86 => (
            "Temperature-dependent sex determination",
            "Incubation conditions affect the reproductive role of offspring.",
        ),
        93 => (
            "Egg guarding",
            "Protects a real clutch at the cost of feeding and travel time.",
        ),
        96 => (
            "Metamorphosis",
            "Changes life-stage needs and movement while retaining the same identity.",
        ),
        101 => (
            "Dam construction",
            "Gradually obstructs flow using gathered material; retained water fills upstream terrain over time.",
        ),
        102 => (
            "Soil mixing",
            "Moves finite soil and organic material between layers through work.",
        ),
        103 => (
            "Tree thinning",
            "Removes living biomass, changing canopy access and bank reinforcement.",
        ),
        104 => (
            "Cavity excavation",
            "Excavates body-sized shelter in a suitable host.",
        ),
        105 => (
            "Reef building",
            "Accumulates persistent underwater structure using finite mineral and biomass inputs.",
        ),
        106 => (
            "Pollination",
            "Carries finite pollen between reachable compatible flowering plants.",
        ),
        107 => (
            "Internal seed dispersal",
            "Carries consumed seeds, then deposits survivors after travel.",
        ),
        121 => (
            "Deep roots",
            "Reaches deeper finite moisture stores at growth and upkeep cost.",
        ),
        125 => (
            "Flood tolerance",
            "Tolerates longer inundation within depth and duration limits.",
        ),
        128 => (
            "Pioneer establishment",
            "Establishes on suitable disturbed ground from surviving seeds.",
        ),
        129 => (
            "Root resprouting",
            "Rebuilds shoots using surviving root reserves, water and time.",
        ),
        132 => (
            "Persistent seed bank",
            "Stores finite viable seeds across seasons; time and damage reduce viability.",
        ),
        135 => (
            "Animal pollination",
            "Reproduction depends on eligible pollinators reaching flowers.",
        ),
        143 => (
            "Buoyant seeds",
            "Disperses finite floating seeds through connected water.",
        ),
        156 => (
            "Dense ground cover",
            "Living cover competes with seedlings and reduces exposed soil.",
        ),
        157 => (
            "Broad shading canopy",
            "Living canopy changes understory shelter and microclimate.",
        ),
        159 => (
            "Nutrient-rich litter",
            "Returns nutrients through decomposition of actual shed or dead biomass.",
        ),
        _ => ("Unidentified adaptation", "Further research is required."),
    }
}

/// This is a crop of already visible native terrain, not synthesized species art.
/// Every copied pixel lies inside the same currently observed recon cell.
fn observed_habitat(c: &Campaign, species_id: u32) -> Option<PreviewImage> {
    let patch = c.ecology.plants.iter().find(|p| {
        p.species_id == species_id
            && p.live_units() > 0
            && c.recon
                .visible(0, crate::campaign::patch_center(p.patch as usize))
    })?;
    let center = crate::campaign::patch_center(patch.patch as usize);
    let x = (center[0] as usize / 32) * 16;
    let y = (center[1] as usize / 32) * 16;
    let width = WW / 2;
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for row in y..y + 16 {
        let start = (row * width + x) * 4;
        rgba.extend_from_slice(c.terrain.rgba.get(start..start + 16 * 4)?);
    }
    Some(PreviewImage {
        width: 16,
        height: 16,
        rgba,
        caption: "Observed habitat".into(),
    })
}
fn panel_projection(c: &Campaign, p: &PanelLayout, query: &str) -> PanelView {
    let mut v = PanelView {
        kind: p.kind,
        title: p.kind.name().into(),
        rect: p.rect,
        pinned: p.pinned,
        collapsed: p.collapsed,
        scroll: p.scroll,
        lines: Vec::new(),
        entries: Vec::new(),
    };
    match p.kind {
        PanelKind::Selection => {
            v.lines.push(
                "Distant selected troops retain their last-known report until video returns."
                    .into(),
            );
        }
        PanelKind::Ecology | PanelKind::FoodWeb => {
            for s in &c.ecology.species {
                let Some(known) = c.ecology.observed_inspect(0, s.id) else {
                    continue;
                };
                let ticks = known["study_ticks"].as_u64().unwrap_or(0);
                let researched = ticks >= 2400;
                let mut details = vec![
                    format!("Research: {}%", (ticks * 100 / 2400).min(100)),
                    format!("In your care: {}", known["captive_count"]),
                ];
                if researched {
                    details.push(format!(
                        "Habitat: {}; movement: {}",
                        habitat_name(s.habitat),
                        locomotion_name(s.locomotion)
                    ));
                    for id in s.traits.ids() {
                        let (name, effect) = trait_description(id);
                        details.push(format!("{name}: {effect}"));
                    }
                    let partners = s
                        .food_species
                        .iter()
                        .filter_map(|id| {
                            c.ecology
                                .observed_inspect(0, *id)
                                .map(|j| j["name"].as_str().unwrap_or("Unidentified").to_owned())
                        })
                        .collect::<Vec<_>>();
                    if !partners.is_empty() {
                        details.push(format!("Known food: {}", partners.join(", ")));
                    }
                } else {
                    details.push("Unknown fields: habitat, traits and relationships. Continue cared-for research.".into());
                }
                v.entries.push(PanelEntry {
                    id: s.id as u64,
                    title: s.name.clone(),
                    subtitle: if researched {
                        "Researched".into()
                    } else {
                        "Observed; research incomplete".into()
                    },
                    icon: if matches!(s.taxon, crate::ecology::Taxon::Plant) {
                        "flora".into()
                    } else {
                        let variant = c
                            .ecology
                            .cohorts
                            .iter()
                            .find(|a| {
                                a.species_id == s.id
                                    && a.count == 1
                                    && a.surface_observable
                                    && a.location_height_mm >= 0
                                    && c.detailed
                                        .get(&a.id)
                                        .is_some_and(|d| c.recon.visible(0, d.position))
                            })
                            .map(|a| a.id % 4)
                            .unwrap_or(0);
                        format!("fauna:{}:{variant}:0@{}", s.id % 4, c.terrain.fauna_appearance(s.id))
                    },
                    progress_permille: Some((ticks * 1000 / 2400).min(1000) as u16),
                    details,
                    action: Some(UiAction::SelectSpecies(s.id)),
                    relations: if researched {
                        s.food_species
                            .iter()
                            .filter(|id| c.ecology.observed_inspect(0, **id).is_some())
                            .map(|id| *id as u64)
                            .collect()
                    } else {
                        vec![]
                    },
                    preview: observed_habitat(c, s.id),
                });
            }
            if v.entries.is_empty() {
                v.lines.push("No species identified yet. Extend video coverage or wait for satellite observation.".into());
            }
            if p.kind == PanelKind::FoodWeb {
                v.lines.push("Only researched relationships to observed species appear. Drag to pan; wheel to zoom.".into());
            }
        }
        PanelKind::Logistics => {
            let r = c.construction.stores[0];
            v.lines.push(format!(
                "Stockpiles: {} metal, {} timber, {} food, {} power",
                r.metal, r.timber, r.rations, r.power
            ));
            for job in c.construction.jobs.iter().filter(|j| {
                j.order.team == 0
                    && !matches!(
                        j.phase,
                        crate::construction::JobPhase::Complete
                            | crate::construction::JobPhase::Cancelled
                    )
            }) {
                v.entries.push(PanelEntry {
                    id: job.id,
                    title: format!("{} order {}", job.order.kind.name(), job.id),
                    subtitle: job_status(job.phase).into(),
                    icon: job.order.kind.atlas_kind().into(),
                    progress_permille: Some(
                        (job.work_done as u64 * 1000 / job.work_required.max(1) as u64) as u16,
                    ),
                    details: vec![
                        format!(
                            "Reserved {} metal / {} timber",
                            job.materials.metal, job.materials.timber
                        ),
                        job.blocked_reason.clone().unwrap_or_else(|| {
                            "Requires physical material delivery and worker arrival".into()
                        }),
                    ],
                    action: None,
                    relations: vec![],
                    preview: None,
                });
            }
            if v.entries.is_empty() {
                v.lines.push(
                    "No pending construction. Mines need workers to haul ore back to base.".into(),
                );
            }
        }
        PanelKind::Damage => {
            v.lines.push(format!(
                "Your attributed damage: {}",
                c.ecological_damage[0]
            ));
            v.lines.push(if c.winner.is_some() {
                format!("Opponent attributed damage: {}", c.ecological_damage[1])
            } else {
                "Opponent damage: unverified outside observed reports".into()
            });
            v.lines.push("Construction clearance, harvesting and combat contribute. Natural erosion is not automatically blamed on a faction.".into());
            v.lines.push(
                "In an ecological stalemate, the faction causing greater damage loses.".into(),
            );
        }
        PanelKind::Help => {
            let sections = [
                (
                    "Selection",
                    "Left-drag selects friendly troops currently in video. Double-click selects the same class on screen. Shift adds or toggles membership.",
                ),
                (
                    "Orders",
                    "Right-click moves selected troops, attacks visible enemies or boards a nearby friendly transport. Shift queues commands. Escape cancels the active placement tool.",
                ),
                (
                    "Control groups",
                    "Ctrl+1..9 assigns the selection; 1..9 recalls it; Shift adds it. Double-tap a group to focus currently visible members.",
                ),
                (
                    "Camera",
                    "WASD/arrows pan; wheel zooms around the pointer. Alt+Shift+1..4 stores camera bookmarks; Alt+1..4 recalls. Edge scrolling is optional in Settings.",
                ),
                (
                    "Economy",
                    "F4 selects Road; F6 selects Trench; F7 selects Camera. Workers collect reserved construction materials, travel and build. Mines require workers and finite deposits. Roads speed movement through jungle.",
                ),
                (
                    "Ecology",
                    "E opens discovered species. Capture needs a pen and a handler; research needs continuing care. Breeding uses compatible real specimens; release requires travel.",
                ),
                (
                    "Recon",
                    "Ordinary soldiers do not grant video. Cameras and battery-limited camera troopers do. Radar dots have no identity. Satellite circles reveal the map on offset three-minute cycles.",
                ),
                (
                    "Layers",
                    "R radar; C video; O roads; L supply; U resources; V ecological health; G disturbance; K known fauna; T territory.",
                ),
                (
                    "Persistence",
                    "F5 saves the world; F9 restores it. UI layout, scale, panels, hints and accessibility settings are saved separately.",
                ),
                (
                    "Combat",
                    "X calls the currently available artillery action. Q recruits a camera trooper. Space pauses; F1/F2/F3 select 1x/2x/4x.",
                ),
            ];
            let query = query.to_ascii_lowercase();
            for (i, (title, body)) in sections.iter().enumerate().filter(|(_, (title, body))| {
                query.is_empty()
                    || format!("{title} {body}")
                        .to_ascii_lowercase()
                        .contains(&query)
            }) {
                v.entries.push(PanelEntry {
                    id: i as u64,
                    title: (*title).into(),
                    subtitle: String::new(),
                    icon: "help".into(),
                    progress_permille: None,
                    details: vec![(*body).into()],
                    action: None,
                    relations: vec![],
                    preview: None,
                });
            }
            v.lines
                .push("Click search, type a keyword; Escape returns to world commands.".into());
        }
        PanelKind::Settings => {
            v.lines = vec![
                "UI scale and accessibility preferences are saved for this machine.".into(),
                "Toggle edge scrolling, hints and reduced motion using the controls below.".into(),
                "These buildings have no garrison space. Transports can embark troops.".into(),
            ];
        }
    }
    v
}

pub fn selectables(c: &Campaign) -> Vec<Selectable> {
    c.tactics
        .agents
        .iter()
        .filter(|a| a.embarked_on.is_none() && c.recon.visible(0, a.position()))
        .map(|a| Selectable {
            id: a.id,
            team: a.team,
            role: a.role,
            position: a.position(),
            visible: c.recon.visible(0, a.position()),
            alive: a.alive(),
            health: a.health,
            max_health: if matches!(a.role, Role::Boat | Role::Transport) {
                600
            } else {
                100
            },
        })
        .collect()
}

fn panel_default(kind: PanelKind) -> UiRect {
    let (width, height) = match kind {
        PanelKind::FoodWeb => (640., 520.),
        PanelKind::Ecology => (460., 600.),
        PanelKind::Logistics => (620., 500.),
        PanelKind::Selection => (420., 450.),
        PanelKind::Help => (600., 500.),
        PanelKind::Settings => (420., 500.),
        PanelKind::Damage => (520., 460.),
    };
    UiRect {
        x: 1280. - width - 12.,
        y: 86.,
        width,
        height,
    }
}
pub fn build_kind(tool: usize) -> Option<BuildKind> {
    if tool>=30 {return [BuildKind::FieldCamera,BuildKind::CameraTower,BuildKind::MountedCamera].get(tool-30).copied();}
    [
        BuildKind::Road,
        BuildKind::Trench,
        BuildKind::Foundation,
        BuildKind::Smooth,
        BuildKind::Depot,
        BuildKind::Mine,
        BuildKind::Mill,
        BuildKind::Barracks,
        BuildKind::Solar,
        BuildKind::Farm,
        BuildKind::Camera,
        BuildKind::Radar,
        BuildKind::Wall,
        BuildKind::Turret,
        BuildKind::Dam,
        BuildKind::Bridge,
        BuildKind::Dock,
        BuildKind::Pen,
        BuildKind::Research,
    ]
    .get(tool)
    .copied()
}
pub fn tool_icon(tool:usize)->&'static str {
    ["road","trench","foundation","smooth","depot","mine","mill","barracks","solar","farm",
        "camera_pole","radar","wall","turret","dam","bridge","dock","pen","research","inspect",
        "capture","research_action","breed","release","move","gunboat","transport","board",
        "disembark","cancel","camera","camera_tower","camera_mounted"].get(tool).copied().unwrap_or("inspect")
}
pub fn tool_label(tool: usize) -> &'static str {
    if let Some(kind) = build_kind(tool) {
        return kind.name();
    }
    [
        "Inspect",
        "Capture",
        "Research",
        "Breed",
        "Release",
        "Rally",
        "Boat",
        "Transport",
        "Board",
        "Disembark",
        "Cancel order",
    ]
    .get(tool.saturating_sub(19))
    .copied()
    .unwrap_or("Command")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn a(id: u64, team: u8, role: Role, p: [f32; 2], visible: bool) -> Selectable {
        Selectable {
            id,
            team,
            role,
            position: p,
            visible,
            alive: true,
            health: 100,
            max_health: 100,
        }
    }
    fn camera() -> [f32; 3] {
        [100., 100., 1.]
    }
    #[test]
    fn box_select_and_double_click_only_choose_visible_friendly_units() {
        let units = vec![
            a(1, 0, Role::Soldier, [100., 100.], true),
            a(2, 0, Role::Soldier, [150., 100.], true),
            a(3, 1, Role::Soldier, [120., 100.], true),
            a(4, 0, Role::Worker, [130., 100.], true),
            a(5, 0, Role::Soldier, [140., 100.], false),
        ];
        let mut ui = UiState::default();
        ui.pointer_down(
            UiHit::World,
            [10., 10.],
            [90., 90.],
            false,
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &units,
            0,
        );
        ui.pointer_up(
            UiHit::World,
            [90., 60.],
            [160., 110.],
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &units,
            1,
        );
        assert_eq!(ui.selection, BTreeSet::from([1, 2, 4]));
        for t in [100, 200] {
            ui.pointer_down(
                UiHit::World,
                [50., 50.],
                [100., 100.],
                false,
                Modifiers::default(),
                camera(),
                [1280., 800.],
                &units,
                t,
            );
            ui.pointer_up(
                UiHit::World,
                [50., 50.],
                [100., 100.],
                Modifiers::default(),
                camera(),
                [1280., 800.],
                &units,
                t + 1,
            );
        }
        assert_eq!(ui.selection, BTreeSet::from([1, 2]));
    }
    #[test]
    fn control_groups_recall_known_refs_without_learning_hidden_live_positions() {
        let mut ui = UiState::default();
        ui.selection = BTreeSet::from([1, 2]);
        ui.group_key(
            1,
            Modifiers {
                control: true,
                ..Default::default()
            },
            camera(),
            0,
            &[],
        );
        ui.selection.clear();
        assert!(
            ui.group_key(1, Modifiers::default(), camera(), 100, &[])
                .is_empty()
        );
        let hidden = vec![
            a(1, 0, Role::Soldier, [4000., 2500.], false),
            a(2, 0, Role::Soldier, [4500., 2700.], false),
        ];
        assert!(
            ui.group_key(1, Modifiers::default(), camera(), 200, &hidden)
                .is_empty()
        );
        assert_eq!(ui.selection, BTreeSet::from([1, 2]));
    }
    #[test]
    fn contextual_orders_are_selected_only_queueable_and_ignore_hidden_enemies() {
        let mut ui = UiState::default();
        ui.selection.insert(8);
        let enemies = vec![a(3, 1, Role::Soldier, [150., 100.], true)];
        let cmd = ui.pointer_down(
            UiHit::World,
            [0., 0.],
            [150., 100.],
            true,
            Modifiers {
                shift: true,
                ..Default::default()
            },
            camera(),
            [1280., 800.],
            &enemies,
            0,
        );
        assert!(
            matches!(&cmd[0],UiCommand::AttackSelected{ids,target:Some(3),queue:true,..}if ids==&vec![8])
        );
        let hidden = vec![a(3, 1, Role::Soldier, [150., 100.], false)];
        let cmd = ui.pointer_down(
            UiHit::World,
            [0., 0.],
            [150., 100.],
            true,
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &hidden,
            1,
        );
        assert!(matches!(&cmd[0], UiCommand::MoveSelected { .. }));
    }
    #[test]
    fn hud_clicks_never_fall_through_to_world_and_placement_cancels_cleanly() {
        let mut ui = UiState::default();
        ui.selection.insert(2);
        let cmd = ui.pointer_down(
            UiHit::Action(UiAction::ChooseTool(10)),
            [10., 10.],
            [100., 100.],
            false,
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &[],
            0,
        );
        assert!(cmd.is_empty());
        assert_eq!(ui.active_tool, Some(10));
        let cmd = ui.pointer_down(
            UiHit::World,
            [10., 10.],
            [100., 100.],
            true,
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &[],
            1,
        );
        assert!(cmd.is_empty());
        assert!(ui.active_tool.is_none());
        assert!(ui.selection.contains(&2));
    }
    #[test]
    fn rally_board_and_disembark_tools_never_issue_legacy_global_actions() {
        let mut ui = UiState::default();
        ui.selection = BTreeSet::from([7, 8]);
        let transports = vec![a(30, 0, Role::Transport, [150., 100.], true)];
        let mut click = |tool, units: &[Selectable]| {
            ui.active_tool = Some(tool);
            let modifiers = Modifiers {
                shift: true,
                ..Default::default()
            };
            ui.pointer_down(
                UiHit::World,
                [50., 50.],
                [150., 100.],
                false,
                modifiers,
                camera(),
                [1280., 800.],
                units,
                0,
            );
            ui.pointer_up(
                UiHit::World,
                [50., 50.],
                [150., 100.],
                modifiers,
                camera(),
                [1280., 800.],
                units,
                1,
            )
        };
        assert!(
            matches!(&click(24,&[])[0],UiCommand::MoveSelected{ids,queue:true,..}if ids==&vec![7,8])
        );
        assert!(
            matches!(&click(27,&transports)[0],UiCommand::BoardSelected{ids,transport:30,queue:true}if ids==&vec![7,8])
        );
        assert!(click(27, &[a(31, 0, Role::Transport, [150., 100.], false)]).is_empty());
        assert!(
            matches!(&click(28,&[])[0],UiCommand::DisembarkSelected{transports,queue:true,..}if transports==&vec![7,8])
        );
        assert_eq!(click(29, &[]), vec![UiCommand::CancelOrder]);
    }
    #[test]
    fn camera_bookmarks_and_foodweb_gestures_never_issue_world_orders() {
        let mut ui = UiState::default();
        let bookmark = [900., 700., 2.];
        assert!(
            ui.group_key(
                2,
                Modifiers {
                    alt: true,
                    shift: true,
                    ..Default::default()
                },
                bookmark,
                0,
                &[]
            )
            .is_empty()
        );
        assert_eq!(
            ui.group_key(
                2,
                Modifiers {
                    alt: true,
                    ..Default::default()
                },
                camera(),
                1,
                &[]
            ),
            vec![UiCommand::CameraJump {
                position: [900., 700.],
                zoom: Some(2.)
            }]
        );
        ui.selection.insert(4);
        assert!(
            ui.pointer_down(
                UiHit::PanelBody(PanelKind::FoodWeb),
                [500., 300.],
                [150., 100.],
                false,
                Modifiers::default(),
                camera(),
                [1280., 800.],
                &[],
                2
            )
            .is_empty()
        );
        assert!(
            ui.pointer_move(
                UiHit::World,
                [550., 340.],
                [180., 140.],
                camera(),
                [1280., 800.],
                &[]
            )
            .is_empty()
        );
        assert!(
            ui.pointer_up(
                UiHit::World,
                [550., 340.],
                [180., 140.],
                Modifiers::default(),
                camera(),
                [1280., 800.],
                &[],
                3
            )
            .is_empty()
        );
        assert_eq!(ui.food_web_pan, [50., 40.]);
        assert_eq!(ui.selection, BTreeSet::from([4]));
    }
    #[test]
    fn minimap_navigation_zoom_and_resizing_change_only_ui_or_camera_command() {
        let mut ui = UiState::default();
        let rect = UiRect {
            x: 10.,
            y: 20.,
            width: 200.,
            height: 100.,
        };
        let cmd = ui.pointer_down(
            UiHit::Minimap(rect),
            [110., 70.],
            [0., 0.],
            false,
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &[],
            0,
        );
        assert!(
            matches!(&cmd[0],UiCommand::CameraJump{position,..}if (position[0]-2399.5).abs()<1.)
        );
        ui.wheel(UiHit::Minimap(rect), 20.);
        assert_eq!(ui.prefs.minimap_zoom_permille, 6000);
        ui.pointer_down(
            UiHit::MinimapResize(rect),
            [210., 120.],
            [0., 0.],
            false,
            Modifiers::default(),
            camera(),
            [1280., 800.],
            &[],
            1,
        );
        ui.pointer_move(
            UiHit::Blocked,
            [310., 170.],
            [0., 0.],
            camera(),
            [1280., 800.],
            &[],
        );
        assert_eq!(ui.prefs.minimap_size, [324, 210]);
    }
    #[test]
    fn panel_layout_and_accessibility_preferences_roundtrip_without_gameplay_state() {
        let mut ui = UiState::default();
        ui.activate(UiAction::OpenPanel(PanelKind::Ecology), camera(), 0);
        ui.activate(UiAction::PinPanel(PanelKind::Ecology), camera(), 0);
        ui.activate(UiAction::ToggleEdgeScroll, camera(), 0);
        ui.activate(UiAction::SetUiScale(1500), camera(), 0);
        let encoded = serde_json::to_string(&ui.prefs).unwrap();
        let prefs: UiPrefs = serde_json::from_str(&encoded).unwrap();
        prefs.validate().unwrap();
        assert!(prefs.edge_scroll);
        assert_eq!(prefs.ui_scale_permille, 1500);
        assert!(
            prefs
                .panels
                .iter()
                .any(|p| p.kind == PanelKind::Ecology && p.pinned)
        );
        assert!(!encoded.contains("selection"));
    }
    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn corrupt_layout_recovers_previous_and_next_save_preserves_both_originals() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = Fixture(std::env::temp_dir().join(format!(
                "last-world-ui-recovery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir_all(&directory.0).unwrap();
        let path = directory.0.join("ui-layout.json");
        let mut original = UiState::default();
        original.prefs.ui_scale_permille = 1250;
        original.prefs.edge_scroll = true;
        original.save_layout(&path).unwrap();
        let expected = original.prefs.clone();
        original.prefs.ui_scale_permille = 1500;
        original.save_layout(&path).unwrap();
        let previous = campaign_support::previous_path(&path);
        let previous_bytes = std::fs::read(&previous).unwrap();
        let corrupt_bytes = b"interrupted UI preferences\0not valid JSON\n";
        std::fs::write(&path, corrupt_bytes).unwrap();

        let mut restored = UiState::load_layout(&path).unwrap();
        assert_eq!(restored.prefs, expected);
        assert_eq!(campaign_support::load::<UiPrefs>(&path).unwrap(), expected);
        assert_eq!(std::fs::read(&previous).unwrap(), previous_bytes);
        let quarantines = std::fs::read_dir(&directory.0)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains(".corrupt-")
            })
            .collect::<Vec<_>>();
        assert_eq!(quarantines.len(), 1);
        assert!(
            quarantines[0]
                .file_name()
                .unwrap()
                .to_string_lossy()
                .contains(&format!(".corrupt-{}-", std::process::id()))
        );
        assert_eq!(std::fs::read(&quarantines[0]).unwrap(), corrupt_bytes);

        restored.prefs.hints = false;
        restored.save_layout(&path).unwrap();
        assert!(!campaign_support::load::<UiPrefs>(&path).unwrap().hints);
        assert_eq!(std::fs::read(&previous).unwrap(), previous_bytes);
        assert_eq!(std::fs::read(&quarantines[0]).unwrap(), corrupt_bytes);
    }
    #[test]
    fn alerts_merge_prioritize_and_jump_only_to_explicit_known_location() {
        let mut ui = UiState::default();
        ui.notice(
            "combat",
            3,
            "Troops under fire".into(),
            Some([90., 120.]),
            10,
        );
        ui.notice(
            "combat",
            3,
            "Troops under fire".into(),
            Some([95., 120.]),
            20,
        );
        ui.notice("supply", 2, "Materials missing".into(), None, 30);
        assert_eq!(ui.alerts.len(), 2);
        assert_eq!(ui.alerts[0].repeats, 2);
        let id = ui.alerts[0].id;
        assert_eq!(
            ui.activate(UiAction::JumpAlert(id), camera(), 40),
            vec![UiCommand::CameraJump {
                position: [95., 120.],
                zoom: None
            }]
        );
        let id = ui.alerts[1].id;
        assert!(
            ui.activate(UiAction::JumpAlert(id), camera(), 40)
                .is_empty()
        );
    }
    #[test]
    fn invalid_layout_is_rejected_and_edgescroll_is_optional() {
        let mut ui = UiState::default();
        assert_eq!(ui.edge_pan([1., 400.], [1280., 800.]), [0., 0.]);
        ui.prefs.edge_scroll = true;
        assert_eq!(ui.edge_pan([1., 400.], [1280., 800.]), [-1., 0.]);
        ui.prefs.ui_scale_permille = 3000;
        assert!(ui.prefs.validate().is_err());
    }
}
