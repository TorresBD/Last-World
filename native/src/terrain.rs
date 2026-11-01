use bytemuck::{Pod, Zeroable};
use std::{fmt, ops::Range};

pub const CELL_METERS: usize = 8;
pub const DYNAMIC_WATER: u32 = 2;
pub const GROUND_DISTURBED: u32 = 4;
pub const SOIL_MOISTURE_SHIFT: u32 = 8;
pub const SOIL_MOISTURE_MASK: u32 = 1023 << SOIL_MOISTURE_SHIFT;
pub const WALK_WATER_LIMIT_MM: u32 = 300;
const FOLIAGE_VISUAL_BANDS: u32 = 64;
const MOISTURE_VISUAL_BANDS: u32 = 32;
const MAX_ELEVATION_MM: i32 = 1_000_000;
const CELL_AREA_MM2: u128 = (CELL_METERS * 1000 * CELL_METERS * 1000) as u128;
pub const WW: usize = 4800;
pub const HH: usize = 3200;
pub const CW: usize = 600;
pub const CH: usize = 400;
pub const NW: usize = 300;
pub const NH: usize = 200;
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, serde::Serialize, serde::Deserialize)]
pub struct Cell {
    pub height: f32,
    pub cover: f32,
    pub water: u32,
    pub flags: u32,
    pub object: u32,
    pub soil: u32,
    pub pad: [u32; 2],
}
#[derive(Clone)]
pub struct Field {
    seed: u64,
}
impl Field {
    pub fn new(seed: u32) -> Self {
        Self {
            seed: (seed as u64).wrapping_mul(1442695040888963407),
        }
    }
    pub fn hash(&self, x: i32, y: i32) -> f32 {
        let n = (x as i64)
            .wrapping_mul(374761393)
            .wrapping_add((y as i64).wrapping_mul(668265263)) as u64;
        let n = n.wrapping_add(self.seed);
        let m = ((n ^ (n >> 13)).wrapping_mul(1274126177)) as u32;
        ((m ^ (m >> 16)) & 65535) as f32 / 65535.
    }
    pub fn noise(&self, x: f32, y: f32, cell: f32) -> f32 {
        let gx = (x / cell).floor() as i32;
        let gy = (y / cell).floor() as i32;
        let mut tx = (x - gx as f32 * cell) / cell;
        let mut ty = (y - gy as f32 * cell) / cell;
        tx = tx * tx * (3. - 2. * tx);
        ty = ty * ty * (3. - 2. * ty);
        let a = self.hash(gx, gy);
        let b = self.hash(gx + 1, gy);
        let c = self.hash(gx, gy + 1);
        let d = self.hash(gx + 1, gy + 1);
        (a + (b - a) * tx) * (1. - ty) + (c + (d - c) * tx) * ty
    }
    pub fn height(&self, x: f32, y: f32) -> f32 {
        (180.
            + self.noise(x, y, 370.) * 900.
            + self.noise(x + 137., y + 89., 103.) * 155.
            + self.noise(x + 53., y + 227., 31.) * 16.)
            .trunc()
    }
    pub fn mix(&self, x: f32, y: f32) -> f64 {
        self.noise(x + 1703., y - 231., 370.) as f64 * 0.62
            + self.noise(x - 431., y + 619., 103.) as f64 * 0.28
            + self.noise(x + 53., y + 227., 31.) as f64 * 0.10
    }
    pub fn species(&self, x: f32, y: f32) -> usize {
        (((self.mix(x, y) - 0.25) * 2.).clamp(0., 0.9999) * 4.) as usize
    }
    pub fn cover(&self, x: f32, y: f32, wet: u32) -> f32 {
        let cover = self.noise(x + 91., y + 47., 37.) * 0.38
            + self.noise(x + 231., y - 27., 11.) * 0.35
            + self.hash(x as i32, y as i32) * 0.27;
        let base = (40. + cover * 195. - wet as f32 * 0.7)
            .clamp(0., 245.)
            .trunc();
        let f = self.noise(x + 91., y + 47., 370.) as f64 * 0.55
            + self.noise(x + 231., y - 27., 103.) as f64 * 0.30
            + self.noise(x + 53., y + 227., 31.) as f64 * 0.15;
        ((base as f64 * (0.82 / 0.5)).min(245.) * (1. - ((f - 0.59) * 8.).clamp(0., 1.) * 0.92))
            as f32
    }
}
pub fn hash(x: i32, y: i32, seed: u32) -> f64 {
    let mut n = ((x as u32 ^ seed).wrapping_mul(374761393))
        ^ ((y as u32).wrapping_add(17).wrapping_mul(668265263));
    n = (n ^ (n >> 13)).wrapping_mul(1274126177);
    (n ^ (n >> 16)) as f64 / 4294967295.
}
pub fn noise(x: f64, y: f64, size: f64, seed: u32) -> f64 {
    let ix = (x / size).floor() as i32;
    let iy = (y / size).floor() as i32;
    let mut u = x / size - ix as f64;
    let mut v = y / size - iy as f64;
    u = u * u * (3. - 2. * u);
    v = v * v * (3. - 2. * v);
    let a = hash(ix, iy, seed);
    let b = hash(ix + 1, iy, seed);
    let c = hash(ix, iy + 1, seed);
    let d = hash(ix + 1, iy + 1, seed);
    (a + (b - a) * u) * (1. - v) + (c + (d - c) * u) * v
}
struct Growth {
    state: u64,
}
impl Growth {
    fn new(seed: u64) -> Self {
        Self {
            state: (seed ^ 0x5deece66d) & 0xffffffffffff,
        }
    }
    fn next(&mut self, bits: u32) -> u32 {
        self.state = self.state.wrapping_mul(0x5deece66d).wrapping_add(11) & 0xffffffffffff;
        (self.state >> (48 - bits)) as u32
    }
    fn int(&mut self, b: u32) -> i32 {
        if b.is_power_of_two() {
            return ((b as u64 * self.next(31) as u64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31);
            let value = bits % b;
            if bits - value + b - 1 <= 2147483647 {
                return value as i32;
            }
        }
    }
    fn float(&mut self) -> f32 {
        self.next(24) as f32 / 16777216.
    }
    fn sign(&mut self) -> i32 {
        if self.next(1) != 0 { 1 } else { -1 }
    }
}
/// Rectangles use texture-pixel coordinates (2m per pixel), not world meters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Rectangles use the current fixture's coarse8m terrain-cell coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CellRect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// Exact geometric displacement for the coarse samples changed by an edit.
/// Callers still own construction work, soil/spoil inventories and ecology.
/// This report is not a soil-material source or an automatic job completion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EditVolume {
    pub cut_mm3: u128,
    pub fill_mm3: u128,
}
impl EditVolume {
    fn add(&mut self, before: i32, after: i32) -> Result<(), TerrainError> {
        let delta = after as i64 - before as i64;
        let amount = (delta.unsigned_abs() as u128)
            .checked_mul(CELL_AREA_MM2)
            .ok_or(TerrainError::ArithmeticOverflow)?;
        if delta < 0 {
            self.cut_mm3 = self
                .cut_mm3
                .checked_add(amount)
                .ok_or(TerrainError::ArithmeticOverflow)?;
        } else {
            self.fill_mm3 = self
                .fill_mm3
                .checked_add(amount)
                .ok_or(TerrainError::ArithmeticOverflow)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TerrainError {
    InvalidIndex,
    InvalidBounds,
    InvalidRadius,
    InvalidStrength,
    ElevationOutOfRange,
    ArithmeticOverflow,
    InvalidVisualFlow,
    InvalidGroundMoisture,
    InvalidFoliage,
    ImplicitFoliageRegrowth,
}
impl fmt::Display for TerrainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidIndex => "Terrain cell index is outside the resident fixture",
            Self::InvalidBounds => "Terrain edit bounds are outside the resident fixture",
            Self::InvalidRadius => {
                "Terrain edit radius must be positive and within the fixture extent"
            }
            Self::InvalidStrength => "Smoothing strength must be in0..=1000permille",
            Self::ElevationOutOfRange => "Terrain elevation is outside the explicit +/-1000m range",
            Self::ArithmeticOverflow => "Terrain edit arithmetic overflow",
            Self::InvalidVisualFlow => "Water display flow must be finite",
            Self::InvalidGroundMoisture => "Ground moisture must be in 0..=1000 permille",
            Self::InvalidFoliage => "Remaining foliage exceeds the generated patch stock",
            Self::ImplicitFoliageRegrowth => {
                "Ground display updates cannot regrow foliage without an explicit growth transfer"
            }
        })
    }
}
impl std::error::Error for TerrainError {}

struct DirtyCells {
    bits: Vec<u64>,
    indices: Vec<usize>,
}
impl DirtyCells {
    fn new(count: usize) -> Self {
        Self {
            bits: vec![0; count.div_ceil(64)],
            indices: Vec::new(),
        }
    }
    fn mark(&mut self, index: usize) {
        let bit = 1u64 << (index % 64);
        if self.bits[index / 64] & bit == 0 {
            self.bits[index / 64] |= bit;
            self.indices.push(index);
        }
    }
    fn take_ranges(&mut self) -> Vec<Range<usize>> {
        self.indices.sort_unstable();
        let mut ranges: Vec<Range<usize>> = Vec::new();
        for index in self.indices.drain(..) {
            self.bits[index / 64] &= !(1u64 << (index % 64));
            if let Some(last) = ranges.last_mut() {
                if last.end == index && last.start / CW == index / CW {
                    last.end += 1;
                    continue;
                }
            }
            ranges.push(index..index + 1);
        }
        ranges
    }
}

/// Saved physical state. Pixel templates are deterministic immutable material
/// data; restoring those templates never generates live organisms or stocks.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TerrainSnapshot {
    pub generator_version: u32,
    pub seed: u32,
    pub cells: Vec<Cell>,
    pub elevations_mm: Vec<i32>,
    pub foliage_units: Vec<u32>,
    pub revision: u64,
}

pub struct Terrain {
    /// GPU-layout cache. Height and dynamic depth are derived via edit APIs;
    /// gameplay elevation authority is the retained integer millimeter vector.
    pub cells: Vec<Cell>,
    pub rgba: Vec<u8>,
    pub seed: u32,
    colors: crate::natural_colors::NaturalColors,
    wood: Vec<u8>,
    life: Vec<u8>,
    /// Initial stock comes from generated grass/wood/leaf samples. Once a
    /// GroundModel is created it owns gameplay biomass; these values are only
    /// its monotone display cache and immutable procedural rendering template.
    initial_foliage_units: Vec<u32>,
    foliage_units: Vec<u32>,
    base_cover: Vec<u8>,
    elevations_mm: Vec<i32>,
    dirty_cells: DirtyCells,
    dirty_visuals: DirtyCells,
    revision: u64,
}
impl Terrain {
    pub fn snapshot(&self) -> TerrainSnapshot {
        TerrainSnapshot {
            generator_version: 1,
            seed: self.seed,
            cells: self.cells.clone(),
            elevations_mm: self.elevations_mm.clone(),
            foliage_units: self.foliage_units.clone(),
            revision: self.revision,
        }
    }

    pub fn restore(saved: &TerrainSnapshot) -> Result<Self, String> {
        if saved.generator_version != 1 || saved.cells.len() != CW * CH
            || saved.elevations_mm.len() != CW * CH || saved.foliage_units.len() != CW * CH
            || saved.cells.iter().any(|c| !c.height.is_finite() || !c.cover.is_finite())
            || saved.elevations_mm.iter().any(|h| !(-MAX_ELEVATION_MM..=MAX_ELEVATION_MM).contains(h)) {
            return Err("Invalid or unsupported saved terrain".into());
        }
        let mut terrain = Self::new(saved.seed);
        if saved.foliage_units.iter().zip(&terrain.initial_foliage_units).any(|(n,initial)| n > initial) {
            return Err("Saved foliage exceeds the material template's capacity".into());
        }
        terrain.cells.clone_from(&saved.cells);
        terrain.elevations_mm.clone_from(&saved.elevations_mm);
        terrain.foliage_units.clone_from(&saved.foliage_units);
        terrain.revision = saved.revision;
        for i in 0..CW*CH {
            // Restore the derived layout from the integer owner.
            terrain.cells[i].height = terrain.elevations_mm[i] as f32 / 10.;
            terrain.dirty_cells.mark(i);
            terrain.dirty_visuals.mark(i);
        }
        Ok(terrain)
    }

    pub fn initial_foliage_units(&self, index: usize) -> Option<u32> {
        self.initial_foliage_units.get(index).copied()
    }

    pub fn validate_ground_surface(&self, index: usize, foliage: u32, moisture: u16) -> Result<(), TerrainError> {
        let initial = self.initial_foliage_units(index).ok_or(TerrainError::InvalidIndex)?;
        let current = self.foliage_units(index)?;
        if self.cells.get(index).is_none() { return Err(TerrainError::InvalidIndex); }
        if moisture > 1000 { return Err(TerrainError::InvalidGroundMoisture); }
        if foliage > initial { return Err(TerrainError::InvalidFoliage); }
        if foliage > current { return Err(TerrainError::ImplicitFoliageRegrowth); }
        Ok(())
    }

    pub fn validate_elevation_edit(&self, index: usize, target: i32) -> Result<(), TerrainError> {
        let before = self.elevation_mm(index)?;
        if self.cells.get(index).is_none() { return Err(TerrainError::InvalidIndex); }
        if !(-MAX_ELEVATION_MM..=MAX_ELEVATION_MM).contains(&target) { return Err(TerrainError::ElevationOutOfRange); }
        if before != target { self.revision.checked_add(1).ok_or(TerrainError::ArithmeticOverflow)?; }
        Ok(())
    }

    /// An explicit biological owner has already paid growth/resource costs.
    /// This changes only the derived material cache, never living stock.
    pub fn apply_biological_cover(&mut self, index: usize, units: u32, moisture: u16, disturbed: bool) -> Result<bool, TerrainError> {
        let initial = self.initial_foliage_units(index).ok_or(TerrainError::InvalidIndex)?;
        if units > initial || moisture > 1000 { return Err(TerrainError::InvalidFoliage); }
        let previous = self.foliage_units[index];
        if units > previous {
            self.foliage_units[index] = units;
            self.dirty_cells.mark(index);
            self.dirty_visuals.mark(index);
        }
        self.set_ground_surface(index, units, moisture, disturbed)
            .map(|changed| changed || units != previous)
    }

    pub fn set_occupancy(&mut self, index: usize, object: u32) -> Result<(), TerrainError> {
        let cell = self.cells.get_mut(index).ok_or(TerrainError::InvalidIndex)?;
        if cell.object != object {
            cell.object = object;
            self.revision = self.revision.checked_add(1).ok_or(TerrainError::ArithmeticOverflow)?;
            self.dirty_cells.mark(index);
        }
        Ok(())
    }

    pub fn new(seed: u32) -> Self {
        let field = Field::new(seed);
        let mut cells = vec![Cell::zeroed(); CW * CH];
        for y in 0..CH {
            for x in 0..CW {
                let wx = (x * 8 + 4) as f32;
                let wy = (y * 8 + 4) as f32;
                let river = 2390.
                    + (noise(0., wy as f64, 840., seed.wrapping_add(106)) - 0.5) * 480.
                    + (noise(0., wy as f64, 193., seed.wrapping_add(107)) - 0.5) * 95.;
                let rw = 18. + noise(0., wy as f64, 230., seed.wrapping_add(108)) * 28.;
                let tributary = 1010.
                    + (noise(wx as f64, 0., 590., seed.wrapping_add(109)) - 0.5) * 320.
                    + (noise(wx as f64, 0., 127., seed.wrapping_add(110)) - 0.5) * 64.;
                let wet = (wx as f64 - river).abs() < rw
                    || (wx as f64 > river
                        && (wy as f64 - tributary).abs()
                            < 12. + noise(wx as f64, 0., 210., seed.wrapping_add(111)) * 9.);
                let mut h = field.height(wx, wy);
                let mut water = if h < 340. {
                    (340. - h).min(160.) as u32
                } else {
                    0
                };
                if wet {
                    h = 310.;
                    water = 100;
                }
                let v = if wet { 12. } else { field.cover(wx, wy, water) };
                cells[y * CW + x] = Cell {
                    height: h,
                    cover: v.round(),
                    water,
                    soil: field.species(wx, wy) as u32,
                    flags: 0,
                    object: 0,
                    pad: [0; 2],
                };
            }
        }
        let mut wood = vec![0u8; WW * HH / 4];
        let mut life = vec![0u8; WW * HH / 4];
        let mut r = Growth::new(seed as u64 ^ 0x4bc3129);
        let roots = (WW as f64 * HH as f64 / 25600. * (0.5 + 0.82 * 12.)).round() as usize;
        let leaf = |x: i32, y: i32, s: usize, v: u8, life: &mut Vec<u8>| {
            let i = y as usize / 2 * (WW / 2) + x as usize / 2;
            if life[i] >> 3 < v {
                life[i] = (v << 3) | (s as u8 + 1);
            }
        };
        for _ in 0..roots {
            let sx = 20 + r.int(WW as u32 - 40);
            let sy = 20 + r.int(HH as u32 - 40);
            let cell = cells[sy as usize / 8 * CW + sx as usize / 8];
            if cell.water != 0 || cell.cover < 65. {
                continue;
            }
            let species = cell.soil as usize;
            let mut branch = 0;
            while branch < 3 + r.int(4) {
                let mut x = sx;
                let mut y = sy;
                let mut dx = r.sign();
                let mut dy = r.sign();
                let steps = 14 + r.int(if species == 1 { 48 } else { 30 });
                for n in 0..steps {
                    let i = y as usize / 2 * (WW / 2) + x as usize / 2;
                    wood[i] = wood[i].max((145 - n * 3).max(4) as u8);
                    leaf(x, y, species, 28, &mut life);
                    if n % 2 == 0 {
                        let mut lx = x;
                        let mut ly = y;
                        let mut ldx = r.sign();
                        let mut ldy = r.sign();
                        let length = 5 + r.int(if species == 1 { 14 } else { 9 });
                        let turn = [0.4, 0.2, 0.45, 0.15][species];
                        for bud in 0..length {
                            leaf(lx, ly, species, 27 - (bud / 2) as u8, &mut life);
                            if r.float() < turn {
                                ldx = r.int(3) - 1;
                            }
                            if r.float() < turn {
                                ldy = r.int(3) - 1;
                            }
                            if ldx == 0 && ldy == 0 {
                                ldy = 1;
                            }
                            lx = (lx + ldx).clamp(2, WW as i32 - 3);
                            ly = (ly + ldy).clamp(2, HH as i32 - 3);
                        }
                    }
                    if r.float() < 0.28 {
                        dx = r.int(3) - 1;
                    }
                    if r.float() < 0.28 {
                        dy = r.int(3) - 1;
                    }
                    if dx == 0 && dy == 0 {
                        dy = 1;
                    }
                    x = (x + dx).clamp(2, WW as i32 - 3);
                    y = (y + dy).clamp(2, HH as i32 - 3);
                }
                branch += 1;
            }
        }
        // This retained raster is the existing 4.8x3.2km reference fixture,
        // not the allocation policy for the future20km streamed world.
        let elevations_mm = cells
            .iter()
            .map(|cell| (cell.height * 10.).round() as i32)
            .collect();
        let base_cover = cells.iter().map(|c| c.cover as u8).collect();
        let mut terrain = Self {
            cells,
            rgba: vec![255; WW * HH],
            seed,
            colors: crate::natural_colors::NaturalColors::new(seed),
            wood,
            life,
            initial_foliage_units: Vec::new(),
            foliage_units: Vec::new(),
            base_cover,
            elevations_mm,
            dirty_cells: DirtyCells::new(CW * CH),
            dirty_visuals: DirtyCells::new(CW * CH),
            revision: 0,
        };
        terrain.repaint(PixelRect {
            x: 0,
            y: 0,
            width: (WW / 2) as u32,
            height: (HH / 2) as u32,
        });
        terrain
    }
    fn repaint(&mut self, rect: PixelRect) {
        // Collect finite initial display units during the existing first paint,
        // avoiding a second whole-fixture noise/growth pass at generation.
        let generating = self.initial_foliage_units.is_empty();
        let mut generated_stock = if generating {
            vec![0u32; self.cells.len()]
        } else {
            Vec::new()
        };
        let field = Field::new(self.seed);
        let soil = [
            [70., 78., 55.],
            [113., 98., 64.],
            [143., 101., 66.],
            [112., 120., 110.],
        ];
        let floor = [
            [35., 69., 41.],
            [43., 88., 57.],
            [60., 83., 39.],
            [67., 92., 51.],
        ];
        for py in rect.y as usize..(rect.y + rect.height) as usize {
            for px in rect.x as usize..(rect.x + rect.width) as usize {
                let x = (px * 2) as f32;
                let y = (py * 2) as f32;
                let i = py * (WW / 2) + px;
                let cell_index = py / 4 * CW + px / 4;
                let mut cell = self.cells[cell_index];
                // Dynamic water is shaded by the GPU from physical depth. Keep
                // the baked substrate dry rather than painting a second river.
                if cell.flags & DYNAMIC_WATER != 0 {
                    cell.water = 0;
                }
                let template_cover = (field.cover(x, y, cell.water)
                    + if self.wood[i] > 0 { 20. } else { 0. })
                .min(245.);
                if generating {
                    generated_stock[cell_index] += template_cover.round() as u32
                        + self.wood[i] as u32 * 4
                        + (self.life[i] >> 3) as u32 * 8;
                }
                let fraction = if generating {
                    1.
                } else {
                    Self::foliage_band(
                        self.foliage_units[cell_index],
                        self.initial_foliage_units[cell_index],
                    ) as f32
                        / FOLIAGE_VISUAL_BANDS as f32
                };
                // A fixed field rank removes existing grown pixels organically
                // as the owning model transfers biomass away. No replacement
                // plants are generated when water drains or the page repaints.
                let retains_grown_pixel = fraction == 1.
                    || (fraction > 0. && field.hash(x as i32 + 419, y as i32 + 977) < fraction);
                let grass = template_cover.round() / 255. * fraction;
                let field_mix = field.mix(x, y);
                let mix = (field_mix * 255.).round() / 255.;
                let ty = ((mix - 0.25) * 2.).clamp(0., 1.) as f32 * 3.;
                let low = ty.floor() as usize;
                let high = (low + 1).min(3);
                let blend = ty - low as f32;
                let grain = (field.hash(x as i32 + 1703, y as i32 - 231) * 23.).trunc() - 11.;
                let shade = ((cell.height - 650.) / 65.).trunc();
                let mud = (cell.water as f32 / 160.).min(1.);
                let mut rgb = [0.; 3];
                for k in 0..3 {
                    rgb[k] = (soil[low][k] * (1. - blend) + soil[high][k] * blend) * (1. - grass)
                        + floor[cell.soil as usize][k] * grass
                        - mud * [26., 20., 12.][k]
                        + if k == 2 { (grain / 2.).trunc() } else { grain }
                        + shade;
                }
                let moisture =
                    ((cell.flags & SOIL_MOISTURE_MASK) >> SOIL_MOISTURE_SHIFT) as f32 / 1000.;
                if cell.flags & GROUND_DISTURBED != 0 || moisture > 0. || fraction < 1. {
                    let dirt = [
                        [105., 75., 43.],
                        [126., 91., 55.],
                        [137., 89., 51.],
                        [108., 84., 63.],
                    ];
                    for k in 0..3 {
                        let substrate = (dirt[low][k] * (1. - blend) + dirt[high][k] * blend)
                            * (1. - moisture * 0.48);
                        rgb[k] = substrate * (1. - grass)
                            + floor[cell.soil as usize][k] * grass
                            + if k == 2 { (grain / 2.).trunc() } else { grain }
                            + shade;
                    }
                }
                if cell.water > 45 {
                    rgb[0] -= 8.;
                    rgb[1] += 2.;
                    rgb[2] += 8.;
                }
                if self.wood[i] > 0 && retains_grown_pixel {
                    let light = (grain / 3.).trunc();
                    rgb = [29. + light, 63. + light, 35. + light];
                }
                if self.life[i] != 0 && retains_grown_pixel && cell.water == 0 && grass > 0.18 {
                    let species = ((self.life[i] & 7) - 1) as usize;
                    let variation = field.hash(x as i32 + 331, y as i32 + 719);
                    let light = 0.72 + (self.life[i] >> 3) as f32 / 100. + variation * 0.2;
                    let mut leaves = self.colors.foliage(species, field_mix);
                    if (species == 2 && variation > 0.76) || (species == 3 && variation > 0.86) {
                        leaves = self.colors.accent(species);
                    }
                    for k in 0..3 {
                        rgb[k] = leaves[k] * light + grain * [0.3, 0.4, 0.3][k];
                    }
                }
                for k in 0..3 {
                    self.rgba[i * 4 + k] = rgb[k].round().clamp(0., 255.) as u8;
                }
            }
        }
        if generating {
            self.foliage_units = generated_stock.clone();
            self.initial_foliage_units = generated_stock;
        }
    }

    /// Renderer-only species coat; independent of all ecology/gameplay stocks.
    pub fn fauna_appearance(&self, species: u32) -> u32 { self.colors.fauna_appearance(species) }

    fn foliage_band(remaining: u32, initial: u32) -> u32 {
        if initial == 0 {
            0
        } else {
            (remaining as u64 * FOLIAGE_VISUAL_BANDS as u64 / initial as u64) as u32
        }
    }

    /// Initial units are finite generated grass/wood/leaf display stock. The
    /// coupled GroundModel takes gameplay ownership after initialization.
    pub fn foliage_units(&self, index: usize) -> Result<u32, TerrainError> {
        self.foliage_units
            .get(index)
            .copied()
            .ok_or(TerrainError::InvalidIndex)
    }

    /// Update derived surface state from its one gameplay owner. This function
    /// does not simulate drying, spawn flora or return washed-away biomass.
    /// Visual moisture is quantized to 32 bands; the owner's water stock remains
    /// exact. Render dirtiness follows visible bands rather than every lost unit.
    pub fn set_ground_surface(
        &mut self,
        index: usize,
        foliage_units: u32,
        moisture_permille: u16,
        disturbed: bool,
    ) -> Result<bool, TerrainError> {
        let initial = *self
            .initial_foliage_units
            .get(index)
            .ok_or(TerrainError::InvalidIndex)?;
        let previous = self.foliage_units[index];
        if moisture_permille > 1000 {
            return Err(TerrainError::InvalidGroundMoisture);
        }
        if foliage_units > initial {
            return Err(TerrainError::InvalidFoliage);
        }
        if foliage_units > previous {
            return Err(TerrainError::ImplicitFoliageRegrowth);
        }
        let before_band = Self::foliage_band(previous, initial);
        let after_band = Self::foliage_band(foliage_units, initial);
        let moisture_band = (moisture_permille as u32 * MOISTURE_VISUAL_BANDS + 500) / 1000;
        let visual_moisture = moisture_band * 1000 / MOISTURE_VISUAL_BANDS;
        let before_flags = self.cells[index].flags;
        let flags = (before_flags & !(SOIL_MOISTURE_MASK | GROUND_DISTURBED))
            | (visual_moisture << SOIL_MOISTURE_SHIFT)
            | if disturbed { GROUND_DISTURBED } else { 0 };
        self.foliage_units[index] = foliage_units;
        if before_band != after_band || flags != before_flags {
            self.cells[index].flags = flags;
            self.cells[index].cover =
                self.base_cover[index] as f32 * after_band as f32 / FOLIAGE_VISUAL_BANDS as f32;
            self.dirty_cells.mark(index);
            self.dirty_visuals.mark(index);
        }
        Ok(previous != foliage_units || before_flags != flags)
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn elevation_mm(&self, index: usize) -> Result<i32, TerrainError> {
        self.elevations_mm
            .get(index)
            .copied()
            .ok_or(TerrainError::InvalidIndex)
    }

    pub fn set_elevation_mm(
        &mut self,
        index: usize,
        height_mm: i32,
    ) -> Result<EditVolume, TerrainError> {
        let before = self.elevation_mm(index)?;
        if !(-MAX_ELEVATION_MM..=MAX_ELEVATION_MM).contains(&height_mm) {
            return Err(TerrainError::ElevationOutOfRange);
        }
        let mut volume = EditVolume::default();
        volume.add(before, height_mm)?;
        if before != height_mm {
            let revision = self
                .revision
                .checked_add(1)
                .ok_or(TerrainError::ArithmeticOverflow)?;
            self.elevations_mm[index] = height_mm;
            self.cells[index].height = height_mm as f32 / 10.;
            self.dirty_cells.mark(index);
            self.dirty_visuals.mark(index);
            self.revision = revision;
        }
        Ok(volume)
    }

    /// Dynamic depth is millimeters, unlike the legacy generated wetness value.
    /// The first transition repaints a dry substrate once; subsequent water-only
    /// changes update the cell buffer without rebuilding the grown pixel raster.
    pub fn set_water_depth_mm(
        &mut self,
        index: usize,
        depth_mm: u32,
    ) -> Result<bool, TerrainError> {
        let cell = self.cells.get(index).ok_or(TerrainError::InvalidIndex)?;
        let first = cell.flags & DYNAMIC_WATER == 0;
        if !first && cell.water == depth_mm {
            return Ok(false);
        }
        if first {
            self.dirty_visuals.mark(index);
        }
        self.cells[index].flags |= DYNAMIC_WATER;
        self.cells[index].water = depth_mm;
        self.dirty_cells.mark(index);
        Ok(true)
    }

    /// Row-bounded ranges are in Cell elements, ready for partial buffer writes.
    /// Distant changes never become a whole-world min/max upload rectangle.
    pub fn take_dirty_cells(&mut self) -> Vec<Range<usize>> {
        self.dirty_cells.take_ranges()
    }

    /// Derived visual flow only. Physical liters remain in WaterGraph.
    pub fn set_water_visual_flow(
        &mut self,
        index: usize,
        velocity: [f32; 2],
    ) -> Result<(), TerrainError> {
        if velocity.iter().any(|v| !v.is_finite()) {
            return Err(TerrainError::InvalidVisualFlow);
        }
        let cell = self
            .cells
            .get_mut(index)
            .ok_or(TerrainError::InvalidIndex)?;
        let packed = velocity.map(f32::to_bits);
        if cell.pad != packed {
            cell.pad = packed;
            self.dirty_cells.mark(index);
        }
        Ok(())
    }

    /// Refresh only dirty pixels and return upload rectangles. Source rgba has
    /// the full fixture row stride WW/2*4; each returned rectangle is bounded.
    pub fn take_dirty_pixels(&mut self) -> Vec<PixelRect> {
        let mut rects: Vec<PixelRect> = Vec::new();
        for range in self.dirty_visuals.take_ranges() {
            let rect = PixelRect {
                x: (range.start % CW * 4) as u32,
                y: (range.start / CW * 4) as u32,
                width: ((range.end - range.start) * 4) as u32,
                height: 4,
            };
            if let Some(last) = rects.last_mut() {
                if last.x == rect.x && last.width == rect.width && last.y + last.height == rect.y {
                    last.height += rect.height;
                    continue;
                }
            }
            rects.push(rect);
        }
        for rect in &rects {
            self.repaint(*rect);
        }
        rects
    }

    /// Integer paraboloid falloff changes the sampled ground, not a sprite.
    /// The current representation is8m resolution; fine1m edit pages are future
    /// streaming work. Soil movement is reported for the caller's finite ledger.
    pub fn deform(
        &mut self,
        center_mm: [i32; 2],
        radius_mm: u32,
        delta_mm: i32,
    ) -> Result<EditVolume, TerrainError> {
        if center_mm[0] < 0
            || center_mm[1] < 0
            || center_mm[0] as usize >= WW * 1000
            || center_mm[1] as usize >= HH * 1000
        {
            return Err(TerrainError::InvalidBounds);
        }
        if radius_mm == 0 || radius_mm as usize > WW.max(HH) * 1000 {
            return Err(TerrainError::InvalidRadius);
        }
        let cell_mm = (CELL_METERS * 1000) as i64;
        let radius = radius_mm as i64;
        let left = ((center_mm[0] as i64 - radius).max(0) / cell_mm) as usize;
        let right = ((center_mm[0] as i64 + radius) / cell_mm).min(CW as i64 - 1) as usize;
        let top = ((center_mm[1] as i64 - radius).max(0) / cell_mm) as usize;
        let bottom = ((center_mm[1] as i64 + radius) / cell_mm).min(CH as i64 - 1) as usize;
        let radius2 = radius as i128 * radius as i128;
        let mut edits = Vec::new();
        for y in top..=bottom {
            for x in left..=right {
                let dx = x as i64 * cell_mm + cell_mm / 2 - center_mm[0] as i64;
                let dy = y as i64 * cell_mm + cell_mm / 2 - center_mm[1] as i64;
                let distance2 = dx as i128 * dx as i128 + dy as i128 * dy as i128;
                if distance2 >= radius2 {
                    continue;
                }
                let index = y * CW + x;
                let delta = delta_mm as i128 * (radius2 - distance2) / radius2;
                let target = self.elevations_mm[index] as i128 + delta;
                if target < -MAX_ELEVATION_MM as i128 || target > MAX_ELEVATION_MM as i128 {
                    return Err(TerrainError::ElevationOutOfRange);
                }
                edits.push((index, target as i32));
            }
        }
        self.apply_edits(&edits)
    }

    /// All targets read the pre-edit neighborhood before any value changes.
    /// This avoids traversal-order bias from smoothing in place.
    pub fn smooth(
        &mut self,
        rect: CellRect,
        strength_permille: u16,
    ) -> Result<EditVolume, TerrainError> {
        if strength_permille > 1000 {
            return Err(TerrainError::InvalidStrength);
        }
        let right = rect
            .x
            .checked_add(rect.width)
            .ok_or(TerrainError::InvalidBounds)?;
        let bottom = rect
            .y
            .checked_add(rect.height)
            .ok_or(TerrainError::InvalidBounds)?;
        if rect.width == 0 || rect.height == 0 || right > CW || bottom > CH {
            return Err(TerrainError::InvalidBounds);
        }
        let mut edits = Vec::with_capacity(rect.width * rect.height);
        for y in rect.y..bottom {
            for x in rect.x..right {
                let mut sum = 0i64;
                let mut count = 0i64;
                for sy in y.saturating_sub(1)..=(y + 1).min(CH - 1) {
                    for sx in x.saturating_sub(1)..=(x + 1).min(CW - 1) {
                        sum += self.elevations_mm[sy * CW + sx] as i64;
                        count += 1;
                    }
                }
                let index = y * CW + x;
                let before = self.elevations_mm[index] as i64;
                let target = before + (sum / count - before) * strength_permille as i64 / 1000;
                edits.push((index, target as i32));
            }
        }
        self.apply_edits(&edits)
    }

    fn apply_edits(&mut self, edits: &[(usize, i32)]) -> Result<EditVolume, TerrainError> {
        // Validate every target and aggregate before committing any geometry.
        let mut volume = EditVolume::default();
        let mut changed = 0u64;
        for &(index, target) in edits {
            let before = self.elevation_mm(index)?;
            if !(-MAX_ELEVATION_MM..=MAX_ELEVATION_MM).contains(&target) {
                return Err(TerrainError::ElevationOutOfRange);
            }
            volume.add(before, target)?;
            if before != target {
                changed += 1;
            }
        }
        self.revision
            .checked_add(changed)
            .ok_or(TerrainError::ArithmeticOverflow)?;
        for &(index, target) in edits {
            self.set_elevation_mm(index, target)?;
        }
        Ok(volume)
    }

    /// Coarse effective-surface query. It includes all committed cut/fill edits.
    pub fn elevation_at_m(&self, position: [f32; 2]) -> Result<i32, TerrainError> {
        if !position[0].is_finite()
            || !position[1].is_finite()
            || position[0] < 0.
            || position[1] < 0.
            || position[0] >= WW as f32
            || position[1] >= HH as f32
        {
            return Err(TerrainError::InvalidBounds);
        }
        self.elevation_mm(
            position[1] as usize / CELL_METERS * CW + position[0] as usize / CELL_METERS,
        )
    }

    pub fn slope_between(&self, from: [f32; 2], to: [f32; 2]) -> Result<f32, TerrainError> {
        let rise = self.elevation_at_m(to)? as f32 - self.elevation_at_m(from)? as f32;
        let run_mm = ((to[0] - from[0]).hypot(to[1] - from[1])) * 1000.;
        if run_mm == 0. {
            return Ok(0.);
        }
        Ok(rise / run_mm)
    }
    pub fn road(&mut self, from: [f32; 2], to: [f32; 2]) {
        let length = (((to[0] - from[0]).hypot(to[1] - from[1]) / 4.).ceil() as usize).max(1);
        for j in 0..=length {
            let a = j as f32 / length as f32;
            let x = ((from[0] + (to[0] - from[0]) * a) / 8.) as usize;
            let y = ((from[1] + (to[1] - from[1]) * a) / 8.) as usize;
            if x < CW && y < CH {
                let index = y * CW + x;
                if self.cells[index].flags & 1 == 0 {
                    self.cells[index].flags |= 1;
                    self.revision = self.revision.checked_add(1).expect("Terrain revision exhausted");
                    self.dirty_cells.mark(index);
                }
            }
        }
    }
    pub fn flow(&self, goal: [f32; 2]) -> Vec<[f32; 2]> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        let target =
            ((goal[1] / 16.) as usize).min(NH - 1) * NW + ((goal[0] / 16.) as usize).min(NW - 1);
        let mut d = vec![u32::MAX; NW * NH];
        d[target] = 0;
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((0u32, target)));
        let dirs = [
            (1, 0),
            (-1, 0),
            (0, 1),
            (0, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
            (-1, -1),
        ];
        while let Some(Reverse((cost, i))) = heap.pop() {
            if cost != d[i] {
                continue;
            }
            for (dx, dy) in dirs {
                let x = i % NW;
                let y = i / NW;
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= NW as i32 || ny >= NH as i32 {
                    continue;
                }
                let n = ny as usize * NW + nx as usize;
                let base = ny as usize * 2 * CW + nx as usize * 2;
                let quad = [
                    self.cells[base],
                    self.cells[base + 1],
                    self.cells[base + CW],
                    self.cells[base + CW + 1],
                ];
                let road = quad.iter().any(|c| c.flags & 1 != 0);
                if (quad.iter().any(|c| {
                    if c.flags & DYNAMIC_WATER != 0 {
                        c.water >= WALK_WATER_LIMIT_MM
                    } else {
                        c.water > 0
                    }
                }) && !road)
                    || quad.iter().all(|c| c.object > 0)
                {
                    continue;
                }
                let cover = quad.iter().map(|c| c.cover).sum::<f32>() / 4.;
                let step = if road { 40 } else { 100 + (cover as u32 * 4) };
                let next = cost + step * if dx != 0 && dy != 0 { 14 } else { 10 };
                if next < d[n] {
                    d[n] = next;
                    heap.push(Reverse((next, n)));
                }
            }
        }
        let mut out = vec![[0.; 2]; NW * NH];
        for i in 0..out.len() {
            let mut best = d[i];
            for (dx, dy) in dirs {
                let x = i % NW;
                let y = i / NW;
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= NW as i32 || ny >= NH as i32 {
                    continue;
                }
                let n = ny as usize * NW + nx as usize;
                if d[n] < best {
                    best = d[n];
                    let length = ((dx * dx + dy * dy) as f32).sqrt();
                    out[i] = [dx as f32 / length, dy as f32 / length];
                }
            }
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn flat_fixture(height_mm: i32, pixels: bool) -> Terrain {
        let mut cell = Cell::zeroed();
        cell.height = height_mm as f32 / 10.;
        cell.cover = 100.;
        Terrain {
            cells: vec![cell; CW * CH],
            rgba: if pixels {
                vec![255; WW * HH]
            } else {
                Vec::new()
            },
            seed: 78421,
            colors: crate::natural_colors::NaturalColors::new(78421),
            wood: if pixels {
                vec![0; WW * HH / 4]
            } else {
                Vec::new()
            },
            life: if pixels {
                vec![0; WW * HH / 4]
            } else {
                Vec::new()
            },
            initial_foliage_units: vec![1000; CW * CH],
            foliage_units: vec![1000; CW * CH],
            base_cover: vec![100; CW * CH],
            elevations_mm: vec![height_mm; CW * CH],
            dirty_cells: DirtyCells::new(CW * CH),
            dirty_visuals: DirtyCells::new(CW * CH),
            revision: 0,
        }
    }

    #[test]
    fn dirty_ranges_are_deduplicated_disjoint_and_never_cross_rows() {
        let mut dirty = DirtyCells::new(CW * 2);
        for index in [CW + 1, CW - 1, 3, CW, 3] {
            dirty.mark(index);
        }
        assert_eq!(dirty.take_ranges(), vec![3..4, CW - 1..CW, CW..CW + 2]);
        assert!(dirty.take_ranges().is_empty());
        dirty.mark(3);
        assert_eq!(dirty.take_ranges(), vec![3..4]);
    }

    #[test]
    fn signed_elevation_edits_report_exact_coarse_volume_and_reject_invalid_targets() {
        let mut terrain = flat_fixture(1000, false);
        let index = CW + 1;
        let moved = terrain.set_elevation_mm(index, -500).unwrap();
        assert_eq!(
            moved,
            EditVolume {
                cut_mm3: 1500 * CELL_AREA_MM2,
                fill_mm3: 0
            }
        );
        assert_eq!(terrain.elevation_mm(index), Ok(-500));
        assert_eq!(terrain.cells[index].height, -50.);
        assert_eq!(terrain.take_dirty_cells(), vec![index..index + 1]);
        let revision = terrain.revision();
        assert_eq!(
            terrain.set_elevation_mm(index, MAX_ELEVATION_MM + 1),
            Err(TerrainError::ElevationOutOfRange)
        );
        assert_eq!(
            terrain.set_elevation_mm(CW * CH, 0),
            Err(TerrainError::InvalidIndex)
        );
        assert_eq!(terrain.elevation_mm(index), Ok(-500));
        assert_eq!(terrain.revision(), revision);
        assert!(terrain.take_dirty_cells().is_empty());
    }

    #[test]
    fn smoothing_reads_the_original_neighborhood_before_committing() {
        let mut terrain = flat_fixture(0, false);
        terrain.set_elevation_mm(CW + 1, 900).unwrap();
        let moved = terrain
            .smooth(
                CellRect {
                    x: 1,
                    y: 1,
                    width: 2,
                    height: 1,
                },
                1000,
            )
            .unwrap();
        // Both9-sample neighborhoods contain the original900mm peak. If the
        // first output were committed during reads, the second would be11mm.
        assert_eq!(terrain.elevation_mm(CW + 1), Ok(100));
        assert_eq!(terrain.elevation_mm(CW + 2), Ok(100));
        assert_eq!(moved.cut_mm3, 800 * CELL_AREA_MM2);
        assert_eq!(moved.fill_mm3, 100 * CELL_AREA_MM2);
        assert_eq!(terrain.elevation_mm(CW + 3), Ok(0));
    }

    #[test]
    fn radial_cut_is_local_and_an_invalid_edit_changes_no_elevation() {
        let mut terrain = flat_fixture(1000, false);
        let index = 2 * CW + 2;
        let moved = terrain.deform([20_000, 20_000], 8_000, -1000).unwrap();
        assert_eq!(terrain.elevation_mm(index), Ok(0));
        assert_eq!(terrain.elevation_mm(index - 1), Ok(1000));
        assert_eq!(terrain.elevation_mm(0), Ok(1000));
        assert_eq!(moved.cut_mm3, 1000 * CELL_AREA_MM2);
        terrain.set_elevation_mm(index, MAX_ELEVATION_MM).unwrap();
        let before = terrain.elevations_mm.clone();
        assert_eq!(
            terrain.deform([20_000, 20_000], 12_000, 100),
            Err(TerrainError::ElevationOutOfRange)
        );
        assert_eq!(terrain.elevations_mm, before);
        assert_eq!(
            terrain.deform([-1, 0], 8_000, -100),
            Err(TerrainError::InvalidBounds)
        );
    }

    #[test]
    fn dynamic_water_repaints_substrate_once_and_keeps_texture_updates_local() {
        let mut terrain = flat_fixture(1000, true);
        let index = CW + 1;
        assert_eq!(terrain.set_water_depth_mm(index, 25), Ok(true));
        assert_eq!(terrain.cells[index].flags & DYNAMIC_WATER, DYNAMIC_WATER);
        assert_eq!(terrain.take_dirty_cells(), vec![index..index + 1]);
        assert_eq!(
            terrain.take_dirty_pixels(),
            vec![PixelRect {
                x: 4,
                y: 4,
                width: 4,
                height: 4
            }]
        );
        assert_eq!(&terrain.rgba[..4], &[255, 255, 255, 255]);
        assert_ne!(
            &terrain.rgba[(4 * (WW / 2) + 4) * 4..][..3],
            &[255, 255, 255]
        );
        assert_eq!(terrain.set_water_depth_mm(index, 100), Ok(true));
        assert!(terrain.take_dirty_pixels().is_empty());
        assert_eq!(terrain.take_dirty_cells(), vec![index..index + 1]);
        assert_eq!(terrain.set_water_depth_mm(index, 100), Ok(false));
        assert!(terrain.take_dirty_cells().is_empty());
    }

    #[test]
    fn effective_height_and_slope_include_committed_cut_fill() {
        let mut terrain = flat_fixture(0, false);
        terrain.set_elevation_mm(1, 800).unwrap();
        assert_eq!(terrain.elevation_at_m([12., 4.]), Ok(800));
        assert!((terrain.slope_between([4., 4.], [12., 4.]).unwrap() - 0.1).abs() < 0.000001);
        assert_eq!(
            terrain.elevation_at_m([f32::NAN, 0.]),
            Err(TerrainError::InvalidBounds)
        );
    }

    #[test]
    fn ground_cache_rejects_regrowth_and_invalid_state_without_committing() {
        let mut terrain = flat_fixture(1000, false);
        let index = CW + 1;
        let revision = terrain.revision();
        assert_eq!(terrain.foliage_units(index), Ok(1000));
        assert_eq!(terrain.set_ground_surface(index, 500, 1000, true), Ok(true));
        assert_eq!(terrain.foliage_units(index), Ok(500));
        assert_eq!(terrain.cells[index].cover, 50.);
        assert_eq!(terrain.revision(), revision);
        assert_eq!(terrain.elevation_mm(index), Ok(1000));
        let before = terrain.cells[index];
        terrain.take_dirty_cells();
        assert_eq!(
            terrain.set_ground_surface(index, 501, 0, false),
            Err(TerrainError::ImplicitFoliageRegrowth)
        );
        assert_eq!(
            terrain.set_ground_surface(index, 1001, 0, false),
            Err(TerrainError::InvalidFoliage)
        );
        assert_eq!(
            terrain.set_ground_surface(index, 0, 1001, false),
            Err(TerrainError::InvalidGroundMoisture)
        );
        assert_eq!(terrain.foliage_units(index), Ok(500));
        assert_eq!(terrain.cells[index].flags, before.flags);
        assert_eq!(terrain.cells[index].cover, before.cover);
        assert!(terrain.take_dirty_cells().is_empty());
        // Adjacent exact moisture values can share one visual band. GroundModel
        // keeps its exact soil water; the texture need not change every tick.
        assert_eq!(terrain.set_ground_surface(index, 500, 999, true), Ok(false));
        assert!(terrain.take_dirty_cells().is_empty());
    }

    #[test]
    fn removing_foliage_clears_existing_grown_pixels_locally_to_brown_soil() {
        let mut terrain = flat_fixture(1000, true);
        let index = CW + 1;
        let rect = PixelRect {
            x: 4,
            y: 4,
            width: 4,
            height: 4,
        };
        for py in 4..8 {
            for px in 4..8 {
                let pixel = py * (WW / 2) + px;
                terrain.wood[pixel] = 180;
                terrain.life[pixel] = (28 << 3) | 1;
            }
        }
        terrain.repaint(rect);
        let center = (4 * (WW / 2) + 4) * 4;
        assert!(terrain.rgba[center + 1] > terrain.rgba[center]);
        terrain.set_ground_surface(index, 0, 0, true).unwrap();
        assert_eq!(terrain.take_dirty_pixels(), vec![rect]);
        for py in 4..8 {
            for px in 4..8 {
                let pixel = (py * (WW / 2) + px) * 4;
                assert!(terrain.rgba[pixel] > terrain.rgba[pixel + 1]);
                assert!(terrain.rgba[pixel + 1] > terrain.rgba[pixel + 2]);
            }
        }
        assert_eq!(terrain.foliage_units(index), Ok(0));
        assert_eq!(terrain.cells[index].cover, 0.);
        assert_eq!(&terrain.rgba[..4], &[255, 255, 255, 255]);
        // Templates remain immutable rendering data, never regenerated stock.
        assert_eq!(terrain.wood[4 * (WW / 2) + 4], 180);
    }

    #[test]
    fn drained_surface_keeps_wet_brown_state_until_owner_changes_moisture() {
        let mut terrain = flat_fixture(1000, true);
        let index = CW + 1;
        terrain.set_ground_surface(index, 0, 1000, true).unwrap();
        terrain.set_water_depth_mm(index, 600).unwrap();
        terrain.take_dirty_pixels();
        let center = (4 * (WW / 2) + 4) * 4;
        let wet = terrain.rgba[center..center + 3].to_vec();
        terrain.take_dirty_cells();
        terrain.set_water_depth_mm(index, 0).unwrap();
        assert!(terrain.take_dirty_pixels().is_empty());
        assert_eq!(&terrain.rgba[center..center + 3], wet.as_slice());
        assert_eq!(terrain.foliage_units(index), Ok(0));
        assert_eq!(
            (terrain.cells[index].flags & SOIL_MOISTURE_MASK) >> SOIL_MOISTURE_SHIFT,
            1000
        );
        terrain.set_ground_surface(index, 0, 0, true).unwrap();
        terrain.take_dirty_pixels();
        for (k, wet_channel) in wet.iter().enumerate() {
            assert!(terrain.rgba[center + k] > *wet_channel);
        }
        assert_eq!(terrain.foliage_units(index), Ok(0));
        assert_eq!(
            terrain.cells[index].flags & GROUND_DISTURBED,
            GROUND_DISTURBED
        );
    }

    #[test]
    fn matches_long_front_reference() {
        let values: serde_json::Value =
            serde_json::from_str(include_str!("../assets/field-reference.json")).unwrap();
        for v in values.as_array().unwrap() {
            let f = Field::new(v["seed"].as_u64().unwrap() as u32);
            let x = v["x"].as_i64().unwrap() as i32;
            let y = v["y"].as_i64().unwrap() as i32;
            assert!((f.hash(x, y) - v["hash"].as_f64().unwrap() as f32).abs() < 0.000001);
            assert_eq!(
                f.height(x as f32, y as f32),
                v["height"].as_f64().unwrap() as f32
            );
            assert!(
                (f.noise(x as f32, y as f32, 103.) - v["noise"].as_f64().unwrap() as f32).abs()
                    < 0.000001
            );
        }
    }
}
