//! Game state, rules, physics and drawing.

use crate::ai::{self, AiKind};
use crate::gfx::{text_w, Fb, Rgb, H, W};
use crate::terrain::Terrain;
use macroquad::prelude::{get_last_key_pressed, is_key_down, is_key_pressed, vec2, KeyCode, Vec2};
use macroquad::rand::{gen_range, ChooseRandom};

pub const G: f32 = 200.0;
pub const WIND_K: f32 = 4.0;
pub const POWER_K: f32 = 0.45;
const SIM_DT: f32 = 1.0 / 600.0;
const SETTLE_RATE: f32 = 240.0; // dirt fall, px/s
const FALL_RATE: f32 = 120.0; // tank fall, px/s
const HUD_H: i32 = 40;
const INF: u32 = u32::MAX;
const MAX_PLAYERS: usize = 8;

const PLAYER_COLORS: [Rgb; MAX_PLAYERS] = [
    Rgb(255, 50, 50),
    Rgb(60, 230, 60),
    Rgb(90, 130, 255),
    Rgb(255, 255, 60),
    Rgb(255, 70, 255),
    Rgb(60, 255, 255),
    Rgb(255, 150, 30),
    Rgb(240, 240, 240),
];

const FIRE_TALK: [&str; 8] = [
    "Incoming!",
    "Special delivery!",
    "Duck and cover!",
    "Say goodnight.",
    "Fore!",
    "This one's for you.",
    "Catch!",
    "Knock knock.",
];
const DEATH_TALK: [&str; 6] = [
    "Not like this!",
    "Tell mom I tried.",
    "I'll be back... maybe.",
    "Ouch.",
    "Was it something I said?",
    "Medic!",
];

// ---------------------------------------------------------------- weapons

#[derive(Clone, Copy, PartialEq)]
pub enum WKind {
    Blast,
    Mirv,
    Funky,
    Roller,
    Dirt,
    Tracer,
}

pub struct Weapon {
    pub name: &'static str,
    pub kind: WKind,
    pub radius: f32,
    pub damage: f32,
    pub ammo: u32,
}

const fn wpn(name: &'static str, kind: WKind, radius: f32, damage: f32, ammo: u32) -> Weapon {
    Weapon { name, kind, radius, damage, ammo }
}

pub const WEAPONS: [Weapon; 9] = [
    wpn("Baby Missile", WKind::Blast, 10.0, 35.0, INF),
    wpn("Missile", WKind::Blast, 20.0, 60.0, 10),
    wpn("Baby Nuke", WKind::Blast, 40.0, 90.0, 3),
    wpn("Nuke", WKind::Blast, 75.0, 120.0, 1),
    wpn("MIRV", WKind::Mirv, 20.0, 50.0, 2),
    wpn("Funky Bomb", WKind::Funky, 25.0, 50.0, 2),
    wpn("Roller", WKind::Roller, 20.0, 60.0, 4),
    wpn("Dirt Ball", WKind::Dirt, 30.0, 0.0, 4),
    wpn("Tracer", WKind::Tracer, 0.0, 0.0, 20),
];

// ---------------------------------------------------------------- tanks

#[derive(Clone, Copy, PartialEq)]
pub enum Ctrl {
    Human,
    Ai(AiKind),
}

const CTRLS: [Ctrl; 4] = [
    Ctrl::Human,
    Ctrl::Ai(AiKind::Moron),
    Ctrl::Ai(AiKind::Shooter),
    Ctrl::Ai(AiKind::Cyborg),
];

impl Ctrl {
    fn label(self) -> &'static str {
        match self {
            Ctrl::Human => "Human",
            Ctrl::Ai(AiKind::Moron) => "Moron",
            Ctrl::Ai(AiKind::Shooter) => "Shooter",
            Ctrl::Ai(AiKind::Cyborg) => "Cyborg",
        }
    }
}

pub struct Tank {
    pub name: String,
    pub color: Rgb,
    pub ctrl: Ctrl,
    /// Centre column and bottom row of the tank (terrain starts at y + 1).
    pub x: i32,
    pub y: i32,
    pub angle: i32,
    pub power: i32,
    pub health: i32,
    pub alive: bool,
    pub ammo: [u32; WEAPONS.len()],
    pub weapon: usize,
    fall: i32,
    last_hit: Option<usize>,
    kills: i32,
    wins: u32,
}

impl Tank {
    pub fn max_power(&self) -> i32 {
        (self.health * 10).max(0)
    }

    pub fn center(&self) -> Vec2 {
        vec2(self.x as f32, self.y as f32 - 4.0)
    }

    /// Barrel tip and unit direction for a given angle (0 = right, 90 = up).
    pub fn muzzle(&self, angle: i32) -> (Vec2, Vec2) {
        let a = (angle as f32).to_radians();
        let dir = vec2(a.cos(), -a.sin());
        (vec2(self.x as f32, (self.y - 6) as f32) + dir * 10.0, dir)
    }

    pub fn hit(&self, x: i32, y: i32) -> bool {
        (x - self.x).abs() <= 7 && y <= self.y && y >= self.y - 10
    }

    fn supported(&self, terrain: &Terrain) -> bool {
        (-7..=7).any(|dx| terrain.solid(self.x + dx, self.y + 1))
    }
}

// ---------------------------------------------------------------- projectiles

#[derive(Clone, Copy, PartialEq)]
enum PKind {
    Shell,
    Carrier, // MIRV before split
    Bomblet,
    Rolling { dir: i32, timer: f32 },
}

#[derive(Clone, Copy)]
struct Proj {
    pos: Vec2,
    vel: Vec2,
    owner: usize,
    weapon: usize,
    kind: PKind,
    radius: f32,
    damage: f32,
    age: f32,
}

enum Step {
    Alive,
    Gone,
    Split,
    Hit(Vec2, bool),
}

struct Explosion {
    pos: Vec2,
    radius: f32,
    damage: f32,
    owner: usize,
    dirt: bool,
    t: f32,
    dur: f32,
}

impl Explosion {
    fn new(pos: Vec2, radius: f32, damage: f32, owner: usize, dirt: bool) -> Self {
        Explosion { pos, radius, damage, owner, dirt, t: 0.0, dur: 0.4 + radius / 100.0 }
    }
}

struct Bubble {
    text: &'static str,
    x: i32,
    y: i32,
    t: f32,
}

// ---------------------------------------------------------------- input

/// Key auto-repeat for held keys (call `hit` once per frame per key).
struct KeyRep {
    held: Vec<(KeyCode, f32, f32)>,
    dt: f32,
}

impl KeyRep {
    fn hit(&mut self, k: KeyCode) -> bool {
        if is_key_pressed(k) {
            self.held.retain(|e| e.0 != k);
            self.held.push((k, 0.0, 0.0));
            return true;
        }
        if !is_key_down(k) {
            self.held.retain(|e| e.0 != k);
            return false;
        }
        let dt = self.dt;
        let Some(e) = self.held.iter_mut().find(|e| e.0 == k) else { return false };
        e.1 += dt;
        if e.1 < 0.3 {
            return false;
        }
        e.2 += dt;
        if e.2 >= 0.025 {
            e.2 -= 0.025;
            true
        } else {
            false
        }
    }
}

// ---------------------------------------------------------------- game

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Menu,
    Aim,
    Resolve,
    RoundOver,
    GameOver,
}

pub struct Game {
    phase: Phase,
    phase_t: f32,
    menu_sel: usize,
    n_players: usize,
    n_rounds: u32,
    cfg: [Ctrl; MAX_PLAYERS],
    round: u32,
    terrain: Terrain,
    tanks: Vec<Tank>,
    projs: Vec<Proj>,
    expls: Vec<Explosion>,
    trail: Vec<(i32, i32, Rgb)>,
    bubbles: Vec<Bubble>,
    current: usize,
    wind: i32,
    sim_acc: f32,
    settle_acc: f32,
    fall_acc: f32,
    ai_plan: Option<ai::Plan>,
    ai_t: f32,
    keys: KeyRep,
    time: f32,
    round_msg: String,
}

impl Game {
    pub fn new() -> Self {
        let mut cfg = [Ctrl::Ai(AiKind::Shooter); MAX_PLAYERS];
        cfg[0] = Ctrl::Human;
        Game {
            phase: Phase::Menu,
            phase_t: 0.0,
            menu_sel: 0,
            n_players: 2,
            n_rounds: 5,
            cfg,
            round: 1,
            terrain: Terrain::generate(),
            tanks: Vec::new(),
            projs: Vec::new(),
            expls: Vec::new(),
            trail: Vec::new(),
            bubbles: Vec::new(),
            current: 0,
            wind: 0,
            sim_acc: 0.0,
            settle_acc: 0.0,
            fall_acc: 0.0,
            ai_plan: None,
            ai_t: 0.0,
            keys: KeyRep { held: Vec::new(), dt: 0.0 },
            time: 0.0,
            round_msg: String::new(),
        }
    }

    fn set_phase(&mut self, p: Phase) {
        self.phase = p;
        self.phase_t = 0.0;
    }

    /// Returns false when the player wants to quit.
    pub fn update(&mut self, dt: f32) -> bool {
        self.keys.dt = dt;
        self.time += dt;
        self.phase_t += dt;
        for b in &mut self.bubbles {
            b.t -= dt;
        }
        self.bubbles.retain(|b| b.t > 0.0);

        match self.phase {
            Phase::Menu => return self.update_menu(),
            Phase::Aim | Phase::Resolve if is_key_pressed(KeyCode::Escape) => {
                self.terrain = Terrain::generate();
                self.set_phase(Phase::Menu);
            }
            Phase::Aim => self.update_aim(dt),
            Phase::Resolve => self.update_resolve(dt),
            Phase::RoundOver => {
                if self.phase_t > 0.8 && get_last_key_pressed().is_some() {
                    if self.round < self.n_rounds {
                        self.round += 1;
                        self.start_round();
                    } else {
                        self.set_phase(Phase::GameOver);
                    }
                }
            }
            Phase::GameOver => {
                if self.phase_t > 0.8 && get_last_key_pressed().is_some() {
                    self.set_phase(Phase::Menu);
                }
            }
        }
        true
    }

    // ------------------------------------------------------------ menu

    fn update_menu(&mut self) -> bool {
        if is_key_pressed(KeyCode::Escape) {
            return false;
        }
        let items = 3 + self.n_players;
        if self.keys.hit(KeyCode::Up) {
            self.menu_sel = (self.menu_sel + items - 1) % items;
        }
        if self.keys.hit(KeyCode::Down) {
            self.menu_sel = (self.menu_sel + 1) % items;
        }
        let mut d = 0;
        if self.keys.hit(KeyCode::Left) {
            d = -1;
        }
        if self.keys.hit(KeyCode::Right) {
            d = 1;
        }
        if d != 0 {
            match self.menu_sel {
                0 => self.n_players = (self.n_players as i32 + d).clamp(2, MAX_PLAYERS as i32) as usize,
                1 => self.n_rounds = (self.n_rounds as i32 + d).clamp(1, 99) as u32,
                s if s < 2 + self.n_players => {
                    let i = CTRLS.iter().position(|c| *c == self.cfg[s - 2]).unwrap();
                    self.cfg[s - 2] = CTRLS[(i as i32 + d).rem_euclid(CTRLS.len() as i32) as usize];
                }
                _ => {}
            }
        }
        self.menu_sel = self.menu_sel.min(2 + self.n_players);
        if is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space) {
            self.start_game();
        }
        true
    }

    fn start_game(&mut self) {
        self.tanks = (0..self.n_players)
            .map(|i| {
                let ctrl = self.cfg[i];
                let name = match ctrl {
                    Ctrl::Human => format!("Player {}", i + 1),
                    _ => format!("{} {}", ctrl.label(), i + 1),
                };
                Tank {
                    name,
                    color: PLAYER_COLORS[i],
                    ctrl,
                    x: 0,
                    y: 0,
                    angle: 90,
                    power: 400,
                    health: 100,
                    alive: true,
                    ammo: [0; WEAPONS.len()],
                    weapon: 0,
                    fall: 0,
                    last_hit: None,
                    kills: 0,
                    wins: 0,
                }
            })
            .collect();
        self.round = 1;
        self.start_round();
    }

    fn start_round(&mut self) {
        self.terrain = Terrain::generate();
        self.wind = gen_range(-10, 11);
        self.projs.clear();
        self.expls.clear();
        self.trail.clear();
        self.bubbles.clear();

        let n = self.tanks.len();
        let slot = W / n as i32;
        let mut slots: Vec<i32> = (0..n as i32).collect();
        slots.shuffle();
        for (i, t) in self.tanks.iter_mut().enumerate() {
            let x = slots[i] * slot + slot / 2 + gen_range(-slot / 5, slot / 5 + 1);
            let x = x.clamp(20, W - 21);
            t.x = x;
            t.y = self.terrain.flatten(x, 9) - 1;
            t.angle = if x < W / 2 { gen_range(30, 80) } else { gen_range(100, 150) };
            t.power = 400;
            t.health = 100;
            t.alive = true;
            t.fall = 0;
            t.last_hit = None;
            t.weapon = 0;
            for (a, w) in t.ammo.iter_mut().zip(WEAPONS.iter()) {
                *a = w.ammo;
            }
        }
        self.current = gen_range(0, n);
        self.begin_turn();
    }

    fn begin_turn(&mut self) {
        let t = &mut self.tanks[self.current];
        t.power = t.power.min(t.max_power());
        if t.ammo[t.weapon] == 0 {
            t.weapon = 0;
        }
        self.ai_plan = None;
        self.ai_t = 0.0;
        self.set_phase(Phase::Aim);
    }

    // ------------------------------------------------------------ aiming

    fn update_aim(&mut self, dt: f32) {
        let cur = self.current;
        match self.tanks[cur].ctrl {
            Ctrl::Human => {
                let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
                let k = &mut self.keys;
                let t = &mut self.tanks[cur];
                let step = if shift { 5 } else { 1 };
                if k.hit(KeyCode::Left) {
                    t.angle = (t.angle + step).min(180);
                }
                if k.hit(KeyCode::Right) {
                    t.angle = (t.angle - step).max(0);
                }
                if k.hit(KeyCode::Up) {
                    t.power = (t.power + step * 5).min(t.max_power());
                }
                if k.hit(KeyCode::Down) {
                    t.power = (t.power - step * 5).max(0);
                }
                if k.hit(KeyCode::PageUp) {
                    t.power = (t.power + 100).min(t.max_power());
                }
                if k.hit(KeyCode::PageDown) {
                    t.power = (t.power - 100).max(0);
                }
                if is_key_pressed(KeyCode::Tab) {
                    let n = WEAPONS.len();
                    let d = if shift { n - 1 } else { 1 };
                    let mut w = t.weapon;
                    loop {
                        w = (w + d) % n;
                        if t.ammo[w] > 0 {
                            break;
                        }
                    }
                    t.weapon = w;
                }
                if is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) {
                    self.fire();
                }
            }
            Ctrl::Ai(kind) => {
                if self.phase_t < 0.4 {
                    return; // short "thinking" pause
                }
                if self.ai_plan.is_none() {
                    let p = ai::plan(kind, &self.terrain, &self.tanks, cur, self.wind);
                    self.tanks[cur].weapon = p.weapon;
                    self.ai_plan = Some(p);
                }
                let plan = self.ai_plan.as_ref().unwrap();
                // Swing the turret visibly towards the planned angle/power.
                self.ai_t += dt;
                let t = &mut self.tanks[cur];
                while self.ai_t >= 0.01 {
                    self.ai_t -= 0.01;
                    t.angle += (plan.angle - t.angle).signum();
                    t.power += (plan.power - t.power).clamp(-10, 10);
                }
                if t.angle == plan.angle && t.power == plan.power && self.phase_t > 1.2 {
                    self.fire();
                }
            }
        }
    }

    fn fire(&mut self) {
        let cur = self.current;
        let t = &mut self.tanks[cur];
        let w = &WEAPONS[t.weapon];
        if t.ammo[t.weapon] != INF {
            t.ammo[t.weapon] -= 1;
        }
        let (pos, dir) = t.muzzle(t.angle);
        self.projs.push(Proj {
            pos,
            vel: dir * t.power as f32 * POWER_K,
            owner: cur,
            weapon: t.weapon,
            kind: if w.kind == WKind::Mirv { PKind::Carrier } else { PKind::Shell },
            radius: w.radius,
            damage: w.damage,
            age: 0.0,
        });
        if gen_range(0.0f32, 1.0) < 0.3 {
            let (x, y) = (t.x, t.y);
            self.say(FIRE_TALK[gen_range(0, FIRE_TALK.len())], x, y);
        }
        self.trail.clear();
        self.sim_acc = 0.0;
        self.set_phase(Phase::Resolve);
    }

    fn say(&mut self, text: &'static str, x: i32, y: i32) {
        self.bubbles.push(Bubble { text, x, y: y - 16, t: 2.0 });
    }

    // ------------------------------------------------------------ resolving

    fn update_resolve(&mut self, dt: f32) {
        self.sim_acc += dt;
        while self.sim_acc >= SIM_DT {
            self.sim_acc -= SIM_DT;
            self.step_projs(SIM_DT);
        }

        for e in &mut self.expls {
            e.t += dt;
        }
        let (done, keep): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.expls).into_iter().partition(|e| e.t >= e.dur);
        self.expls = keep;
        for e in done {
            self.apply_explosion(e);
        }

        let mut settling = false;
        self.settle_acc += dt * SETTLE_RATE;
        while self.settle_acc >= 1.0 {
            self.settle_acc -= 1.0;
            settling |= self.terrain.settle_step();
        }

        let falling = self.update_falling(dt);

        if !self.projs.is_empty() || !self.expls.is_empty() || settling || falling {
            return;
        }

        // Everything is quiet: blow up the dead, then move on.
        let mut any_died = false;
        for i in 0..self.tanks.len() {
            let t = &mut self.tanks[i];
            if t.alive && t.health <= 0 {
                t.alive = false;
                any_died = true;
                let killer = t.last_hit.unwrap_or(i);
                let (c, x, y) = (t.center(), t.x, t.y);
                if killer == i {
                    self.tanks[i].kills -= 1;
                } else {
                    self.tanks[killer].kills += 1;
                }
                self.expls.push(Explosion::new(c, 30.0, 40.0, killer, false));
                if gen_range(0.0f32, 1.0) < 0.6 {
                    self.say(DEATH_TALK[gen_range(0, DEATH_TALK.len())], x, y);
                }
            }
        }
        if any_died {
            return;
        }

        let alive: Vec<usize> = (0..self.tanks.len()).filter(|&i| self.tanks[i].alive).collect();
        if alive.len() <= 1 {
            self.round_msg = match alive.first() {
                Some(&w) => {
                    self.tanks[w].wins += 1;
                    format!("{} wins the round!", self.tanks[w].name)
                }
                None => "Nobody survived!".to_string(),
            };
            self.set_phase(Phase::RoundOver);
            return;
        }
        let n = self.tanks.len();
        let mut next = self.current;
        loop {
            next = (next + 1) % n;
            if self.tanks[next].alive {
                break;
            }
        }
        self.current = next;
        self.begin_turn();
    }

    fn update_falling(&mut self, dt: f32) -> bool {
        self.fall_acc += dt * FALL_RATE;
        let steps = self.fall_acc as i32;
        self.fall_acc -= steps as f32;
        let mut any = false;
        for t in self.tanks.iter_mut().filter(|t| t.alive) {
            if t.supported(&self.terrain) {
                if t.fall > 0 {
                    t.health -= t.fall / 2;
                    t.fall = 0;
                }
                continue;
            }
            any = true;
            for _ in 0..steps {
                if t.supported(&self.terrain) {
                    break;
                }
                t.y += 1;
                t.fall += 1;
            }
            if t.y > H + 10 {
                t.health = 0;
                t.fall = 0;
                t.y = H + 10;
                any = false;
            }
        }
        any
    }

    fn tank_at(&self, x: i32, y: i32, owner: usize, age: f32) -> Option<usize> {
        self.tanks
            .iter()
            .enumerate()
            .find(|(i, t)| t.alive && t.hit(x, y) && !(*i == owner && age < 0.25))
            .map(|(i, _)| i)
    }

    fn step_projs(&mut self, dt: f32) {
        let mut next = Vec::with_capacity(self.projs.len());
        for mut p in std::mem::take(&mut self.projs) {
            match self.step_proj(&mut p, dt) {
                Step::Alive => next.push(p),
                Step::Gone => {}
                Step::Split => {
                    for k in -2..=2 {
                        let mut w = p;
                        w.kind = PKind::Bomblet;
                        w.vel.x += k as f32 * 30.0;
                        next.push(w);
                    }
                }
                Step::Hit(pos, on_tank) => self.impact(p, pos, on_tank, &mut next),
            }
        }
        // Projectiles spawned by impacts this step are already in `next`.
        self.projs = next;
    }

    fn step_proj(&mut self, p: &mut Proj, dt: f32) -> Step {
        p.age += dt;
        if let PKind::Rolling { dir, timer } = &mut p.kind {
            *timer += dt;
            while *timer >= 1.0 / 150.0 {
                *timer -= 1.0 / 150.0;
                let (nx, mut y) = (p.pos.x as i32 + *dir, p.pos.y as i32);
                if !(0..W).contains(&nx) {
                    return Step::Gone;
                }
                if self.tank_at(nx, y, p.owner, 1.0).is_some() {
                    return Step::Hit(p.pos, true);
                }
                if self.terrain.solid(nx, y) {
                    return Step::Hit(p.pos, false); // reached a valley
                }
                while !self.terrain.solid(nx, y + 1) {
                    y += 1;
                }
                p.pos = vec2(nx as f32, y as f32);
                self.trail.push((nx, y, self.tanks[p.owner].color));
            }
            if p.age > 15.0 {
                return Step::Hit(p.pos, false);
            }
            return Step::Alive;
        }

        let prev_vy = p.vel.y;
        p.vel.y += G * dt;
        p.vel.x += self.wind as f32 * WIND_K * dt;
        p.pos += p.vel * dt;
        if p.kind == PKind::Carrier && prev_vy < 0.0 && p.vel.y >= 0.0 {
            return Step::Split;
        }
        let (x, y) = (p.pos.x.floor() as i32, p.pos.y.floor() as i32);
        if !(0..W).contains(&x) || p.age > 30.0 {
            return Step::Gone;
        }
        if y >= H {
            return Step::Hit(vec2(p.pos.x, (H - 1) as f32), false);
        }
        if y >= 0 {
            if self.terrain.solid(x, y) {
                return Step::Hit(p.pos, false);
            }
            if self.tank_at(x, y, p.owner, p.age).is_some() {
                return Step::Hit(p.pos, true);
            }
            let c = self.tanks[p.owner].color;
            if self.trail.last().map_or(true, |&(lx, ly, _)| lx != x || ly != y) {
                self.trail.push((x, y, c));
            }
        }
        Step::Alive
    }

    fn impact(&mut self, p: Proj, pos: Vec2, on_tank: bool, spawn: &mut Vec<Proj>) {
        let kind = WEAPONS[p.weapon].kind;
        let boom = |g: &mut Game, r: f32, d: f32, dirt: bool| {
            g.expls.push(Explosion::new(pos, r, d, p.owner, dirt));
        };
        match (p.kind, kind) {
            (PKind::Shell, WKind::Tracer) => {}
            (PKind::Shell, WKind::Dirt) => boom(self, p.radius, 0.0, true),
            (PKind::Shell, WKind::Funky) => {
                boom(self, p.radius, p.damage, false);
                for _ in 0..6 {
                    spawn.push(Proj {
                        pos: pos - vec2(0.0, 4.0),
                        vel: vec2(gen_range(-120.0, 120.0), gen_range(-220.0, -100.0)),
                        kind: PKind::Bomblet,
                        radius: p.radius * 0.6,
                        damage: p.damage * 0.6,
                        age: 0.0,
                        ..p
                    });
                }
            }
            (PKind::Shell, WKind::Roller) if !on_tank => {
                let (x, mut y) = (pos.x as i32, pos.y as i32);
                while self.terrain.solid(x, y) && y > 0 {
                    y -= 1;
                }
                let (l, r) = (self.terrain.surface(x - 4), self.terrain.surface(x + 4));
                let dir = if l != r {
                    if l > r { -1 } else { 1 }
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
            _ => boom(self, p.radius, p.damage, false),
        }
    }

    fn apply_explosion(&mut self, e: Explosion) {
        let (cx, cy, r) = (e.pos.x as i32, e.pos.y as i32, e.radius as i32);
        if e.dirt {
            self.terrain.add_dirt(cx, cy, r, HUD_H);
            return;
        }
        self.terrain.carve(cx, cy, r);
        let reach = e.radius + 6.0;
        for t in self.tanks.iter_mut().filter(|t| t.alive) {
            let d = t.center().distance(e.pos);
            if d < reach {
                let dmg = (e.damage * (1.0 - d / reach)).ceil() as i32;
                t.health -= dmg;
                t.last_hit = Some(e.owner);
            }
        }
    }

    // ------------------------------------------------------------ drawing

    pub fn draw(&self, fb: &mut Fb) {
        self.terrain.render(fb);
        if self.phase == Phase::Menu {
            self.draw_menu(fb);
            return;
        }

        for &(x, y, c) in &self.trail {
            fb.put(x, y, c);
        }
        for t in self.tanks.iter().filter(|t| t.alive) {
            draw_tank(fb, t);
        }
        if self.phase == Phase::Aim && (self.time * 3.0) as i32 % 2 == 0 {
            let t = &self.tanks[self.current];
            fb.line(t.x - 3, t.y - 20, t.x, t.y - 16, Rgb::WHITE);
            fb.line(t.x + 3, t.y - 20, t.x, t.y - 16, Rgb::WHITE);
        }
        for p in &self.projs {
            fb.rect(p.pos.x as i32, p.pos.y as i32 - 1, 2, 2, Rgb::WHITE);
        }
        for e in &self.expls {
            draw_explosion(fb, e);
        }
        for b in &self.bubbles {
            let w = text_w(b.text) + 6;
            let x = (b.x - w / 2).clamp(0, W - w);
            let y = (b.y - 20).max(HUD_H + 2);
            fb.rect(x, y, w, 20, Rgb::BLACK);
            fb.frame(x, y, w, 20, Rgb::WHITE);
            fb.text(x + 3, y + 2, b.text, Rgb::WHITE);
        }
        self.draw_hud(fb);

        match self.phase {
            Phase::RoundOver => {
                let lines = [
                    format!("Round {} of {}", self.round, self.n_rounds),
                    self.round_msg.clone(),
                    String::new(),
                    "Press any key".to_string(),
                ];
                dialog(fb, &lines);
            }
            Phase::GameOver => {
                let mut order: Vec<&Tank> = self.tanks.iter().collect();
                order.sort_by_key(|t| (std::cmp::Reverse(t.wins), std::cmp::Reverse(t.kills)));
                let mut lines = vec!["FINAL STANDINGS".to_string(), String::new()];
                for (i, t) in order.iter().enumerate() {
                    lines.push(format!("{}. {:<12} wins {:>2}  kills {:>3}", i + 1, t.name, t.wins, t.kills));
                }
                lines.push(String::new());
                lines.push("Press any key".to_string());
                dialog(fb, &lines);
            }
            _ => {}
        }
    }

    fn draw_hud(&self, fb: &mut Fb) {
        fb.bevel(0, 0, W, HUD_H);
        let t = &self.tanks[self.current];
        fb.rect(8, 6, 10, 12, t.color);
        fb.frame(8, 6, 10, 12, Rgb::BLACK);
        fb.text(24, 4, &t.name, Rgb::BLACK);
        fb.text(200, 4, &format!("Power: {:4}", t.power), Rgb::BLACK);
        fb.text(340, 4, &format!("Angle: {:3}", t.angle), Rgb::BLACK);
        fb.text(470, 4, &format!("Health: {:3}", t.health.max(0)), Rgb::BLACK);

        let ammo = t.ammo[t.weapon];
        let w = if ammo == INF {
            WEAPONS[t.weapon].name.to_string()
        } else {
            format!("{}: {}", WEAPONS[t.weapon].name, ammo)
        };
        fb.text(8, 21, &w, Rgb::BLACK);
        let wind = match self.wind {
            0 => "Wind: calm".to_string(),
            v if v > 0 => format!("Wind: {} >>", v),
            v => format!("Wind: << {}", -v),
        };
        fb.text(270, 21, &wind, Rgb::BLACK);
        fb.text(470, 21, &format!("Round {}/{}", self.round, self.n_rounds), Rgb::BLACK);
    }

    fn draw_menu(&self, fb: &mut Fb) {
        let (pw, ph) = (520, 150 + self.n_players as i32 * 18 + 60);
        let (px, py) = (W / 2 - pw / 2, 40);
        fb.bevel(px, py, pw, ph);
        let title = "SCORCHED EARTH";
        fb.text_scaled(W / 2 - text_w(title) * 3 / 2 + 3, py + 15, title, Rgb::BLACK, 3, 2);
        fb.text_scaled(W / 2 - text_w(title) * 3 / 2, py + 12, title, Rgb(200, 0, 0), 3, 2);
        fb.text_centered(py + 50, "a Rust tribute", Rgb::DARK);

        let mut items = vec![
            format!("Players:  < {} >", self.n_players),
            format!("Rounds:   < {} >", self.n_rounds),
        ];
        for i in 0..self.n_players {
            items.push(format!("Player {}: < {} >", i + 1, self.cfg[i].label()));
        }
        items.push("START GAME".to_string());

        let y0 = py + 85;
        for (i, s) in items.iter().enumerate() {
            let y = y0 + i as i32 * 18 + if i + 1 == items.len() { 10 } else { 0 };
            if i == self.menu_sel {
                fb.rect(px + 30, y - 1, pw - 60, 18, Rgb(0, 0, 170));
            }
            let c = if i == self.menu_sel { Rgb::WHITE } else { Rgb::BLACK };
            if (2..2 + self.n_players).contains(&i) {
                fb.rect(px + 50, y + 3, 10, 10, PLAYER_COLORS[i - 2]);
            }
            fb.text(px + 70, y, s, c);
        }
        let help = "Up/Down select  Left/Right change  Enter start  Esc quit";
        fb.text_centered(py + ph - 24, help, Rgb::DARK);

        let keys = [
            "In game:  Left/Right angle   Up/Down power   PgUp/PgDn power x100",
            "Shift = faster   Tab weapon   Space fire   Esc menu",
        ];
        for (i, k) in keys.iter().enumerate() {
            let y = H - 44 + i as i32 * 18;
            fb.rect(W / 2 - text_w(k) / 2 - 4, y - 1, text_w(k) + 8, 18, Rgb::BLACK);
            fb.text_centered(y, k, Rgb::WHITE);
        }
    }
}

fn draw_tank(fb: &mut Fb, t: &Tank) {
    let (x, y) = (t.x, t.y);
    fb.rect(x - 7, y - 1, 15, 2, t.color.scale(0.5));
    fb.rect(x - 6, y - 3, 13, 2, t.color);
    for dy in -4..=0 {
        for dx in -4..=4 {
            if dx * dx + dy * dy <= 16 {
                fb.put(x + dx, y - 4 + dy, t.color);
            }
        }
    }
    let a = (t.angle as f32).to_radians();
    let ex = x as f32 + a.cos() * 10.0;
    let ey = (y - 6) as f32 - a.sin() * 10.0;
    fb.line(x, y - 6, ex.round() as i32, ey.round() as i32, t.color.scale(1.4));
}

fn fire_color(i: i32, dirt: bool) -> Rgb {
    let f = i as f32 / 15.0;
    let v = 1.0 - (2.0 * f - 1.0).abs(); // triangle wave: cycles smoothly
    if dirt {
        return Rgb::lerp(Rgb(60, 30, 10), Rgb(200, 150, 80), v);
    }
    let c = |k: f32| (k.clamp(0.0, 1.0) * 255.0) as u8;
    Rgb(c(0.3 + v * 1.5), c(v * 1.6 - 0.5), c(v * 3.0 - 2.2))
}

fn draw_explosion(fb: &mut Fb, e: &Explosion) {
    let p = e.t / e.dur;
    let r = if p < 0.4 { e.radius * p / 0.4 } else { e.radius };
    let ri = r.ceil() as i32;
    let (cx, cy) = (e.pos.x as i32, e.pos.y as i32);
    let phase = (e.t * 40.0) as i32;
    for dy in -ri..=ri {
        for dx in -ri..=ri {
            let d2 = (dx * dx + dy * dy) as f32;
            if d2 <= r * r {
                let idx = ((d2.sqrt() / 3.0) as i32 - phase).rem_euclid(16);
                fb.put(cx + dx, cy + dy, fire_color(idx, e.dirt));
            }
        }
    }
}

/// Centred grey message box.
fn dialog(fb: &mut Fb, lines: &[String]) {
    let w = lines.iter().map(|l| text_w(l)).max().unwrap_or(0) + 40;
    let h = lines.len() as i32 * 18 + 24;
    let (x, y) = (W / 2 - w / 2, H / 2 - h / 2);
    fb.bevel(x, y, w, h);
    for (i, l) in lines.iter().enumerate() {
        fb.text_centered(y + 12 + i as i32 * 18, l, Rgb::BLACK);
    }
}
