//! Destructible pixel terrain with falling dirt.

use crate::gfx::{Fb, Rgb, H, W};
use macroquad::rand::gen_range;

// DS:5036: six RGB triples in VGA 0..63 units, expanded to 8-bit RGB.
const DIRTS: [Rgb; 6] = [
    Rgb(154, 101, 69),
    Rgb(219, 146, 113),
    Rgb(215, 215, 190),
    Rgb(81, 251, 81),
    Rgb(36, 142, 36),
    Rgb(146, 219, 113),
];

pub struct Terrain {
    solid: Vec<bool>,
    dirty: Vec<bool>,
    sky: Vec<Rgb>,
    dirt: Vec<Rgb>,
    paint: Option<Vec<Rgb>>,
    cavern: bool,
    top: i32,
    pub sky_kind: crate::settings::Sky,
    pub scanned: bool,
}

/// Live terrain and an AI's private erasure overlay share the same projectile code.
pub(crate) trait TerrainAccess {
    fn solid(&self, x: i32, y: i32) -> bool;
    fn erase(&mut self, x: i32, y: i32);
}

pub(crate) struct PredictionTerrain<'a> {
    original: &'a Terrain,
    erased: Vec<(i32, i32)>,
}

impl<'a> PredictionTerrain<'a> {
    pub fn new(original: &'a Terrain) -> Self {
        Self {
            original,
            erased: Vec::new(),
        }
    }
}

impl TerrainAccess for PredictionTerrain<'_> {
    fn solid(&self, x: i32, y: i32) -> bool {
        self.original.solid(x, y) && !self.erased.contains(&(x, y))
    }
    fn erase(&mut self, x: i32, y: i32) {
        self.erased.push((x, y));
    }
}

impl TerrainAccess for Terrain {
    fn solid(&self, x: i32, y: i32) -> bool {
        Terrain::solid(self, x, y)
    }
    fn erase(&mut self, x: i32, y: i32) {
        self.set(x, y, false);
    }
}

impl Terrain {
    pub fn generate() -> Self {
        Self::generate_for(
            &crate::settings::LandscapeSettings::default(),
            crate::ui::HUD_H,
        )
        .0
    }

    pub fn generate_for(
        settings: &crate::settings::LandscapeSettings,
        top: i32,
    ) -> (Self, Option<String>) {
        Self::generate_with_direction(settings, top, false)
    }

    pub fn generate_with_direction(
        settings: &crate::settings::LandscapeSettings,
        top: i32,
        reverse: bool,
    ) -> (Self, Option<String>) {
        let seed = gen_range(1, u32::MAX) as u64;
        let mut rng = crate::landscape::Random::new(seed ^ 0x9127_2145);
        let mut notice = None;
        let mountain = if settings.mountains_folder.is_some()
            && rng.range(100) < u32::from(settings.scanned_mountains_percent)
        {
            match crate::landscape::load_mountain(
                settings.mountains_folder.as_ref().unwrap(),
                &mut rng,
            ) {
                Ok(mountain) => Some(mountain),
                Err(error) => {
                    notice = Some(format!(
                        "{error}\nUsing generated landscape.\nPress any key to continue."
                    ));
                    None
                }
            }
        } else {
            None
        };
        (
            Self::seeded_with_direction(settings, seed, top, mountain.as_ref(), reverse),
            notice,
        )
    }

    #[cfg(test)]
    pub fn seeded(
        settings: &crate::settings::LandscapeSettings,
        seed: u64,
        top: i32,
        mountain: Option<&crate::landscape::Mountain>,
    ) -> Self {
        Self::seeded_with_direction(settings, seed, top, mountain, false)
    }

    pub fn seeded_with_direction(
        settings: &crate::settings::LandscapeSettings,
        seed: u64,
        top: i32,
        mountain: Option<&crate::landscape::Mountain>,
        reverse: bool,
    ) -> Self {
        use crate::landscape::{choose_sky, contours, Random};
        let top = top.clamp(0, H - 160);
        let mut rng = Random::new(seed ^ 0x21d5_28a1);
        let sky_kind = choose_sky(settings.sky, &mut rng);
        let mut palette_rng = Random::new(seed ^ 0x7654_3210);
        let da = DIRTS[palette_rng.range(DIRTS.len() as u32) as usize];
        let dirt = vec![da; H as usize];
        let cavern = sky_kind == crate::settings::Sky::Cavern;
        let recipe = contours(settings, seed ^ 0x1357_9bdf, top, cavern, reverse);
        let heights = recipe.floor;
        let mut solid = vec![false; (W * H) as usize];
        let mut paint = mountain.map(|_| vec![Rgb::BLACK; (W * H) as usize]);
        for x in 0..W {
            for y in top..H {
                let i = (y * W + x) as usize;
                if let Some(mountain) = mountain {
                    let source_x = x as usize * mountain.width / W as usize;
                    let source_y = (y - top) as usize * mountain.height / (H - top) as usize;
                    if let Some(color) = mountain.pixels[source_y * mountain.width + source_x] {
                        solid[i] = true;
                        paint.as_mut().unwrap()[i] = color;
                    }
                } else {
                    solid[i] = y >= heights[x as usize];
                }
            }
        }
        let mut floor: Vec<i32> = (0..W)
            .map(|x| {
                (top..H)
                    .rev()
                    .find(|&y| !solid[(y * W + x) as usize])
                    .map_or(top, |y| y + 1)
            })
            .collect();
        let ceiling = recipe.ceiling;
        if cavern {
            for x in 0..W {
                // Imported art is our extension: leave room below its generated roof.
                // Native floor/roof walks already have the original >=41-pixel gap.
                if mountain.is_some() {
                    floor[x as usize] = floor[x as usize].max((top + H - 1) / 2 + 40).min(H - 1);
                }
                for y in 0..floor[x as usize] {
                    let i = (y * W + x) as usize;
                    solid[i] = y < ceiling[x as usize];
                    if let Some(paint) = &mut paint {
                        paint[i] = dirt[y as usize];
                    }
                }
            }
        }
        let sky = crate::environment::background(sky_kind, seed ^ 0x625a_781b, da, top);
        Self {
            solid,
            dirty: vec![false; W as usize],
            sky,
            dirt,
            paint,
            cavern,
            top,
            sky_kind,
            scanned: mountain.is_some(),
        }
    }

    #[cfg(test)]
    pub(crate) fn flat(top: i32) -> Self {
        Self {
            solid: (0..W * H).map(|i| i / W >= top).collect(),
            dirty: vec![false; W as usize],
            sky: vec![Rgb(0, 0, 80); (W * H) as usize],
            dirt: vec![Rgb(120, 80, 30); H as usize],
            paint: None,
            cavern: false,
            top: crate::ui::HUD_H,
            sky_kind: crate::settings::Sky::Plain,
            scanned: false,
        }
    }

    #[inline]
    pub fn solid(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= W || y < 0 {
            return false;
        }
        if y >= H {
            return true;
        }
        self.solid[(y * W + x) as usize]
    }

    fn set(&mut self, x: i32, y: i32, v: bool) {
        if x >= 0 && x < W && y >= 0 && y < H {
            let i = (y * W + x) as usize;
            if self.solid[i] != v {
                if let Some(paint) = &mut self.paint {
                    paint[i] = self.dirt[y as usize];
                }
            }
            self.solid[i] = v;
            self.dirty[x as usize] = true;
        }
    }

    pub fn surface(&self, x: i32) -> i32 {
        let start = if self.cavern {
            (0..H).find(|&y| !self.solid(x, y)).unwrap_or(H)
        } else {
            0
        };
        (start..H).find(|&y| self.solid(x, y)).unwrap_or(H)
    }
    pub fn floor_surface(&self, x: i32) -> i32 {
        (0..H)
            .rev()
            .find(|&y| !self.solid(x, y))
            .map_or(0, |y| y + 1)
    }

    fn circle(&mut self, cx: i32, cy: i32, r: i32, v: bool, min_y: i32) {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r && cy + dy >= min_y {
                    self.set(cx + dx, cy + dy, v);
                }
            }
        }
    }

    pub fn carve(&mut self, cx: i32, cy: i32, r: i32) {
        self.circle(cx, cy, r, false, 0);
    }

    pub fn add_dirt(&mut self, cx: i32, cy: i32, r: i32, min_y: i32) {
        self.circle(cx, cy, r, true, min_y);
    }

    pub fn disturb_all(&mut self) {
        self.dirty.fill(true);
    }

    pub fn carve_wedge(&mut self, cx: i32, cy: i32, radius: i32, angle: f32, half_angle: f32) {
        let direction =
            macroquad::prelude::vec2(angle.to_radians().cos(), -angle.to_radians().sin());
        let threshold = half_angle.to_radians().cos();
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let offset = macroquad::prelude::vec2(dx as f32, dy as f32);
                let distance = offset.length();
                if distance <= radius as f32
                    && (distance == 0.0 || offset.dot(direction) >= threshold * distance - 0.0001)
                {
                    self.set(cx + dx, cy + dy, false);
                }
            }
        }
    }

    /// Makes a flat platform `2*half+1` wide at the surface height of `x`.
    pub fn flatten(&mut self, x: i32, half: i32) -> i32 {
        let top = self.floor_surface(x).clamp(self.top + 24, H - 10);
        for cx in x - half..=x + half {
            let ceiling = if self.cavern {
                (0..H).find(|&y| !self.solid(cx, y)).unwrap_or(0)
            } else {
                0
            };
            for y in ceiling..H {
                self.set(cx, y, y >= top);
            }
        }
        top
    }

    /// Moves every floating dirt pixel down by one. Returns true if anything moved.
    pub fn settle_step(&mut self) -> bool {
        let mut any = false;
        for x in 0..W {
            if !self.dirty[x as usize] {
                continue;
            }
            let mut moved = false;
            let anchored = if self.cavern {
                (0..H).find(|&y| !self.solid(x, y)).unwrap_or(H)
            } else {
                0
            };
            for y in (anchored..H - 1).rev() {
                let i = (y * W + x) as usize;
                let j = i + W as usize;
                if self.solid[i] && !self.solid[j] {
                    self.solid[i] = false;
                    self.solid[j] = true;
                    if let Some(paint) = &mut self.paint {
                        paint[j] = paint[i];
                        paint[i] = self.dirt[y as usize];
                    }
                    moved = true;
                }
            }
            if moved {
                any = true;
            } else {
                self.dirty[x as usize] = false;
            }
        }
        any
    }

    pub fn render(&self, fb: &mut Fb) {
        let px = fb.pixels_mut();
        for y in 0..H {
            let d = self.dirt[y as usize];
            for x in 0..W {
                let i = (y * W + x) as usize;
                let c = if self.solid[i] {
                    self.paint.as_ref().map_or(d, |paint| paint[i])
                } else {
                    self.sky[i]
                };
                let o = i * 4;
                px[o] = c.0;
                px[o + 1] = c.1;
                px[o + 2] = c.2;
                px[o + 3] = 255;
            }
        }
    }
}

#[cfg(test)]
mod landscape_tests {
    use super::*;
    use crate::settings::{Settings, Sky};
    #[test]
    fn terrain_fidelity_previews() {
        let Ok(directory) = std::env::var("SCORCHED_TERRAIN_FIDELITY_DIR") else {
            return;
        };
        std::fs::create_dir_all(&directory).unwrap();
        let mut manifest = String::from("source = \"rust\"\n");
        let mut fb = crate::gfx::Fb::new();
        for (sky, label) in [(Sky::Plain, "plain"), (Sky::Cavern, "cavern")] {
            for index in 0..30 {
                let settings = crate::settings::LandscapeSettings {
                    sky,
                    scanned_mountains_percent: 0,
                    ..Settings::default().landscape
                };
                let seed = (index + 1) * 7919;
                let terrain = Terrain::seeded(&settings, seed, 24, None);
                terrain.render(&mut fb);
                let name = format!("rust-{label}-{index:02}.png");
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}"));
                manifest.push_str(&format!("\n[[samples]]\nsky = \"{label}\"\nfile = \"{name}\"\nseed = {seed}\ntop = 24\nbottom = 479\n"));
            }
        }
        std::fs::write(format!("{directory}/manifest.toml"), manifest).unwrap();
    }
    #[test]
    fn cavern_roof_is_anchored_but_detached_pieces_fall_and_spawn_uses_floor() {
        let mut settings = Settings::default().landscape;
        settings.sky = Sky::Cavern;
        let mut terrain = Terrain::seeded(&settings, 731, 44, None);
        let roof = (0..H).find(|&y| !terrain.solid(300, y)).unwrap();
        terrain.disturb_all();
        assert!(!terrain.settle_step());
        let floor = terrain.flatten(300, 9);
        assert!(floor >= roof + 41);
        assert!(terrain.solid(300, roof - 1));
        assert!(!terrain.solid(300, roof));
        assert!(terrain.solid(300, floor));
        assert_eq!(terrain.surface(300), floor);
        assert!(!terrain.settle_step());
        terrain.set(300, roof - 5, false);
        assert!(terrain.settle_step());
        assert!(terrain.solid(300, roof - 6));
        assert!(!terrain.solid(300, roof - 4));
        assert!(terrain.solid(300, roof));
    }
    #[test]
    fn six_palettes_are_reachable_and_external_pixels_keep_their_colours() {
        let mut colors = std::collections::HashSet::new();
        let settings = Settings::default().landscape;
        for seed in 1..=60 {
            let terrain = Terrain::seeded(&settings, seed * 7919, 22, None);
            let c = terrain.dirt[(H - 1) as usize];
            colors.insert((c.0, c.1, c.2));
        }
        assert_eq!(colors.len(), 6);
        assert_eq!(colors, DIRTS.iter().map(|c| (c.0, c.1, c.2)).collect());
        let mountain = crate::landscape::Mountain {
            width: 1,
            height: 2,
            pixels: vec![None, Some(Rgb(123, 45, 67))],
        };
        let mut settings = settings;
        settings.sky = Sky::Black;
        let mut terrain = Terrain::seeded(&settings, 1, 22, Some(&mountain));
        assert!(terrain.scanned);
        assert!(!terrain.solid(100, 100));
        assert!(terrain.solid(100, 400));
        let mut fb = Fb::new();
        terrain.render(&mut fb);
        let i = ((400 * W + 100) * 4) as usize;
        assert_eq!(&fb.img.bytes[i..i + 3], &[123, 45, 67]);
        terrain.carve(100, 400, 3);
        terrain.render(&mut fb);
        assert_eq!(&fb.img.bytes[i..i + 3], &[0, 0, 0]);
    }
    #[test]
    fn ten_tank_pads_fit_the_recovered_cavern_gap_without_removing_the_roof() {
        let settings = crate::settings::LandscapeSettings {
            sky: Sky::Cavern,
            ..Settings::default().landscape
        };
        for top in [22, 120] {
            for seed in 1..=30 {
                let mut terrain = Terrain::seeded(&settings, seed * 7919, top, None);
                for player in 0..10 {
                    let x = 24 + player * 64;
                    let roof = (0..H).find(|&y| !terrain.solid(x, y)).unwrap();
                    let floor = terrain.flatten(x, 9);
                    assert!(floor >= roof + 41);
                    assert!(terrain.solid(x, roof - 1));
                    for dx in -7..=7 {
                        assert!(!terrain.solid(x + dx, floor - 10));
                    }
                }
            }
        }
    }
}
