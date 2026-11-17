//! Bounded shared fluid simulation for napalm and terrain-producing Liquid Dirt.
//! Recovered bounds live in MECHANICS §7.2; local tuning is recorded in TODO batch 11.
use crate::{
    economy::{DamageEvent, DamageTarget},
    gfx::{Fb, Rgb, H, W},
    settings::WallType,
    tank::Tank,
    terrain::Terrain,
    weapons::{ItemId, Proj},
};
use macroquad::prelude::Vec2;
use std::collections::{BTreeMap, VecDeque};

const STEP: f32 = 1.0 / 120.0;
const QUEUE_LIMIT: usize = 100;
const POOL_LIMIT: usize = crate::settings::MAX_PLAYERS;
const TRANSPORT_TICKS: u16 = 1200;
const GROW_CYCLES: u8 = 8;
const BURN_CYCLES: u8 = 50;
type Point = (i32, i32);

struct Emitter {
    origin: Point,
    remaining: u16,
}
struct Pool {
    owner: usize,
    weapon: ItemId,
    emitters: Vec<Emitter>,
    cells: BTreeMap<Point, usize>,
    queue: VecDeque<Point>,
    ticks: u16,
    burn: Option<u8>,
    done: bool,
}
#[derive(Default)]
pub struct Effects {
    pools: Vec<Pool>,
    time: f32,
}
#[derive(Default)]
pub struct Update {
    pub damage: Vec<DamageEvent>,
    pub burning: bool,
    pub depositing: bool,
}

pub fn is_liquid(id: ItemId) -> bool {
    matches!(id, ItemId::Napalm | ItemId::HotNapalm | ItemId::LiquidDirt)
}

impl Effects {
    pub fn active_for(&self, owner: usize) -> bool {
        self.pools.iter().any(|p| p.owner == owner)
    }
    pub fn active(&self) -> bool {
        !self.pools.is_empty()
    }

    /// A napalm carrier that stops underground fizzles. A contact-trigger shot
    /// never enters dirt; crossing a thin layer and emerging can still succeed.
    pub fn impact(&mut self, p: &Proj, pos: Vec2, top: i32) -> bool {
        if !is_liquid(p.weapon)
            || self.pools.len() >= POOL_LIMIT
            || (p.weapon != ItemId::LiquidDirt && p.tunneling && p.in_dirt)
            || !(0.0..W as f32).contains(&pos.x)
            || !(top as f32..H as f32).contains(&pos.y)
        {
            return false;
        }
        let count = if p.weapon == ItemId::Napalm { 15 } else { 20 };
        let volume = match p.weapon {
            ItemId::Napalm => 32,
            ItemId::HotNapalm => 48,
            _ => 64,
        };
        let emitters = (0..count)
            .map(|i| Emitter {
                origin: (
                    (pos.x as i32 + 2 * i - count + 1).clamp(0, W - 1),
                    (pos.y as i32 - 2).max(top),
                ),
                remaining: volume,
            })
            .collect();
        self.pools.push(Pool {
            owner: p.owner,
            weapon: p.weapon,
            emitters,
            cells: BTreeMap::new(),
            queue: VecDeque::new(),
            ticks: 0,
            burn: None,
            done: false,
        });
        true
    }

    pub fn update(
        &mut self,
        dt: f32,
        terrain: &mut Terrain,
        tanks: &mut [Tank],
        top: i32,
        wall: WallType,
    ) -> Update {
        let mut result = Update::default();
        if !self.active() {
            self.time = 0.0;
            return result;
        }
        self.time += dt;
        while self.time >= STEP && self.active() {
            self.time -= STEP;
            let mut queued: usize = self.pools.iter().map(|p| p.queue.len()).sum();
            for pool in &mut self.pools {
                let old = pool.queue.len();
                pool.tick(
                    terrain,
                    tanks,
                    top,
                    wall,
                    QUEUE_LIMIT - queued + old,
                    &mut result,
                );
                queued = queued - old + pool.queue.len();
            }
            self.pools.retain(|p| !p.done);
        }
        if !self.active() {
            self.time = 0.0;
        }
        result
    }

    pub fn draw(&self, fb: &mut Fb) {
        for pool in &self.pools {
            let color = match pool.weapon {
                ItemId::LiquidDirt => Rgb(180, 130, 65),
                ItemId::HotNapalm => Rgb(255, 90, 10),
                _ => Rgb(210, 45, 10),
            };
            for &(x, y) in pool.cells.keys() {
                let shimmer = pool.weapon != ItemId::LiquidDirt
                    && (x + y + i32::from(pool.ticks) / 4).rem_euclid(7) < 2;
                fb.put(x, y, if shimmer { Rgb(255, 160, 20) } else { color });
            }
            if let Some(cycle) = pool.burn {
                for source in 0..pool.emitters.len() {
                    if let Some((base, depth)) = pool.flame(source) {
                        let height = pool.flame_height(depth)
                            * i32::from((cycle + 1).min(GROW_CYCLES))
                            / i32::from(GROW_CYCLES);
                        let surface = base.1 - depth + 1;
                        for dy in 0..height {
                            let sway = ((pool.ticks as i32 / 3 + dy + source as i32) % 5) - 2;
                            fb.line(
                                base.0 + sway - 1,
                                surface - dy,
                                base.0 + sway + 1,
                                surface - dy,
                                if dy < height / 2 {
                                    Rgb(255, 230, 60)
                                } else {
                                    color
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}

impl Pool {
    fn neighbor(&self, (x, y): Point, dx: i32, dy: i32, top: i32, wall: WallType) -> Option<Point> {
        let mut x = x + dx;
        if !(0..W).contains(&x) {
            if wall == WallType::WrapAround {
                x = x.rem_euclid(W);
            } else {
                return None;
            }
        }
        let y = y + dy;
        (top..H).contains(&y).then_some((x, y))
    }
    fn free(&self, p: Point, terrain: &Terrain) -> bool {
        !terrain.solid(p.0, p.1) && !self.cells.contains_key(&p)
    }
    fn flow(&self, point: Point, terrain: &Terrain, top: i32, wall: WallType) -> Option<Point> {
        let side = if (self.ticks as i32 + point.0) % 2 == 0 {
            1
        } else {
            -1
        };
        for dx in [0, side, -side] {
            if let Some(p) = self
                .neighbor(point, dx, 1, top, wall)
                .filter(|p| self.free(*p, terrain))
            {
                return Some(p);
            }
        }
        // A short level run can carry fluid to a lower ledge, without endless
        // sideways oscillation on a flat, fully supported surface.
        for direction in [side, -side] {
            for distance in 1..=8 {
                let Some(p) = self.neighbor(point, direction * distance, 0, top, wall) else {
                    break;
                };
                if !self.free(p, terrain) {
                    break;
                }
                if self
                    .neighbor(p, 0, 1, top, wall)
                    .is_some_and(|below| self.free(below, terrain))
                {
                    return self.neighbor(point, direction, 0, top, wall);
                }
            }
        }
        None
    }
    fn enqueue(&mut self, point: Point, limit: usize) {
        if self.queue.len() < limit && !self.queue.contains(&point) {
            self.queue.push_back(point);
        }
    }
    fn tick(
        &mut self,
        terrain: &mut Terrain,
        tanks: &mut [Tank],
        top: i32,
        wall: WallType,
        limit: usize,
        result: &mut Update,
    ) {
        self.ticks += 1;
        if let Some(cycle) = self.burn {
            if self.ticks.is_multiple_of(6) {
                if cycle >= GROW_CYCLES {
                    self.heat(tanks, result);
                    result.burning = true;
                }
                let next = cycle + 1;
                self.done = next >= GROW_CYCLES + BURN_CYCLES;
                self.burn = Some(next);
            }
            return;
        }
        let count = self.queue.len();
        for _ in 0..count {
            let point = self.queue.pop_front().unwrap();
            let Some(source) = self.cells.get(&point).copied() else {
                continue;
            };
            if terrain.solid(point.0, point.1) {
                self.cells.remove(&point);
                continue;
            }
            if let Some(next) = self.flow(point, terrain, top, wall) {
                self.cells.remove(&point);
                self.cells.insert(next, source);
                self.enqueue(next, limit);
            } else if self.weapon == ItemId::LiquidDirt {
                terrain.add_dirt(point.0, point.1, 0, top);
                self.cells.remove(&point);
                result.depositing = true;
            }
        }
        if self.ticks.is_multiple_of(4) {
            for source in 0..self.emitters.len() {
                if self.queue.len() >= limit {
                    break;
                }
                if self.emitters[source].remaining == 0 {
                    continue;
                }
                let (x, mut y) = self.emitters[source].origin;
                while y >= top && !self.free((x, y), terrain) {
                    y -= 1;
                }
                self.emitters[source].remaining -= 1;
                if y >= top {
                    self.cells.insert((x, y), source);
                    self.enqueue((x, y), limit);
                }
            }
        }
        if self.ticks.is_multiple_of(6) {
            let unsettled: Vec<_> = self
                .cells
                .keys()
                .copied()
                .filter(|point| self.flow(*point, terrain, top, wall).is_some())
                .take(limit)
                .collect();
            for point in unsettled {
                self.enqueue(point, limit);
            }
        }
        let emitted = self.emitters.iter().all(|e| e.remaining == 0);
        if (emitted && self.queue.is_empty()) || self.ticks >= TRANSPORT_TICKS {
            self.queue.clear();
            if self.weapon == ItemId::LiquidDirt || self.cells.is_empty() {
                self.done = true;
            } else {
                self.burn = Some(0);
            }
        }
    }
    fn flame(&self, source: usize) -> Option<(Point, i32)> {
        let base = self
            .cells
            .iter()
            .filter(|(_, emitter)| **emitter == source)
            .map(|(p, _)| *p)
            .max_by_key(|p| p.1)?;
        let mut depth = 1;
        while self.cells.contains_key(&(base.0, base.1 - depth)) {
            depth += 1;
        }
        Some((base, depth))
    }
    fn flame_height(&self, depth: i32) -> i32 {
        6 + depth.min(24)
            + if self.weapon == ItemId::HotNapalm {
                4
            } else {
                0
            }
    }
    fn heat(&self, tanks: &mut [Tank], result: &mut Update) {
        for (victim, tank) in tanks.iter_mut().enumerate() {
            if !tank.alive || tank.health <= 0 {
                continue;
            }
            let mut dose = 0;
            for source in 0..self.emitters.len() {
                if let Some((base, depth)) = self.flame(source) {
                    let surface = base.1 - depth + 1;
                    if (tank.x - base.0).abs() <= 5 + depth.min(24) / 2
                        && tank.y >= surface - self.flame_height(depth)
                        && tank.y - 10 <= base.1 + 2
                    {
                        dose += ((depth + 3) / 4).min(6)
                            * if self.weapon == ItemId::HotNapalm {
                                2
                            } else {
                                1
                            };
                    }
                }
            }
            let absorbed = if victim == self.owner {
                0
            } else {
                tank.drain_shield(dose)
            };
            if absorbed > 0 {
                result.damage.push(DamageEvent {
                    owner: self.owner,
                    victim,
                    amount: absorbed,
                    target: DamageTarget::Shield,
                });
            }
            let damage = (dose - absorbed).min(tank.health);
            if damage > 0 {
                tank.health -= damage;
                tank.power = tank.power.min(tank.max_power());
                tank.last_hit = Some(self.owner);
                tank.environmental_hit = false;
                result.damage.push(DamageEvent {
                    owner: self.owner,
                    victim,
                    amount: damage,
                    target: DamageTarget::Tank,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapons::PKind;
    use macroquad::prelude::vec2;

    pub(crate) fn pit() -> Terrain {
        let mut terrain = Terrain::flat(300);
        for x in 300..340 {
            for y in 300..330 {
                terrain.carve(x, y, 0);
            }
        }
        terrain
    }
    fn shot(weapon: ItemId) -> Proj {
        Proj {
            pos: vec2(320.0, 299.0),
            vel: vec2(100.0, 0.0),
            owner: 0,
            weapon,
            kind: PKind::Shell,
            radius: 0.0,
            scale: 1.0,
            damage: weapon.item().damage(),
            age: 0.0,
            tunneling: false,
            in_dirt: false,
            guidance: None,
        }
    }
    fn tanks() -> Vec<Tank> {
        [100, 320]
            .into_iter()
            .map(|x| {
                let mut tank = crate::shop::tests::tank(0);
                tank.x = x;
                tank.y = if x == 320 { 329 } else { 299 };
                tank
            })
            .collect()
    }
    fn layer(weapon: ItemId, depth: i32, sources: usize) -> Pool {
        Pool {
            owner: 0,
            weapon,
            emitters: (0..sources)
                .map(|_| Emitter {
                    origin: (320, 299),
                    remaining: 0,
                })
                .collect(),
            cells: (0..depth)
                .map(|d| ((320, 329 - d), d as usize % sources))
                .collect(),
            queue: VecDeque::new(),
            ticks: 0,
            burn: Some(GROW_CYCLES),
            done: false,
        }
    }

    #[test]
    fn napalm_flows_into_a_valley_builds_depth_and_burns_for_fifty_cycles() {
        let mut effects = Effects::default();
        let p = shot(ItemId::Napalm);
        assert!(effects.impact(&p, p.pos, 22));
        let mut terrain = pit();
        let mut tanks = tanks();
        let mut depth = 0;
        let mut cycles = 0;
        let mut damage = 0;
        for _ in 0..1800 {
            if !effects.active() {
                break;
            }
            let result = effects.update(STEP, &mut terrain, &mut tanks, 22, WallType::None);
            cycles += usize::from(result.burning);
            damage += result
                .damage
                .iter()
                .filter(|e| e.victim == 1 && e.target == DamageTarget::Tank)
                .map(|e| e.amount)
                .sum::<i32>();
            for pool in &effects.pools {
                assert!(pool.queue.len() <= QUEUE_LIMIT);
                assert!(pool.cells.len() <= 15 * 32);
                for source in 0..15 {
                    if let Some((_, d)) = pool.flame(source) {
                        depth = depth.max(d);
                    }
                }
            }
        }
        assert!(!effects.active());
        assert!(depth >= 4, "pool depth {depth}");
        assert_eq!(cycles, 50);
        assert_eq!(tanks[1].health, 0);
        assert_eq!(damage, 100);
        assert_eq!(tanks[1].last_hit, Some(0));
        assert!(terrain.solid(320, 330));
        assert!(!terrain.solid(320, 329)); // Fire never produces a crater or dirt.
    }

    #[test]
    fn depth_emitters_and_hot_napalm_each_increase_heat() {
        let dose = |weapon, depth, sources| {
            let mut tanks = tanks();
            let mut result = Update::default();
            layer(weapon, depth, sources).heat(&mut tanks, &mut result);
            100 - tanks[1].health
        };
        assert!(dose(ItemId::Napalm, 12, 1) > dose(ItemId::Napalm, 1, 1));
        assert!(dose(ItemId::Napalm, 12, 3) > dose(ItemId::Napalm, 12, 1));
        assert!(dose(ItemId::HotNapalm, 12, 1) > dose(ItemId::Napalm, 12, 1));
        let mut effects = Effects::default();
        let p = shot(ItemId::HotNapalm);
        effects.impact(&p, p.pos, 22);
        assert_eq!(effects.pools[0].emitters.len(), 20);
        assert_eq!(
            effects.pools[0]
                .emitters
                .iter()
                .map(|e| e.remaining)
                .sum::<u16>(),
            960
        );
    }

    #[test]
    fn owner_heat_bypasses_own_shield_while_enemy_shields_absorb_and_overflow() {
        let pool = layer(ItemId::Napalm, 12, 1);
        let mut tanks = tanks();
        tanks[0].x = 320;
        tanks[0].y = 329;
        for tank in &mut tanks {
            let mut shield = crate::shields::ActiveShield::new(ItemId::Shield).unwrap();
            shield.strength = 2;
            tank.equipment.active_shield = Some(shield);
        }
        let mut result = Update::default();
        pool.heat(&mut tanks, &mut result);
        assert_eq!(tanks[0].health, 97);
        assert_eq!(
            tanks[0].equipment.active_shield.as_ref().unwrap().strength,
            2
        );
        assert_eq!(tanks[1].health, 99);
        assert!(tanks[1].equipment.active_shield.is_none());
        assert!(result
            .damage
            .iter()
            .any(|e| e.victim == 1 && e.target == DamageTarget::Shield && e.amount == 2));
        assert!(result
            .damage
            .iter()
            .all(|e| !(e.victim == 0 && e.target == DamageTarget::Shield)));
    }

    #[test]
    fn liquid_dirt_fills_a_hole_without_damage_or_lifting_tanks() {
        let mut effects = Effects::default();
        let p = shot(ItemId::LiquidDirt);
        effects.impact(&p, p.pos, 22);
        let mut terrain = pit();
        let mut tanks = tanks();
        let mut deposited = false;
        for _ in 0..1800 {
            if !effects.active() {
                break;
            }
            let result = effects.update(STEP, &mut terrain, &mut tanks, 22, WallType::None);
            deposited |= result.depositing;
            assert!(result.damage.is_empty());
        }
        assert!(!effects.active());
        assert!(deposited);
        assert!(terrain.solid(320, 329));
        assert!(terrain.solid(320, 320));
        assert!(tanks.iter().all(|t| t.health == 100));
        assert_eq!(tanks[1].y, 329);
    }

    #[test]
    fn buried_napalm_fizzles_but_contact_and_emerged_carriers_do_not() {
        for id in [ItemId::Napalm, ItemId::HotNapalm] {
            let mut p = shot(id);
            p.tunneling = true;
            p.in_dirt = true;
            let mut effects = Effects::default();
            assert!(!effects.impact(&p, p.pos, 22));
            assert!(!effects.active());
            p.tunneling = false;
            p.in_dirt = false;
            assert!(effects.impact(&p, p.pos, 22));
            p.tunneling = true;
            assert!(effects.impact(&p, p.pos, 22));
        }
    }

    #[test]
    fn all_walls_and_open_bottom_have_finite_bounded_liquid_lifecycles() {
        for wall in crate::physics::FIXED_WALLS {
            let mut effects = Effects::default();
            for i in 0..POOL_LIMIT {
                let mut p = shot(if i % 2 == 0 {
                    ItemId::Napalm
                } else {
                    ItemId::LiquidDirt
                });
                p.pos = vec2(if i % 2 == 0 { 0.0 } else { (W - 1) as f32 }, 250.0);
                assert!(effects.impact(&p, p.pos, 44));
            }
            assert!(!effects.impact(&shot(ItemId::Napalm), vec2(100.0, 200.0), 44));
            let mut terrain = Terrain::flat(H);
            let mut tanks = tanks();
            for _ in 0..1800 {
                if !effects.active() {
                    break;
                }
                effects.update(STEP, &mut terrain, &mut tanks, 44, wall);
                assert!(effects.pools.iter().map(|p| p.queue.len()).sum::<usize>() <= QUEUE_LIMIT);
                assert!(effects
                    .pools
                    .iter()
                    .flat_map(|p| p.cells.keys())
                    .all(|p| (0..W).contains(&p.0) && (44..H).contains(&p.1)));
            }
            assert!(!effects.active());
        }
    }

    #[test]
    fn fluid_fixed_steps_match_across_frame_sizes_and_fast_forward_finishes() {
        let run = |dt, frames| {
            let mut effects = Effects::default();
            let p = shot(ItemId::Napalm);
            effects.impact(&p, p.pos, 22);
            let mut terrain = pit();
            let mut tanks = tanks();
            for _ in 0..frames {
                effects.update(dt, &mut terrain, &mut tanks, 22, WallType::None);
            }
            effects
        };
        let slow = run(STEP, 120);
        let fast = run(STEP * 4.0, 30);
        assert_eq!(slow.pools[0].ticks, fast.pools[0].ticks);
        assert_eq!(slow.pools[0].cells, fast.pools[0].cells);
        assert!(!run(30.0, 1).active());
    }
}
