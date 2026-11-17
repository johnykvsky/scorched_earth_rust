//! Original computer personalities, persistent ranging memory and bounded private prediction.
#[cfg(test)]
use crate::physics::MAX_FLIGHT_TIME;
use crate::physics::{Physics, SIM_DT};
use crate::settings::{WallType, MAX_PLAYERS};
use crate::tank::{Ctrl, Tank};
use crate::terrain::{PredictionTerrain, Terrain};
use crate::weapons::{self, ItemId, PKind, Proj, Step, ITEMS};
use macroquad::prelude::{vec2, Vec2};
use macroquad::rand::gen_range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiKind {
    Moron,
    Shooter,
    Poolshark,
    Tosser,
    Chooser,
    Spoiler,
    Cyborg,
    Unknown,
}
impl AiKind {
    pub const KNOWN: [Self; 7] = [
        Self::Moron,
        Self::Shooter,
        Self::Poolshark,
        Self::Tosser,
        Self::Chooser,
        Self::Spoiler,
        Self::Cyborg,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Moron => "Moron",
            Self::Shooter => "Shooter",
            Self::Poolshark => "Poolshark",
            Self::Tosser => "Tosser",
            Self::Chooser => "Chooser",
            Self::Spoiler => "Spoiler",
            Self::Cyborg => "Cyborg",
            Self::Unknown => "Unknown",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RangingShot {
    pub angle: i32,
    pub power: i32,
    pub origin: Vec2,
    pub target: Vec2,
    pub landing: Option<Vec2>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// Internal only: UI continues to display Unknown.
    pub resolved: Option<AiKind>,
    pub shots: [Option<RangingShot>; MAX_PLAYERS],
    pub grudges: [u32; MAX_PLAYERS],
    pub active_target: Option<usize>,
    pub prepared: bool,
}
impl State {
    pub fn new(ctrl: Ctrl) -> Self {
        let resolved = match ctrl {
            Ctrl::Human => None,
            Ctrl::Ai(AiKind::Unknown) => Some(AiKind::KNOWN[gen_range(0, 7)]),
            Ctrl::Ai(kind) => Some(kind),
        };
        Self {
            resolved,
            ..Self::default()
        }
    }
    pub fn round_reset(&mut self) {
        self.shots.fill(None);
        self.active_target = None;
        self.prepared = false;
    }
    pub fn effective(&self, selected: AiKind) -> AiKind {
        if selected == AiKind::Unknown {
            self.resolved.unwrap_or(AiKind::Moron)
        } else {
            selected
        }
    }
}

pub struct Plan {
    pub angle: i32,
    pub power: i32,
    pub weapon: ItemId,
    pub target: Option<usize>,
}

pub fn name_computers(tanks: &mut [Tank]) {
    const NAMES: [&str; 16] = [
        "Boomer",
        "Daisy",
        "Gunner",
        "Ricochet",
        "Rusty",
        "Sparks",
        "Dusty",
        "Cannonball",
        "Echo",
        "Fuse",
        "Buster",
        "Zigzag",
        "Copper",
        "Crater",
        "Ace",
        "Flint",
    ];
    let mut used: Vec<String> = tanks
        .iter()
        .filter(|t| t.ctrl == Ctrl::Human)
        .map(|t| t.name.trim().to_lowercase())
        .collect();
    for tank in tanks.iter_mut().filter(|t| t.ctrl != Ctrl::Human) {
        let start = gen_range(0, NAMES.len());
        let mut number = 0;
        let name = loop {
            let candidate = if number < NAMES.len() {
                NAMES[(start + number) % NAMES.len()].to_string()
            } else {
                format!("Computer {}", number - NAMES.len() + 1)
            };
            number += 1;
            if !used.contains(&candidate.to_lowercase()) {
                break candidate;
            }
        };
        used.push(name.to_lowercase());
        tank.name = name;
    }
}

/// Defensive supplies are used independently of the Computers Buy setting.
pub fn prepare(tank: &mut Tank) {
    if let Ctrl::Ai(kind) = tank.ctrl {
        if kind == AiKind::Unknown && tank.ai_state.resolved.is_none() {
            tank.ai_state = State::new(tank.ctrl);
        }
    } else {
        return;
    }
    while tank.health <= 90 && tank.use_battery() {}
    tank.equipment.parachutes_deployed = true;
    if let Some(id) =
        crate::tank::SHIELDS.iter().rev().copied().find(|&id| {
            tank.inventory.count(id) > 0
                && tank.equipment.active_shield.as_ref().is_none_or(|active| {
                    crate::shields::spec(id).unwrap().strength > active.strength
                })
        })
    {
        tank.engage_shield(id);
    }
}

pub fn clear_shot(terrain: &Terrain, from: Vec2, to: Vec2) -> bool {
    let length = from.distance(to).ceil().max(1.0) as usize;
    (0..length).all(|i| {
        let point = from.lerp(to, i as f32 / length as f32);
        !terrain.solid(point.x.floor() as i32, point.y.floor() as i32)
    })
}
fn enemies(tanks: &[Tank], me: usize) -> Vec<usize> {
    tanks
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            t.alive
                && t.health > 0
                && crate::economy::relation(tanks, me, *i) == crate::economy::Relation::Enemy
        })
        .map(|(i, _)| i)
        .collect()
}
fn nearest(tanks: &[Tank], me: usize, choices: &[usize]) -> Option<usize> {
    choices.iter().copied().min_by(|&a, &b| {
        tanks[a]
            .center()
            .distance_squared(tanks[me].center())
            .total_cmp(&tanks[b].center().distance_squared(tanks[me].center()))
    })
}
pub fn target_for(kind: AiKind, tanks: &[Tank], me: usize) -> Option<usize> {
    let choices = enemies(tanks, me);
    if choices.is_empty() {
        return None;
    }
    match kind {
        AiKind::Moron | AiKind::Spoiler => Some(choices[gen_range(0, choices.len())]),
        AiKind::Tosser => tanks[me]
            .ai_state
            .active_target
            .filter(|i| choices.contains(i))
            .or_else(|| nearest(tanks, me, &choices)),
        AiKind::Cyborg => choices.iter().copied().max_by_key(|&i| {
            let richer = choices
                .iter()
                .filter(|&&j| tanks[j].account.cash < tanks[i].account.cash)
                .count() as u64;
            (100 - tanks[i].health).max(0) as u64 * 3
                + u64::from(tanks[i].wins) * 100
                + tanks[i].kills.max(0) as u64 * 20
                + u64::from(tanks[me].ai_state.grudges[i]) * 5
                + richer * 20
        }),
        _ => nearest(tanks, me, &choices),
    }
}
pub fn method_for(kind: AiKind, clear: bool, wall: WallType) -> AiKind {
    if kind != AiKind::Chooser {
        return kind;
    }
    if clear {
        AiKind::Shooter
    } else if matches!(wall, WallType::Rubber | WallType::Spring) {
        AiKind::Poolshark
    } else {
        AiKind::Spoiler
    }
}

struct Prediction {
    point: Vec2,
    bounces: usize,
    hit: Option<usize>,
}
fn probe(
    terrain: &Terrain,
    tanks: &[Tank],
    me: usize,
    aim: (i32, i32),
    physics: &Physics,
    tunneling: bool,
    seconds: f32,
) -> Prediction {
    let (angle, power) = aim;
    let (pos, dir) = tanks[me].muzzle(angle);
    let mut shot = Proj {
        pos,
        vel: dir * power as f32,
        owner: me,
        weapon: ItemId::BabyMissile,
        kind: PKind::Shell,
        radius: 10.0,
        scale: 1.0,
        damage: ItemId::BabyMissile.item().damage(),
        age: 0.0,
        tunneling,
        in_dirt: false,
        guidance: None,
    };
    let mut trail = Vec::with_capacity(4);
    let mut predicted_tanks = tanks.to_vec();
    let mut predicted_terrain = PredictionTerrain::new(terrain);
    let mut bounces = 0;
    for _ in 0..=(seconds / SIM_DT) as usize {
        let previous = shot;
        let step = weapons::step_proj(
            &mut shot,
            SIM_DT,
            &mut predicted_terrain,
            &predicted_tanks,
            physics,
            &mut trail,
        );
        if matches!(step, Step::Alive)
            && physics.walls.bounce_factor().is_some()
            && (((previous.pos.x < 2.0 || previous.pos.x > crate::gfx::W as f32 - 2.0)
                && previous.vel.x * shot.vel.x < 0.0)
                || (previous.pos.y < physics.ceiling + 2.0
                    && previous.vel.y < 0.0
                    && shot.vel.y > 0.0))
        {
            bounces += 1;
        }
        match step {
            Step::Alive => trail.clear(),
            Step::Shield {
                victim,
                cost,
                continues,
            } => {
                predicted_tanks[victim].drain_shield(cost);
                if !continues {
                    return Prediction {
                        point: shot.pos,
                        bounces,
                        hit: Some(victim),
                    };
                }
                trail.clear();
            }
            Step::Hit(point, hit) => {
                return Prediction {
                    point,
                    bounces,
                    hit,
                }
            }
            Step::Wall(point) => {
                return Prediction {
                    point,
                    bounces,
                    hit: None,
                }
            }
            Step::Gone | Step::Split => break,
        }
    }
    Prediction {
        point: shot.pos,
        bounces,
        hit: None,
    }
}
#[cfg(test)]
fn sim(
    terrain: &Terrain,
    tanks: &[Tank],
    me: usize,
    angle: i32,
    power: i32,
    physics: &Physics,
    tunneling: bool,
) -> Vec2 {
    probe(
        terrain,
        tanks,
        me,
        (angle, power),
        physics,
        tunneling,
        crate::physics::MAX_FLIGHT_TIME,
    )
    .point
}
fn aiming_physics(kind: AiKind, physics: &Physics) -> Physics {
    let mut predicted = *physics;
    if !matches!(kind, AiKind::Spoiler | AiKind::Cyborg) {
        predicted.wind = 0.0;
    }
    if kind != AiKind::Moron {
        predicted.air_viscosity = 0;
    }
    predicted
}
fn angle_to(from: Vec2, to: Vec2) -> i32 {
    let delta = to - from;
    (-delta.y)
        .atan2(delta.x)
        .to_degrees()
        .round()
        .clamp(0.0, 180.0) as i32
}
fn start_angle(kind: AiKind, from: Vec2, target: Vec2, wind: i32, viscosity: u8) -> i32 {
    let right = target.x >= from.x;
    match kind {
        AiKind::Shooter | AiKind::Poolshark => angle_to(from, target),
        AiKind::Cyborg => {
            let tilt = ((3 * wind / 10).abs() + i32::from(viscosity) / 2).min(70);
            if right {
                85 - tilt
            } else {
                95 + tilt
            }
        }
        _ => {
            if right {
                65
            } else {
                115
            }
        }
    }
}

/// Tosser changes once from the last real landing, never searches an oracle.
fn toss(tank: &Tank, target_index: usize, target: Vec2) -> (i32, i32) {
    let right = target.x >= tank.x as f32;
    if let Some(previous) = tank.ai_state.shots[target_index].filter(|s| s.landing.is_some()) {
        let landing = previous.landing.unwrap();
        let sign = if right { 1.0 } else { -1.0 };
        let overshot = (landing.x - target.x) * sign > 0.0;
        let mut angle = previous.angle;
        let power = previous.power + if overshot { -10 } else { 10 };
        if !overshot && landing.y < target.y && !(85..=95).contains(&angle) {
            angle += if angle < 90 { 2 } else { -2 };
        }
        (angle.clamp(0, 180), power.clamp(0, tank.max_power()))
    } else {
        let angle = gen_range(10, 80);
        (
            if right { angle } else { 180 - angle },
            gen_range(1, tank.health.max(1) + 1) * 10,
        )
    }
}

pub fn plan(
    kind: AiKind,
    terrain: &Terrain,
    tanks: &[Tank],
    me: usize,
    physics: &Physics,
    tunneling: bool,
    scale: f32,
) -> Plan {
    let tank = &tanks[me];
    if !tank.alive || tank.health <= 0 {
        return Plan {
            angle: 90,
            power: 0,
            weapon: ItemId::BabyMissile,
            target: None,
        };
    }
    let kind = tank.ai_state.effective(kind);
    let Some(target_index) = target_for(kind, tanks, me) else {
        return Plan {
            angle: 90,
            power: 0,
            weapon: ItemId::BabyMissile,
            target: None,
        };
    };
    let target = vec2(tanks[target_index].x as f32, tanks[target_index].y as f32);
    let home = tank.center();
    let method = method_for(
        kind,
        clear_shot(terrain, home, tanks[target_index].center()),
        physics.walls,
    );
    let buried = terrain.solid(
        tank.muzzle(tank.angle).0.x as i32,
        tank.muzzle(tank.angle).0.y as i32,
    );
    if buried {
        if let Some(weapon) = [ItemId::RiotBlast, ItemId::RiotCharge]
            .into_iter()
            .find(|&id| tank.inventory.can_select(id))
        {
            return Plan {
                angle: 90,
                power: 0,
                weapon,
                target: Some(target_index),
            };
        }
    }
    if method == AiKind::Moron {
        let choices: Vec<_> = ITEMS
            .iter()
            .filter(|item| tank.inventory.can_select(item.id))
            .map(|item| item.id)
            .collect();
        let left = enemies(tanks, me).iter().any(|&i| tanks[i].x < tank.x);
        let right = enemies(tanks, me).iter().any(|&i| tanks[i].x >= tank.x);
        let mut angle = gen_range(0, 181);
        if !left {
            angle = gen_range(0, 90);
        } else if !right {
            angle = gen_range(91, 181);
        }
        return Plan {
            angle,
            power: gen_range(1, tank.health.max(1) + 1) * 10,
            weapon: choices[gen_range(0, choices.len())],
            target: Some(target_index),
        };
    }
    let predicted = aiming_physics(method, physics);
    let (mut angle, mut power) = if method == AiKind::Tosser {
        toss(tank, target_index, target)
    } else {
        let start = start_angle(
            method,
            home,
            target,
            (physics.wind / 1.25) as i32,
            physics.air_viscosity,
        );
        let mut best = (start, tank.max_power(), f32::MAX);
        let search = if method == AiKind::Shooter || method == AiKind::Poolshark {
            15
        } else {
            89
        };
        for offset in 0..=search {
            for sign in [1, -1] {
                if offset == 0 && sign == -1 {
                    continue;
                }
                let candidate = start + offset * sign;
                if !(0..=180).contains(&candidate) || ((candidate < 90) != (target.x >= home.x)) {
                    continue;
                }
                let (origin, direction) = tank.muzzle(candidate);
                let power = crate::guidance::ballistic_power(
                    origin,
                    direction,
                    target,
                    &predicted,
                    tank.max_power(),
                );
                let hit = probe(
                    terrain,
                    tanks,
                    me,
                    (candidate, power),
                    &predicted,
                    tunneling,
                    12.0,
                );
                let error = if hit.hit == Some(target_index) {
                    0.0
                } else {
                    hit.point.distance(target)
                };
                if error < best.2 {
                    best = (candidate, power, error);
                }
                if error == 0.0 {
                    break;
                }
            }
            if best.2 == 0.0 {
                break;
            }
        }
        if method == AiKind::Poolshark
            && matches!(physics.walls, WallType::Rubber | WallType::Spring)
        {
            // Images seed side-wall banks. The second sweep includes ceiling banks;
            // every candidate is checked against the shared live wall physics.
            let mut bank = (best.0, best.1, f32::MAX);
            for candidate in (5..180).step_by(5) {
                let (origin, direction) = tank.muzzle(candidate);
                for mirror in [
                    vec2(-target.x, target.y),
                    vec2(2.0 * crate::gfx::W as f32 - target.x, target.y),
                ] {
                    let solved = crate::guidance::ballistic_power(
                        origin,
                        direction,
                        mirror,
                        &predicted,
                        tank.max_power(),
                    );
                    for factor in [0.5, 0.75, 1.0] {
                        let power = (solved as f32 * factor).round() as i32;
                        let hit = probe(
                            terrain,
                            tanks,
                            me,
                            (candidate, power),
                            &predicted,
                            tunneling,
                            12.0,
                        );
                        if hit.bounces > 0 {
                            let error = if hit.hit == Some(target_index) {
                                0.0
                            } else {
                                hit.point.distance(target)
                            };
                            if error < bank.2 {
                                bank = (candidate, power, error);
                            }
                        }
                    }
                }
            }
            for candidate in (10..180).step_by(10) {
                for power in (100..=tank.max_power()).step_by(100) {
                    let hit = probe(
                        terrain,
                        tanks,
                        me,
                        (candidate, power),
                        &predicted,
                        tunneling,
                        12.0,
                    );
                    if hit.bounces > 0 {
                        let error = if hit.hit == Some(target_index) {
                            0.0
                        } else {
                            hit.point.distance(target)
                        };
                        if error < bank.2 {
                            bank = (candidate, power, error);
                        }
                    }
                }
            }
            if bank.2.is_finite() {
                let seed = bank;
                for a in -3..=3 {
                    for delta in [-30, -15, 0, 15, 30] {
                        let candidate = (seed.0 + a).clamp(0, 180);
                        let power = (seed.1 + delta).clamp(0, tank.max_power());
                        let hit = probe(
                            terrain,
                            tanks,
                            me,
                            (candidate, power),
                            &predicted,
                            tunneling,
                            12.0,
                        );
                        if hit.bounces > 0 {
                            let error = if hit.hit == Some(target_index) {
                                0.0
                            } else {
                                hit.point.distance(target)
                            };
                            if error < bank.2 {
                                bank = (candidate, power, error);
                            }
                        }
                    }
                }
                if bank.2 <= best.2 + 1.0 {
                    best = bank;
                }
            }
        }
        (best.0, best.1)
    };
    let distance = home.distance(target);
    let mut weapon = ItemId::BabyMissile;
    for id in [
        ItemId::Nuke,
        ItemId::BabyNuke,
        ItemId::DeathsHead,
        ItemId::FunkyBomb,
        ItemId::Mirv,
        ItemId::LeapFrog,
        ItemId::HeavyRoller,
        ItemId::Roller,
        ItemId::HeavySandhog,
        ItemId::Sandhog,
        ItemId::BabySandhog,
        ItemId::Missile,
    ] {
        let reach = if id == ItemId::FunkyBomb {
            80.0 + 40.0 * scale
        } else {
            weapons::initial_radius(id, scale)
        };
        if tank.inventory.can_select(id) && reach + 25.0 < distance {
            weapon = id;
            break;
        }
    }
    if method != AiKind::Tosser {
        let batteries = tank.inventory.count(ItemId::Battery).min(10);
        if tank.inventory.can_select(ItemId::Laser)
            && batteries > 0
            && distance + 15.0 < (tank.max_power() * i32::from(batteries) / 40) as f32
            && clear_shot(terrain, home, tanks[target_index].center())
        {
            weapon = ItemId::Laser;
            angle = angle_to(home, tanks[target_index].center());
            power = tank.max_power();
        } else if tank.inventory.can_select(ItemId::PlasmaBlast)
            && distance < 20.0 + 5.5 * f32::from(batteries)
        {
            weapon = ItemId::PlasmaBlast;
        } else if (tank.inventory.can_select(ItemId::HotNapalm)
            || tank.inventory.can_select(ItemId::Napalm))
            && (!tunneling || tank.inventory.count(ItemId::ContactTrigger) > 0)
        {
            let hit = probe(terrain, tanks, me, (angle, power), &predicted, false, 12.0);
            if hit.point.distance(target) < 25.0 {
                weapon = if tank.inventory.can_select(ItemId::HotNapalm) {
                    ItemId::HotNapalm
                } else {
                    ItemId::Napalm
                };
            }
        }
    }
    Plan {
        angle,
        power,
        weapon,
        target: Some(target_index),
    }
}

pub fn equip_for_plan(
    tank: &mut Tank,
    plan: &Plan,
    terrain: &Terrain,
    target: Option<Vec2>,
    tunneling: bool,
) {
    tank.equipment.triggers = tunneling && tank.inventory.count(ItemId::ContactTrigger) > 0;
    if !crate::guidance::applies(plan.weapon) {
        return;
    }
    let blocked = target.is_some_and(|point| !clear_shot(terrain, tank.center(), point));
    let preferences = if blocked {
        [
            ItemId::LazyBoy,
            ItemId::VerticalGuidance,
            ItemId::HeatGuidance,
            ItemId::HorizontalGuidance,
            ItemId::BallisticGuidance,
        ]
    } else {
        [
            ItemId::HeatGuidance,
            ItemId::BallisticGuidance,
            ItemId::HorizontalGuidance,
            ItemId::VerticalGuidance,
            ItemId::LazyBoy,
        ]
    };
    tank.equipment.guidance = preferences
        .into_iter()
        .find(|&id| tank.inventory.count(id) > 0);
}
pub fn record_shot(tank: &mut Tank, target: Option<usize>, point: Option<Vec2>) {
    if tank.ctrl == Ctrl::Human {
        return;
    }
    tank.ai_state.active_target = target;
    if let Some(index) = target.filter(|i| *i < MAX_PLAYERS) {
        tank.ai_state.shots[index] = Some(RangingShot {
            angle: tank.angle,
            power: tank.power,
            origin: tank.center(),
            target: point.unwrap_or(tank.center()),
            landing: None,
        });
    }
}
pub fn record_landing(tank: &mut Tank, point: Vec2) {
    if let Some(index) = tank.ai_state.active_target {
        if let Some(shot) = tank.ai_state.shots[index].as_mut() {
            if shot.landing.is_none() {
                shot.landing = Some(point);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gfx::Rgb, inventory::Inventory, settings::PhysicsSettings, tank::Ctrl};

    fn tanks() -> Vec<Tank> {
        [100, 600]
            .into_iter()
            .map(|x| Tank {
                bank: None,
                ai_state: crate::ai::State::default(),
                shape: crate::tank::Shape::Treads,
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
                equipment: crate::tank::Equipment::default(),
                weapon: ItemId::BabyMissile,
                fall: 0,
                last_hit: None,
                environmental_hit: false,
                kills: 0,
                wins: 0,
            })
            .collect()
    }

    #[test]
    fn every_personality_produces_legal_owned_shots_and_handles_no_enemies() {
        let terrain = Terrain::flat(400);
        let mut tanks = tanks();
        for tank in &mut tanks {
            tank.y = 399;
        }
        let physics = Physics::new(&PhysicsSettings::default(), 0);
        for kind in AiKind::KNOWN.into_iter().chain([AiKind::Unknown]) {
            tanks[0].ctrl = Ctrl::Ai(kind);
            tanks[0].ai_state = State::new(tanks[0].ctrl);
            let plan = plan(kind, &terrain, &tanks, 0, &physics, false, 1.0);
            assert!((0..=180).contains(&plan.angle));
            assert!((0..=1000).contains(&plan.power));
            assert_eq!(plan.target, Some(1));
            assert!(tanks[0].inventory.can_select(plan.weapon));
        }
        tanks[1].alive = false;
        assert!(
            plan(AiKind::Cyborg, &terrain, &tanks, 0, &physics, false, 1.0)
                .target
                .is_none()
        );
    }

    #[test]
    fn nearest_target_uses_geometry_and_excludes_eliminated_tanks() {
        let mut tanks = tanks();
        tanks[1].x = 500;
        let mut nearer = tanks[1].clone();
        nearer.x = 350;
        nearer.y = 150;
        tanks.push(nearer);
        assert_eq!(target_for(AiKind::Shooter, &tanks, 0), Some(2));
        tanks[2].health = 0;
        assert_eq!(target_for(AiKind::Shooter, &tanks, 0), Some(1));
    }

    #[test]
    fn moron_uses_health_quantized_power_and_only_a_side_with_enemies() {
        let terrain = Terrain::flat(450);
        let mut tanks = tanks();
        tanks[0].health = 37;
        let physics = Physics::new(&PhysicsSettings::default(), 0);
        for _ in 0..100 {
            let plan = plan(AiKind::Moron, &terrain, &tanks, 0, &physics, false, 1.0);
            assert!(plan.angle < 90);
            assert!((10..=370).contains(&plan.power) && plan.power % 10 == 0);
        }
        tanks[0].x = 620;
        for _ in 0..30 {
            assert!(plan(AiKind::Moron, &terrain, &tanks, 0, &physics, false, 1.0).angle > 90);
        }
    }

    #[test]
    fn shooter_hits_open_targets_but_spoiler_can_lob_over_a_ridge() {
        let mut terrain = Terrain::flat(400);
        let mut tanks = tanks();
        for tank in &mut tanks {
            tank.y = 399;
        }
        let physics = Physics::new(&PhysicsSettings::default(), 0);
        let direct = plan(AiKind::Shooter, &terrain, &tanks, 0, &physics, false, 1.0);
        let hit = probe(
            &terrain,
            &tanks,
            0,
            (direct.angle, direct.power),
            &physics,
            false,
            12.0,
        );
        assert_eq!(hit.hit, Some(1));
        for x in 300..310 {
            for y in 150..400 {
                terrain.add_dirt(x, y, 0, 22);
            }
        }
        let shooter = plan(AiKind::Shooter, &terrain, &tanks, 0, &physics, false, 1.0);
        let spoiler = plan(AiKind::Spoiler, &terrain, &tanks, 0, &physics, false, 1.0);
        let blocked = probe(
            &terrain,
            &tanks,
            0,
            (shooter.angle, shooter.power),
            &physics,
            false,
            12.0,
        );
        let lob = probe(
            &terrain,
            &tanks,
            0,
            (spoiler.angle, spoiler.power),
            &physics,
            false,
            12.0,
        );
        assert_ne!(blocked.hit, Some(1));
        assert_eq!(lob.hit, Some(1));
        assert!(terrain.solid(300, 200));
    }

    #[test]
    fn poolshark_finds_a_real_bank_and_chooser_selects_only_applicable_methods() {
        let terrain = Terrain::flat(400);
        let mut tanks = tanks();
        for tank in &mut tanks {
            tank.y = 399;
        }
        let mut physics = Physics::new(&PhysicsSettings::default(), 0);
        physics.walls = WallType::Rubber;
        let bank = plan(AiKind::Poolshark, &terrain, &tanks, 0, &physics, false, 1.0);
        let hit = probe(
            &terrain,
            &tanks,
            0,
            (bank.angle, bank.power),
            &physics,
            false,
            12.0,
        );
        assert!(
            hit.bounces > 0,
            "no bank: angle {} power {}",
            bank.angle,
            bank.power
        );
        assert_eq!(hit.hit, Some(1));
        assert_eq!(
            method_for(AiKind::Chooser, true, WallType::Rubber),
            AiKind::Shooter
        );
        assert_eq!(
            method_for(AiKind::Chooser, false, WallType::Rubber),
            AiKind::Poolshark
        );
        assert_eq!(
            method_for(AiKind::Chooser, false, WallType::Spring),
            AiKind::Poolshark
        );
        assert_eq!(
            method_for(AiKind::Chooser, false, WallType::Padded),
            AiKind::Spoiler
        );
        assert_eq!(
            method_for(AiKind::Chooser, false, WallType::None),
            AiKind::Spoiler
        );
    }

    #[test]
    fn poolshark_handles_spring_walls_and_uses_the_ceiling_over_a_ridge() {
        let mut terrain = Terrain::flat(400);
        let mut tanks = tanks();
        for tank in &mut tanks {
            tank.y = 399;
        }
        let mut physics = Physics::new(&PhysicsSettings::default(), 0);
        physics.walls = WallType::Spring;
        let spring = plan(AiKind::Poolshark, &terrain, &tanks, 0, &physics, false, 1.0);
        let hit = probe(
            &terrain,
            &tanks,
            0,
            (spring.angle, spring.power),
            &physics,
            false,
            12.0,
        );
        assert!(hit.bounces > 0);
        assert_eq!(hit.hit, Some(1));
        physics.walls = WallType::Rubber;
        for x in 300..310 {
            for y in 150..400 {
                terrain.add_dirt(x, y, 0, 22);
            }
        }
        let bank = plan(AiKind::Poolshark, &terrain, &tanks, 0, &physics, false, 1.0);
        let hit = probe(
            &terrain,
            &tanks,
            0,
            (bank.angle, bank.power),
            &physics,
            false,
            12.0,
        );
        assert_eq!(
            hit.hit,
            Some(1),
            "ridge bank: {} {} at {:?}",
            bank.angle,
            bank.power,
            hit.point
        );
        let (pos, dir) = tanks[0].muzzle(bank.angle);
        let mut shot = Proj {
            pos,
            vel: dir * bank.power as f32,
            owner: 0,
            weapon: ItemId::BabyMissile,
            kind: PKind::Shell,
            radius: 10.0,
            scale: 1.0,
            damage: ItemId::BabyMissile.item().damage(),
            age: 0.0,
            tunneling: false,
            in_dirt: false,
            guidance: None,
        };
        let mut ceiling = false;
        for _ in 0..7200 {
            let previous = shot;
            let step = weapons::step_proj(
                &mut shot,
                SIM_DT,
                &mut terrain,
                &tanks,
                &physics,
                &mut Vec::new(),
            );
            ceiling |=
                previous.pos.y < physics.ceiling + 2.0 && previous.vel.y < 0.0 && shot.vel.y > 0.0;
            if !matches!(step, Step::Alive) {
                break;
            }
        }
        assert!(ceiling, "bank never reached the ceiling");
    }

    #[test]
    fn tosser_adjusts_once_from_real_per_target_landings() {
        let mut tanks = tanks();
        tanks[0].ctrl = Ctrl::Ai(AiKind::Tosser);
        tanks[0].angle = 60;
        tanks[0].power = 400;
        for (landing, angle, power) in [
            (vec2(650.0, 400.0), 60, 390),
            (vec2(500.0, 350.0), 62, 410),
            (vec2(500.0, 450.0), 60, 410),
        ] {
            record_shot(&mut tanks[0], Some(1), Some(vec2(600.0, 400.0)));
            record_landing(&mut tanks[0], landing);
            record_landing(&mut tanks[0], vec2(0.0, 0.0)); // Child impacts cannot overwrite the ranging result.
            assert_eq!(toss(&tanks[0], 1, vec2(600.0, 400.0)), (angle, power));
            assert!(tanks[0].ai_state.shots[2].is_none());
        }
        tanks[0].angle = 90;
        record_shot(&mut tanks[0], Some(1), Some(vec2(600.0, 400.0)));
        record_landing(&mut tanks[0], vec2(500.0, 350.0));
        assert_eq!(toss(&tanks[0], 1, vec2(600.0, 400.0)), (90, 410));
    }

    #[test]
    fn cyborg_targets_weakness_leaders_and_attackers_but_never_dead_teammates() {
        let mut tanks = tanks();
        let mut weak = tanks[1].clone();
        weak.x = 350;
        weak.health = 20;
        tanks.push(weak);
        assert_eq!(target_for(AiKind::Cyborg, &tanks, 0), Some(2));
        tanks[1].wins = 5;
        assert_eq!(target_for(AiKind::Cyborg, &tanks, 0), Some(1));
        tanks[0].ai_state.grudges[2] = 100;
        assert_eq!(target_for(AiKind::Cyborg, &tanks, 0), Some(2));
        tanks[2].alive = false;
        assert_eq!(target_for(AiKind::Cyborg, &tanks, 0), Some(1));
        tanks[0].team = Some(1);
        tanks[1].team = Some(1);
        assert!(target_for(AiKind::Cyborg, &tanks, 0).is_none());
        assert_eq!(
            start_angle(
                AiKind::Cyborg,
                vec2(100.0, 400.0),
                vec2(600.0, 400.0),
                100,
                20
            ),
            45
        );
    }

    #[test]
    fn unknown_is_stable_secret_and_round_reset_keeps_personality_and_grudges() {
        let mut tank = tanks().remove(0);
        tank.ctrl = Ctrl::Ai(AiKind::Unknown);
        tank.ai_state = State::new(tank.ctrl);
        let hidden = tank.ai_state.resolved.unwrap();
        assert!(AiKind::KNOWN.contains(&hidden));
        tank.ai_state.grudges[1] = 12;
        for _ in 0..10 {
            prepare(&mut tank);
            assert_eq!(tank.ai_state.resolved, Some(hidden));
        }
        tank.ai_state.round_reset();
        assert_eq!(tank.ai_state.resolved, Some(hidden));
        assert_eq!(tank.ai_state.grudges[1], 12);
        assert_eq!(tank.ctrl.label(), "Unknown");
    }

    #[test]
    fn computer_names_avoid_humans_and_each_other_and_defenses_do_not_waste_stock() {
        let mut tanks = tanks();
        tanks[0].name = "DAISY".into();
        for _ in 0..8 {
            let mut t = tanks[0].clone();
            t.ctrl = Ctrl::Ai(AiKind::Cyborg);
            tanks.push(t);
        }
        name_computers(&mut tanks);
        for (i, tank) in tanks.iter().enumerate() {
            assert!(tanks[..i]
                .iter()
                .all(|t| !t.name.eq_ignore_ascii_case(&tank.name)));
        }
        let tank = &mut tanks[2];
        tank.health = 70;
        tank.inventory.add(ItemId::Battery, 5);
        tank.inventory.add(ItemId::Shield, 2);
        tank.inventory.add(ItemId::SuperMag, 1);
        prepare(tank);
        assert_eq!(tank.health, 100);
        assert_eq!(tank.inventory.count(ItemId::Battery), 2);
        assert_eq!(
            tank.equipment.active_shield.as_ref().unwrap().item,
            ItemId::SuperMag
        );
        prepare(tank);
        assert_eq!(tank.inventory.count(ItemId::Shield), 2);
        assert_eq!(tank.inventory.count(ItemId::Battery), 2);
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
    fn wall_predictions_match_live_projectile_paths_for_every_fixed_wall() {
        use crate::{physics::FIXED_WALLS, settings::WallType};
        let tanks = tanks();
        let terrain = Terrain::flat(450);
        let baseline = sim(
            &terrain,
            &tanks,
            0,
            20,
            800,
            &Physics::new(&PhysicsSettings::default(), 0),
            false,
        );
        for wall in FIXED_WALLS {
            let mut settings = PhysicsSettings::default();
            settings.walls = wall;
            let physics = Physics::new(&settings, 0);
            let expected = sim(&terrain, &tanks, 0, 20, 800, &physics, false);
            let (pos, dir) = tanks[0].muzzle(20);
            let mut live = Proj {
                pos,
                vel: dir * 800.0,
                owner: 0,
                weapon: ItemId::BabyMissile,
                kind: PKind::Shell,
                radius: 10.0,
                scale: 1.0,
                damage: ItemId::BabyMissile.item().damage(),
                age: 0.0,
                tunneling: false,
                in_dirt: false,
                guidance: None,
            };
            let mut terrain = Terrain::flat(450);
            let mut endpoint = live.pos;
            for _ in 0..=(MAX_FLIGHT_TIME / SIM_DT) as usize {
                match weapons::step_proj(
                    &mut live,
                    SIM_DT,
                    &mut terrain,
                    &tanks,
                    &physics,
                    &mut Vec::new(),
                ) {
                    Step::Alive => {}
                    Step::Shield {
                        continues: true, ..
                    } => {}
                    Step::Shield {
                        continues: false, ..
                    } => {
                        endpoint = live.pos;
                        break;
                    }
                    Step::Hit(pos, _) | Step::Wall(pos) => {
                        endpoint = pos;
                        break;
                    }
                    Step::Gone | Step::Split => {
                        endpoint = live.pos;
                        break;
                    }
                }
            }
            assert!(endpoint.distance(expected) < 0.001, "{}", wall.label());
            if wall != WallType::None {
                assert!(
                    expected.distance(baseline) > 10.0,
                    "wall had no effect: {}",
                    wall.label()
                );
            }
        }
    }

    #[test]
    fn shield_prediction_uses_private_strength_and_matches_live_absorption() {
        let mut tanks = tanks();
        tanks[0].x = 100;
        tanks[0].y = 400;
        tanks[1].x = 300;
        tanks[1].y = 400;
        tanks[1].inventory.add(ItemId::Shield, 1);
        tanks[1].engage_shield(ItemId::Shield);
        let terrain = Terrain::flat(crate::gfx::H);
        let mut physics = Physics::new(&PhysicsSettings::default(), 0);
        physics.gravity = 0.0;
        let hit = sim(&terrain, &tanks, 0, 0, 500, &physics, false);
        assert!(hit.distance(tanks[1].center()) <= 15.1);
        assert!(hit.x < 293.0); // Intercepted before reaching the body's left edge.
        assert_eq!(tanks[1].shield_percent(), 100); // Predictor never damages live state.
        assert_eq!(sim(&terrain, &tanks, 0, 0, 500, &physics, false), hit);
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
        let plan = plan(AiKind::Spoiler, &terrain, &tanks, 0, &actual, true, 1.0);
        assert!((0..=180).contains(&plan.angle));
        assert!((0..=tanks[0].max_power()).contains(&plan.power));
        assert!(tanks[0].inventory.can_select(plan.weapon));
    }
}
