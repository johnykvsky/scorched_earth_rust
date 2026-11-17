//! Shield state and shared collision rules for live and predicted projectiles.
use crate::gfx::{Fb, Rgb, H};
use crate::tank::Tank;
use crate::weapons::{ItemId, ItemKind, WKind};
use macroquad::prelude::{vec2, Vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShieldSpec {
    pub strength: i32,
    pub radius: i32,
    pub color: Rgb,
    vga: [i32; 3],
}
pub fn spec(id: ItemId) -> Option<ShieldSpec> {
    let (strength, radius, rgb) = match id {
        ItemId::MagDeflector => (55, 16, [63, 63, 23]),
        ItemId::Shield => (100, 15, [63, 63, 63]),
        ItemId::ForceShield => (100, 15, [63, 23, 63]),
        ItemId::HeavyShield => (150, 16, [63, 63, 63]),
        ItemId::SuperMag => (200, 16, [63, 53, 33]),
        _ => return None,
    };
    Some(ShieldSpec {
        strength,
        radius,
        vga: rgb,
        color: Rgb(
            (rgb[0] * 255 / 63) as u8,
            (rgb[1] * 255 / 63) as u8,
            (rgb[2] * 255 / 63) as u8,
        ),
    })
}
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveShield {
    pub item: ItemId,
    pub strength: i32,
    pub flash: f32,
}
impl ActiveShield {
    pub fn new(item: ItemId) -> Option<Self> {
        Some(Self {
            item,
            strength: spec(item)?.strength,
            flash: 0.0,
        })
    }
    pub fn percent(&self) -> i32 {
        self.strength.max(0) * 100 / spec(self.item).unwrap().strength
    }
    pub fn magnetic(&self) -> bool {
        matches!(self.item, ItemId::MagDeflector | ItemId::SuperMag)
    }
    pub fn reflective(&self) -> bool {
        matches!(self.item, ItemId::ForceShield | ItemId::SuperMag)
    }
    /// Super Mag converts incoming laser energy into charge instead of taking damage.
    pub fn laser_immune(&self) -> bool {
        self.item == ItemId::SuperMag
    }
    pub fn color(&self) -> Rgb {
        let base = spec(self.item).unwrap();
        let channel =
            |v| ((v * self.strength.clamp(0, base.strength) / base.strength) * 255 / 63) as u8;
        Rgb(
            channel(base.vga[0]),
            channel(base.vga[1]),
            channel(base.vga[2]),
        )
    }
}

pub enum Contact {
    Hit {
        victim: usize,
        cost: i32,
        reflection: Option<(Vec2, Vec2)>,
    },
    Roll {
        victim: usize,
        angle: f32,
        direction: f32,
    },
}

pub fn contact(
    pos: Vec2,
    velocity: Vec2,
    owner: usize,
    weapon: ItemId,
    tanks: &[Tank],
) -> Option<Contact> {
    for (victim, tank) in tanks.iter().enumerate() {
        if victim == owner || !tank.alive || tank.health <= 0 {
            continue;
        }
        let Some(shield) = &tank.equipment.active_shield else {
            continue;
        };
        if shield.strength <= 0 {
            continue;
        }
        let radius = spec(shield.item).unwrap().radius as f32;
        let offset = pos - tank.center();
        if offset.length_squared() > radius * radius || offset.dot(velocity) > 0.0 {
            continue;
        }
        let normal = offset.try_normalize().unwrap_or(vec2(0.0, -1.0));
        if weapon.item().kind == ItemKind::Weapon(WKind::Roller) {
            let angle = normal.y.atan2(normal.x).clamp(-std::f32::consts::PI, 0.0);
            return Some(Contact::Roll {
                victim,
                angle,
                direction: if normal.x.abs() > 0.15 {
                    normal.x.signum()
                } else if velocity.x < 0.0 {
                    -1.0
                } else {
                    1.0
                },
            });
        }
        if shield.reflective() {
            let reflected = (velocity - 2.0 * velocity.dot(normal) * normal) * 0.7;
            return Some(Contact::Hit {
                victim,
                cost: (velocity.length() / 100.0).round() as i32,
                reflection: Some((tank.center() + normal * (radius + 0.5), reflected)),
            });
        }
        return Some(Contact::Hit {
            victim,
            cost: 10,
            reflection: None,
        });
    }
    None
}

/// Fixed-step translation of the reciprocal-step force: use 2500 px/s² so
/// animation delays cannot alter trajectories. Exact DOSBox tuning is still
/// a fidelity check (the recovered rule is 50 / the original step divisor).
pub fn magnetic_push(
    pos: Vec2,
    velocity: &mut Vec2,
    owner: usize,
    dt: f32,
    tanks: &[Tank],
) -> bool {
    if velocity.x == 0.0 {
        return false;
    }
    let mut affected = false;
    for (i, tank) in tanks.iter().enumerate() {
        if i == owner || !tank.alive || tank.health <= 0 {
            continue;
        }
        if !tank
            .equipment
            .active_shield
            .as_ref()
            .is_some_and(|shield| shield.strength > 0 && shield.magnetic())
        {
            continue;
        }
        let height = tank.y as f32 - pos.y;
        if (tank.x as f32 - pos.x).abs() <= 15.0 && (0.0..=(H - 1) as f32 / 4.0).contains(&height) {
            velocity.y -= 2500.0 * dt;
            affected = true;
        }
    }
    affected
}

pub fn draw(fb: &mut Fb, tank: &Tank, time: f32) {
    let Some(shield) = &tank.equipment.active_shield else {
        return;
    };
    if shield.strength <= 0 || !tank.alive {
        return;
    }
    if shield.flash > 0.0 && (time * 40.0) as i32 % 2 == 0 {
        return;
    }
    let center = tank.center();
    let radius = spec(shield.item).unwrap().radius as f32;
    let color = if shield.flash > 0.0 {
        Rgb::WHITE
    } else {
        shield.color()
    };
    let mut previous = center + vec2(radius, 0.0);
    for i in 1..=96 {
        let a = std::f32::consts::TAU * i as f32 / 96.0;
        let point = center + vec2(a.cos(), a.sin()) * radius;
        fb.line(
            previous.x.round() as i32,
            previous.y.round() as i32,
            point.x.round() as i32,
            point.y.round() as i32,
            color,
        );
        previous = point;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{physics::SIM_DT, shop::tests::tank};

    fn protected(id: ItemId) -> Tank {
        let mut t = tank(0);
        t.x = 300;
        t.y = 300;
        t.inventory.add(id, 2);
        assert!(t.engage_shield(id));
        t
    }

    #[test]
    fn shield_table_engagement_replacement_and_strength_fading_match_documented_values() {
        for (id, strength, radius, color) in [
            (ItemId::MagDeflector, 55, 16, Rgb(255, 255, 93)),
            (ItemId::Shield, 100, 15, Rgb::WHITE),
            (ItemId::ForceShield, 100, 15, Rgb(255, 93, 255)),
            (ItemId::HeavyShield, 150, 16, Rgb::WHITE),
            (ItemId::SuperMag, 200, 16, Rgb(255, 214, 133)),
        ] {
            let data = spec(id).unwrap();
            assert_eq!(data.strength, strength);
            assert_eq!(data.radius, radius);
            assert_eq!(data.color, color);
            let mut t = protected(id);
            assert_eq!(t.inventory.count(id), 1);
            assert_eq!(t.shield_percent(), 100);
            assert_eq!(t.equipment.active_shield.as_ref().unwrap().color(), color);
            assert!(t.engage_shield(id));
            assert_eq!(t.inventory.count(id), 0);
            assert_eq!(t.drain_shield(strength / 2), strength / 2);
            assert!(t.equipment.active_shield.as_ref().unwrap().flash > 0.0);
            assert!(!t.engage_shield(id)); // No spare: existing shield is retained.
            assert_eq!(t.drain_shield(1000), strength - strength / 2);
            assert!(t.equipment.active_shield.is_none());
        }
        let mut t = protected(ItemId::HeavyShield);
        t.inventory.add(ItemId::MagDeflector, 1);
        t.engage_shield(ItemId::MagDeflector);
        assert_eq!(t.equipment.active_shield.as_ref().unwrap().strength, 55);
        assert_eq!(t.inventory.count(ItemId::HeavyShield), 1); // Used Heavy Shield never returned.
    }

    #[test]
    fn force_reflection_is_a_seventy_percent_mirror_and_plain_shields_stop_the_shot() {
        let tanks = vec![tank(0), protected(ItemId::ForceShield)];
        let center = tanks[1].center();
        let incoming = vec2(100.0, 200.0);
        let Some(Contact::Hit {
            victim,
            cost,
            reflection: Some((pos, velocity)),
        }) = contact(
            center - vec2(0.0, 15.0),
            incoming,
            0,
            ItemId::Missile,
            &tanks,
        )
        else {
            panic!("not reflected")
        };
        assert_eq!(victim, 1);
        assert_eq!(cost, 2);
        assert_eq!(velocity, vec2(70.0, -140.0));
        assert!(pos.distance(center) > 15.0);
        assert!(contact(pos, velocity, 0, ItemId::Missile, &tanks).is_none());
        assert!(contact(center, incoming, 1, ItemId::Missile, &tanks).is_none());
        let tanks = vec![tank(0), protected(ItemId::Shield)];
        assert!(matches!(
            contact(
                center - vec2(0.0, 15.0),
                incoming,
                0,
                ItemId::Missile,
                &tanks
            ),
            Some(Contact::Hit {
                cost: 10,
                reflection: None,
                ..
            })
        ));
    }

    #[test]
    fn magnetic_push_obeys_band_ownership_and_has_no_artificial_speed_cutoff() {
        for id in [ItemId::MagDeflector, ItemId::SuperMag] {
            let tanks = vec![tank(0), protected(id)];
            for downward in [50.0, 1000.0] {
                let mut velocity = vec2(10.0, downward);
                assert!(magnetic_push(
                    vec2(300.0, 250.0),
                    &mut velocity,
                    0,
                    SIM_DT,
                    &tanks
                ));
                assert!(velocity.y < downward);
                assert_eq!(velocity.x, 10.0);
            }
            for (pos, owner, mut velocity) in [
                (vec2(316.0, 250.0), 0, vec2(1.0, 20.0)),
                (vec2(300.0, 170.0), 0, vec2(1.0, 20.0)),
                (vec2(300.0, 301.0), 0, vec2(1.0, 20.0)),
                (vec2(300.0, 250.0), 1, vec2(1.0, 20.0)),
                (vec2(300.0, 250.0), 0, vec2(0.0, 20.0)),
            ] {
                let before = velocity;
                assert!(!magnetic_push(pos, &mut velocity, owner, SIM_DT, &tanks));
                assert_eq!(velocity, before);
            }
        }
        assert!(ActiveShield::new(ItemId::SuperMag).unwrap().laser_immune());
        assert!(!ActiveShield::new(ItemId::HeavyShield)
            .unwrap()
            .laser_immune());
    }

    #[test]
    fn flicker_then_fade_changes_rendered_circle_without_changing_state() {
        let mut t = protected(ItemId::Shield);
        let mut fb = Fb::new();
        draw(&mut fb, &t, 0.0);
        let offset = (((t.y - 4) * crate::gfx::W + t.x + 15) * 4) as usize;
        assert_eq!(&fb.img.bytes[offset..offset + 3], &[255, 255, 255]);
        t.drain_shield(50);
        let mut fb = Fb::new();
        draw(&mut fb, &t, 0.0);
        assert_eq!(&fb.img.bytes[offset..offset + 3], &[0, 0, 0]);
        t.equipment.active_shield.as_mut().unwrap().flash = 0.0;
        draw(&mut fb, &t, 1.0);
        assert_eq!(&fb.img.bytes[offset..offset + 3], &[125, 125, 125]);
        assert_eq!(t.shield_percent(), 50);
    }
}
