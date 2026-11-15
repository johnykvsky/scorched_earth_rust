//! Game state, rules, physics and drawing.

use crate::ai;
use crate::economy::{self, DamageEvent, DamageTarget, Relation};
use crate::gfx::{text_w, Fb, Rgb, H, W};
use crate::inventory::Inventory;
use crate::physics::{self, Physics, SIM_DT};
use crate::settings::Settings;
use crate::shop::{self, OpeningShop};
use crate::tank::{Ctrl, Tank, PLAYER_COLORS};
use crate::terrain::Terrain;
use crate::ui::{self, draw_explosion, draw_tank, KeyRep, MenuAction, MenuState, HUD_H};
use crate::weapons::{
    self, explosion_damage, Damage, Explosion, ItemId, ItemKind, PKind, Proj, Step, WKind,
};
use macroquad::prelude::{get_last_key_pressed, is_key_down, is_key_pressed, vec2, KeyCode};
use macroquad::rand::{gen_range, ChooseRandom};

const SETTLE_RATE: f32 = 240.0; // dirt fall, px/s

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

struct Bubble {
    text: &'static str,
    x: i32,
    y: i32,
    t: f32,
}

/// Keep player identities stable while cycling through battlefield positions.
fn next_tank(tanks: &[Tank], current: usize) -> usize {
    let position = (tanks[current].x, current);
    tanks
        .iter()
        .enumerate()
        .filter(|(i, t)| *i != current && t.alive)
        .min_by_key(|(i, t)| ((t.x, *i) <= position, t.x, *i))
        .map_or(current, |(i, _)| i)
}

// ---------------------------------------------------------------- game

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Menu,
    OpeningShop,
    Aim,
    Resolve,
    RoundOver,
    GameOver,
}

pub struct Game {
    phase: Phase,
    phase_t: f32,
    menu: MenuState,
    config_path: Option<std::path::PathBuf>,
    settings: Settings,
    round: u32,
    interest_round: u32,
    round_paid: bool,
    action_owner: Option<usize>,
    prices: [u32; crate::weapons::ITEM_COUNT],
    opening_shop: OpeningShop,
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
    settle_dirt: bool,
    fall_acc: f32,
    ai_plan: Option<ai::Plan>,
    ai_t: f32,
    keys: KeyRep,
    time: f32,
    round_msg: String,
}

impl Game {
    pub fn new() -> Self {
        Game {
            phase: Phase::Menu,
            phase_t: 0.0,
            menu: MenuState::default(),
            config_path: None,
            settings: Settings::default(),
            round: 1,
            interest_round: 0,
            round_paid: false,
            action_owner: None,
            prices: economy::base_prices(),
            opening_shop: OpeningShop::default(),
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
            settle_dirt: true,
            fall_acc: 0.0,
            ai_plan: None,
            ai_t: 0.0,
            keys: KeyRep { held: Vec::new(), dt: 0.0 },
            time: 0.0,
            round_msg: String::new(),
        }
    }

    pub fn load_settings(&mut self, path: std::path::PathBuf) {
        self.menu.notice = None;
        match Settings::load(&path) {
            Ok(settings) => self.settings = settings,
            Err(error) => {
                self.settings = Settings::default();
                eprintln!("{error}");
                self.menu.notice = Some(format!("{error}\nUsing defaults for this session."));
            }
        }
        self.config_path = Some(path);
    }

    fn save_settings(&mut self) {
        let result = self
            .config_path
            .clone()
            .map(Ok)
            .unwrap_or_else(|| crate::settings::config_path(std::iter::empty()));
        self.menu.notice = Some(match result {
            Ok(path) => match self.settings.save(&path) {
                Ok(()) => format!("Settings saved to {}", path.display()),
                Err(error) => {
                    eprintln!("{error}");
                    error
                }
            },
            Err(error) => error,
        });
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
            Phase::Menu => {
                match ui::update_menu(&mut self.settings, &mut self.menu, &mut self.keys) {
                    MenuAction::Quit => return false,
                    MenuAction::Start => self.start_game(),
                    MenuAction::Save => self.save_settings(),
                    MenuAction::Stay => {}
                }
            }
            Phase::OpeningShop => {
                let player = self.opening_shop.player;
                if ui::update_opening_shop(
                    &mut self.tanks[player],
                    &self.prices,
                    &mut self.opening_shop.selected,
                    &mut self.keys,
                ) {
                    self.opening_shop.player += 1;
                    self.advance_opening_shop();
                }
            }
            Phase::Aim | Phase::Resolve if is_key_pressed(KeyCode::Escape) => {
                self.terrain = Terrain::generate();
                self.set_phase(Phase::Menu);
            }
            Phase::Aim => self.update_aim(dt),
            Phase::Resolve => self.update_resolve(dt),
            Phase::RoundOver => {
                if self.phase_t > 0.8 && get_last_key_pressed().is_some() {
                    if self.round < self.settings.rounds {
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

    fn start_game(&mut self) {
        self.tanks = (0..self.settings.players)
            .map(|i| {
                let ctrl = self.settings.controllers[i];
                let name = match ctrl {
                    Ctrl::Human => format!("Player {}", i + 1),
                    _ => format!("{} {}", ctrl.label(), i + 1),
                };
                Tank {
                    name,
                    color: PLAYER_COLORS[i],
                    ctrl,
                    account: crate::economy::Account::new(self.settings.economics.cash_at_start),
                    team: None,
                    x: 0,
                    y: 0,
                    angle: 90,
                    power: 400,
                    health: 100,
                    alive: true,
                    inventory: Inventory::starter(),
                    weapon: ItemId::BabyMissile,
                    fall: 0,
                    last_hit: None,
                    kills: 0,
                    wins: 0,
                }
            })
            .collect();
        self.round = 1;
        self.interest_round = 0;
        self.prices = economy::base_prices();
        self.opening_shop = OpeningShop::default();
        if self.settings.economics.cash_at_start > 0 {
            self.set_phase(Phase::OpeningShop);
            self.advance_opening_shop();
        } else {
            self.start_round();
        }
    }

    fn advance_opening_shop(&mut self) {
        while self.opening_shop.player < self.tanks.len() {
            let tank = &mut self.tanks[self.opening_shop.player];
            self.opening_shop.selected = 0;
            if tank.ctrl == Ctrl::Human && !tank.account.cash.is_zero() {
                return;
            }
            if tank.ctrl != Ctrl::Human && self.settings.economics.computers_buy {
                shop::computer_buys(tank, &self.prices);
            }
            self.opening_shop.player += 1;
        }
        self.start_round();
    }

    fn finish_round(&mut self, alive: &[usize]) {
        if self.round_paid {
            return;
        }
        self.round_paid = true;
        if !alive.is_empty() {
            let share = economy::survival_pool(self.settings.economics.scoring, self.tanks.len())
                / alive.len() as u32;
            for &survivor in alive {
                self.tanks[survivor].account.cash.award(share as i32);
            }
        }
        self.round_msg = match alive.first() {
            Some(&winner) => {
                self.tanks[winner].wins += 1;
                format!("{} wins the round!", self.tanks[winner].name)
            }
            None => "Nobody survived!".into(),
        };
        self.set_phase(Phase::RoundOver);
    }

    fn start_round(&mut self) {
        if self.interest_round != self.round {
            for tank in &mut self.tanks {
                tank.account.pay_interest(self.settings.economics.interest_rate);
            }
            self.interest_round = self.round;
        }
        self.round_paid = false;
        self.action_owner = None;
        self.terrain = Terrain::generate();
        self.wind = physics::round_wind(self.settings.landscape.max_wind);
        self.settle_dirt = true;
        self.sim_acc = 0.0;
        self.settle_acc = 0.0;
        self.fall_acc = 0.0;
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
            t.weapon = ItemId::BabyMissile;
        }
        self.current = gen_range(0, n);
        self.begin_turn();
    }

    fn begin_turn(&mut self) {
        let t = &mut self.tanks[self.current];
        t.power = t.power.min(t.max_power());
        if !t.inventory.can_select(t.weapon) {
            t.weapon = ItemId::BabyMissile;
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
                let alt = is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt);
                let angle_delta = k.aim_steps(KeyCode::Left, shift, alt)
                    - k.aim_steps(KeyCode::Right, shift, alt);
                let power_delta = k.aim_steps(KeyCode::Up, shift, alt)
                    - k.aim_steps(KeyCode::Down, shift, alt)
                    + 15 * (k.aim_steps(KeyCode::PageUp, shift, alt)
                        - k.aim_steps(KeyCode::PageDown, shift, alt));
                t.angle = (t.angle + angle_delta).clamp(0, 180);
                t.power = (t.power + power_delta).clamp(0, t.max_power());
                if is_key_pressed(KeyCode::Tab) {
                    t.weapon = t.inventory.cycle_weapon(t.weapon, shift);
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
                    let physics = Physics::new(&self.settings.physics, self.wind);
                    let p = ai::plan(
                        kind,
                        &self.terrain,
                        &self.tanks,
                        cur,
                        &physics,
                        self.settings.weapons.tunneling,
                    );
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
        if !t.inventory.can_select(t.weapon) {
            return;
        }
        let w = t.weapon.item();
        if !t.inventory.remove(t.weapon, 1) {
            return;
        }
        let (pos, dir) = t.muzzle(t.angle);
        self.projs.push(Proj {
            pos,
            vel: dir * t.power as f32,
            owner: cur,
            weapon: t.weapon,
            kind: if w.kind == ItemKind::Weapon(WKind::Mirv) {
                PKind::Carrier
            } else {
                PKind::Shell
            },
            radius: w.radius,
            damage: w.damage(),
            age: 0.0,
            tunneling: self.settings.weapons.tunneling,
            in_dirt: false,
        });
        if gen_range(0.0f32, 1.0) < 0.3 {
            let (x, y) = (t.x, t.y);
            self.say(FIRE_TALK[gen_range(0, FIRE_TALK.len())], x, y);
        }
        self.trail.clear();
        self.sim_acc = 0.0;
        self.action_owner = Some(cur);
        self.settle_dirt =
            physics::dirt_falls(self.settings.physics.suspend_dirt, gen_range(0, 100));
        self.set_phase(Phase::Resolve);
    }

    fn say(&mut self, text: &'static str, x: i32, y: i32) {
        self.bubbles.push(Bubble { text, x, y: y - 16, t: 2.0 });
    }

    // ------------------------------------------------------------ resolving

    fn update_resolve(&mut self, dt: f32) {
        let shot_time = self.settings.hardware.shot_time(dt);
        self.sim_acc += shot_time;
        while self.sim_acc >= SIM_DT && !self.projs.is_empty() {
            self.sim_acc -= SIM_DT;
            self.step_projs(SIM_DT);
        }
        if self.projs.is_empty() {
            self.sim_acc = 0.0;
        }

        for e in &mut self.expls {
            e.t += shot_time;
        }
        let (done, keep): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.expls).into_iter().partition(|e| e.t >= e.dur);
        self.expls = keep;
        for e in done {
            self.apply_explosion(e);
        }

        let mut settling = false;
        if self.settle_dirt && self.projs.is_empty() && self.expls.is_empty() {
            self.settle_acc += dt * SETTLE_RATE;
            while self.settle_acc >= 1.0 {
                self.settle_acc -= 1.0;
                settling |= self.terrain.settle_step();
            }
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
                let relation = economy::relation(&self.tanks, killer, i);
                self.tanks[killer].kills += if relation == Relation::Enemy { 1 } else { -1 };
                self.tanks[killer]
                    .account
                    .cash
                    .award(economy::kill_award(self.settings.economics.scoring, relation));
                self.expls.push(Explosion::new(c, 30.0, Damage::Death, killer, false));
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
            self.finish_round(&alive);
            return;
        }
        self.current = next_tank(&self.tanks, self.current);
        if self.settings.landscape.changing_wind {
            self.wind = physics::changed_wind(
                self.wind,
                self.settings.landscape.max_wind,
                gen_range(0, 11),
            );
        }
        self.begin_turn();
    }

    fn update_falling(&mut self, dt: f32) -> bool {
        if !self.settings.physics.tanks_fall {
            self.fall_acc = 0.0;
            return false;
        }
        let impact_damage = self.settings.physics.impact_damage;
        let delay = self.settings.hardware.falling_delay;
        let steps = if delay == 0 {
            self.fall_acc = 0.0;
            H + 20
        } else {
            self.fall_acc += dt * 1000.0 / delay as f32;
            let steps = self.fall_acc as i32;
            self.fall_acc -= steps as f32;
            steps
        };
        let mut any = false;
        let mut events = Vec::new();
        for (i, t) in self.tanks.iter_mut().enumerate().filter(|(_, t)| t.alive) {
            let before = t.health.max(0);
            if !t.supported(&self.terrain) && t.fall == 0 {
                t.last_hit = Some(self.action_owner.or(t.last_hit).unwrap_or(i));
            }
            for _ in 0..steps {
                if t.supported(&self.terrain) {
                    break;
                }
                t.y += 1;
                t.fall += 1;
                if !impact_damage {
                    t.health -= 2;
                    t.power = t.power.min(t.max_power());
                }
            }
            if t.supported(&self.terrain) {
                t.finish_fall(impact_damage);
            } else {
                any = true;
            }
            if t.y > H + 10 {
                t.health = 0;
                t.fall = 0;
                t.y = H + 10;
                any = false;
            }
            let lost = before - t.health.max(0);
            if lost > 0 {
                events.push(DamageEvent {
                    owner: t.last_hit.unwrap_or(i),
                    victim: i,
                    amount: lost,
                    target: DamageTarget::Tank,
                });
            }
        }
        for event in events {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
        any
    }

    fn step_projs(&mut self, dt: f32) {
        let physics = Physics::new(&self.settings.physics, self.wind);
        let mut next = Vec::with_capacity(self.projs.len());
        for mut p in std::mem::take(&mut self.projs) {
            match weapons::step_proj(
                &mut p,
                dt,
                &mut self.terrain,
                &self.tanks,
                &physics,
                &mut self.trail,
            ) {
                Step::Alive => next.push(p),
                Step::Gone => {}
                Step::Split => {
                    for k in -2..=2 {
                        let mut w = p;
                        w.kind = PKind::Bomblet;
                        w.weapon = ItemId::Missile; // Each child is a Missile, not the MIRV carrier.
                        w.radius = ItemId::Missile.item().radius;
                        w.damage = ItemId::Missile.item().damage();
                        w.vel.x += k as f32 * 30.0;
                        next.push(w);
                    }
                }
                Step::Hit(pos, on_tank) => {
                    if let Some(event) = weapons::impact(
                        p,
                        pos,
                        on_tank,
                        &self.terrain,
                        &mut self.tanks,
                        &mut next,
                        &mut self.expls,
                    ) {
                        economy::award_damage(
                            &mut self.tanks,
                            self.settings.economics.scoring,
                            event,
                        );
                    }
                }
            }
        }
        // Projectiles spawned by impacts this step are already in `next`.
        self.projs = next;
    }

    fn apply_explosion(&mut self, e: Explosion) {
        let (cx, cy, r) = (e.pos.x as i32, e.pos.y as i32, e.radius as i32);
        if e.dirt {
            self.terrain.add_dirt(cx, cy, r, HUD_H);
            return;
        }
        self.terrain.carve(cx, cy, r);
        let mut events = Vec::new();
        for (i, t) in self.tanks.iter_mut().enumerate().filter(|(_, t)| t.alive && t.health > 0) {
            let point = match e.damage {
                Damage::Weapon(_) => vec2(t.x as f32, t.y as f32),
                Damage::Death => t.center(),
            };
            let d = point.distance(e.pos);
            let dmg = explosion_damage(d, e.radius, e.damage);
            if dmg > 0 {
                events.push(DamageEvent {
                    owner: e.owner,
                    victim: i,
                    amount: dmg.min(t.health),
                    target: DamageTarget::Tank,
                });
                t.health -= dmg;
                t.power = t.power.min(t.max_power());
                t.last_hit = Some(e.owner);
            }
        }
        for event in events {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
    }

    // ------------------------------------------------------------ drawing

    pub fn draw(&self, fb: &mut Fb) {
        self.terrain.render(fb);
        if self.phase == Phase::OpeningShop {
            ui::draw_opening_shop(
                fb,
                &self.tanks[self.opening_shop.player],
                &self.prices,
                self.opening_shop.selected,
            );
            return;
        }
        if self.phase == Phase::Menu {
            ui::draw_menu(fb, &self.settings, &self.menu);
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
        ui::draw_hud(fb, &self.tanks[self.current], self.wind);

        match self.phase {
            Phase::RoundOver => ui::draw_standings(
                fb,
                &self.tanks,
                &self.prices,
                self.settings.economics.scoring,
                &format!("Round {} of {}", self.round, self.settings.rounds),
                &self.round_msg,
            ),
            Phase::GameOver => ui::draw_standings(
                fb,
                &self.tanks,
                &self.prices,
                self.settings.economics.scoring,
                "FINAL STANDINGS",
                "",
            ),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{aim_repeat_steps, display_angle};

    fn game() -> Game {
        let mut g = Game::new();
        g.settings.players = 3;
        g.start_game();
        for (i, t) in g.tanks.iter_mut().enumerate() {
            t.x = 100 + i as i32 * 200;
            t.y = 100;
        }
        g
    }

    fn projectile(weapon: ItemId, kind: PKind) -> Proj {
        Proj {
            pos: vec2(100.0, 80.0),
            vel: vec2(0.0, -1.0),
            owner: 0,
            weapon,
            kind,
            radius: weapon.item().radius,
            damage: weapon.item().damage(),
            age: 1.0,
            tunneling: false,
            in_dirt: false,
        }
    }

    #[test]
    fn aiming_repeat_uses_original_rates_at_different_frame_rates() {
        for fps in [30, 60, 144] {
            for shift in [false, true] {
                let mut elapsed = 0.0;
                let mut steps = aim_repeat_steps(&mut elapsed, true, true, 0.0, shift, false);
                for _ in 0..fps {
                    steps +=
                        aim_repeat_steps(&mut elapsed, false, true, 1.0 / fps as f32, shift, false);
                }
                assert_eq!(steps, if shift { 122 } else { 244 });
            }
        }
    }

    #[test]
    fn alt_only_changes_once_per_press_and_release_resets_repeat() {
        let mut elapsed = 0.0;
        assert_eq!(aim_repeat_steps(&mut elapsed, true, true, 0.016, false, true), 1);
        for _ in 0..100 {
            assert_eq!(aim_repeat_steps(&mut elapsed, false, true, 0.016, false, true), 0);
        }
        assert_eq!(aim_repeat_steps(&mut elapsed, false, false, 0.016, false, true), 0);
        assert_eq!(aim_repeat_steps(&mut elapsed, true, true, 0.016, false, true), 1);
        assert_eq!(elapsed, 0.0);
    }

    #[test]
    fn displayed_angles_mirror_about_vertical() {
        for (internal, shown) in [(0, 0), (35, 35), (90, 90), (145, 35), (180, 0)] {
            assert_eq!(display_angle(internal), shown);
        }
    }

    #[test]
    fn radial_damage_truncates_before_multiplying_and_only_caps_above_100() {
        assert_eq!(explosion_damage(5.05, 10.0, Damage::Weapon(0)), 49);
        assert_eq!(explosion_damage(10.1, 20.0, Damage::Weapon(1)), 98);
        assert_eq!(explosion_damage(10.0, 20.0, Damage::Weapon(1)), 100);
        assert_eq!(explosion_damage(9.8, 20.0, Damage::Weapon(1)), 110);
        assert_eq!(explosion_damage(20.0, 20.0, Damage::Weapon(1)), 0);
        assert_eq!(explosion_damage(21.0, 20.0, Damage::Weapon(1)), 0);
        assert_eq!(explosion_damage(0.0, 0.0, Damage::Weapon(1)), 0);
        assert_eq!(explosion_damage(0.0, 10.0, Damage::Weapon(0)), 100);
    }

    #[test]
    fn blast_measures_distance_to_base_and_preserves_lethal_hit_credit() {
        let mut g = game();
        g.tanks[1].x = 100;
        // Base at (100,100) is exactly on the edge; centre (100,96) is inside.
        g.apply_explosion(Explosion::new(vec2(100.0, 90.0), 10.0, Damage::Weapon(0), 0, false));
        assert_eq!(g.tanks[1].health, 100);
        g.apply_explosion(Explosion::new(vec2(100.0, 100.0), 10.0, Damage::Weapon(0), 0, false));
        assert_eq!(g.tanks[1].health, 0);
        assert_eq!(g.tanks[1].power, 0);
        g.apply_explosion(Explosion::new(vec2(100.0, 100.0), 10.0, Damage::Weapon(0), 2, false));
        assert_eq!(g.tanks[1].last_hit, Some(0));
    }

    #[test]
    fn damaging_direct_hit_is_lethal_and_still_explodes() {
        let mut g = game();
        // At the top of the hull, a Baby Missile blast alone would miss the base.
        let mut p = projectile(ItemId::BabyMissile, PKind::Shell);
        p.pos = vec2(300.0, 90.0);
        p.vel = vec2(0.0, 1.0);
        g.projs.push(p);
        g.step_projs(SIM_DT);
        assert_eq!(g.tanks[1].health, 0);
        assert_eq!(g.tanks[1].last_hit, Some(0));
        assert_eq!(g.expls.len(), 1);
    }

    #[test]
    fn harmless_weapons_and_unsplit_mirv_do_not_kill_on_contact() {
        for (weapon, kind, explosions) in [
            (ItemId::Tracer, PKind::Shell, 0),
            (ItemId::DirtBall, PKind::Shell, 1),
            (ItemId::Mirv, PKind::Carrier, 0),
        ] {
            for victim in [None, Some(1)] {
                let mut g = game();
                let mut spawn = Vec::new();
                let _ = weapons::impact(
                    projectile(weapon, kind),
                    vec2(300.0, 90.0),
                    victim,
                    &g.terrain,
                    &mut g.tanks,
                    &mut spawn,
                    &mut g.expls,
                );
                assert_eq!(g.tanks[1].health, 100);
                assert_eq!(g.expls.len(), explosions);
                assert!(spawn.is_empty());
            }
        }
    }

    #[test]
    fn mirv_splits_at_apex_into_five_missiles() {
        let mut g = game();
        g.projs.push(projectile(ItemId::Mirv, PKind::Carrier));
        g.step_projs(0.01);
        assert_eq!(g.projs.len(), 5);
        for p in &g.projs {
            assert!(p.kind == PKind::Bomblet);
            assert_eq!(p.weapon, ItemId::Missile);
            assert_eq!(p.radius, 20.0);
            assert!(matches!(p.damage, Damage::Weapon(1)));
        }
        let velocities: Vec<f32> = g.projs.iter().map(|p| p.vel.x).collect();
        for pair in velocities.windows(2) {
            assert!((pair[1] - pair[0] - 30.0).abs() < 0.001);
        }
    }

    #[test]
    fn turns_follow_positions_wrap_and_skip_dead_players() {
        let mut g = game();
        g.tanks[0].x = 500;
        g.tanks[1].x = 100;
        g.tanks[2].x = 300;
        assert_eq!(next_tank(&g.tanks, 0), 1);
        assert_eq!(next_tank(&g.tanks, 1), 2);
        assert_eq!(next_tank(&g.tanks, 2), 0);
        g.tanks[2].alive = false;
        assert_eq!(next_tank(&g.tanks, 1), 0);
        assert_eq!(next_tank(&g.tanks, 0), 1);
    }

    #[test]
    fn blast_sizes_and_health_limited_power_match_batch_zero() {
        let mut g = game();
        assert_eq!(ItemId::DirtBall.item().radius, 35.0);
        assert_eq!(ItemId::Roller.item().radius, ItemId::Missile.item().radius);
        assert!(matches!(ItemId::Roller.item().damage(), Damage::Weapon(13)));
        for health in [0, 1, 37, 100] {
            g.tanks[0].health = health;
            assert_eq!(g.tanks[0].max_power(), health * 10);
        }
    }

    #[test]
    fn spent_inventory_and_accessories_survive_rounds_but_reset_for_a_new_game() {
        let mut g = game();
        g.current = 0;
        g.tanks[0].weapon = ItemId::Missile;
        let initial = g.tanks[0].inventory.count(ItemId::Missile);
        g.fire();
        assert_eq!(g.projs.len(), 1);
        assert_eq!(g.projs[0].weapon, ItemId::Missile);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), initial - 1);
        g.tanks[0].inventory.add(ItemId::Battery, 25);
        g.tanks[1].inventory.remove(ItemId::Nuke, 1);
        g.tanks[1].alive = false;
        let stock: Vec<_> = g.tanks.iter().map(|t| t.inventory.clone()).collect();
        g.round += 1;
        g.start_round();
        for (tank, expected) in g.tanks.iter().zip(&stock) {
            assert_eq!(&tank.inventory, expected);
            assert!(tank.alive);
            assert_eq!(tank.health, 100);
        }
        assert_eq!(g.tanks[0].inventory.count(ItemId::BabyMissile), 99);
        g.start_game();
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), initial);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 0);
        assert_eq!(g.tanks[1].inventory.count(ItemId::Nuke), 1);
    }

    #[test]
    fn empty_or_unimplemented_items_cannot_fire_or_consume_stock() {
        let mut g = game();
        g.current = 0;
        g.tanks[0].inventory.remove(ItemId::Nuke, 1);
        for id in [ItemId::Nuke, ItemId::LeapFrog, ItemId::Battery] {
            if id != ItemId::Nuke {
                g.tanks[0].inventory.add(id, 2);
            }
            g.tanks[0].weapon = id;
            let before = g.tanks[0].inventory.clone();
            g.fire();
            assert!(g.projs.is_empty());
            assert_eq!(g.tanks[0].inventory, before);
            g.begin_turn();
            assert_eq!(g.tanks[0].weapon, ItemId::BabyMissile);
        }
        g.fire();
        assert_eq!(g.projs.len(), 1);
        assert_eq!(g.tanks[0].inventory.count(ItemId::BabyMissile), 99);
    }

    #[test]
    fn game_uses_owned_settings_and_keeps_them_between_rounds_and_games() {
        let mut g = Game::new();
        g.settings.players = 4;
        g.settings.rounds = 7;
        g.settings.controllers.fill(Ctrl::Human);
        g.settings.physics.gravity = 0.7;
        let settings = g.settings.clone();
        g.start_game();
        assert_eq!(g.tanks.len(), 4);
        assert!(g.tanks.iter().all(|t| t.ctrl == Ctrl::Human));
        g.start_round();
        g.start_game();
        assert_eq!(g.settings, settings);
    }

    #[test]
    fn ten_players_get_distinct_colors_positions_and_valid_round_state() {
        let mut g = Game::new();
        g.settings.players = 10;
        g.settings.rounds = 1000;
        g.start_game();
        assert_eq!(g.tanks.len(), 10);
        for (i, tank) in g.tanks.iter().enumerate() {
            assert!(tank.alive && tank.health == 100);
            assert!((20..W - 20).contains(&tank.x));
            assert!(g.tanks[..i]
                .iter()
                .all(|other| other.x != tank.x && other.color != tank.color));
        }
        assert!(g.current < 10);
        assert_eq!(g.settings.rounds, 1000);
    }

    #[test]
    fn hardware_delays_change_animation_speed_with_fixed_projectile_steps() {
        let mut fast = game();
        let mut slow = game();
        fast.wind = 0;
        slow.wind = 0;
        fast.settings.hardware.firing_delay = 50;
        slow.settings.hardware.firing_delay = 500;
        let mut shot = projectile(ItemId::BabyMissile, PKind::Shell);
        shot.vel = vec2(50.0, -100.0);
        fast.projs.push(shot);
        slow.projs.push(shot);
        fast.update_resolve(0.01);
        slow.update_resolve(0.01);
        assert!(fast.projs[0].pos.x > slow.projs[0].pos.x);
        slow.update_resolve(0.09);
        assert!((fast.projs[0].pos - slow.projs[0].pos).length() < 0.5);
        assert!((fast.projs[0].vel - slow.projs[0].vel).length() < 0.5);
        fast.settings.hardware.firing_delay = 0;
        fast.update_resolve(0.01);
        assert!(fast.projs.is_empty());
        for delay in [0, 10, 1000] {
            let mut g = game();
            g.settings.hardware.falling_delay = delay;
            for t in &mut g.tanks {
                t.alive = false;
            }
            let base = g.terrain.flatten(g.tanks[0].x, 9) - 1;
            g.tanks[0].alive = true;
            g.tanks[0].y = base - 20;
            g.update_falling(0.1);
            let drop = g.tanks[0].y - (base - 20);
            assert_eq!(
                drop,
                match delay {
                    0 => 20,
                    10 => 10,
                    _ => 0,
                }
            );
            // All speeds produce the same landing and damage.
            for _ in 0..220 {
                g.update_falling(0.1);
            }
            assert_eq!(g.tanks[0].y, base);
            assert_eq!(g.tanks[0].health, 60);
        }
    }

    #[test]
    fn custom_config_path_is_used_for_loading_and_saving_and_errors_are_visible() {
        let path =
            std::env::temp_dir().join(format!("scorched-game-config-{}.toml", std::process::id()));
        let mut settings = Settings::default();
        settings.players = 10;
        settings.save(&path).unwrap();
        let mut g = Game::new();
        g.load_settings(path.clone());
        assert_eq!(g.settings.players, 10);
        g.settings.rounds = 42;
        assert_eq!(Settings::load(&path).unwrap().rounds, 10);
        g.save_settings();
        assert_eq!(Settings::load(&path).unwrap().rounds, 42);
        assert!(g.menu.notice.as_ref().unwrap().contains("Settings saved"));
        std::fs::write(&path, "players = 999").unwrap();
        g.load_settings(path.clone());
        assert_eq!(g.settings, Settings::default());
        assert!(g.menu.notice.as_ref().unwrap().contains("Invalid settings"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "players = 999");
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn launch_power_uses_original_speed_units() {
        let mut g = game();
        g.current = 0;
        g.tanks[0].angle = 45;
        g.tanks[0].power = 1000;
        g.fire();
        assert!((g.projs[0].vel.length() - 1000.0).abs() < 0.001);
    }

    #[test]
    fn fall_damage_timing_changes_but_total_is_two_points_per_pixel() {
        for impact in [false, true] {
            let mut g = game();
            g.terrain = Terrain::flat(300);
            g.settings.physics.impact_damage = impact;
            for t in &mut g.tanks {
                t.y = 299;
            }
            g.tanks[0].y = 279;
            g.update_falling(0.05);
            assert_eq!(g.tanks[0].y, 284);
            assert_eq!(g.tanks[0].health, if impact { 100 } else { 90 });
            g.update_falling(0.2);
            assert_eq!(g.tanks[0].y, 299);
            assert_eq!(g.tanks[0].health, 60);
            assert_eq!(g.tanks[0].fall, 0);
            assert!(g.tanks[0].power <= g.tanks[0].max_power());
        }
        let mut g = game();
        g.terrain = Terrain::flat(300);
        g.settings.physics.tanks_fall = false;
        assert!(!g.update_falling(10.0));
        assert!(g.tanks.iter().all(|t| t.y == 100 && t.health == 100));
    }

    #[test]
    fn three_support_pixels_are_required_to_hold_a_tank() {
        let mut g = game();
        g.terrain = Terrain::flat(H);
        for dx in [0, 1] {
            g.terrain.add_dirt(g.tanks[0].x + dx, 101, 0, 0);
        }
        assert!(!g.tanks[0].supported(&g.terrain));
        g.terrain.add_dirt(g.tanks[0].x - 1, 101, 0, 0);
        assert!(g.tanks[0].supported(&g.terrain));
    }

    #[test]
    fn suspend_dirt_roll_is_per_shot_and_later_shots_can_collapse_old_dirt() {
        let mut g = game();
        g.current = 0;
        g.settings.physics.tanks_fall = false;
        g.settings.physics.suspend_dirt = 100;
        g.terrain = Terrain::flat(H);
        g.terrain.add_dirt(200, 200, 2, 0);
        g.fire();
        assert!(!g.settle_dirt);
        g.projs.clear();
        g.settings.physics.suspend_dirt = 0;
        g.update_resolve(0.1);
        assert!(g.terrain.solid(200, 200));
        assert!(!g.settle_dirt); // Changing the setting never rerolls a shot in progress.
        g.fire();
        assert!(g.settle_dirt);
        g.update_resolve(0.01);
        assert!(g.terrain.solid(200, 200)); // Wait for all projectiles and blasts.
        g.projs.clear();
        g.expls.clear();
        g.update_resolve(0.1);
        assert!(!g.terrain.solid(200, 200));
    }

    #[test]
    fn wind_changes_only_between_turns_when_enabled_and_respects_zero_limit() {
        let mut g = game();
        g.terrain = Terrain::flat(300);
        for t in &mut g.tanks {
            t.y = 299;
        }
        g.wind = 37;
        g.settings.landscape.changing_wind = false;
        g.update_resolve(0.1);
        assert_eq!(g.wind, 37);
        g.settings.landscape.changing_wind = true;
        g.wind = 199;
        g.update_resolve(0.1);
        assert!((194..=200).contains(&g.wind));
        let wind = g.wind;
        g.begin_turn(); // Preparing aim does not apply a second wind change.
        assert_eq!(g.wind, wind);
        g.settings.landscape.max_wind = 0;
        g.update_resolve(0.1);
        assert_eq!(g.wind, 0);
        g.start_round();
        assert_eq!(g.wind, 0);
    }

    #[test]
    fn border_margin_allows_wind_to_return_a_shot_and_discards_shots_beyond_it() {
        let mut g = game();
        g.terrain = Terrain::flat(H);
        g.settings.physics.gravity = 0.05;
        g.wind = 500;
        let mut shot = projectile(ItemId::BabyMissile, PKind::Shell);
        shot.pos = vec2(-1.0, 40.0);
        shot.vel = vec2(-20.0, -10.0);
        g.projs.push(shot);
        for _ in 0..600 {
            g.step_projs(SIM_DT);
            assert_eq!(g.projs.len(), 1);
            if g.projs[0].pos.x >= 0.0 {
                break;
            }
        }
        assert!(g.projs[0].pos.x >= 0.0);
        assert!(g.trail.iter().all(|&(x, y, _)| (0..W).contains(&x) && (0..H).contains(&y)));
        g.projs.clear();
        g.settings.physics.borders_extend = 0;
        g.projs.push(shot);
        g.step_projs(SIM_DT);
        assert!(g.projs.is_empty());
        g.settings.physics.borders_extend = 75;
        shot.pos.x = -76.0;
        g.projs.push(shot);
        g.step_projs(SIM_DT);
        assert!(g.projs.is_empty());
        g.settings.physics.borders_extend = 10000;
        shot.pos.x = -500.0;
        g.projs.push(shot);
        g.step_projs(SIM_DT);
        assert_eq!(g.projs.len(), 1);
    }

    #[test]
    fn fired_shots_and_mirv_children_keep_their_tunneling_setting() {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(H);
        g.tanks[0].weapon = ItemId::Mirv;
        g.fire();
        assert!(g.projs[0].tunneling);
        g.projs[0].pos = vec2(100.0, 80.0);
        g.projs[0].vel = vec2(0.0, -1.0);
        g.settings.weapons.tunneling = false;
        g.step_projs(0.01);
        assert_eq!(g.projs.len(), 5);
        assert!(g.projs.iter().all(|p| p.tunneling && !p.in_dirt));
        g.projs.clear();
        g.fire();
        assert!(!g.projs[0].tunneling);
    }

    #[test]
    fn a_shot_detonates_inside_a_hillside_and_carves_a_buried_crater() {
        let mut g = game();
        g.terrain = Terrain::flat(250);
        g.current = 0;
        let mut shot = projectile(ItemId::BabyMissile, PKind::Shell);
        shot.pos = vec2(200.25, 240.25);
        shot.vel = vec2(0.0, 900.0);
        shot.tunneling = true;
        g.projs.push(shot);
        for _ in 0..600 {
            g.step_projs(SIM_DT);
            if g.projs.is_empty() {
                break;
            }
        }
        assert!(g.projs.is_empty());
        assert_eq!(g.expls.len(), 1);
        let explosion = &g.expls[0];
        assert!(explosion.pos.y >= 260.0, "explosion at surface: {}", explosion.pos.y);
        let center = (explosion.pos.x as i32, explosion.pos.y as i32);
        assert!(g.terrain.solid(center.0 + 2, center.1));
        if let Ok(path) = std::env::var("SCORCHED_TUNNEL_PREVIEW_PATH") {
            for t in &mut g.tanks {
                t.y = 249;
            }
            g.expls[0].t = g.expls[0].dur * 0.5;
            g.phase = Phase::Resolve;
            let mut fb = Fb::new();
            g.draw(&mut fb);
            fb.img.bytes =
                fb.img.bytes.chunks_exact((W * 4) as usize).rev().flatten().copied().collect();
            fb.img.export_png(&path);
        }
        let explosion = g.expls.pop().unwrap();
        g.apply_explosion(explosion);
        assert!(!g.terrain.solid(center.0 + 2, center.1));
    }

    #[test]
    fn ten_round_games_pay_damage_kills_survival_and_interest_once_in_each_mode() {
        use crate::economy::Money;
        use crate::settings::ScoringMode;
        for mode in [ScoringMode::Basic, ScoringMode::Standard, ScoringMode::Greedy] {
            let mut g = Game::new();
            g.settings.economics.scoring = mode;
            g.settings.physics.tanks_fall = false;
            g.settings.hardware.firing_delay = 0;
            g.start_game();
            let mut expected = 0u128;
            let mut interest = 0u128;
            for round in 1..=10 {
                if round > 1 {
                    let earned = expected * 500 / 10000;
                    interest += earned;
                    expected += earned;
                    g.round = round;
                    g.start_round();
                    g.start_round(); // Preparing the same round cannot pay interest twice.
                }
                g.terrain = Terrain::flat(H);
                g.tanks[0].x = 100;
                g.tanks[1].x = 500;
                for t in &mut g.tanks {
                    t.y = 100;
                }
                let mut shot = projectile(ItemId::BabyMissile, PKind::Shell);
                shot.pos = vec2(500.0, 90.0);
                shot.vel = vec2(0.0, 1.0);
                g.projs.push(shot);
                g.step_projs(SIM_DT);
                assert_eq!(
                    g.tanks[0].account.cash.to_string(),
                    (expected + if mode == ScoringMode::Basic { 0 } else { 3000 }).to_string()
                );
                for _ in 0..20 {
                    g.update_resolve(0.1);
                    if g.phase == Phase::RoundOver {
                        break;
                    }
                }
                assert!(g.phase == Phase::RoundOver);
                expected += if mode == ScoringMode::Basic { 6000 } else { 8500 };
                assert_eq!(g.tanks[0].account.cash.to_string(), expected.to_string());
                assert_eq!(g.tanks[0].account.interest_earned.to_string(), interest.to_string());
                assert_eq!(g.tanks[0].wins, round);
                assert_eq!(g.tanks[0].kills, round as i32);
                g.finish_round(&[0]);
                assert_eq!(g.tanks[0].account.cash.to_string(), expected.to_string());
                assert_eq!(economy::ranking(&g.tanks, mode, &g.prices), vec![0, 1]);
                assert_eq!(g.tanks[1].account.cash, Money::default());
            }
        }
    }

    #[test]
    fn friendly_damage_and_kills_deduct_cash_and_never_make_it_negative() {
        use crate::economy::Account;
        use crate::settings::ScoringMode;
        for mode in [ScoringMode::Basic, ScoringMode::Standard] {
            for victim in [0, 1] {
                let mut g = game();
                g.terrain = Terrain::flat(H);
                g.settings.economics.scoring = mode;
                g.settings.physics.tanks_fall = false;
                g.settings.hardware.firing_delay = 0;
                g.tanks[0].account = Account::new(10000);
                g.tanks[0].team = Some(1);
                g.tanks[1].team = Some(1);
                let pos = vec2(g.tanks[victim].x as f32, g.tanks[victim].y as f32);
                g.apply_explosion(Explosion::new(pos, 20.0, Damage::Weapon(1), 0, false));
                // A 110-point blast against 100 health pays/deducts only 100 health.
                let damage_penalty = if mode == ScoringMode::Basic { 0 } else { 1500 };
                assert_eq!(
                    g.tanks[0].account.cash,
                    crate::economy::Money::from(10000 - damage_penalty)
                );
                g.update_resolve(0.1);
                let kill_penalty = if victim == 0 { 1500 } else { 2000 };
                assert_eq!(
                    g.tanks[0].account.cash,
                    crate::economy::Money::from(10000 - damage_penalty - kill_penalty)
                );
                let before = g.tanks[0].account.cash.clone();
                g.update_resolve(0.1);
                assert_eq!(g.tanks[0].account.cash, before);
            }
        }
        let mut g = game();
        g.apply_explosion(Explosion::new(vec2(100.0, 100.0), 20.0, Damage::Weapon(1), 0, false));
        assert!(g.tanks[0].account.cash.is_zero());
    }

    #[test]
    fn fall_scoring_matches_both_damage_timings_and_credits_the_shooter() {
        for impact in [true, false] {
            let mut g = game();
            g.terrain = Terrain::flat(300);
            g.action_owner = Some(0);
            g.settings.physics.impact_damage = impact;
            for t in &mut g.tanks {
                t.y = 299;
            }
            g.tanks[1].y = 279;
            g.update_falling(0.05);
            assert_eq!(
                g.tanks[0].account.cash,
                crate::economy::Money::from(if impact { 0 } else { 300 })
            );
            g.update_falling(0.2);
            assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(1200));
            assert_eq!(g.tanks[1].last_hit, Some(0));
        }
    }

    #[test]
    fn survival_pools_split_and_all_dead_rounds_do_not_pay() {
        use crate::settings::ScoringMode;
        for mode in [ScoringMode::Basic, ScoringMode::Standard] {
            let mut g = game();
            g.settings.economics.scoring = mode;
            g.finish_round(&[0, 1, 2]);
            let share = if mode == ScoringMode::Basic { 1000 } else { 1666 };
            assert!(g.tanks.iter().all(|t| t.account.cash == crate::economy::Money::from(share)));
            g.finish_round(&[0, 1, 2]);
            assert!(g.tanks.iter().all(|t| t.account.cash == crate::economy::Money::from(share)));
            let mut g = game();
            g.finish_round(&[]);
            assert!(g.tanks.iter().all(|t| t.account.cash.is_zero()));
        }
    }

    #[test]
    fn greedy_ranks_inventory_wealth_while_other_modes_rank_cash() {
        use crate::economy::{Account, Money};
        use crate::settings::ScoringMode;
        let mut g = game();
        for t in &mut g.tanks {
            t.inventory = Inventory::default();
        }
        g.tanks[0].account = Account::new(1000);
        g.tanks[1].account = Account::new(900);
        g.tanks[1].inventory.add(ItemId::Nuke, 1);
        assert_eq!(economy::ranking(&g.tanks, ScoringMode::Basic, &g.prices), vec![0, 1, 2]);
        assert_eq!(economy::ranking(&g.tanks, ScoringMode::Standard, &g.prices), vec![0, 1, 2]);
        assert_eq!(economy::ranking(&g.tanks, ScoringMode::Greedy, &g.prices), vec![1, 0, 2]);
        assert_eq!(economy::net_worth(&g.tanks[1], &g.prices), Money::from(900 + 3168 + 9600));
        g.prices[ItemId::Nuke as usize] = 0;
        assert_eq!(economy::ranking(&g.tanks, ScoringMode::Greedy, &g.prices), vec![0, 1, 2]);
    }

    #[test]
    fn opening_purchases_precede_interest_and_computers_buy_can_be_disabled() {
        use crate::economy::Money;
        let mut g = Game::new();
        g.settings.players = 3;
        g.settings.controllers[2] = Ctrl::Human;
        g.settings.economics.cash_at_start = 20000;
        g.settings.economics.computers_buy = false;
        g.start_game();
        assert!(g.phase == Phase::OpeningShop);
        assert_eq!(g.opening_shop.player, 0);
        assert_eq!(g.tanks[0].account.cash, Money::from(20000));
        assert!(shop::buy(&mut g.tanks[0], ItemId::Missile, &g.prices));
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), 15);
        g.opening_shop.player += 1;
        g.advance_opening_shop();
        assert_eq!(g.opening_shop.player, 2);
        assert_eq!(g.tanks[1].inventory, Inventory::starter());
        g.opening_shop.player += 1;
        g.advance_opening_shop();
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].account.cash, Money::from(19031));
        assert_eq!(g.tanks[1].account.cash, Money::from(21000));
        assert_eq!(g.tanks[2].account.cash, Money::from(21000));
        g.start_game();
        assert_eq!(g.tanks[0].account.cash, Money::from(20000));
        assert_eq!(g.tanks[0].inventory, Inventory::starter());
        let mut g = Game::new();
        g.settings.controllers.fill(Ctrl::Ai(crate::ai::AiKind::Shooter));
        g.settings.economics.cash_at_start = 20000;
        g.start_game();
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].account.cash, Money::from(131));
        assert_eq!(g.tanks[0].inventory.count(ItemId::Nuke), 2);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), 15);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Roller), 9);
    }

    #[test]
    fn opening_shop_rejects_unavailable_or_overstocked_purchases_without_charging() {
        let mut g = game();
        let prices = g.prices;
        let tank = &mut g.tanks[0];
        assert!(shop::offers(tank, &prices).is_empty());
        tank.account = crate::economy::Account::new(100000);
        assert!(!shop::buy(tank, ItemId::BabyMissile, &prices));
        assert!(!shop::buy(tank, ItemId::Battery, &prices));
        assert!(!shop::buy(tank, ItemId::LeapFrog, &prices));
        tank.inventory.add(ItemId::Missile, 89);
        let cash = tank.account.cash.clone();
        assert!(!shop::buy(tank, ItemId::Missile, &prices));
        assert_eq!(tank.account.cash, cash);
        assert_eq!(tank.inventory.count(ItemId::Missile), 99);
    }

    #[test]
    fn economic_screens_render_for_ten_players_and_large_balances() {
        use crate::settings::ScoringMode;
        let mut g = Game::new();
        g.settings.players = 10;
        g.settings.controllers.fill(Ctrl::Human);
        g.settings.economics.cash_at_start = 100000;
        g.start_game();
        let mut fb = Fb::new();
        g.draw(&mut fb);
        let directory = std::env::var("SCORCHED_ECONOMY_PREVIEW_DIR").ok();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes =
                    image.bytes.chunks_exact((W * 4) as usize).rev().flatten().copied().collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        save(&fb, "opening-shop");
        let mut huge = crate::economy::Money::from(1);
        for _ in 0..200 {
            huge = huge.scaled(2, 1);
        }
        g.tanks[9].account.cash = huge.clone();
        assert!(crate::economy::money_text(&huge).len() <= 14);
        for mode in [ScoringMode::Basic, ScoringMode::Standard, ScoringMode::Greedy] {
            g.settings.economics.scoring = mode;
            g.phase = Phase::RoundOver;
            g.round_msg = "Player 10 wins the round!".into();
            g.draw(&mut fb);
            save(&fb, &format!("standings-{mode:?}"));
            g.phase = Phase::GameOver;
            g.draw(&mut fb);
            assert_eq!(g.tanks[9].account.cash, huge);
        }
    }

    #[test]
    fn render_batch_zero_preview() {
        let mut g = game();
        g.tanks[0].name = "Spoiler 1".into();
        g.tanks[0].angle = 145;
        g.tanks[0].power = 345;
        g.tanks[0].weapon = ItemId::DirtBall;
        g.current = 0;
        g.wind = -7;
        for t in &mut g.tanks {
            t.y = g.terrain.surface(t.x) - 1;
        }
        let mut fb = Fb::new();
        g.draw(&mut fb);
        if let Ok(path) = std::env::var("SCORCHED_PREVIEW_PATH") {
            // export_png expects bottom-up rows; our software framebuffer is top-down.
            fb.img.bytes =
                fb.img.bytes.chunks_exact((W * 4) as usize).rev().flatten().copied().collect();
            fb.img.export_png(&path);
        }
    }
}
