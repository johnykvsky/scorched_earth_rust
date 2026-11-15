//! Original item catalogue, in binary weapon-table order (MECHANICS.md §7–8).

use crate::economy::{DamageEvent, DamageTarget};
use crate::gfx::{Rgb, H, W};
use crate::physics::{Physics, MAX_FLIGHT_TIME};
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
    Funky,
    Roller,
    Dirt,
    Tracer,
    /// Catalogued for future batches, without a firing implementation yet.
    Pending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Weapon(WKind),
    Accessory,
}

// Prices and shop filters become runtime inputs in batch 7.
#[allow(dead_code)]
pub struct Item {
    pub id: ItemId,
    pub name: &'static str,
    pub kind: ItemKind,
    /// Base price per bundle; Auto Defense pricing is added in batch 13.
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
}

// Generate IDs and storage size from the same rows, so adding an item needs no
// parallel enum/count/inventory edits. Keep the original 33 weapons in order.
macro_rules! catalogue {
    ($( $id:ident, $name:literal, $kind:expr, $cost:literal / $bundle:literal, $radius:literal, $arms:literal, $stock:literal; )*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
use WKind::{Blast, Dirt, Funky, Mirv, Pending, Roller, Tracer};

// ID, display name, behavior, cost / bundle, initial radius, arms, starter stock.
// Funky Bomb uses its radius-20 opening burst, not the ±80 scatter range.
catalogue! {
    BabyMissile, "Baby Missile", Weapon(Blast), 400 / 10, 10.0, 0, 99;
    Missile, "Missile", Weapon(Blast), 1875 / 5, 20.0, 0, 10;
    BabyNuke, "Baby Nuke", Weapon(Blast), 10000 / 3, 40.0, 0, 3;
    Nuke, "Nuke", Weapon(Blast), 12000 / 1, 75.0, 1, 1;
    LeapFrog, "Leap Frog", Weapon(Pending), 10000 / 2, 30.0, 3, 0;
    FunkyBomb, "Funky Bomb", Weapon(Funky), 7000 / 2, 20.0, 4, 2;
    Mirv, "MIRV", Weapon(Mirv), 10000 / 3, 20.0, 2, 2;
    DeathsHead, "Death's Head", Weapon(Pending), 20000 / 1, 35.0, 4, 0;
    Napalm, "Napalm", Weapon(Pending), 10000 / 10, 0.0, 2, 0;
    HotNapalm, "Hot Napalm", Weapon(Pending), 20000 / 2, 0.0, 4, 0;
    Tracer, "Tracer", Weapon(Tracer), 10 / 20, 0.0, 0, 20;
    SmokeTracer, "Smoke Tracer", Weapon(Pending), 500 / 10, 0.0, 1, 0;
    BabyRoller, "Baby Roller", Weapon(Pending), 5000 / 10, 10.0, 2, 0;
    Roller, "Roller", Weapon(Roller), 6000 / 5, 20.0, 2, 4;
    HeavyRoller, "Heavy Roller", Weapon(Pending), 6750 / 2, 45.0, 3, 0;
    RiotCharge, "Riot Charge", Weapon(Pending), 2000 / 10, 36.0, 2, 0;
    RiotBlast, "Riot Blast", Weapon(Pending), 5000 / 5, 60.0, 3, 0;
    RiotBomb, "Riot Bomb", Weapon(Pending), 5000 / 5, 30.0, 3, 0;
    HeavyRiotBomb, "Heavy Riot Bomb", Weapon(Pending), 4750 / 2, 45.0, 3, 0;
    BabyDigger, "Baby Digger", Weapon(Pending), 3000 / 10, 0.0, 0, 0;
    Digger, "Digger", Weapon(Pending), 2500 / 5, 0.0, 0, 0;
    HeavyDigger, "Heavy Digger", Weapon(Pending), 6750 / 2, 0.0, 1, 0;
    BabySandhog, "Baby Sandhog", Weapon(Pending), 10000 / 10, 0.0, 0, 0;
    Sandhog, "Sandhog", Weapon(Pending), 16750 / 5, 0.0, 0, 0;
    HeavySandhog, "Heavy Sandhog", Weapon(Pending), 25000 / 2, 0.0, 1, 0;
    DirtClod, "Dirt Clod", Weapon(Pending), 5000 / 10, 20.0, 0, 0;
    DirtBall, "Dirt Ball", Weapon(Dirt), 5000 / 5, 35.0, 0, 4;
    TonOfDirt, "Ton of Dirt", Weapon(Pending), 6750 / 2, 70.0, 1, 0;
    LiquidDirt, "Liquid Dirt", Weapon(Pending), 5000 / 10, 0.0, 2, 0;
    DirtCharge, "Dirt Charge", Weapon(Pending), 5000 / 5, 0.0, 1, 0;
    EarthDisrupter, "Earth Disrupter", Weapon(Pending), 5000 / 10, 0.0, 0, 0;
    PlasmaBlast, "Plasma Blast", Weapon(Pending), 9000 / 5, 20.0, 3, 0;
    Laser, "Laser", Weapon(Pending), 5000 / 5, 0.0, 2, 0;
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
    Rolling { dir: i32, timer: f32 },
}

#[derive(Clone, Copy)]
pub(crate) struct Proj {
    pub(crate) pos: Vec2,
    pub(crate) vel: Vec2,
    pub(crate) owner: usize,
    pub(crate) weapon: ItemId,
    pub(crate) kind: PKind,
    pub(crate) radius: f32,
    pub(crate) damage: Damage,
    pub(crate) age: f32,
    /// Copied to child warheads; Contact Triggers can override this in batch 14.
    pub(crate) tunneling: bool,
    /// Stay underground until leaving the last erased pixel, even at subpixel speeds.
    pub(crate) in_dirt: bool,
}

pub(crate) enum Step {
    Alive,
    Gone,
    Split,
    Hit(Vec2, Option<usize>),
}

pub(crate) struct Explosion {
    pub(crate) pos: Vec2,
    pub(crate) radius: f32,
    pub(crate) damage: Damage,
    pub(crate) owner: usize,
    pub(crate) dirt: bool,
    pub(crate) t: f32,
    pub(crate) dur: f32,
}

impl Explosion {
    pub(crate) fn new(pos: Vec2, radius: f32, damage: Damage, owner: usize, dirt: bool) -> Self {
        Explosion { pos, radius, damage, owner, dirt, t: 0.0, dur: 0.4 + radius / 100.0 }
    }
}

pub(crate) fn explosion_damage(distance: f32, radius: f32, damage: Damage) -> i32 {
    match damage {
        Damage::Weapon(index) => {
            if radius <= 0.0 || distance >= radius {
                return 0;
            }
            let damage =
                (((radius - distance) * 100.0 / radius).trunc() as i32) * (index as i32 + 1);
            if damage > 100 {
                110
            } else {
                damage
            }
        }
        Damage::Death => {
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

pub(crate) fn step_proj<T: TerrainAccess>(
    p: &mut Proj,
    dt: f32,
    terrain: &mut T,
    tanks: &[Tank],
    physics: &Physics,
    trail: &mut Vec<(i32, i32, Rgb)>,
) -> Step {
    p.age += dt;
    if let PKind::Rolling { dir, timer } = &mut p.kind {
        *timer += dt;
        while *timer >= 1.0 / 150.0 {
            *timer -= 1.0 / 150.0;
            let (nx, mut y) = (p.pos.x as i32 + *dir, p.pos.y as i32);
            if !(0..W).contains(&nx) {
                return Step::Gone;
            }
            if let Some(victim) = tank_at(tanks, nx, y, p.owner, 1.0) {
                return Step::Hit(vec2(nx as f32, y as f32), Some(victim));
            }
            if terrain.solid(nx, y) {
                return Step::Hit(p.pos, None); // reached a valley
            }
            while !terrain.solid(nx, y + 1) {
                y += 1;
            }
            p.pos = vec2(nx as f32, y as f32);
            trail.push((nx, y, tanks[p.owner].color));
        }
        if p.age > 15.0 {
            return Step::Hit(p.pos, None);
        }
        return Step::Alive;
    }

    if p.age > MAX_FLIGHT_TIME {
        return Step::Gone;
    }
    let can_tunnel = p.tunneling && p.kind != PKind::Carrier && p.weapon.item().can_tunnel();
    let prev_vy = p.vel.y;
    Physics::cap_speed(&mut p.vel);
    let mut remaining = dt;
    let mut air_time = 0.0;
    let mut previous_cell =
        if p.in_dirt { Some((p.pos.x.floor() as i32, p.pos.y.floor() as i32)) } else { None };

    // Walk grid boundaries in time order. A fast shot cannot skip a one-pixel
    // layer, and slower movement after each dirt pixel uses the remaining time.
    loop {
        let (x, y) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32);
        if physics.beyond_border(p.pos.x) {
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
            let c = tanks[p.owner].color;
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

        let bx = if p.vel.x > 0.0 { (x + 1) as f32 } else { x as f32 };
        let by = if p.vel.y > 0.0 { (y + 1) as f32 } else { y as f32 };
        let tx = if p.vel.x == 0.0 { f32::INFINITY } else { (bx - p.pos.x) / p.vel.x };
        let ty = if p.vel.y == 0.0 { f32::INFINITY } else { (by - p.pos.y) / p.vel.y };
        let elapsed = remaining.min(tx.min(ty)).max(0.0);
        if !p.in_dirt {
            air_time += elapsed;
        }
        p.pos += p.vel * elapsed;
        remaining = (remaining - elapsed).max(0.0);
        // Move to the next representable coordinate, including negative travel,
        // so an exact grid-boundary hit always progresses to the next cell.
        if tx <= elapsed {
            p.pos.x = if p.vel.x > 0.0 { bx.next_up() } else { bx.next_down() };
        }
        if ty <= elapsed {
            p.pos.y = if p.vel.y > 0.0 { by.next_up() } else { by.next_down() };
        }
        if tx > elapsed && ty > elapsed {
            break;
        }
    }
    physics.air_forces(&mut p.vel, air_time, dt);
    if p.kind == PKind::Carrier && prev_vy < 0.0 && p.vel.y >= 0.0 {
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
    if p.kind != PKind::Carrier && !matches!(kind, WKind::Tracer | WKind::Dirt) {
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
            }
        }
    }
    let mut boom = |r: f32, d: Damage, dirt: bool| {
        expls.push(Explosion::new(pos, r, d, p.owner, dirt));
    };
    match (p.kind, kind) {
        (PKind::Carrier, _) => {} // Before apogee, any impact is a dud.
        (PKind::Shell, WKind::Tracer) => {}
        (PKind::Shell, WKind::Dirt) => boom(p.radius, p.damage, true),
        (PKind::Shell, WKind::Funky) => {
            boom(p.radius, p.damage, false);
            for _ in 0..6 {
                spawn.push(Proj {
                    pos: pos - vec2(0.0, 4.0),
                    vel: vec2(gen_range(-120.0, 120.0), gen_range(-220.0, -100.0)),
                    kind: PKind::Bomblet,
                    radius: 15.0,
                    damage: p.damage,
                    age: 0.0,
                    in_dirt: false,
                    ..p
                });
            }
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
                kind: PKind::Rolling { dir, timer: 0.0 },
                ..p
            });
        }
        (_, WKind::Pending) => {}
        _ => boom(p.radius, p.damage, false),
    }
    direct_hit
}

#[cfg(test)]
mod catalogue_tests {
    use super::*;

    #[test]
    fn catalogue_matches_documented_prices_bundles_arms_and_original_weapon_indices() {
        let manual = include_str!("../MECHANICS.md");
        let weapons =
            manual.split("### 7.1 Weapon table").nth(1).unwrap().split("### 7.2").next().unwrap();
        let accessories = manual
            .split("### 8.1 Accessory table")
            .nth(1)
            .unwrap()
            .split("### 8.2")
            .next()
            .unwrap();
        let mut checked = 0;
        for (section, category) in
            [(weapons, Category::Weapon), (accessories, Category::Miscellaneous)]
        {
            for line in section.lines().filter(|line| line.starts_with('|') && line.contains('$')) {
                let cells: Vec<_> = line.split('|').map(str::trim).collect();
                let (name, offer, arms) = match category {
                    Category::Weapon => (cells[2], cells[3], cells[5]),
                    Category::Miscellaneous => (cells[1], cells[2], cells[3]),
                };
                let item = ITEMS.iter().find(|item| item.name == name).expect(name);
                let (price, bundle) = offer.split_once('/').unwrap();
                let price: u32 =
                    price.chars().filter(char::is_ascii_digit).collect::<String>().parse().unwrap();
                let bundle: u8 = bundle.split_whitespace().next().unwrap().parse().unwrap();
                assert_eq!(
                    (item.cost, item.bundle_size, item.arms_level, item.category),
                    (price, bundle, arms.parse::<u8>().unwrap(), category),
                    "{name}"
                );
                assert_eq!(ITEMS[item.id as usize].id, item.id);
                if category == Category::Weapon {
                    assert_eq!(item.id as usize, cells[1].parse::<usize>().unwrap(), "{name}");
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
                    damage: Damage::Weapon(0),
                    age: 0.0,
                    tunneling: true,
                    in_dirt: false,
                },
                terrain: Terrain::flat(H),
                tanks: vec![Tank {
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
                    weapon: ItemId::BabyMissile,
                    fall: 0,
                    last_hit: None,
                    kills: 0,
                    wins: 0,
                }],
                physics: Physics { gravity: 0.0, wind: 0.0, air_viscosity: 0, borders_extend: 75 },
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
            &PhysicsSettings { air_viscosity: 20, gravity: 10.0, ..Default::default() },
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
        assert_eq!(children.len(), 6);
        assert!(children.iter().all(|p| p.tunneling && !p.in_dirt));
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
        for id in [ItemId::BabyMissile, ItemId::Roller, ItemId::Tracer, ItemId::DirtCharge] {
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
