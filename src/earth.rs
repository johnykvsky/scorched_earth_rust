//! Animated earth weapons. Tunnels follow soil; no effect steers towards tanks.
use crate::economy::{DamageEvent, DamageTarget};
use crate::gfx::{Fb, Rgb, H, W};
use crate::physics::{Physics, WallContact, SIM_DT};
use crate::tank::Tank;
use crate::terrain::Terrain;
use crate::weapons::{Damage, Explosion, ItemId, ItemKind, Proj, WKind};
use macroquad::prelude::{vec2, IVec2, Vec2};
use macroquad::rand::gen_range;

const DIRECTIONS: [IVec2; 8] = [
    IVec2::new(1, 0),
    IVec2::new(1, 1),
    IVec2::new(0, 1),
    IVec2::new(-1, 1),
    IVec2::new(-1, 0),
    IVec2::new(-1, -1),
    IVec2::new(0, -1),
    IVec2::new(1, -1),
];
const HEAD_LIMIT: usize = 20;
const HEAD_LIFETIME: u16 = 120;

struct Head {
    pos: IVec2,
    direction: IVec2,
    age: u16,
    victim: Option<usize>,
}
struct Swarm {
    owner: usize,
    weapon: ItemId,
    scale: f32,
    heads: Vec<Head>,
    births_left: u16,
    ticks: u32,
    time: f32,
}
struct Grain {
    owner: usize,
    pos: Vec2,
    velocity: Vec2,
    age: f32,
}
struct Flash {
    owner: usize,
    pos: Vec2,
    radius: f32,
    wedge: Option<(f32, f32)>,
    age: f32,
}
#[derive(Default)]
pub struct Effects {
    swarms: Vec<Swarm>,
    grains: Vec<Grain>,
    grain_time: f32,
    flashes: Vec<Flash>,
    disruption: Vec<(usize, f32)>,
}
#[derive(Default)]
pub struct Update {
    pub damage: Vec<DamageEvent>,
    pub explosions: Vec<Explosion>,
    pub force_settle: bool,
}

pub fn is_earth(id: ItemId) -> bool {
    matches!(
        id.item().kind,
        ItemKind::Weapon(
            WKind::Riot | WKind::Wedge | WKind::Tunnel | WKind::DirtCharge | WKind::Disruptor
        )
    )
}
pub fn is_instant(id: ItemId) -> bool {
    matches!(
        id.item().kind,
        ItemKind::Weapon(WKind::Wedge | WKind::Disruptor)
    )
}
fn sandhog(id: ItemId) -> bool {
    matches!(
        id,
        ItemId::BabySandhog | ItemId::Sandhog | ItemId::HeavySandhog
    )
}
pub fn birth_budget(id: ItemId) -> u16 {
    match id {
        ItemId::BabyDigger => 20,
        ItemId::Digger => 40,
        ItemId::HeavyDigger => 70,
        ItemId::BabySandhog => 10,
        ItemId::Sandhog => 20,
        ItemId::HeavySandhog => 35,
        _ => 0,
    }
}
fn chip(tanks: &mut [Tank], owner: usize, victim: usize, result: &mut Update) {
    let t = &mut tanks[victim];
    let damage = t.health.clamp(0, 10);
    if damage == 0 {
        return;
    }
    t.health -= damage;
    t.power = t.power.min(t.max_power());
    t.last_hit = Some(owner);
    t.environmental_hit = false;
    result.damage.push(DamageEvent {
        owner,
        victim,
        amount: damage,
        target: DamageTarget::Tank,
    });
}
impl Effects {
    pub fn active(&self) -> bool {
        !self.swarms.is_empty()
            || !self.grains.is_empty()
            || !self.flashes.is_empty()
            || !self.disruption.is_empty()
    }

    pub fn active_for(&self, owner: usize) -> bool {
        self.swarms.iter().any(|s| s.owner == owner)
            || self.grains.iter().any(|g| g.owner == owner)
            || self.flashes.iter().any(|f| f.owner == owner)
            || self.disruption.iter().any(|(i, _)| *i == owner)
    }

    #[cfg(test)]
    pub fn instant(&mut self, id: ItemId, pos: Vec2, angle: f32, terrain: &mut Terrain) {
        self.instant_owned(0, id, pos, angle, terrain);
    }

    pub fn instant_owned(
        &mut self,
        owner: usize,
        id: ItemId,
        pos: Vec2,
        angle: f32,
        terrain: &mut Terrain,
    ) {
        if id == ItemId::EarthDisrupter {
            self.disruption.push((owner, 0.0));
            return;
        }
        let half_angle = if id == ItemId::RiotBlast { 60.0 } else { 45.0 };
        let radius = id.item().radius;
        terrain.carve_wedge(pos.x as i32, pos.y as i32, radius as i32, angle, half_angle);
        self.flashes.push(Flash {
            owner,
            pos,
            radius,
            wedge: Some((angle, half_angle)),
            age: 0.0,
        });
    }

    pub fn impact(
        &mut self,
        p: Proj,
        pos: Vec2,
        on_tank: Option<usize>,
        extra_dirt: bool,
        physics: &Physics,
        terrain: &mut Terrain,
        tanks: &mut [Tank],
    ) -> Update {
        let mut result = Update::default();
        match p.weapon.item().kind {
            ItemKind::Weapon(WKind::Riot) => {
                let supported = crate::tank::support_mask(tanks, terrain);
                terrain.carve(pos.x as i32, pos.y as i32, p.radius as i32);
                crate::tank::record_undermining(tanks, terrain, supported, Some(p.owner));
                self.flashes.push(Flash {
                    owner: p.owner,
                    pos,
                    radius: p.radius,
                    wedge: None,
                    age: 0.0,
                });
            }
            ItemKind::Weapon(WKind::Tunnel) => {
                if let Some(victim) = on_tank {
                    if !sandhog(p.weapon) {
                        return result;
                    }
                    chip(tanks, p.owner, victim, &mut result);
                }
                let direction = DIRECTIONS
                    .iter()
                    .copied()
                    .max_by(|a, b| {
                        a.as_vec2()
                            .normalize()
                            .dot(p.vel)
                            .total_cmp(&b.as_vec2().normalize().dot(p.vel))
                    })
                    .unwrap();
                self.swarms.push(Swarm {
                    owner: p.owner,
                    weapon: p.weapon,
                    scale: p.scale,
                    heads: vec![Head {
                        pos: pos.as_ivec2(),
                        direction,
                        age: 0,
                        victim: on_tank,
                    }],
                    births_left: birth_budget(p.weapon) - 1,
                    ticks: 0,
                    time: 0.0,
                });
            }
            ItemKind::Weapon(WKind::DirtCharge) => {
                // The original's exact particle count/Extra Dirt multiplier is unknown.
                // Four times the grains is an explicit, bounded local policy.
                for _ in 0..if extra_dirt { 320 } else { 80 } {
                    let height = gen_range(H as f32 / 12.0, H as f32 / 3.0);
                    let upward = (2.0 * physics.gravity * height).sqrt();
                    self.grains.push(Grain {
                        owner: p.owner,
                        pos: pos - vec2(0.0, 1.0),
                        velocity: vec2(gen_range(-0.65, 0.65) * upward, -upward),
                        age: 0.0,
                    });
                }
            }
            _ => {}
        }
        result
    }

    pub fn update(
        &mut self,
        dt: f32,
        physics: &Physics,
        terrain: &mut Terrain,
        tanks: &mut [Tank],
    ) -> Update {
        let mut result = Update::default();
        for flash in &mut self.flashes {
            flash.age += dt;
        }
        self.flashes.retain(|flash| flash.age < 0.5);
        for (_, time) in &mut self.disruption {
            *time += dt;
            if *time * 120.0 >= 100.0 {
                terrain.disturb_all();
                result.force_settle = true;
            }
        }
        self.disruption.retain(|(_, time)| *time * 120.0 < 100.0);
        for swarm in &mut self.swarms {
            swarm.time += dt * 120.0;
            while swarm.time >= 1.0 && !swarm.heads.is_empty() {
                swarm.time -= 1.0;
                let supported = crate::tank::support_mask(tanks, terrain);
                swarm.tick(terrain, tanks, &mut result);
                crate::tank::record_undermining(tanks, terrain, supported, Some(swarm.owner));
            }
        }
        self.swarms.retain(|swarm| !swarm.heads.is_empty());
        self.grain_time += dt;
        while self.grain_time >= SIM_DT && !self.grains.is_empty() {
            self.grain_time -= SIM_DT;
            self.grains.retain_mut(|grain| grain.step(physics, terrain));
        }
        if self.grains.is_empty() {
            self.grain_time = 0.0;
        }
        result
    }

    pub fn draw(&self, fb: &mut Fb) {
        for swarm in &self.swarms {
            let color = if sandhog(swarm.weapon) {
                Rgb(255, 180, 40)
            } else {
                Rgb(120, 220, 255)
            };
            for head in &swarm.heads {
                fb.rect(head.pos.x - 1, head.pos.y - 1, 3, 3, color);
            }
        }
        for grain in &self.grains {
            fb.rect(
                grain.pos.x as i32,
                grain.pos.y as i32,
                2,
                2,
                Rgb(170, 110, 50),
            );
        }
        for flash in &self.flashes {
            let radius = flash.radius * (flash.age / 0.2).min(1.0);
            let color = if (flash.age * 30.0) as i32 % 2 == 0 {
                Rgb::WHITE
            } else {
                Rgb(100, 200, 255)
            };
            let (angle, half) = flash.wedge.unwrap_or((0.0, 180.0));
            let mut previous = None;
            for i in 0..=60 {
                let a = (angle - half + 2.0 * half * i as f32 / 60.0).to_radians();
                let point = flash.pos + vec2(a.cos(), -a.sin()) * radius;
                if let Some((x, y)) = previous {
                    fb.line(x, y, point.x as i32, point.y as i32, color);
                }
                previous = Some((point.x as i32, point.y as i32));
            }
            if flash.wedge.is_some() {
                for a in [angle - half, angle + half] {
                    let a = a.to_radians();
                    let edge = flash.pos + vec2(a.cos(), -a.sin()) * radius;
                    fb.line(
                        flash.pos.x as i32,
                        flash.pos.y as i32,
                        edge.x as i32,
                        edge.y as i32,
                        color,
                    );
                }
            }
        }
        for &(_, time) in &self.disruption {
            if (time * 120.0) as i32 % 2 == 0 {
                for y in (crate::ui::HUD_H..H).step_by(8) {
                    fb.line(0, y, W - 1, y, Rgb(140, 140, 200));
                }
            }
        }
    }
}

impl Swarm {
    fn terminal(&self, head: &Head, result: &mut Update) {
        if sandhog(self.weapon) {
            result.explosions.push(Explosion::new(
                head.pos.as_vec2(),
                10.0 * self.scale,
                Damage::Sandhog(self.weapon as usize),
                self.owner,
                false,
            ));
        }
    }
    fn tick(&mut self, terrain: &mut Terrain, tanks: &mut [Tank], result: &mut Update) {
        self.ticks += 1;
        let mut next = Vec::new();
        let mut branches = Vec::new();
        let heads = std::mem::take(&mut self.heads);
        let initial = heads.len();
        for (index, mut head) in heads.into_iter().enumerate() {
            if head.age >= HEAD_LIFETIME {
                self.terminal(&head, result);
                continue;
            }
            let heading = if head.age > 0 && head.age % 20 == 0 {
                DIRECTIONS[gen_range(0, 8)]
            } else {
                head.direction
            };
            let candidates: Vec<IVec2> = DIRECTIONS
                .iter()
                .copied()
                .filter(|direction| {
                    let target = head.pos + *direction * 2;
                    (0..W).contains(&target.x)
                        && (0..H).contains(&target.y)
                        && (terrain.solid(target.x, target.y)
                            || terrain.solid(head.pos.x + direction.x, head.pos.y + direction.y))
                })
                .collect();
            if candidates.is_empty() {
                self.terminal(&head, result);
                continue;
            }
            let direction = *candidates
                .iter()
                .max_by_key(|d| d.dot(heading) * 4 + gen_range(0, 7))
                .unwrap();
            if self.ticks % 8 == 0
                && self.births_left > 0
                && next.len() + branches.len() + initial - index < HEAD_LIMIT
            {
                let branch = candidates[gen_range(0, candidates.len())];
                branches.push(Head {
                    pos: head.pos,
                    direction: branch,
                    age: 0,
                    victim: None,
                });
                self.births_left -= 1;
            }
            let mut stopped = false;
            for _ in 0..2 {
                head.pos += direction;
                terrain.carve(head.pos.x, head.pos.y, 1);
                if let Some(victim) = tanks
                    .iter()
                    .enumerate()
                    .find(|(_, tank)| {
                        tank.alive && tank.health > 0 && tank.hit(head.pos.x, head.pos.y)
                    })
                    .map(|(i, _)| i)
                {
                    if !sandhog(self.weapon) {
                        stopped = true;
                        break;
                    }
                    if head.victim != Some(victim) {
                        chip(tanks, self.owner, victim, result);
                        head.victim = Some(victim);
                    }
                } else {
                    head.victim = None;
                }
            }
            head.direction = direction;
            head.age += 1;
            if !stopped {
                next.push(head);
            }
        }
        next.extend(
            branches
                .into_iter()
                .take(HEAD_LIMIT.saturating_sub(next.len())),
        );
        self.heads = next;
    }
}

impl Grain {
    fn step(&mut self, physics: &Physics, terrain: &mut Terrain) -> bool {
        self.age += SIM_DT;
        // Bound reflected grains too: repeated Spring hits must not create
        // unbounded speeds or ever-growing work inside one animation step.
        let limit = ((2.0 * physics.gravity * H as f32 / 3.0).sqrt() * 1.5).max(1000.0);
        let mut remaining = SIM_DT;
        while remaining > 0.0 {
            if self.velocity.length_squared() > limit * limit {
                self.velocity = self.velocity.normalize() * limit;
            }
            let step = remaining.min(1.0 / self.velocity.abs().max_element().max(1.0));
            remaining = (remaining - step).max(0.0);
            let previous = self.pos;
            self.pos += self.velocity * step;
            if physics.wall_contact(&mut self.pos, &mut self.velocity) == WallContact::Blocked {
                terrain.add_dirt(
                    self.pos.x as i32,
                    self.pos.y as i32,
                    2,
                    physics.ceiling as i32,
                );
                return false;
            }
            if self.pos.x < 0.0
                || self.pos.x >= W as f32
                || self.pos.y >= H as f32
                || self.age > 8.0
            {
                return false;
            }
            if self.pos.y >= physics.ceiling && terrain.solid(self.pos.x as i32, self.pos.y as i32)
            {
                terrain.add_dirt(
                    previous.x as i32,
                    previous.y as i32,
                    2,
                    physics.ceiling as i32,
                );
                return false;
            }
        }
        physics.air_forces(&mut self.velocity, SIM_DT, SIM_DT);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{settings::Settings, shop::tests::tank, weapons::PKind};

    fn shot(id: ItemId, pos: Vec2) -> Proj {
        Proj {
            pos,
            vel: vec2(0.0, 300.0),
            owner: 0,
            weapon: id,
            kind: PKind::Shell,
            radius: id.item().radius,
            scale: 1.0,
            damage: id.item().damage(),
            age: 1.0,
            tunneling: false,
            in_dirt: false,
            guidance: None,
        }
    }
    fn physics() -> Physics {
        Physics::new(&Settings::default().physics, 0)
    }
    fn solid_count(terrain: &Terrain) -> usize {
        (0..H)
            .flat_map(|y| (0..W).map(move |x| (x, y)))
            .filter(|&(x, y)| terrain.solid(x, y))
            .count()
    }

    #[test]
    fn riot_wedges_use_the_barrel_direction_and_distinct_angles_without_blast_damage() {
        for (id, radius) in [(ItemId::RiotCharge, 36), (ItemId::RiotBlast, 60)] {
            let mut terrain = Terrain::flat(0);
            let mut effects = Effects::default();
            effects.instant(id, vec2(200.0, 200.0), 90.0, &mut terrain);
            assert!(!terrain.solid(200, 200 - radius + 1));
            assert!(terrain.solid(200, 201));
            assert!(terrain.solid(200 + radius + 1, 200));
            assert!(effects.active());
            let update = effects.update(1.0, &physics(), &mut terrain, &mut [tank(0)]);
            assert!(update.damage.is_empty() && update.explosions.is_empty());
            assert!(!effects.active());
        }
        let mut narrow = Terrain::flat(0);
        let mut wide = Terrain::flat(0);
        narrow.carve_wedge(200, 200, 60, 90.0, 45.0);
        wide.carve_wedge(200, 200, 60, 90.0, 60.0);
        assert!(narrow.solid(225, 180));
        assert!(!wide.solid(225, 180));
        assert!(solid_count(&wide) < solid_count(&narrow));
    }

    #[test]
    fn riot_bombs_only_remove_their_circle_even_on_direct_tank_contact() {
        for (id, radius) in [(ItemId::RiotBomb, 30), (ItemId::HeavyRiotBomb, 45)] {
            let mut terrain = Terrain::flat(150);
            let mut tanks = vec![tank(0)];
            let mut effects = Effects::default();
            let p = shot(id, vec2(200.0, 200.0));
            let result = effects.impact(
                p,
                p.pos,
                Some(0),
                false,
                &physics(),
                &mut terrain,
                &mut tanks,
            );
            assert_eq!(tanks[0].health, 100);
            assert!(result.damage.is_empty() && result.explosions.is_empty());
            assert!(!terrain.solid(200 + radius, 200));
            assert!(terrain.solid(200 + radius + 1, 200));
        }
    }

    #[test]
    fn diggers_fizzle_on_tanks_and_tunnel_only_in_soil_with_bounded_births_and_heads() {
        for id in [ItemId::BabyDigger, ItemId::Digger, ItemId::HeavyDigger] {
            let mut effects = Effects::default();
            let mut terrain = Terrain::flat(100);
            let mut tanks = vec![tank(0)];
            let p = shot(id, vec2(300.0, 200.0));
            let result = effects.impact(
                p,
                p.pos,
                Some(0),
                false,
                &physics(),
                &mut terrain,
                &mut tanks,
            );
            assert!(!effects.active());
            assert!(result.damage.is_empty() && result.explosions.is_empty());
            assert_eq!(tanks[0].health, 100);
            effects.impact(p, p.pos, None, false, &physics(), &mut terrain, &mut tanks);
            assert_eq!(effects.swarms[0].births_left + 1, birth_budget(id));
            let before = solid_count(&terrain);
            let mut remaining = birth_budget(id) - 1;
            for _ in 0..10000 {
                let update = effects.update(1.0 / 120.0, &physics(), &mut terrain, &mut tanks);
                assert!(update.damage.is_empty() && update.explosions.is_empty());
                for swarm in &effects.swarms {
                    assert!(swarm.heads.len() <= HEAD_LIMIT);
                    assert!(swarm.births_left <= remaining);
                    remaining = swarm.births_left;
                }
                if !effects.active() {
                    break;
                }
            }
            assert!(!effects.active());
            assert!(solid_count(&terrain) < before);
            assert_eq!(tanks[0].health, 100);
        }
    }

    #[test]
    fn sandhog_end_charges_scale_and_direct_contact_chips_ten_instead_of_killing() {
        for id in [ItemId::BabySandhog, ItemId::Sandhog, ItemId::HeavySandhog] {
            let mut effects = Effects::default();
            let mut terrain = Terrain::flat(H);
            let mut tanks = vec![tank(0)];
            let mut p = shot(id, vec2(300.0, 200.0));
            p.scale = 3.0;
            let result = effects.impact(
                p,
                p.pos,
                Some(0),
                false,
                &physics(),
                &mut terrain,
                &mut tanks,
            );
            assert_eq!(tanks[0].health, 90);
            assert_eq!(result.damage.len(), 1);
            assert_eq!(result.damage[0].amount, 10);
            let result = effects.update(1.0 / 120.0, &physics(), &mut terrain, &mut tanks);
            assert_eq!(result.explosions.len(), 1);
            assert_eq!(result.explosions[0].radius, 30.0);
            assert!(
                matches!(result.explosions[0].damage, Damage::Sandhog(index) if index == id as usize)
            );
            assert!(!effects.active());
        }
    }

    #[test]
    fn dirt_charge_extra_dirt_quadruples_grains_and_deposits_soil_without_harming_tanks() {
        for (extra, count) in [(false, 80), (true, 320)] {
            let mut effects = Effects::default();
            let mut terrain = Terrain::flat(400);
            let mut tanks = vec![tank(0)];
            let p = shot(ItemId::DirtCharge, vec2(320.0, 399.0));
            let result = effects.impact(
                p,
                p.pos,
                Some(0),
                extra,
                &physics(),
                &mut terrain,
                &mut tanks,
            );
            assert_eq!(effects.grains.len(), count);
            for grain in &effects.grains {
                let height = grain.velocity.y.powi(2) / (2.0 * physics().gravity);
                assert!(height >= H as f32 / 12.0 - 0.01 && height <= H as f32 / 3.0 + 0.01);
                assert!(grain.velocity.y < 0.0);
            }
            assert!(result.damage.is_empty());
            let before = solid_count(&terrain);
            let update = effects.update(10.0, &physics(), &mut terrain, &mut tanks);
            assert!(update.damage.is_empty() && update.explosions.is_empty());
            assert!(!effects.active());
            assert!(solid_count(&terrain) > before);
            assert_eq!(tanks[0].health, 100);
        }
    }

    #[test]
    fn disrupter_waits_for_a_hundred_flash_ticks_then_forces_settling() {
        let mut terrain = Terrain::flat(400);
        terrain.add_dirt(200, 200, 3, 0);
        let mut effects = Effects::default();
        effects.instant(ItemId::EarthDisrupter, vec2(0.0, 0.0), 0.0, &mut terrain);
        let mut tanks = vec![tank(0)];
        assert!(
            !effects
                .update(0.5, &physics(), &mut terrain, &mut tanks)
                .force_settle
        );
        assert!(effects.active());
        let update = effects.update(0.4, &physics(), &mut terrain, &mut tanks);
        assert!(update.force_settle);
        assert!(update.damage.is_empty() && update.explosions.is_empty());
        assert!(terrain.settle_step());
        assert!(!effects.active());
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    use crate::{
        settings::{Settings, WallType},
        shop::tests::tank,
        weapons::PKind,
    };
    #[test]
    fn dirt_spray_finishes_under_every_wall_and_extreme_physics_without_damage() {
        for wall in crate::physics::FIXED_WALLS {
            let mut settings = Settings::default();
            settings.physics.walls = wall;
            settings.physics.gravity = 10.0;
            let physics = Physics::new(&settings.physics, 500);
            let mut terrain = Terrain::flat(450);
            let mut tanks = vec![tank(0)];
            let mut effects = Effects::default();
            let p = Proj {
                pos: vec2(5.0, 440.0),
                vel: vec2(0.0, 0.0),
                owner: 0,
                weapon: ItemId::DirtCharge,
                kind: PKind::Shell,
                radius: 0.0,
                scale: 1.0,
                damage: ItemId::DirtCharge.item().damage(),
                age: 0.0,
                tunneling: false,
                in_dirt: false,
                guidance: None,
            };
            effects.impact(p, p.pos, None, true, &physics, &mut terrain, &mut tanks);
            let result = effects.update(10.0, &physics, &mut terrain, &mut tanks);
            assert!(!effects.active(), "{}", wall.label());
            assert!(result.damage.is_empty() && result.explosions.is_empty());
            assert_eq!(tanks[0].health, 100);
            assert!(wall != WallType::Random && wall != WallType::Erratic);
        }
    }
}
