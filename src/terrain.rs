//! Destructible pixel terrain with falling dirt.

use crate::gfx::{Fb, Rgb, H, W};
use macroquad::rand::gen_range;

const SKIES: [(Rgb, Rgb); 4] = [
    (Rgb(0, 0, 40), Rgb(110, 40, 150)),    // dusk
    (Rgb(0, 0, 0), Rgb(0, 0, 140)),        // night
    (Rgb(30, 60, 190), Rgb(170, 200, 255)), // day
    (Rgb(60, 0, 20), Rgb(255, 150, 40)),   // sunset
];

const DIRTS: [(Rgb, Rgb); 4] = [
    (Rgb(190, 150, 70), Rgb(80, 45, 15)),  // sand / earth
    (Rgb(70, 190, 70), Rgb(15, 60, 15)),   // grass
    (Rgb(210, 210, 225), Rgb(80, 80, 110)), // snow
    (Rgb(210, 90, 40), Rgb(90, 25, 10)),   // mars
];

pub struct Terrain {
    solid: Vec<bool>,
    dirty: Vec<bool>,
    sky: Vec<Rgb>,
    dirt: Vec<Rgb>,
}

/// Per-row gradient quantised to `n` bands, like a 256-colour VGA palette.
fn bands(a: Rgb, b: Rgb, n: i32, y0: i32, y1: i32) -> Vec<Rgb> {
    (0..H)
        .map(|y| {
            let t = ((y - y0) as f32 / (y1 - y0) as f32).clamp(0.0, 1.0);
            let band = ((t * n as f32) as i32).min(n - 1);
            Rgb::lerp(a, b, band as f32 / (n - 1) as f32)
        })
        .collect()
}

impl Terrain {
    pub fn generate() -> Self {
        // Midpoint displacement for the mountain silhouette.
        let n = 1024usize;
        let mut h = vec![0.0f32; n + 1];
        h[0] = gen_range(150.0, 420.0);
        h[n] = gen_range(150.0, 420.0);
        let mut step = n;
        let mut amp = gen_range(90.0, 180.0);
        while step > 1 {
            let half = step / 2;
            let mut i = half;
            while i < n {
                h[i] = (h[i - half] + h[i + half]) / 2.0 + gen_range(-amp, amp);
                i += step;
            }
            amp *= 0.55;
            step = half;
        }

        let mut solid = vec![false; (W * H) as usize];
        for x in 0..W {
            let top = h[x as usize * n / W as usize].clamp(110.0, (H - 20) as f32) as i32;
            for y in top..H {
                solid[(y * W + x) as usize] = true;
            }
        }

        // Never pair a sky with its same-index (similar-hued) ground.
        let si = gen_range(0, SKIES.len());
        let di = (si + gen_range(1, DIRTS.len())) % DIRTS.len();
        let (sa, sb) = SKIES[si];
        let (da, db) = DIRTS[di];
        Terrain {
            solid,
            dirty: vec![false; W as usize],
            sky: bands(sa, sb, 16, 0, H),
            dirt: bands(da, db, 12, 110, H),
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
            self.solid[(y * W + x) as usize] = v;
            self.dirty[x as usize] = true;
        }
    }

    pub fn surface(&self, x: i32) -> i32 {
        (0..H).find(|&y| self.solid(x, y)).unwrap_or(H)
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

    /// Makes a flat platform `2*half+1` wide at the surface height of `x`.
    pub fn flatten(&mut self, x: i32, half: i32) -> i32 {
        let top = self.surface(x).min(H - 10);
        for cx in x - half..=x + half {
            for y in 0..H {
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
            for y in (0..H - 1).rev() {
                let i = (y * W + x) as usize;
                let j = i + W as usize;
                if self.solid[i] && !self.solid[j] {
                    self.solid[i] = false;
                    self.solid[j] = true;
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
            let (s, d) = (self.sky[y as usize], self.dirt[y as usize]);
            for x in 0..W {
                let i = (y * W + x) as usize;
                let c = if self.solid[i] { d } else { s };
                let o = i * 4;
                px[o] = c.0;
                px[o + 1] = c.1;
                px[o + 2] = c.2;
                px[o + 3] = 255;
            }
        }
    }
}
