//! Original item catalogue, in binary weapon-table order (MECHANICS.md §7–8).

use crate::economy::{DamageEvent, DamageTarget};
use crate::gfx::{Rgb, H, W};
use crate::physics::{Physics, WallContact, MAX_FLIGHT_TIME};
use crate::settings::WallType;
use crate::tank::Tank;
use crate::terrain::{Terrain, TerrainAccess};
use macroquad::prelude::{vec2, Vec2};
use macroquad::rand::gen_range;

impl ItemId {
    pub fn item(self) -> &'static Item {
        &ITEMS[self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Weapon,
    Miscellaneous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WKind {
    Blast,
    Mirv,
    Leap,
    Funky,
    Roller,
    Dirt,
    Tracer,
    Riot,
    Wedge,
    Tunnel,
    DirtCharge,
    Disruptor,
    Energy,
    Liquid,
    /// Catalogued for future batches, without a firing implementation yet.
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Weapon(WKind),
    Accessory,
}

// Base catalogue values; the shop supplies current prices and runtime filters.
#[allow(dead_code)]
pub struct Item {
    pub id: ItemId,
    pub name: &'static str,
    pub kind: ItemKind,
    /// Base price per bundle; Auto Defense uses a remaining-round annuity.
    pub cost: u32,
    pub bundle_size: u8,
    /// Initial blast radius (0 for non-blast items); variable effects come later.
    pub radius: f32,
    pub arms_level: u8,
    pub category: Category,
    /// Temporary starter stock until the economy/shop batches.
    pub starting_ammo: u8,
}

impl Item {
    pub fn is_usable_weapon(&self) -> bool {
        matches!(self.kind, ItemKind::Weapon(kind) if kind != WKind::Pending)
    }

    pub fn can_tunnel(&self) -> bool {
        self.category == Category::Weapon
            && self.kind != ItemKind::Weapon(WKind::Tunnel)
            && self.kind != ItemKind::Weapon(WKind::Energy)
            && !matches!(
                self.id,
                ItemId::BabyRoller
                    | ItemId::Roller
                    | ItemId::HeavyRoller
                    | ItemId::Tracer
                    | ItemId::SmokeTracer
                    | ItemId::DirtCharge
            )
    }

    pub fn damage(&self) -> Damage {
        Damage::Weapon(self.id as usize)
    }
}

#[derive(Clone, Copy)]
pub enum Damage {
    Weapon(usize),
    /// Preserve prototype death blasts until batch 21.
    Death,
    Environment,
    /// Underground charge: rounded radial damage that bypasses shields.
    Sandhog(usize),
}

// Generate IDs and storage size from the same rows, so adding an item needs no
// parallel enum/count/inventory edits. Keep the original 33 weapons in order.
macro_rules! catalogue {
    ($( $id:ident, $name:literal, $kind:expr, $cost:literal / $bundle:literal, $radius:literal, $arms:literal, $stock:literal; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        #[repr(usize)]
        pub enum ItemId { $( $id, )* }
        pub const ITEM_COUNT: usize = [$(stringify!($id)),*].len();
        pub const ITEMS: [Item; ITEM_COUNT] = [$(
            Item {
                id: ItemId::$id, name: $name, kind: $kind, cost: $cost,
                bundle_size: $bundle, radius: $radius, arms_level: $arms,
                starting_ammo: $stock,
                category: match $kind {
                    ItemKind::Weapon(_) => Category::Weapon,
                    ItemKind::Accessory => Category::Miscellaneous,
                },
            },
        )*];
    };
}

use ItemKind::{Accessory, Weapon};
use WKind::{
    Blast, Dirt, DirtCharge, Disruptor, Funky, Leap, Mirv, Riot, Roller, Tracer, Tunnel, Wedge,
};

// ID, display name, behavior, cost / bundle, initial radius, arms, starter stock.
// Funky Bomb uses its radius-20 opening burst, not the ±80 scatter range.
catalogue! {
    BabyMissile, "Baby Missile", Weapon(Blast), 400 / 10, 10.0, 0, 99;
    Missile, "Missile", Weapon(Blast), 1875 / 5, 20.0, 0, 10;
    BabyNuke, "Baby Nuke", Weapon(Blast), 10000 / 3, 40.0, 0, 3;
    Nuke, "Nuke", Weapon(Blast), 12000 / 1, 75.0, 1, 1;
    LeapFrog, "Leap Frog", Weapon(Leap), 10000 / 2, 30.0, 3, 0;
    FunkyBomb, "Funky Bomb", Weapon(Funky), 7000 / 2, 20.0, 4, 2;
    Mirv, "MIRV", Weapon(Mirv), 10000 / 3, 20.0, 2, 2;
    DeathsHead, "Death's Head", Weapon(Mirv), 20000 / 1, 35.0, 4, 0;
    Napalm, "Napalm", Weapon(WKind::Liquid), 10000 / 10, 0.0, 2, 0;
    HotNapalm, "Hot Napalm", Weapon(WKind::Liquid), 20000 / 2, 0.0, 4, 0;
    Tracer, "Tracer", Weapon(Tracer), 10 / 20, 0.0, 0, 20;
    SmokeTracer, "Smoke Tracer", Weapon(Tracer), 500 / 10, 0.0, 1, 0;
    BabyRoller, "Baby Roller", Weapon(Roller), 5000 / 10, 10.0, 2, 0;
    Roller, "Roller", Weapon(Roller), 6000 / 5, 20.0, 2, 4;
    HeavyRoller, "Heavy Roller", Weapon(Roller), 6750 / 2, 45.0, 3, 0;
    RiotCharge, "Riot Charge", Weapon(Wedge), 2000 / 10, 36.0, 2, 0;
    RiotBlast, "Riot Blast", Weapon(Wedge), 5000 / 5, 60.0, 3, 0;
    RiotBomb, "Riot Bomb", Weapon(Riot), 5000 / 5, 30.0, 3, 0;
    HeavyRiotBomb, "Heavy Riot Bomb", Weapon(Riot), 4750 / 2, 45.0, 3, 0;
    BabyDigger, "Baby Digger", Weapon(Tunnel), 3000 / 10, 0.0, 0, 0;
    Digger, "Digger", Weapon(Tunnel), 2500 / 5, 0.0, 0, 0;
    HeavyDigger, "Heavy Digger", Weapon(Tunnel), 6750 / 2, 0.0, 1, 0;
    BabySandhog, "Baby Sandhog", Weapon(Tunnel), 10000 / 10, 0.0, 0, 0;
    Sandhog, "Sandhog", Weapon(Tunnel), 16750 / 5, 0.0, 0, 0;
    HeavySandhog, "Heavy Sandhog", Weapon(Tunnel), 25000 / 2, 0.0, 1, 0;
    DirtClod, "Dirt Clod", Weapon(Dirt), 5000 / 10, 20.0, 0, 0;
    DirtBall, "Dirt Ball", Weapon(Dirt), 5000 / 5, 35.0, 0, 4;
    TonOfDirt, "Ton of Dirt", Weapon(Dirt), 6750 / 2, 70.0, 1, 0;
    LiquidDirt, "Liquid Dirt", Weapon(WKind::Liquid), 5000 / 10, 0.0, 2, 0;
    DirtCharge, "Dirt Charge", Weapon(DirtCharge), 5000 / 5, 0.0, 1, 0;
    EarthDisrupter, "Earth Disrupter", Weapon(Disruptor), 5000 / 10, 0.0, 0, 0;
    PlasmaBlast, "Plasma Blast", Weapon(WKind::Energy), 9000 / 5, 20.0, 3, 0;
    Laser, "Laser", Weapon(WKind::Energy), 5000 / 5, 0.0, 2, 0;
    HeatGuidance, "Heat Guidance", Accessory, 10000 / 6, 0.0, 2, 0;
    BallisticGuidance, "Ballistic Guidance", Accessory, 10000 / 2, 0.0, 2, 0;
    HorizontalGuidance, "Horz Guidance", Accessory, 15000 / 5, 0.0, 1, 0;
    VerticalGuidance, "Vert Guidance", Accessory, 20000 / 5, 0.0, 1, 0;
    LazyBoy, "Lazy Boy", Accessory, 20000 / 2, 0.0, 3, 0;
    Parachute, "Parachute", Accessory, 10000 / 8, 0.0, 2, 0;
    Battery, "Battery", Accessory, 5000 / 10, 0.0, 2, 0;
    MagDeflector, "Mag Deflector", Accessory, 10000 / 2, 0.0, 2, 0;
    Shield, "Shield", Accessory, 20000 / 3, 0.0, 3, 0;
    ForceShield, "Force Shield", Accessory, 25000 / 3, 0.0, 3, 0;
    HeavyShield, "Heavy Shield", Accessory, 30000 / 2, 0.0, 4, 0;
    SuperMag, "Super Mag", Accessory, 40000 / 2, 0.0, 4, 0;
    AutoDefense, "Auto Defense", Accessory, 1500 / 1, 0.0, 3, 0;
    FuelTank, "Fuel Tank", Accessory, 10000 / 10, 0.0, 3, 0;
    ContactTrigger, "Contact Trigger", Accessory, 1000 / 25, 0.0, 3, 0;
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum PKind {
    Shell,
    Carrier, // MIRV before split
    Bomblet,
    Leap {
        remaining: u8,
    },
    FunkyTrail {
        start: Vec2,
        target: Vec2,
        height: f32,
        duration: f32,
    },
    FunkyFinal,
    ShieldRoll {
        victim: usize,
        angle: f32,
        direction: f32,
        timer: f32,
    },
    Rolling {
        dir: i32,
        timer: f32,
        speed: f32,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct Proj {
    pub(crate) pos: Vec2,
    pub(crate) vel: Vec2,
    pub(crate) owner: usize,
    pub(crate) weapon: ItemId,
    pub(crate) kind: PKind,
    pub(crate) radius: f32,
    pub(crate) scale: f32,
    pub(crate) damage: Damage,
    pub(crate) age: f32,
    /// Snapshotted at launch after guidance/triggers; copied to child warheads.
    pub(crate) tunneling: bool,
    /// Stay underground until leaving the last erased pixel, even at subpixel speeds.
    pub(crate) in_dirt: bool,
    pub(crate) guidance: Option<crate::guidance::Guidance>,
}

pub(crate) enum Step {
    Alive,
    Gone,
    Split,
    Hit(Vec2, Option<usize>),
    Wall(Vec2),
    Shield {
        victim: usize,
        cost: i32,
        continues: bool,
    },
}

pub(crate) struct Explosion {
    pub(crate) pos: Vec2,
    pub(crate) radius: f32,
    pub(crate) damage: Damage,
    pub(crate) owner: usize,
    pub(crate) dirt: bool,
    pub(crate) t: f32,
    pub(crate) dur: f32,
    pub(crate) follow_up: Option<Proj>,
    pub(crate) death: Option<crate::presentation::Death>,
}

impl Explosion {
    pub(crate) fn new(pos: Vec2, radius: f32, damage: Damage, owner: usize, dirt: bool) -> Self {
        Explosion {
            pos,
            radius,
            damage,
            owner,
            dirt,
            t: 0.0,
            dur: if matches!(damage, Damage::Weapon(2 | 3)) && !dirt {
                3.0
            } else {
                0.4 + radius / 100.0
            },
            follow_up: None,
            death: None,
        }
    }
}

pub(crate) fn explosion_damage(distance: f32, radius: f32, damage: Damage) -> i32 {
    match damage {
        Damage::Weapon(index) | Damage::Sandhog(index) => {
            if radius <= 0.0 || distance >= radius {
                return 0;
            }
            let radial = (radius - distance) * 100.0 / radius;
            let damage = (if matches!(damage, Damage::Sandhog(_)) {
                radial.round()
            } else {
                radial.trunc()
            } as i32)
                * (index as i32 + 1);
            if damage > 100 {
                110
            } else {
                damage
            }
        }
        Damage::Death | Damage::Environment => {
            let reach = radius + 6.0;
            if distance < reach {
                (40.0 * (1.0 - distance / reach)).ceil() as i32
            } else {
                0
            }
        }
    }
}

fn tank_at(tanks: &[Tank], x: i32, y: i32, owner: usize, age: f32) -> Option<usize> {
    tanks
        .iter()
        .enumerate()
        .find(|(i, t)| t.alive && t.hit(x, y) && !(*i == owner && age < 0.25))
        .map(|(i, _)| i)
}

fn trail_color(p: &Proj, tanks: &[Tank]) -> Rgb {
    if matches!(p.kind, PKind::FunkyTrail { .. }) {
        let colors = [
            Rgb(255, 70, 60),
            Rgb(255, 230, 60),
            Rgb(60, 255, 100),
            Rgb(60, 180, 255),
            Rgb(230, 80, 255),
        ];
        colors[((p.age * 25.0) as usize + p.owner) % colors.len()]
    } else {
        tanks[p.owner].color
    }
}

fn shield_contact(p: &mut Proj, contact: crate::shields::Contact) -> Step {
    match contact {
        crate::shields::Contact::Roll {
            victim,
            angle,
            direction,
        } => {
            p.guidance = None;
            p.kind = PKind::ShieldRoll {
                victim,
                angle,
                direction,
                timer: 0.0,
            };
            Step::Alive
        }
        crate::shields::Contact::Hit {
            victim,
            cost,
            reflection,
        } => {
            if let Some((pos, velocity)) = reflection {
                p.pos = pos;
                p.vel = velocity;
                p.in_dirt = false;
                if matches!(p.kind, PKind::FunkyTrail { .. }) {
                    p.kind = PKind::Bomblet;
                }
            }
            Step::Shield {
                victim,
                cost,
                continues: reflection.is_some(),
            }
        }
    }
}

fn roll_off_shield(
    p: &mut Proj,
    dt: f32,
    tanks: &[Tank],
    trail: &mut Vec<(i32, i32, Rgb)>,
) -> Step {
    let PKind::ShieldRoll {
        victim,
        mut angle,
        direction,
        mut timer,
    } = p.kind
    else {
        unreachable!()
    };
    p.age += dt;
    let active = tanks[victim]
        .equipment
        .active_shield
        .as_ref()
        .filter(|shield| shield.strength > 0 && tanks[victim].alive && tanks[victim].health > 0);
    let Some(shield) = active else {
        p.kind = PKind::Shell;
        p.vel = vec2(direction * 150.0, 80.0);
        return Step::Alive;
    };
    let radius = crate::shields::spec(shield.item).unwrap().radius as f32 + 1.0;
    timer += dt * 150.0;
    while timer >= 1.0 {
        timer -= 1.0;
        angle = (angle + direction / radius).clamp(-std::f32::consts::PI, 0.0);
        p.pos = tanks[victim].center() + vec2(angle.cos(), angle.sin()) * radius;
        trail.push((p.pos.x as i32, p.pos.y as i32, tanks[p.owner].color));
        if angle == 0.0 || angle == -std::f32::consts::PI {
            p.kind = PKind::Shell;
            p.vel = vec2(direction * 150.0, 80.0);
            p.in_dirt = false;
            return Step::Alive;
        }
    }
    p.kind = PKind::ShieldRoll {
        victim,
        angle,
        direction,
        timer,
    };
    Step::Alive
}

pub(crate) fn step_proj<T: TerrainAccess>(
    p: &mut Proj,
    dt: f32,
    terrain: &mut T,
    tanks: &[Tank],
    physics: &Physics,
    trail: &mut Vec<(i32, i32, Rgb)>,
) -> Step {
    if p.guidance.is_some_and(|g| g.item == ItemId::LazyBoy) {
        return crate::guidance::step_lazy(p, dt, terrain, tanks, trail);
    }
    let flat = crate::guidance::steer(p, dt, tanks);
    let guided_physics = Physics {
        gravity: if flat { 0.0 } else { physics.gravity },
        wind: if flat { 0.0 } else { physics.wind },
        air_viscosity: if flat { 0 } else { physics.air_viscosity },
        ..*physics
    };
    let physics = &guided_physics;
    if matches!(p.kind, PKind::ShieldRoll { .. }) {
        return roll_off_shield(p, dt, tanks, trail);
    }
    if let PKind::FunkyTrail {
        start,
        target,
        height,
        duration,
    } = p.kind
    {
        let t = ((p.age + dt) / duration).min(1.0);
        let desired = start.lerp(target, t) - vec2(0.0, 4.0 * height * t * (1.0 - t));
        p.vel = (desired - p.pos) / dt.max(f32::EPSILON);
        let curve_physics = Physics {
            gravity: 0.0,
            wind: 0.0,
            air_viscosity: 0,
            borders_extend: 0,
            walls: physics.walls,
            ceiling: physics.ceiling,
        };
        let result = step_motion(p, dt, terrain, tanks, &curve_physics, trail);
        return match result {
            Step::Alive if t >= 1.0 && matches!(p.kind, PKind::FunkyTrail { .. }) => {
                Step::Hit(p.pos, None)
            }
            _ => result,
        };
    }
    step_motion(p, dt, terrain, tanks, physics, trail)
}

fn step_motion<T: TerrainAccess>(
    p: &mut Proj,
    dt: f32,
    terrain: &mut T,
    tanks: &[Tank],
    physics: &Physics,
    trail: &mut Vec<(i32, i32, Rgb)>,
) -> Step {
    p.age += dt;
    let color = trail_color(p, tanks);
    if let PKind::Rolling { dir, timer, speed } = &mut p.kind {
        *timer += dt;
        while *timer >= 1.0 / *speed {
            *timer -= 1.0 / *speed;
            let (x, y) = (p.pos.x as i32, p.pos.y as i32);
            // Descend one pixel per rolling tick rather than teleporting into valleys.
            let (mut nx, ny) = if !terrain.solid(x, y + 1) {
                (x, y + 1)
            } else {
                (x + *dir, y)
            };
            if !(0..W).contains(&nx) {
                match physics.walls {
                    WallType::WrapAround => nx = nx.rem_euclid(W),
                    WallType::Concrete => return Step::Wall(p.pos),
                    wall if wall.bounce_factor().is_some() => {
                        *dir = -*dir;
                        *speed = (*speed * wall.bounce_factor().unwrap()).min(1000.0);
                        nx = x + *dir;
                        p.vel.x = *dir as f32 * *speed;
                    }
                    _ => return Step::Gone,
                }
            }
            if ny >= H {
                return Step::Hit(vec2(nx as f32, (H - 1) as f32), None);
            }
            if let Some(contact) = crate::shields::contact(
                vec2(nx as f32, ny as f32),
                vec2(*dir as f32 * *speed, 0.0),
                p.owner,
                p.weapon,
                tanks,
            ) {
                p.pos = vec2(nx as f32, ny as f32);
                return shield_contact(p, contact);
            }
            if let Some(victim) = tank_at(tanks, nx, ny, p.owner, 1.0) {
                return Step::Hit(vec2(nx as f32, ny as f32), Some(victim));
            }
            if terrain.solid(nx, ny) {
                return Step::Hit(p.pos, None);
            }
            p.pos = vec2(nx as f32, ny as f32);
            trail.push((nx, ny, color));
        }
        if p.age > 15.0 {
            return Step::Hit(p.pos, None);
        }
        return Step::Alive;
    }

    if p.age > MAX_FLIGHT_TIME {
        return Step::Gone;
    }
    if !p.in_dirt
        && crate::shields::magnetic_push(p.pos, &mut p.vel, p.owner, dt, tanks)
        && matches!(p.kind, PKind::FunkyTrail { .. })
    {
        p.kind = PKind::Bomblet;
    }
    let can_tunnel = p.tunneling && p.kind != PKind::Carrier && p.weapon.item().can_tunnel();
    let prev_vy = p.vel.y;
    Physics::cap_speed(&mut p.vel);
    let mut remaining = dt;
    let mut air_time = 0.0;
    let mut previous_cell = if p.in_dirt {
        Some((p.pos.x.floor() as i32, p.pos.y.floor() as i32))
    } else {
        None
    };

    // Walk grid boundaries in time order. A fast shot cannot skip a one-pixel
    // layer, and slower movement after each dirt pixel uses the remaining time.
    loop {
        if physics.at_wall(p.pos) {
            physics.air_forces(&mut p.vel, air_time, dt);
            air_time = 0.0;
            match physics.wall_contact(&mut p.pos, &mut p.vel) {
                WallContact::Blocked => return Step::Wall(p.pos),
                WallContact::Bounced => {
                    if matches!(p.kind, PKind::FunkyTrail { .. }) {
                        p.kind = PKind::Bomblet;
                    }
                }
                _ => {}
            }
            p.in_dirt = false;
            previous_cell = None;
        }
        let (x, y) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32);
        if matches!(
            physics.walls,
            WallType::None | WallType::Random | WallType::Erratic
        ) && physics.beyond_border(p.pos.x)
        {
            return Step::Gone;
        }
        if y >= H {
            return if (0..W).contains(&x) {
                Step::Hit(vec2(p.pos.x, (H - 1) as f32), None)
            } else {
                Step::Gone
            };
        }
        let visible = y >= 0 && (0..W).contains(&x);
        if visible {
            if let Some(target) = crate::guidance::arrival(p) {
                return Step::Hit(target, None);
            }
            if crate::shields::contact(p.pos, p.vel, p.owner, p.weapon, tanks).is_some() {
                physics.air_forces(&mut p.vel, air_time, dt);
                air_time = 0.0;
                if let Some(contact) =
                    crate::shields::contact(p.pos, p.vel, p.owner, p.weapon, tanks)
                {
                    return shield_contact(p, contact);
                }
            }
            if let Some(victim) = tank_at(tanks, x, y, p.owner, p.age) {
                return Step::Hit(p.pos, Some(victim));
            }
            if terrain.solid(x, y) {
                // Finish the air portion before entering soil: its forces are
                // then slowed along with the incoming velocity, never added underground.
                physics.air_forces(&mut p.vel, air_time, dt);
                air_time = 0.0;
                if !can_tunnel {
                    return Step::Hit(p.pos, None);
                }
                terrain.erase(x, y);
                p.vel *= 0.75;
                p.in_dirt = true;
                if p.vel.length_squared() < 2000.0 {
                    return Step::Hit(p.pos, None);
                }
            } else if previous_cell != Some((x, y)) {
                p.in_dirt = false;
            }
            let c = color;
            if trail.last().map_or(true, |&(lx, ly, _)| lx != x || ly != y) {
                trail.push((x, y, c));
            }
        } else {
            p.in_dirt = false;
        }
        previous_cell = Some((x, y));
        if remaining <= 0.0 {
            break;
        }

        let bx = if p.vel.x > 0.0 {
            (x + 1) as f32
        } else {
            x as f32
        };
        let by = if p.vel.y > 0.0 {
            (y + 1) as f32
        } else {
            y as f32
        };
        let tx = if p.vel.x == 0.0 {
            f32::INFINITY
        } else {
            (bx - p.pos.x) / p.vel.x
        };
        let ty = if p.vel.y == 0.0 {
            f32::INFINITY
        } else {
            (by - p.pos.y) / p.vel.y
        };
        let elapsed = remaining.min(tx.min(ty)).max(0.0);
        if !p.in_dirt {
            air_time += elapsed;
        }
        p.pos += p.vel * elapsed;
        remaining = (remaining - elapsed).max(0.0);
        // Move to the next representable coordinate, including negative travel,
        // so an exact grid-boundary hit always progresses to the next cell.
        if tx <= elapsed {
            p.pos.x = if p.vel.x > 0.0 {
                bx.next_up()
            } else {
                bx.next_down()
            };
        }
        if ty <= elapsed {
            p.pos.y = if p.vel.y > 0.0 {
                by.next_up()
            } else {
                by.next_down()
            };
        }
        if tx > elapsed && ty > elapsed {
            break;
        }
    }
    physics.air_forces(&mut p.vel, air_time, dt);
    if p.kind == PKind::Carrier && prev_vy <= 0.0 && p.vel.y > 0.0 {
        Step::Split
    } else {
        Step::Alive
    }
}

pub(crate) fn impact(
    p: Proj,
    pos: Vec2,
    on_tank: Option<usize>,
    terrain: &Terrain,
    tanks: &mut [Tank],
    spawn: &mut Vec<Proj>,
    expls: &mut Vec<Explosion>,
) -> Option<DamageEvent> {
    if !p.weapon.item().is_usable_weapon() {
        return None;
    }
    let ItemKind::Weapon(kind) = p.weapon.item().kind else {
        return None;
    };
    let mut direct_hit = None;
    // Non-damaging weapons and pre-apogee MIRVs must not kill on contact.
    if p.kind != PKind::Carrier
        && !matches!(
            kind,
            WKind::Tracer
                | WKind::Dirt
                | WKind::Riot
                | WKind::Wedge
                | WKind::Tunnel
                | WKind::DirtCharge
                | WKind::Disruptor
                | WKind::Energy
                | WKind::Liquid
        )
    {
        if let Some(victim) = on_tank {
            let t = &mut tanks[victim];
            if t.health > 0 {
                direct_hit = Some(DamageEvent {
                    owner: p.owner,
                    victim,
                    amount: t.health,
                    target: DamageTarget::Tank,
                });
                t.health = 0;
                t.power = 0;
                t.last_hit = Some(p.owner);
                t.environmental_hit = false;
            }
        }
    }
    let mut boom = |r: f32, d: Damage, dirt: bool| {
        expls.push(Explosion::new(pos, r, d, p.owner, dirt));
    };
    match (p.kind, kind) {
        (PKind::Carrier, _) => {} // Before apogee, any impact is a dud.
        (_, WKind::Tracer) => {}
        (PKind::Shell, WKind::Dirt) => boom(p.radius, p.damage, true),
        (PKind::Shell, WKind::Funky) => {
            let mut explosion = Explosion::new(pos, 20.0, p.damage, p.owner, false);
            explosion.follow_up = Some(Proj { pos, ..p });
            expls.push(explosion);
        }
        (_, WKind::Leap) => {
            let remaining = if let PKind::Leap { remaining } = p.kind {
                remaining
            } else {
                3
            };
            let mut explosion = Explosion::new(pos, p.radius, p.damage, p.owner, false);
            if remaining > 1 {
                explosion.follow_up = Some(Proj {
                    pos: pos - vec2(0.0, 1.0),
                    vel: vec2(p.vel.x, -p.vel.y.abs()) / 1.5,
                    kind: PKind::Leap {
                        remaining: remaining - 1,
                    },
                    radius: p.radius - 5.0 * p.scale,
                    age: 0.0,
                    in_dirt: false,
                    guidance: None,
                    ..p
                });
            }
            expls.push(explosion);
        }
        (PKind::Shell, WKind::Roller) if on_tank.is_none() => {
            let (x, mut y) = (pos.x as i32, pos.y as i32);
            while terrain.solid(x, y) && y > 0 {
                y -= 1;
            }
            let (l, r) = (terrain.surface(x - 4), terrain.surface(x + 4));
            let dir = if l != r {
                if l > r {
                    -1
                } else {
                    1
                }
            } else if p.vel.x < 0.0 {
                -1
            } else {
                1
            };
            spawn.push(Proj {
                pos: vec2(x as f32, y as f32),
                guidance: None,
                kind: PKind::Rolling {
                    dir,
                    timer: 0.0,
                    speed: 150.0,
                },
                age: 0.0,
                ..p
            });
        }
        (
            _,
            WKind::Pending
            | WKind::Riot
            | WKind::Wedge
            | WKind::Tunnel
            | WKind::DirtCharge
            | WKind::Disruptor
            | WKind::Energy
            | WKind::Liquid,
        ) => {}
        _ => boom(p.radius, p.damage, false),
    }
    direct_hit
}

/// Radius policy for the currently implemented families. Dirt and Funky's
/// opening/trail bursts retain their documented fixed sizes.
pub(crate) fn initial_radius(id: ItemId, scale: f32) -> f32 {
    let item = id.item();
    if matches!(
        item.kind,
        ItemKind::Weapon(WKind::Blast | WKind::Mirv | WKind::Leap | WKind::Roller)
    ) {
        item.radius * scale
    } else {
        item.radius
    }
}

pub(crate) fn follow_explosion(p: Proj, terrain: &Terrain, spawn: &mut Vec<Proj>) {
    if p.weapon == ItemId::FunkyBomb {
        for _ in 0..gen_range(5, 11) {
            let x = (p.pos.x as i32 + gen_range(-80, 80)).clamp(0, W - 1);
            let target = vec2(x as f32, (terrain.surface(x) - 1) as f32);
            spawn.push(Proj {
                kind: PKind::FunkyTrail {
                    start: p.pos,
                    target,
                    height: gen_range(20.0, 61.0) + (target.y - p.pos.y).max(0.0) / 4.0,
                    duration: 0.4 + target.distance(p.pos) / 160.0,
                },
                radius: gen_range(15, 25) as f32,
                age: 0.0,
                tunneling: false,
                in_dirt: false,
                guidance: None,
                ..p
            });
        }
        spawn.push(Proj {
            kind: PKind::FunkyFinal,
            ..p
        });
    } else {
        spawn.push(p);
    }
}

pub(crate) fn split_carrier(p: Proj, spawn: &mut Vec<Proj>) {
    let half = if p.weapon == ItemId::DeathsHead { 4 } else { 2 };
    for k in -half..=half {
        let (weapon, radius, damage) = if p.weapon == ItemId::DeathsHead {
            (p.weapon, p.radius, p.damage)
        } else {
            (
                ItemId::Missile,
                ItemId::Missile.item().radius * p.scale,
                ItemId::Missile.item().damage(),
            )
        };
        spawn.push(Proj {
            kind: PKind::Bomblet,
            weapon,
            radius,
            damage,
            vel: p.vel + vec2(k as f32 * 30.0, 0.0),
            ..p
        });
    }
}

#[cfg(test)]
mod catalogue_tests {
    use super::*;

    #[test]
    fn catalogue_matches_documented_prices_bundles_arms_and_original_weapon_indices() {
        let manual = include_str!("../MECHANICS.md");
        let weapons = manual
            .split("### 7.1 Weapon table")
            .nth(1)
            .unwrap()
            .split("### 7.2")
            .next()
            .unwrap();
        let accessories = manual
            .split("### 8.1 Accessory table")
            .nth(1)
            .unwrap()
            .split("### 8.2")
            .next()
            .unwrap();
        let mut checked = 0;
        for (section, category) in [
            (weapons, Category::Weapon),
            (accessories, Category::Miscellaneous),
        ] {
            for line in section
                .lines()
                .filter(|line| line.starts_with('|') && line.contains('$'))
            {
                let cells: Vec<_> = line.split('|').map(str::trim).collect();
                let (name, offer, arms) = match category {
                    Category::Weapon => (cells[2], cells[3], cells[5]),
                    Category::Miscellaneous => (cells[1], cells[2], cells[3]),
                };
                let item = ITEMS.iter().find(|item| item.name == name).expect(name);
                let (price, bundle) = offer.split_once('/').unwrap();
                let price: u32 = price
                    .chars()
                    .filter(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .unwrap();
                let bundle: u8 = bundle.split_whitespace().next().unwrap().parse().unwrap();
                assert_eq!(
                    (item.cost, item.bundle_size, item.arms_level, item.category),
                    (price, bundle, arms.parse::<u8>().unwrap(), category),
                    "{name}"
                );
                assert_eq!(ITEMS[item.id as usize].id, item.id);
                if category == Category::Weapon {
                    assert_eq!(
                        item.id as usize,
                        cells[1].parse::<usize>().unwrap(),
                        "{name}"
                    );
                }
                checked += 1;
            }
        }
        assert_eq!(checked, ITEMS.len());
    }
}

#[cfg(test)]
mod tunneling_tests {
    use super::*;
    use crate::{inventory::Inventory, physics::SIM_DT, settings::PhysicsSettings, tank::Ctrl};

    struct Scene {
        shot: Proj,
        terrain: Terrain,
        tanks: Vec<Tank>,
        physics: Physics,
        trail: Vec<(i32, i32, Rgb)>,
    }
    impl Scene {
        fn new() -> Self {
            Self {
                shot: Proj {
                    pos: vec2(100.25, 200.25),
                    vel: vec2(1000.0, 0.0),
                    owner: 0,
                    weapon: ItemId::BabyMissile,
                    kind: PKind::Shell,
                    radius: 10.0,
                    scale: 1.0,
                    damage: Damage::Weapon(0),
                    age: 0.0,
                    tunneling: true,
                    in_dirt: false,
                    guidance: None,
                },
                terrain: Terrain::flat(H),
                tanks: vec![Tank {
                    bank: None,
                    ai_state: crate::ai::State::default(),
                    shape: crate::tank::Shape::Treads,
                    name: "Test".into(),
                    color: Rgb::WHITE,
                    ctrl: Ctrl::Human,
                    account: crate::economy::Account::default(),
                    team: None,
                    x: 500,
                    y: 100,
                    angle: 0,
                    power: 1000,
                    health: 100,
                    alive: true,
                    inventory: Inventory::default(),
                    equipment: crate::tank::Equipment::default(),
                    weapon: ItemId::BabyMissile,
                    fall: 0,
                    last_hit: None,
                    environmental_hit: false,
                    kills: 0,
                    wins: 0,
                }],
                physics: Physics {
                    gravity: 0.0,
                    wind: 0.0,
                    air_viscosity: 0,
                    borders_extend: 75,
                    walls: WallType::None,
                    ceiling: crate::ui::HUD_H as f32,
                },
                trail: Vec::new(),
            }
        }
        fn dirt(&mut self, x: i32, y: i32) {
            self.terrain.add_dirt(x, y, 0, 0);
        }
        fn step(&mut self, dt: f32) -> Step {
            step_proj(
                &mut self.shot,
                dt,
                &mut self.terrain,
                &self.tanks,
                &self.physics,
                &mut self.trail,
            )
        }
    }

    #[test]
    fn fast_shot_crosses_a_thin_layer_at_reduced_speed_without_skipping_it() {
        let mut s = Scene::new();
        s.dirt(101, 200);
        assert!(matches!(s.step(0.01), Step::Alive));
        assert!(!s.terrain.solid(101, 200));
        assert!(s.shot.pos.x > 102.0);
        assert_eq!(s.shot.vel, vec2(750.0, 0.0));
        assert!(!s.shot.in_dirt);
        assert!((s.shot.pos.x - 107.9375).abs() < 0.001);
    }

    #[test]
    fn thick_dirt_stops_after_the_eleventh_pixel_and_marks_soil_for_settling() {
        let mut s = Scene::new();
        s.terrain = Terrain::flat(190); // Initially clean columns; drilling must mark them dirty.
        assert!(matches!(s.step(0.5), Step::Hit(_, None)));
        assert!((110.0..111.0).contains(&s.shot.pos.x));
        assert!((s.shot.vel.x - 1000.0 * 0.75f32.powi(11)).abs() < 0.001);
        for x in 100..=110 {
            assert!(!s.terrain.solid(x, 200));
        }
        assert!(s.terrain.solid(111, 200));
        assert!(s.terrain.settle_step());
    }

    #[test]
    fn forces_and_repeat_slowdown_are_suppressed_while_still_in_the_erased_pixel() {
        let mut s = Scene::new();
        s.physics = Physics::new(
            &PhysicsSettings {
                air_viscosity: 20,
                gravity: 10.0,
                ..Default::default()
            },
            500,
        );
        s.shot.vel = vec2(80.0, 0.0);
        s.dirt(100, 200);
        for _ in 0..3 {
            assert!(matches!(s.step(SIM_DT), Step::Alive));
            assert!(s.shot.in_dirt);
            assert_eq!(s.shot.vel, vec2(60.0, 0.0));
            assert_eq!(s.shot.pos.y, 200.25);
        }
        for _ in 0..20 {
            s.step(SIM_DT);
            if !s.shot.in_dirt {
                break;
            }
        }
        assert!(!s.shot.in_dirt);
        let before = s.shot.vel;
        s.step(SIM_DT);
        assert!(s.shot.vel.y > before.y);
        assert!(s.shot.vel.x > before.x);
    }

    #[test]
    fn entry_applies_only_the_air_portion_before_slowing_and_new_children_start_fresh() {
        let mut s = Scene::new();
        s.physics.gravity = 100.0;
        s.physics.wind = 100.0;
        s.shot.vel = vec2(300.0, 0.0);
        s.dirt(101, 200);
        assert!(matches!(s.step(0.003), Step::Alive));
        assert!(s.shot.in_dirt);
        assert!((s.shot.vel - vec2(225.1875, 0.1875)).length() < 0.001);
        s.shot.weapon = ItemId::FunkyBomb;
        let mut explosions = Vec::new();
        let mut children = Vec::new();
        let _ = impact(
            s.shot,
            s.shot.pos,
            None,
            &s.terrain,
            &mut s.tanks,
            &mut children,
            &mut explosions,
        );
        assert!(children.is_empty()); // The opening blast must carve dirt first.
        let follow = explosions[0].follow_up.take().unwrap();
        follow_explosion(follow, &s.terrain, &mut children);
        assert!((6..=11).contains(&children.len())); // 5–10 trails plus final-blast marker.
        assert!(children
            .iter()
            .filter(|p| p.kind != PKind::FunkyFinal)
            .all(|p| !p.tunneling && !p.in_dirt));
    }

    #[test]
    fn exactly_2000_speed_squared_continues_but_less_explodes() {
        let mut s = Scene::new();
        s.shot.vel = vec2(20.0 / 0.75, 40.0 / 0.75);
        s.dirt(100, 200);
        assert!(matches!(s.step(SIM_DT), Step::Alive));
        assert_eq!(s.shot.vel.length_squared(), 2000.0);
        let mut s = Scene::new();
        s.shot.vel = vec2(20.0 / 0.75, 39.0 / 0.75);
        s.dirt(100, 200);
        assert!(matches!(s.step(SIM_DT), Step::Hit(_, None)));
        assert!(!s.terrain.solid(100, 200));
    }

    #[test]
    fn disabling_tunneling_or_using_exempt_items_stops_at_the_first_contact() {
        for id in [
            ItemId::BabyMissile,
            ItemId::Roller,
            ItemId::Tracer,
            ItemId::DirtCharge,
        ] {
            let mut s = Scene::new();
            s.shot.weapon = id;
            s.shot.tunneling = id != ItemId::BabyMissile;
            s.dirt(101, 200);
            assert!(matches!(s.step(0.01), Step::Hit(_, None)));
            assert!(s.terrain.solid(101, 200));
            assert!((101.0..102.0).contains(&s.shot.pos.x));
            assert_eq!(s.shot.vel, vec2(1000.0, 0.0));
        }
        for id in [
            ItemId::BabyRoller,
            ItemId::Roller,
            ItemId::HeavyRoller,
            ItemId::Tracer,
            ItemId::SmokeTracer,
            ItemId::DirtCharge,
        ] {
            assert!(!id.item().can_tunnel());
        }
    }

    #[test]
    fn negative_and_diagonal_travel_visit_each_entered_pixel_once() {
        let mut s = Scene::new();
        s.shot.pos.x = 102.75;
        s.shot.vel.x = -1000.0;
        s.dirt(101, 200);
        assert!(matches!(s.step(0.01), Step::Alive));
        assert_eq!(s.shot.vel, vec2(-750.0, 0.0));
        assert!(!s.terrain.solid(101, 200));
        let mut s = Scene::new();
        s.shot.pos.y = 200.25;
        s.shot.vel = vec2(600.0, 600.0);
        s.dirt(101, 201);
        assert!(matches!(s.step(0.01), Step::Alive));
        assert_eq!(s.shot.vel, vec2(450.0, 450.0));
        assert!(!s.terrain.solid(101, 201));
    }

    #[test]
    fn pre_apex_mirvs_remain_duds_and_bedrock_cannot_be_dug_out() {
        let mut s = Scene::new();
        s.shot.weapon = ItemId::Mirv;
        s.shot.kind = PKind::Carrier;
        s.dirt(101, 200);
        assert!(matches!(s.step(0.01), Step::Hit(_, None)));
        assert!(s.terrain.solid(101, 200));
        let mut explosions = Vec::new();
        let mut children = Vec::new();
        let _ = impact(
            s.shot,
            s.shot.pos,
            None,
            &s.terrain,
            &mut s.tanks,
            &mut children,
            &mut explosions,
        );
        assert!(explosions.is_empty() && children.is_empty());
        let mut s = Scene::new();
        s.shot.pos = vec2(100.25, H as f32 - 0.5);
        s.shot.vel = vec2(0.0, 1000.0);
        assert!(matches!(s.step(SIM_DT), Step::Hit(_, None)));
        assert!(s.terrain.solid(100, H));
    }
}

#[cfg(test)]
mod batch_nine_tests {
    use super::*;
    use crate::physics::SIM_DT;
    use crate::settings::Settings;
    use crate::shop::tests::tank;

    fn shot(id: ItemId) -> Proj {
        Proj {
            pos: vec2(300.0, 199.0),
            vel: vec2(90.0, 120.0),
            owner: 0,
            weapon: id,
            kind: PKind::Shell,
            radius: id.item().radius,
            scale: 1.0,
            damage: id.item().damage(),
            age: 1.0,
            tunneling: true,
            in_dirt: false,
            guidance: None,
        }
    }

    #[test]
    fn leap_frog_has_three_ordered_shrinking_blasts_with_reduced_hop_speed() {
        let mut tanks = vec![tank(0)];
        let mut terrain = Terrain::flat(200);
        let mut p = shot(ItemId::LeapFrog);
        for (index, expected) in [30.0, 25.0, 20.0].into_iter().enumerate() {
            let old_speed = p.vel.length();
            let mut explosions = Vec::new();
            let mut children = Vec::new();
            impact(
                p,
                p.pos,
                None,
                &terrain,
                &mut tanks,
                &mut children,
                &mut explosions,
            );
            assert!(children.is_empty());
            assert_eq!(explosions.len(), 1);
            assert_eq!(explosions[0].radius, expected);
            terrain.carve(p.pos.x as i32, p.pos.y as i32, expected as i32);
            match explosions[0].follow_up.take() {
                Some(next) => {
                    assert!(index < 2);
                    assert!((next.vel.length() - old_speed / 1.5).abs() < 0.001);
                    assert!(next.vel.y < 0.0);
                    follow_explosion(next, &terrain, &mut children);
                    assert_eq!(children.len(), 1);
                    p = children[0];
                }
                None => assert_eq!(index, 2),
            }
        }
    }

    #[test]
    fn mirv_and_deaths_head_preserve_the_central_parent_and_original_damage_indices() {
        for (id, count, radius, damage_index) in
            [(ItemId::Mirv, 5, 40.0, 1), (ItemId::DeathsHead, 9, 70.0, 7)]
        {
            let mut p = shot(id);
            p.kind = PKind::Carrier;
            p.scale = 2.0;
            p.radius = id.item().radius * p.scale;
            p.vel = vec2(125.0, 0.0);
            let mut children = Vec::new();
            split_carrier(p, &mut children);
            assert_eq!(children.len(), count);
            let middle = &children[count / 2];
            assert_eq!(middle.vel, p.vel);
            assert_eq!(middle.pos, p.pos);
            assert_eq!(middle.age, p.age);
            for child in children {
                assert_eq!(child.radius, radius);
                assert!(matches!(child.damage, Damage::Weapon(i) if i == damage_index));
                assert_eq!(child.owner, p.owner);
                assert!(child.tunneling);
            }
        }
        let terrain = Terrain::flat(200);
        let mut tanks = vec![tank(0)];
        for id in [ItemId::Mirv, ItemId::DeathsHead] {
            let p = Proj {
                kind: PKind::Carrier,
                ..shot(id)
            };
            let mut children = Vec::new();
            let mut explosions = Vec::new();
            assert!(impact(
                p,
                p.pos,
                Some(0),
                &terrain,
                &mut tanks,
                &mut children,
                &mut explosions
            )
            .is_none());
            assert_eq!(tanks[0].health, 100);
            assert!(children.is_empty() && explosions.is_empty());
        }
    }

    #[test]
    fn funky_trails_are_bounded_non_tunneling_curves_with_documented_burst_sizes() {
        let terrain = Terrain::flat(300);
        let mut tanks = vec![tank(0)];
        let p = Proj {
            pos: vec2(300.0, 200.0),
            scale: 3.0,
            ..shot(ItemId::FunkyBomb)
        };
        let mut children = Vec::new();
        let mut explosions = Vec::new();
        impact(
            p,
            p.pos,
            None,
            &terrain,
            &mut tanks,
            &mut children,
            &mut explosions,
        );
        assert!(children.is_empty());
        assert_eq!(explosions[0].radius, 20.0);
        follow_explosion(
            explosions[0].follow_up.take().unwrap(),
            &terrain,
            &mut children,
        );
        assert!((6..=11).contains(&children.len()));
        assert_eq!(
            children
                .iter()
                .filter(|p| p.kind == PKind::FunkyFinal)
                .count(),
            1
        );
        for child in children.iter().filter(|p| p.kind != PKind::FunkyFinal) {
            let PKind::FunkyTrail { target, start, .. } = child.kind else {
                panic!("not a curved trail")
            };
            assert!((target.x - start.x).abs() <= 80.0);
            assert!((0.0..W as f32).contains(&target.x));
            assert!((15.0..=24.0).contains(&child.radius));
            assert!(!child.tunneling && !child.in_dirt);
        }
        let mut child = children[0];
        let mut terrain = terrain;
        let physics = Physics::new(&Settings::default().physics, 500);
        let mut trace = Vec::new();
        assert!(matches!(
            step_proj(&mut child, 0.05, &mut terrain, &tanks, &physics, &mut trace),
            Step::Alive
        ));
        assert!(child.pos.y < p.pos.y);
        assert!(!trace.is_empty());
        let mut hit = false;
        for _ in 0..2000 {
            if matches!(
                step_proj(
                    &mut child,
                    SIM_DT,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut trace
                ),
                Step::Hit(..)
            ) {
                hit = true;
                break;
            }
        }
        assert!(hit);
    }

    #[test]
    fn all_rollers_fall_one_pixel_per_tick_stop_at_obstructions_and_can_hit_the_owner() {
        let physics = Physics::new(&Settings::default().physics, 0);
        for (id, radius) in [
            (ItemId::BabyRoller, 10.0),
            (ItemId::Roller, 20.0),
            (ItemId::HeavyRoller, 45.0),
        ] {
            let mut terrain = Terrain::flat(210);
            let tanks = vec![tank(0)];
            let mut p = Proj {
                pos: vec2(100.0, 200.0),
                kind: PKind::Rolling {
                    dir: 1,
                    timer: 0.0,
                    speed: 150.0,
                },
                ..shot(id)
            };
            let mut trail = Vec::new();
            assert!(matches!(
                step_proj(
                    &mut p,
                    1.0 / 150.0,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut trail
                ),
                Step::Alive
            ));
            assert_eq!(p.pos, vec2(100.0, 201.0));
            p.pos = vec2(100.0, 209.0);
            assert!(matches!(
                step_proj(
                    &mut p,
                    1.0 / 150.0,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut trail
                ),
                Step::Alive
            ));
            assert_eq!(p.pos.x, 101.0);
            terrain.add_dirt(104, 209, 2, 0);
            assert!(matches!(
                step_proj(
                    &mut p,
                    1.0 / 150.0,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut trail
                ),
                Step::Hit(..)
            ));
            assert_eq!(p.radius, radius);
            let mut tanks = tanks;
            tanks[0].x = 100;
            tanks[0].y = 209;
            p.pos = vec2(92.0, 209.0);
            let result = step_proj(
                &mut p,
                1.0 / 150.0,
                &mut terrain,
                &tanks,
                &physics,
                &mut trail,
            );
            assert!(matches!(result, Step::Hit(_, Some(0))));
        }
    }

    #[test]
    fn scale_affects_explosive_families_but_not_dirt_or_tracers() {
        for scale in [1.0, 2.0, 3.0] {
            for id in [
                ItemId::BabyMissile,
                ItemId::Missile,
                ItemId::BabyNuke,
                ItemId::Nuke,
                ItemId::LeapFrog,
                ItemId::Mirv,
                ItemId::DeathsHead,
                ItemId::BabyRoller,
                ItemId::Roller,
                ItemId::HeavyRoller,
            ] {
                assert_eq!(initial_radius(id, scale), id.item().radius * scale);
            }
            for id in [
                ItemId::DirtBall,
                ItemId::FunkyBomb,
                ItemId::Tracer,
                ItemId::SmokeTracer,
            ] {
                assert_eq!(initial_radius(id, scale), id.item().radius);
            }
        }
    }
}

#[cfg(test)]
mod wall_tests {
    use super::*;
    use crate::{settings::Settings, shop::tests::tank};

    fn scene(wall: WallType) -> (Physics, Terrain, Vec<Tank>) {
        let mut settings = Settings::default();
        settings.physics.walls = wall;
        let mut physics = Physics::new(&settings.physics, 0);
        physics.gravity = 0.0;
        (physics, Terrain::flat(H), vec![tank(0)])
    }
    fn shot(id: ItemId, pos: Vec2, vel: Vec2) -> Proj {
        Proj {
            pos,
            vel,
            weapon: id,
            owner: 0,
            kind: PKind::Shell,
            radius: id.item().radius,
            scale: 1.0,
            damage: id.item().damage(),
            age: 1.0,
            tunneling: true,
            in_dirt: false,
            guidance: None,
        }
    }

    #[test]
    fn every_implemented_weapon_reflects_or_wraps_and_checks_opposite_edge_dirt() {
        for item in ITEMS.iter().filter(|item| item.is_usable_weapon()) {
            for wall in [WallType::Rubber, WallType::WrapAround] {
                let (physics, mut terrain, tanks) = scene(wall);
                let mut p = shot(item.id, vec2(W as f32 - 0.5, 150.5), vec2(1000.0, 0.0));
                let mut trace = Vec::new();
                assert!(matches!(
                    step_proj(&mut p, 0.002, &mut terrain, &tanks, &physics, &mut trace),
                    Step::Alive
                ));
                assert!((0.0..W as f32).contains(&p.pos.x));
                if wall == WallType::Rubber {
                    assert!(p.vel.x < 0.0);
                } else {
                    assert!(p.pos.x < 3.0 && p.vel.x > 0.0);
                }
                assert!(trace
                    .iter()
                    .all(|&(x, y, _)| (0..W).contains(&x) && (0..H).contains(&y)));
            }
        }
        let (physics, mut terrain, tanks) = scene(WallType::WrapAround);
        terrain.add_dirt(0, 150, 0, 0);
        let mut p = shot(
            ItemId::Missile,
            vec2(W as f32 - 0.5, 150.5),
            vec2(1000.0, 0.0),
        );
        p.tunneling = false;
        assert!(matches!(
            step_proj(
                &mut p,
                0.002,
                &mut terrain,
                &tanks,
                &physics,
                &mut Vec::new()
            ),
            Step::Hit(_, None)
        ));
    }

    #[test]
    fn long_steps_keep_remaining_travel_across_multiple_wraps_or_bounces() {
        for wall in [WallType::WrapAround, WallType::Rubber] {
            let (physics, mut terrain, tanks) = scene(wall);
            let mut p = shot(ItemId::BabyMissile, vec2(100.0, 150.0), vec2(1000.0, 0.0));
            assert!(matches!(
                step_proj(&mut p, 2.0, &mut terrain, &tanks, &physics, &mut Vec::new()),
                Step::Alive
            ));
            let expected = if wall == WallType::WrapAround {
                180.0
            } else {
                460.0
            };
            assert!(
                (p.pos.x - expected).abs() < 0.5,
                "{}: {}",
                wall.label(),
                p.pos.x
            );
            assert_eq!(p.vel.x.is_sign_negative(), wall == WallType::Rubber);
        }
    }

    #[test]
    fn top_walls_prevent_escape_and_concrete_cannot_be_tunneled_through() {
        for wall in [
            WallType::Padded,
            WallType::Rubber,
            WallType::Spring,
            WallType::Concrete,
        ] {
            let (physics, mut terrain, tanks) = scene(wall);
            let mut p = shot(
                ItemId::Missile,
                vec2(100.0, physics.ceiling + 0.5),
                vec2(0.0, -1000.0),
            );
            let result = step_proj(
                &mut p,
                0.002,
                &mut terrain,
                &tanks,
                &physics,
                &mut Vec::new(),
            );
            if wall == WallType::Concrete {
                assert!(matches!(result, Step::Wall(_)));
            } else {
                assert!(matches!(result, Step::Alive));
                assert!(p.vel.y > 0.0);
            }
            assert!(p.pos.y >= physics.ceiling);
        }
        let (physics, mut terrain, tanks) = scene(WallType::None);
        let mut p = shot(ItemId::Missile, vec2(100.0, 0.5), vec2(0.0, -1000.0));
        assert!(matches!(
            step_proj(
                &mut p,
                0.01,
                &mut terrain,
                &tanks,
                &physics,
                &mut Vec::new()
            ),
            Step::Alive
        ));
        assert!(p.pos.y < 0.0);
    }

    #[test]
    fn roller_edges_wrap_reflect_with_the_wall_factor_or_stop_at_concrete() {
        for wall in FIXED_WALL_TYPES {
            let (physics, _, tanks) = scene(wall);
            let mut terrain = Terrain::flat(210);
            for id in [ItemId::BabyRoller, ItemId::Roller, ItemId::HeavyRoller] {
                let mut p = shot(id, vec2((W - 1) as f32, 209.0), vec2(150.0, 0.0));
                p.kind = PKind::Rolling {
                    dir: 1,
                    timer: 0.0,
                    speed: 150.0,
                };
                let result = step_proj(
                    &mut p,
                    1.0 / 150.0,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut Vec::new(),
                );
                match wall {
                    WallType::None => assert!(matches!(result, Step::Gone)),
                    WallType::Concrete => assert!(matches!(result, Step::Wall(_))),
                    WallType::WrapAround => {
                        assert!(matches!(result, Step::Alive));
                        assert_eq!(p.pos.x, 0.0);
                    }
                    _ => {
                        assert!(matches!(result, Step::Alive));
                        let PKind::Rolling { dir, speed, .. } = p.kind else {
                            panic!("roller state lost")
                        };
                        assert_eq!(dir, -1);
                        assert_eq!(speed, 150.0 * wall.bounce_factor().unwrap());
                        assert_eq!(p.pos.x, (W - 2) as f32);
                    }
                }
            }
        }
    }
    const FIXED_WALL_TYPES: [WallType; 6] = crate::physics::FIXED_WALLS;

    #[test]
    fn repeated_spring_roller_bounces_remain_bounded_and_expire() {
        let (physics, _, tanks) = scene(WallType::Spring);
        let mut terrain = Terrain::flat(210);
        let mut p = shot(
            ItemId::Roller,
            vec2((W - 1) as f32, 209.0),
            vec2(150.0, 0.0),
        );
        p.kind = PKind::Rolling {
            dir: 1,
            timer: 0.0,
            speed: 150.0,
        };
        let mut stopped = false;
        for _ in 0..160 {
            let result = step_proj(&mut p, 0.1, &mut terrain, &tanks, &physics, &mut Vec::new());
            if let PKind::Rolling { speed, .. } = p.kind {
                assert!(speed <= 1000.0);
            }
            if matches!(result, Step::Hit(..)) {
                stopped = true;
                break;
            }
        }
        assert!(stopped);
    }

    #[test]
    fn split_warheads_and_funky_curves_respect_the_same_active_wall() {
        for id in [ItemId::Mirv, ItemId::DeathsHead] {
            let (physics, mut terrain, tanks) = scene(WallType::Spring);
            let mut parent = shot(id, vec2(W as f32 - 0.5, 100.0), vec2(100.0, 0.0));
            parent.kind = PKind::Carrier;
            let mut children = Vec::new();
            split_carrier(parent, &mut children);
            for mut child in children {
                child.vel = vec2(1000.0, 0.0);
                assert!(matches!(
                    step_proj(
                        &mut child,
                        0.002,
                        &mut terrain,
                        &tanks,
                        &physics,
                        &mut Vec::new()
                    ),
                    Step::Alive
                ));
                assert_eq!(child.vel.x, -2000.0);
                assert!(child.pos.x < W as f32);
            }
        }
        let (physics, mut terrain, tanks) = scene(WallType::Rubber);
        let start = vec2(200.0, physics.ceiling + 1.0);
        let mut p = shot(ItemId::FunkyBomb, start, vec2(0.0, 0.0));
        p.age = 0.0;
        p.kind = PKind::FunkyTrail {
            start,
            target: start + vec2(20.0, 0.0),
            height: 40.0,
            duration: 1.0,
        };
        assert!(matches!(
            step_proj(
                &mut p,
                0.02,
                &mut terrain,
                &tanks,
                &physics,
                &mut Vec::new()
            ),
            Step::Alive
        ));
        assert!(p.kind == PKind::Bomblet && p.vel.y > 0.0);
    }
}
