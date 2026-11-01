//! Derived visual genes only. Never consumes the terrain/biology RNG streams.
pub const FAUNA_TAG: u32 = 0x8000_0000;
const STEPS: usize = 256;

fn mix(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}
fn unit(key: u32, salt: u32) -> f32 {
    (mix(key ^ salt.wrapping_mul(0x9e37_79b9)) & 65535) as f32 / 65535.
}
fn hsv(h: f32, s: f32, v: f32) -> [f32; 3] {
    let sector = h / 60.;
    let c = v * s;
    let x = c * (1. - (sector % 2. - 1.).abs());
    let m = v - c;
    let rgb = if sector < 1. {
        [c, x, 0.]
    } else if sector < 2. {
        [x, c, 0.]
    } else {
        [0., c, x]
    };
    rgb.map(|n| (n + m) * 255.)
}
/// Continuous hue/saturation/value ranges, not a list of predetermined RGBs.
pub fn sample(seed: u32, species: u32, variant: u32, flora: bool) -> [f32; 3] {
    let key = mix(seed ^ species.wrapping_mul(0x9e37_79b9) ^ variant.wrapping_mul(0x85eb_ca6b));
    let choice = unit(key, 1);
    let ranges: [f32; 6] = if flora {
        if choice < 0.70 {
            [65., 134., 0.40, 0.78, 0.35, 0.72]
        } else if choice < 0.82 {
            [18., 46., 0.38, 0.70, 0.25, 0.57]
        } else if choice < 0.92 {
            [0., 18., 0.50, 0.80, 0.30, 0.65]
        } else {
            [42., 64., 0.38, 0.75, 0.50, 0.82]
        }
    } else if choice < 0.26 {
        [65., 130., 0.25, 0.62, 0.30, 0.58]
    } else if choice < 0.64 {
        [18., 42., 0.20, 0.70, 0.22, 0.65]
    } else if choice < 0.80 {
        [0., 22., 0.30, 0.76, 0.28, 0.62]
    } else if choice < 0.93 {
        [36., 62., 0.38, 0.75, 0.48, 0.76]
    } else {
        [25., 55., 0.03, 0.18, 0.12, 0.72]
    };
    let range = |i: usize| ranges[i] + (ranges[i + 1] - ranges[i]) * unit(key, i as u32 + 3);
    hsv(range(0), range(2), range(4))
}

/// ~13 KiB per resident world, independent of tile/animal count.
pub struct NaturalColors {
    flora: Vec<[f32; 3]>,
    accents: [[f32; 3]; 4],
    fauna: [[u8; 3]; STEPS],
}
impl NaturalColors {
    pub fn new(seed: u32) -> Self {
        let mut flora = Vec::with_capacity(4 * STEPS);
        for family in 0..4 {
            let anchors: Vec<_> = (0..8).map(|n| sample(seed, family, n, true)).collect();
            for n in 0..STEPS {
                let t = n as f32 * 7. / (STEPS - 1) as f32;
                let lo = t.floor() as usize;
                let hi = (lo + 1).min(7);
                flora.push(std::array::from_fn(|c| {
                    anchors[lo][c] + (anchors[hi][c] - anchors[lo][c]) * (t - lo as f32)
                }));
            }
        }
        Self {
            flora,
            accents: std::array::from_fn(|n| sample(seed ^ 0x51af_032d, n as u32, 91, false)),
            fauna: std::array::from_fn(|n| {
                sample(seed, n as u32, 37, false).map(|c| c.round() as u8)
            }),
        }
    }
    pub fn foliage(&self, family: usize, field_mix: f64) -> [f32; 3] {
        // Reuse the existing smooth growth/soil field; no grid-shaped colors,
        // extra noise pass or unrelated random stream is introduced.
        let t = (((field_mix - (0.25 + family as f64 * 0.125)) / 0.125).clamp(0., 1.)
            * (STEPS - 1) as f64) as f32;
        let lo = t.floor() as usize;
        let hi = (lo + 1).min(STEPS - 1);
        let base = family * STEPS;
        std::array::from_fn(|c| {
            self.flora[base + lo][c]
                + (self.flora[base + hi][c] - self.flora[base + lo][c]) * (t - lo as f32)
        })
    }
    pub fn accent(&self, family: usize) -> [f32; 3] {
        self.accents[family]
    }
    /// Draw-only role4 appearance: tag, existing atlas family, and 24-bit coat.
    /// Interpolating cached samples gives species many more than256 colors.
    pub fn fauna_appearance(&self, species: u32) -> u32 {
        let key = mix(species ^ 0x612b_90ad);
        let i = (key & 255) as usize;
        let t = (key >> 8) & 255;
        let rgb: [u8; 3] = std::array::from_fn(|c| {
            ((self.fauna[i][c] as u32 * (255 - t) + self.fauna[(i + 1) & 255][c] as u32 * t + 127)
                / 255) as u8
        });
        FAUNA_TAG
            | (species % 4) << 24
            | rgb[0] as u32
            | (rgb[1] as u32) << 8
            | (rgb[2] as u32) << 16
    }
}
pub fn valid_fauna_appearance(packed: u32) -> bool {
    packed & FAUNA_TAG != 0 && packed & 0x7c00_0000 == 0
}
/// Same luminance modulation as the role4 fragment shader; keeps pixel detail.
pub fn tint_fauna(pixel: [u8; 4], packed: u32) -> [u8; 4] {
    let shade = 0.35 + (pixel[0] as f32 + pixel[1] as f32 + pixel[2] as f32) / (3. * 255.) * 1.15;
    let mut result = pixel;
    for c in 0..3 {
        result[c] = ((((packed >> (c * 8)) & 255) as f32 * shade)
            .round()
            .clamp(0., 255.)) as u8;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn natural_genes_span_colors_repeat_by_seed_and_exclude_pink_and_cyan() {
        let mut bins = [0usize; 4];
        let mut distinct = std::collections::BTreeSet::new();
        for seed in 0..32 {
            for species in 0..128 {
                let a = sample(seed, species, 0, true);
                assert_eq!(a, sample(seed, species, 0, true));
                assert_ne!(a, sample(seed.wrapping_add(1), species, 0, true));
                assert!(a.iter().all(|c| c.is_finite() && (0. ..=255.).contains(c)));
                let max = a[0].max(a[1]).max(a[2]);
                let min = a[0].min(a[1]).min(a[2]);
                let hue = if max == a[0] {
                    60. * ((a[1] - a[2]) / (max - min)).rem_euclid(6.)
                } else if max == a[1] {
                    60. * ((a[2] - a[0]) / (max - min) + 2.)
                } else {
                    240.
                };
                assert!((0. ..=134.01).contains(&hue), "forbidden hue{hue}");
                bins[if hue < 18. {
                    0
                } else if hue < 42. {
                    1
                } else if hue < 65. {
                    2
                } else {
                    3
                }] += 1;
                distinct.insert(a.map(|c| c.round() as u8));
            }
        }
        assert!(
            bins.iter().all(|n| *n > 100),
            "missing natural family{bins:?}"
        );
        assert!(distinct.len() > 3000);
    }
    #[test]
    fn rust_matches_actual_js_generator_vectors() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../assets/natural-colour-golden.json")).unwrap();
        for case in fixtures["cases"].as_array().unwrap() {
            let actual = sample(
                case["seed"].as_u64().unwrap() as u32,
                case["species"].as_u64().unwrap() as u32,
                37,
                false,
            );
            for c in 0..3 {
                assert!(
                    (actual[c] as f64 - case["rgb"][c].as_f64().unwrap()).abs() < 0.002,
                    "cross-language color mismatch{case}"
                );
            }
        }
    }
    #[test]
    fn world_cache_changes_species_coats_and_preserves_alpha_and_packet_layout() {
        let a = NaturalColors::new(78421);
        let b = NaturalColors::new(31415);
        let mut coats = std::collections::BTreeSet::new();
        for species in 0..256 {
            let p = a.fauna_appearance(species);
            assert!(valid_fauna_appearance(p));
            assert_eq!((p >> 24) & 3, species % 4);
            assert_eq!(p, a.fauna_appearance(species));
            coats.insert(p & 0x00ff_ffff);
            assert_eq!(tint_fauna([120, 140, 100, 73], p)[3], 73);
        }
        assert!(coats.len() > 240);
        assert_ne!(a.fauna_appearance(3), b.fauna_appearance(3));
        for family in 0..4 {
            for t in [0.25, 0.4, 0.5, 0.65, 0.8] {
                assert_eq!(
                    a.foliage(family, t),
                    NaturalColors::new(78421).foliage(family, t)
                );
            }
        }
        assert!(!valid_fauna_appearance(4));
        assert!(!valid_fauna_appearance(0x8400_0000));
    }
}
