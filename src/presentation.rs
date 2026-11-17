//! Pure framebuffer effects; combat damage remains attached to the parent explosion.
use crate::{
    gfx::{Fb, Rgb},
    tank::Shape,
    weapons::Explosion,
};

#[derive(Clone, Copy)]
pub struct Death {
    pub style: u8,
    pub color: Rgb,
    pub shape: Shape,
    pub angle: i32,
}

pub fn death(fb: &mut Fb, e: &Explosion, death: Death) {
    let p = (e.t / e.dur).clamp(0.0, 1.0);
    let (x, y) = (e.pos.x as i32, e.pos.y as i32);
    match death.style {
        // Thud: a flattened hull and a fading puff of dust.
        0 => {
            crate::ui::draw_tank_shape(
                fb,
                x,
                y + 4,
                death.angle,
                death.color.scale(1.0 - p),
                death.shape,
            );
            for n in 0..14 {
                let dx = (n - 7) * 3;
                fb.rect(
                    x + dx,
                    y - (p * 15.0) as i32 + n % 3,
                    2,
                    2,
                    Rgb(150, 120, 85).scale(1.0 - p),
                );
            }
        }
        // One, two, or three escalating bursts.
        1..=3 => {
            let count = i32::from(death.style);
            let stage = ((p * count as f32) as i32).min(count - 1);
            let local = (p * count as f32 - stage as f32).clamp(0.0, 1.0);
            let radius = 12.0 + stage as f32 * 12.0;
            let mut burst =
                Explosion::new(e.pos, radius, crate::weapons::Damage::Death, e.owner, false);
            burst.t = local * burst.dur;
            crate::ui::draw_explosion(fb, &burst);
        }
        // Six flame plumes, with independently swaying tips.
        4 => {
            for n in 0..6 {
                let dx = (n - 3) * 6;
                let height = (65.0 * (p * std::f32::consts::PI).sin()) as i32;
                let sway = ((e.t * 12.0 + n as f32).sin() * 7.0) as i32;
                for h in 0..height {
                    let offset = sway * h / height.max(1);
                    let width = (height - h) / 12 + 1;
                    fb.rect(
                        x + dx + offset - width,
                        y - h,
                        width * 2 + 1,
                        1,
                        if h % 8 < 4 {
                            Rgb(255, 65, 0)
                        } else {
                            Rgb(255, 210, 40)
                        },
                    );
                }
            }
        }
        // Grey bubbles drift upwards and spread.
        5 => {
            for n in 0..18 {
                let dx = (n * 17 % 39 - 19) as f32 * (0.4 + p);
                let cy = y - (p * (30 + n * 3) as f32) as i32;
                let radius = 2 + n % 4;
                for a in 0..20 {
                    let angle = a as f32 * std::f32::consts::TAU / 20.0;
                    fb.put(
                        x + dx as i32 + (angle.cos() * radius as f32) as i32,
                        cy + (angle.sin() * radius as f32) as i32,
                        Rgb(190, 190, 190).scale(1.0 - p * 0.7),
                    );
                }
            }
        }
        // Render the actual hull into a small source, then throw its pixels apart.
        6 => {
            let mut hull = Fb::new();
            crate::ui::draw_tank_shape(&mut hull, 16, 16, death.angle, death.color, death.shape);
            for hy in 4..18 {
                for hx in 3..30 {
                    let i = ((hy * crate::gfx::W + hx) * 4) as usize;
                    let pixel = &hull.img.bytes[i..i + 3];
                    if pixel != [0, 0, 0] {
                        let vx = (hx - 16) as f32 * 5.0;
                        let vy = -(20 + (hx * 13 + hy * 7) % 65) as f32;
                        fb.put(
                            x + ((hx - 16) as f32 + vx * e.t) as i32,
                            y + ((hy - 12) as f32 + vy * e.t + 35.0 * e.t * e.t) as i32,
                            Rgb(pixel[0], pixel[1], pixel[2]).scale(1.0 - p * 0.8),
                        );
                    }
                }
            }
        }
        // Spark rays bounce off the screen edges.
        _ => {
            for n in 0..24 {
                let angle = n as f32 * std::f32::consts::TAU / 24.0;
                let distance = p * 104.0;
                let reflect = |v: i32, size: i32| {
                    let v = v.rem_euclid(2 * size);
                    if v >= size {
                        2 * size - 1 - v
                    } else {
                        v
                    }
                };
                let xx = reflect(x + (angle.cos() * distance) as i32, crate::gfx::W);
                let yy = reflect(y + (angle.sin() * distance) as i32, crate::gfx::H);
                fb.line(
                    xx,
                    yy,
                    xx - (angle.cos() * 7.0) as i32,
                    yy - (angle.sin() * 7.0) as i32,
                    Rgb(255, 240, 150).scale(1.0 - p * 0.6),
                );
            }
        }
    }
}

pub fn nuclear(fb: &mut Fb, e: &Explosion) {
    let radius = e.radius * e.t.clamp(0.0, 1.0);
    let r = radius.ceil() as i32;
    let phase = (e.t * 18.0) as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            if (dx * dx + dy * dy) as f32 > radius * radius {
                continue;
            }
            // Distorted cells give a mottled plasma rather than concentric fire rings.
            let cell_x = (dx + (dy as f32 * 0.24).sin().mul_add(4.0, 0.0) as i32).div_euclid(5);
            let cell_y = (dy + (dx as f32 * 0.19).sin().mul_add(3.0, 0.0) as i32).div_euclid(4);
            let mut hash = (cell_x as u32).wrapping_mul(0x85ebca6b)
                ^ (cell_y as u32).wrapping_mul(0xc2b2ae35)
                ^ (phase as u32).wrapping_mul(0x27d4eb2d);
            hash ^= hash >> 16;
            hash = hash.wrapping_mul(0x7feb352d);
            hash ^= hash >> 15;
            let hash = hash % 13;
            let color = match hash {
                0..=4 => Rgb(12, 0, 0),
                5..=7 => Rgb(110, 0, 0),
                8..=10 => Rgb(210, 12, 4),
                _ => Rgb(255, 55, 12),
            };
            fb.put(e.pos.x as i32 + dx, e.pos.y as i32 + dy, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ui::draw_explosion, weapons::Damage};
    use macroquad::prelude::vec2;

    fn export(fb: &Fb, name: &str) {
        if let Ok(dir) = std::env::var("SCORCHED_PRESENTATION_PREVIEW_DIR") {
            std::fs::create_dir_all(&dir).unwrap();
            let mut img = fb.img.clone();
            img.bytes = fb
                .img
                .bytes
                .as_chunks::<{ crate::gfx::W as usize * 4 }>()
                .0
                .iter()
                .rev()
                .flatten()
                .copied()
                .collect();
            img.export_png(&format!("{dir}/{name}.png"));
        }
    }

    #[test]
    fn nuclear_grows_for_one_second_then_shimmers_at_full_blast_radius() {
        let mut e = Explosion::new(vec2(320.0, 240.0), 75.0, Damage::Weapon(3), 0, false);
        assert_eq!(e.dur, 3.0);
        let count = |fb: &Fb| {
            fb.img
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] > 0)
                .count()
        };
        let mut small = Fb::new();
        e.t = 0.25;
        draw_explosion(&mut small, &e);
        let mut full = Fb::new();
        e.t = 1.0;
        draw_explosion(&mut full, &e);
        assert!(count(&full) > count(&small) * 14);
        let full_count = count(&full);
        export(&full, "nuke-full");
        e.t = 2.8;
        let mut shimmer = Fb::new();
        draw_explosion(&mut shimmer, &e);
        assert_eq!(count(&shimmer), full_count);
        assert_ne!(full.img.bytes, shimmer.img.bytes);
        assert_eq!(e.radius, 75.0);
        assert!(Explosion::new(e.pos, 40.0, Damage::Weapon(2), 0, false).dur == 3.0);
        assert!(Explosion::new(e.pos, 30.0, Damage::Weapon(1), 0, false).dur < 3.0);
    }

    #[test]
    fn death_variants_are_distinct_and_clip_at_screen_edges() {
        use std::{
            collections::HashSet,
            hash::{Hash, Hasher},
        };
        let mut seen = HashSet::new();
        let mut gallery = Fb::new();
        for style in 0..8 {
            let mut e = Explosion::new(vec2(320.0, 240.0), 30.0, Damage::Death, 0, false);
            e.dur = 1.6;
            e.death = Some(Death {
                style,
                color: Rgb(70, 220, 130),
                shape: Shape::Treads,
                angle: 55,
            });
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            for time in [0.25, 0.7, 1.4] {
                e.t = time;
                let mut fb = Fb::new();
                draw_explosion(&mut fb, &e);
                fb.img.bytes.hash(&mut hasher);
            }
            assert!(
                seen.insert(hasher.finish()),
                "duplicate death style {style}"
            );
            e.t = 0.9;
            e.pos = vec2(
                80.0 + f32::from(style % 4) * 160.0,
                180.0 + f32::from(style / 4) * 220.0,
            );
            gallery.text(
                e.pos.x as i32 - 20,
                e.pos.y as i32 - 110,
                &format!("Death {}", style + 1),
                Rgb::WHITE,
            );
            draw_explosion(&mut gallery, &e);
            for pos in [vec2(0.0, 0.0), vec2(639.0, 479.0)] {
                e.pos = pos;
                draw_explosion(&mut Fb::new(), &e);
            }
        }
        export(&gallery, "death-variants");
    }
}
