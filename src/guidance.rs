//! Per-shot guidance state and targeting rules (MECHANICS §8.2).
use crate::gfx::{Rgb, H, W};
use crate::physics::{Physics, MAX_FLIGHT_TIME};
use crate::tank::Tank;
use crate::terrain::TerrainAccess;
use crate::weapons::{ItemId, Proj, Step};
use macroquad::prelude::{vec2, Vec2};

#[derive(Clone, Copy)]
pub struct Guidance {
    pub item: ItemId,
    pub target: Vec2,
    pub locked: Option<usize>,
    pub engaged: bool,
}

pub fn applies(weapon: ItemId) -> bool {
    weapon.item().is_usable_weapon()
        && !matches!(
            weapon,
            ItemId::Mirv
                | ItemId::DeathsHead
                | ItemId::RiotCharge
                | ItemId::RiotBlast
                | ItemId::PlasmaBlast
                | ItemId::Laser
                | ItemId::EarthDisrupter
        )
}

pub fn needs_target(item: ItemId) -> bool {
    matches!(
        item,
        ItemId::BallisticGuidance
            | ItemId::HorizontalGuidance
            | ItemId::VerticalGuidance
            | ItemId::LazyBoy
    )
}

pub fn sorted_tanks(tanks: &[Tank]) -> Vec<usize> {
    let mut order: Vec<_> = (0..tanks.len()).collect();
    order.sort_by_key(|&i| (tanks[i].x, i));
    order
}

pub fn tank_target(tanks: &[Tank], number: usize) -> Option<Vec2> {
    let tank = &tanks[*sorted_tanks(tanks).get(number)?];
    (tank.alive && tank.health > 0).then_some(vec2(tank.x as f32, tank.y as f32))
}

pub fn point_target(tanks: &[Tank], point: Vec2, nearest: bool, top: i32) -> Option<Vec2> {
    if point.x < 0.0 || point.x >= W as f32 || point.y < top as f32 || point.y >= H as f32 {
        return None;
    }
    if !nearest {
        return Some(point);
    }
    tanks
        .iter()
        .filter(|t| t.alive && t.health > 0)
        .map(|t| vec2(t.x as f32, t.y as f32))
        .filter(|p| p.distance_squared(point) <= 100.0 * 100.0)
        .min_by(|a, b| {
            a.distance_squared(point)
                .total_cmp(&b.distance_squared(point))
        })
}

/// Solve delta = direction*power*time + acceleration*time²/2. Drag is deliberately ignored.
pub fn ballistic_power(
    origin: Vec2,
    direction: Vec2,
    target: Vec2,
    physics: &Physics,
    max: i32,
) -> i32 {
    let delta = target - origin;
    let acceleration = vec2(physics.wind, physics.gravity);
    let cross = |a: Vec2, b: Vec2| a.x * b.y - a.y * b.x;
    let denominator = cross(acceleration, direction);
    let t_squared = 2.0 * cross(delta, direction) / denominator;
    if denominator.abs() < 0.00001 || !t_squared.is_finite() || t_squared <= 0.0 {
        return max;
    }
    let power = (delta - acceleration * (t_squared / 2.0)).dot(direction) / t_squared.sqrt();
    if !power.is_finite() || power < 0.0 {
        max
    } else {
        (power.round() as i32).clamp(0, max)
    }
}

pub fn steer(p: &mut Proj, dt: f32, tanks: &[Tank]) -> bool {
    let Some(mut state) = p.guidance else {
        return false;
    };
    if state.item == ItemId::HeatGuidance {
        if state
            .locked
            .is_some_and(|i| !tanks[i].alive || tanks[i].health <= 0)
        {
            state.locked = None;
        }
        if state.locked.is_none() {
            state.locked = tanks
                .iter()
                .enumerate()
                .find(|(i, t)| {
                    *i != p.owner
                        && t.alive
                        && t.health > 0
                        && p.pos.distance_squared(vec2(t.x as f32, t.y as f32)) <= 40.0 * 40.0
                })
                .map(|(i, _)| i);
        }
        if let Some(i) = state.locked {
            state.target = vec2(tanks[i].x as f32, tanks[i].y as f32);
            state.engaged = true;
        } else {
            state.engaged = false;
        }
    } else if !state.engaged {
        let (position, velocity, target) = if state.item == ItemId::HorizontalGuidance {
            (p.pos.y, p.vel.y, state.target.y)
        } else {
            (p.pos.x, p.vel.x, state.target.x)
        };
        state.engaged = (position - target).abs() <= 1.0
            || (position - target) * (position + velocity * dt - target) <= 0.0;
    }
    if state.engaged {
        if state.item == ItemId::HorizontalGuidance {
            p.pos.y = state.target.y;
            p.vel.y = 0.0;
        } else if state.item == ItemId::VerticalGuidance {
            p.pos.x = state.target.x;
            p.vel.x = 0.0;
        }
        let offset = state.target - p.pos;
        let distance_squared = offset.length_squared();
        if distance_squared >= 0.001 {
            let k = if state.item == ItemId::HorizontalGuidance {
                15000.0
            } else {
                10000.0
            };
            p.vel += offset * (k * dt / distance_squared.sqrt().sqrt());
        }
    }
    p.guidance = Some(state);
    state.engaged
        && matches!(
            state.item,
            ItemId::HorizontalGuidance | ItemId::VerticalGuidance
        )
}

pub fn arrival(p: &Proj) -> Option<Vec2> {
    let state = p.guidance?;
    (state.engaged
        && matches!(
            state.item,
            ItemId::HorizontalGuidance | ItemId::VerticalGuidance
        )
        && p.pos.distance_squared(state.target) <= 1.0)
        .then_some(state.target)
}

/// Lazy Boy has its own pixel walk: no air forces or shield interception, and dirt is erased.
pub fn step_lazy<T: TerrainAccess>(
    p: &mut Proj,
    dt: f32,
    terrain: &mut T,
    tanks: &[Tank],
    trail: &mut Vec<(i32, i32, Rgb)>,
) -> Step {
    p.age += dt;
    if p.age > MAX_FLIGHT_TIME {
        return Step::Gone;
    }
    let target = p.guidance.unwrap().target;
    let distance = p.pos.distance(target);
    let travel = distance.min(500.0 * dt);
    let origin = p.pos;
    let direction = (target - origin).try_normalize().unwrap_or(Vec2::ZERO);
    p.vel = direction * 500.0;
    let steps = travel.ceil().max(1.0) as usize;
    for i in 0..=steps {
        p.pos = origin + direction * (travel * i as f32 / steps as f32);
        let (x, y) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32);
        if !(0..W).contains(&x) || !(0..H).contains(&y) {
            return Step::Gone;
        }
        if let Some((victim, _)) = tanks.iter().enumerate().find(|(i, t)| {
            t.alive
                && t.health > 0
                && t.hit(x, y)
                && !(*i == p.owner && p.age < 0.25)
                && !t.hit(target.x.floor() as i32, target.y.floor() as i32)
        }) {
            return Step::Hit(p.pos, Some(victim));
        }
        terrain.erase(x, y);
        trail.push((x, y, tanks[p.owner].color));
    }
    if travel >= distance {
        p.pos = target;
        let victim = tanks
            .iter()
            .enumerate()
            .find(|(i, t)| {
                t.alive
                    && t.health > 0
                    && !(*i == p.owner && p.age < 0.25)
                    && t.hit(target.x.floor() as i32, target.y.floor() as i32)
            })
            .map(|(i, _)| i);
        Step::Hit(target, victim)
    } else {
        Step::Alive
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        physics::SIM_DT,
        settings::PhysicsSettings,
        terrain::Terrain,
        weapons::{self, PKind},
    };

    fn scene(item: ItemId, target: Vec2) -> (Proj, Vec<Tank>, Physics) {
        let mut tank = crate::shop::tests::tank(0);
        tank.x = 50;
        tank.y = 299;
        let shot = Proj {
            pos: vec2(100.0, 150.0),
            vel: vec2(80.0, 120.0),
            owner: 0,
            weapon: ItemId::BabyMissile,
            kind: PKind::Shell,
            radius: 10.0,
            scale: 1.0,
            damage: ItemId::BabyMissile.item().damage(),
            age: 0.0,
            tunneling: false,
            in_dirt: false,
            guidance: Some(Guidance {
                item,
                target,
                locked: None,
                engaged: false,
            }),
        };
        (
            shot,
            vec![tank],
            Physics::new(&PhysicsSettings::default(), 0),
        )
    }

    #[test]
    fn ballistic_solver_accounts_for_wind_ignores_drag_and_clamps_impossible_shots() {
        let origin = vec2(100.0, 300.0);
        let direction = vec2(0.6, -0.8);
        let target = vec2(350.0, 250.0);
        for wind in [-100, 0, 100] {
            let mut physics = Physics::new(&PhysicsSettings::default(), wind);
            let power = ballistic_power(origin, direction, target, &physics, 1000);
            let mut pos = origin;
            let mut vel = direction * power as f32;
            let mut closest = f32::MAX;
            for _ in 0..1800 {
                physics.advance(&mut pos, &mut vel, SIM_DT);
                closest = closest.min(pos.distance(target));
            }
            assert!(closest < 1.0, "wind {wind}: miss {closest}, power {power}");
            physics.air_viscosity = 100;
            assert_eq!(
                ballistic_power(origin, direction, target, &physics, 1000),
                power
            );
        }
        let physics = Physics::new(&PhysicsSettings::default(), 0);
        assert_eq!(
            ballistic_power(origin, vec2(-1.0, 0.0), target, &physics, 170),
            170
        );
        assert_eq!(
            ballistic_power(origin, direction, target, &physics, 100),
            100
        );
        assert_eq!(
            ballistic_power(origin, vec2(0.0, -1.0), target, &physics, 0),
            0
        );
    }

    #[test]
    fn heat_locks_first_live_non_owner_including_teammates_and_tracks_it() {
        let (mut shot, mut tanks, _) = scene(ItemId::HeatGuidance, Vec2::ZERO);
        shot.pos = vec2(100.0, 200.0);
        shot.vel = Vec2::ZERO;
        tanks[0].x = 100;
        tanks[0].y = 200;
        tanks[0].team = Some(1);
        let mut first = tanks[0].clone();
        first.x = 120;
        let mut closer = first.clone();
        closer.x = 110;
        tanks.extend([first, closer]);
        steer(&mut shot, SIM_DT, &tanks);
        assert_eq!(shot.guidance.unwrap().locked, Some(1));
        assert!(shot.vel.x > 0.0);
        tanks[1].x = 500;
        steer(&mut shot, SIM_DT, &tanks);
        assert_eq!(shot.guidance.unwrap().target.x, 500.0);
        tanks[1].health = 0;
        steer(&mut shot, SIM_DT, &tanks);
        assert_eq!(shot.guidance.unwrap().locked, Some(2));
        tanks[2].x = 141;
        shot.guidance.as_mut().unwrap().locked = None;
        steer(&mut shot, SIM_DT, &tanks);
        assert!(shot.guidance.unwrap().locked.is_none());
    }

    #[test]
    fn horizontal_and_vertical_engage_then_reach_exact_points_in_midair() {
        for (item, target) in [
            (ItemId::HorizontalGuidance, vec2(180.0, 180.0)),
            (ItemId::VerticalGuidance, vec2(140.0, 250.0)),
        ] {
            let (mut shot, tanks, mut physics) = scene(item, target);
            physics.gravity = 0.0;
            if item == ItemId::VerticalGuidance {
                shot.vel = vec2(200.0, -150.0);
            }
            let mut terrain = Terrain::flat(400);
            let mut trail = Vec::new();
            let mut hit = None;
            for _ in 0..6000 {
                if shot.guidance.unwrap().engaged {
                    physics.gravity = 500.0;
                    physics.wind = 125.0;
                    physics.air_viscosity = 100;
                }
                match weapons::step_proj(
                    &mut shot,
                    SIM_DT,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut trail,
                ) {
                    Step::Alive => {}
                    Step::Hit(point, _) => {
                        hit = Some(point);
                        break;
                    }
                    _ => panic!("guided shot did not reach target"),
                }
            }
            assert_eq!(hit, Some(target));
            assert!(shot.guidance.unwrap().engaged);
        }
    }

    #[test]
    fn lazy_boy_digs_reaches_exact_point_and_ignores_super_mag() {
        let target = vec2(200.0, 200.0);
        let (mut shot, mut tanks, mut physics) = scene(ItemId::LazyBoy, target);
        shot.pos = vec2(100.0, 200.0);
        let mut defended = tanks[0].clone();
        defended.x = 150;
        defended.y = 214; // Beam passes through shield, above the tank body.
        defended.equipment.active_shield = crate::shields::ActiveShield::new(ItemId::SuperMag);
        tanks.push(defended);
        physics.wind = 1000.0;
        physics.air_viscosity = 100;
        let mut terrain = Terrain::flat(100);
        let mut trail = Vec::new();
        let mut hit = None;
        for _ in 0..200 {
            match weapons::step_proj(
                &mut shot,
                SIM_DT,
                &mut terrain,
                &tanks,
                &physics,
                &mut trail,
            ) {
                Step::Alive => {}
                Step::Hit(point, _) => {
                    hit = Some(point);
                    break;
                }
                _ => panic!("Lazy Boy intercepted"),
            }
        }
        assert_eq!(hit, Some(target));
        assert!(!terrain.solid(150, 200));
        assert!(terrain.solid(150, 201));
        assert_eq!(tanks[1].shield_percent(), 100);
        assert!(shot.age >= 0.19 && shot.age < 0.21);
    }

    #[test]
    fn lazy_boy_hits_a_different_tank_early_but_reaches_the_chosen_tank_point() {
        for target in [vec2(150.0, 200.0), vec2(220.0, 200.0)] {
            let (mut shot, mut tanks, physics) = scene(ItemId::LazyBoy, target);
            shot.pos = vec2(100.0, 200.0);
            let mut victim = tanks[0].clone();
            victim.x = 150;
            victim.y = 200;
            tanks.push(victim);
            let mut terrain = Terrain::flat(300);
            let mut trail = Vec::new();
            let mut hit = None;
            for _ in 0..200 {
                if let Step::Hit(point, victim) = weapons::step_proj(
                    &mut shot,
                    SIM_DT,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut trail,
                ) {
                    hit = Some((point, victim));
                    break;
                }
            }
            let (point, victim) = hit.expect("Lazy Boy hit");
            assert_eq!(victim, Some(1));
            if target.x == 150.0 {
                assert_eq!(point, target);
            } else {
                assert!(point.x < 150.0);
            }
        }
    }

    #[test]
    fn targets_use_left_to_right_numbers_and_bounded_nearest_live_tanks() {
        let (_, mut tanks, _) = scene(ItemId::LazyBoy, Vec2::ZERO);
        tanks[0].x = 300;
        let mut left = tanks[0].clone();
        left.x = 100;
        let mut dead = left.clone();
        dead.x = 200;
        dead.alive = false;
        tanks.extend([left, dead]);
        assert_eq!(tank_target(&tanks, 0), Some(vec2(100.0, 299.0)));
        assert!(tank_target(&tanks, 1).is_none());
        assert_eq!(tank_target(&tanks, 2), Some(vec2(300.0, 299.0)));
        assert!(tank_target(&tanks, 9).is_none());
        assert_eq!(
            point_target(&tanks, vec2(120.0, 290.0), true, 22),
            tank_target(&tanks, 0)
        );
        assert!(point_target(&tanks, vec2(500.0, 290.0), true, 22).is_none());
        assert!(point_target(&tanks, vec2(-1.0, 200.0), false, 22).is_none());
        assert!(point_target(&tanks, vec2(120.0, 21.0), false, 22).is_none());
    }
}
