//! Non-ballistic energy weapons. See TODO batch 12 for unrecovered tuning choices.
use crate::economy::{DamageEvent, DamageTarget};
use crate::gfx::{Fb, Rgb, H, W};
use crate::tank::Tank;
use crate::terrain::Terrain;
use crate::weapons::{explosion_damage, ItemId};
use macroquad::prelude::{vec2, Vec2};

pub fn is_energy(id: ItemId) -> bool {
    matches!(id, ItemId::PlasmaBlast | ItemId::Laser)
}

/// Damage is applied once at discharge; this bounded visual holds the turn open.
#[derive(Default)]
pub struct Effect {
    points: Vec<Vec2>,
    plasma: Option<(Vec2, f32)>,
    remaining: f32,
}

impl Effect {
    pub fn active(&self) -> bool {
        self.remaining > 0.0
    }

    pub fn update(&mut self, dt: f32) {
        self.remaining = (self.remaining - dt).max(0.0);
    }

    pub fn draw(&self, fb: &mut Fb) {
        if !self.active() {
            return;
        }
        if let Some((center, radius)) = self.plasma {
            let grow = (1.0 - self.remaining / 0.5).min(0.4) / 0.4;
            for ring in [0.6, 0.8, 1.0] {
                let r = radius * grow * ring;
                let mut previous = center + vec2(r, 0.0);
                for i in 1..=128 {
                    let a = std::f32::consts::TAU * i as f32 / 128.0;
                    let point = center + vec2(a.cos(), a.sin()) * r;
                    fb.line(
                        previous.x as i32,
                        previous.y as i32,
                        point.x as i32,
                        point.y as i32,
                        Rgb(180, 120, 255),
                    );
                    previous = point;
                }
            }
        }
        for point in &self.points {
            fb.put(point.x as i32, point.y as i32, Rgb(255, 90, 100));
        }
    }
}

fn hit(tank: &mut Tank, owner: usize, victim: usize, amount: i32, events: &mut Vec<DamageEvent>) {
    let absorbed = tank.drain_shield(amount);
    if absorbed > 0 {
        events.push(DamageEvent {
            owner,
            victim,
            amount: absorbed,
            target: DamageTarget::Shield,
        });
    }
    let damage = (amount - absorbed).min(tank.health.max(0));
    if damage > 0 {
        tank.health -= damage;
        tank.power = tank.power.min(tank.max_power());
        tank.last_hit = Some(owner);
        tank.environmental_hit = false;
        events.push(DamageEvent {
            owner,
            victim,
            amount: damage,
            target: DamageTarget::Tank,
        });
    }
}

/// Battery use and ammunition belong to the caller. No guidance or trigger is consumed.
pub fn discharge(
    weapon: ItemId,
    batteries: u8,
    owner: usize,
    top: i32,
    terrain: &mut Terrain,
    tanks: &mut [Tank],
) -> (Effect, Vec<DamageEvent>) {
    let batteries = batteries.min(10);
    let mut events = Vec::new();
    let mut effect = Effect::default();
    if weapon == ItemId::PlasmaBlast {
        let center = tanks[owner].center();
        let radius = 20.0 + 55.0 * f32::from(batteries) / 10.0;
        effect.plasma = Some((center, radius));
        effect.remaining = 0.5;
        for (victim, tank) in tanks.iter_mut().enumerate() {
            if victim == owner || !tank.alive || tank.health <= 0 {
                continue;
            }
            let distance = vec2(tank.x as f32, tank.y as f32).distance(center);
            let damage = explosion_damage(distance, radius, weapon.item().damage());
            hit(tank, owner, victim, damage, &mut events);
        }
    } else if weapon == ItemId::Laser {
        let (origin, direction) = tanks[owner].muzzle(tanks[owner].angle);
        // Local battery-to-energy policy: power per battery. Air costs 40/pixel,
        // dirt costs 80/pixel. Bound traversal even if future power limits change.
        let mut energy = tanks[owner].power.clamp(0, 1000) * i32::from(batteries);
        let mut previous = None;
        for step in 0..(W + H) * 2 {
            let point = origin + direction * step as f32;
            let (x, y) = (point.x.round() as i32, point.y.round() as i32);
            if !(0..W).contains(&x) || !(top..H).contains(&y) {
                break;
            }
            if previous == Some((x, y)) {
                continue;
            }
            previous = Some((x, y));
            let dirt = terrain.solid(x, y);
            let cost = if dirt { 80 } else { 40 };
            if energy < cost {
                break;
            }
            energy -= cost;
            effect.points.push(vec2(x as f32, y as f32));
            if dirt {
                terrain.carve(x, y, 0);
            }
            let mut converted = false;
            for (victim, tank) in tanks.iter_mut().enumerate() {
                if victim == owner || !tank.alive || tank.health <= 0 {
                    continue;
                }
                let shield_contact = tank.equipment.active_shield.as_ref().is_some_and(|s| {
                    point.distance(tank.center())
                        <= crate::shields::spec(s.item).unwrap().radius as f32
                });
                if shield_contact {
                    let shield = tank.equipment.active_shield.as_mut().unwrap();
                    if shield.laser_immune() {
                        // Convert the remaining beam into charge, with no hit award.
                        shield.strength = (shield.strength + energy / 40 + 1).min(200);
                        shield.flash = 0.18;
                        converted = true;
                        break;
                    }
                }
                if shield_contact || tank.hit(x, y) {
                    hit(tank, owner, victim, 10, &mut events);
                }
            }
            if converted {
                break;
            }
        }
        if !effect.points.is_empty() {
            effect.remaining = 0.3;
        }
    }
    (effect, events)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tanks() -> Vec<Tank> {
        [100, 150, 300]
            .into_iter()
            .map(|x| {
                let mut tank = crate::shop::tests::tank(0);
                tank.x = x;
                tank.y = 200;
                tank.angle = 0;
                tank.power = 1000;
                tank
            })
            .collect()
    }

    #[test]
    fn plasma_charge_changes_reach_excludes_owner_and_never_carves() {
        let mut terrain = Terrain::flat(190); // Buried tanks: radiation ignores dirt.
        let before: Vec<_> = (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .map(|(x, y)| terrain.solid(x, y))
            .collect();
        let mut weak = tanks();
        discharge(ItemId::PlasmaBlast, 0, 0, 20, &mut terrain, &mut weak);
        assert_eq!(weak[1].health, 100);
        let mut strong = tanks();
        let (mut effect, events) =
            discharge(ItemId::PlasmaBlast, 10, 0, 20, &mut terrain, &mut strong);
        assert!(strong[1].health <= 0);
        assert_eq!(strong[0].health, 100);
        assert_eq!(strong[2].health, 100);
        assert!(events.iter().all(|e| e.victim == 1 && e.owner == 0));
        for (index, solid) in before.into_iter().enumerate() {
            assert_eq!(terrain.solid(index as i32 % W, index as i32 / W), solid);
        }
        assert!(effect.active());
        effect.update(1.0);
        assert!(!effect.active());
    }

    #[test]
    fn plasma_shields_absorb_and_overflow_instead_of_reflecting() {
        let mut tanks = tanks();
        tanks[1].equipment.active_shield = crate::shields::ActiveShield::new(ItemId::ForceShield);
        let (_, events) = discharge(
            ItemId::PlasmaBlast,
            10,
            0,
            20,
            &mut Terrain::flat(300),
            &mut tanks,
        );
        assert_eq!(tanks[1].health, 90);
        assert!(tanks[1].equipment.active_shield.is_none());
        assert_eq!(events.iter().map(|e| e.amount).sum::<i32>(), 110);
        assert!(events.iter().any(|e| e.target == DamageTarget::Shield));
    }

    #[test]
    fn laser_charge_power_and_dirt_cost_bound_its_reach() {
        let mut tanks = tanks();
        let (none, events) =
            discharge(ItemId::Laser, 0, 0, 20, &mut Terrain::flat(300), &mut tanks);
        assert!(!none.active());
        assert!(events.is_empty());
        let (air, _) = discharge(ItemId::Laser, 1, 0, 20, &mut Terrain::flat(300), &mut tanks);
        assert_eq!(air.points.len(), 25);
        let mut dirt = Terrain::flat(190);
        let (cut, _) = discharge(ItemId::Laser, 1, 0, 20, &mut dirt, &mut tanks);
        assert_eq!(cut.points.len(), 12);
        assert!(!dirt.solid(121, 194));
        assert!(dirt.solid(122, 194)); // No energy for another dirt pixel.
        assert!(dirt.solid(121, 195)); // Narrow cut, no ordinary blast.
        tanks[0].power = 400;
        let (low, _) = discharge(ItemId::Laser, 1, 0, 20, &mut Terrain::flat(300), &mut tanks);
        assert_eq!(low.points.len(), 10);
    }

    #[test]
    fn laser_pulses_cut_regular_shields_and_continue_through_tanks() {
        let mut tanks = tanks();
        tanks[2].x = 200;
        tanks[1].equipment.active_shield = crate::shields::ActiveShield::new(ItemId::Shield);
        let (_, events) = discharge(
            ItemId::Laser,
            10,
            0,
            20,
            &mut Terrain::flat(300),
            &mut tanks,
        );
        assert_eq!(tanks[0].health, 100);
        assert_eq!(tanks[1].health, 0);
        assert_eq!(tanks[2].health, 0);
        assert!(tanks[1].equipment.active_shield.is_none());
        assert!(events.iter().all(|e| e.amount == 10));
        assert_eq!(tanks[1].last_hit, Some(0));
    }

    #[test]
    fn super_mag_converts_and_stops_laser_at_200_without_damage_awards() {
        let mut tanks = tanks();
        tanks[2].x = 200;
        let mut shield = crate::shields::ActiveShield::new(ItemId::SuperMag).unwrap();
        shield.strength = 180;
        tanks[1].equipment.active_shield = Some(shield);
        let (_, events) = discharge(
            ItemId::Laser,
            10,
            0,
            20,
            &mut Terrain::flat(300),
            &mut tanks,
        );
        assert_eq!(tanks[1].shield_percent(), 100);
        assert!(events.is_empty());
        assert!(tanks.iter().all(|t| t.health == 100));
    }

    #[test]
    fn laser_stops_at_field_edges_in_every_direction() {
        for angle in [0, 45, 90, 135, 180] {
            let mut tanks = tanks();
            tanks[0].angle = angle;
            let (effect, _) = discharge(
                ItemId::Laser,
                10,
                0,
                40,
                &mut Terrain::flat(300),
                &mut tanks,
            );
            assert!(effect
                .points
                .iter()
                .all(|p| p.x >= 0.0 && p.x < W as f32 && p.y >= 40.0 && p.y < H as f32));
        }
    }
}
