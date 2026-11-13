//! Computer players: brute-force simulate shots, pick the closest, add noise.

use crate::game::{Tank, WKind, G, POWER_K, WEAPONS, WIND_K};
use crate::gfx::{H, W};
use crate::terrain::Terrain;
use macroquad::prelude::{vec2, Vec2};
use macroquad::rand::gen_range;

#[derive(Clone, Copy, PartialEq)]
pub enum AiKind {
    Moron,
    Shooter,
    Cyborg,
}

pub struct Plan {
    pub angle: i32,
    pub power: i32,
    pub weapon: usize,
}

/// Where a shot would land (simplified: no MIRV split, no rolling).
fn sim(terrain: &Terrain, tanks: &[Tank], me: usize, angle: i32, power: i32, wind: f32) -> Vec2 {
    let (mut pos, dir) = tanks[me].muzzle(angle);
    let mut vel = dir * power as f32 * POWER_K;
    let dt = 1.0 / 200.0;
    for _ in 0..4000 {
        vel.y += G * dt;
        vel.x += wind * WIND_K * dt;
        pos += vel * dt;
        let (x, y) = (pos.x.floor() as i32, pos.y.floor() as i32);
        if x < 0 || x >= W {
            return pos;
        }
        if y >= H {
            return vec2(pos.x, (H - 1) as f32);
        }
        if y >= 0 {
            if terrain.solid(x, y) {
                return pos;
            }
            if tanks.iter().enumerate().any(|(i, t)| i != me && t.alive && t.hit(x, y)) {
                return pos;
            }
        }
    }
    pos
}

pub fn plan(kind: AiKind, terrain: &Terrain, tanks: &[Tank], me: usize, wind: i32) -> Plan {
    let t = &tanks[me];
    let enemies: Vec<usize> = (0..tanks.len()).filter(|&i| i != me && tanks[i].alive).collect();
    let target = match kind {
        AiKind::Cyborg => *enemies.iter().min_by_key(|&&i| tanks[i].health).unwrap(),
        _ => enemies[gen_range(0, enemies.len())],
    };
    let goal = tanks[target].center();
    let home = t.center();

    // (samples, considers wind, angle noise, power noise)
    let (samples, sees_wind, a_noise, p_noise) = match kind {
        AiKind::Moron => (30, false, 12, 120),
        AiKind::Shooter => (250, false, 2, 20),
        AiKind::Cyborg => (500, true, 0, 5),
    };
    let wind = if sees_wind { wind as f32 } else { 0.0 };
    let maxp = t.max_power().max(1);
    let score = |a: i32, p: i32| {
        let hit = sim(terrain, tanks, me, a, p, wind);
        let mut s = hit.distance(goal);
        if hit.distance(home) < 40.0 {
            s += 1000.0;
        }
        s
    };

    let mut best = (90, maxp / 2, f32::MAX);
    for _ in 0..samples {
        let (a, p) = (gen_range(10, 171), gen_range(0, maxp + 1));
        let s = score(a, p);
        if s < best.2 {
            best = (a, p, s);
        }
    }
    for _ in 0..samples {
        let a = (best.0 + gen_range(-3, 4)).clamp(0, 180);
        let p = (best.1 + gen_range(-30, 31)).clamp(0, maxp);
        let s = score(a, p);
        if s < best.2 {
            best = (a, p, s);
        }
    }

    let angle = (best.0 + gen_range(-a_noise, a_noise + 1)).clamp(0, 180);
    let power = (best.1 + gen_range(-p_noise, p_noise + 1)).clamp(0, maxp);

    // Sometimes use something heavier, but not if it would hit ourselves.
    let dist = goal.distance(home);
    let options: Vec<usize> = (1..WEAPONS.len())
        .filter(|&i| {
            let w = &WEAPONS[i];
            t.ammo[i] > 0
                && matches!(w.kind, WKind::Blast | WKind::Mirv | WKind::Funky | WKind::Roller)
                && w.radius + 25.0 < dist
        })
        .collect();
    let weapon = if !options.is_empty() && gen_range(0.0f32, 1.0) < 0.5 {
        options[gen_range(0, options.len())]
    } else {
        0
    };

    Plan { angle, power, weapon }
}
