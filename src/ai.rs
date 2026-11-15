//! Computer players: brute-force simulate shots, pick the closest, add noise.

use crate::physics::{Physics, MAX_FLIGHT_TIME, SIM_DT};
use crate::tank::Tank;
use crate::terrain::{PredictionTerrain, Terrain};
use crate::weapons::{self, ItemId, ItemKind, PKind, Proj, Step, WKind, ITEMS};
use macroquad::prelude::Vec2;
use macroquad::rand::gen_range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiKind {
    Moron,
    Shooter,
    Spoiler,
}

pub struct Plan {
    pub angle: i32,
    pub power: i32,
    pub weapon: ItemId,
}

/// Where a shot would land (simplified: no MIRV split, no rolling).
fn sim(
    terrain: &Terrain,
    tanks: &[Tank],
    me: usize,
    angle: i32,
    power: i32,
    physics: &Physics,
    tunneling: bool,
) -> Vec2 {
    let (pos, dir) = tanks[me].muzzle(angle);
    let mut shot = Proj {
        pos,
        vel: dir * power as f32,
        owner: me,
        weapon: ItemId::BabyMissile,
        kind: PKind::Shell,
        radius: 10.0,
        damage: ItemId::BabyMissile.item().damage(),
        age: 0.0,
        tunneling,
        in_dirt: false,
    };
    let mut trail = Vec::with_capacity(1);
    let mut predicted_terrain = PredictionTerrain::new(terrain);
    for _ in 0..=(MAX_FLIGHT_TIME / SIM_DT) as usize {
        match weapons::step_proj(
            &mut shot,
            SIM_DT,
            &mut predicted_terrain,
            tanks,
            physics,
            &mut trail,
        ) {
            Step::Alive => trail.clear(),
            Step::Hit(pos, _) => return pos,
            Step::Gone | Step::Split => break,
        }
    }
    shot.pos
}

fn aiming_physics(kind: AiKind, physics: &Physics) -> Physics {
    let mut predicted = *physics;
    if kind != AiKind::Spoiler {
        predicted.wind = 0.0;
    }
    if matches!(kind, AiKind::Shooter | AiKind::Spoiler) {
        predicted.air_viscosity = 0;
    }
    predicted
}

pub fn plan(
    kind: AiKind,
    terrain: &Terrain,
    tanks: &[Tank],
    me: usize,
    physics: &Physics,
    tunneling: bool,
) -> Plan {
    let t = &tanks[me];
    let enemies: Vec<usize> = (0..tanks.len()).filter(|&i| i != me && tanks[i].alive).collect();
    let target = match kind {
        AiKind::Spoiler => *enemies.iter().min_by_key(|&&i| tanks[i].health).unwrap(),
        _ => enemies[gen_range(0, enemies.len())],
    };
    let goal = tanks[target].center();
    let home = t.center();

    // (samples, angle noise, power noise); original aiming algorithms arrive in batch 16.
    let (samples, a_noise, p_noise) = match kind {
        AiKind::Moron => (30, 12, 120),
        AiKind::Shooter => (250, 2, 20),
        AiKind::Spoiler => (500, 0, 5),
    };
    let predicted = aiming_physics(kind, physics);
    let maxp = t.max_power().max(1);
    let score = |a: i32, p: i32| {
        let hit = sim(terrain, tanks, me, a, p, &predicted, tunneling);
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
    let options: Vec<ItemId> = ITEMS
        .iter()
        .filter(|item| {
            item.id != ItemId::BabyMissile
                && t.inventory.can_select(item.id)
                && matches!(
                    item.kind,
                    ItemKind::Weapon(WKind::Blast | WKind::Mirv | WKind::Funky | WKind::Roller)
                )
                && item.radius + 25.0 < dist
        })
        .map(|item| item.id)
        .collect();
    let weapon = if !options.is_empty() && gen_range(0.0f32, 1.0) < 0.5 {
        options[gen_range(0, options.len())]
    } else {
        ItemId::BabyMissile
    };

    Plan { angle, power, weapon }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gfx::Rgb, inventory::Inventory, settings::PhysicsSettings, tank::Ctrl};

    fn tanks() -> Vec<Tank> {
        [100, 600]
            .into_iter()
            .map(|x| Tank {
                name: "Test".into(),
                color: Rgb::WHITE,
                ctrl: Ctrl::Human,
                account: crate::economy::Account::default(),
                team: None,
                x,
                y: 400,
                angle: 60,
                power: 400,
                health: 100,
                alive: true,
                inventory: Inventory::default(),
                weapon: ItemId::BabyMissile,
                fall: 0,
                last_hit: None,
                kills: 0,
                wins: 0,
            })
            .collect()
    }

    #[test]
    fn predicted_shots_follow_gravity_and_wind_in_original_units() {
        let terrain = Terrain::flat(450);
        let tanks = tanks();
        for gravity in [0.2, 0.4] {
            for wind in [-40, 40] {
                let mut settings = PhysicsSettings::default();
                settings.gravity = gravity;
                let physics = Physics::new(&settings, wind);
                let hit = sim(&terrain, &tanks, 0, 60, 400, &physics, false);
                let (start, dir) = tanks[0].muzzle(60);
                let velocity = dir * 400.0;
                let time = (-velocity.y
                    + (velocity.y * velocity.y + 2.0 * physics.gravity * (450.0 - start.y)).sqrt())
                    / physics.gravity;
                let expected = start.x + velocity.x * time + 0.5 * physics.wind * time * time;
                assert!(
                    (hit.x - expected).abs() < 1.0,
                    "gravity={gravity} wind={wind}: {} vs {expected}",
                    hit.x
                );
                assert!(hit.y >= 450.0 && hit.y < 452.0);
            }
        }
    }

    #[test]
    fn tunneling_predictions_are_repeatable_and_never_erase_live_terrain() {
        let terrain = Terrain::flat(350);
        let tanks = tanks();
        let physics = Physics::new(&PhysicsSettings::default(), 0);
        let surface_hit = sim(&terrain, &tanks, 0, 60, 1000, &physics, false);
        let buried_hit = sim(&terrain, &tanks, 0, 60, 1000, &physics, true);
        let repeat = sim(&terrain, &tanks, 0, 60, 1000, &physics, true);
        assert!(buried_hit.x > surface_hit.x + 3.0);
        assert_eq!(buried_hit, repeat);
        assert!(terrain.solid(surface_hit.x.floor() as i32, surface_hit.y.floor() as i32));
        assert!(terrain.solid(buried_hit.x.floor() as i32, buried_hit.y.floor() as i32));
    }

    #[test]
    fn smart_ai_keeps_its_drag_weakness_while_spoiler_accounts_for_wind() {
        let terrain = Terrain::flat(450);
        let tanks = tanks();
        let mut settings = PhysicsSettings::default();
        settings.air_viscosity = 20;
        let actual = Physics::new(&settings, 40);
        let spoiler = aiming_physics(AiKind::Spoiler, &actual);
        let shooter = aiming_physics(AiKind::Shooter, &actual);
        assert_eq!(spoiler.wind, actual.wind);
        assert_eq!(spoiler.gravity, actual.gravity);
        assert_eq!(spoiler.air_viscosity, 0);
        assert_eq!(shooter.wind, 0.0);
        assert_eq!(shooter.air_viscosity, 0);
        assert_eq!(aiming_physics(AiKind::Moron, &actual).air_viscosity, 20);
        assert!(
            sim(&terrain, &tanks, 0, 60, 400, &spoiler, false).x
                - sim(&terrain, &tanks, 0, 60, 400, &actual, false).x
                > 50.0
        );
        // A real plan remains usable with changed physics and exhausted heavy ammunition.
        let plan = plan(AiKind::Spoiler, &terrain, &tanks, 0, &actual, true);
        assert!((0..=180).contains(&plan.angle));
        assert!((0..=tanks[0].max_power()).contains(&plan.power));
        assert!(tanks[0].inventory.can_select(plan.weapon));
    }
}
