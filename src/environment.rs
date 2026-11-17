//! Distinct sky backgrounds and a neutral, non-target-seeking storm simulation.
use crate::gfx::{Fb, Rgb, H, W};
use crate::landscape::Random;
use crate::settings::Sky;
use crate::tank::Tank;
use crate::terrain::Terrain;
use macroquad::prelude::{vec2, Vec2};

pub fn background(kind: Sky, seed: u64, earth: Rgb, top: i32) -> Vec<Rgb> {
    let mut pixels = vec![Rgb::BLACK; (W * H) as usize];
    for y in 0..H {
        for x in 0..W {
            let t = y as f32 / (H - 1) as f32;
            let color = match kind {
                Sky::Plain => Rgb(36, 36, 125), // VGA (9,9,31), 42c2:0336
                Sky::Black | Sky::Stars => Rgb::BLACK,
                Sky::Shaded => {
                    Rgb::lerp(Rgb(0, 0, 35), Rgb(60, 110, 210), (t * 24.0).floor() / 24.0)
                }
                Sky::Sunset => {
                    Rgb::lerp(Rgb(35, 0, 65), Rgb(255, 135, 35), (t * 24.0).floor() / 24.0)
                }
                Sky::Stormy => {
                    let cloud = ((x as f32 * 0.025 + y as f32 * 0.045).sin()
                        + (x as f32 * 0.007 - y as f32 * 0.055).cos())
                        * 0.13;
                    Rgb::lerp(
                        Rgb(12, 17, 32),
                        Rgb(80, 88, 105),
                        (t * 0.6 + cloud + 0.12).clamp(0.0, 1.0),
                    )
                }
                Sky::Cavern => {
                    // 42c2:084a..090d: fixed horizontal palette bands, independent
                    // of local surfaces. Palette component = trunc(earth*(i+8)/45).
                    let middle = (top + H - 1) / 2;
                    let half = ((H - 1 - top) / 2).max(1);
                    let band = (29 * (y - middle).abs() / half).min(29) as u32;
                    let shade = |c: u8| {
                        let vga = u32::from(c) >> 2;
                        let component = vga * (band + 8) / 45;
                        ((component << 2) | (component >> 4)) as u8
                    };
                    Rgb(shade(earth.0), shade(earth.1), shade(earth.2))
                }
                Sky::Random => unreachable!("resolve Random before rendering"),
            };
            pixels[(y * W + x) as usize] = color;
        }
    }
    let mut rng = Random::new(seed);
    if kind == Sky::Stars {
        for _ in 0..180 {
            let x = rng.range(W as u32) as usize;
            let y = rng.range((H * 3 / 4) as u32) as usize;
            let shade = 140 + rng.range(116) as u8;
            pixels[y * W as usize + x] = Rgb(shade, shade, shade);
        }
    }
    if kind == Sky::Sunset {
        let (cx, cy, r) = (W * 3 / 4, H * 2 / 5, 34);
        for y in cy - r..=cy + r {
            for x in cx - r..=cx + r {
                if (x - cx).pow(2) + (y - cy).pow(2) <= r * r {
                    pixels[(y * W + x) as usize] = Rgb(255, 220, 100);
                }
            }
        }
    }
    pixels
}

struct Bolt {
    points: Vec<Vec2>,
    time: f32,
}
pub struct Weather {
    pub sky: Sky,
    rng: Random,
    wait: f32,
    bolt: Option<Bolt>,
    top: i32,
}
impl Weather {
    pub fn new(sky: Sky, seed: u64, top: i32) -> Self {
        Self {
            sky,
            rng: Random::new(seed),
            wait: 4.0,
            bolt: None,
            top,
        }
    }
    pub fn update(
        &mut self,
        dt: f32,
        can_strike: bool,
        terrain: &Terrain,
        tanks: &[Tank],
    ) -> Option<Vec2> {
        if let Some(bolt) = &mut self.bolt {
            bolt.time -= dt;
            if bolt.time <= 0.0 {
                self.bolt = None;
            }
        }
        if self.sky != Sky::Stormy || !can_strike {
            return None;
        }
        self.wait -= dt;
        if self.wait > 0.0 {
            return None;
        }
        self.wait = 3.0 + self.rng.unit() as f32 * 5.0;
        let x = self.rng.range(W as u32) as i32;
        Some(self.strike(x, terrain, tanks))
    }
    pub fn strike(&mut self, x: i32, terrain: &Terrain, tanks: &[Tank]) -> Vec2 {
        let x = x.clamp(0, W - 1);
        let end = vec2(
            x as f32,
            terrain.surface(x).min(H - 1).max(self.top + 1) as f32,
        );
        let start = vec2(
            (x + self.rng.range(81) as i32 - 40).clamp(0, W - 1) as f32,
            self.top as f32,
        );
        let mut points = vec![start];
        let mut previous = start;
        let mut impact = end;
        'segments: for i in 1..=16 {
            let t = i as f32 / 16.0;
            let next =
                start.lerp(end, t) + vec2((self.rng.range(17) as f32 - 8.0) * (1.0 - t), 0.0);
            let steps = (next - previous).abs().max_element().ceil().max(1.0) as i32;
            for j in 1..=steps {
                let point = previous.lerp(next, j as f32 / steps as f32);
                let hit_tank = tanks
                    .iter()
                    .filter(|tank| tank.alive && tank.health > 0)
                    .any(|tank| {
                        tank.hit(point.x as i32, point.y as i32)
                            || tank.equipment.active_shield.as_ref().is_some_and(|shield| {
                                point.distance(tank.center())
                                    <= crate::shields::spec(shield.item).unwrap().radius as f32
                            })
                    });
                if hit_tank || terrain.solid(point.x as i32, point.y as i32) {
                    impact = point;
                    points.push(point);
                    break 'segments;
                }
            }
            points.push(next);
            previous = next;
        }
        self.bolt = Some(Bolt { points, time: 0.25 });
        impact
    }
    pub fn draw(&self, fb: &mut Fb) {
        if let Some(bolt) = &self.bolt {
            for pair in bolt.points.windows(2) {
                for offset in [-1, 1] {
                    fb.line(
                        pair[0].x as i32 + offset,
                        pair[0].y as i32,
                        pair[1].x as i32 + offset,
                        pair[1].y as i32,
                        Rgb(90, 150, 255),
                    );
                }
                fb.line(
                    pair[0].x as i32,
                    pair[0].y as i32,
                    pair[1].x as i32,
                    pair[1].y as i32,
                    Rgb::WHITE,
                );
            }
        }
    }
    #[cfg(test)]
    pub fn visible(&self) -> bool {
        self.bolt.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cavern_bands_are_global_and_match_original_vga_samples() {
        let pixels = background(Sky::Cavern, 1, Rgb(36, 142, 36), 24);
        for (y, color) in [
            (124, Rgb(16, 73, 16)),
            (184, Rgb(12, 48, 12)),
            (244, Rgb(4, 24, 4)),
            (249, Rgb(4, 24, 4)),
            (304, Rgb(8, 40, 8)),
        ] {
            for x in [0, 170, 300, W - 1] {
                assert_eq!(pixels[(y * W + x) as usize], color);
            }
        }
        assert_eq!(pixels[(250 * W) as usize], pixels[(251 * W) as usize]);
    }
    #[test]
    fn skies_are_distinct_random_excludes_black_and_weather_pauses() {
        let skies = [
            Sky::Plain,
            Sky::Stormy,
            Sky::Stars,
            Sky::Shaded,
            Sky::Sunset,
            Sky::Cavern,
            Sky::Black,
        ];
        let images: Vec<_> = skies
            .iter()
            .map(|&sky| background(sky, 731, Rgb(130, 80, 30), 22))
            .collect();
        for i in 0..images.len() {
            for j in i + 1..images.len() {
                assert_ne!(images[i], images[j]);
            }
        }
        assert!(images[6].iter().all(|&c| c == Rgb::BLACK));
        let mut rng = Random::new(731);
        let mut seen = Vec::new();
        for _ in 0..500 {
            let sky = crate::landscape::choose_sky(Sky::Random, &mut rng);
            assert_ne!(sky, Sky::Black);
            if !seen.contains(&sky) {
                seen.push(sky);
            }
        }
        assert_eq!(seen.len(), 6);
        let terrain = Terrain::flat(300);
        let mut weather = Weather::new(Sky::Stormy, 731, 22);
        assert!(weather.update(100.0, false, &terrain, &[]).is_none());
        assert!(weather.update(3.0, true, &terrain, &[]).is_none());
        assert!(weather.update(1.1, true, &terrain, &[]).is_some());
        assert!(weather.visible());
        weather.update(0.3, false, &terrain, &[]);
        assert!(!weather.visible());
        for sky in skies.into_iter().filter(|&sky| sky != Sky::Stormy) {
            assert!(Weather::new(sky, 1, 22)
                .update(100.0, true, &terrain, &[])
                .is_none());
        }
    }
}
