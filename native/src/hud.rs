//! Cached native RTS interface. Projection data is read-only; actions are sent
//! back through normal campaign commands. No gameplay quantity is owned here.
use crate::ui_interaction::{
    self as ui, CommandCategory, Layer, PanelKind, UiAction, UiHit, UiPresentation, UiRect,
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Range,
    path::{Path, PathBuf},
    sync::OnceLock,
};

pub const MAX_WIDTH: u32 = 4096;
pub const MAX_HEIGHT: u32 = 2304;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HudModel {
    pub lines: Vec<String>,
    pub tools: Vec<String>,
    pub selected_tool: usize,
    pub footer: Vec<String>,
    pub ui: Option<UiPresentation>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl Rect {
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x as f64
            && y >= self.y as f64
            && x < (self.x + self.width) as f64
            && y < (self.y + self.height) as f64
    }
    fn end_x(self) -> u32 {
        self.x.saturating_add(self.width)
    }
    fn end_y(self) -> u32 {
        self.y.saturating_add(self.height)
    }
    fn intersect(self, b: Self) -> Option<Self> {
        let x = self.x.max(b.x);
        let y = self.y.max(b.y);
        let right = self.end_x().min(b.end_x());
        let bottom = self.end_y().min(b.end_y());
        (right > x && bottom > y).then_some(Self {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        })
    }
    fn union(self, b: Self) -> Self {
        let x = self.x.min(b.x);
        let y = self.y.min(b.y);
        Self {
            x,
            y,
            width: self.end_x().max(b.end_x()) - x,
            height: self.end_y().max(b.end_y()) - y,
        }
    }
    fn inset(self, p: u32) -> Self {
        Self {
            x: self.x + p.min(self.width / 2),
            y: self.y + p.min(self.height / 2),
            width: self.width.saturating_sub(p * 2),
            height: self.height.saturating_sub(p * 2),
        }
    }
    fn as_ui(self) -> UiRect {
        UiRect {
            x: self.x as f32,
            y: self.y as f32,
            width: self.width as f32,
            height: self.height as f32,
        }
    }
}
pub fn canvas_size(width: u32, height: u32) -> (u32, u32) {
    (width.clamp(1, MAX_WIDTH), height.clamp(1, MAX_HEIGHT))
}
struct UiAssets {
    roots: Vec<PathBuf>,
}
impl UiAssets {
    fn new(data: &Path, supplied_app: Option<&Path>, executable: Option<&Path>) -> Self {
        let mut roots = vec![data.join("ui")];
        if let Some(app) = supplied_app
            .filter(|p| p.is_absolute())
            .or_else(|| executable.and_then(|p| p.parent()))
        {
            let ui = app.join("ui");
            if !roots.contains(&ui) {
                roots.push(ui);
            }
        }
        Self { roots }
    }
    fn runtime() -> Self {
        let data = std::env::var_os("LAST_WORLD_DATA_DIR")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_default();
        let app = std::env::var_os("LAST_WORLD_ASSET_DIR").map(PathBuf::from);
        let executable = std::env::current_exe().ok();
        Self::new(&data, app.as_deref(), executable.as_deref())
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct FontRef {
    atlas: String,
    metrics: String,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct ToolRef {
    index: usize,
    label: String,
    icon: String,
    hotkey: String,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct IconRef {
    #[allow(dead_code)]
    icon: String,
    #[allow(dead_code)]
    label: String,
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Theme {
    palette: BTreeMap<String, [u8; 4]>,
    metrics: BTreeMap<String, f32>,
    fonts: BTreeMap<String, FontRef>,
    tools: Vec<ToolRef>,
    categories: BTreeMap<String, IconRef>,
    layers: BTreeMap<String, String>,
    panels: BTreeMap<String, String>,
    resources: BTreeMap<String, String>,
    materials: serde_json::Value,
}
impl Theme {
    fn global() -> &'static Self {
        static THEME: OnceLock<Theme> = OnceLock::new();
        THEME.get_or_init(|| {
            let (theme, invalid) = Self::load(&UiAssets::runtime());
            if invalid {
                crate::logging::log(
                    "UI_THEME_WARNING",
                    "External UI theme invalid; next valid default used",
                );
            }
            theme
        })
    }
    fn load(assets: &UiAssets) -> (Self, bool) {
        let valid = |s: &str| {
            serde_json::from_str::<Theme>(s).ok().filter(|t| {
                t.metrics.values().all(|v| v.is_finite() && *v >= 0.)
                    && t.palette.contains_key("text")
                    && t.fonts.contains_key("body")
            })
        };
        let mut invalid = false;
        for root in &assets.roots {
            if let Ok(loaded) = std::fs::read_to_string(root.join("theme.json")) {
                if let Some(theme) = valid(&loaded) {
                    return (theme, invalid);
                }
                invalid = true;
            }
        }
        (
            valid(include_str!("../assets/ui/theme.json")).expect("bundled UI theme"),
            invalid,
        )
    }
    fn color(&self, key: &str) -> [u8; 4] {
        self.palette
            .get(key)
            .copied()
            .unwrap_or([238, 233, 216, 255])
    }
    fn metric(&self, key: &str) -> f32 {
        *self.metrics.get(key).unwrap_or(&8.)
    }
    fn tool(&self, index: usize) -> Option<&ToolRef> {
        self.tools.iter().find(|t| t.index == index)
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq)]
struct Glyph {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    left: i32,
    top: i32,
    advance: f32,
}
#[derive(Debug, Deserialize, PartialEq)]
struct FontMeta {
    source_pixels: f32,
    width: u32,
    height: u32,
    glyphs: BTreeMap<u32, Glyph>,
}
#[derive(Debug, PartialEq)]
struct Font {
    meta: FontMeta,
    alpha: Vec<u8>,
}
fn png_bytes(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32, png::ColorType), String> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut out = vec![0; reader.output_buffer_size().ok_or("Image size missing")?];
    let info = reader.next_frame(&mut out).map_err(|e| e.to_string())?;
    out.truncate(info.buffer_size());
    Ok((out, info.width, info.height, info.color_type))
}
impl Font {
    fn load(role: &str, image: &[u8], meta: &str) -> Self {
        let (font, invalid) =
            Self::load_from_assets(Theme::global(), &UiAssets::runtime(), role, image, meta);
        if invalid {
            crate::logging::log(
                "UI_FONT_WARNING",
                "External glyph atlas invalid; next valid licensed glyphs used",
            );
        }
        font
    }
    fn load_from_assets(
        theme: &Theme,
        assets: &UiAssets,
        role: &str,
        image: &[u8],
        meta: &str,
    ) -> (Self, bool) {
        let reference = theme.fonts.get(role);
        let parse = |image: &[u8], meta: &str| -> Result<Self, String> {
            let m: FontMeta = serde_json::from_str(meta).map_err(|e| e.to_string())?;
            let (a, w, h, color) = png_bytes(image)?;
            if color != png::ColorType::Grayscale
                || w != m.width
                || h != m.height
                || a.len() != w as usize * h as usize
                || !m.source_pixels.is_finite()
                || m.source_pixels <= 0.
                || m.source_pixels > 256.
                || !m.glyphs.values().all(|g| {
                    g.advance.is_finite()
                        && g.advance >= 0.
                        && g.x.checked_add(g.width).is_some_and(|x| x <= w)
                        && g.y.checked_add(g.height).is_some_and(|y| y <= h)
                })
            {
                return Err("Invalid UI glyph atlas".into());
            }
            Ok(Self { meta: m, alpha: a })
        };
        let mut invalid = false;
        if let Some(reference) = reference {
            for root in &assets.roots {
                if let (Ok(bytes), Ok(metadata)) = (
                    std::fs::read(root.join(&reference.atlas)),
                    std::fs::read_to_string(root.join(&reference.metrics)),
                ) {
                    if let Ok(font) = parse(&bytes, &metadata) {
                        return (font, invalid);
                    }
                    invalid = true;
                }
            }
        }
        (parse(image, meta).expect("bundled glyph atlas"), invalid)
    }
    fn glyph(&self, ch: char) -> &Glyph {
        self.meta
            .glyphs
            .get(&(ch as u32))
            .or_else(|| self.meta.glyphs.get(&63))
            .expect("question glyph")
    }
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|ch| self.glyph(ch).advance * size / self.meta.source_pixels)
            .sum()
    }
}
struct Fonts {
    body: Font,
    numbers: Font,
    heading: Font,
}
impl Fonts {
    fn get() -> &'static Self {
        static F: OnceLock<Fonts> = OnceLock::new();
        F.get_or_init(|| Self {
            body: Font::load(
                "body",
                include_bytes!("../assets/ui/body-atlas.png"),
                include_str!("../assets/ui/body-atlas.json"),
            ),
            numbers: Font::load(
                "numbers",
                include_bytes!("../assets/ui/numbers-atlas.png"),
                include_str!("../assets/ui/numbers-atlas.json"),
            ),
            heading: Font::load(
                "heading",
                include_bytes!("../assets/ui/heading-atlas.png"),
                include_str!("../assets/ui/heading-atlas.json"),
            ),
        })
    }
}
struct Atlas {
    rgba: Vec<u8>,
    width: u32,
    rects: BTreeMap<String, Rect>,
}
impl Atlas {
    fn get() -> &'static Self {
        static A: OnceLock<Atlas> = OnceLock::new();
        A.get_or_init(|| {
            let (rgba, width, _, color) =
                png_bytes(include_bytes!("../assets/pixel-atlas.png")).expect("grown icon atlas");
            assert_eq!(color, png::ColorType::Rgba);
            let metadata: serde_json::Value =
                serde_json::from_str(include_str!("../assets/pixel-atlas.json")).unwrap();
            let mut rects = BTreeMap::new();
            for (key, v) in metadata["rects"].as_object().unwrap() {
                let x = v[0].as_u64().unwrap() as u32;
                let y = v[1].as_u64().unwrap() as u32;
                let w = v[2].as_u64().unwrap() as u32;
                let h = v[3].as_u64().unwrap() as u32;
                let (mut l, mut t, mut r, mut b) = (w, h, 0, 0);
                for yy in 0..h {
                    for xx in 0..w {
                        if rgba[((y + yy) as usize * width as usize + (x + xx) as usize) * 4 + 3]
                            >= 128
                        {
                            l = l.min(xx);
                            t = t.min(yy);
                            r = r.max(xx + 1);
                            b = b.max(yy + 1);
                        }
                    }
                }
                if r > l && b > t {
                    rects.insert(
                        key.clone(),
                        Rect {
                            x: x + l,
                            y: y + t,
                            width: r - l,
                            height: b - t,
                        },
                    );
                }
            }
            Self { rgba, width, rects }
        })
    }
    fn rect(&self, key: &str) -> Option<Rect> {
        let resolved = match key {
            "fauna" | "unknown" => "fauna:0:0:0",
            "flora" => "farm:0:0",
            "worker" => "unit:0:0:0:0",
            "soldier" | "unit" => "unit:0:1:0:0",
            "scout" => "unit:0:2:0:0",
            "boat" | "transport" => "ship:0:0",
            _ => key,
        };
        self.rects
            .get(resolved)
            .copied()
            .or_else(|| self.rects.get(&format!("{resolved}:0:0")).copied())
    }
}
fn scale(view: &UiPresentation, height: u32) -> f32 {
    ((height as f32 / 1080.).powf(0.75).clamp(0.875, 2.))
        * view.prefs.ui_scale_permille.clamp(750, 2000) as f32
        / 1000.
}
fn rect(x: f32, y: f32, w: f32, h: f32, cw: u32, ch: u32) -> Rect {
    let x = x.max(0.).min(cw as f32) as u32;
    let y = y.max(0.).min(ch as f32) as u32;
    Rect {
        x,
        y,
        width: (w.max(0.) as u32).min(cw - x),
        height: (h.max(0.) as u32).min(ch - y),
    }
}
#[derive(Clone)]
struct Target {
    rect: Rect,
    hit: UiHit,
}
#[derive(Clone)]
pub struct UiLayout {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub top: Rect,
    top_resources: Rect,
    top_controls: Rect,
    pub minimap: Rect,
    pub minimap_inner: Rect,
    pub selection: Rect,
    selection_actions: Vec<(String,Rect,UiAction,bool)>,
    pub command: Rect,
    targets: Vec<Target>,
    category_buttons: Vec<(CommandCategory, Rect)>,
    tool_buttons: Vec<(usize, Rect)>,
    layer_buttons: Vec<(Layer, Rect)>,
    quick_buttons: Vec<(String, Rect, UiAction)>,
    resource_buttons: Vec<Rect>,
    panels: Vec<(ui::PanelView, Rect)>,
    alerts: Vec<(ui::AlertView, Rect)>,
}
impl UiLayout {
    fn new(model: &HudModel, width: u32, height: u32) -> Self {
        let (w, h) = canvas_size(width, height);
        let fallback = UiPresentation::default();
        let v = model.ui.as_ref().unwrap_or(&fallback);
        let t = Theme::global();
        let s = scale(v, h);
        let g = t.metric("gap") * s;
        let p = t.metric("padding") * s;
        let hit =
            (t.metric("hit") * s).max(t.metric("hit") * v.prefs.ui_scale_permille as f32 / 1000.);
        let top_h = (t.metric("top_height") * s).max(hit + 4. * s);
        let columns = t.metric("command_columns").round().clamp(3., 4.) as usize;
        let rows = v.category.tools().len().div_ceil(columns).max(1);
        let quick_h = (t.metric("quick_height") * s)
            .max(hit)
            .max(t.metric("quick_min_height"));
        let quick_w = (t.metric("quick_width") * s).max(t.metric("quick_min_width"));
        let tile_h = (t.metric("command_tile_height") * s).max(hit);
        let max_command_h = (h as f32 - top_h - quick_h - 5. * g).max(hit * 2.);
        let tile_h =
            tile_h.min(((max_command_h - 22. * s - hit - 2. * g) / rows as f32 - 4. * s).max(hit));
        let bottom_h = 22. * s + hit + 4. * s + rows as f32 * (tile_h + 4. * s) + g;
        let command_w = (t.metric("command_width") * s)
            .max(6. * hit.max(t.metric("category_min_width")) + 2. * g)
            .min(w as f32 - 2. * g);
        let mini_w =
            (v.prefs.minimap_size[0] as f32).clamp(140. * s, (w as f32 * 0.3).max(140. * s));
        let mini_h =
            (v.prefs.minimap_size[1] as f32).clamp(100. * s, (h as f32 * 0.38).max(100. * s));
        let selection_w = (t.metric("selection_width") * s)
            .min(w as f32 - command_w - mini_w - 5. * g)
            .max(110. * s);
        let top = rect(g, g, w as f32 - 2. * g, top_h, w, h);
        let command = rect(
            w as f32 - command_w - g,
            h as f32 - bottom_h - g,
            command_w,
            bottom_h,
            w,
            h,
        );
        let minimap = rect(
            g,
            h as f32 - mini_h - 26. * s - g,
            mini_w,
            mini_h + 26. * s,
            w,
            h,
        );
        let minimap_inner = rect(
            minimap.x as f32 + 6. * s,
            minimap.y as f32 + 22. * s,
            minimap.width as f32 - 12. * s,
            minimap.height as f32 - 28. * s,
            w,
            h,
        );
        let selection_h = if v.selection.selected_count == 0 {
            (140.*s).max(118.)
        } else {
            (320.*s).max(264.)
        };
        let selection = rect(
            minimap.end_x() as f32 + g,
            h as f32 - selection_h - g,
            selection_w,
            selection_h,
            w,
            h,
        );
        let mut out = Self {
            width: w,
            height: h,
            scale: s,
            top,
            top_resources: top,
            top_controls: top,
            minimap,
            minimap_inner,
            selection,
            selection_actions: vec![],
            command,
            targets: vec![],
            category_buttons: vec![],
            tool_buttons: vec![],
            layer_buttons: vec![],
            quick_buttons: vec![],
            resource_buttons: vec![],
            panels: vec![],
            alerts: vec![],
        };
        let control_w = (hit * 6. + p).min(top.width as f32 * 0.42);
        let resource_end = top.end_x() as f32 - control_w - 16. * s;
        let count = v.resources.len().max(1);
        let resource_w = ((resource_end - top.x as f32 - 100. * s) / count as f32)
            .min(152. * s)
            .max(70. * s);
        for i in 0..v.resources.len() {
            let r = rect(
                top.x as f32 + 100. * s + i as f32 * resource_w,
                top.y as f32,
                resource_w,
                top.height as f32,
                w,
                h,
            );
            out.resource_buttons.push(r);
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(UiAction::OpenPanel(PanelKind::Logistics)),
            });
        }
        out.top_resources = rect(
            top.x as f32,
            top.y as f32,
            100. * s + v.resources.len() as f32 * resource_w,
            top.height as f32,
            w,
            h,
        );
        out.top_controls = rect(
            top.end_x() as f32 - 6. * hit,
            top.y as f32,
            6. * hit,
            top.height as f32,
            w,
            h,
        );
        for (i, action) in [
            UiAction::Speed(1),
            UiAction::Speed(2),
            UiAction::Speed(4),
            UiAction::Pause,
            UiAction::OpenPanel(PanelKind::Help),
            UiAction::OpenPanel(PanelKind::Settings),
        ]
        .into_iter()
        .enumerate()
        {
            let r = rect(
                top.end_x() as f32 - (6 - i) as f32 * hit,
                top.y as f32,
                hit,
                top.height as f32,
                w,
                h,
            );
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(action),
            });
        }
        let category_y = command.y as f32 + 22. * s;
        let category_w = (command.width as f32 - 2. * g) / 6.;
        for (i, cat) in CommandCategory::ALL.into_iter().enumerate() {
            let r = rect(
                command.x as f32 + g + i as f32 * category_w,
                category_y,
                category_w,
                hit,
                w,
                h,
            );
            out.category_buttons.push((cat, r));
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(UiAction::ChooseCategory(cat)),
            });
        }
        let grid_y = category_y + hit + 4. * s;
        let grid_w = (command.width as f32 - 2. * g) / columns as f32;
        for (i, index) in v.category.tools().iter().copied().enumerate() {
            let r = rect(
                command.x as f32 + g + (i % columns) as f32 * grid_w,
                grid_y + (i / columns) as f32 * (tile_h + 4. * s),
                grid_w - 4. * s,
                tile_h,
                w,
                h,
            );
            out.tool_buttons.push((index, r));
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(UiAction::ChooseTool(index)),
            });
        }
        for (i, (icon, action)) in [
            ("cursor", UiAction::SelectMode),
            ("road", UiAction::ChooseTool(0)),
            ("camera_pole", UiAction::ChooseTool(10)),
            ("depot", UiAction::ChooseCategory(CommandCategory::Build)),
            ("fauna", UiAction::OpenPanel(PanelKind::Ecology)),
        ]
        .into_iter()
        .enumerate()
        {
            let r = rect(
                command.end_x() as f32 - (5 - i) as f32 * (quick_w + 4. * s),
                command.y as f32 - quick_h - g,
                quick_w,
                quick_h,
                w,
                h,
            );
            out.quick_buttons.push((icon.into(), r, action.clone()));
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(action),
            });
        }
        for (i, layer) in [Layer::Radar, Layer::Cameras, Layer::Roads, Layer::Ecology]
            .into_iter()
            .enumerate()
        {
            let r = rect(
                minimap.x as f32 + i as f32 * (hit + 2. * s),
                minimap.y as f32 - hit - g,
                hit,
                hit,
                w,
                h,
            );
            out.layer_buttons.push((layer, r));
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(UiAction::ToggleLayer(layer)),
            });
        }
        out.targets.push(Target {
            rect: minimap_inner,
            hit: UiHit::Minimap(minimap_inner.as_ui()),
        });
        out.targets.push(Target {
            rect: rect(
                minimap.end_x() as f32 - hit,
                minimap.end_y() as f32 - hit,
                hit,
                hit,
                w,
                h,
            ),
            hit: UiHit::MinimapResize(minimap.as_ui()),
        });
        out.targets.push(Target {
            rect: selection,
            hit: UiHit::Action(UiAction::OpenPanel(PanelKind::Selection)),
        });
        let button_h=(36.*s).max(32.);
        let rows=v.selection.actions.len().div_ceil(3);
        for (i,(label,action,enabled)) in v.selection.actions.iter().enumerate() {
            let width=(selection.width as f32-12.*s)/3.;
            let rr=rect(selection.x as f32+6.*s+(i%3) as f32*width,
                selection.end_y() as f32-rows as f32*(button_h+4.)-6.*s+(i/3) as f32*(button_h+4.),
                width-4.,button_h,w,h);
            out.selection_actions.push((label.clone(),rr,action.clone(),*enabled));
            out.targets.push(Target{rect:rr,hit:if *enabled {UiHit::Action(action.clone())}else{UiHit::Blocked}});
        }
        for (i, a) in v.alerts.iter().take(3).cloned().enumerate() {
            let aw = (t.metric("alert_width") * s).min(w as f32 * 0.36);
            let ah = (t.metric("alert_height") * s).max(hit);
            let r = rect(
                w as f32 - aw - g,
                top.end_y() as f32 + g + i as f32 * (ah + g),
                aw,
                ah,
                w,
                h,
            );
            out.targets.push(Target {
                rect: r,
                hit: UiHit::Action(UiAction::JumpAlert(a.id)),
            });
            out.targets.push(Target {
                rect: rect(r.end_x() as f32 - hit, r.y as f32, hit, ah, w, h),
                hit: UiHit::Action(UiAction::DismissAlert(a.id)),
            });
            out.alerts.push((a, r));
        }
        for panel in &v.panels {
            let x = panel
                .rect
                .x
                .max(g)
                .min((w as f32 - panel.rect.width - g).max(g));
            let y = panel
                .rect
                .y
                .max(top.end_y() as f32 + g)
                .min((h as f32 - panel.rect.height - g).max(top.end_y() as f32 + g));
            let ph = if panel.collapsed {
                hit
            } else {
                panel.rect.height.max(hit * 2.)
            };
            let r = rect(
                x,
                y,
                panel.rect.width.min(w as f32 - 2. * g),
                ph.min(h as f32 - top.end_y() as f32 - 2. * g),
                w,
                h,
            );
            out.panels.push((panel.clone(), r));
        }
        out
    }
}
pub fn layout(model: &HudModel, width: u32, height: u32) -> UiLayout {
    UiLayout::new(model, width, height)
}
pub fn tool_rects(model: &HudModel, width: u32, height: u32) -> Vec<Rect> {
    UiLayout::new(model, width, height)
        .tool_buttons
        .into_iter()
        .map(|(_, r)| r)
        .collect()
}
pub fn tool_at(model: &HudModel, width: u32, height: u32, x: f64, y: f64) -> Option<usize> {
    match hit_test(model, width, height, x, y) {
        UiHit::Action(UiAction::ChooseTool(i)) => Some(i),
        _ => None,
    }
}
pub fn hit_test(model: &HudModel, width: u32, height: u32, x: f64, y: f64) -> UiHit {
    if width == 0 || height == 0 || !x.is_finite() || !y.is_finite() {
        return UiHit::Blocked;
    }
    let l = layout(model, width, height);
    let (w, h) = canvas_size(width, height);
    let px = x * w as f64 / width as f64;
    let py = y * h as f64 / height as f64;
    let hit = (Theme::global().metric("hit") * l.scale).max(
        44. * model
            .ui
            .as_ref()
            .map_or(1., |v| v.prefs.ui_scale_permille as f32 / 1000.),
    );
    if model.ui.as_ref().is_some_and(|v|v.modal.is_some()) {
        let (_,button)=modal_rects(&l);
        return if button.contains(px,py){UiHit::Action(UiAction::DismissModal)}else{UiHit::Blocked};
    }
    for (panel, r) in l.panels.iter().rev() {
        if !r.contains(px, py) {
            continue;
        }
        let right = r.end_x() as f32;
        for (i, action) in [
            UiAction::ClosePanel(panel.kind),
            UiAction::PinPanel(panel.kind),
            UiAction::CollapsePanel(panel.kind),
        ]
        .into_iter()
        .enumerate()
        {
            if rect(right - (i + 1) as f32 * hit, r.y as f32, hit, hit, w, h).contains(px, py) {
                return UiHit::Action(action);
            }
        }
        if py < r.y as f64 + hit as f64 {
            return UiHit::PanelHeader(panel.kind);
        }
        if panel.collapsed {
            return UiHit::Blocked;
        }
        if panel.kind == PanelKind::Settings {
            let actions = settings_actions(model);
            for (i, (_, a)) in actions.into_iter().skip(panel.scroll as usize).enumerate() {
                let rr = rect(
                    r.x as f32 + 12. * l.scale,
                    r.y as f32 + hit + 10. * l.scale + i as f32 * (hit + 4. * l.scale),
                    r.width as f32 - 24. * l.scale,
                    hit,
                    w,
                    h,
                );
                if rr.contains(px, py) {
                    return UiHit::Action(a);
                }
            }
        } else if panel.kind == PanelKind::FoodWeb {
            for (i, action) in [UiAction::FoodWebZoom(-1), UiAction::FoodWebZoom(1)]
                .into_iter()
                .enumerate()
            {
                if rect(
                    r.x as f32 + 10. * l.scale + i as f32 * (hit + 4. * l.scale),
                    r.end_y() as f32 - hit - 8. * l.scale,
                    hit,
                    hit,
                    w,
                    h,
                )
                .contains(px, py)
                {
                    return UiHit::Action(action);
                }
            }
            if let Some(v) = &model.ui {
                for (entry, nr) in graph_nodes(panel, *r, v, l.scale) {
                    if nr.contains(px, py) {
                        return entry
                            .action
                            .clone()
                            .map(UiHit::Action)
                            .unwrap_or(UiHit::PanelBody(panel.kind));
                    }
                }
            }
        } else {
            if panel.kind == PanelKind::Help {
                let search = rect(
                    r.x as f32 + 12. * l.scale,
                    r.y as f32 + hit + 8. * l.scale,
                    r.width as f32 - 24. * l.scale,
                    hit,
                    w,
                    h,
                );
                if search.contains(px, py) {
                    return UiHit::Action(if px > search.end_x() as f64 - hit as f64 {
                        UiAction::ClearHelpSearch
                    } else {
                        UiAction::FocusHelpSearch
                    });
                }
            }
            for (entry, rr) in panel_rows(panel, *r, l.scale) {
                if rr.contains(px, py) {
                    return entry
                        .action
                        .clone()
                        .map(UiHit::Action)
                        .unwrap_or(UiHit::PanelBody(panel.kind));
                }
            }
        }
        return UiHit::PanelBody(panel.kind);
    }
    for target in l.targets.iter().rev() {
        if target.rect.contains(px, py) {
            if let UiHit::Action(UiAction::ToggleLayer(layer)) = &target.hit {
                if model.ui.as_ref().is_some_and(|v| {
                    v.disabled_layers
                        .iter()
                        .any(|(disabled, _)| disabled == layer)
                }) {
                    return UiHit::Blocked;
                }
            }
            return target.hit.clone();
        }
    }
    if [
        l.top_resources,
        l.top_controls,
        l.minimap,
        l.selection,
        l.command,
    ]
    .iter()
    .any(|r| r.contains(px, py))
    {
        UiHit::Blocked
    } else {
        UiHit::World
    }
}
fn settings_actions(model: &HudModel) -> Vec<(String, UiAction)> {
    let d = UiPresentation::default();
    let v = model.ui.as_ref().unwrap_or(&d);
    let mut a = vec![
        (
            format!("UI scale {}%  /  decrease", v.prefs.ui_scale_permille / 10),
            UiAction::SetUiScale(v.prefs.ui_scale_permille.saturating_sub(250).max(750)),
        ),
        (
            "Increase UI scale".into(),
            UiAction::SetUiScale((v.prefs.ui_scale_permille + 250).min(2000)),
        ),
        (
            format!(
                "Edge scroll: {}",
                if v.prefs.edge_scroll { "ON" } else { "OFF" }
            ),
            UiAction::ToggleEdgeScroll,
        ),
        (
            format!(
                "Context hints: {}",
                if v.prefs.hints { "ON" } else { "OFF" }
            ),
            UiAction::ToggleHints,
        ),
        (
            format!(
                "Reduce motion: {}",
                if v.prefs.reduce_motion { "ON" } else { "OFF" }
            ),
            UiAction::ToggleReduceMotion,
        ),
        (format!("Show icon IDs: {}",if v.prefs.show_icon_ids {"ON"}else{"OFF"}),UiAction::ToggleIconIds),
        ("Save world  /  F5".into(), UiAction::Save),
        ("Load world  /  F9".into(), UiAction::Load),
    ];
    for layer in Layer::ALL {
        a.push((
            format!("{} layer  [{}]", layer.name(), layer.hotkey()),
            UiAction::ToggleLayer(layer),
        ));
    }
    a
}
fn panel_rows(panel: &ui::PanelView, r: Rect, s: f32) -> Vec<(&ui::PanelEntry, Rect)> {
    let header = (44. * s).max(44.);
    let row = 72. * s;
    let start = r.y as f32 + header + 12. * s + panel.lines.len().min(5) as f32 * 22. * s;
    panel
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let y = start + (i as f32 - panel.scroll as f32) * row;
            let rw = if panel.kind == PanelKind::Ecology {
                (r.width as f32 * 0.38)
                    .max(110. * s)
                    .min(r.width as f32 - 100. * s)
            } else {
                r.width as f32 - 20. * s
            };
            let rr = rect(
                r.x as f32 + 10. * s,
                y,
                rw,
                row - 4. * s,
                r.end_x(),
                r.end_y(),
            );
            if y < r.y as f32 + header || y + row > r.end_y() as f32 {
                return None;
            }
            Some((e, rr))
        })
        .collect()
}
fn graph_nodes<'a>(
    panel: &'a ui::PanelView,
    r: Rect,
    v: &UiPresentation,
    s: f32,
) -> Vec<(&'a ui::PanelEntry, Rect)> {
    let z = v.food_web_zoom.clamp(0.4, 2.5);
    let nw = 100. * s * z;
    let nh = 76. * s * z;
    let cols = ((r.width as f32 - 24. * s + 16. * s) / (nw + 16. * s))
        .floor()
        .max(1.) as usize;
    panel
        .entries
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            let x =
                r.x as f32 + 12. * s + (i % cols) as f32 * (nw + 16. * s) + v.food_web_pan[0] * s;
            let y =
                r.y as f32 + 80. * s + (i / cols) as f32 * (nh + 38. * s) + v.food_web_pan[1] * s;
            let rr = rect(x, y, nw, nh, r.end_x(), r.end_y());
            (x >= r.x as f32 + 4.
                && y >= r.y as f32 + 64. * s
                && x + nw <= r.end_x() as f32 - 4.
                && y + nh <= r.end_y() as f32 - 4.)
                .then_some((e, rr))
        })
        .collect()
}
fn blend(dst: &mut [u8], src: [u8; 4]) {
    let a = src[3] as u32;
    if a == 0 {
        return;
    }
    if a == 255 || dst[3] == 0 {
        dst.copy_from_slice(&src);
        return;
    }
    let d = dst[3] as u32;
    let out = a + (d * (255 - a) + 127) / 255;
    for i in 0..3 {
        dst[i] = ((src[i] as u32 * a * 255 + dst[i] as u32 * d * (255 - a) + out * 127)
            / (out * 255)) as u8;
    }
    dst[3] = out as u8;
}
struct Paint {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    origin: [u32; 2],
    clip: Option<Rect>,
}
impl Paint {
    fn new(r: Rect) -> Self {
        Self {
            width: r.width,
            height: r.height,
            rgba: vec![0; r.width as usize * r.height as usize * 4],
            origin: [r.x, r.y],
            clip: None,
        }
    }
    fn px(&mut self, x: i32, y: i32, c: [u8; 4]) {
        if self.clip.is_some_and(|r| !r.contains(x as f64, y as f64)) {
            return;
        }
        let x = x - self.origin[0] as i32;
        let y = y - self.origin[1] as i32;
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        blend(&mut self.rgba[i..i + 4], c);
    }
    fn fill(&mut self, r: Rect, c: [u8; 4]) {
        let bounds = Rect {
            x: self.origin[0],
            y: self.origin[1],
            width: self.width,
            height: self.height,
        };
        if let Some(a) = bounds.intersect(r) {
            for y in a.y..a.end_y() {
                for x in a.x..a.end_x() {
                    self.px(x as i32, y as i32, c);
                }
            }
        }
    }
    fn line(&mut self, a: [f32; 2], b: [f32; 2], c: [u8; 4], thick: u32) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let n = dx
            .abs()
            .max(dy.abs())
            .ceil()
            .min((self.width + self.height) as f32 * 3.) as usize;
        for i in 0..=n.max(1) {
            let f = i as f32 / n.max(1) as f32;
            let x = (a[0] + dx * f).round() as i32;
            let y = (a[1] + dy * f).round() as i32;
            for oy in 0..thick.max(1) {
                for ox in 0..thick.max(1) {
                    self.px(x + ox as i32, y + oy as i32, c);
                }
            }
        }
    }
    fn outline(&mut self, r: Rect, c: [u8; 4], n: u32) {
        self.fill(
            Rect {
                height: n.min(r.height),
                ..r
            },
            c,
        );
        self.fill(
            Rect {
                y: r.end_y().saturating_sub(n),
                height: n.min(r.height),
                ..r
            },
            c,
        );
        self.fill(
            Rect {
                width: n.min(r.width),
                ..r
            },
            c,
        );
        self.fill(
            Rect {
                x: r.end_x().saturating_sub(n),
                width: n.min(r.width),
                ..r
            },
            c,
        );
    }
    fn metal(&mut self, r: Rect, raised: bool, accent: bool) {
        let t = Theme::global();
        let base = t.color(if raised { "panel" } else { "frame" });
        let seed = t.materials["seed"].as_u64().unwrap_or(78421) as u32;
        let grain = t.materials["grain_strength"].as_i64().unwrap_or(4) as i32;
        let scan = t.materials["scanline_stride"].as_u64().unwrap_or(4).max(1) as u32;
        let bevel = t.materials["bevel_px"].as_u64().unwrap_or(2) as u32;
        let chamfer = 6.min(r.width / 3).min(r.height / 3);
        for yy in 0..r.height {
            let cut = if yy < chamfer {
                chamfer - yy
            } else if yy >= r.height.saturating_sub(chamfer) {
                yy - r.height.saturating_sub(chamfer) + 1
            } else {
                0
            };
            for xx in cut..r.width.saturating_sub(cut) {
                let mut n = (xx.wrapping_mul(374761393) ^ yy.wrapping_mul(668265263) ^ seed)
                    .wrapping_mul(1274126177);
                n ^= n >> 13;
                let delta = (n % (grain.max(0) as u32 * 2 + 1)) as i32
                    - grain
                    - i32::from(yy % scan == 0) * 2;
                let mut c = base;
                for v in c.iter_mut().take(3) {
                    *v = (*v as i32 + delta).clamp(0, 255) as u8;
                }
                if yy < bevel || xx < bevel {
                    c = t.color("edge");
                }
                if yy >= r.height.saturating_sub(bevel) || xx >= r.width.saturating_sub(bevel) {
                    c = t.color("black");
                }
                self.px((r.x + xx) as i32, (r.y + yy) as i32, c);
            }
        }
        for (x, y) in [
            (r.x + 7, r.y + 7),
            (r.end_x().saturating_sub(10), r.y + 7),
            (r.x + 7, r.end_y().saturating_sub(10)),
            (r.end_x().saturating_sub(10), r.end_y().saturating_sub(10)),
        ] {
            self.fill(
                Rect {
                    x,
                    y,
                    width: 3,
                    height: 3,
                },
                t.color("edge"),
            );
            self.px(x as i32 + 1, y as i32 + 1, t.color("black"));
        }
        if accent {
            let c = t.color("faction0");
            for offset in [0, 6] {
                self.line(
                    [r.x as f32 + 4., r.y as f32 + 4. + offset as f32],
                    [r.x as f32 + 4., r.y as f32 + 12. + offset as f32],
                    c,
                    1,
                );
            }
            self.fill(
                Rect {
                    x: r.x + 5,
                    y: r.y + 2,
                    width: (r.width / 5).min(46),
                    height: 1,
                },
                c,
            );
        }
    }
    fn text(
        &mut self,
        x: f32,
        y: f32,
        text: &str,
        size: f32,
        c: [u8; 4],
        role: &str,
        max_width: f32,
    ) {
        let fonts = Fonts::get();
        let f = match role {
            "heading" => &fonts.heading,
            "numbers" => &fonts.numbers,
            _ => &fonts.body,
        };
        let ratio = size / f.meta.source_pixels;
        let cap = -f.glyph('A').top as f32 * ratio;
        let first_line = text.lines().next().unwrap_or("");
        let mut fitted = first_line.to_owned();
        if f.width(&fitted, size) > max_width {
            let suffix = "...";
            while !fitted.is_empty() && f.width(&format!("{fitted}{suffix}"), size) > max_width {
                fitted.pop();
            }
            // Prefer a whole final word when the remaining prefix is useful.
            if let Some(space) = fitted.rfind(' ') {
                if space > fitted.len() / 2 {
                    fitted.truncate(space);
                }
            }
            fitted = format!("{}{suffix}", fitted.trim_end());
        }
        let mut cursor = x;
        for ch in fitted.chars() {
            if ch == '\n' {
                break;
            }
            let g = f.glyph(ch);
            let advance = g.advance * ratio;
            if cursor + advance > x + max_width {
                break;
            }
            let dw = (g.width as f32 * ratio).ceil().max(1.) as u32;
            let dh = (g.height as f32 * ratio).ceil().max(1.) as u32;
            let left = cursor + g.left as f32 * ratio;
            let top = y + cap + g.top as f32 * ratio;
            for dy in 0..dh {
                for dx in 0..dw {
                    // Area-average real font coverage: fine stems survive
                    // scaling down instead of falling between nearest samples.
                    let x0 = (dx as f32 / ratio).min(g.width as f32);
                    let x1 = ((dx + 1) as f32 / ratio).min(g.width as f32);
                    let y0 = (dy as f32 / ratio).min(g.height as f32);
                    let y1 = ((dy + 1) as f32 / ratio).min(g.height as f32);
                    let mut coverage = 0.;
                    for yy in y0.floor() as u32..y1.ceil() as u32 {
                        for xx in x0.floor() as u32..x1.ceil() as u32 {
                            let weight = (x1.min(xx as f32 + 1.) - x0.max(xx as f32)).max(0.)
                                * (y1.min(yy as f32 + 1.) - y0.max(yy as f32)).max(0.);
                            coverage += f.alpha[((g.y + yy) * f.meta.width + g.x + xx) as usize]
                                as f32
                                * weight;
                        }
                    }
                    let a = (coverage / ((x1 - x0) * (y1 - y0)).max(0.001))
                        .round()
                        .clamp(0., 255.) as u8;
                    if a == 0 {
                        continue;
                    }
                    let mut color = c;
                    color[3] = (color[3] as u16 * a as u16 / 255) as u8;
                    self.px(
                        (left + dx as f32).round() as i32,
                        (top + dy as f32).round() as i32,
                        color,
                    );
                }
            }
            cursor += advance;
        }
    }
    fn wrap(
        &mut self,
        x: f32,
        mut y: f32,
        text: &str,
        size: f32,
        c: [u8; 4],
        max_width: f32,
        max_lines: usize,
    ) -> f32 {
        let f = &Fonts::get().body;
        let words: Vec<_> = text.split_whitespace().collect();
        let mut line = String::new();
        let mut n = 0;
        if max_lines == 0 {
            return y;
        }
        for (i, word) in words.iter().enumerate() {
            let candidate = if line.is_empty() {
                (*word).to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && f.width(&candidate, size) > max_width {
                if n + 1 >= max_lines {
                    // Keep the continuation in this last logical line so text()
                    // emits an ellipsis instead of silently dropping words.
                    self.text(
                        x,
                        y,
                        &format!("{line} {}", words[i..].join(" ")),
                        size,
                        c,
                        "body",
                        max_width,
                    );
                    return y + size * 1.35;
                }
                self.text(x, y, &line, size, c, "body", max_width);
                y += size * 1.35;
                n += 1;
                line.clear();
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        if !line.is_empty() && n < max_lines {
            self.text(x, y, &line, size, c, "body", max_width);
            y += size * 1.35;
        }
        y
    }
    fn icon(&mut self, r: Rect, key: &str, muted: bool) {
        if r.width == 0 || r.height == 0 {
            return;
        }
        if let Some(icon)=crate::ui_icons::get(key) {
            let size=if r.width>=32&&r.height>=32{32}else{16};
            let rows=if size==32{&icon.large}else{&icon.small};
            let fit=(r.width/size).min(r.height/size).max(1);
            let x=r.x+(r.width.saturating_sub(size*fit))/2;
            let y=r.y+(r.height.saturating_sub(size*fit))/2;
            for (yy,row) in rows.iter().enumerate(){for (xx,symbol) in row.bytes().enumerate(){
                if let Some(mut c)=crate::ui_icons::color(symbol){
                    if muted{let v=(c[0] as u16+c[1] as u16+c[2] as u16)/5;c=[v as u8,v as u8,v as u8,255];}
                    self.fill(Rect{x:x+xx as u32*fit,y:y+yy as u32*fit,width:fit,height:fit},c);
                }
            }}
            return;
        }
        let atlas = Atlas::get();
        let theme = Theme::global();
        let resolved = theme
            .resources
            .get(&key.to_ascii_lowercase())
            .map(|v| v.as_str())
            .unwrap_or(key);
        let (resolved, coat) = resolved.split_once('@')
            .filter(|(name, _)| name.starts_with("fauna:"))
            .map(|(name, coat)| (name, coat.parse::<u32>().ok().filter(|n|crate::natural_colors::valid_fauna_appearance(*n))))
            .unwrap_or((resolved, None));
        let Some(src) = atlas.rect(resolved) else {
            self.utility(r, key, theme.color("text"));
            return;
        };
        let fit = (r.width as f32 / src.width as f32).min(r.height as f32 / src.height as f32);
        let w = (src.width as f32 * fit).max(1.) as u32;
        let h = (src.height as f32 * fit).max(1.) as u32;
        let x = r.x + (r.width - w) / 2;
        let y = r.y + (r.height - h) / 2;
        for yy in 0..h {
            for xx in 0..w {
                let sx = src.x + (xx as f32 / fit).min(src.width.saturating_sub(1) as f32) as u32;
                let sy = src.y + (yy as f32 / fit).min(src.height.saturating_sub(1) as f32) as u32;
                let i = (sy * atlas.width + sx) as usize * 4;
                let mut c = [
                    atlas.rgba[i],
                    atlas.rgba[i + 1],
                    atlas.rgba[i + 2],
                    atlas.rgba[i + 3],
                ];
                if let Some(coat) = coat { c = crate::natural_colors::tint_fauna(c, coat); }
                if muted {
                    let v = (c[0] as u16 + c[1] as u16 + c[2] as u16) / 5;
                    c = [v as u8, v as u8, v as u8, c[3]];
                }
                self.px((x + xx) as i32, (y + yy) as i32, c);
            }
        }
    }
    fn utility(&mut self, r: Rect, key: &str, c: [u8; 4]) {
        let cx = r.x as f32 + r.width as f32 / 2.;
        let cy = r.y as f32 + r.height as f32 / 2.;
        let q = r.width.min(r.height) as f32 * 0.24;
        let n = (r.width as f32 / 24.).round().max(1.) as u32;
        match key {
            "cursor" => {
                self.line([cx - q, cy - q], [cx - q, cy + q], c, n);
                self.line([cx - q, cy - q], [cx + q, cy + q * 0.3], c, n);
                self.line([cx - q, cy + q], [cx, cy + q * 0.2], c, n);
                self.line([cx, cy + q * 0.2], [cx + q, cy + q * 0.3], c, n);
            }
            "close" => {
                self.line([cx - q, cy - q], [cx + q, cy + q], c, n);
                self.line([cx - q, cy + q], [cx + q, cy - q], c, n);
            }
            "pause" => {
                self.fill(
                    Rect {
                        x: (cx - q) as u32,
                        y: (cy - q) as u32,
                        width: (q * 0.6) as u32,
                        height: (2. * q) as u32,
                    },
                    c,
                );
                self.fill(
                    Rect {
                        x: (cx + q * 0.4) as u32,
                        y: (cy - q) as u32,
                        width: (q * 0.6) as u32,
                        height: (2. * q) as u32,
                    },
                    c,
                );
            }
            "pin" => {
                self.line([cx - q, cy - q], [cx + q, cy - q], c, n);
                self.line([cx, cy - q], [cx, cy + q], c, n);
                self.line([cx - q * 0.7, cy], [cx + q * 0.7, cy], c, n);
            }
            "collapse" => self.line([cx - q, cy], [cx + q, cy], c, n),
            "warning" => {
                self.line([cx, cy - q], [cx + q, cy + q], c, n);
                self.line([cx + q, cy + q], [cx - q, cy + q], c, n);
                self.line([cx - q, cy + q], [cx, cy - q], c, n);
                self.line([cx, cy - q * 0.2], [cx, cy + q * 0.4], c, n);
                self.fill(
                    Rect {
                        x: cx as u32,
                        y: (cy + q * 0.65) as u32,
                        width: n,
                        height: n,
                    },
                    c,
                );
            }
            "help" => self.text(cx - q * 0.5, cy - q, "?", q * 2.5, c, "heading", q * 2.),
            "settings" => {
                for i in 0..8 {
                    let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                    let d = [angle.cos(), angle.sin()];
                    self.line(
                        [cx + d[0] * q * 0.6, cy + d[1] * q * 0.6],
                        [cx + d[0] * q * 1.15, cy + d[1] * q * 1.15],
                        c,
                        n,
                    );
                }
                self.outline(
                    Rect {
                        x: (cx - q * 0.45) as u32,
                        y: (cy - q * 0.45) as u32,
                        width: (q * 0.9) as u32,
                        height: (q * 0.9) as u32,
                    },
                    c,
                    n,
                );
            }
            _ => {
                self.line([cx - q, cy + q * 0.6], [cx, cy - q], c, n);
                self.line([cx, cy - q], [cx + q, cy + q * 0.6], c, n);
                self.line([cx + q, cy + q * 0.6], [cx - q, cy + q * 0.6], c, n);
            }
        }
    }
    fn bar(&mut self, r: Rect, value: u16, c: [u8; 4]) {
        self.fill(r, Theme::global().color("black"));
        self.fill(
            Rect {
                width: r.width * value.min(1000) as u32 / 1000,
                ..r
            },
            c,
        );
    }
}
#[derive(Clone, PartialEq)]
enum Content {
    Top(Vec<ui::ResourceView>, String),
    Mini(ui::MinimapView),
    Selection(ui::SelectionView),
    Commands(CommandCategory, Option<usize>, Vec<String>,Vec<(usize,String)>),
    Layers(Vec<Layer>, Vec<(Layer, String)>),
    Quick(Option<usize>),
    Alert(ui::AlertView),
    Panel(
        ui::PanelView,
        [f32; 2],
        f32,
        String,
        Vec<(String, UiAction)>,
        Option<u32>,
    ),
    Modal(ui::ModalView),
    Tooltip(ui::TooltipView),
    Hint(String),
    Legacy(HudModel),
}
#[derive(Clone, PartialEq)]
struct Spec {
    id: u32,
    rect: Rect,
    scale: f32,
    content: Content,
}
struct CachedLayer {
    spec: Spec,
    pixels: Vec<u8>,
}
#[derive(Default)]
pub struct RasterCache {
    width: u32,
    height: u32,
    canvas: Vec<u8>,
    layers: BTreeMap<u32, CachedLayer>,
    pub total_layers_redrawn: u64,
    pub total_layers_reused: u64,
    pub updates: u64,
}
pub struct HudUpdate<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
    pub dirty: Vec<Rect>,
    pub regions: Vec<Rect>,
    pub layers_redrawn: u64,
    pub layers_reused: u64,
}
pub struct HudImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub bands: Vec<Range<u32>>,
    pub regions: Vec<Rect>,
}
fn merge_rects(mut rects: Vec<Rect>) -> Vec<Rect> {
    rects.retain(|r| r.width > 0 && r.height > 0);
    let mut out: Vec<Rect> = vec![];
    for r in rects {
        if let Some(i) = out.iter().position(|a| a.intersect(r).is_some()) {
            out[i] = out[i].union(r);
        } else {
            out.push(r);
        }
    }
    out
}
impl RasterCache {
    pub fn draw<'a>(&'a mut self, model: &HudModel, width: u32, height: u32) -> HudUpdate<'a> {
        let l = layout(model, width, height);
        let (w, h) = (l.width, l.height);
        let resized = w != self.width || h != self.height;
        let mut dirty = vec![];
        if resized {
            self.width = w;
            self.height = h;
            self.canvas = vec![0; w as usize * h as usize * 4];
            self.layers.clear();
            dirty.push(Rect {
                x: 0,
                y: 0,
                width: w,
                height: h,
            });
        }
        let specs = specs(model, &l);
        let ids: BTreeSet<_> = specs.iter().map(|s| s.id).collect();
        let old: Vec<_> = self
            .layers
            .keys()
            .copied()
            .filter(|k| !ids.contains(k))
            .collect();
        for id in old {
            if let Some(layer) = self.layers.remove(&id) {
                dirty.push(layer.spec.rect);
            }
        }
        let (mut redrawn, mut reused) = (0, 0);
        for spec in specs {
            if self
                .layers
                .get(&spec.id)
                .is_some_and(|old| old.spec == spec)
            {
                reused += 1;
                continue;
            }
            if let Some(old) = self.layers.get(&spec.id) {
                dirty.push(old.spec.rect);
            }
            dirty.push(spec.rect);
            let pixels = draw_spec(&spec, &l);
            self.layers.insert(spec.id, CachedLayer { spec, pixels });
            redrawn += 1;
        }
        let dirty = merge_rects(dirty);
        for region in &dirty {
            for y in region.y..region.end_y() {
                let start = (y as usize * w as usize + region.x as usize) * 4;
                self.canvas[start..start + region.width as usize * 4].fill(0);
            }
            for layer in self.layers.values() {
                let Some(inter) = region.intersect(layer.spec.rect) else {
                    continue;
                };
                for y in inter.y..inter.end_y() {
                    for x in inter.x..inter.end_x() {
                        let src = ((y - layer.spec.rect.y) as usize
                            * layer.spec.rect.width as usize
                            + (x - layer.spec.rect.x) as usize)
                            * 4;
                        let dst = (y as usize * w as usize + x as usize) * 4;
                        let c = &layer.pixels[src..src + 4];
                        if c[3] > 0 {
                            blend(&mut self.canvas[dst..dst + 4], [c[0], c[1], c[2], c[3]]);
                        }
                    }
                }
            }
        }
        self.updates += 1;
        self.total_layers_redrawn += redrawn;
        self.total_layers_reused += reused;
        HudUpdate {
            width: w,
            height: h,
            rgba: &self.canvas,
            dirty,
            regions: merge_rects(self.layers.values().map(|x| x.spec.rect).collect()),
            layers_redrawn: redrawn,
            layers_reused: reused,
        }
    }
}
pub fn rasterize(model: &HudModel, width: u32, height: u32) -> HudImage {
    let mut cache = RasterCache::default();
    let frame = cache.draw(model, width, height);
    HudImage {
        width: frame.width,
        height: frame.height,
        rgba: frame.rgba.to_vec(),
        bands: frame.regions.iter().map(|r| r.y..r.end_y()).collect(),
        regions: frame.regions.clone(),
    }
}
fn specs(model: &HudModel, l: &UiLayout) -> Vec<Spec> {
    let s = l.scale;
    let mut out = vec![];
    let add = |out: &mut Vec<Spec>, id: u32, rect: Rect, content: Content| {
        if rect.width > 0 && rect.height > 0 {
            out.push(Spec {
                id,
                rect,
                scale: s,
                content,
            });
        }
    };
    let Some(v) = &model.ui else {
        add(&mut out, 100, l.top, Content::Legacy(model.clone()));
        return out;
    };
    add(
        &mut out,
        100,
        l.top,
        Content::Top(
            v.resources.clone(),
            model.lines.get(2).cloned().unwrap_or_default(),
        ),
    );
    add(&mut out, 200, l.minimap, Content::Mini(v.minimap.clone()));
    add(
        &mut out,
        300,
        l.selection,
        Content::Selection(v.selection.clone()),
    );
    add(
        &mut out,
        400,
        l.command,
        Content::Commands(v.category, v.active_tool, model.tools.clone(),v.disabled_tools.clone()),
    );
    if let (Some((_, first)), Some((_, last))) = (l.layer_buttons.first(), l.layer_buttons.last()) {
        add(
            &mut out,
            210,
            first.union(*last),
            Content::Layers(v.layers.clone(), v.disabled_layers.clone()),
        );
    }
    if let (Some((_, first, _)), Some((_, last, _))) =
        (l.quick_buttons.first(), l.quick_buttons.last())
    {
        add(
            &mut out,
            410,
            first.union(*last),
            Content::Quick(v.active_tool),
        );
    }
    for (i, (a, r)) in l.alerts.iter().enumerate() {
        add(&mut out, 500 + i as u32, *r, Content::Alert(a.clone()));
    }
    if v.prefs.hints {
        if let Some(hint) = v.hints.last() {
            let w = (440. * s).min(l.width as f32 * 0.45);
            add(
                &mut out,
                600,
                rect(
                    (l.width as f32 - w) * 0.5,
                    l.top.end_y() as f32 + 8. * s,
                    w,
                    Theme::global().metric("hint_height") * s,
                    l.width,
                    l.height,
                ),
                Content::Hint(hint.clone()),
            );
        }
    }
    for (i, (panel, r)) in l.panels.iter().enumerate() {
        add(
            &mut out,
            1000 + i as u32 * 10 + panel.kind as u32,
            *r,
            Content::Panel(
                panel.clone(),
                v.food_web_pan,
                v.food_web_zoom,
                v.help_query.clone(),
                settings_actions(model),
                v.selected_species,
            ),
        );
    }
    if let Some(tooltip) = &v.tooltip {
        let w = (Theme::global().metric("tooltip_width") * s).min(l.width as f32 - 16.);
        let h = ((tooltip.lines.len().min(9) + 2) as f32 * 22. * s).min(l.height as f32 * 0.6);
        let x = (tooltip.anchor[0] + 16.)
            .min(l.width as f32 - w - 8.)
            .max(8.);
        let y = (tooltip.anchor[1] + 22.)
            .min(l.height as f32 - h - 8.)
            .max(8.);
        add(
            &mut out,
            2000,
            rect(x, y, w, h, l.width, l.height),
            Content::Tooltip(tooltip.clone()),
        );
    }
    if let Some(modal)=&v.modal {
        add(&mut out,3000,Rect{x:0,y:0,width:l.width,height:l.height},Content::Modal(modal.clone()));
    }
    out
}
fn modal_rects(l:&UiLayout)->(Rect,Rect) {
    let width=(760.*l.scale).max(560.).min(l.width as f32-32.);
    let height=(600.*l.scale).max(540.).min(l.height as f32-32.);
    let r=rect((l.width as f32-width)*0.5,(l.height as f32-height)*0.5,width,height,l.width,l.height);
    let b=rect(r.x as f32+16.,r.end_y() as f32-60.,r.width as f32-32.,44.,l.width,l.height);
    (r,b)
}
fn draw_spec(spec: &Spec, l: &UiLayout) -> Vec<u8> {
    let mut p = Paint::new(spec.rect);
    let r = spec.rect;
    let s = spec.scale;
    let t = Theme::global();
    let text = t.color("text");
    let muted = t.color("muted");
    let accent = t.color("faction0");
    let green = t.color("biology");
    let body = (t.metric("body_px") * s).max(14.);
    let small = (t.metric("small_px") * s).max(14.);
    let heading = (t.metric("heading_px") * s).max(18.);
    match &spec.content {
        Content::Modal(modal)=>{
            p.fill(r,[0,0,0,180]);
            let (card,button)=modal_rects(l);
            p.metal(card,false,true);
            let x=card.x as f32+20.; let width=card.width as f32-40.;
            let mut y=p.wrap(x,card.y as f32+18.,&modal.title,heading,accent,width,3)+16.;
            let font=body.min(18.).max(14.);
            for line in &modal.lines {
                if y+font*2.7>button.y as f32-8. {break;}
                y=p.wrap(x,y,line,font,text,width,3)+10.;
            }
            p.metal(button,true,true);
            p.text(button.x as f32+12.,button.y as f32+12.,&modal.button,body,text,"body",button.width as f32-24.);
        }
        Content::Top(resources, clock) => {
            p.metal(l.top_resources, false, true);
            p.metal(l.top_controls, false, false);
            p.text(
                r.x as f32 + 12. * s,
                r.y as f32 + 10. * s,
                "LAST",
                heading,
                accent,
                "heading",
                74. * s,
            );
            p.text(
                r.x as f32 + 12. * s,
                r.y as f32 + 28. * s,
                "WORLD",
                small,
                text,
                "heading",
                74. * s,
            );
            for (i, res) in resources.iter().enumerate() {
                if let Some(rr) = l.resource_buttons.get(i) {
                    p.icon(
                        rect(
                            rr.x as f32 + 4. * s,
                            rr.y as f32 + 12. * s,
                            32. * s,
                            32. * s,
                            l.width,
                            l.height,
                        ),
                        &res.icon,
                        false,
                    );
                    p.text(
                        rr.x as f32 + 42. * s,
                        rr.y as f32 + 4. * s,
                        &res.name,
                        small,
                        muted,
                        "body",
                        rr.width as f32 - 44. * s,
                    );
                    p.text(
                        rr.x as f32 + 42. * s,
                        rr.y as f32 + 21. * s,
                        &res.value,
                        (t.metric("resource_px") * s).max(16.),
                        if res.warning.is_some() {
                            t.color("warning")
                        } else {
                            text
                        },
                        "numbers",
                        rr.width as f32 - 44. * s,
                    );
                    p.text(
                        rr.x as f32 + 42. * s,
                        rr.y as f32 + 40. * s,
                        &res.trend,
                        small,
                        if res.trend.starts_with('-') {
                            t.color("damage")
                        } else {
                            green
                        },
                        "body",
                        rr.width as f32 - 44. * s,
                    );
                }
            }
            let controls = l
                .targets
                .iter()
                .filter_map(|a| match &a.hit {
                    UiHit::Action(UiAction::Speed(n)) => Some((a.rect, format!("{}×", n))),
                    UiHit::Action(UiAction::Pause) => Some((a.rect, "pause".into())),
                    UiHit::Action(UiAction::OpenPanel(PanelKind::Help)) => {
                        Some((a.rect, "help".into()))
                    }
                    UiHit::Action(UiAction::OpenPanel(PanelKind::Settings)) => {
                        Some((a.rect, "settings".into()))
                    }
                    _ => None,
                })
                .take(6);
            for (rr, label) in controls {
                if label.ends_with('×') {
                    p.text(
                        rr.x as f32 + 8. * s,
                        rr.y as f32 + 14. * s,
                        &label,
                        heading,
                        if clock.starts_with(label.trim_end_matches('×')) {
                            accent
                        } else {
                            muted
                        },
                        "numbers",
                        rr.width as f32 - 8. * s,
                    );
                } else {
                    p.utility(rr.inset((10. * s) as u32), &label, text);
                }
            }
        }
        Content::Commands(category, active, names, disabled) => {
            p.metal(r, false, true);
            p.text(
                r.x as f32 + 12. * s,
                r.y as f32 + 4. * s,
                &format!("COMMAND / {}", category.name()),
                heading,
                accent,
                "heading",
                r.width as f32 - 24. * s,
            );
            for (cat, rr) in &l.category_buttons {
                p.metal(*rr, true, cat == category);
                let label = match cat {
                    CommandCategory::Economy => "Econ",
                    CommandCategory::Defense => "Def",
                    CommandCategory::Ecology => "Life",
                    _ => t
                        .categories
                        .get(cat.name())
                        .map(|x| x.label.as_str())
                        .unwrap_or(cat.name()),
                };
                let key = match cat {
                    CommandCategory::Build => "B",
                    CommandCategory::Economy => "Ctrl+E",
                    CommandCategory::Defense => "Ctrl+D",
                    CommandCategory::Recon => "Ctrl+R",
                    CommandCategory::Naval => "N",
                    CommandCategory::Ecology => "",
                };
                p.text(
                    rr.x as f32 + 5. * s,
                    rr.y as f32 + 6. * s,
                    label,
                    small,
                    if cat == category { accent } else { text },
                    "body",
                    rr.width as f32 - 8. * s,
                );
                p.text(
                    rr.x as f32 + 5. * s,
                    rr.y as f32 + 25. * s,
                    key,
                    small,
                    muted,
                    "body",
                    rr.width as f32 - 8. * s,
                );
            }
            for (index, rr) in &l.tool_buttons {
                p.metal(*rr, true, *active == Some(*index));
                let reference = t.tool(*index);
                let icon = ui::tool_icon(*index);
                let icon_r = rect(
                    rr.x as f32 + 7. * s,
                    rr.y as f32 + 4. * s,
                    (rr.width as f32 - 14. * s).min(54. * s),
                    (rr.height as f32 - 26. * s).max(20.),
                    l.width,
                    l.height,
                );
                p.icon(icon_r, icon, disabled.iter().any(|(tool,_)|tool==index));
                if let Some(key) = reference
                    .map(|x| x.hotkey.as_str())
                    .filter(|x| !x.is_empty())
                {
                    p.text(
                        rr.end_x() as f32 - 28. * s,
                        rr.y as f32 + 6. * s,
                        key,
                        small,
                        text,
                        "body",
                        26. * s,
                    );
                }
                let label = ui::tool_label(*index);
                p.text(
                    rr.x as f32 + 6. * s,
                    rr.end_y() as f32 - 20. * s,
                    label,
                    small,
                    text,
                    "body",
                    rr.width as f32 - 12. * s,
                );
            }
        }
        Content::Quick(active) => {
            for (icon, rr, action) in &l.quick_buttons {
                let selected = match action {
                    UiAction::SelectMode => active.is_none(),
                    UiAction::ChooseTool(i) => *active == Some(*i),
                    _ => false,
                };
                let (label, key) = match action {
                    UiAction::SelectMode => ("Select", "Esc"),
                    UiAction::ChooseTool(0) => ("Road", "F4"),
                    UiAction::ChooseTool(10) => ("Pole", "F7"),
                    UiAction::ChooseCategory(_) => ("Build", "B"),
                    _ => ("Ecology", "E"),
                };
                p.metal(*rr, true, selected);
                let ir = rect(
                    rr.x as f32 + 6. * s,
                    rr.y as f32 + 4. * s,
                    34. * s,
                    (rr.height as f32 - 25. * s).min(34. * s),
                    l.width,
                    l.height,
                );
                p.icon(ir, icon, false);
                p.text(
                    rr.end_x() as f32 - 31. * s,
                    rr.y as f32 + 7. * s,
                    key,
                    small,
                    muted,
                    "body",
                    29. * s,
                );
                p.text(
                    rr.x as f32 + 5. * s,
                    rr.end_y() as f32 - 20. * s,
                    label,
                    small,
                    text,
                    "body",
                    rr.width as f32 - 10. * s,
                );
            }
        }
        Content::Layers(layers, disabled) => {
            for (layer, rr) in &l.layer_buttons {
                let locked = disabled.iter().any(|(disabled, _)| disabled == layer);
                p.metal(*rr, true, layers.contains(layer) && !locked);
                let icon = t
                    .layers
                    .get(layer.name())
                    .map(|x| x.as_str())
                    .unwrap_or("radar");
                p.icon(
                    rr.inset((9. * s) as u32),
                    icon,
                    locked || !layers.contains(layer),
                );
                if locked {
                    for n in 0..5 {
                        let x = rr.x as f32 + n as f32 * rr.width as f32 / 5.;
                        p.line(
                            [x, rr.y as f32],
                            [x + rr.width as f32 / 4., rr.end_y() as f32],
                            t.color("disabled"),
                            1,
                        );
                    }
                }
            }
        }
        Content::Mini(map) => {
            p.metal(r, false, true);
            p.text(
                r.x as f32 + 10. * s,
                r.y as f32 + 5. * s,
                "SECTOR",
                heading,
                accent,
                "heading",
                r.width as f32 - 20. * s,
            );
            let inner = l.minimap_inner;
            p.clip = Some(inner);
            p.fill(inner, t.color("fog"));
            if map.width > 0
                && map.height > 0
                && map.rgba.len() == map.width as usize * map.height as usize * 4
            {
                for yy in 0..inner.height {
                    for xx in 0..inner.width {
                        let nx = (xx as f32 / inner.width as f32 - 0.5)
                            / (map.zoom_permille.max(1) as f32 / 1000.)
                            + map.center[0];
                        let ny = (yy as f32 / inner.height as f32 - 0.5)
                            / (map.zoom_permille.max(1) as f32 / 1000.)
                            + map.center[1];
                        if !(0.0..1.0).contains(&nx) || !(0.0..1.0).contains(&ny) {
                            continue;
                        }
                        let sx = (nx * map.width as f32) as u32;
                        let sy = (ny * map.height as f32) as u32;
                        let i = (sy * map.width + sx) as usize * 4;
                        p.px(
                            (inner.x + xx) as i32,
                            (inner.y + yy) as i32,
                            [map.rgba[i], map.rgba[i + 1], map.rgba[i + 2], 255],
                        );
                    }
                }
            }
            let at = |pos: [f32; 2]| -> [f32; 2] {
                [
                    inner.x as f32
                        + ((pos[0] - map.center[0]) * (map.zoom_permille as f32 / 1000.) + 0.5)
                            * inner.width as f32,
                    inner.y as f32
                        + ((pos[1] - map.center[1]) * (map.zoom_permille as f32 / 1000.) + 0.5)
                            * inner.height as f32,
                ]
            };
            for m in &map.markers {
                let pt = at(m.position);
                if pt[0] < inner.x as f32
                    || pt[1] < inner.y as f32
                    || pt[0] >= inner.end_x() as f32
                    || pt[1] >= inner.end_y() as f32
                {
                    continue;
                }
                let c = match m.team {
                    Some(0) => accent,
                    Some(_) => t.color("faction1"),
                    None => muted,
                };
                let size = if m.selected { 4 } else { 2 };
                let rr = Rect {
                    x: pt[0] as u32,
                    y: pt[1] as u32,
                    width: size,
                    height: size,
                };
                if m.kind == "alert" {
                    p.utility(
                        Rect {
                            x: pt[0] as u32,
                            y: pt[1] as u32,
                            width: 10,
                            height: 10,
                        },
                        "warning",
                        t.color("warning"),
                    );
                } else if m.stale {
                    p.outline(
                        Rect {
                            width: 4,
                            height: 4,
                            ..rr
                        },
                        c,
                        1,
                    )
                } else {
                    p.fill(rr, c)
                }
            }
            let a = at([map.camera.x, map.camera.y]);
            let b = at([
                map.camera.x + map.camera.width,
                map.camera.y + map.camera.height,
            ]);
            p.outline(
                rect(a[0], a[1], b[0] - a[0], b[1] - a[1], l.width, l.height),
                accent,
                1,
            );
            if let Some(sat) = map.satellite {
                let center = at([sat[0], sat[1]]);
                let rx = sat[2] * inner.width as f32 * map.zoom_permille as f32 / 1000.;
                let ry = sat[2] * 1.5 * inner.height as f32 * map.zoom_permille as f32 / 1000.;
                for i in 0..64 {
                    let a = i as f32 * std::f32::consts::TAU / 64.;
                    let b = (i + 1) as f32 * std::f32::consts::TAU / 64.;
                    p.line(
                        [center[0] + a.cos() * rx, center[1] + a.sin() * ry],
                        [center[0] + b.cos() * rx, center[1] + b.sin() * ry],
                        green,
                        1,
                    );
                }
            }
            p.clip = None;
            p.line(
                [r.end_x() as f32 - 13., r.end_y() as f32 - 3.],
                [r.end_x() as f32 - 3., r.end_y() as f32 - 13.],
                muted,
                1,
            );
        }
        Content::Selection(sel) => {
            p.metal(r, false, true);
            p.text(
                r.x as f32 + 12. * s,
                r.y as f32 + 6. * s,
                &sel.title,
                heading,
                accent,
                "heading",
                r.width as f32 - 24. * s,
            );
            if sel.selected_count == 0 {
                p.text(
                    r.x as f32 + 12. * s,
                    r.y as f32 + 30. * s,
                    &sel.subtitle,
                    small,
                    muted,
                    "body",
                    r.width as f32 - 24. * s,
                );
            } else {
                let portrait = rect(
                    r.x as f32 + 10. * s,
                    r.y as f32 + 32. * s,
                    68. * s,
                    68. * s,
                    l.width,
                    l.height,
                );
                p.metal(portrait, true, false);
                p.icon(portrait.inset((8. * s) as u32), &sel.portrait, false);
                let tx = portrait.end_x() as f32 + 10. * s;
                let avail = r.end_x() as f32 - tx - 10. * s;
                p.text(
                    tx,
                    r.y as f32 + 34. * s,
                    &sel.subtitle,
                    body,
                    text,
                    "body",
                    avail,
                );
                if let Some(health) = sel.health_permille {
                    p.bar(
                        rect(tx, r.y as f32 + 58. * s, avail, 5. * s, l.width, l.height),
                        health,
                        green,
                    );
                }
                for (i, line) in sel.stats.iter().take(3).enumerate() {
                    p.text(
                        tx,
                        r.y as f32 + 70. * s + i as f32 * 20. * s,
                        line,
                        small,
                        muted,
                        "body",
                        avail,
                    );
                }
            }
            for (label,rr,_,enabled) in &l.selection_actions {
                p.metal(*rr,true,false);
                p.text(rr.x as f32+5.,rr.y as f32+8.,label,small,if *enabled{text}else{muted},"body",rr.width as f32-10.);
            }
            let qy = if let Some((_,rr,_,_))=l.selection_actions.first() {rr.y as f32-44.*s} else if sel.selected_count == 0 {
                r.end_y() as f32 - 84. * s
            } else {
                r.end_y() as f32 - 132. * s
            };
            for (i, q) in sel.queue.iter().take(7).enumerate() {
                let rr = rect(
                    r.x as f32 + 10. * s + i as f32 * 46. * s,
                    qy,
                    36. * s,
                    36. * s,
                    l.width,
                    l.height,
                );
                if rr.end_x() > r.end_x() {
                    break;
                }
                p.metal(rr, true, false);
                p.icon(rr.inset((5. * s) as u32), &q.icon, false);
                p.bar(
                    Rect {
                        x: rr.x + 2,
                        y: rr.end_y().saturating_sub(5),
                        width: rr.width.saturating_sub(4),
                        height: 3,
                    },
                    q.progress_permille,
                    if q.warning.is_some() {
                        t.color("warning")
                    } else {
                        accent
                    },
                );
                if q.warning.is_some() {
                    p.utility(
                        Rect {
                            x: rr.end_x().saturating_sub(16),
                            y: rr.y,
                            width: 16,
                            height: 16,
                        },
                        "warning",
                        t.color("warning"),
                    );
                }
            }
        }
        Content::Alert(a) => {
            p.metal(r, true, true);
            p.icon(
                rect(
                    r.x as f32 + 8. * s,
                    r.y as f32 + 5. * s,
                    34. * s,
                    34. * s,
                    l.width,
                    l.height,
                ),
                &a.icon,
                false,
            );
            let color = if a.priority >= 2 {
                t.color("damage")
            } else {
                t.color("warning")
            };
            p.wrap(
                r.x as f32 + 50. * s,
                r.y as f32 + 9. * s,
                &a.text,
                small,
                color,
                r.width as f32 - 94. * s,
                2,
            );
            p.utility(
                rect(
                    r.end_x() as f32 - 36. * s,
                    r.y as f32 + 8. * s,
                    28. * s,
                    28. * s,
                    l.width,
                    l.height,
                ),
                "close",
                muted,
            );
        }
        Content::Hint(hint) => {
            let mut c = t.color("recess");
            c[3] = 215;
            p.fill(r, c);
            p.wrap(
                r.x as f32 + 8. * s,
                r.y as f32 + 6. * s,
                hint,
                small,
                muted,
                r.width as f32 - 16. * s,
                2,
            );
        }
        Content::Tooltip(tip) => {
            p.metal(r, true, true);
            p.text(
                r.x as f32 + 12. * s,
                r.y as f32 + 10. * s,
                &tip.title,
                heading,
                accent,
                "heading",
                r.width as f32 - 24. * s,
            );
            let mut y = r.y as f32 + 40. * s;
            for line in tip.lines.iter().take(8) {
                y = p.wrap(
                    r.x as f32 + 12. * s,
                    y,
                    line,
                    body,
                    text,
                    r.width as f32 - 24. * s,
                    3,
                );
                if y + body > r.end_y() as f32 {
                    break;
                }
            }
        }
        Content::Panel(panel, pan, zoom, query, settings, selected) => draw_panel(
            &mut p, panel, r, s, *pan, *zoom, query, settings, *selected, l,
        ),
        Content::Legacy(model) => {
            p.metal(r, false, false);
            p.text(
                r.x as f32 + 10.,
                r.y as f32 + 10.,
                model
                    .lines
                    .first()
                    .map(|x| x.as_str())
                    .unwrap_or("LAST WORLD"),
                body,
                text,
                "body",
                r.width as f32 - 20.,
            );
        }
    }
    p.rgba
}
fn draw_panel(
    p: &mut Paint,
    panel: &ui::PanelView,
    r: Rect,
    s: f32,
    pan: [f32; 2],
    zoom: f32,
    query: &str,
    settings: &[(String, UiAction)],
    selected: Option<u32>,
    l: &UiLayout,
) {
    let t = Theme::global();
    let text = t.color("text");
    let muted = t.color("muted");
    let accent = t.color("faction0");
    let green = t.color("biology");
    let body = (t.metric("body_px") * s).max(14.);
    let small = (t.metric("small_px") * s).max(14.);
    let heading = (t.metric("heading_px") * s).max(18.);
    let hit = (t.metric("hit") * s).max(44.);
    p.metal(r, false, true);
    if let Some(icon) = t.panels.get(panel.kind.name()) {
        p.icon(
            rect(
                r.x as f32 + 8. * s,
                r.y as f32 + 8. * s,
                26. * s,
                26. * s,
                l.width,
                l.height,
            ),
            icon,
            false,
        );
    }
    p.text(
        r.x as f32 + 42. * s,
        r.y as f32 + 12. * s,
        &panel.title,
        heading,
        accent,
        "heading",
        r.width as f32 - hit * 3. - 20. * s,
    );
    for (i, key) in ["close", "pin", "collapse"].into_iter().enumerate() {
        let rr = rect(
            r.end_x() as f32 - (i + 1) as f32 * hit,
            r.y as f32,
            hit,
            hit,
            l.width,
            l.height,
        );
        p.utility(
            rr.inset((11. * s) as u32),
            key,
            if key == "pin" && panel.pinned {
                accent
            } else {
                muted
            },
        );
    }
    if panel.collapsed {
        return;
    }
    let mut y = r.y as f32 + hit + 10. * s;
    if panel.kind == PanelKind::Settings {
        for (label, _) in settings.iter().skip(panel.scroll as usize) {
            if y + hit > r.end_y() as f32 - 8. * s {
                break;
            }
            let rr = rect(
                r.x as f32 + 12. * s,
                y,
                r.width as f32 - 24. * s,
                hit,
                l.width,
                l.height,
            );
            p.metal(rr, true, false);
            p.text(
                rr.x as f32 + 10. * s,
                rr.y as f32 + 12. * s,
                label,
                body,
                text,
                "body",
                rr.width as f32 - 20. * s,
            );
            y += hit + 4. * s;
        }
        return;
    }
    if panel.kind == PanelKind::Help {
        let search = rect(
            r.x as f32 + 12. * s,
            y,
            r.width as f32 - 24. * s,
            hit,
            l.width,
            l.height,
        );
        p.metal(search, true, true);
        p.text(
            search.x as f32 + 10. * s,
            search.y as f32 + 12. * s,
            if query.is_empty() {
                "Search field manual..."
            } else {
                query
            },
            body,
            if query.is_empty() { muted } else { text },
            "body",
            search.width as f32 - hit - 12. * s,
        );
        p.utility(
            rect(
                search.end_x() as f32 - hit,
                search.y as f32,
                hit,
                hit,
                l.width,
                l.height,
            )
            .inset((12. * s) as u32),
            "close",
            muted,
        );
        y += hit + 10. * s;
    }
    for line in panel.lines.iter().take(5) {
        y = p.wrap(
            r.x as f32 + 12. * s,
            y,
            line,
            body,
            muted,
            r.width as f32 - 24. * s,
            2,
        );
        if y > r.end_y() as f32 - 40. * s {
            break;
        }
    }
    if panel.kind == PanelKind::FoodWeb {
        let mut view = UiPresentation {
            food_web_pan: pan,
            food_web_zoom: zoom,
            ..Default::default()
        };
        view.panels.push(panel.clone());
        let nodes = graph_nodes(panel, r, &view, s);
        let positions: BTreeMap<_, _> = nodes
            .iter()
            .map(|(e, n)| {
                (
                    e.id,
                    [
                        n.x as f32 + n.width as f32 / 2.,
                        n.y as f32 + n.height as f32 / 2.,
                    ],
                )
            })
            .collect();
        for (entry, _) in &nodes {
            if let Some(a) = positions.get(&entry.id) {
                for id in &entry.relations {
                    if let Some(b) = positions.get(id) {
                        p.line(*a, *b, t.color("edge"), (s.round() as u32).max(1));
                        let dx = b[0] - a[0];
                        let dy = b[1] - a[1];
                        let len = dx.hypot(dy).max(1.);
                        let mid = [a[0] + dx * 0.62, a[1] + dy * 0.62];
                        p.line(
                            [
                                mid[0] - dx / len * 6. * s - dy / len * 3. * s,
                                mid[1] - dy / len * 6. * s + dx / len * 3. * s,
                            ],
                            mid,
                            green,
                            1,
                        );
                        p.line(
                            [
                                mid[0] - dx / len * 6. * s + dy / len * 3. * s,
                                mid[1] - dy / len * 6. * s - dx / len * 3. * s,
                            ],
                            mid,
                            green,
                            1,
                        );
                    }
                }
            }
        }
        for (entry, nr) in nodes {
            p.metal(nr, true, selected == Some(entry.id as u32));
            let icon = rect(
                nr.x as f32 + nr.width as f32 * 0.3,
                nr.y as f32 + 5. * s,
                nr.width as f32 * 0.4,
                nr.height as f32 * 0.53,
                l.width,
                l.height,
            );
            entry_icon(p, entry, icon);
            p.text(
                nr.x as f32 + 5. * s,
                nr.end_y() as f32 - 18. * s,
                &entry.title,
                small,
                text,
                "body",
                nr.width as f32 - 10. * s,
            );
        }
        for (i, label) in ["-", "+"].into_iter().enumerate() {
            let rr = rect(
                r.x as f32 + 10. * s + i as f32 * (hit + 4. * s),
                r.end_y() as f32 - hit - 8. * s,
                hit,
                hit,
                l.width,
                l.height,
            );
            p.metal(rr, true, false);
            p.text(
                rr.x as f32 + 12. * s,
                rr.y as f32 + 8. * s,
                label,
                heading,
                text,
                "heading",
                rr.width as f32 - 12. * s,
            );
        }
        return;
    }
    let rows = panel_rows(panel, r, s);
    for (entry, rr) in &rows {
        p.metal(*rr, true, selected == Some(entry.id as u32));
        let is_ecology = panel.kind == PanelKind::Ecology;
        let icon = rect(
            rr.x as f32 + 6. * s,
            rr.y as f32 + 8. * s,
            36. * s,
            36. * s,
            l.width,
            l.height,
        );
        entry_icon(p, entry, icon);
        let tx = if is_ecology {
            rr.x as f32 + 5. * s
        } else {
            rr.x as f32 + 50. * s
        };
        let ty = if is_ecology {
            rr.y as f32 + 45. * s
        } else {
            rr.y as f32 + 8. * s
        };
        p.text(
            tx,
            ty,
            &entry.title,
            small,
            text,
            "body",
            rr.end_x() as f32 - tx - 6. * s,
        );
        if !is_ecology {
            p.text(
                tx,
                ty + 20. * s,
                &entry.subtitle,
                small,
                muted,
                "body",
                rr.end_x() as f32 - tx - 6. * s,
            );
            if let Some(detail) = entry.details.first() {
                p.text(
                    tx,
                    ty + 39. * s,
                    detail,
                    small,
                    muted,
                    "body",
                    rr.end_x() as f32 - tx - 6. * s,
                );
            }
        }
        if let Some(progress) = entry.progress_permille {
            p.bar(
                Rect {
                    x: rr.x + 4,
                    y: rr.end_y().saturating_sub(5),
                    width: rr.width.saturating_sub(8),
                    height: 3,
                },
                progress,
                green,
            );
        }
    }
    if panel.kind == PanelKind::Ecology {
        if let Some(entry) = panel
            .entries
            .iter()
            .find(|e| Some(e.id as u32) == selected)
            .or_else(|| panel.entries.first())
        {
            let left = r.x as f32 + (r.width as f32 * 0.38).max(110. * s) + 24. * s;
            let width = r.end_x() as f32 - left - 12. * s;
            let mut yy = r.y as f32 + hit + 12. * s;
            entry_icon(
                p,
                entry,
                rect(left, yy, 64. * s, 64. * s, l.width, l.height),
            );
            p.text(
                left + 74. * s,
                yy + 4. * s,
                &entry.title,
                heading,
                green,
                "heading",
                width - 74. * s,
            );
            p.wrap(
                left + 74. * s,
                yy + 30. * s,
                &entry.subtitle,
                small,
                muted,
                width - 74. * s,
                2,
            );
            yy += 78. * s;
            if let Some(progress) = entry.progress_permille {
                p.bar(
                    rect(left, yy, width, 5. * s, l.width, l.height),
                    progress,
                    green,
                );
                yy += 16. * s;
            }
            for line in &entry.details {
                yy = p.wrap(left, yy, line, body, text, width, 4);
                yy += 8. * s;
                if yy + body > r.end_y() as f32 - 12. * s {
                    break;
                }
            }
        }
    }
}

/// Small UI geometry is instanced directly on the existing GPU. It never
/// becomes physical artwork or a per-unit text/rasterization workload.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiQuad {
    pub rect: [f32; 4],
    pub color: [f32; 4],
}
pub fn world_quads(model: &HudModel, width: u32, height: u32) -> Vec<UiQuad> {
    let Some(v) = &model.ui else {
        return vec![];
    };
    let (w, h) = canvas_size(width, height);
    let t = Theme::global();
    let mut out = Vec::new();
    let mut add = |r: UiRect, c: [u8; 4]| {
        if ![r.x, r.y, r.width, r.height].iter().all(|n| n.is_finite())
            || r.width <= 0.
            || r.height <= 0.
        {
            return;
        }
        if r.x + r.width < 0. || r.y + r.height < 0. || r.x > w as f32 || r.y > h as f32 {
            return;
        }
        out.push(UiQuad {
            rect: [r.x, r.y, r.width, r.height],
            color: c.map(|n| n as f32 / 255.),
        });
    };
    for cell in &v.overlay_cells {
        add(cell.rect, cell.color);
    }
    for m in &v.world_marks {
        if !m.selected && !m.hovered {
            continue;
        }
        let r = m.radius.max(3.);
        let c = t.color(if m.team == 0 { "faction0" } else { "faction1" });
        let x = m.screen[0] - r;
        let y = m.screen[1] - r;
        let edge = (2. * scale(v, h)).clamp(1., 4.);
        for rr in [
            UiRect {
                x,
                y,
                width: r * 2.,
                height: edge,
            },
            UiRect {
                x,
                y: y + r * 2. - edge,
                width: r * 2.,
                height: edge,
            },
            UiRect {
                x,
                y,
                width: edge,
                height: r * 2.,
            },
            UiRect {
                x: x + r * 2. - edge,
                y,
                width: edge,
                height: r * 2.,
            },
        ] {
            add(rr, c);
        }
        if let Some(health) = m.health_permille {
            let bar = UiRect {
                x,
                y: y - 6.,
                width: r * 2.,
                height: 3.,
            };
            add(bar, t.color("black"));
            add(
                UiRect {
                    width: bar.width * health.min(1000) as f32 / 1000.,
                    ..bar
                },
                t.color("biology"),
            );
        }
    }
    if let Some(r) = v.selection_box {
        let c = t.color("faction0");
        for rr in [
            UiRect { height: 1., ..r },
            UiRect {
                y: r.y + r.height - 1.,
                height: 1.,
                ..r
            },
            UiRect { width: 1., ..r },
            UiRect {
                x: r.x + r.width - 1.,
                width: 1.,
                ..r
            },
        ] {
            add(rr, c);
        }
    }
    out
}

fn entry_icon(p: &mut Paint, entry: &ui::PanelEntry, r: Rect) {
    if let Some(image) = &entry.preview {
        if image.width > 0
            && image.height > 0
            && image.rgba.len() == image.width as usize * image.height as usize * 4
            && r.width > 0
            && r.height > 0
        {
            for y in 0..r.height {
                for x in 0..r.width {
                    let sx = x * image.width / r.width;
                    let sy = y * image.height / r.height;
                    let i = (sy * image.width + sx) as usize * 4;
                    p.px(
                        (r.x + x) as i32,
                        (r.y + y) as i32,
                        [
                            image.rgba[i],
                            image.rgba[i + 1],
                            image.rgba[i + 2],
                            image.rgba[i + 3],
                        ],
                    );
                }
            }
            return;
        }
    }
    if entry.icon == "flora" {
        let c = Theme::global().color("biology");
        let x = r.x as f32;
        let y = r.y as f32;
        let w = r.width as f32;
        let h = r.height as f32;
        let points = [
            [x + w * 0.2, y + h * 0.85],
            [x + w * 0.1, y + h * 0.45],
            [x + w * 0.4, y + h * 0.2],
            [x + w * 0.9, y + h * 0.1],
            [x + w * 0.85, y + h * 0.6],
            [x + w * 0.5, y + h * 0.9],
            [x + w * 0.2, y + h * 0.85],
        ];
        for pair in points.windows(2) {
            p.line(pair[0], pair[1], c, 1);
        }
        p.line(
            [x + w * 0.2, y + h * 0.85],
            [x + w * 0.8, y + h * 0.2],
            c,
            1,
        );
    } else {
        p.icon(r, &entry.icon, entry.subtitle.contains("incomplete"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct UiFixture(PathBuf);
    impl UiFixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "last-world-ui-paths-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn install(&self, name: &str) -> PathBuf {
            let directory = self.0.join(name);
            let ui = directory.join("ui");
            std::fs::create_dir_all(&ui).unwrap();
            std::fs::write(
                ui.join("theme.json"),
                include_str!("../assets/ui/theme.json"),
            )
            .unwrap();
            for (role, image, meta) in font_sources() {
                std::fs::write(ui.join(format!("{role}-atlas.png")), image).unwrap();
                std::fs::write(ui.join(format!("{role}-atlas.json")), meta).unwrap();
            }
            directory
        }
    }
    impl Drop for UiFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn font_sources() -> [(&'static str, &'static [u8], &'static str); 3] {
        [
            (
                "body",
                include_bytes!("../assets/ui/body-atlas.png"),
                include_str!("../assets/ui/body-atlas.json"),
            ),
            (
                "numbers",
                include_bytes!("../assets/ui/numbers-atlas.png"),
                include_str!("../assets/ui/numbers-atlas.json"),
            ),
            (
                "heading",
                include_bytes!("../assets/ui/heading-atlas.png"),
                include_str!("../assets/ui/heading-atlas.json"),
            ),
        ]
    }
    #[test]
    fn ui_asset_roots_prioritize_data_and_absolute_app_or_executable_defaults() {
        let root = std::env::temp_dir().join("last-world-ui-resolution");
        let data = root.join("Data");
        let app = root.join("App");
        let executable = app.join("last-world.exe");
        let supplied_app = root.join("AlternateApp");
        assert_eq!(
            UiAssets::new(&data, Some(&supplied_app), Some(&executable)).roots,
            vec![data.join("ui"), supplied_app.join("ui")]
        );
        assert_eq!(
            UiAssets::new(&data, Some(Path::new("relative")), Some(&executable)).roots,
            vec![data.join("ui"), app.join("ui")]
        );
        assert_eq!(
            UiAssets::new(&app, None, Some(&executable)).roots,
            vec![app.join("ui")]
        );
    }
    #[test]
    fn actual_theme_and_all_licensed_glyphs_match_across_data_app_and_embedded_roots() {
        let fixture = UiFixture::new();
        let data = fixture.install("Data");
        let app = fixture.install("App");
        let missing = fixture.0.join("Missing");
        let roots = [
            UiAssets::new(&data, Some(&app), None),
            UiAssets::new(&missing, Some(&app), None),
            UiAssets::new(&missing, None, None),
        ];
        let (bundled, invalid) = Theme::load(&roots[2]);
        assert!(!invalid);
        for assets in &roots {
            let (theme, invalid) = Theme::load(assets);
            assert!(!invalid);
            assert_eq!(theme, bundled);
            for (role, image, meta) in font_sources() {
                let (expected, _) = Font::load_from_assets(&bundled, &roots[2], role, image, meta);
                let (loaded, invalid) = Font::load_from_assets(&theme, assets, role, image, meta);
                assert!(!invalid);
                assert_eq!(loaded, expected, "{role} glyph metadata and atlas pixels");
                assert_eq!(
                    loaded.width("LAST WORLD 0123456789?", 18.),
                    expected.width("LAST WORLD 0123456789?", 18.)
                );
            }
        }
    }
    #[test]
    fn data_ui_overrides_win_and_invalid_theme_or_glyphs_fall_back_per_root() {
        let fixture = UiFixture::new();
        let data = fixture.install("Data");
        let app = fixture.install("App");
        let assets = UiAssets::new(&data, Some(&app), None);
        let embedded = UiAssets::new(&fixture.0.join("Missing"), None, None);
        let (default_theme, _) = Theme::load(&embedded);
        let mut custom: serde_json::Value =
            serde_json::from_str(include_str!("../assets/ui/theme.json")).unwrap();
        custom["palette"]["text"] = serde_json::json!([1, 2, 3, 255]);
        std::fs::write(data.join("ui/theme.json"), custom.to_string()).unwrap();
        let (theme, invalid) = Theme::load(&assets);
        assert!(!invalid);
        assert_eq!(theme.color("text"), [1, 2, 3, 255]);
        let (role, image, meta) = font_sources()[0];
        let (default_font, _) =
            Font::load_from_assets(&default_theme, &embedded, role, image, meta);
        let mut custom: serde_json::Value = serde_json::from_str(meta).unwrap();
        let advance = default_font.glyph('A').advance + 1.;
        custom["glyphs"]["65"]["advance"] = serde_json::json!(advance);
        std::fs::write(data.join("ui/body-atlas.json"), custom.to_string()).unwrap();
        let (font, invalid) = Font::load_from_assets(&theme, &assets, role, image, meta);
        assert!(!invalid);
        assert_eq!(font.glyph('A').advance, advance);

        std::fs::write(data.join("ui/theme.json"), "invalid JSON").unwrap();
        std::fs::write(data.join("ui/body-atlas.png"), b"invalid PNG").unwrap();
        for remove_app in [false, true] {
            if remove_app {
                std::fs::remove_dir_all(&app).unwrap();
            }
            let (theme, invalid) = Theme::load(&assets);
            assert!(invalid);
            assert_eq!(theme, default_theme);
            let (font, invalid) = Font::load_from_assets(&theme, &assets, role, image, meta);
            assert!(invalid);
            assert_eq!(font, default_font);
        }
    }
    fn model() -> HudModel {
        let mut ui = UiPresentation::default();
        ui.resources.push(ui::ResourceView {
            name: "Metal".into(),
            icon: "mine".into(),
            value: "1200".into(),
            trend: "+4/min".into(),
            warning: None,
        });
        ui.minimap = ui::MinimapView {
            width: 150,
            height: 100,
            rgba: vec![40; 150 * 100 * 4],
            zoom_permille: 1000,
            center: [0.5, 0.5],
            ..Default::default()
        };
        HudModel {
            ui: Some(ui),
            ..Default::default()
        }
    }
    #[test]
    fn native_layout_hits_match_visible_controls_across_sizes_and_scales() {
        for (w, h) in [(1366, 768), (1920, 1080), (2560, 1440), (3840, 2160)] {
            for scale in [750, 1000, 2000] {
                let mut m = model();
                m.ui.as_mut().unwrap().prefs.ui_scale_permille = scale;
                let l = layout(&m, w, h);
                for r in [l.top, l.minimap, l.selection, l.command] {
                    assert!(r.end_x() <= w && r.end_y() <= h, "{w}x{h} scale{scale}");
                }
                for (index, r) in &l.tool_buttons {
                    let hit = hit_test(
                        &m,
                        w,
                        h,
                        r.x as f64 + r.width as f64 * 0.5,
                        r.y as f64 + r.height as f64 * 0.5,
                    );
                    assert_eq!(hit, UiHit::Action(UiAction::ChooseTool(*index)));
                }
            }
        }
    }
    #[test]
    fn resource_change_only_repaints_resource_plate_and_unchanged_layers_reuse() {
        let mut m = model();
        let mut cache = RasterCache::default();
        let first = cache.draw(&m, 1366, 768).layers_redrawn;
        assert!(first >= 5);
        let frame = cache.draw(&m, 1366, 768);
        assert!(frame.dirty.is_empty());
        assert_eq!(frame.layers_redrawn, 0);
        m.ui.as_mut().unwrap().resources[0].value = "1199".into();
        let frame = cache.draw(&m, 1366, 768);
        assert_eq!(frame.layers_redrawn, 1);
        assert!(frame.layers_reused >= 4);
        assert!(frame.dirty.iter().all(|r| r.end_y() < 100));
    }
    #[test]
    fn licensed_fonts_are_proportional_with_tabular_numbers_and_real_coverage() {
        let fonts = Fonts::get();
        assert!(fonts.body.width("WWW", 16.) > fonts.body.width("III", 16.) * 1.5);
        let digits: Vec<_> = (b'0'..=b'9')
            .map(|c| fonts.numbers.glyph(c as char).advance)
            .collect();
        assert!(digits.iter().all(|v| (*v - digits[0]).abs() < 0.001));
        assert!(fonts.body.meta.source_pixels >= 64.);
        assert!(fonts.body.alpha.iter().any(|a| *a > 0 && *a < 255));
    }
}
