//! Game state, rules, physics and drawing.

use crate::ai;
use crate::controls::{self, Action as ControlAction, Controls};
use crate::earth;
use crate::economy::{self, DamageEvent, DamageTarget, Relation};
use crate::gfx::{text_w, Fb, Rgb, H, W};
use crate::inventory::Inventory;
use crate::physics::{self, Physics, SIM_DT};
use crate::settings::{PlayMode, Settings, TeamMode, WallType};
use crate::shop::{self, Market, Shop};
use crate::sound::{self, Cue};
use crate::tank::{Ctrl, Tank, PLAYER_COLORS};
use crate::terrain::Terrain;
use crate::ui::{self, draw_explosion, draw_tank, KeyRep, MenuAction, MenuState, HUD_H};
use crate::weapons::{
    self, explosion_damage, Damage, Explosion, ItemId, ItemKind, PKind, Proj, Step, WKind,
};
use macroquad::prelude::{
    get_last_key_pressed, is_key_down, is_key_pressed, is_mouse_button_down,
    is_mouse_button_pressed, mouse_position, screen_height, screen_width, vec2, KeyCode,
    MouseButton,
};
use macroquad::rand::{gen_range, ChooseRandom};

const SETTLE_RATE: f32 = 240.0; // dirt fall, px/s

struct Bubble {
    text: String,
    x: i32,
    y: i32,
    t: f32,
}

/// Keep player identities stable while cycling through battlefield positions.
#[cfg(test)]
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
    Shop,
    Defense,
    Aim,
    Batteries,
    Target,
    Move,
    MoveFall,
    Resolve,
    Simultaneous,
    Retreat,
    RoundOver,
    GameOver,
}

pub struct Game {
    concurrent: crate::modes::Battle,
    system: Option<crate::system::System>,
    edit_settings: Option<Settings>,
    edit_menu: MenuState,
    turn_order: crate::play::Order,
    vicious_split: bool,
    speech: crate::speech::Speech,
    phase: Phase,
    phase_t: f32,
    menu: MenuState,
    controls: Controls,
    sounds: sound::Requests,
    defense_player: usize,
    config_path: Option<std::path::PathBuf>,
    save_filename: String,
    hall: crate::hall_of_fame::HallOfFame,
    hall_path: Option<std::path::PathBuf>,
    hall_notice: Option<String>,
    hall_open: bool,
    hall_mode: crate::settings::ScoringMode,
    settings: Settings,
    round: u32,
    interest_round: u32,
    round_paid: bool,
    action_owner: Option<usize>,
    environment_action: bool,
    resume_aim_after_weather: bool,
    resume_aim_after_movement: bool,
    weather: crate::environment::Weather,
    prices: [u32; crate::weapons::ITEM_COUNT],
    shop: Shop,
    market: Market,
    market_path: Option<std::path::PathBuf>,
    shop_notice: Option<String>,
    terrain: Terrain,
    landscape_reverse: bool,
    tanks: Vec<Tank>,
    projs: Vec<Proj>,
    earth: earth::Effects,
    liquid: crate::liquid::Effects,
    energy: crate::energy::Effect,
    battery_input: Option<u8>,
    battle_mouse: ui::BattleMouse,
    expls: Vec<Explosion>,
    trail: Vec<(i32, i32, Rgb)>,
    persistent_traces: crate::gfx::TraceLayer,
    bubbles: Vec<Bubble>,
    current: usize,
    wind: i32,
    active_walls: WallType,
    sim_acc: f32,
    settle_acc: f32,
    settle_dirt: bool,
    ai_plan: Option<ai::Plan>,
    ai_t: f32,
    keys: KeyRep,
    time: f32,
    round_msg: String,
}

impl Game {
    pub fn new() -> Self {
        Game {
            concurrent: crate::modes::Battle::default(),
            system: None,
            edit_settings: None,
            edit_menu: MenuState::default(),
            turn_order: crate::play::Order::default(),
            vicious_split: false,
            speech: crate::speech::Speech::default(),
            phase: Phase::Menu,
            phase_t: 0.0,
            menu: MenuState::default(),
            controls: Controls::default(),
            sounds: sound::Requests::default(),
            defense_player: 0,
            config_path: None,
            save_filename: "game.toml".into(),
            hall: crate::hall_of_fame::HallOfFame::default(),
            hall_path: None,
            hall_notice: None,
            hall_open: false,
            hall_mode: crate::settings::ScoringMode::Standard,
            settings: Settings::default(),
            round: 1,
            interest_round: 0,
            round_paid: false,
            action_owner: None,
            environment_action: false,
            resume_aim_after_weather: false,
            resume_aim_after_movement: false,
            weather: crate::environment::Weather::new(crate::settings::Sky::Plain, 1, ui::HUD_H),
            prices: economy::base_prices(),
            shop: Shop::default(),
            market: Market::default(),
            market_path: None,
            shop_notice: None,
            terrain: Terrain::generate(),
            landscape_reverse: false,
            tanks: Vec::new(),
            projs: Vec::new(),
            earth: earth::Effects::default(),
            liquid: crate::liquid::Effects::default(),
            energy: crate::energy::Effect::default(),
            battery_input: None,
            battle_mouse: ui::BattleMouse::default(),
            expls: Vec::new(),
            trail: Vec::new(),
            persistent_traces: crate::gfx::TraceLayer::default(),
            bubbles: Vec::new(),
            current: 0,
            wind: 0,
            active_walls: WallType::None,
            sim_acc: 0.0,
            settle_acc: 0.0,
            settle_dirt: true,
            ai_plan: None,
            ai_t: 0.0,
            keys: KeyRep {
                held: Vec::new(),
                dt: 0.0,
            },
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
        let hall_path = path.with_file_name("hall_of_fame.toml");
        match crate::hall_of_fame::HallOfFame::load(&hall_path) {
            Ok(hall) => {
                self.hall = hall;
                self.hall_path = Some(hall_path);
                self.hall_notice = None;
            }
            Err(error) => {
                eprintln!("{error}");
                self.hall_path = None;
                self.hall_notice = Some(format!("{error}. Scores kept for this session only."));
            }
        }
        self.config_path = Some(path);
        self.market_path = dirs::data_local_dir().map(|p| p.join("scorched/market.toml"));
        if let Some(path) = &self.market_path {
            match Market::load(path) {
                Ok(market) => self.market = market,
                Err(error) => {
                    eprintln!("{error}");
                    self.menu.notice = Some(format!("{error}\nUsing base market prices."));
                }
            }
        }
    }

    pub fn fullscreen(&self) -> bool {
        self.settings.hardware.fullscreen
    }

    pub fn toggle_fullscreen(&mut self) {
        self.settings.hardware.fullscreen = !self.settings.hardware.fullscreen;
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

    fn fast_scale(&self) -> f32 {
        let live: Vec<_> = self
            .tanks
            .iter()
            .filter(|t| t.alive && t.health > 0)
            .collect();
        if self.settings.play.fast_computers
            && self.system.is_none()
            && matches!(
                self.phase,
                Phase::Aim
                    | Phase::Resolve
                    | Phase::Retreat
                    | Phase::MoveFall
                    | Phase::Simultaneous
            )
            && live.len() > 1
            && live.iter().all(|t| t.ctrl != Ctrl::Human)
        {
            6.0
        } else {
            1.0
        }
    }
    fn speak(&mut self, player: usize, death: bool) {
        let count = if death {
            self.speech.death.len()
        } else {
            self.speech.attack.len()
        };
        if count == 0 {
            return;
        }
        let choice = gen_range(0, count);
        if let Some(text) = self.speech.phrase(
            self.tanks[player].ctrl,
            &self.settings.play,
            death,
            gen_range(0, 100),
            choice,
        ) {
            let (x, y) = (self.tanks[player].x, self.tanks[player].y);
            self.say(text, x, y);
        }
    }
    fn apply_team_assignments(&mut self) {
        for (i, tank) in self.tanks.iter_mut().enumerate() {
            tank.team = if self.settings.play.teams == TeamMode::None {
                None
            } else {
                Some(self.settings.team_ids[i])
            };
        }
        economy::configure_banks(
            &mut self.tanks,
            self.settings.play.teams == TeamMode::Corporate,
        );
        self.vicious_split = false;
        self.ai_plan = None;
    }
    fn mass_kill(&mut self) {
        if self.round_paid {
            return;
        }
        let share = crate::play::mass_kill_share(&self.tanks, self.settings.economics.scoring);
        for survivor in crate::play::alive(&self.tanks) {
            economy::add_cash(&mut self.tanks, survivor, &share);
        }
        for tank in &mut self.tanks {
            tank.alive = false;
            tank.health = 0;
            tank.equipment.active_shield = None;
            tank.last_hit = None;
            tank.reset_descent();
        }
        self.projs.clear();
        self.expls.clear();
        self.earth = earth::Effects::default();
        self.liquid = crate::liquid::Effects::default();
        self.energy = crate::energy::Effect::default();
        self.concurrent.reset(self.tanks.len());
        self.controls = Controls::default();
        self.action_owner = None;
        self.ai_plan = None;
        self.finish_round(&[]);
        self.round_msg = format!("Mass Kill: {} per survivor", economy::money_text(&share));
    }
    fn stage_players(&mut self) {
        let mut settings = self.settings.clone();
        settings.players = self.tanks.len();
        for (i, tank) in self.tanks.iter().enumerate() {
            settings.names[i] = tank.name.clone();
            settings.controllers[i] = tank.ctrl;
            settings.shapes[i] = tank.shape;
        }
        self.edit_settings = Some(settings);
        self.edit_menu = MenuState {
            page: Some(ui::Page::Players),
            reassign_only: true,
            ..Default::default()
        };
    }
    fn apply_players(&mut self) -> bool {
        let Some(settings) = self.edit_settings.take() else {
            return false;
        };
        if let Err(error) = settings.validate() {
            self.edit_settings = Some(settings);
            self.edit_menu.notice = Some(error);
            return false;
        }
        self.settings.keys = settings.keys;
        let mut rename = Vec::new();
        let actor_changed = self.tanks[self.current].ctrl != settings.controllers[self.current];
        for (i, tank) in self.tanks.iter_mut().enumerate() {
            if tank.ctrl == Ctrl::Human
                && settings.controllers[i] != Ctrl::Human
                && tank.name == settings.names[i]
            {
                rename.push(i);
            }
            tank.name = settings.names[i].trim().to_string();
            if tank.ctrl != settings.controllers[i] {
                if let Some(plan) = self.concurrent.ai.get_mut(i) {
                    *plan = None;
                }
                if let Some(time) = self.concurrent.ai_time.get_mut(i) {
                    *time = 0.0;
                }
                tank.ctrl = settings.controllers[i];
                let grudges = tank.ai_state.grudges;
                tank.ai_state = ai::State::new(tank.ctrl);
                tank.ai_state.grudges = grudges;
                if tank.ctrl == Ctrl::Human && tank.shape == crate::tank::Shape::Triple {
                    tank.shape = crate::tank::Shape::Treads;
                }
            }
            self.settings.names[i] = tank.name.clone();
            self.settings.controllers[i] = tank.ctrl;
            self.settings.shapes[i] = tank.shape;
        }
        for i in rename {
            let mut names = self.tanks.clone();
            for (j, tank) in names.iter_mut().enumerate() {
                if j != i {
                    tank.ctrl = Ctrl::Human;
                }
            }
            ai::name_computers(&mut names);
            self.tanks[i].name = names[i].name.clone();
            self.settings.names[i] = names[i].name.clone();
        }
        self.ai_plan = None;
        if actor_changed {
            self.controls = Controls::default();
            if self.tanks[self.current].ctrl != Ctrl::Human {
                match self.phase {
                    Phase::Target => self.fire(),
                    Phase::Batteries => self.discharge_energy(10),
                    Phase::Move => self.end_move(),
                    _ => {}
                }
            }
        }
        if self.phase == Phase::Shop {
            self.advance_shop();
        }
        if self.phase == Phase::Defense && self.tanks[self.defense_player].ctrl != Ctrl::Human {
            ai::prepare(&mut self.tanks[self.defense_player]);
            self.defense_player += 1;
            self.advance_defense();
        }
        true
    }
    fn system_input(&mut self, input: &ui::MenuInput) -> bool {
        use crate::system::{Command, Panel, Request};
        let Some(mut system) = self.system.take() else {
            return true;
        };
        system.can_save = self.phase == Phase::Shop;
        if let Panel::File { command, mut path } = system.panel.clone() {
            if input.escape || ui::button_index(input, 2) == Some(1) {
                system.panel = Panel::Menu;
            } else {
                if input.home {
                    path.clear();
                }
                if input.backspace {
                    path.pop();
                }
                for ch in input.text.chars().filter(|c| !c.is_control()) {
                    if path.len() + ch.len_utf8() <= 1024 {
                        path.push(ch);
                    }
                }
                system.panel = Panel::File {
                    command,
                    path: path.clone(),
                };
                if input.submit || ui::button_index(input, 2) == Some(0) {
                    match self.game_file_path(&path) {
                        Err(error) => system.panel = Panel::Message(error),
                        Ok(file) => {
                            self.save_filename = path.trim().into();
                            if command == Command::Save && file.exists() {
                                system.panel = Panel::Overwrite {
                                    path: self.save_filename.clone(),
                                };
                            } else {
                                system.panel =
                                    Panel::Message(self.game_file_action(command, &file));
                            }
                        }
                    }
                }
            }
            system.can_save = self.phase == Phase::Shop;
            self.system = Some(system);
            return true;
        }
        if let Panel::Overwrite { path } = &system.panel {
            if input.escape || input.hotkey == Some('N') || ui::button_index(input, 2) == Some(1) {
                system.panel = Panel::Menu;
            } else if input.hotkey == Some('Y') || ui::button_index(input, 2) == Some(0) {
                let result = match self.game_file_path(path) {
                    Ok(file) => self.game_file_action(Command::Save, &file),
                    Err(error) => error,
                };
                system.panel = Panel::Message(result);
            }
            self.system = Some(system);
            return true;
        }
        if system.panel == Panel::Players {
            let action = self
                .edit_menu
                .handle(self.edit_settings.as_mut().unwrap(), input);
            if action == MenuAction::Start {
                if self.apply_players() {
                    system.panel = Panel::Menu;
                }
            } else if self.edit_menu.page.is_none() {
                self.edit_settings = None;
                system.panel = Panel::Menu;
            }
            self.system = Some(system);
            return true;
        }
        if system.panel == Panel::Teams {
            if input.escape {
                self.edit_settings = None;
                system.panel = Panel::Menu;
                self.system = Some(system);
                return true;
            }
            let settings = self.edit_settings.as_mut().unwrap();
            let count = settings.players + 2;
            if input.up {
                system.row = (system.row + count - 1) % count;
            }
            if input.down {
                system.row = (system.row + 1) % count;
            }
            if let Some(digit) = input.hotkey.and_then(|c| c.to_digit(10)) {
                let index = if digit == 0 { 9 } else { digit as usize - 1 };
                if index < settings.players {
                    system.row = index + 1;
                }
            }
            let mut clicked = false;
            if let Some((x, y, _)) = input.click {
                let top = (H - (72 + count as i32 * 24)) / 2;
                if (52..588).contains(&x) && (top + 32..top + 32 + count as i32 * 24).contains(&y) {
                    system.row = ((y - top - 32) / 24) as usize;
                    clicked = true;
                }
            }
            let commit = input.hotkey == Some('D')
                || (system.row + 1 == count && (input.activate || clicked));
            if commit {
                self.settings.play.teams = settings.play.teams;
                self.settings.team_ids = settings.team_ids;
                self.edit_settings = None;
                self.apply_team_assignments();
                if !self.round_paid {
                    if let Some(winners) = crate::play::outcome(
                        &mut self.tanks,
                        self.settings.play.teams,
                        &mut self.vicious_split,
                    ) {
                        self.finish_round(&winners);
                    }
                }
                system.panel = Panel::Menu;
                system.row = 0;
            } else if input.left || input.right || input.activate || clicked {
                let direction = if input.left || input.alt || input.click.is_some_and(|c| c.2 < 0) {
                    -1
                } else {
                    1
                };
                if system.row == 0 {
                    let modes = [
                        TeamMode::None,
                        TeamMode::Standard,
                        TeamMode::Corporate,
                        TeamMode::Vicious,
                    ];
                    let i = modes
                        .iter()
                        .position(|m| *m == settings.play.teams)
                        .unwrap();
                    settings.play.teams = modes[(i as i32 + direction).rem_euclid(4) as usize];
                } else if system.row <= settings.players {
                    let team = &mut settings.team_ids[system.row - 1];
                    *team = (i32::from(*team) + direction).rem_euclid(10) as u8;
                }
            }
            self.system = Some(system);
            return true;
        }
        let game = !self.tanks.is_empty() && self.phase != Phase::Menu;
        let active = game
            && !self.round_paid
            && !matches!(self.phase, Phase::Shop | Phase::GameOver | Phase::RoundOver);
        match system.handle(input, game, active) {
            Request::Close => {
                self.edit_settings = None;
                return true;
            }
            Request::None => {}
            Request::Run(command) => match command {
                Command::Clear => self.clear_traces(),
                Command::Sound => self.settings.sound.enabled = !self.settings.sound.enabled,
                Command::Players => {
                    self.stage_players();
                    system.panel = Panel::Players;
                }
                Command::Teams => {
                    self.edit_settings = Some(self.settings.clone());
                    system.panel = Panel::Teams;
                    system.row = 0;
                }
                Command::MassKill => {
                    self.mass_kill();
                    return true;
                }
                Command::NewGame => {
                    self.start_game();
                    return true;
                }
                Command::Quit => return false,
                Command::Save | Command::Restore => {
                    system.panel = Panel::File {
                        command,
                        path: self.save_filename.clone(),
                    };
                }
            },
        }
        self.system = Some(system);
        true
    }

    fn game_file_path(&self, name: &str) -> Result<std::path::PathBuf, String> {
        let path = std::path::Path::new(name.trim());
        if name.trim().is_empty() || path.file_name().is_none() {
            return Err("Enter a saved-game filename.".into());
        }
        if path.is_absolute() {
            return Ok(path.into());
        }
        let base = self
            .config_path
            .as_deref()
            .and_then(|p| p.parent())
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."));
        Ok(base.join("saves").join(path))
    }

    fn checkpoint(&self) -> Result<crate::saved_game::SavedGame, String> {
        if self.phase != Phase::Shop {
            return Err("Save only from the buying screen.".into());
        }
        let saved = crate::saved_game::SavedGame::capture(
            &self.tanks,
            &self.settings,
            self.round,
            self.interest_round,
            self.shop.player,
            &self.turn_order.circle,
            &self.market,
        );
        saved.validate()?;
        Ok(saved)
    }

    fn restore_game(&mut self, path: &std::path::Path) -> Result<(), String> {
        let saved = crate::saved_game::SavedGame::load(path)?;
        // Build the complete replacement before discarding the current world.
        let mut game = Self::new();
        game.config_path = self.config_path.clone();
        game.market_path = self.market_path.clone();
        game.save_filename = self.save_filename.clone();
        game.hall = self.hall.clone();
        game.hall_path = self.hall_path.clone();
        game.hall_notice = self.hall_notice.clone();
        game.settings = self.settings.clone();
        game.landscape_reverse = self.landscape_reverse;
        game.settings.players = saved.players.len();
        game.settings.rounds = saved.rounds;
        for (i, p) in saved.players.iter().enumerate() {
            game.settings.names[i] = p.name.clone();
            game.settings.controllers[i] = p.ctrl;
            game.settings.shapes[i] = p.shape;
            game.settings.team_ids[i] = p.team;
        }
        game.settings.validate()?;
        game.tanks = saved
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| p.tank(i))
            .collect();
        game.apply_team_assignments();
        game.round = saved.round;
        game.interest_round = saved.interest_round;
        game.round_paid = saved.round > 1;
        game.market = saved.market;
        game.turn_order.circle = saved.circle;
        game.refresh_prices();
        game.shop.player = saved.buyer;
        game.set_phase(Phase::Shop);
        let base = game
            .config_path
            .as_deref()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."));
        let (speech, errors) = crate::speech::Speech::load(&game.settings.play, base);
        game.speech = speech;
        if !errors.is_empty() {
            game.shop_notice = Some(errors.join("\n"));
        }
        *self = game;
        Ok(())
    }

    fn game_file_action(
        &mut self,
        command: crate::system::Command,
        path: &std::path::Path,
    ) -> String {
        use crate::system::Command;
        let result = match command {
            Command::Save => self.checkpoint().and_then(|saved| saved.save(path)),
            Command::Restore => self.restore_game(path),
            _ => unreachable!(),
        };
        match result {
            Ok(()) => format!(
                "Game {}.\n{}",
                if command == Command::Save {
                    "saved"
                } else {
                    "restored"
                },
                path.display()
            ),
            Err(error) => error,
        }
    }
    fn draw_system(&self, fb: &mut Fb) {
        use crate::system::Panel;
        let Some(system) = &self.system else {
            if self.phase == Phase::Simultaneous {
                fb.text(
                    8,
                    H - 20,
                    "F1: System Menu   * Shot active   Mouse disabled",
                    Rgb::WHITE,
                );
                return;
            }
            fb.bevel(0, H - 22, 64, 22);
            fb.text(8, H - 19, "System", Rgb::BLACK);
            return;
        };
        match &system.panel {
            Panel::Players => {
                ui::draw_menu(fb, self.edit_settings.as_ref().unwrap(), &self.edit_menu)
            }
            Panel::Teams => {
                let settings = self.edit_settings.as_ref().unwrap();
                let count = settings.players + 2;
                let h = 72 + count as i32 * 24;
                let y = (H - h) / 2;
                fb.bevel(40, y, 560, h);
                fb.text(56, y + 8, "REASSIGN TEAMS", Rgb::BLACK);
                for row in 0..count {
                    let text = if row == 0 {
                        format!("Mode: {:?}", settings.play.teams)
                    } else if row == count - 1 {
                        "D Done".into()
                    } else {
                        format!(
                            "{} {}: Team {}",
                            row % 10,
                            self.tanks[row - 1].name,
                            settings.team_ids[row - 1] + 1
                        )
                    };
                    let yy = y + 32 + row as i32 * 24;
                    if row == system.row {
                        fb.frame(48, yy - 2, 544, 23, Rgb::DARK);
                    }
                    fb.text(56, yy, &text, Rgb::BLACK);
                }
                fb.text(
                    56,
                    y + h - 24,
                    "Arrows: select/change  D: apply  Esc: cancel",
                    Rgb::DARK,
                );
            }
            _ => {
                let game = !self.tanks.is_empty() && self.phase != Phase::Menu;
                system.draw(
                    fb,
                    game,
                    game && !self.round_paid && self.phase != Phase::Shop,
                );
                if system.panel == Panel::Menu {
                    fb.text(
                        360,
                        76,
                        if self.settings.sound.enabled {
                            "Sound: on"
                        } else {
                            "Sound: off"
                        },
                        Rgb::BLACK,
                    );
                }
            }
        }
    }

    pub fn take_sound_frame(&mut self) -> sound::Frame {
        let cues = self.sounds.take();
        let enabled = self.settings.sound.enabled && self.system.is_none();
        let flight = if enabled && matches!(self.phase, Phase::Resolve | Phase::Simultaneous) {
            self.projs
                .iter()
                .find(|p| {
                    !matches!(
                        p.kind,
                        PKind::Rolling { .. } | PKind::ShieldRoll { .. } | PKind::FunkyFinal
                    )
                })
                .and_then(|p| {
                    sound::flight_bin(self.settings.sound.flight, p.pos.y, p.vel.length())
                })
        } else {
            None
        };
        sound::Frame {
            enabled,
            flight,
            cues: if enabled { cues } else { Vec::new() },
        }
    }

    fn sound_for_explosion(&mut self, index: usize) {
        let e = &self.expls[index];
        self.sounds.post(if e.dirt {
            Cue::Dirt
        } else {
            match e.damage {
                Damage::Death | Damage::Environment => Cue::Death,
                Damage::Weapon(2 | 3) => Cue::Nuke,
                _ => Cue::Explosion,
            }
        });
    }

    /// Returns false when the player wants to quit.
    pub fn update(&mut self, dt: f32) -> bool {
        self.keys.dt = dt;
        if self.hall_open {
            let input = ui::MenuInput::poll(&mut self.keys);
            self.hall_input(&input);
            return true;
        }
        let (mx, my) = mouse_position();
        let (mx, my) = crate::gfx::Viewport::new(screen_width(), screen_height()).point(mx, my);
        let system_key = is_key_pressed(KeyCode::F1)
            || ((is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt))
                && is_key_pressed(KeyCode::S))
            || (self.system.is_none()
                && self.phase != Phase::Simultaneous
                && is_mouse_button_pressed(MouseButton::Left)
                && !is_mouse_button_down(MouseButton::Right)
                && (0..64).contains(&mx)
                && (H - 22..H).contains(&my));
        if system_key {
            if self.system.is_some() {
                self.system = None;
                self.edit_settings = None;
            } else {
                self.system = Some(crate::system::System {
                    can_save: self.phase == Phase::Shop,
                    ..Default::default()
                });
            }
            return true;
        }
        if self.system.is_some() {
            let input = ui::MenuInput::poll(&mut self.keys);
            return self.system_input(&input);
        }
        let dt = dt * self.fast_scale();
        self.time += dt;
        self.phase_t += dt;
        for b in &mut self.bubbles {
            b.t -= dt;
        }
        self.bubbles.retain(|b| b.t > 0.0);
        for tank in &mut self.tanks {
            if let Some(shield) = &mut tank.equipment.active_shield {
                shield.flash = (shield.flash - dt).max(0.0);
            }
        }

        if self.shop_notice.is_some() {
            if get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left) {
                self.shop_notice = None;
            }
            return true;
        }
        let can_strike = (self.phase == Phase::Aim
            && self.controls.panel == controls::Panel::Closed)
            || self.phase == Phase::Simultaneous;
        if let Some(impact) = self
            .weather
            .update(dt, can_strike, &self.terrain, &self.tanks)
        {
            self.lightning_hit(impact);
        }
        match self.phase {
            Phase::Menu => {
                let (action, clicked) =
                    ui::update_menu(&mut self.settings, &mut self.menu, &mut self.keys);
                if clicked {
                    self.sounds.post(Cue::Click);
                }
                match action {
                    MenuAction::Quit => return false,
                    MenuAction::Start => match self.settings.validate() {
                        Ok(()) => self.start_game(),
                        Err(error) => self.menu.notice = Some(error),
                    },
                    MenuAction::Save => self.save_settings(),
                    MenuAction::HallOfFame => self.open_hall(),
                    MenuAction::Stay => {}
                }
            }
            Phase::Shop => {
                let player = self.shop.player;
                let before = (
                    self.shop.selected,
                    self.shop.panel,
                    self.shop.category,
                    self.tanks[player].inventory.clone(),
                );
                let done = ui::update_shop(
                    &mut self.tanks[player],
                    &self.prices,
                    &self.settings,
                    &mut self.shop,
                    &mut self.market,
                    &mut self.keys,
                );
                economy::sync_bank(&mut self.tanks, player);
                if done
                    || before
                        != (
                            self.shop.selected,
                            self.shop.panel,
                            self.shop.category,
                            self.tanks[player].inventory.clone(),
                        )
                {
                    self.sounds.post(Cue::Click);
                }
                if done {
                    self.save_market();
                    self.shop.player += 1;
                    self.advance_shop();
                }
            }
            Phase::Defense => {
                let player = self.defense_player;
                self.controls
                    .update(&mut self.tanks[player], dt, self.settings.play.status_bar);
                if self.controls.clicked {
                    self.sounds.post(Cue::Click);
                }
                if self.controls.is_closed() {
                    self.defense_player += 1;
                    self.advance_defense();
                }
            }
            Phase::Aim | Phase::Resolve
                if is_key_pressed(KeyCode::Escape) && self.controls.is_closed() =>
            {
                self.terrain = Terrain::generate();
                self.set_phase(Phase::Menu);
            }
            Phase::Aim => self.update_aim(dt),
            Phase::Move => {
                let input = ui::MenuInput::poll(&mut self.keys);
                self.move_input(&input);
            }
            Phase::MoveFall => self.update_move_fall(dt),
            Phase::Batteries => self.update_batteries(),
            Phase::Target => {
                let input = ui::MenuInput::poll(&mut self.keys);
                self.target_input(&input);
            }
            Phase::Resolve => self.update_resolve(dt),
            Phase::Simultaneous => {
                let inputs = self.poll_simultaneous();
                self.simultaneous_input(dt, &inputs);
                self.update_resolve(dt);
            }
            Phase::Retreat => self.update_retreat(),
            Phase::RoundOver | Phase::GameOver => {
                let any_key = get_last_key_pressed().is_some();
                let mut input = ui::MenuInput::poll(&mut self.keys);
                input.activate |= any_key;
                self.standings_input(&input);
            }
        }
        true
    }

    fn start_game(&mut self) {
        self.system = None;
        self.edit_settings = None;
        self.shop_notice = None;
        self.controls = Controls::default();
        self.tanks = (0..self.settings.players)
            .map(|i| {
                let ctrl = self.settings.controllers[i];
                let name = match ctrl {
                    Ctrl::Human => self.settings.names[i].trim().to_string(),
                    _ => format!("{} {}", ctrl.label(), i + 1),
                };
                Tank {
                    bank: None,
                    ai_state: crate::ai::State::new(ctrl),
                    shape: if ctrl == Ctrl::Human
                        && self.settings.shapes[i] == crate::tank::Shape::Triple
                    {
                        crate::tank::Shape::Treads
                    } else {
                        self.settings.shapes[i]
                    },
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
                    equipment: crate::tank::Equipment::default(),
                    weapon: ItemId::BabyMissile,
                    fall: 0,
                    last_hit: None,
                    environmental_hit: false,
                    kills: 0,
                    wins: 0,
                }
            })
            .collect();
        self.current = 0;
        self.concurrent.reset(self.tanks.len());
        ai::name_computers(&mut self.tanks);
        self.apply_team_assignments();
        self.turn_order.reset(self.tanks.len());
        let base = self
            .config_path
            .as_deref()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."));
        let (speech, errors) = crate::speech::Speech::load(&self.settings.play, base);
        self.speech = speech;
        self.round = 1;
        self.interest_round = 0;
        self.refresh_prices();
        self.shop = Shop::default();
        if self.settings.economics.cash_at_start > 0 {
            self.begin_shop();
        } else {
            self.start_round();
        }
        if !errors.is_empty() {
            let mut notice = self.shop_notice.take().unwrap_or_default();
            if !notice.is_empty() {
                notice.push('\n');
            }
            notice.push_str(&errors.join("\n"));
            self.shop_notice = Some(notice);
        }
    }

    fn refresh_prices(&mut self) {
        self.prices = if self.settings.economics.free_market {
            self.market.prices()
        } else {
            economy::base_prices()
        };
        let id = ItemId::AutoDefense as usize;
        self.prices[id] = shop::auto_defense_price(
            self.prices[id],
            self.settings.economics.interest_rate,
            self.settings.rounds.saturating_sub(self.round) + 1,
        );
    }

    fn save_market(&mut self) {
        if self.settings.economics.free_market {
            if let Some(path) = &self.market_path {
                if let Err(error) = self.market.save(path) {
                    eprintln!("{error}");
                    self.shop_notice = Some(format!(
                        "{error}\nMarket changes remain in memory.\nPress any key to continue."
                    ));
                }
            }
        }
    }

    fn begin_shop(&mut self) {
        self.refresh_prices();
        self.shop = Shop::default();
        self.set_phase(Phase::Shop);
        self.advance_shop();
    }

    fn advance_shop(&mut self) {
        while self.shop.player < self.tanks.len() {
            let player = self.shop.player;
            let tank = &mut self.tanks[player];
            self.shop.selected = 0;
            self.shop.category = weapons::Category::Weapon;
            self.shop.panel = shop::Panel::Buy;
            if tank.ctrl == Ctrl::Human && !tank.account.cash.is_zero() {
                return;
            }
            if tank.ctrl != Ctrl::Human && self.settings.economics.computers_buy {
                shop::computer_buys(tank, &self.prices, &self.settings, &mut self.market);
            }
            economy::sync_bank(&mut self.tanks, player);
            self.shop.player += 1;
        }
        self.save_market();
        self.start_round();
    }

    fn finish_round(&mut self, alive: &[usize]) {
        if self.round_paid {
            return;
        }
        self.round_paid = true;
        self.concurrent.reset(self.tanks.len());
        self.projs.clear();
        self.expls.clear();
        self.earth = earth::Effects::default();
        self.liquid = crate::liquid::Effects::default();
        self.energy = crate::energy::Effect::default();
        crate::play::survival(
            &mut self.tanks,
            alive,
            self.settings.play.teams,
            self.settings.economics.scoring,
            self.vicious_split,
        );
        self.round_msg = match alive.first() {
            Some(&winner) => {
                if self.settings.play.teams != TeamMode::None
                    && !self.vicious_split
                    && self.tanks[winner].team.is_some()
                {
                    format!(
                        "Team {} wins the round!",
                        self.tanks[winner].team.unwrap() + 1
                    )
                } else {
                    format!("{} wins the round!", self.tanks[winner].name)
                }
            }
            None => "Nobody survived!".into(),
        };
        if self.settings.economics.free_market {
            self.market
                .advance(self.tanks.len(), self.settings.economics.interest_rate);
            self.refresh_prices();
            self.save_market();
        }
        if self.round == self.settings.rounds {
            self.hall.record(
                &self.tanks,
                &self.prices,
                self.settings.economics.scoring,
                self.settings.rounds,
            );
            if let Some(path) = &self.hall_path {
                self.hall_notice = self.hall.save(path).err().map(|error| {
                    eprintln!("{error}");
                    format!("Scores kept for this session. {error}")
                });
            }
        }
        self.set_phase(Phase::RoundOver);
    }

    fn standings_input(&mut self, input: &ui::MenuInput) {
        if self.phase_t <= 0.8 || !(input.activate || input.click.is_some()) {
            return;
        }
        match self.phase {
            Phase::RoundOver => self.continue_after_round(),
            Phase::GameOver => {
                self.set_phase(Phase::Menu);
                self.open_hall();
            }
            _ => {}
        }
    }

    fn open_hall(&mut self) {
        self.hall_mode = self.settings.economics.scoring;
        self.hall_open = true;
    }

    fn hall_input(&mut self, input: &ui::MenuInput) {
        let button = ui::button_index(input, 3);
        if input.escape || input.activate || button == Some(2) {
            self.hall_open = false;
            self.sounds.post(Cue::Click);
        } else if input.left || input.right || button.is_some() {
            let modes = crate::hall_of_fame::MODES;
            let i = modes.iter().position(|&m| m == self.hall_mode).unwrap();
            let backwards = input.left || button == Some(0);
            self.hall_mode = modes[(i + if backwards { 2 } else { 1 }) % 3];
            self.sounds.post(Cue::Click);
        }
    }

    fn continue_after_round(&mut self) {
        if self.phase != Phase::RoundOver {
            return;
        }
        if self.round < self.settings.rounds {
            self.round += 1;
            self.begin_shop();
        } else {
            self.set_phase(Phase::GameOver);
        }
    }

    fn start_round(&mut self) {
        self.concurrent.reset(self.tanks.len());
        if self.interest_round != self.round {
            economy::interest(&mut self.tanks, self.settings.economics.interest_rate);
            self.interest_round = self.round;
        }
        self.round_paid = false;
        self.vicious_split = false;
        self.action_owner = None;
        self.environment_action = false;
        self.resume_aim_after_weather = false;
        if self.settings.landscape.random_land {
            self.settings.landscape.bumpiness = gen_range(0, 100);
            self.settings.landscape.slope = gen_range(0, 100);
            self.settings.landscape.flatten_peaks = gen_range(0, 2) == 1;
        }
        let mut landscape = self.settings.landscape.clone();
        if let Some(folder) = &landscape.mountains_folder {
            if folder.is_relative() {
                if let Some(parent) = self.config_path.as_ref().and_then(|p| p.parent()) {
                    landscape.mountains_folder = Some(parent.join(folder));
                }
            }
        }
        let (terrain, notice) =
            Terrain::generate_with_direction(&landscape, self.field_top(), self.landscape_reverse);
        if !terrain.scanned {
            self.landscape_reverse = !self.landscape_reverse;
        }
        self.terrain = terrain;
        self.shop_notice = notice;
        self.weather = crate::environment::Weather::new(
            self.terrain.sky_kind,
            gen_range(1, u32::MAX) as u64,
            self.field_top(),
        );
        self.wind = physics::round_wind(self.settings.landscape.max_wind);
        self.active_walls = if self.settings.physics.walls == WallType::Erratic {
            WallType::None
        } else {
            self.settings.physics.walls.choose()
        };
        self.settle_dirt = true;
        self.sim_acc = 0.0;
        self.settle_acc = 0.0;
        self.projs.clear();
        self.earth = earth::Effects::default();
        self.liquid = crate::liquid::Effects::default();
        self.energy = crate::energy::Effect::default();
        self.expls.clear();
        self.clear_traces();
        self.bubbles.clear();

        let n = self.tanks.len();
        let slot = W / n as i32;
        let mut slots: Vec<i32> = (0..n as i32).collect();
        slots.shuffle();
        for (i, t) in self.tanks.iter_mut().enumerate() {
            t.team = if self.settings.play.teams == TeamMode::None {
                None
            } else {
                Some(self.settings.team_ids[i])
            };
            t.ai_state.round_reset();
            let x = slots[i] * slot + slot / 2 + gen_range(-slot / 5, slot / 5 + 1);
            let x = x.clamp(20, W - 21);
            t.x = x;
            t.y = self.terrain.flatten(x, 9) - 1;
            t.angle = if x < W / 2 {
                gen_range(30, 80)
            } else {
                gen_range(100, 150)
            };
            t.power = 400;
            t.health = 100;
            t.alive = true;
            t.reset_descent();
            t.equipment.active_shield = None;
            if t.equipment
                .shield
                .is_some_and(|id| t.inventory.count(id) == 0)
            {
                t.equipment.shield = None;
            }
            if t.equipment
                .guidance
                .is_some_and(|id| t.inventory.count(id) == 0)
            {
                t.equipment.guidance = None;
            }
            if t.inventory.count(ItemId::ContactTrigger) == 0 {
                t.equipment.triggers = false;
            }
            t.last_hit = None;
            t.environmental_hit = false;
            t.weapon = ItemId::BabyMissile;
            if self.settings.play.mode == PlayMode::Simultaneous {
                t.equipment.parachutes_deployed = true;
            }
        }
        self.turn_order.begin(
            &self.tanks,
            self.settings.play.order,
            self.round,
            self.settings.economics.scoring,
            &self.prices,
        );
        self.current = self.turn_order.active.first().copied().unwrap_or(0);
        self.defense_player = 0;
        self.advance_defense();
    }

    fn advance_defense(&mut self) {
        while self.defense_player < self.tanks.len() {
            let tank = &mut self.tanks[self.defense_player];
            if tank.inventory.count(ItemId::AutoDefense) > 0 {
                if tank.ctrl == Ctrl::Human {
                    self.controls = Controls::default();
                    self.controls.defensive_only = true;
                    self.controls.panel = controls::Panel::Control;
                    self.set_phase(Phase::Defense);
                    return;
                }
                ai::prepare(tank);
            }
            self.defense_player += 1;
        }
        self.begin_turn();
    }

    /// Shared clear action for round setup and the future System Menu (batch 17).
    fn clear_traces(&mut self) {
        self.trail.clear();
        for trail in &mut self.concurrent.trails {
            trail.clear();
        }
        self.persistent_traces.clear();
    }

    fn wall_type(&self) -> WallType {
        match self.settings.physics.walls {
            WallType::Random | WallType::Erratic => self.active_walls,
            wall => wall,
        }
    }

    fn field_top(&self) -> i32 {
        if self.settings.play.mode == PlayMode::Simultaneous {
            return crate::modes::hud_height(self.tanks.len().max(self.settings.players));
        }
        HUD_H + if self.settings.play.status_bar { 22 } else { 0 }
    }

    fn physics(&self) -> Physics {
        let mut physics = Physics::new(&self.settings.physics, self.wind);
        physics.walls = self.wall_type();
        physics.ceiling = self.field_top() as f32;
        physics
    }

    fn begin_turn(&mut self) {
        if self.settings.play.teams != TeamMode::None && !self.round_paid {
            if let Some(winners) = crate::play::outcome(
                &mut self.tanks,
                self.settings.play.teams,
                &mut self.vicious_split,
            ) {
                self.finish_round(&winners);
                return;
            }
        }
        self.tanks[self.current].ai_state.prepared = false;
        self.resume_aim_after_movement = false;
        self.environment_action = false;
        self.resume_aim_after_weather = false;
        if self.settings.physics.walls == WallType::Erratic
            && (self.settings.play.mode != PlayMode::Synchronous
                || self.concurrent.queued.iter().all(Option::is_none))
        {
            self.active_walls = self.settings.physics.walls.choose();
        }
        self.controls = Controls::default();
        let t = &mut self.tanks[self.current];
        t.power = t.power.min(t.max_power());
        if !t.inventory.can_select(t.weapon) {
            t.weapon = ItemId::BabyMissile;
        }
        self.ai_plan = None;
        self.ai_t = 0.0;
        if self.settings.play.mode == PlayMode::Simultaneous {
            self.controls = Controls::default();
            self.set_phase(Phase::Simultaneous);
        } else {
            self.set_phase(Phase::Aim);
        }
    }

    // ------------------------------------------------------------ aiming

    fn queue_shot(&mut self, target: Option<macroquad::prelude::Vec2>, batteries: Option<u8>) {
        let owner = self.current;
        let t = &self.tanks[owner];
        if !t.alive
            || t.health <= 0
            || !t.inventory.can_select(t.weapon)
            || self.concurrent.queued[owner].is_some()
            || (batteries.is_none()
                && self
                    .applicable_guidance()
                    .is_some_and(crate::guidance::needs_target)
                && target.is_none())
        {
            return;
        }
        self.concurrent.queued[owner] = Some(crate::modes::Shot {
            owner,
            angle: t.angle,
            power: t.power,
            weapon: t.weapon,
            target,
            batteries,
            intended: self.ai_plan.as_ref().and_then(|p| p.target),
        });
        self.advance_queue();
    }

    fn advance_queue(&mut self) {
        let mut pending: Vec<_> = self
            .turn_order
            .active
            .iter()
            .copied()
            .filter(|&i| {
                self.tanks[i].alive
                    && self.tanks[i].health > 0
                    && self.concurrent.queued[i].is_none()
            })
            .collect();
        if self.settings.play.order == crate::settings::PlayOrder::Random {
            pending.shuffle();
        }
        if let Some(&next) = pending.first() {
            self.current = next;
            self.begin_turn();
            return;
        }
        let shots: Vec<_> = self
            .concurrent
            .queued
            .iter_mut()
            .filter_map(Option::take)
            .filter(|s| self.tanks[s.owner].alive && self.tanks[s.owner].health > 0)
            .collect();
        self.concurrent.launching = true;
        self.trail.clear();
        self.sim_acc = 0.0;
        for trail in &mut self.concurrent.trails {
            trail.clear();
        }
        self.settle_dirt =
            physics::dirt_falls(self.settings.physics.suspend_dirt, gen_range(0, 100));
        for shot in shots {
            self.current = shot.owner;
            let t = &mut self.tanks[shot.owner];
            t.angle = shot.angle;
            t.power = shot.power;
            t.weapon = shot.weapon;
            self.ai_plan = Some(ai::Plan {
                angle: shot.angle,
                power: shot.power,
                weapon: shot.weapon,
                target: shot.intended,
            });
            if let Some(batteries) = shot.batteries {
                self.discharge_energy(batteries);
            } else {
                self.launch(shot.target);
            }
        }
        self.concurrent.launching = false;
        self.ai_plan = None;
        self.action_owner = None;
        self.environment_action = false;
        self.set_phase(Phase::Resolve);
    }

    fn owner_busy(&self, owner: usize) -> bool {
        self.projs.iter().any(|p| p.owner == owner)
            || self.expls.iter().any(|e| e.owner == owner)
            || self.earth.active_for(owner)
            || self.liquid.active_for(owner)
            || self
                .concurrent
                .energy
                .iter()
                .any(|(i, e)| *i == owner && e.active())
    }

    fn poll_simultaneous(&mut self) -> Vec<crate::modes::Input> {
        self.settings.keys[..self.tanks.len()]
            .iter()
            .map(|keys| crate::modes::Input {
                clockwise: self.keys.aim_steps(keys[0].code(), false, false),
                counter_clockwise: self.keys.aim_steps(keys[1].code(), false, false),
                power_up: self.keys.aim_steps(keys[2].code(), false, false),
                power_down: self.keys.aim_steps(keys[3].code(), false, false),
                fire: is_key_down(keys[4].code()),
                weapon: is_key_pressed(keys[5].code()),
            })
            .collect()
    }

    fn automatic_batteries(&mut self) {
        if self.settings.play.mode != PlayMode::Simultaneous {
            return;
        }
        for (i, tank) in self.tanks.iter_mut().enumerate() {
            tank.equipment.parachutes_deployed = true;
            let mut used = false;
            while tank.health <= 90 && tank.use_battery() {
                used = true;
            }
            if used {
                self.concurrent.flash[i] = Some((Rgb(60, 255, 60), 0.18));
            }
        }
    }

    fn round_draining(&self) -> bool {
        let live = crate::play::alive(&self.tanks);
        live.len() <= 1
            || (self.settings.play.teams != TeamMode::None
                && !self.vicious_split
                && live
                    .iter()
                    .all(|&i| self.tanks[i].team == self.tanks[live[0]].team))
    }

    fn simultaneous_input(&mut self, dt: f32, inputs: &[crate::modes::Input]) {
        self.automatic_batteries();
        for flash in &mut self.concurrent.flash {
            if let Some((_, time)) = flash {
                *time -= dt;
                if *time <= 0.0 {
                    *flash = None;
                }
            }
        }
        if self.round_draining() {
            return;
        }
        self.concurrent.wind_time += dt;
        if self.concurrent.wind_time >= 1.2 {
            self.concurrent.wind_time %= 1.2;
            if self.settings.landscape.changing_wind {
                self.wind = physics::changed_wind(
                    self.wind,
                    self.settings.landscape.max_wind,
                    gen_range(0, 11),
                );
            }
            if self.settings.physics.walls == WallType::Erratic {
                self.active_walls = self.settings.physics.walls.choose();
            }
        }
        let mut shots = Vec::new();
        for owner in 0..self.tanks.len() {
            if !self.tanks[owner].alive || self.tanks[owner].health <= 0 {
                continue;
            }
            let busy = self.owner_busy(owner);
            let fire = match self.tanks[owner].ctrl {
                Ctrl::Human => {
                    let input = inputs.get(owner).copied().unwrap_or_default();
                    let t = &mut self.tanks[owner];
                    t.angle = (t.angle + input.counter_clockwise - input.clockwise).clamp(0, 180);
                    let power = t.power;
                    t.power =
                        (t.power + 2 * (input.power_up - input.power_down)).clamp(0, t.max_power());
                    if t.power != power {
                        self.concurrent.flash[owner] = Some((
                            if t.power > power {
                                Rgb::WHITE
                            } else {
                                Rgb(35, 35, 35)
                            },
                            0.12,
                        ));
                    }
                    if input.weapon {
                        t.weapon = t.inventory.cycle_weapon(t.weapon, false);
                        self.concurrent.flash[owner] = Some((Rgb(255, 30, 30), 0.18));
                    }
                    input.fire && !busy
                }
                Ctrl::Ai(kind) => {
                    if busy {
                        self.concurrent.ai_time[owner] = 0.0;
                        self.concurrent.ai_step[owner] = 0.0;
                        continue;
                    }
                    self.concurrent.ai_time[owner] += dt;
                    if self.concurrent.ai[owner].is_none() && self.concurrent.ai_time[owner] >= 0.4
                    {
                        let plan = ai::plan(
                            kind,
                            &self.terrain,
                            &self.tanks,
                            owner,
                            &self.physics(),
                            self.settings.weapons.tunneling,
                            self.settings.weapons.scale.multiplier(),
                        );
                        self.tanks[owner].weapon = plan.weapon;
                        self.concurrent.ai[owner] = Some(plan);
                    }
                    if let Some(plan) = &self.concurrent.ai[owner] {
                        let t = &mut self.tanks[owner];
                        self.concurrent.ai_step[owner] += dt;
                        while self.concurrent.ai_step[owner] >= 0.01 {
                            self.concurrent.ai_step[owner] -= 0.01;
                            t.angle += (plan.angle - t.angle).signum();
                            t.power = (t.power + (plan.power - t.power).clamp(-10, 10))
                                .min(t.max_power());
                        }
                        self.concurrent.ai_time[owner] >= 1.2
                            && t.angle == plan.angle
                            && t.power == plan.power.min(t.max_power())
                    } else {
                        false
                    }
                }
            };
            if fire {
                if !self.tanks[owner]
                    .inventory
                    .can_select(self.tanks[owner].weapon)
                {
                    self.tanks[owner].weapon = ItemId::BabyMissile;
                }
                let t = &self.tanks[owner];
                shots.push(crate::modes::Shot {
                    owner,
                    angle: t.angle,
                    power: t.power,
                    weapon: t.weapon,
                    target: None,
                    batteries: crate::energy::is_energy(t.weapon).then_some(10),
                    intended: self.concurrent.ai[owner].as_ref().and_then(|p| p.target),
                });
                self.concurrent.ai_time[owner] = 0.0;
                self.concurrent.ai_step[owner] = 0.0;
            }
        }
        for shot in shots {
            self.current = shot.owner;
            self.ai_plan = self.concurrent.ai[shot.owner].take();
            self.tanks[shot.owner].angle = shot.angle;
            self.tanks[shot.owner].power = shot.power;
            self.tanks[shot.owner].weapon = shot.weapon;
            if let Some(charge) = shot.batteries {
                self.discharge_energy(charge);
            } else {
                self.launch(None);
            }
            self.ai_plan = None;
        }
    }

    fn update_aim(&mut self, dt: f32) {
        let cur = self.current;
        match self.tanks[cur].ctrl {
            Ctrl::Human => {
                if self.controls.is_closed() {
                    let (x, y) = mouse_position();
                    let point =
                        crate::gfx::Viewport::new(screen_width(), screen_height()).point(x, y);
                    let mouse = self.battle_mouse.update(
                        is_mouse_button_down(MouseButton::Left),
                        is_mouse_button_down(MouseButton::Right),
                        point,
                        dt,
                    );
                    if self.aim_mouse(mouse) {
                        return;
                    }
                } else {
                    self.battle_mouse = ui::BattleMouse::default();
                }
                let action =
                    self.controls
                        .update(&mut self.tanks[cur], dt, self.settings.play.status_bar);
                if self.controls.clicked {
                    self.sounds.post(Cue::Click);
                }
                if self.handle_control_action(action) {
                    return;
                }
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
                    if !self.prepare_ai_turn() {
                        return;
                    }
                    let physics = self.physics();
                    let p = ai::plan(
                        kind,
                        &self.terrain,
                        &self.tanks,
                        cur,
                        &physics,
                        self.settings.weapons.tunneling,
                        self.settings.weapons.scale.multiplier(),
                    );
                    let target = p.target.map(|i| self.tanks[i].center());
                    ai::equip_for_plan(
                        &mut self.tanks[cur],
                        &p,
                        &self.terrain,
                        target,
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

    fn aim_mouse(&mut self, mouse: ui::BattleClick) -> bool {
        let ui::BattleClick::Click(x, y, direction) = mouse else {
            if matches!(mouse, ui::BattleClick::Fire) {
                self.fire();
                return true;
            }
            return false;
        };
        let t = &mut self.tanks[self.current];
        if (0..HUD_H).contains(&y) {
            match x {
                0..=111 => t.power = (t.power - direction).clamp(0, t.max_power()),
                112..=207 => t.angle = (t.angle - direction).clamp(0, 180),
                208..=351 => {
                    self.controls.panel = if direction > 0 {
                        controls::Panel::Control
                    } else {
                        controls::Panel::Inventory
                    };
                    self.controls.selected = 0;
                }
                352..=639 => t.weapon = t.inventory.cycle_weapon(t.weapon, direction < 0),
                _ => return false,
            }
        } else {
            let sorted = crate::guidance::sorted_tanks(&self.tanks);
            let Some(position) = sorted.iter().position(|&i| {
                let t = &self.tanks[i];
                t.alive && (t.x - x).abs() <= 10 && (t.y - 5 - y).abs() <= 10
            }) else {
                return false;
            };
            self.handle_control_action(ControlAction::Info(position));
        }
        self.sounds.post(Cue::Click);
        true
    }

    fn prepare_ai_turn(&mut self) -> bool {
        let current = self.current;
        if self.tanks[current].ai_state.prepared {
            return true;
        }
        ai::prepare(&mut self.tanks[current]);
        self.tanks[current].ai_state.prepared = true;
        let Ctrl::Ai(selected) = self.tanks[current].ctrl else {
            return true;
        };
        let kind = self.tanks[current].ai_state.effective(selected);
        let Some(target) = ai::target_for(kind, &self.tanks, current) else {
            return true;
        };
        let has_riot = [ItemId::RiotCharge, ItemId::RiotBlast]
            .into_iter()
            .any(|id| self.tanks[current].inventory.count(id) > 0);
        if !has_riot
            && self.tanks[current].shape.mobile()
            && self.tanks[current].fuel_available() > 0
            && !ai::clear_shot(
                &self.terrain,
                self.tanks[current].center(),
                self.tanks[target].center(),
            )
        {
            let top = self.field_top();
            for _ in 0..8 {
                let direction = (self.tanks[target].x - self.tanks[current].x).signum();
                match crate::movement::drive(
                    &mut self.tanks,
                    current,
                    &self.terrain,
                    direction,
                    top,
                ) {
                    crate::movement::MoveResult::Blocked => break,
                    crate::movement::MoveResult::Moved => {}
                    crate::movement::MoveResult::Slip => {
                        self.action_owner = Some(current);
                        self.environment_action = false;
                        self.resume_aim_after_weather = false;
                        self.set_phase(Phase::MoveFall);
                        return false;
                    }
                }
                if ai::clear_shot(
                    &self.terrain,
                    self.tanks[current].center(),
                    self.tanks[target].center(),
                ) {
                    break;
                }
            }
        }
        true
    }

    fn handle_control_action(&mut self, action: ControlAction) -> bool {
        match action {
            ControlAction::None => return false,
            ControlAction::Consumed => {}
            ControlAction::Move => self.begin_move(),
            ControlAction::Status => self.controls.message(vec![
                format!("Round {} of {}", self.round, self.settings.rounds),
                format!(
                    "Rounds left: {}",
                    self.settings.rounds.saturating_sub(self.round) + 1
                ),
                format!("Walls: {}", self.wall_type().label()),
                format!("Wind: {}", self.wind),
                format!(
                    "Order: {:?}  Teams: {:?}",
                    self.settings.play.order, self.settings.play.teams
                ),
                format!(
                    "Sky: {:?} / {}",
                    self.terrain.sky_kind,
                    if self.terrain.scanned {
                        "scanned"
                    } else {
                        "generated"
                    }
                ),
                "Press a key to continue.".into(),
            ]),
            ControlAction::Info(position) => {
                let mut indices: Vec<usize> = (0..self.tanks.len()).collect();
                indices.sort_by_key(|&i| (self.tanks[i].x, i));
                if let Some(&index) = indices.get(position) {
                    let tank = &self.tanks[index];
                    self.controls.message(vec![
                        format!("Tank {}: {}", position + 1, tank.name),
                        format!(
                            "{} - {}",
                            tank.ctrl.label(),
                            if tank.alive { "alive" } else { "out" }
                        ),
                        format!(
                            "Health: {}   Max power: {}",
                            tank.health.max(0),
                            tank.max_power()
                        ),
                        format!(
                            "Team: {}",
                            tank.team
                                .map_or("None".into(), |team| (team + 1).to_string())
                        ),
                        format!(
                            "{}: {}",
                            if tank.bank.is_some() {
                                "Team bank"
                            } else {
                                "Cash"
                            },
                            economy::money_text(&tank.account.cash)
                        ),
                        format!("Kills: {}   Wins: {}", tank.kills, tank.wins),
                        "Press a key to continue.".into(),
                    ]);
                }
            }
            ControlAction::Retreat => self.begin_retreat(),
        }
        true
    }

    fn begin_retreat(&mut self) {
        self.sounds.post(Cue::Death);
        let tank = &mut self.tanks[self.current];
        tank.health = 0;
        tank.alive = false;
        tank.last_hit = None;
        tank.reset_descent();
        self.action_owner = None;
        self.projs.clear();
        self.expls.clear();
        self.controls = Controls::default();
        self.set_phase(Phase::Retreat);
    }

    fn update_retreat(&mut self) {
        if self.phase_t >= 2.5 {
            self.set_phase(Phase::Resolve);
        }
    }

    fn fire(&mut self) {
        let tank = &self.tanks[self.current];
        if !tank.inventory.can_select(tank.weapon) {
            return;
        }
        if crate::energy::is_energy(tank.weapon) && tank.inventory.can_select(tank.weapon) {
            self.battery_input = None;
            if tank.ctrl == Ctrl::Human {
                self.set_phase(Phase::Batteries);
            } else {
                self.discharge_energy(10);
            }
            return;
        }
        let selected = self.applicable_guidance();
        if selected.is_some_and(crate::guidance::needs_target) {
            if tank.ctrl == Ctrl::Human {
                self.set_phase(Phase::Target);
                return;
            }
            let planned = self
                .ai_plan
                .as_ref()
                .and_then(|plan| plan.target)
                .filter(|&i| {
                    i < self.tanks.len() && self.tanks[i].alive && self.tanks[i].health > 0
                });
            let target = planned
                .map(|i| vec2(self.tanks[i].x as f32, self.tanks[i].y as f32))
                .or_else(|| {
                    self.tanks
                        .iter()
                        .enumerate()
                        .find(|(i, t)| {
                            *i != self.current
                                && t.alive
                                && t.health > 0
                                && economy::relation(&self.tanks, self.current, *i)
                                    == Relation::Enemy
                        })
                        .map(|(_, t)| vec2(t.x as f32, t.y as f32))
                });
            self.launch(target);
        } else {
            self.launch(None);
        }
    }

    fn applicable_guidance(&self) -> Option<ItemId> {
        if self.settings.play.mode == PlayMode::Simultaneous {
            return None;
        }
        let tank = &self.tanks[self.current];
        tank.equipment.guidance.filter(|&id| {
            crate::guidance::applies(tank.weapon)
                && crate::tank::GUIDANCE.contains(&id)
                && tank.inventory.count(id) > 0
        })
    }

    fn target_input(&mut self, input: &ui::MenuInput) {
        if input.escape || ui::button_index(input, 1) == Some(0) {
            self.set_phase(Phase::Aim);
            return;
        }
        let target = if let Some(digit) = input.hotkey.and_then(|c| c.to_digit(10)) {
            crate::guidance::tank_target(
                &self.tanks,
                if digit == 0 { 9 } else { digit as usize - 1 },
            )
        } else if let Some((x, y, direction)) = input.click {
            crate::guidance::point_target(
                &self.tanks,
                vec2(x as f32, y as f32),
                direction < 0,
                self.field_top(),
            )
        } else {
            None
        };
        if let Some(target) = target {
            self.launch(Some(target));
        }
    }

    fn launch(&mut self, target: Option<macroquad::prelude::Vec2>) {
        if self.settings.play.mode == PlayMode::Synchronous && !self.concurrent.launching {
            self.queue_shot(target, None);
            return;
        }
        let cur = self.current;
        if self.settings.play.mode != PlayMode::Sequential {
            self.concurrent.trails[cur].clear();
        }
        let intended = self.ai_plan.as_ref().and_then(|plan| plan.target);
        let target_point = intended.map(|i| vec2(self.tanks[i].x as f32, self.tanks[i].y as f32));
        let selected = self.applicable_guidance();
        if selected.is_some_and(crate::guidance::needs_target) && target.is_none() {
            return;
        }
        let physics = self.physics();
        let t = &mut self.tanks[cur];
        if !t.inventory.can_select(t.weapon) {
            return;
        }
        let w = t.weapon.item();
        if !t.inventory.remove(t.weapon, 1) {
            return;
        }
        let mut guidance = None;
        if let Some(id) = selected {
            t.inventory.remove(id, 1);
            t.equipment.guidance = None;
            if id == ItemId::BallisticGuidance {
                let (origin, direction) = t.muzzle(t.angle);
                t.power = crate::guidance::ballistic_power(
                    origin,
                    direction,
                    target.unwrap(),
                    &physics,
                    t.max_power(),
                );
            } else {
                guidance = Some(crate::guidance::Guidance {
                    item: id,
                    target: target.unwrap_or_default(),
                    locked: None,
                    engaged: false,
                });
            }
        }
        let forced_contact = guidance.is_some();
        let uses_trigger = !earth::is_instant(t.weapon)
            && !crate::energy::is_energy(t.weapon)
            && !forced_contact
            && t.equipment.triggers
            && t.inventory.count(ItemId::ContactTrigger) > 0;
        if uses_trigger {
            t.inventory.remove(ItemId::ContactTrigger, 1);
            if t.inventory.count(ItemId::ContactTrigger) == 0 {
                t.equipment.triggers = false;
            }
        }
        self.sounds.post(match t.weapon {
            ItemId::RiotCharge | ItemId::RiotBlast | ItemId::RiotBomb | ItemId::HeavyRiotBomb => {
                Cue::Riot
            }
            ItemId::EarthDisrupter => Cue::Disruptor,
            _ => Cue::Fire,
        });
        let (pos, dir) = t.muzzle(t.angle);
        if earth::is_instant(t.weapon) {
            let supported = crate::tank::support_mask(&self.tanks, &self.terrain);
            let t = &self.tanks[cur];
            self.earth.instant_owned(
                cur,
                t.weapon,
                vec2(t.x as f32, (t.y - 6) as f32),
                t.angle as f32,
                &mut self.terrain,
            );
            crate::tank::record_undermining(&mut self.tanks, &self.terrain, supported, Some(cur));
        } else {
            let projectile = Proj {
                pos,
                vel: dir * t.power as f32,
                owner: cur,
                weapon: t.weapon,
                kind: if w.kind == ItemKind::Weapon(WKind::Mirv) {
                    PKind::Carrier
                } else {
                    PKind::Shell
                },
                radius: weapons::initial_radius(t.weapon, self.settings.weapons.scale.multiplier()),
                scale: self.settings.weapons.scale.multiplier(),
                damage: w.damage(),
                age: 0.0,
                tunneling: self.settings.weapons.tunneling && !uses_trigger && !forced_contact,
                in_dirt: false,
                guidance,
            };
            let angles = if matches!(t.weapon, ItemId::Missile | ItemId::BabyMissile) {
                t.shape.barrel_angles(t.angle)
            } else {
                vec![t.angle]
            };
            for angle in angles {
                let (pos, direction) = t.muzzle(angle);
                self.projs.push(Proj {
                    pos,
                    vel: direction * t.power as f32,
                    ..projectile
                });
            }
        }
        ai::record_shot(&mut self.tanks[cur], intended, target_point);
        self.speak(cur, false);
        if self.settings.play.mode == PlayMode::Sequential {
            self.trail.clear();
            self.sim_acc = 0.0;
        }
        self.action_owner = if self.settings.play.mode == PlayMode::Sequential {
            Some(cur)
        } else {
            None
        };
        if self.settings.play.mode != PlayMode::Synchronous || !self.concurrent.launching {
            self.settle_dirt =
                physics::dirt_falls(self.settings.physics.suspend_dirt, gen_range(0, 100));
        }
        if self.settings.play.mode != PlayMode::Simultaneous {
            self.set_phase(Phase::Resolve);
        }
    }

    fn begin_move(&mut self) {
        let tank = &self.tanks[self.current];
        if tank.alive && tank.health > 0 && tank.shape.mobile() && tank.fuel_available() > 0 {
            self.controls = Controls::default();
            self.ai_plan = None;
            self.set_phase(Phase::Move);
        }
    }

    fn move_input(&mut self, input: &ui::MenuInput) {
        let button = ui::button_index(input, 3);
        if input.escape || input.hotkey == Some('D') || button == Some(2) {
            self.end_move();
            return;
        }
        let direction = i32::from(input.right || button == Some(1))
            - i32::from(input.left || button == Some(0));
        let top = self.field_top();
        let result =
            crate::movement::drive(&mut self.tanks, self.current, &self.terrain, direction, top);
        match result {
            crate::movement::MoveResult::Blocked => {}
            crate::movement::MoveResult::Moved => {
                self.sounds.post(Cue::Click);
            }
            crate::movement::MoveResult::Slip => {
                self.action_owner = Some(self.current);
                self.environment_action = false;
                self.resume_aim_after_weather = false;
                self.set_phase(Phase::MoveFall);
            }
        }
        if self.phase == Phase::Move && self.tanks[self.current].fuel_available() == 0 {
            self.end_move();
        }
    }

    fn end_move(&mut self) {
        if self.settings.physics.tanks_fall
            && !self.supported_at(self.current, self.tanks[self.current].y)
        {
            self.action_owner = Some(self.current);
            self.environment_action = false;
            self.resume_aim_after_weather = false;
            self.set_phase(Phase::MoveFall);
        } else {
            self.set_phase(Phase::Aim);
        }
    }

    fn update_move_fall(&mut self, dt: f32) {
        let falling = self.update_falling(dt);
        if self.tanks.iter().any(|t| t.alive && t.health <= 0) {
            self.resume_aim_after_movement = self.tanks[self.current].health > 0;
            self.settle_dirt = false;
            self.set_phase(Phase::Resolve);
        } else if !falling {
            self.ai_plan = None;
            self.set_phase(Phase::Aim);
        }
    }

    fn battery_limit(&self) -> u8 {
        self.tanks[self.current]
            .inventory
            .count(ItemId::Battery)
            .min(10)
    }

    fn update_batteries(&mut self) {
        if let Some(key) = get_last_key_pressed() {
            self.battery_key(key);
        }
        let input = ui::MenuInput::poll(&mut self.keys);
        self.battery_mouse(&input);
    }

    fn battery_mouse(&mut self, input: &ui::MenuInput) {
        if let Some(button) = ui::button_index(input, 4) {
            self.battery_key([KeyCode::Down, KeyCode::Up, KeyCode::Enter, KeyCode::Escape][button]);
        }
    }

    fn battery_key(&mut self, key: KeyCode) {
        let limit = self.battery_limit();
        let value = self.battery_input.unwrap_or(limit);
        match key {
            KeyCode::Escape => self.set_phase(Phase::Aim),
            KeyCode::Enter | KeyCode::KpEnter | KeyCode::Space => self.discharge_energy(value),
            KeyCode::Backspace | KeyCode::Delete => self.battery_input = None,
            KeyCode::Up | KeyCode::Right => self.battery_input = Some((value + 1).min(limit)),
            KeyCode::Down | KeyCode::Left => self.battery_input = Some(value.saturating_sub(1)),
            _ => {
                let digit = match key {
                    KeyCode::Key0 | KeyCode::Kp0 => 0,
                    KeyCode::Key1 | KeyCode::Kp1 => 1,
                    KeyCode::Key2 | KeyCode::Kp2 => 2,
                    KeyCode::Key3 | KeyCode::Kp3 => 3,
                    KeyCode::Key4 | KeyCode::Kp4 => 4,
                    KeyCode::Key5 | KeyCode::Kp5 => 5,
                    KeyCode::Key6 | KeyCode::Kp6 => 6,
                    KeyCode::Key7 | KeyCode::Kp7 => 7,
                    KeyCode::Key8 | KeyCode::Kp8 => 8,
                    KeyCode::Key9 | KeyCode::Kp9 => 9,
                    _ => return,
                };
                self.battery_input =
                    Some((self.battery_input.unwrap_or(0) * 10 + digit).min(limit));
            }
        }
    }

    fn discharge_energy(&mut self, requested: u8) {
        if self.settings.play.mode == PlayMode::Synchronous && !self.concurrent.launching {
            self.queue_shot(None, Some(requested.min(self.battery_limit())));
            return;
        }
        let owner = self.current;
        if self.settings.play.mode != PlayMode::Sequential {
            self.concurrent.trails[owner].clear();
        }
        let batteries = requested.min(self.battery_limit());
        let weapon = self.tanks[owner].weapon;
        if !crate::energy::is_energy(weapon) || !self.tanks[owner].inventory.remove(weapon, 1) {
            return;
        }
        self.tanks[owner]
            .inventory
            .remove(ItemId::Battery, u32::from(batteries));
        let supported = crate::tank::support_mask(&self.tanks, &self.terrain);
        let (effect, events) = crate::energy::discharge(
            weapon,
            batteries,
            owner,
            self.field_top(),
            &mut self.terrain,
            &mut self.tanks,
        );
        crate::tank::record_undermining(&mut self.tanks, &self.terrain, supported, Some(owner));
        if self.settings.play.mode == PlayMode::Sequential {
            self.energy = effect;
        } else {
            self.concurrent.energy.push((owner, effect));
        }
        for event in events {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
        self.sounds.post(if weapon == ItemId::Laser {
            Cue::Laser
        } else {
            Cue::Plasma
        });
        if self.settings.play.mode == PlayMode::Sequential {
            self.trail.clear();
            self.sim_acc = 0.0;
        }
        self.action_owner = if self.settings.play.mode == PlayMode::Sequential {
            Some(owner)
        } else {
            None
        };
        if self.settings.play.mode != PlayMode::Synchronous || !self.concurrent.launching {
            self.settle_dirt =
                physics::dirt_falls(self.settings.physics.suspend_dirt, gen_range(0, 100));
        }
        if self.settings.play.mode != PlayMode::Simultaneous {
            self.set_phase(Phase::Resolve);
        }
    }

    fn say(&mut self, text: String, x: i32, y: i32) {
        self.bubbles.push(Bubble {
            text,
            x,
            y: y - 16,
            t: f32::from(self.settings.play.talk_delay).max(1.0) * 0.1,
        });
    }

    // ------------------------------------------------------------ resolving

    fn update_resolve(&mut self, dt: f32) {
        let shot_time = self.settings.hardware.shot_time(dt);
        self.energy.update(shot_time);
        for (_, effect) in &mut self.concurrent.energy {
            effect.update(shot_time);
        }
        self.concurrent.energy.retain(|(_, e)| e.active());
        self.sim_acc += shot_time;
        while self.sim_acc >= SIM_DT && !self.projs.is_empty() {
            self.sim_acc -= SIM_DT;
            self.step_projs(SIM_DT);
        }
        if self.projs.is_empty() {
            self.sim_acc = 0.0;
        }

        let physics = self.physics();
        let effects = self
            .earth
            .update(shot_time, &physics, &mut self.terrain, &mut self.tanks);
        self.apply_earth_update(effects);

        let top = self.field_top();
        let wall = self.wall_type();
        let liquids = self
            .liquid
            .update(shot_time, &mut self.terrain, &mut self.tanks, top, wall);
        for event in liquids.damage {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
        if liquids.burning {
            self.sounds.post(Cue::Burn);
        }
        if liquids.depositing {
            self.sounds.post(Cue::Dirt);
        }

        let dirt_ceiling = self.field_top();
        for e in &mut self.expls {
            e.t += shot_time;
            if e.dirt {
                let radius = (e.radius * (e.t / e.dur / 0.4).min(1.0)) as i32;
                self.terrain
                    .add_dirt(e.pos.x as i32, e.pos.y as i32, radius, dirt_ceiling);
            }
        }
        let (done, keep): (Vec<_>, Vec<_>) = std::mem::take(&mut self.expls)
            .into_iter()
            .partition(|e| e.t >= e.dur);
        self.expls = keep;
        for mut e in done {
            let follow_up = e.follow_up.take();
            self.apply_explosion(e);
            if let Some(p) = follow_up {
                weapons::follow_explosion(p, &self.terrain, &mut self.projs);
            }
        }

        let mut settling = false;
        if self.settle_dirt
            && ((self.projs.is_empty()
                && self.expls.is_empty()
                && !self.earth.active()
                && !self.liquid.active()
                && !self.energy.active()
                && self.concurrent.energy.is_empty())
                || self.settings.play.mode == PlayMode::Simultaneous)
        {
            self.settle_acc += dt * SETTLE_RATE;
            while self.settle_acc >= 1.0 {
                self.settle_acc -= 1.0;
                settling |= self.terrain.settle_step();
            }
        }

        let falling = self.update_falling(dt);
        if self.settings.play.mode != PlayMode::Sequential {
            self.automatic_batteries();
            self.resolve_deaths();
        }

        if !self.projs.is_empty()
            || !self.expls.is_empty()
            || self.earth.active()
            || self.liquid.active()
            || self.energy.active()
            || !self.concurrent.energy.is_empty()
            || settling
            || falling
        {
            return;
        }

        if self.settings.play.mode == PlayMode::Sequential && self.resolve_deaths() {
            return;
        }

        if let Some(alive) = crate::play::outcome(
            &mut self.tanks,
            self.settings.play.teams,
            &mut self.vicious_split,
        ) {
            self.finish_round(&alive);
            return;
        }
        if self.settings.play.mode == PlayMode::Simultaneous {
            self.set_phase(Phase::Simultaneous);
            return;
        }
        if self.resume_aim_after_movement && self.tanks[self.current].alive {
            self.resume_aim_after_movement = false;
            self.ai_plan = None;
            self.set_phase(Phase::Aim);
            return;
        }
        if self.resume_aim_after_weather && self.tanks[self.current].alive {
            self.begin_turn();
            return;
        }
        if self.settings.play.mode == PlayMode::Synchronous
            && self.concurrent.queued.iter().any(Option::is_some)
        {
            self.advance_queue();
            return;
        }
        self.current = self
            .turn_order
            .next(&self.tanks, self.current, self.settings.play.order);
        if self.settings.landscape.changing_wind {
            self.wind = physics::changed_wind(
                self.wind,
                self.settings.landscape.max_wind,
                gen_range(0, 11),
            );
        }
        self.begin_turn();
    }

    fn resolve_deaths(&mut self) -> bool {
        let mut any_died = false;
        for i in 0..self.tanks.len() {
            let t = &mut self.tanks[i];
            if t.alive && t.health <= 0 {
                t.alive = false;
                any_died = true;
                self.sounds.post(Cue::Death);
                let neutral = t.environmental_hit;
                let killer = t.last_hit.unwrap_or(i);
                let c = t.center();
                let death = crate::presentation::Death {
                    style: gen_range(0, 8),
                    color: t.color,
                    shape: t.shape,
                    angle: t.angle,
                };
                if !neutral {
                    let relation = economy::relation(&self.tanks, killer, i);
                    if relation == Relation::Enemy {
                        self.tanks[killer].ai_state.grudges[i] =
                            self.tanks[killer].ai_state.grudges[i].saturating_add(5);
                    }
                    self.tanks[killer].kills += if relation == Relation::Enemy { 1 } else { -1 };
                    let award = economy::kill_award(self.settings.economics.scoring, relation);
                    economy::award_cash(&mut self.tanks, killer, award);
                }
                self.expls.push(Explosion::new(
                    c,
                    30.0,
                    if neutral {
                        Damage::Environment
                    } else {
                        Damage::Death
                    },
                    killer,
                    false,
                ));
                let blast = self.expls.last_mut().unwrap();
                blast.death = Some(death);
                blast.dur = 1.6;
                self.speak(i, true);
            }
        }
        any_died
    }

    fn landing_tank(&self, index: usize, y: i32) -> Option<usize> {
        let tank = &self.tanks[index];
        self.tanks
            .iter()
            .enumerate()
            .find(|(other, t)| {
                *other != index
                    && t.alive
                    && t.health > 0
                    && (t.x - tank.x).abs() <= 12
                    && y == t.y - 9
            })
            .map(|(other, _)| other)
    }

    fn supported_at(&self, index: usize, y: i32) -> bool {
        let x = self.tanks[index].x;
        (if y == self.tanks[index].y {
            self.tanks[index].supported(&self.terrain)
        } else {
            (-7..=7)
                .filter(|dx| self.terrain.solid(x + dx, y + 1))
                .take(3)
                .count()
                >= 3
        }) || self.landing_tank(index, y).is_some()
    }

    fn predicted_landing(&self, index: usize) -> (i32, Option<usize>) {
        let start = self.tanks[index].y;
        let mut y = start;
        while y <= H + 10 && !self.supported_at(index, y) {
            y += 1;
        }
        (y - start, self.landing_tank(index, y))
    }

    fn update_falling(&mut self, dt: f32) -> bool {
        if !self.settings.physics.tanks_fall {
            return false;
        }
        let impact_damage = self.settings.physics.impact_damage;
        let mut events = Vec::new();
        for i in 0..self.tanks.len() {
            if !self.tanks[i].alive {
                continue;
            }
            if self.supported_at(i, self.tanks[i].y) && !self.tanks[i].equipment.fall_started {
                continue;
            }
            let (drop, collision) = self.predicted_landing(i);
            let before = self.tanks[i].health.max(0);
            let t = &mut self.tanks[i];
            if !t.equipment.fall_started {
                t.equipment.fall_started = true;
                if self.settings.play.mode == PlayMode::Sequential {
                    t.environmental_hit = self.environment_action;
                }
                t.last_hit = if self.environment_action {
                    None
                } else {
                    Some(self.action_owner.or(t.last_hit).unwrap_or(i))
                };
                if t.equipment.parachutes_deployed
                    && (self.settings.play.mode == PlayMode::Simultaneous
                        || i32::from(t.equipment.parachute_threshold) < 2 * drop)
                    && t.inventory.count(ItemId::Parachute) > 0
                {
                    t.inventory.remove(ItemId::Parachute, 1);
                    t.equipment.chute_open = true;
                }
            }
            let delay = if t.equipment.chute_open {
                20
            } else {
                self.settings.hardware.falling_delay
            };
            let steps = if delay == 0 {
                H + 20
            } else {
                t.equipment.descent_time += dt * 1000.0 / f32::from(delay);
                let steps = t.equipment.descent_time as i32;
                t.equipment.descent_time -= steps as f32;
                steps
            };
            for _ in 0..steps {
                if self.supported_at(i, self.tanks[i].y) {
                    break;
                }
                let t = &mut self.tanks[i];
                t.y += 1;
                t.fall += 1;
                if !impact_damage && !t.equipment.chute_open && collision.is_none() {
                    t.health -= 2;
                    t.power = t.power.min(t.max_power());
                }
                if t.y > H + 10 {
                    break;
                }
            }
            if self.supported_at(i, self.tanks[i].y) {
                if let Some(victim) = self.landing_tank(i, self.tanks[i].y) {
                    if self.tanks[i].fall > 0
                        && economy::relation(&self.tanks, i, victim) == Relation::Enemy
                    {
                        let drop = self.tanks[i].fall;
                        let victim_before = self.tanks[victim].health.max(0);
                        self.tanks[victim].health -= 2 * drop + 50;
                        self.tanks[victim].power =
                            self.tanks[victim].power.min(self.tanks[victim].max_power());
                        self.tanks[victim].last_hit = if self.environment_action {
                            None
                        } else {
                            Some(i)
                        };
                        self.tanks[victim].environmental_hit = self.environment_action;
                        events.push(DamageEvent {
                            owner: i,
                            victim,
                            amount: victim_before - self.tanks[victim].health.max(0),
                            target: DamageTarget::Tank,
                        });
                        self.tanks[i].health -= drop + 10;
                    } else if !self.tanks[i].equipment.chute_open {
                        self.tanks[i].health -= 2 * self.tanks[i].fall;
                    }
                    self.tanks[i].reset_descent();
                } else {
                    self.tanks[i].finish_fall(impact_damage);
                }
            }
            if self.tanks[i].y > H + 10 {
                self.tanks[i].health = 0;
                self.tanks[i].y = H + 10;
                self.tanks[i].reset_descent();
            }
            let t = &mut self.tanks[i];
            t.power = t.power.min(t.max_power());
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
        for event in events.into_iter().filter(|_| !self.environment_action) {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
        self.tanks
            .iter()
            .enumerate()
            .any(|(i, t)| t.alive && t.y < H + 10 && !self.supported_at(i, t.y))
    }

    fn step_projs(&mut self, dt: f32) {
        let physics = self.physics();
        let pending = std::mem::take(&mut self.projs);
        let busy_funky: Vec<usize> = pending
            .iter()
            .filter(|p| p.weapon == ItemId::FunkyBomb && p.kind != PKind::FunkyFinal)
            .map(|p| p.owner)
            .collect();
        let mut next = Vec::with_capacity(pending.len());
        for mut p in pending {
            if p.kind == PKind::FunkyFinal {
                let busy = busy_funky.contains(&p.owner)
                    || self
                        .expls
                        .iter()
                        .any(|e| e.owner == p.owner && matches!(e.damage, Damage::Weapon(5)));
                if busy {
                    next.push(p);
                } else {
                    self.sounds.post(Cue::Explosion);
                    self.expls.push(Explosion::new(
                        p.pos,
                        40.0 * p.scale,
                        p.damage,
                        p.owner,
                        false,
                    ));
                }
                continue;
            }
            let trace_start = self.trail.len();
            let supported = crate::tank::support_mask(&self.tanks, &self.terrain);
            let result = weapons::step_proj(
                &mut p,
                dt,
                &mut self.terrain,
                &self.tanks,
                &physics,
                &mut self.trail,
            );
            if self.settings.play.mode != PlayMode::Sequential {
                crate::tank::record_undermining(
                    &mut self.tanks,
                    &self.terrain,
                    supported,
                    Some(p.owner),
                );
            }
            if self.settings.weapons.trace_paths || p.weapon == ItemId::SmokeTracer {
                self.persistent_traces
                    .record(&self.trail[trace_start..], Some(self.tanks[p.owner].color));
            }
            // Tracers and Funky trails remain visible for this shot independently
            // of persistent paths. Other shots only leave paths when configured.
            if p.weapon != ItemId::Tracer && !matches!(p.kind, PKind::FunkyTrail { .. }) {
                self.trail.truncate(trace_start);
            }
            if self.settings.play.mode != PlayMode::Sequential {
                self.concurrent.trails[p.owner].extend(self.trail.drain(trace_start..));
            }
            match result {
                Step::Shield {
                    victim,
                    cost,
                    continues,
                } => {
                    self.sounds.post(Cue::ShieldHit);
                    let lost = self.tanks[victim].drain_shield(cost);
                    economy::award_damage(
                        &mut self.tanks,
                        self.settings.economics.scoring,
                        DamageEvent {
                            owner: p.owner,
                            victim,
                            amount: lost,
                            target: DamageTarget::Shield,
                        },
                    );
                    if continues {
                        if p.kind == PKind::Carrier && p.vel.y >= 0.0 {
                            weapons::split_carrier(p, &mut next);
                        } else {
                            next.push(p);
                        }
                    } else {
                        ai::record_landing(&mut self.tanks[p.owner], p.pos);
                    }
                }
                Step::Alive => next.push(p),
                Step::Gone => ai::record_landing(&mut self.tanks[p.owner], p.pos),
                Step::Split => {
                    self.sounds.post(Cue::Split);
                    weapons::split_carrier(p, &mut next);
                }
                Step::Hit(pos, on_tank) => self.impact_projectile(p, pos, on_tank, &mut next),
                Step::Wall(pos) => {
                    if p.weapon.item().kind == ItemKind::Weapon(WKind::Tunnel) {
                        continue;
                    }
                    // Concrete is an obstruction, not dirt where a roller starts rolling.
                    if p.kind == PKind::Shell
                        && p.weapon.item().kind == ItemKind::Weapon(WKind::Roller)
                    {
                        p.kind = PKind::Bomblet;
                    }
                    self.impact_projectile(p, pos, None, &mut next);
                }
            }
        }
        self.projs = next;
    }

    fn impact_projectile(
        &mut self,
        p: Proj,
        pos: macroquad::prelude::Vec2,
        on_tank: Option<usize>,
        next: &mut Vec<Proj>,
    ) {
        ai::record_landing(&mut self.tanks[p.owner], pos);
        if crate::liquid::is_liquid(p.weapon) {
            if self.liquid.impact(&p, pos, self.field_top()) {
                self.sounds.post(Cue::Splash);
            }
            return;
        }
        if earth::is_earth(p.weapon) {
            match p.weapon.item().kind {
                ItemKind::Weapon(WKind::Riot) => self.sounds.post(Cue::Riot),
                ItemKind::Weapon(WKind::Tunnel) if on_tank.is_none() => self.sounds.post(Cue::Dig),
                ItemKind::Weapon(WKind::DirtCharge) => self.sounds.post(Cue::Dirt),
                _ => {}
            }
            let physics = self.physics();
            let result = self.earth.impact(
                p,
                pos,
                on_tank,
                self.settings.weapons.extra_dirt,
                &physics,
                &mut self.terrain,
                &mut self.tanks,
            );
            self.apply_earth_update(result);
            return;
        }
        let first_explosion = self.expls.len();
        if let Some(event) = weapons::impact(
            p,
            pos,
            on_tank,
            &self.terrain,
            &mut self.tanks,
            next,
            &mut self.expls,
        ) {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
        for i in first_explosion..self.expls.len() {
            self.sound_for_explosion(i);
        }
    }

    fn apply_earth_update(&mut self, effects: earth::Update) {
        if effects.force_settle {
            self.settle_dirt = true;
        }
        for event in effects.damage {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
        let first_explosion = self.expls.len();
        self.expls.extend(effects.explosions);
        for i in first_explosion..self.expls.len() {
            self.sound_for_explosion(i);
        }
    }

    fn lightning_hit(&mut self, impact: macroquad::prelude::Vec2) {
        self.sounds.post(Cue::Thunder);
        if !self.settings.landscape.hostile_environment {
            return;
        }
        for tank in self.tanks.iter_mut().filter(|t| t.alive && t.health > 0) {
            let distance = tank.center().distance(impact);
            let shield_reach = tank.equipment.active_shield.as_ref().is_some_and(|s| {
                distance <= crate::shields::spec(s.item).unwrap().radius as f32 + 2.0
            });
            if distance <= 24.0 || shield_reach {
                let damage = 35 - tank.drain_shield(35);
                if damage > 0 {
                    tank.health -= damage;
                    tank.power = tank.power.min(tank.max_power());
                    tank.last_hit = None;
                    tank.environmental_hit = true;
                    self.ai_plan = None;
                }
            }
        }
        if self.tanks.iter().any(|t| t.alive && t.health <= 0) {
            if self.settings.play.mode == PlayMode::Simultaneous {
                return;
            }
            self.environment_action = true;
            self.resume_aim_after_weather = self.tanks[self.current].health > 0;
            self.action_owner = None;
            self.settle_dirt = true;
            self.set_phase(Phase::Resolve);
        }
    }

    fn apply_explosion(&mut self, e: Explosion) {
        let neutral = matches!(e.damage, Damage::Environment);
        let supported = crate::tank::support_mask(&self.tanks, &self.terrain);
        let (cx, cy, r) = (e.pos.x as i32, e.pos.y as i32, e.radius as i32);
        if e.dirt {
            self.terrain.add_dirt(cx, cy, r, self.field_top());
            return;
        }
        self.terrain.carve(cx, cy, r);
        let mut events = Vec::new();
        for (i, t) in self
            .tanks
            .iter_mut()
            .enumerate()
            .filter(|(_, t)| t.alive && t.health > 0)
        {
            let point = match e.damage {
                Damage::Weapon(_) | Damage::Sandhog(_) => vec2(t.x as f32, t.y as f32),
                Damage::Death | Damage::Environment => t.center(),
            };
            let d = point.distance(e.pos);
            let mut dmg = explosion_damage(d, e.radius, e.damage);
            if (neutral || e.owner != i) && !matches!(e.damage, Damage::Sandhog(_)) {
                let absorbed = t.drain_shield(dmg);
                if absorbed > 0 {
                    events.push(DamageEvent {
                        owner: e.owner,
                        victim: i,
                        amount: absorbed,
                        target: DamageTarget::Shield,
                    });
                }
                dmg -= absorbed;
            }
            if dmg > 0 {
                events.push(DamageEvent {
                    owner: e.owner,
                    victim: i,
                    amount: dmg.min(t.health),
                    target: DamageTarget::Tank,
                });
                t.health -= dmg;
                t.power = t.power.min(t.max_power());
                t.last_hit = if neutral { None } else { Some(e.owner) };
                t.environmental_hit = neutral;
            }
        }
        if self.settings.play.mode != PlayMode::Sequential {
            crate::tank::record_undermining(
                &mut self.tanks,
                &self.terrain,
                supported,
                if neutral { None } else { Some(e.owner) },
            );
        }
        for event in events.into_iter().filter(|_| !neutral) {
            economy::award_damage(&mut self.tanks, self.settings.economics.scoring, event);
        }
    }

    // ------------------------------------------------------------ drawing

    pub fn draw(&self, fb: &mut Fb) {
        if self.hall_open {
            crate::hall_of_fame::draw(fb, &self.hall, self.hall_mode, self.hall_notice.as_deref());
            return;
        }
        self.terrain.render(fb);
        if let Some(notice) = &self.shop_notice {
            ui::dialog(
                fb,
                &ui::wrap_text(notice, 60)
                    .into_iter()
                    .take(22)
                    .collect::<Vec<_>>(),
            );
            self.draw_system(fb);
            return;
        }
        if self.phase == Phase::Shop {
            ui::draw_shop(
                fb,
                &self.tanks[self.shop.player],
                &self.prices,
                &self.settings,
                &self.shop,
                self.round,
            );
            self.draw_system(fb);
            return;
        }
        if self.phase == Phase::Menu {
            ui::draw_menu(fb, &self.settings, &self.menu);
            self.draw_system(fb);
            return;
        }

        let displayed_player = if self.phase == Phase::Defense {
            self.defense_player
        } else {
            self.current
        };
        self.weather.draw(fb);
        self.persistent_traces.draw(fb);
        self.earth.draw(fb);
        self.liquid.draw(fb);
        for &(x, y, c) in &self.trail {
            fb.put(x, y, c);
        }
        for trail in &self.concurrent.trails {
            for &(x, y, c) in trail {
                fb.put(x, y, c);
            }
        }
        for (i, t) in self.tanks.iter().enumerate().filter(|(_, t)| t.alive) {
            if let Some(team) = t.team {
                fb.text(
                    (t.x - 8).clamp(0, W - 16),
                    (t.y - 39).max(self.field_top()),
                    &format!("T{}", team + 1),
                    PLAYER_COLORS[team as usize % 10],
                );
            }
            crate::shields::draw(fb, t, self.time);
            if let Some((color, _)) = self.concurrent.flash.get(i).copied().flatten() {
                ui::draw_tank_shape(fb, t.x, t.y, t.angle, color, t.shape);
            } else {
                draw_tank(fb, t);
            }
            if self.concurrent.queued.get(i).is_some_and(Option::is_some) {
                fb.frame(t.x - 10, t.y - 14, 21, 17, Rgb::WHITE);
                fb.text(
                    (t.x - 20).clamp(0, W - 40),
                    (t.y - 34).max(self.field_top()),
                    "READY",
                    Rgb::WHITE,
                );
            }
            if t.equipment.chute_open {
                controls::draw_parachute(fb, t);
            }
        }
        if self.phase == Phase::Aim && (self.time * 3.0) as i32 % 2 == 0 {
            let t = &self.tanks[self.current];
            fb.line(t.x - 3, t.y - 20, t.x, t.y - 16, Rgb::WHITE);
            fb.line(t.x + 3, t.y - 20, t.x, t.y - 16, Rgb::WHITE);
        }
        for p in &self.projs {
            if p.kind != PKind::FunkyFinal {
                ui::draw_projectile(fb, p, self.settings.weapons.bomb_icon);
            }
        }
        for e in &self.expls {
            draw_explosion(fb, e);
        }
        self.energy.draw(fb);
        for (_, effect) in &self.concurrent.energy {
            effect.draw(fb);
        }
        for b in &self.bubbles {
            let w = text_w(&b.text) + 6;
            let x = (b.x - w / 2).clamp(0, W - w);
            let y = (b.y - 20).max(HUD_H + 2);
            fb.rect(x, y, w, 20, Rgb::BLACK);
            fb.frame(x, y, w, 20, Rgb::WHITE);
            fb.text(x + 3, y + 2, &b.text, Rgb::WHITE);
        }
        if self.phase == Phase::Retreat {
            controls::draw_retreat(fb, &self.tanks[self.current], self.phase_t);
        }
        ui::draw_walls(fb, self.wall_type(), self.field_top());
        if self.settings.play.mode == PlayMode::Simultaneous && self.phase != Phase::Defense {
            let busy: Vec<_> = (0..self.tanks.len()).map(|i| self.owner_busy(i)).collect();
            crate::modes::draw_hud(fb, &self.tanks, &busy, self.wind);
        } else {
            ui::draw_hud(
                fb,
                &self.tanks[displayed_player],
                self.wind,
                self.settings.play.status_bar,
            );
        }
        if self.settings.play.status_bar && self.settings.play.mode != PlayMode::Simultaneous {
            controls::draw_status(fb, &self.tanks[displayed_player]);
        }
        if self.vicious_split {
            fb.text(8, self.field_top() + 4, "Vicious: free-for-all", Rgb::WHITE);
        }

        if matches!(self.phase, Phase::Aim | Phase::Defense) {
            controls::draw(fb, &self.tanks[displayed_player], &self.controls);
        }
        if self.settings.play.mode == PlayMode::Synchronous
            && matches!(self.phase, Phase::Aim | Phase::Target | Phase::Batteries)
        {
            let ready = self
                .concurrent
                .queued
                .iter()
                .filter(|q| q.is_some())
                .count();
            fb.text(
                8,
                self.field_top() + 4,
                &format!(
                    "Synchronous: {ready}/{} ready",
                    crate::play::alive(&self.tanks).len()
                ),
                Rgb::WHITE,
            );
        }

        match self.phase {
            Phase::Move | Phase::MoveFall => {
                fb.bevel(80, H - 104, W - 160, 94);
                fb.text(
                    96,
                    H - 96,
                    &format!("Move - Fuel: {}", self.tanks[self.current].fuel_available()),
                    Rgb::BLACK,
                );
                fb.text(
                    96,
                    H - 44,
                    if self.phase == Phase::MoveFall {
                        "Slipping..."
                    } else {
                        "Left/Right: drive   Esc/D: aim"
                    },
                    Rgb::BLACK,
                );
            }
            Phase::Target => {
                for (position, index) in crate::guidance::sorted_tanks(&self.tanks)
                    .into_iter()
                    .enumerate()
                {
                    let tank = &self.tanks[index];
                    if tank.alive && tank.health > 0 {
                        fb.text(
                            (tank.x - 4).clamp(0, W - 10),
                            (tank.y - 32).max(self.field_top()),
                            &((position + 1) % 10).to_string(),
                            Rgb::WHITE,
                        );
                    }
                }
                fb.bevel(8, H - 52, W - 16, 44);
                fb.text(
                    16,
                    H - 48,
                    "Choose Target: 1-9/0 = tank; left click = point",
                    Rgb::BLACK,
                );
                fb.text(
                    16,
                    H - 30,
                    "Right click = nearby tank (100px); Esc cancels",
                    Rgb::BLACK,
                );
            }
            Phase::Batteries => ui::dialog(
                fb,
                &[
                    self.tanks[self.current].weapon.item().name.into(),
                    format!(
                        "Batteries to discharge: {}",
                        self.battery_input.unwrap_or(self.battery_limit())
                    ),
                    format!(
                        "Available: {} (maximum 10)",
                        self.tanks[self.current].inventory.count(ItemId::Battery)
                    ),
                    "Type 0-10 or use arrows; Enter fires.".into(),
                    "Esc cancels; Backspace resets.".into(),
                ],
            ),
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
        match self.phase {
            Phase::Move => ui::mouse_buttons(fb, &["Drive left", "Drive right", "Aim"]),
            Phase::Target => ui::mouse_buttons(fb, &["Cancel target"]),
            Phase::Batteries => ui::mouse_buttons(fb, &["-", "+", "Fire", "Cancel"]),
            _ => {}
        }
        self.draw_system(fb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{aim_repeat_steps, display_angle};

    fn concurrent_game(mode: PlayMode, players: usize) -> Game {
        let mut g = Game::new();
        g.settings.players = players;
        g.settings.controllers.fill(Ctrl::Human);
        g.settings.play.mode = mode;
        g.settings.landscape.sky = crate::settings::Sky::Plain;
        g.settings.landscape.scanned_mountains_percent = 0;
        g.settings.landscape.max_wind = 0;
        g.settings.weapons.tunneling = false;
        g.settings.hardware.falling_delay = 0;
        g.start_game();
        g.terrain = Terrain::flat(300);
        g.wind = 0;
        g.turn_order = crate::play::Order::sequential(players);
        for (i, tank) in g.tanks.iter_mut().enumerate() {
            tank.x = 70 + i as i32 * (500 / (players as i32 - 1));
            tank.y = 299;
            tank.weapon = ItemId::Tracer;
            tank.angle = 90;
            tank.power = 200;
        }
        g.current = 0;
        g.begin_turn();
        g
    }

    #[test]
    fn synchronous_three_players_queue_without_launching_then_share_one_volley() {
        let mut g = concurrent_game(PlayMode::Synchronous, 3);
        let initial: Vec<_> = g.tanks.iter().map(|t| t.inventory.clone()).collect();
        g.settings.play.order = crate::settings::PlayOrder::RoundRobin;
        g.fire();
        assert_eq!(g.current, 1);
        assert!(g.phase == Phase::Aim && g.projs.is_empty());
        assert!(g.concurrent.queued[0].is_some());
        assert_eq!(g.tanks[0].inventory, initial[0]);
        g.fire();
        assert_eq!(g.current, 2);
        assert!(g.projs.is_empty());
        g.fire();
        assert!(g.phase == Phase::Resolve);
        assert_eq!(g.projs.len(), 3);
        for i in 0..3 {
            let p = g.projs.iter().find(|p| p.owner == i).unwrap();
            assert_eq!(p.age, 0.0);
            assert_eq!(p.vel.y, -200.0);
            assert_eq!(g.tanks[i].inventory.count(ItemId::Tracer), 19);
        }
        for _ in 0..3000 {
            g.update_resolve(0.02);
            if g.phase != Phase::Resolve {
                break;
            }
        }
        assert!(g.phase == Phase::Aim);
        assert!(g.tanks.iter().all(|t| t.alive && t.health == 100));
        assert!(g.concurrent.queued.iter().all(Option::is_none));
        assert!(g.projs.is_empty());
    }

    #[test]
    fn synchronous_energy_and_targeted_guidance_wait_for_the_last_player() {
        let mut g = concurrent_game(PlayMode::Synchronous, 3);
        g.settings.play.order = crate::settings::PlayOrder::RoundRobin;
        for t in &mut g.tanks {
            t.inventory.add(ItemId::Battery, 10);
        }
        g.tanks[0].weapon = ItemId::Laser;
        g.tanks[1].inventory.add(ItemId::PlasmaBlast, 1);
        g.tanks[1].weapon = ItemId::PlasmaBlast;
        g.tanks[2].weapon = ItemId::Missile;
        g.tanks[2].inventory.add(ItemId::BallisticGuidance, 1);
        g.tanks[2].equipment.guidance = Some(ItemId::BallisticGuidance);
        g.tanks[0].inventory.add(ItemId::Laser, 1);
        g.fire();
        assert!(g.phase == Phase::Batteries);
        g.discharge_energy(2);
        g.fire();
        g.discharge_energy(3);
        assert!(g.concurrent.energy.is_empty());
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 10);
        assert_eq!(g.tanks[1].inventory.count(ItemId::Battery), 10);
        g.fire();
        assert!(g.phase == Phase::Target);
        g.launch(Some(vec2(400.0, 299.0)));
        assert!(g.phase == Phase::Resolve);
        assert_eq!(g.concurrent.energy.len(), 2);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 8);
        assert_eq!(g.tanks[1].inventory.count(ItemId::Battery), 7);
        assert_eq!(g.projs.len(), 1);
        assert_eq!(g.projs[0].owner, 2);
        assert_eq!(g.tanks[2].inventory.count(ItemId::BallisticGuidance), 0);
    }

    #[test]
    fn synchronous_dead_ready_players_are_skipped_and_same_volley_deaths_do_not_cancel_shots() {
        let mut g = concurrent_game(PlayMode::Synchronous, 3);
        g.settings.play.order = crate::settings::PlayOrder::RoundRobin;
        g.fire();
        g.tanks[0].health = 0;
        g.fire();
        g.fire();
        assert_eq!(g.projs.len(), 2);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Tracer), 20);
        let mut g = concurrent_game(PlayMode::Synchronous, 2);
        g.tanks[0].x = 200;
        g.tanks[1].x = 230;
        g.tanks[0].inventory.add(ItemId::PlasmaBlast, 1);
        g.tanks[0].inventory.add(ItemId::Battery, 10);
        g.tanks[0].weapon = ItemId::PlasmaBlast;
        g.discharge_energy(10);
        g.fire();
        assert!(g.tanks[1].health <= 0);
        assert_eq!(g.projs.len(), 1);
        assert_eq!(g.projs[0].owner, 1);
        assert_eq!(g.tanks[1].inventory.count(ItemId::Tracer), 19);
    }

    #[test]
    fn simultaneous_two_humans_can_aim_while_busy_and_refire_independently() {
        use crate::modes::Input;
        let mut g = concurrent_game(PlayMode::Simultaneous, 2);
        let fire = Input {
            fire: true,
            ..Default::default()
        };
        g.simultaneous_input(0.02, &[fire, fire]);
        assert!(g.phase == Phase::Simultaneous);
        assert_eq!(g.projs.len(), 2);
        assert!(g.owner_busy(0) && g.owner_busy(1));
        let power = g.tanks[0].power;
        g.simultaneous_input(
            0.02,
            &[
                Input {
                    power_up: 1,
                    clockwise: 1,
                    ..fire
                },
                fire,
            ],
        );
        assert_eq!(g.tanks[0].power, power + 2);
        assert_eq!(g.tanks[0].angle, 89);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Tracer), 19);
        assert_eq!(g.projs.len(), 2);
        g.projs.retain(|p| p.owner == 1);
        g.simultaneous_input(0.02, &[fire, fire]);
        assert_eq!(g.projs.len(), 2);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Tracer), 18);
        assert_eq!(g.tanks[1].inventory.count(ItemId::Tracer), 19);
        assert_eq!(
            g.projs
                .iter()
                .find(|p| p.owner == 0)
                .unwrap()
                .vel
                .length()
                .round() as i32,
            power + 2
        );
    }

    #[test]
    fn simultaneous_automatic_supplies_and_flashes_obey_health_and_defense_rules() {
        let mut g = concurrent_game(PlayMode::Simultaneous, 3);
        for t in &mut g.tanks {
            t.inventory.add(ItemId::Battery, 5);
            t.inventory.add(ItemId::Shield, 1);
        }
        g.tanks[0].health = 90;
        g.tanks[1].health = 91;
        g.tanks[2].health = 0;
        g.automatic_batteries();
        assert_eq!(g.tanks[0].health, 100);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 4);
        assert_eq!(g.tanks[1].health, 91);
        assert_eq!(g.tanks[1].inventory.count(ItemId::Battery), 5);
        assert_eq!(g.tanks[2].health, 0);
        assert!(g.concurrent.flash[0].is_some());
        assert!(g
            .tanks
            .iter()
            .all(|t| t.equipment.parachutes_deployed && t.equipment.active_shield.is_none()));
        g.simultaneous_input(
            0.01,
            &[crate::modes::Input {
                power_down: 1,
                ..Default::default()
            }],
        );
        assert_eq!(g.concurrent.flash[0].unwrap().0, Rgb(35, 35, 35));
        g.simultaneous_input(
            0.01,
            &[crate::modes::Input {
                weapon: true,
                ..Default::default()
            }],
        );
        assert_eq!(g.concurrent.flash[0].unwrap().0, Rgb(255, 30, 30));
        g.tanks[0].equipment.guidance = Some(ItemId::BallisticGuidance);
        g.tanks[0].inventory.add(ItemId::BallisticGuidance, 1);
        g.tanks[0].weapon = ItemId::Missile;
        g.current = 0;
        g.launch(None);
        assert!(g.projs.last().unwrap().guidance.is_none());
        assert_eq!(g.tanks[0].inventory.count(ItemId::BallisticGuidance), 1);
    }

    #[test]
    fn simultaneous_deaths_resolve_during_flight_and_round_awards_wait_for_all_effects() {
        let mut g = concurrent_game(PlayMode::Simultaneous, 2);
        g.settings.rounds = 1;
        let fire = crate::modes::Input {
            fire: true,
            ..Default::default()
        };
        g.simultaneous_input(0.02, &[fire, fire]);
        g.tanks[1].health = 0;
        g.tanks[1].last_hit = Some(0);
        g.update_resolve(0.01);
        assert!(!g.tanks[1].alive);
        assert!(!g.projs.is_empty());
        assert!(!g.round_paid);
        assert_eq!(g.tanks[0].kills, 1);
        let count = g.projs.len();
        g.simultaneous_input(0.01, &[fire, fire]);
        assert_eq!(g.projs.len(), count);
        for _ in 0..3000 {
            g.update_resolve(0.02);
            if g.phase == Phase::RoundOver {
                break;
            }
        }
        assert!(g.phase == Phase::RoundOver);
        assert_eq!(g.tanks[0].kills, 1);
        assert!(g.round_paid);
        g.continue_after_round();
        assert!(g.phase == Phase::GameOver);
    }

    #[test]
    fn concurrent_computer_matches_finish_in_both_modes() {
        for mode in [PlayMode::Synchronous, PlayMode::Simultaneous] {
            let mut g = concurrent_game(mode, 3);
            g.settings.rounds = 1;
            for (i, t) in g.tanks.iter_mut().enumerate() {
                t.ctrl = Ctrl::Ai(ai::AiKind::Shooter);
                t.ai_state = ai::State::new(t.ctrl);
                t.x = 230 + i as i32 * 55;
                t.weapon = ItemId::BabyMissile;
            }
            for _ in 0..12000 {
                g.phase_t += 0.05;
                match g.phase {
                    Phase::Aim => g.update_aim(0.05),
                    Phase::Simultaneous => {
                        g.simultaneous_input(0.05, &[]);
                        g.update_resolve(0.05);
                    }
                    Phase::Resolve => g.update_resolve(0.05),
                    Phase::RoundOver => g.continue_after_round(),
                    Phase::GameOver => break,
                    _ => panic!("unexpected phase in {mode:?}"),
                }
            }
            assert!(g.phase == Phase::GameOver, "mode {mode:?} failed to finish");
            assert!(g.tanks.iter().any(|t| t.kills > 0));
        }
    }

    #[test]
    fn simultaneous_earth_liquid_and_energy_effects_keep_separate_owner_locks() {
        let mut g = concurrent_game(PlayMode::Simultaneous, 3);
        g.tanks[0].weapon = ItemId::RiotCharge;
        g.tanks[0].inventory.add(ItemId::RiotCharge, 1);
        g.tanks[1].weapon = ItemId::EarthDisrupter;
        g.tanks[1].inventory.add(ItemId::EarthDisrupter, 1);
        let fire = crate::modes::Input {
            fire: true,
            ..Default::default()
        };
        g.simultaneous_input(0.01, &[fire, fire]);
        assert!(g.owner_busy(0) && g.owner_busy(1));
        assert!(!g.owner_busy(2));
        g.update_resolve(0.6);
        assert!(!g.owner_busy(0));
        assert!(g.owner_busy(1));
        g.tanks[0].weapon = ItemId::Tracer;
        g.simultaneous_input(0.01, &[fire, fire]);
        assert_eq!(g.projs.len(), 1);
        assert_eq!(g.projs[0].owner, 0);

        let mut g = concurrent_game(PlayMode::Simultaneous, 3);
        for i in 0..2 {
            g.current = i;
            g.tanks[i].weapon = ItemId::Laser;
            g.tanks[i].inventory.add(ItemId::Laser, 1);
            g.tanks[i].inventory.add(ItemId::Battery, 2);
            g.discharge_energy(2);
        }
        assert_eq!(g.concurrent.energy.len(), 2);
        assert!(g.owner_busy(0) && g.owner_busy(1) && !g.owner_busy(2));
        g.update_resolve(0.6);
        assert!(!g.owner_busy(0) && !g.owner_busy(1));

        for i in 0..2 {
            g.current = i;
            g.tanks[i].weapon = ItemId::Tracer;
            g.launch(None);
            let mut p = g.projs.pop().unwrap();
            p.weapon = ItemId::LiquidDirt;
            let top = g.field_top();
            assert!(g
                .liquid
                .impact(&p, vec2(200.0 + i as f32 * 200.0, 299.0), top));
        }
        assert!(g.owner_busy(0) && g.owner_busy(1) && !g.owner_busy(2));
        for _ in 0..2000 {
            g.update_resolve(0.02);
            if !g.liquid.active() {
                break;
            }
        }
        assert!(!g.liquid.active());
        assert!(!g.owner_busy(0) && !g.owner_busy(1));
    }

    #[test]
    fn concurrent_support_loss_credits_the_shot_owner_instead_of_the_last_shooter() {
        let mut g = concurrent_game(PlayMode::Simultaneous, 3);
        g.settings.physics.suspend_dirt = 100;
        g.settle_dirt = false;
        let victim = 1;
        g.tanks[victim].inventory.remove(ItemId::Parachute, 99);
        // Riot has no damage but removing support must still attribute the fall.
        let mut p = {
            g.current = 0;
            g.launch(None);
            g.projs.pop().unwrap()
        };
        p.weapon = ItemId::HeavyRiotBomb;
        p.radius = 45.0;
        let physics = g.physics();
        let effects = g.earth.impact(
            p,
            vec2(g.tanks[victim].x as f32, 300.0),
            None,
            false,
            &physics,
            &mut g.terrain,
            &mut g.tanks,
        );
        g.apply_earth_update(effects);
        g.current = 2; // another shooter owns neither the excavation nor the fall
        g.tanks[2].weapon = ItemId::Tracer;
        g.launch(None);
        g.update_falling(0.1);
        assert!(g.tanks[victim].health < 100);
        assert_eq!(g.tanks[victim].last_hit, Some(0));
        assert!(!g.tanks[victim].environmental_hit);
    }

    #[test]
    fn simultaneous_pause_and_reassignment_preserve_shots_and_validate_key_maps() {
        let mut g = concurrent_game(PlayMode::Simultaneous, 3);
        let fire = crate::modes::Input {
            fire: true,
            ..Default::default()
        };
        g.simultaneous_input(0.02, &[fire, fire]);
        let before: Vec<_> = g.projs.iter().map(|p| (p.owner, p.age, p.pos)).collect();
        g.system = Some(crate::system::System::default());
        for _ in 0..30 {
            g.system_input(&ui::MenuInput::default());
        }
        assert_eq!(
            g.projs
                .iter()
                .map(|p| (p.owner, p.age, p.pos))
                .collect::<Vec<_>>(),
            before
        );
        g.stage_players();
        g.edit_settings.as_mut().unwrap().keys[1][0] = g.settings.keys[0][0];
        assert!(!g.apply_players());
        assert!(g.edit_settings.is_some());
        g.edit_settings.as_mut().unwrap().keys[1][0] = crate::modes::Key::Kp0;
        assert!(g.apply_players());
        assert_eq!(g.settings.keys[1][0], crate::modes::Key::Kp0);
        assert_eq!(
            g.projs
                .iter()
                .map(|p| (p.owner, p.age, p.pos))
                .collect::<Vec<_>>(),
            before
        );
        g.system_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        assert!(g.system.is_none());
        assert!(g.phase == Phase::Simultaneous);
    }

    #[test]
    fn simultaneous_temporary_traces_are_replaced_per_owner_and_clear_screen_clears_everyone() {
        let mut g = concurrent_game(PlayMode::Simultaneous, 2);
        let fire = crate::modes::Input {
            fire: true,
            ..Default::default()
        };
        g.simultaneous_input(0.02, &[fire, fire]);
        g.update_resolve(0.1);
        assert!(g.trail.is_empty());
        assert!(g.concurrent.trails.iter().all(|trail| !trail.is_empty()));
        let other = g.concurrent.trails[1].clone();
        g.projs.retain(|p| p.owner == 1);
        g.simultaneous_input(0.01, &[fire, fire]);
        assert!(g.concurrent.trails[0].is_empty());
        assert_eq!(g.concurrent.trails[1], other);
        g.clear_traces();
        assert!(g.concurrent.trails.iter().all(Vec::is_empty));
        assert_eq!(g.projs.len(), 2);
    }

    #[test]
    fn saved_games_enter_the_selected_concurrent_mode_with_clean_runtime_state() {
        for mode in [PlayMode::Synchronous, PlayMode::Simultaneous] {
            let (mut g, directory) = saved_game_test();
            g.settings.play.mode = mode;
            let path = directory.join("concurrent.toml");
            g.checkpoint().unwrap().save(&path).unwrap();
            g.restore_game(&path).unwrap();
            g.start_round();
            assert_eq!(g.concurrent.queued.len(), 3);
            assert!(g.concurrent.queued.iter().all(Option::is_none));
            assert!(g.concurrent.energy.is_empty());
            if mode == PlayMode::Simultaneous {
                assert!(g.phase == Phase::Simultaneous);
                g.simultaneous_input(
                    0.02,
                    &[crate::modes::Input {
                        fire: true,
                        ..Default::default()
                    }],
                );
                assert!(g.projs.iter().any(|p| p.owner == 0));
            } else {
                assert!(g.phase == Phase::Aim);
            }
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn concurrent_modes_previews() {
        let Ok(directory) = std::env::var("SCORCHED_MODES_PREVIEW_DIR") else {
            return;
        };
        std::fs::create_dir_all(&directory).unwrap();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            let mut image = fb.img.clone();
            image.bytes = image
                .bytes
                .as_chunks::<{ (W * 4) as usize }>()
                .0
                .iter()
                .rev()
                .flatten()
                .copied()
                .collect();
            image.export_png(&format!("{directory}/{name}.png"));
        };
        let mut g = concurrent_game(PlayMode::Synchronous, 3);
        g.settings.play.order = crate::settings::PlayOrder::RoundRobin;
        g.fire();
        g.fire();
        g.draw(&mut fb);
        save(&fb, "synchronous-ready");
        g.fire();
        g.update_resolve(0.2);
        g.draw(&mut fb);
        save(&fb, "synchronous-volley");
        let mut g = concurrent_game(PlayMode::Simultaneous, 2);
        let fire = crate::modes::Input {
            fire: true,
            ..Default::default()
        };
        g.simultaneous_input(0.02, &[fire, fire]);
        g.update_resolve(0.2);
        g.concurrent.flash[0] = Some((Rgb::WHITE, 0.1));
        g.concurrent.flash[1] = Some((Rgb(255, 30, 30), 0.1));
        g.draw(&mut fb);
        save(&fb, "simultaneous-two-humans");
        let g = concurrent_game(PlayMode::Simultaneous, 10);
        g.draw(&mut fb);
        save(&fb, "simultaneous-ten-players");
        let menu = MenuState {
            page: Some(ui::Page::Play),
            ..Default::default()
        };
        ui::draw_menu(&mut fb, &g.settings, &menu);
        save(&fb, "play-mode-option");
        let editor = crate::modes::KeyEditor::new(&g.settings, 1);
        editor.draw(&mut fb);
        save(&fb, "key-editor");
    }

    fn saved_game_test() -> (Game, std::path::PathBuf) {
        static SERIAL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let directory =
            std::env::temp_dir().join(format!("scorched-saves-{}-{id}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut game = Game::new();
        game.config_path = Some(directory.join("settings.toml"));
        game.settings.players = 3;
        game.settings.controllers[..3].copy_from_slice(&[
            Ctrl::Human,
            Ctrl::Human,
            Ctrl::Ai(ai::AiKind::Unknown),
        ]);
        game.settings.economics.cash_at_start = 50000;
        game.settings.economics.free_market = true;
        game.start_game();
        assert!(game.phase == Phase::Shop);
        (game, directory)
    }

    #[test]
    fn hall_of_fame_records_only_completed_matches_once_and_survives_restore() {
        let (mut g, directory) = saved_game_test();
        let path = directory.join("hall_of_fame.toml");
        g.hall_path = Some(path.clone());
        g.settings.rounds = 2;
        skip_human_shops(&mut g);
        g.finish_round(&[0]);
        g.continue_after_round();
        assert!(!path.exists());
        assert_eq!(g.hall.entries(g.settings.economics.scoring).count(), 0);
        let checkpoint = directory.join("checkpoint.toml");
        g.checkpoint().unwrap().save(&checkpoint).unwrap();
        skip_human_shops(&mut g);
        g.finish_round(&[0]);
        g.continue_after_round();
        let mode = g.settings.economics.scoring;
        let entries: Vec<_> = g.hall.entries(mode).cloned().collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(
            entries,
            crate::hall_of_fame::HallOfFame::load(&path)
                .unwrap()
                .entries(mode)
                .cloned()
                .collect::<Vec<_>>()
        );
        let saved = std::fs::read(&path).unwrap();
        g.continue_after_round();
        assert_eq!(std::fs::read(&path).unwrap(), saved);
        g.phase_t = 1.0;
        g.standings_input(&ui::MenuInput {
            activate: true,
            ..Default::default()
        });
        assert!(g.hall_open && g.phase == Phase::Menu);
        g.hall_input(&ui::MenuInput {
            click: Some((500, 410, 1)),
            ..Default::default()
        });
        assert!(!g.hall_open);
        g.restore_game(&checkpoint).unwrap();
        assert_eq!(g.hall.entries(mode).cloned().collect::<Vec<_>>(), entries);
        assert_eq!(g.hall_path, Some(path));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn hall_of_fame_load_errors_preserve_file_and_save_errors_keep_session_scores() {
        let (mut g, directory) = saved_game_test();
        let path = directory.join("hall_of_fame.toml");
        std::fs::write(&path, "invalid TOML").unwrap();
        g.load_settings(directory.join("settings.toml"));
        assert!(g.hall_notice.is_some() && g.hall_path.is_none());
        g.settings.rounds = 1;
        g.round = 1;
        g.finish_round(&[0]);
        g.continue_after_round();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid TOML");
        assert_eq!(g.hall.entries(g.settings.economics.scoring).count(), 3);
        let original = g.hall.entries(g.settings.economics.scoring).count();
        g.hall_path = Some(directory.clone()); // A directory cannot be replaced by a score file.
        g.round_paid = false;
        g.finish_round(&[]);
        assert!(g.hall_notice.as_ref().unwrap().contains("Cannot save"));
        assert_eq!(
            g.hall.entries(g.settings.economics.scoring).count(),
            original * 2
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn hall_of_fame_menu_mouse_keyboard_navigation_and_restart_loading() {
        let (g, directory) = saved_game_test();
        let mut loaded = Game::new();
        loaded.load_settings(directory.join("settings.toml"));
        assert!(loaded.hall_notice.is_none()); // Missing file is a normal first launch.
        let action = loaded.menu.handle(
            &mut loaded.settings,
            &ui::MenuInput {
                hotkey: Some('F'),
                ..Default::default()
            },
        );
        assert_eq!(action, MenuAction::HallOfFame);
        assert_eq!(
            loaded.menu.handle(
                &mut loaded.settings,
                &ui::MenuInput {
                    click: Some((100, 410, 1)),
                    ..Default::default()
                }
            ),
            MenuAction::HallOfFame
        );
        loaded.open_hall();
        loaded.hall_input(&ui::MenuInput {
            left: true,
            ..Default::default()
        });
        assert_eq!(loaded.hall_mode, crate::settings::ScoringMode::Basic);
        loaded.hall_input(&ui::MenuInput {
            left: true,
            ..Default::default()
        });
        assert_eq!(loaded.hall_mode, crate::settings::ScoringMode::Greedy);
        loaded.hall_input(&ui::MenuInput {
            click: Some((300, 410, 1)),
            ..Default::default()
        });
        assert_eq!(loaded.hall_mode, crate::settings::ScoringMode::Basic);
        loaded.hall_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        assert!(!loaded.hall_open);
        loaded
            .hall
            .record(&g.tanks, &g.prices, crate::settings::ScoringMode::Basic, 1);
        loaded
            .hall
            .save(loaded.hall_path.as_ref().unwrap())
            .unwrap();
        let mut restarted = Game::new();
        restarted.load_settings(directory.join("settings.toml"));
        assert_eq!(
            restarted
                .hall
                .entries(crate::settings::ScoringMode::Basic)
                .count(),
            3
        );
        let path = directory.join("hall_of_fame.toml");
        std::fs::write(&path, vec![b' '; 256 * 1024 + 1]).unwrap();
        assert!(crate::hall_of_fame::HallOfFame::load(&path).is_err());
        std::fs::remove_dir_all(directory).unwrap();
    }
    fn system_key(game: &mut Game, c: char) {
        game.system_input(&ui::MenuInput {
            hotkey: Some(c),
            ..Default::default()
        });
    }
    fn system_submit(game: &mut Game) {
        game.system_input(&ui::MenuInput {
            submit: true,
            activate: true,
            ..Default::default()
        });
    }

    #[test]
    fn saved_game_resumes_the_current_buyer_market_stock_ai_and_round_accounting() {
        let (mut g, directory) = saved_game_test();
        g.settings.rounds = 2;
        g.settings.play.teams = TeamMode::Corporate;
        g.settings.team_ids[..3].copy_from_slice(&[0, 0, 1]);
        g.apply_team_assignments();
        g.start_round();
        g.finish_round(&[0, 1]);
        g.continue_after_round();
        assert_eq!(g.round, 2);
        assert!(shop::buy(
            &mut g.tanks[0],
            ItemId::Missile,
            &g.prices,
            &g.settings,
            &mut g.market
        ));
        economy::sync_bank(&mut g.tanks, 0);
        g.tanks[0].equipment.fuel = 7;
        g.tanks[0].equipment.guidance = Some(ItemId::LazyBoy);
        g.tanks[0].inventory.add(ItemId::LazyBoy, 2);
        g.tanks[0].ai_state.grudges[2] = 17;
        g.shop.player += 1;
        g.advance_shop();
        let saved = g.checkpoint().unwrap();
        let path = directory.join("round-two.toml");
        saved.save(&path).unwrap();
        let accounts: Vec<_> = g.tanks.iter().map(|t| t.account.clone()).collect();
        let inventories: Vec<_> = g.tanks.iter().map(|t| t.inventory.clone()).collect();
        let personality = g.tanks[2].ai_state.resolved;
        let circle = g.turn_order.circle.clone();
        let prices = g.prices;
        let market = toml::to_string(&g.market).unwrap();
        let mut restored = Game::new();
        restored.settings.physics.gravity = 0.65;
        restored.settings.sound.enabled = false;
        restored.restore_game(&path).unwrap();
        assert!(restored.phase == Phase::Shop);
        assert_eq!(restored.shop.player, 1);
        assert_eq!(restored.round, 2);
        assert_eq!(restored.interest_round, 1);
        assert_eq!(restored.settings.rounds, g.settings.rounds);
        assert_eq!(restored.settings.players, 3);
        assert_eq!(restored.settings.physics.gravity, 0.65);
        assert!(!restored.settings.sound.enabled);
        // Changing team options from Corporate splits funds instead of duplicating them.
        let mut total = economy::Money::default();
        for tank in &restored.tanks {
            total.add(&tank.account.cash);
        }
        let mut expected_total = accounts[0].cash.clone();
        expected_total.add(&accounts[2].cash);
        assert_eq!(total, expected_total);
        g.restore_game(&path).unwrap();
        assert!(g.phase == Phase::Shop);
        assert_eq!(g.shop.player, 1);
        assert_eq!(g.turn_order.circle, circle);
        assert_eq!(g.prices, prices);
        assert_eq!(toml::to_string(&g.market).unwrap(), market);
        for (i, tank) in g.tanks.iter().enumerate() {
            assert_eq!(tank.account, accounts[i]);
            assert_eq!(tank.inventory, inventories[i]);
        }
        assert_eq!(g.tanks[2].ai_state.resolved, personality);
        assert_eq!(g.tanks[0].ai_state.grudges[2], 17);
        assert_eq!(g.tanks[0].equipment.fuel, 7);
        assert_eq!(g.tanks[0].equipment.guidance, Some(ItemId::LazyBoy));
        let mut expected = accounts[0].clone();
        expected.pay_interest(g.settings.economics.interest_rate);
        g.start_round();
        assert_eq!(g.tanks[0].account, expected);
        assert_eq!(g.tanks[1].account, expected);
        g.start_round();
        assert_eq!(g.tanks[0].account, expected);
        g.mass_kill();
        g.continue_after_round();
        assert!(g.phase == Phase::GameOver);
        assert_eq!(g.round, 2);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saved_game_keeps_completed_computer_purchases_and_accepts_space_in_filenames() {
        let (mut g, directory) = saved_game_test();
        g.settings.controllers[0] = Ctrl::Ai(ai::AiKind::Cyborg);
        g.start_game();
        assert_eq!(g.shop.player, 1);
        let stock = g.tanks[0].inventory.clone();
        let account = g.tanks[0].account.clone();
        let market = toml::to_string(&g.market).unwrap();
        g.system = Some(crate::system::System::default());
        system_key(&mut g, 'V');
        g.system_input(&ui::MenuInput {
            home: true,
            text: "round one.tomlx".into(),
            activate: true,
            ..Default::default()
        });
        assert!(!directory.join("saves/round one.tomlx").exists());
        g.system_input(&ui::MenuInput {
            backspace: true,
            ..Default::default()
        });
        system_submit(&mut g);
        let path = directory.join("saves/round one.toml");
        g.restore_game(&path).unwrap();
        assert_eq!(g.shop.player, 1);
        assert_eq!(g.tanks[0].inventory, stock);
        assert_eq!(g.tanks[0].account, account);
        assert_eq!(toml::to_string(&g.market).unwrap(), market);
        // An interrupted atomic write must leave the previous checkpoint readable.
        let bytes = std::fs::read(&path).unwrap();
        let temporary = path.with_file_name(format!("round one.toml.{}.tmp", std::process::id()));
        std::fs::write(&temporary, "occupied").unwrap();
        assert!(g.checkpoint().unwrap().save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(crate::saved_game::SavedGame::load(&path).is_ok());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saved_game_previews() {
        let Ok(directory) = std::env::var("SCORCHED_SAVED_GAME_PREVIEW_DIR") else {
            return;
        };
        std::fs::create_dir_all(&directory).unwrap();
        let (mut g, scratch) = saved_game_test();
        let mut fb = Fb::new();
        for (name, phase, panel) in [
            ("shop-system", Phase::Shop, crate::system::Panel::Menu),
            (
                "save-file",
                Phase::Shop,
                crate::system::Panel::File {
                    command: crate::system::Command::Save,
                    path: "round-two.toml".into(),
                },
            ),
            (
                "overwrite",
                Phase::Shop,
                crate::system::Panel::Overwrite {
                    path: "round-two.toml".into(),
                },
            ),
            (
                "restore-file",
                Phase::Menu,
                crate::system::Panel::File {
                    command: crate::system::Command::Restore,
                    path: "round-two.toml".into(),
                },
            ),
            (
                "file-error",
                Phase::Shop,
                crate::system::Panel::Message(
                    "Cannot read saved game: No such file or directory".into(),
                ),
            ),
        ] {
            g.phase = phase;
            g.system = Some(crate::system::System {
                panel,
                can_save: phase == Phase::Shop,
                ..Default::default()
            });
            g.draw(&mut fb);
            let mut image = fb.img.clone();
            image.bytes = image
                .bytes
                .as_chunks::<{ (W * 4) as usize }>()
                .0
                .iter()
                .rev()
                .flatten()
                .copied()
                .collect();
            image.export_png(&format!("{directory}/{name}.png"));
        }
        std::fs::remove_dir_all(scratch).unwrap();
    }

    #[test]
    fn saved_game_system_flow_confirms_overwrite_and_cancels_without_touching_the_file() {
        use crate::system::{Command, Panel, System};
        let (mut g, directory) = saved_game_test();
        g.system = Some(System::default());
        system_key(&mut g, 'V');
        assert!(matches!(
            g.system.as_ref().unwrap().panel,
            Panel::File {
                command: Command::Save,
                ..
            }
        ));
        system_submit(&mut g);
        let path = directory.join("saves/game.toml");
        let original = std::fs::read(&path).unwrap();
        system_submit(&mut g); // dismiss success
        g.tanks[0].inventory.add(ItemId::Nuke, 4);
        system_key(&mut g, 'V');
        system_submit(&mut g);
        assert!(matches!(
            g.system.as_ref().unwrap().panel,
            Panel::Overwrite { .. }
        ));
        system_key(&mut g, 'N');
        assert_eq!(std::fs::read(&path).unwrap(), original);
        system_key(&mut g, 'V');
        system_submit(&mut g);
        g.system_input(&mouse(440, 410)); // cancel overwrite
        assert_eq!(std::fs::read(&path).unwrap(), original);
        system_key(&mut g, 'V');
        system_submit(&mut g);
        g.system_input(&mouse(160, 410)); // confirm overwrite
        assert_ne!(std::fs::read(&path).unwrap(), original);
        system_submit(&mut g);
        system_key(&mut g, 'V');
        g.system_input(&ui::MenuInput {
            home: true,
            text: "discard.toml".into(),
            ..Default::default()
        });
        g.system_input(&mouse(440, 410)); // cancel filename
        assert!(!directory.join("saves/discard.toml").exists());
        // Restore works from the title menu, using the same mouse submission path.
        g.phase = Phase::Menu;
        system_key(&mut g, 'R');
        g.system_input(&mouse(160, 410));
        assert!(g.phase == Phase::Shop);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Nuke), 5);
        assert!(matches!(
            g.system.as_ref().unwrap().panel,
            Panel::Message(_)
        ));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn saved_game_failures_preserve_a_live_shot_and_restore_discards_old_effects() {
        use crate::system::{Panel, System};
        let (mut g, directory) = saved_game_test();
        let path = directory.join("good.toml");
        g.checkpoint().unwrap().save(&path).unwrap();
        g.start_round();
        g.current = 0;
        g.fire();
        let pos = g.projs[0].pos;
        let age = g.projs[0].age;
        let account = g.tanks[0].account.clone();
        g.system = Some(System::default());
        system_key(&mut g, 'V');
        assert!(matches!(
            g.system.as_ref().unwrap().panel,
            Panel::Message(_)
        ));
        assert!(g.checkpoint().is_err());
        let bad = directory.join("bad.toml");
        for data in [
            "truncated = [".to_string(),
            "x".repeat(1024 * 1024 + 1),
            std::fs::read_to_string(&path)
                .unwrap()
                .replace("version = 1", "version = 2"),
        ] {
            std::fs::write(&bad, data).unwrap();
            assert!(g.restore_game(&bad).is_err());
            assert!(g.phase == Phase::Resolve);
            assert_eq!(g.projs[0].pos, pos);
            assert_eq!(g.projs[0].age, age);
            assert_eq!(g.tanks[0].account, account);
        }
        assert!(g.restore_game(&directory.join("missing.toml")).is_err());
        g.trail.push((5, 50, Rgb::WHITE));
        system_submit(&mut g); // dismiss unavailable-save notice
        system_key(&mut g, 'R');
        g.system_input(&ui::MenuInput {
            home: true,
            text: path.to_str().unwrap().into(),
            ..Default::default()
        });
        system_submit(&mut g);
        assert!(g.phase == Phase::Shop);
        assert!(g.projs.is_empty() && g.expls.is_empty() && g.trail.is_empty());
        assert!(g.ai_plan.is_none());
        assert_eq!(g.round, 1);
        // Write failure leaves the paused shop and an existing destination intact.
        let snapshot = g.checkpoint().unwrap();
        assert!(snapshot.save(&directory).is_err());
        assert!(g.phase == Phase::Shop);
        assert!(path.is_file());
        std::fs::remove_dir_all(directory).unwrap();
    }

    fn mouse(x: i32, y: i32) -> ui::MenuInput {
        ui::MenuInput {
            click: Some((x, y, 1)),
            ..Default::default()
        }
    }

    #[test]
    fn mouse_target_cancel_move_and_system_confirmation_use_visible_buttons() {
        let mut g = guided_game(ItemId::BallisticGuidance);
        g.aim_mouse(ui::BattleClick::Fire);
        assert!(g.phase == Phase::Target);
        g.target_input(&mouse(320, 410));
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].inventory.count(ItemId::BallisticGuidance), 2);
        g.aim_mouse(ui::BattleClick::Fire);
        g.target_input(&mouse(300, 295));
        assert!(g.phase == Phase::Resolve);
        g = energy_game(ItemId::Missile, 0);
        g.tanks[0].inventory.add(ItemId::FuelTank, 1);
        g.begin_move();
        assert!(g.phase == Phase::Move);
        let x = g.tanks[0].x;
        g.move_input(&mouse(320, 410));
        assert_eq!(g.tanks[0].x, x + 1);
        g.move_input(&mouse(500, 410));
        assert!(g.phase == Phase::Aim);
        g.round = 3;
        g.tanks[0].health = 60;
        g.system = Some(crate::system::System::default());
        g.system.as_mut().unwrap().panel =
            crate::system::Panel::Confirm(crate::system::Command::NewGame);
        g.system_input(&mouse(500, 410));
        assert!(g.phase == Phase::Aim);
        g.system.as_mut().unwrap().panel =
            crate::system::Panel::Confirm(crate::system::Command::NewGame);
        g.system_input(&mouse(100, 410));
        assert_eq!(g.tanks[0].health, 100);
        assert!(g.system.is_none());
        assert_eq!(g.round, 1);
    }

    #[test]
    fn batch_twenty_one_previews() {
        let mut g = energy_game(ItemId::Laser, 10);
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Ok(dir) = std::env::var("SCORCHED_PRESENTATION_PREVIEW_DIR") {
                std::fs::create_dir_all(&dir).unwrap();
                let mut img = fb.img.clone();
                img.bytes = img
                    .bytes
                    .as_chunks::<{ W as usize * 4 }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                img.export_png(&format!("{dir}/{name}.png"));
            }
        };
        for (name, panel) in [
            ("mouse-control", controls::Panel::Control),
            ("mouse-inventory", controls::Panel::Inventory),
            ("mouse-threshold", controls::Panel::Threshold),
        ] {
            g.controls.panel = panel;
            g.draw(&mut fb);
            save(&fb, name);
        }
        g.controls.panel = controls::Panel::Closed;
        g.fire();
        g.draw(&mut fb);
        save(&fb, "mouse-batteries");
        g.phase = Phase::Menu;
        g.menu.page = Some(ui::Page::Hardware);
        g.draw(&mut fb);
        save(&fb, "hardware-fullscreen");
    }

    #[test]
    fn mouse_aim_panels_fire_batteries_and_standings_complete_a_match() {
        let mut g = energy_game(ItemId::Laser, 10);
        g.settings.rounds = 1;
        g.tanks[1].x = 170;
        g.tanks[2].x = 240;
        g.aim_mouse(ui::BattleClick::Click(20, 10, 1));
        assert_eq!(g.tanks[0].power, 999);
        g.aim_mouse(ui::BattleClick::Click(20, 10, -1));
        assert_eq!(g.tanks[0].power, 1000);
        g.aim_mouse(ui::BattleClick::Click(120, 10, -1));
        assert_eq!(g.tanks[0].angle, 1);
        g.aim_mouse(ui::BattleClick::Click(120, 10, 1));
        assert_eq!(g.tanks[0].angle, 0);
        g.aim_mouse(ui::BattleClick::Click(380, 10, 1));
        assert_ne!(g.tanks[0].weapon, ItemId::Laser);
        g.aim_mouse(ui::BattleClick::Click(380, 10, -1));
        assert_eq!(g.tanks[0].weapon, ItemId::Laser);
        for (direction, panel) in [
            (1, controls::Panel::Control),
            (-1, controls::Panel::Inventory),
        ] {
            g.aim_mouse(ui::BattleClick::Click(240, 10, direction));
            assert_eq!(g.controls.panel, panel);
            let action = g.controls.handle(&mut g.tanks[0], &mouse(500, 410), false);
            g.handle_control_action(action);
            assert!(g.controls.is_closed());
        }
        g.aim_mouse(ui::BattleClick::Click(170, 295, 1));
        assert!(
            matches!(&g.controls.panel, controls::Panel::Message(lines) if lines[0].contains("Tank 2:"))
        );
        g.controls.handle(&mut g.tanks[0], &mouse(320, 240), false);
        g.aim_mouse(ui::BattleClick::Fire);
        assert!(g.phase == Phase::Batteries);
        g.battery_mouse(&mouse(500, 410)); // Cancel without spending stock.
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 10);
        g.aim_mouse(ui::BattleClick::Fire);
        g.battery_mouse(&mouse(100, 410));
        assert_eq!(g.battery_input, Some(9));
        g.battery_mouse(&mouse(260, 410));
        assert_eq!(g.battery_input, Some(10));
        g.battery_mouse(&mouse(380, 410));
        assert!(g.phase == Phase::Resolve);
        for _ in 0..1000 {
            if g.phase != Phase::Resolve {
                break;
            }
            g.update_resolve(0.02);
        }
        assert!(g.phase == Phase::RoundOver);
        assert_eq!(g.tanks[0].kills, 2);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 0);
        g.phase_t = 0.1;
        g.standings_input(&mouse(300, 300));
        assert!(g.phase == Phase::RoundOver);
        g.phase_t = 1.0;
        g.standings_input(&mouse(300, 300));
        assert!(g.phase == Phase::GameOver);
        g.phase_t = 1.0;
        g.standings_input(&mouse(300, 300));
        assert!(g.phase == Phase::Menu);
    }

    #[test]
    fn nuclear_crater_waits_until_plasma_vanishes() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[1].x = 550;
        g.tanks[2].x = 600;
        g.expls.push(Explosion::new(
            vec2(320.0, 300.0),
            75.0,
            Damage::Weapon(3),
            0,
            false,
        ));
        g.set_phase(Phase::Resolve);
        g.update_resolve(2.9);
        assert!(g.terrain.solid(320, 300));
        assert_eq!(g.expls.len(), 1);
        g.update_resolve(0.2);
        assert!(!g.terrain.solid(320, 300));
        assert!(g.expls.is_empty());
    }

    fn game() -> Game {
        let mut g = Game::new();
        g.settings.players = 3;
        g.start_game();
        // Combat fixtures need fixed clearance; native v1.5 terrain can rise
        // above y=80, where several projectile tests deliberately start.
        g.terrain = Terrain::flat(101);
        g.settings.play.order = crate::settings::PlayOrder::RoundRobin;
        g.turn_order = crate::play::Order::sequential(3);
        for (i, t) in g.tanks.iter_mut().enumerate() {
            t.x = 100 + i as i32 * 200;
            t.y = 100;
        }
        g
    }

    fn energy_game(weapon: ItemId, batteries: u32) -> Game {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(300);
        for tank in &mut g.tanks {
            tank.y = 299;
            tank.ctrl = Ctrl::Human;
        }
        g.tanks[0].weapon = weapon;
        g.tanks[0].angle = 0;
        g.tanks[0].power = 1000;
        g.tanks[0].inventory.add(weapon, 2);
        g.tanks[0].inventory.add(ItemId::Battery, batteries);
        g.begin_turn();
        g
    }

    fn guided_game(item: ItemId) -> Game {
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[0].inventory.add(item, 2);
        g.tanks[0].equipment.guidance = Some(item);
        g
    }

    fn skip_human_shops(g: &mut Game) {
        for _ in 0..12 {
            if g.phase != Phase::Shop {
                break;
            }
            g.shop.player += 1;
            g.advance_shop();
        }
    }
    fn eliminate_team(g: &mut Game, killer: usize, victims: &[usize]) {
        g.terrain = Terrain::flat(300);
        for (i, tank) in g.tanks.iter_mut().enumerate() {
            tank.x = 80 + i as i32 * 160;
            tank.y = 299;
        }
        g.current = killer;
        g.action_owner = Some(killer);
        for &victim in victims {
            g.apply_explosion(Explosion::new(
                vec2(g.tanks[victim].x as f32, 299.0),
                10.0,
                Damage::Weapon(0),
                killer,
                false,
            ));
        }
        g.set_phase(Phase::Resolve);
        for _ in 0..1000 {
            if g.phase != Phase::Resolve {
                break;
            }
            g.update_resolve(0.02);
        }
    }
    #[test]
    fn two_versus_two_corporate_game_shares_shop_awards_interest_and_finishes() {
        let mut g = Game::new();
        g.settings.players = 4;
        g.settings.rounds = 2;
        g.settings.controllers.fill(Ctrl::Human);
        g.settings.play.teams = TeamMode::Corporate;
        g.settings.economics.cash_at_start = 10_000;
        g.start_game();
        assert!(g.phase == Phase::Shop);
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(20_000));
        assert!(shop::buy(
            &mut g.tanks[0],
            ItemId::Missile,
            &g.prices,
            &g.settings,
            &mut g.market
        ));
        economy::sync_bank(&mut g.tanks, 0);
        assert_eq!(g.tanks[2].account.cash, economy::Money::from(18_125));
        assert!(shop::sell(
            &mut g.tanks[2],
            ItemId::Missile,
            1,
            &g.prices,
            false
        ));
        economy::sync_bank(&mut g.tanks, 2);
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(18_425));
        skip_human_shops(&mut g);
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(19_346));
        assert_eq!(g.tanks[1].account.cash, economy::Money::from(21_000));
        eliminate_team(&mut g, 0, &[1, 3]);
        assert!(g.phase == Phase::RoundOver);
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(31_346));
        assert_eq!(g.tanks[0].account, g.tanks[2].account);
        assert_eq!(g.tanks[0].wins, 1);
        assert_eq!(g.tanks[2].wins, 1);
        let paid = g.tanks[0].account.clone();
        g.finish_round(&[0, 2]);
        assert_eq!(g.tanks[0].account, paid);
        g.continue_after_round();
        skip_human_shops(&mut g);
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(32_913));
        eliminate_team(&mut g, 1, &[0, 2]);
        assert!(g.phase == Phase::RoundOver);
        assert_eq!(g.tanks[1].account.cash, economy::Money::from(34_050));
        assert_eq!(g.tanks[1].account, g.tanks[3].account);
        g.continue_after_round();
        assert!(g.phase == Phase::GameOver);
    }

    #[test]
    fn corporate_computers_complete_two_rounds_with_real_projectiles() {
        let mut g = Game::new();
        g.settings.players = 4;
        g.settings.rounds = 2;
        g.settings.play.teams = TeamMode::Corporate;
        g.settings.controllers.fill(Ctrl::Ai(ai::AiKind::Spoiler));
        g.settings.economics.computers_buy = false;
        g.settings.hardware.firing_delay = 0;
        g.settings.hardware.falling_delay = 0;
        g.settings.landscape.sky = crate::settings::Sky::Plain;
        g.settings.landscape.max_wind = 0;
        g.start_game();
        let place = |g: &mut Game| {
            g.terrain = Terrain::flat(300);
            for (i, t) in g.tanks.iter_mut().enumerate() {
                t.x = 60 + i as i32 * 170;
                t.y = 299;
            }
        };
        place(&mut g);
        for _ in 0..3000 {
            g.phase_t += 0.2;
            match g.phase {
                Phase::Aim => g.update_aim(0.2),
                Phase::Resolve => g.update_resolve(0.02),
                Phase::MoveFall => g.update_move_fall(0.02),
                Phase::RoundOver => {
                    g.continue_after_round();
                    if g.phase == Phase::Aim {
                        place(&mut g);
                    }
                }
                Phase::GameOver => break,
                _ => panic!("computer team game opened a human-only phase"),
            }
        }
        assert!(g.phase == Phase::GameOver);
        assert_eq!(g.round, 2);
        assert_eq!(g.tanks[0].account, g.tanks[2].account);
        assert_eq!(g.tanks[1].account, g.tanks[3].account);
        assert!(g.tanks.iter().any(|t| t.wins > 0));
    }

    #[test]
    fn reassigning_teams_is_cancellable_and_regroups_banks_without_duplicate_funds() {
        let mut g = energy_game(ItemId::Missile, 0);
        for (i, tank) in g.tanks.iter_mut().enumerate() {
            tank.account.cash = economy::Money::from(100 + i as u32);
        }
        let key = |c| ui::MenuInput {
            hotkey: Some(c),
            ..Default::default()
        };
        g.system = Some(crate::system::System::default());
        g.system_input(&key('T'));
        g.edit_settings.as_mut().unwrap().play.teams = TeamMode::Corporate;
        g.system_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        assert_eq!(g.settings.play.teams, TeamMode::None);
        assert!(g.tanks.iter().all(|t| t.bank.is_none()));
        g.system_input(&key('T'));
        let settings = g.edit_settings.as_mut().unwrap();
        settings.play.teams = TeamMode::Corporate;
        settings.team_ids[..3].copy_from_slice(&[0, 0, 1]);
        let count = 5;
        let top = (H - (72 + count * 24)) / 2;
        g.system_input(&ui::MenuInput {
            click: Some((100, top + 32 + 4 * 24 + 8, 1)),
            ..Default::default()
        });
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(201));
        assert_eq!(g.tanks[1].account, g.tanks[0].account);
        g.system_input(&key('T'));
        g.edit_settings.as_mut().unwrap().team_ids[..3].copy_from_slice(&[0, 1, 0]);
        g.system_input(&key('D'));
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(202));
        assert_eq!(g.tanks[1].account.cash, economy::Money::from(101));
        g.system_input(&key('T'));
        g.edit_settings.as_mut().unwrap().play.teams = TeamMode::None;
        g.system_input(&key('D'));
        let mut total = economy::Money::default();
        for tank in &g.tanks {
            assert!(tank.bank.is_none());
            total.add(&tank.account.cash);
        }
        assert_eq!(total, economy::Money::from(303));
    }

    #[test]
    fn system_player_cancel_preserves_identity_and_new_game_resets_the_session() {
        let mut g = energy_game(ItemId::Missile, 0);
        let name = g.tanks[0].name.clone();
        let key = |c| ui::MenuInput {
            hotkey: Some(c),
            ..Default::default()
        };
        g.system = Some(crate::system::System::default());
        g.system_input(&key('P'));
        g.edit_settings.as_mut().unwrap().names[0] = "Discard".into();
        g.system_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        assert_eq!(g.tanks[0].name, name);
        g.round = 3;
        g.tanks[0].health = 20;
        g.system_input(&key('N'));
        assert_eq!(g.round, 3);
        g.system_input(&key('Y'));
        assert_eq!(g.round, 1);
        assert!(g.system.is_none());
        assert_eq!(g.tanks[0].health, 100);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), 10);
    }

    #[test]
    fn batch_seventeen_previews() {
        let directory = std::env::var("SCORCHED_PLAY_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .as_chunks::<{ (W * 4) as usize }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        let mut g = Game::new();
        g.settings.players = 4;
        g.settings.controllers.fill(Ctrl::Human);
        g.settings.play.teams = TeamMode::Corporate;
        for (i, name) in ["Alice", "Bob", "Carol", "Dave"].iter().enumerate() {
            g.settings.names[i] = name.to_string();
        }
        g.start_game();
        g.terrain = Terrain::flat(300);
        for (i, tank) in g.tanks.iter_mut().enumerate() {
            tank.x = 80 + i as i32 * 160;
            tank.y = 299;
        }
        g.system = Some(crate::system::System::default());
        g.draw(&mut fb);
        save(&fb, "system-menu");
        let key = |c| ui::MenuInput {
            hotkey: Some(c),
            ..Default::default()
        };
        g.system_input(&key('M'));
        g.draw(&mut fb);
        save(&fb, "mass-kill-confirm");
        g.system_input(&key('N'));
        g.system_input(&key('T'));
        g.draw(&mut fb);
        save(&fb, "reassign-teams");
        g.system_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        g.system_input(&key('P'));
        g.draw(&mut fb);
        save(&fb, "reassign-players");
        g.system = None;
        g.phase = Phase::Menu;
        g.menu.page = Some(ui::Page::Players);
        g.menu.handle(
            &mut g.settings,
            &ui::MenuInput {
                activate: true,
                ..Default::default()
            },
        );
        g.draw(&mut fb);
        save(&fb, "player-editor");
        g.menu = MenuState {
            page: Some(ui::Page::Play),
            ..Default::default()
        };
        g.draw(&mut fb);
        save(&fb, "play-options");
        g.phase = Phase::Aim;
        g.tanks[1].alive = false;
        g.tanks[3].alive = false;
        g.finish_round(&[0, 2]);
        g.draw(&mut fb);
        save(&fb, "corporate-standings");
        g.settings.play.teams = TeamMode::Vicious;
        g.start_game();
        g.terrain = Terrain::flat(300);
        for (i, tank) in g.tanks.iter_mut().enumerate() {
            tank.x = 80 + i as i32 * 160;
            tank.y = 299;
        }
        g.tanks[1].alive = false;
        g.tanks[3].alive = false;
        g.set_phase(Phase::Resolve);
        g.update_resolve(0.02);
        assert!(g.vicious_split);
        g.draw(&mut fb);
        save(&fb, "vicious-breakup");
        g.settings.play.talking_tanks = crate::settings::TalkingTanks::All;
        g.speak(g.current, false);
        g.draw(&mut fb);
        save(&fb, "talking-tank");
    }
    #[test]
    fn system_mass_kill_confirms_splits_remaining_points_and_pays_once() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[1].health = 50;
        g.tanks[2].alive = false;
        g.tanks[2].health = 0;
        g.tanks[0].equipment.active_shield = crate::shields::ActiveShield::new(ItemId::Shield);
        g.fire();
        let mut liquid = projectile(ItemId::Napalm, PKind::Shell);
        liquid.tunneling = false;
        assert!(g.liquid.impact(&liquid, vec2(300.0, 299.0), HUD_H));
        g.system = Some(crate::system::System::default());
        let key = |c| ui::MenuInput {
            hotkey: Some(c),
            ..Default::default()
        };
        g.system_input(&key('M'));
        assert!(g.phase == Phase::Resolve);
        assert_eq!(g.projs.len(), 1);
        g.system_input(&key('N'));
        assert!(g.tanks[0].alive);
        g.system_input(&key('M'));
        g.system_input(&key('Y'));
        assert!(g.phase == Phase::RoundOver);
        assert!(g.system.is_none());
        assert!(g.projs.is_empty() && !g.liquid.active());
        assert!(g
            .tanks
            .iter()
            .all(|t| !t.alive && t.equipment.active_shield.is_none()));
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(5350));
        assert_eq!(g.tanks[1].account.cash, economy::Money::from(5350));
        assert!(g.tanks[2].account.cash.is_zero());
        assert!(g.tanks.iter().all(|t| t.wins == 0 && t.kills == 0));
        g.mass_kill();
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(5350));
    }
    #[test]
    fn system_pause_preserves_shot_timers_and_clear_sound_and_quit_are_explicit() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.fire();
        g.trail.push((5, 50, Rgb::WHITE));
        let pos = g.projs[0].pos;
        let age = g.projs[0].age;
        let phase_t = g.phase_t;
        let wind = g.wind;
        g.system = Some(crate::system::System::default());
        for _ in 0..10 {
            assert!(g.system_input(&ui::MenuInput::default()));
        }
        assert_eq!(g.projs[0].pos, pos);
        assert_eq!(g.projs[0].age, age);
        assert_eq!(g.phase_t, phase_t);
        assert_eq!(g.wind, wind);
        assert!(!g.take_sound_frame().enabled);
        g.system_input(&ui::MenuInput {
            hotkey: Some('C'),
            ..Default::default()
        });
        assert!(g.trail.is_empty());
        let sound = g.settings.sound.enabled;
        g.system_input(&ui::MenuInput {
            hotkey: Some('S'),
            ..Default::default()
        });
        assert_ne!(g.settings.sound.enabled, sound);
        g.system_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        assert!(g.system.is_none());
        assert_eq!(g.projs[0].pos, pos);
        g.system = Some(crate::system::System::default());
        g.system_input(&ui::MenuInput {
            hotkey: Some('Q'),
            ..Default::default()
        });
        assert!(!g.system_input(&ui::MenuInput {
            hotkey: Some('Y'),
            ..Default::default()
        }));
    }
    #[test]
    fn reassigning_a_waiting_human_to_ai_finishes_its_guidance_prompt() {
        let mut g = guided_game(ItemId::LazyBoy);
        g.fire();
        assert!(g.phase == Phase::Target);
        g.system = Some(crate::system::System::default());
        g.system_input(&ui::MenuInput {
            hotkey: Some('P'),
            ..Default::default()
        });
        g.edit_settings.as_mut().unwrap().controllers[0] = Ctrl::Ai(ai::AiKind::Shooter);
        g.system_input(&ui::MenuInput {
            hotkey: Some('D'),
            ..Default::default()
        });
        assert!(g.phase == Phase::Resolve);
        assert_eq!(g.projs.len(), 1);
        assert_eq!(g.tanks[0].inventory.count(ItemId::LazyBoy), 1);
        assert!(g.system.is_some());
        assert_ne!(g.tanks[0].name, "Player 1");
    }
    #[test]
    fn fast_computers_only_accelerates_computer_battles_and_speech_respects_options() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.settings.play.fast_computers = true;
        assert_eq!(g.fast_scale(), 1.0);
        for tank in &mut g.tanks {
            tank.ctrl = Ctrl::Ai(ai::AiKind::Shooter);
        }
        assert_eq!(g.fast_scale(), 6.0);
        g.phase = Phase::Shop;
        assert_eq!(g.fast_scale(), 1.0);
        g.phase = Phase::Aim;
        g.speak(0, false);
        assert!(g.bubbles.is_empty());
        g.settings.play.talking_tanks = crate::settings::TalkingTanks::Computers;
        g.settings.play.talk_probability = 100;
        g.speak(0, false);
        assert_eq!(g.bubbles.len(), 1);
        g.tanks[1].ctrl = Ctrl::Human;
        g.speak(1, true);
        assert_eq!(g.bubbles.len(), 1);
        g.settings.play.talk_probability = 0;
        g.speak(0, true);
        assert_eq!(g.bubbles.len(), 1);
    }
    #[test]
    fn vicious_breakup_is_live_and_original_assignments_return_next_round() {
        let mut g = Game::new();
        g.settings.players = 4;
        g.settings.play.teams = TeamMode::Vicious;
        g.start_game();
        g.tanks[1].alive = false;
        g.tanks[3].alive = false;
        g.terrain = Terrain::flat(300);
        for tank in &mut g.tanks {
            tank.y = 299;
        }
        g.set_phase(Phase::Resolve);
        g.update_resolve(0.02);
        assert!(g.vicious_split);
        assert!(g.phase == Phase::Aim);
        assert_eq!(economy::relation(&g.tanks, 0, 2), Relation::Enemy);
        g.start_round();
        assert!(!g.vicious_split);
        assert_eq!(g.tanks[0].team, Some(0));
        assert_eq!(g.tanks[2].team, Some(0));
    }

    #[test]
    fn all_ai_personalities_take_a_live_turn_without_opening_human_prompts() {
        for kind in ai::AiKind::KNOWN.into_iter().chain([ai::AiKind::Unknown]) {
            let mut g = energy_game(ItemId::BabyMissile, 0);
            g.tanks[0].inventory = Inventory::default();
            g.tanks[0].ctrl = Ctrl::Ai(kind);
            g.tanks[0].ai_state = ai::State::new(g.tanks[0].ctrl);
            g.wind = 0;
            g.begin_turn();
            for _ in 0..300 {
                g.phase_t += 0.02;
                g.update_aim(0.02);
                if g.phase != Phase::Aim {
                    break;
                }
            }
            assert!(g.phase == Phase::Resolve, "{} did not fire", kind.label());
            assert!(!g.projs.is_empty());
            assert!(g.tanks[0].ai_state.active_target.is_some());
            for _ in 0..1500 {
                if g.phase != Phase::Resolve {
                    break;
                }
                g.update_resolve(0.02);
            }
            assert!(g.phase == Phase::Aim, "{} did not finish", kind.label());
            assert_ne!(g.current, 0);
        }
    }

    #[test]
    fn ai_uses_owned_supplies_and_riot_to_escape_even_when_buying_is_off() {
        let mut g = energy_game(ItemId::BabyMissile, 0);
        g.settings.economics.computers_buy = false;
        g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Shooter);
        g.tanks[0].health = 50;
        for (id, count) in [
            (ItemId::Battery, 5),
            (ItemId::SuperMag, 1),
            (ItemId::Parachute, 2),
            (ItemId::RiotCharge, 1),
        ] {
            g.tanks[0].inventory.add(id, count);
        }
        for x in 90..=110 {
            for y in 275..300 {
                g.terrain.add_dirt(x, y, 0, HUD_H);
            }
        }
        g.begin_turn();
        assert!(g.prepare_ai_turn());
        assert_eq!(g.tanks[0].health, 100);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 0);
        assert_eq!(g.tanks[0].shield_percent(), 100);
        assert_eq!(
            g.tanks[0].equipment.active_shield.as_ref().unwrap().item,
            ItemId::SuperMag
        );
        let plan = ai::plan(
            ai::AiKind::Shooter,
            &g.terrain,
            &g.tanks,
            0,
            &g.physics(),
            true,
            1.0,
        );
        assert_eq!(plan.weapon, ItemId::RiotCharge);
        g.tanks[0].angle = plan.angle;
        g.tanks[0].power = plan.power;
        g.tanks[0].weapon = plan.weapon;
        g.ai_plan = Some(plan);
        g.fire();
        assert_eq!(g.tanks[0].inventory.count(ItemId::RiotCharge), 0);
        let muzzle = g.tanks[0].muzzle(90).0;
        assert!(!g.terrain.solid(muzzle.x as i32, muzzle.y as i32));
    }

    #[test]
    fn live_ai_shots_update_tosser_memory_and_damage_updates_cyborg_grudges() {
        let mut g = energy_game(ItemId::BabyMissile, 0);
        g.tanks[0].inventory = Inventory::default();
        g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Tosser);
        let plan = ai::Plan {
            angle: 60,
            power: 300,
            weapon: ItemId::BabyMissile,
            target: Some(1),
        };
        g.tanks[0].angle = plan.angle;
        g.tanks[0].power = plan.power;
        g.ai_plan = Some(plan);
        g.fire();
        for _ in 0..500 {
            if g.projs.is_empty() {
                break;
            }
            g.step_projs(0.02);
        }
        assert!(g.tanks[0].ai_state.shots[1].unwrap().landing.is_some());
        assert_eq!(g.tanks[0].ai_state.shots[1].unwrap().power, 300);
        economy::award_damage(
            &mut g.tanks,
            g.settings.economics.scoring,
            DamageEvent {
                owner: 1,
                victim: 0,
                amount: 30,
                target: DamageTarget::Tank,
            },
        );
        assert_eq!(g.tanks[0].ai_state.grudges[1], 30);
        let hidden = g.tanks[0].ai_state.resolved;
        g.start_round();
        assert!(g.tanks[0].ai_state.shots.iter().all(Option::is_none));
        assert_eq!(g.tanks[0].ai_state.grudges[1], 30);
        assert_eq!(g.tanks[0].ai_state.resolved, hidden);
    }

    #[test]
    fn ai_guidance_targets_the_planned_victim_and_unknown_stays_secret() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Unknown);
        g.tanks[0].ai_state = ai::State::new(g.tanks[0].ctrl);
        g.tanks[0].inventory.add(ItemId::LazyBoy, 1);
        g.tanks[0].equipment.guidance = Some(ItemId::LazyBoy);
        g.ai_plan = Some(ai::Plan {
            angle: 60,
            power: 300,
            weapon: ItemId::Missile,
            target: Some(2),
        });
        g.fire();
        assert!(g.phase == Phase::Resolve);
        assert_eq!(g.projs[0].guidance.unwrap().target, vec2(500.0, 299.0));
        assert_eq!(g.tanks[0].inventory.count(ItemId::LazyBoy), 0);
        assert_eq!(g.tanks[0].ctrl.label(), "Unknown");
    }

    #[test]
    fn ai_driving_is_bounded_and_slips_resume_without_another_supply_pass() {
        for slip in [false, true] {
            let mut g = energy_game(ItemId::BabyMissile, 0);
            g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Shooter);
            g.tanks[0].inventory.add(ItemId::FuelTank, 1);
            if slip {
                g.terrain = Terrain::flat(310);
                g.tanks[1].y = 309;
                g.tanks[2].y = 309;
            }
            for y in 200..310 {
                g.terrain.add_dirt(200, y, 0, HUD_H);
            }
            g.begin_turn();
            let ready = g.prepare_ai_turn();
            assert_eq!(ready, !slip);
            if slip {
                assert!(g.phase == Phase::MoveFall);
                for _ in 0..100 {
                    if g.phase != Phase::MoveFall {
                        break;
                    }
                    g.update_move_fall(0.02);
                }
                assert!(g.phase == Phase::Aim);
                assert_eq!(g.tanks[0].health, 80);
                assert_eq!(g.tanks[0].fuel_available(), 9);
            } else {
                assert_eq!(g.tanks[0].x, 108);
                assert_eq!(g.tanks[0].fuel_available(), 2);
            }
            let fuel = g.tanks[0].fuel_available();
            assert!(g.prepare_ai_turn());
            assert_eq!(g.tanks[0].fuel_available(), fuel);
        }
    }

    #[test]
    fn ai_previews() {
        let directory = std::env::var("SCORCHED_AI_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .as_chunks::<{ (W * 4) as usize }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for kind in ai::AiKind::KNOWN.into_iter().chain([ai::AiKind::Unknown]) {
            let mut g = energy_game(ItemId::BabyMissile, 0);
            g.terrain = Terrain::flat(400);
            g.tanks[0].y = 399;
            g.tanks[1].y = 399;
            g.tanks[1].x = 600;
            g.tanks[2].alive = false;
            g.tanks[0].inventory = Inventory::default();
            g.tanks[0].ctrl = Ctrl::Ai(kind);
            g.tanks[0].name = kind.label().into();
            g.tanks[0].ai_state = ai::State::new(g.tanks[0].ctrl);
            g.wind = 0;
            g.settings.weapons.tunneling = false;
            g.settings.weapons.trace_paths = true;
            if kind == ai::AiKind::Poolshark {
                g.settings.physics.walls = WallType::Rubber;
            }
            if matches!(
                kind,
                ai::AiKind::Shooter | ai::AiKind::Spoiler | ai::AiKind::Poolshark
            ) {
                for x in 300..310 {
                    for y in 150..400 {
                        g.terrain.add_dirt(x, y, 0, HUD_H);
                    }
                }
            }
            let plan = ai::plan(kind, &g.terrain, &g.tanks, 0, &g.physics(), false, 1.0);
            g.tanks[0].angle = plan.angle;
            g.tanks[0].power = plan.power;
            g.ai_plan = Some(plan);
            g.fire();
            for _ in 0..7200 {
                if g.projs.is_empty() {
                    break;
                }
                g.step_projs(SIM_DT);
            }
            g.draw(&mut fb);
            save(&fb, kind.label());
        }
        let mut g = Game::new();
        g.settings.players = 9;
        g.settings.controllers[..9].copy_from_slice(&crate::tank::CTRLS);
        g.menu.page = Some(ui::Page::Players);
        g.draw(&mut fb);
        save(&fb, "controller-menu");
    }

    fn liquid_game(weapon: ItemId) -> Game {
        let mut g = energy_game(weapon, 0);
        g.terrain = Terrain::flat(300);
        for x in 300..340 {
            for y in 300..330 {
                g.terrain.carve(x, y, 0);
            }
        }
        g.tanks[1].x = 320;
        g.tanks[1].y = 329;
        g
    }

    #[test]
    fn all_liquids_can_be_bought_selected_fired_and_resolve_without_contact_kills() {
        for weapon in [ItemId::Napalm, ItemId::HotNapalm, ItemId::LiquidDirt] {
            let mut g = liquid_game(weapon);
            g.settings.weapons.arms_level = 4;
            g.tanks[0].account.cash = economy::Money::from(100_000);
            assert!(shop::buy(
                &mut g.tanks[0],
                weapon,
                &g.prices,
                &g.settings,
                &mut g.market
            ));
            assert_eq!(g.tanks[0].use_item(weapon), crate::tank::ItemUse::Applied);
            g.tanks[0].inventory.add(ItemId::ContactTrigger, 1);
            g.tanks[0].equipment.triggers = true;
            let stock = g.tanks[0].inventory.count(weapon);
            let cash = g.tanks[0].account.cash.clone();
            g.fire();
            assert_eq!(g.tanks[0].inventory.count(weapon), stock - 1);
            assert_eq!(g.tanks[0].inventory.count(ItemId::ContactTrigger), 0);
            g.projs[0].pos = vec2(320.0, 318.0);
            g.projs[0].vel = vec2(0.0, 1000.0);
            g.step_projs(0.02);
            assert!(g.projs.is_empty());
            assert!(g.expls.is_empty());
            assert!(g.liquid.active());
            assert_eq!(g.tanks[1].health, 100);
            assert!(g.take_sound_frame().cues.contains(&Cue::Splash));
            g.update_resolve(0.02);
            assert!(g.phase == Phase::Resolve);
            for _ in 0..2000 {
                if g.phase != Phase::Resolve {
                    break;
                }
                g.update_resolve(0.02);
            }
            assert!(!g.liquid.active());
            assert!(g.phase == Phase::Aim);
            if weapon == ItemId::LiquidDirt {
                assert_eq!(g.tanks[1].health, 100);
                assert_eq!(g.tanks[1].y, 329);
                assert_eq!(g.tanks[0].account.cash, cash);
                assert!(g.terrain.solid(320, 320));
            } else {
                assert!(!g.tanks[1].alive);
                assert_eq!(g.tanks[0].kills, 1);
                let mut expected = cash;
                expected.add(&economy::Money::from(3500));
                assert_eq!(g.tanks[0].account.cash, expected);
                assert_eq!(g.current, 2);
            }
        }
    }

    #[test]
    fn contact_triggers_and_nonballistic_guidance_prevent_live_napalm_fizzles() {
        for mode in 0..3 {
            let mut g = liquid_game(ItemId::Napalm);
            g.terrain = Terrain::flat(300);
            g.tanks[1].x = 500;
            g.tanks[1].y = 299;
            g.tanks[2].x = 600;
            if mode == 1 {
                g.tanks[0].inventory.add(ItemId::ContactTrigger, 1);
                g.tanks[0].equipment.triggers = true;
            } else if mode == 2 {
                g.tanks[0].inventory.add(ItemId::HeatGuidance, 1);
                g.tanks[0].equipment.guidance = Some(ItemId::HeatGuidance);
            }
            g.fire();
            g.projs[0].pos = vec2(320.5, 299.5);
            g.projs[0].vel = vec2(0.0, 1000.0);
            for _ in 0..300 {
                if g.projs.is_empty() {
                    break;
                }
                g.step_projs(SIM_DT);
            }
            assert!(g.projs.is_empty());
            assert_eq!(g.liquid.active(), mode != 0);
            assert!(g.expls.is_empty());
            assert_eq!(g.terrain.solid(320, 300), mode != 0);
        }
    }

    #[test]
    fn shields_intercept_liquid_carriers_without_starting_a_pool() {
        let mut g = liquid_game(ItemId::Napalm);
        g.tanks[1].equipment.active_shield = crate::shields::ActiveShield::new(ItemId::Shield);
        g.tanks[0].inventory.add(ItemId::ContactTrigger, 1);
        g.tanks[0].equipment.triggers = true;
        g.fire();
        g.projs[0].pos = vec2(320.0, 310.0);
        g.projs[0].vel = vec2(0.0, 300.0);
        g.step_projs(0.01);
        assert!(g.projs.is_empty());
        assert!(!g.liquid.active());
        assert_eq!(g.tanks[1].shield_percent(), 90);
        assert_eq!(g.tanks[1].health, 100);
        assert!(g.take_sound_frame().cues.contains(&Cue::ShieldHit));
    }

    #[test]
    fn own_napalm_heat_uses_self_damage_and_suicide_awards_once() {
        let mut g = liquid_game(ItemId::Napalm);
        g.tanks[0].x = 320;
        g.tanks[0].y = 329;
        g.tanks[1].x = 500;
        g.tanks[1].y = 299;
        g.tanks[2].x = 600;
        g.tanks[0].account.cash = economy::Money::from(10_000);
        g.tanks[0].equipment.active_shield = crate::shields::ActiveShield::new(ItemId::SuperMag);
        g.action_owner = Some(0);
        g.settle_dirt = true;
        let mut p = projectile(ItemId::Napalm, PKind::Shell);
        p.pos = vec2(320.0, 329.0);
        p.tunneling = false;
        let mut next = Vec::new();
        g.impact_projectile(p, p.pos, Some(0), &mut next);
        g.set_phase(Phase::Resolve);
        for _ in 0..2000 {
            if g.phase != Phase::Resolve {
                break;
            }
            g.update_resolve(0.02);
        }
        assert!(!g.tanks[0].alive);
        assert_eq!(g.tanks[0].shield_percent(), 100);
        assert_eq!(g.tanks[0].account.cash, economy::Money::from(7000));
        assert_eq!(g.tanks[0].kills, -1);
        assert!(g.phase == Phase::Aim);
    }

    #[test]
    fn liquid_previews() {
        let directory = std::env::var("SCORCHED_LIQUID_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .as_chunks::<{ (W * 4) as usize }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for weapon in [ItemId::Napalm, ItemId::HotNapalm, ItemId::LiquidDirt] {
            let mut g = liquid_game(weapon);
            let mut p = projectile(weapon, PKind::Shell);
            p.pos = vec2(320.0, 300.0);
            p.tunneling = false;
            g.impact_projectile(p, p.pos, None, &mut Vec::new());
            g.set_phase(Phase::Resolve);
            for (time, name) in [(0.3, "splash"), (1.2, "pool"), (2.0, "burn")] {
                let result =
                    g.liquid
                        .update(time, &mut g.terrain, &mut g.tanks, HUD_H, WallType::None);
                assert!(result.damage.iter().all(|event| event.owner == 0));
                g.draw(&mut fb);
                save(&fb, &format!("{}-{name}", weapon.item().name));
            }
        }
    }

    #[test]
    fn movement_preserves_turn_and_shot_and_uses_one_fuel_unit_per_pixel() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[0].inventory.add(ItemId::FuelTank, 1);
        let weapon_stock = g.tanks[0].inventory.count(ItemId::Missile);
        let wall = g.active_walls;
        g.handle_control_action(ControlAction::Move);
        assert!(g.phase == Phase::Move);
        g.move_input(&ui::MenuInput {
            right: true,
            ..Default::default()
        });
        assert_eq!(g.tanks[0].x, 101);
        assert_eq!(g.tanks[0].fuel_available(), 9);
        g.move_input(&ui::MenuInput {
            activate: true,
            ..Default::default()
        });
        assert!(g.projs.is_empty());
        g.move_input(&ui::MenuInput {
            escape: true,
            ..Default::default()
        });
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 0);
        assert_eq!(g.active_walls, wall);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), weapon_stock);
        g.fire();
        assert_eq!(g.projs.len(), 1);
        assert!(g.projs[0].pos.x >= 111.0);
    }

    #[test]
    fn movement_slips_use_normal_fall_damage_and_parachutes_then_return_to_aiming() {
        for parachute in [false, true] {
            let mut g = energy_game(ItemId::Missile, 0);
            g.terrain = Terrain::flat(310);
            g.tanks[1].y = 309;
            g.tanks[2].y = 309;
            g.tanks[0].inventory.add(ItemId::FuelTank, 1);
            if parachute {
                g.tanks[0].inventory.add(ItemId::Parachute, 1);
            }
            g.begin_move();
            g.move_input(&ui::MenuInput {
                right: true,
                ..Default::default()
            });
            assert!(g.phase == Phase::MoveFall);
            for _ in 0..100 {
                if g.phase != Phase::MoveFall {
                    break;
                }
                g.update_move_fall(0.02);
            }
            assert!(g.phase == Phase::Aim);
            assert_eq!(g.current, 0);
            assert_eq!(g.tanks[0].y, 309);
            assert_eq!(g.tanks[0].health, if parachute { 100 } else { 80 });
            assert_eq!(g.tanks[0].fuel_available(), 9);
            assert_eq!(g.tanks[0].inventory.count(ItemId::Parachute), 0);
        }
    }

    #[test]
    fn movement_lethal_slips_resolve_death_and_exhausted_fuel_returns_to_aiming() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.terrain = Terrain::flat(370);
        g.tanks[1].y = 369;
        g.tanks[2].y = 369;
        g.tanks[0].equipment.fuel = 1;
        g.begin_move();
        g.move_input(&ui::MenuInput {
            right: true,
            ..Default::default()
        });
        for _ in 0..1000 {
            if g.phase == Phase::MoveFall {
                g.update_move_fall(0.02);
            } else if g.phase == Phase::Resolve {
                g.update_resolve(0.02);
            } else {
                break;
            }
        }
        assert!(!g.tanks[0].alive);
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 1);
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[0].equipment.fuel = 1;
        g.begin_move();
        g.move_input(&ui::MenuInput {
            left: true,
            ..Default::default()
        });
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].fuel_available(), 0);
        assert_eq!(g.current, 0);
    }

    #[test]
    fn surviving_driver_keeps_its_turn_after_a_landing_kills_another_tank() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.terrain = Terrain::flat(330);
        g.tanks[1].x = 101;
        g.tanks[1].y = 329;
        g.tanks[1].health = 30;
        g.tanks[2].y = 329;
        g.tanks[0].equipment.fuel = 3;
        // Protect the driver from the dead tank's disappearing support and crater.
        g.tanks[0].inventory.add(ItemId::Parachute, 3);
        g.begin_move();
        g.move_input(&ui::MenuInput {
            right: true,
            ..Default::default()
        });
        for _ in 0..1000 {
            if g.phase == Phase::MoveFall {
                g.update_move_fall(0.02);
            } else if g.phase == Phase::Resolve {
                g.update_resolve(0.02);
            } else {
                break;
            }
        }
        assert!(g.tanks[0].alive && g.tanks[0].health > 0);
        assert!(!g.tanks[1].alive);
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 0);
    }

    #[test]
    fn fuel_and_shapes_carry_between_rounds_and_new_games_reset_fuel() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.settings.shapes[0] = crate::tank::Shape::Wheels;
        g.tanks[0].shape = crate::tank::Shape::Wheels;
        g.tanks[0].inventory.add(ItemId::FuelTank, 2);
        assert!(g.tanks[0].spend_fuel(3));
        g.start_round();
        assert_eq!(g.tanks[0].fuel_available(), 17);
        assert_eq!(g.tanks[0].shape, crate::tank::Shape::Wheels);
        g.start_game();
        assert_eq!(g.tanks[0].fuel_available(), 0);
        assert_eq!(g.tanks[0].shape, crate::tank::Shape::Wheels);
    }

    #[test]
    fn triple_turret_fires_one_three_projectile_volley_for_small_missiles_only() {
        for weapon in [
            ItemId::BabyMissile,
            ItemId::Missile,
            ItemId::BabyNuke,
            ItemId::Mirv,
        ] {
            let mut g = energy_game(weapon, 0);
            g.tanks[0].shape = crate::tank::Shape::Triple;
            g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Moron);
            g.tanks[0].angle = 60;
            g.tanks[0].inventory.add(ItemId::ContactTrigger, 3);
            g.tanks[0].equipment.triggers = true;
            let before = g.tanks[0].inventory.count(weapon);
            g.fire();
            let triple = matches!(weapon, ItemId::BabyMissile | ItemId::Missile);
            assert_eq!(g.projs.len(), if triple { 3 } else { 1 });
            assert_eq!(
                g.tanks[0].inventory.count(weapon),
                if weapon == ItemId::BabyMissile {
                    99
                } else {
                    before - 1
                }
            );
            assert_eq!(g.tanks[0].inventory.count(ItemId::ContactTrigger), 2);
            assert!(g.projs.iter().all(|p| !p.tunneling));
            if triple {
                assert!(g.projs[0].vel != g.projs[1].vel && g.projs[1].vel != g.projs[2].vel);
            }
        }
    }

    #[test]
    fn movement_and_shape_previews() {
        let directory = std::env::var("SCORCHED_MOVEMENT_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .as_chunks::<{ (W * 4) as usize }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        fb.rect(0, 0, W, H, Rgb(0, 0, 45));
        fb.text(32, 140, "TANK SHAPES", Rgb::WHITE);
        for (i, shape) in crate::tank::Shape::ALL.into_iter().enumerate() {
            let x = 50 + i as i32 * 86;
            ui::draw_tank_shape(&mut fb, x, 220, 55, PLAYER_COLORS[i], shape);
            fb.text(x - 24, 240, shape.label(), Rgb::WHITE);
            fb.text(
                x - 24,
                262,
                if shape == crate::tank::Shape::Triple {
                    "AI"
                } else if shape.mobile() {
                    "mobile"
                } else {
                    "fixed"
                },
                Rgb::WHITE,
            );
        }
        save(&fb, "tank-shapes");
        let mut g = energy_game(ItemId::Missile, 0);
        g.phase = Phase::Menu;
        g.settings.players = 10;
        for i in 0..10 {
            g.settings.shapes[i] = crate::tank::Shape::ALL[i % 7];
        }
        g.menu.page = Some(ui::Page::Players);
        g.draw(&mut fb);
        save(&fb, "player-shapes");
        let mut g = energy_game(ItemId::Missile, 0);
        g.settings.play.status_bar = true;
        g.tanks[0].inventory.add(ItemId::FuelTank, 2);
        g.begin_move();
        for _ in 0..5 {
            g.move_input(&ui::MenuInput {
                right: true,
                ..Default::default()
            });
        }
        g.draw(&mut fb);
        save(&fb, "move-panel");
        g.end_move();
        g.tanks[0].shape = crate::tank::Shape::Triple;
        g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Moron);
        g.tanks[0].angle = 65;
        g.tanks[0].power = 450;
        g.settings.weapons.trace_paths = true;
        g.fire();
        assert_eq!(g.projs.len(), 3);
        for _ in 0..120 {
            g.step_projs(SIM_DT);
        }
        g.draw(&mut fb);
        save(&fb, "triple-volley");
    }

    #[test]
    fn guidance_target_prompt_cancels_without_spending_and_launch_resets_selection() {
        for item in crate::tank::GUIDANCE {
            let mut g = guided_game(item);
            let before = g.tanks[0].inventory.clone();
            g.fire();
            if item == ItemId::HeatGuidance {
                assert!(g.phase == Phase::Resolve);
            } else {
                assert!(g.phase == Phase::Target);
                assert_eq!(g.tanks[0].inventory, before);
                g.target_input(&ui::MenuInput {
                    escape: true,
                    ..Default::default()
                });
                assert!(g.phase == Phase::Aim);
                assert_eq!(g.tanks[0].inventory, before);
                assert_eq!(g.tanks[0].equipment.guidance, Some(item));
                g.fire();
                g.target_input(&ui::MenuInput {
                    hotkey: Some('9'),
                    ..Default::default()
                });
                assert!(g.phase == Phase::Target); // Missing target doesn't fire.
                g.target_input(&ui::MenuInput {
                    hotkey: Some('2'),
                    ..Default::default()
                });
                assert!(g.phase == Phase::Resolve);
            }
            assert_eq!(g.tanks[0].inventory.count(item), 1);
            assert_eq!(g.tanks[0].equipment.guidance, None);
            assert_eq!(g.projs.len(), 1);
            assert_eq!(g.projs[0].tunneling, item == ItemId::BallisticGuidance);
            if item != ItemId::BallisticGuidance {
                assert_eq!(g.projs[0].guidance.unwrap().item, item);
            } else {
                assert!(g.projs[0].guidance.is_none());
            }
        }
    }

    #[test]
    fn target_clicks_select_arbitrary_points_or_nearby_tanks_and_tenth_key_works() {
        for nearest in [false, true] {
            let mut g = guided_game(ItemId::LazyBoy);
            g.fire();
            g.target_input(&ui::MenuInput {
                click: Some((-10, 100, 1)),
                ..Default::default()
            });
            assert!(g.phase == Phase::Target);
            g.target_input(&ui::MenuInput {
                click: Some((310, 285, if nearest { -1 } else { 1 })),
                ..Default::default()
            });
            assert!(g.phase == Phase::Resolve);
            assert_eq!(
                g.projs[0].guidance.unwrap().target,
                if nearest {
                    vec2(300.0, 299.0)
                } else {
                    vec2(310.0, 285.0)
                }
            );
        }
        let mut g = guided_game(ItemId::VerticalGuidance);
        let mut tank = g.tanks[2].clone();
        for i in 0..7 {
            tank.x = 510 + i * 15;
            g.tanks.push(tank.clone());
        }
        g.fire();
        g.target_input(&ui::MenuInput {
            hotkey: Some('0'),
            ..Default::default()
        });
        assert_eq!(g.projs[0].guidance.unwrap().target, vec2(600.0, 299.0));
    }

    #[test]
    fn unsupported_weapons_leave_guidance_owned_and_selected() {
        for weapon in [
            ItemId::Mirv,
            ItemId::DeathsHead,
            ItemId::RiotCharge,
            ItemId::RiotBlast,
            ItemId::EarthDisrupter,
            ItemId::PlasmaBlast,
            ItemId::Laser,
        ] {
            let mut g = guided_game(ItemId::LazyBoy);
            g.tanks[0].inventory.add(weapon, 1);
            g.tanks[0].weapon = weapon;
            g.fire();
            if crate::energy::is_energy(weapon) {
                g.battery_key(KeyCode::Enter);
            }
            assert!(g.phase == Phase::Resolve);
            assert_eq!(g.tanks[0].inventory.count(ItemId::LazyBoy), 2);
            assert_eq!(g.tanks[0].equipment.guidance, Some(ItemId::LazyBoy));
            assert!(g.projs.iter().all(|p| p.guidance.is_none()));
        }
    }

    #[test]
    fn contact_trigger_is_spent_once_for_all_mirv_warheads_and_normal_tunneling_returns() {
        for weapon in [ItemId::Mirv, ItemId::DeathsHead] {
            let mut g = energy_game(weapon, 0);
            g.tanks[0].inventory.add(ItemId::ContactTrigger, 1);
            g.tanks[0].equipment.triggers = true;
            g.fire();
            assert_eq!(g.tanks[0].inventory.count(ItemId::ContactTrigger), 0);
            assert!(!g.tanks[0].equipment.triggers);
            assert!(!g.projs[0].tunneling);
            let mut children = Vec::new();
            weapons::split_carrier(g.projs[0], &mut children);
            assert_eq!(children.len(), if weapon == ItemId::Mirv { 5 } else { 9 });
            assert!(children
                .iter()
                .all(|p| !p.tunneling && p.guidance.is_none()));
            g.projs.clear();
            g.begin_turn();
            g.fire();
            assert!(g.projs[0].tunneling);
        }
    }

    #[test]
    fn nonballistic_guidance_does_not_waste_an_armed_trigger() {
        for item in [
            ItemId::HeatGuidance,
            ItemId::HorizontalGuidance,
            ItemId::VerticalGuidance,
            ItemId::LazyBoy,
        ] {
            let mut g = guided_game(item);
            g.tanks[0].inventory.add(ItemId::ContactTrigger, 3);
            g.tanks[0].equipment.triggers = true;
            g.fire();
            if crate::guidance::needs_target(item) {
                g.target_input(&ui::MenuInput {
                    click: Some((200, 200, 1)),
                    ..Default::default()
                });
            }
            assert_eq!(g.tanks[0].inventory.count(ItemId::ContactTrigger), 3);
            assert!(!g.projs[0].tunneling);
        }
    }

    #[test]
    fn contact_trigger_stops_at_the_first_dirt_pixel_instead_of_burrowing() {
        let mut g = energy_game(ItemId::Missile, 0);
        g.tanks[0].inventory.add(ItemId::ContactTrigger, 1);
        g.tanks[0].equipment.triggers = true;
        g.fire();
        let mut triggered = g.projs[0];
        triggered.pos = vec2(120.5, 200.5);
        triggered.vel = vec2(1000.0, 0.0);
        let mut normal = triggered;
        normal.tunneling = true;
        let mut dirt = Terrain::flat(200);
        let mut trail = Vec::new();
        assert!(matches!(
            weapons::step_proj(
                &mut triggered,
                SIM_DT,
                &mut dirt,
                &g.tanks,
                &g.physics(),
                &mut trail
            ),
            Step::Hit(_, None)
        ));
        assert!(dirt.solid(120, 200));
        weapons::step_proj(
            &mut normal,
            SIM_DT,
            &mut dirt,
            &g.tanks,
            &g.physics(),
            &mut trail,
        );
        assert!(!dirt.solid(120, 200));
    }

    #[test]
    fn lazy_boy_completes_live_damage_death_and_turn_flow() {
        let mut g = guided_game(ItemId::LazyBoy);
        g.tanks[1].x = 200;
        g.fire();
        g.target_input(&ui::MenuInput {
            hotkey: Some('2'),
            ..Default::default()
        });
        for _ in 0..1000 {
            if g.phase != Phase::Resolve {
                break;
            }
            g.update_resolve(0.02);
        }
        assert!(!g.tanks[1].alive);
        assert_eq!(g.tanks[1].last_hit, Some(0));
        assert_eq!(g.tanks[0].kills, 1);
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 2);
        assert!(g.tanks[0].account.cash >= economy::Money::from(3500));
    }

    #[test]
    fn guidance_previews() {
        let directory = std::env::var("SCORCHED_GUIDANCE_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .as_chunks::<{ (W * 4) as usize }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for item in crate::tank::GUIDANCE {
            let mut g = guided_game(item);
            g.settings.weapons.trace_paths = true;
            g.wind = 0;
            g.terrain = Terrain::flat(350);
            g.tanks[0].y = 349;
            g.tanks[0].angle = 60;
            g.tanks[0].power = 500;
            g.tanks[1].x = 250;
            g.tanks[1].y = 200;
            g.fire();
            if g.phase == Phase::Target {
                g.draw(&mut fb);
                save(&fb, "choose-target");
                g.target_input(&ui::MenuInput {
                    click: Some((250, 200, 1)),
                    ..Default::default()
                });
            }
            for _ in 0..2400 {
                if g.projs.is_empty() {
                    break;
                }
                g.step_projs(SIM_DT);
            }
            assert!(g.projs.is_empty(), "{} did not finish", item.item().name);
            g.draw(&mut fb);
            save(&fb, item.item().name);
        }
    }

    #[test]
    fn energy_prompt_can_cancel_and_accept_ten_without_consuming_guidance() {
        for weapon in [ItemId::PlasmaBlast, ItemId::Laser] {
            let mut g = energy_game(weapon, 15);
            g.tanks[0].inventory.add(ItemId::BallisticGuidance, 1);
            g.tanks[0].equipment.guidance = Some(ItemId::BallisticGuidance);
            g.tanks[0].inventory.add(ItemId::ContactTrigger, 1);
            g.tanks[0].equipment.triggers = true;
            let before = g.tanks[0].inventory.clone();
            g.fire();
            assert!(g.phase == Phase::Batteries);
            assert_eq!(g.tanks[0].inventory, before);
            g.battery_key(KeyCode::Escape);
            assert!(g.phase == Phase::Aim);
            assert_eq!(g.tanks[0].inventory, before);
            g.fire();
            g.battery_key(KeyCode::Key1);
            g.battery_key(KeyCode::Key0);
            assert_eq!(g.battery_input, Some(10));
            g.battery_key(KeyCode::Enter);
            assert!(g.phase == Phase::Resolve);
            assert_eq!(g.tanks[0].inventory.count(weapon), 1);
            assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 5);
            assert_eq!(g.tanks[0].inventory.count(ItemId::BallisticGuidance), 1);
            assert_eq!(g.tanks[0].inventory.count(ItemId::ContactTrigger), 1);
            assert!(g.projs.is_empty());
            assert!(g.expls.is_empty());
            assert!(g.energy.active());
            assert!(g.take_sound_frame().flight.is_none());
            g.update_resolve(0.01);
            assert!(g.phase == Phase::Resolve); // Animation holds the turn.
            for _ in 0..1000 {
                if g.phase != Phase::Resolve {
                    break;
                }
                g.update_resolve(0.02);
            }
            assert!(g.phase == Phase::Aim);
            assert_ne!(g.current, 0);
        }
    }

    #[test]
    fn energy_prompt_clamps_stock_and_supports_zero_batteries() {
        for weapon in [ItemId::PlasmaBlast, ItemId::Laser] {
            for available in [0, 3, 10, 99] {
                let mut g = energy_game(weapon, available);
                g.fire();
                g.battery_key(KeyCode::Key9);
                g.battery_key(KeyCode::Key9);
                assert_eq!(g.battery_input, Some(available.min(10) as u8));
                g.battery_key(KeyCode::Backspace);
                g.battery_key(KeyCode::Key0);
                g.battery_key(KeyCode::Enter);
                assert!(g.phase == Phase::Resolve);
                assert_eq!(
                    u32::from(g.tanks[0].inventory.count(ItemId::Battery)),
                    available
                );
                assert_eq!(g.tanks[0].inventory.count(weapon), 1);
                assert!(g.tanks.iter().all(|t| t.health == 100));
            }
        }
    }

    #[test]
    fn energy_weapons_are_buyable_selectable_and_award_actual_damage() {
        for weapon in [ItemId::PlasmaBlast, ItemId::Laser] {
            let mut g = energy_game(weapon, 10);
            g.settings.weapons.arms_level = 4;
            g.tanks[0].account.cash = economy::Money::from(100_000);
            assert!(shop::buy(
                &mut g.tanks[0],
                weapon,
                &g.prices,
                &g.settings,
                &mut g.market
            ));
            assert_eq!(g.tanks[0].use_item(weapon), crate::tank::ItemUse::Applied);
            g.tanks[1].x = 150;
            g.tanks[1].health = 15;
            let before = g.tanks[0].account.cash.clone();
            g.fire();
            g.battery_key(KeyCode::Enter);
            assert_eq!(g.tanks[1].health, 0);
            assert_eq!(g.tanks[1].last_hit, Some(0));
            let mut expected = before;
            expected.add(&economy::Money::from(450));
            assert_eq!(g.tanks[0].account.cash, expected);
            // Repeated animation updates cannot award the hit a second time.
            g.update_resolve(0.01);
            assert_eq!(g.tanks[0].account.cash, expected);
        }
    }

    #[test]
    fn energy_ai_discharge_never_waits_for_human_input() {
        let mut g = energy_game(ItemId::Laser, 3);
        g.tanks[0].ctrl = Ctrl::Ai(ai::AiKind::Moron);
        g.fire();
        assert!(g.phase == Phase::Resolve);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Battery), 0);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Laser), 1);
    }

    #[test]
    fn energy_previews() {
        let directory = std::env::var("SCORCHED_ENERGY_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .as_chunks::<{ (W * 4) as usize }>()
                    .0
                    .iter()
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for (weapon, name) in [(ItemId::PlasmaBlast, "plasma"), (ItemId::Laser, "laser")] {
            let mut g = energy_game(weapon, 10);
            g.tanks[1].x = 160;
            g.fire();
            g.draw(&mut fb);
            save(&fb, &format!("{name}-prompt"));
            g.battery_key(KeyCode::Enter);
            g.update_resolve(0.15);
            assert!(g.energy.active());
            g.draw(&mut fb);
            save(&fb, name);
        }
    }

    #[test]
    fn lightning_is_visible_harmless_when_disabled_and_neutral_when_lethal() {
        let mut g = game();
        g.terrain = Terrain::flat(300);
        g.current = 0;
        for t in &mut g.tanks {
            t.y = 299;
        }
        g.begin_turn();
        g.settings.landscape.hostile_environment = false;
        let cash: Vec<_> = g.tanks.iter().map(|t| t.account.cash.clone()).collect();
        g.weather = crate::environment::Weather::new(crate::settings::Sky::Stormy, 123, HUD_H);
        let impact = g.weather.strike(g.tanks[1].x, &g.terrain, &g.tanks);
        g.lightning_hit(impact);
        assert!(g.weather.visible());
        assert!(g.tanks.iter().all(|t| t.health == 100));
        g.settings.landscape.hostile_environment = true;
        g.tanks[1].health = 30;
        g.lightning_hit(g.tanks[1].center());
        assert!(g.phase == Phase::Resolve);
        assert!(g.tanks[1].environmental_hit);
        for _ in 0..500 {
            if g.phase != Phase::Resolve {
                break;
            }
            g.update_resolve(0.02);
        }
        assert!(!g.tanks[1].alive);
        assert_eq!(g.current, 0);
        assert!(g.phase == Phase::Aim);
        assert_eq!(
            g.tanks
                .iter()
                .map(|t| t.account.cash.clone())
                .collect::<Vec<_>>(),
            cash
        );
        assert!(g.tanks.iter().all(|t| t.kills == 0));
    }

    #[test]
    fn lightning_drains_shields_and_environment_blasts_keep_neutral_attribution() {
        let mut g = game();
        g.tanks[1].inventory.add(ItemId::HeavyShield, 1);
        g.tanks[1].equipment.shield = Some(ItemId::HeavyShield);
        g.tanks[1].use_item(ItemId::HeavyShield);
        // Activation is shared with the normal accessory path.
        if g.tanks[1].equipment.active_shield.is_none() {
            panic!("shield did not activate");
        }
        let before = g.tanks[1]
            .equipment
            .active_shield
            .as_ref()
            .unwrap()
            .strength;
        g.lightning_hit(g.tanks[1].center());
        assert_eq!(g.tanks[1].health, 100);
        assert_eq!(
            g.tanks[1]
                .equipment
                .active_shield
                .as_ref()
                .unwrap()
                .strength,
            before - 35
        );
        let cash = g.tanks[2].account.cash.clone();
        let pos = g.tanks[2].center();
        g.apply_explosion(Explosion::new(pos, 30.0, Damage::Environment, 1, false));
        assert!(g.tanks[2].environmental_hit);
        assert_eq!(g.tanks[2].last_hit, None);
        assert_eq!(g.tanks[2].account.cash, cash);
    }

    #[test]
    fn configured_rounds_randomize_land_and_place_ten_tanks_below_cavern_roof() {
        let mut g = Game::new();
        g.settings.players = 10;
        g.settings.landscape.sky = crate::settings::Sky::Cavern;
        g.settings.landscape.random_land = true;
        g.settings.landscape.bumpiness = 100;
        g.settings.landscape.slope = 100;
        g.start_game();
        g.start_round();
        assert!(g.settings.landscape.bumpiness < 100 && g.settings.landscape.slope < 100);
        assert_eq!(g.terrain.sky_kind, crate::settings::Sky::Cavern);
        for t in &g.tanks {
            assert!(t.supported(&g.terrain));
            assert!(!g.terrain.solid(t.x, t.y - 10));
            assert!(t.y > HUD_H + 100 && t.y < H);
        }
    }

    #[test]
    fn weather_losing_current_player_advances_without_scoring_a_suicide() {
        let mut g = game();
        g.terrain = Terrain::flat(300);
        for t in &mut g.tanks {
            t.y = 299;
        }
        g.current = 0;
        g.begin_turn();
        g.tanks[0].health = 1;
        let cash = g.tanks[0].account.cash.clone();
        g.lightning_hit(g.tanks[0].center());
        for _ in 0..500 {
            if g.phase != Phase::Resolve {
                break;
            }
            g.update_resolve(0.02);
        }
        assert!(!g.tanks[0].alive);
        assert_eq!(g.current, 1);
        assert_eq!(g.tanks[0].kills, 0);
        assert_eq!(g.tanks[0].account.cash, cash);
    }

    #[test]
    fn round_loads_relative_png_folder_from_config_parent() {
        let folder =
            std::env::temp_dir().join(format!("scorched-round-mountains-{}", std::process::id()));
        std::fs::create_dir_all(folder.join("land")).unwrap();
        std::fs::write(
            folder.join("land/custom.PNG"),
            include_bytes!("../tests/fixtures/mountain-alpha.png"),
        )
        .unwrap();
        let mut g = game();
        g.config_path = Some(folder.join("settings.toml"));
        g.settings.landscape.mountains_folder = Some("land".into());
        g.settings.landscape.scanned_mountains_percent = 100;
        g.settings.landscape.sky = crate::settings::Sky::Plain;
        g.start_round();
        assert!(g.terrain.scanned);
        assert!(g.shop_notice.is_none());
        assert_eq!(g.settings.landscape.mountains_folder, Some("land".into()));
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn landscape_previews() {
        use crate::settings::{LandscapeSettings, Sky};
        let directory = std::env::var("SCORCHED_LANDSCAPE_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for sky in [
            Sky::Plain,
            Sky::Stormy,
            Sky::Stars,
            Sky::Shaded,
            Sky::Sunset,
            Sky::Cavern,
            Sky::Black,
        ] {
            let settings = LandscapeSettings {
                sky,
                ..Settings::default().landscape
            };
            let mut terrain = Terrain::seeded(&settings, 7185921, HUD_H, None);
            let mut g = game();
            for t in &mut g.tanks {
                t.y = terrain.flatten(t.x, 9) - 1;
            }
            terrain.render(&mut fb);
            for t in &g.tanks {
                draw_tank(&mut fb, t);
            }
            if sky == Sky::Stormy {
                let mut weather = crate::environment::Weather::new(sky, 92, HUD_H);
                weather.strike(300, &terrain, &g.tanks);
                weather.draw(&mut fb);
            }
            save(&fb, &format!("sky-{sky:?}"));
        }
        for (bumpiness, slope, flatten_peaks, name) in [
            (0, 20, false, "smooth"),
            (100, 20, false, "bumpy"),
            (20, 0, false, "flat"),
            (20, 100, false, "steep"),
            (100, 100, true, "flattened"),
            (20, 20, false, "default"),
        ] {
            let settings = LandscapeSettings {
                sky: Sky::Shaded,
                bumpiness,
                slope,
                flatten_peaks,
                ..Settings::default().landscape
            };
            Terrain::seeded(&settings, 7185921, HUD_H, None).render(&mut fb);
            save(&fb, name);
        }
        let settings = Settings::default();
        let menu = MenuState {
            page: Some(ui::Page::Landscape),
            ..MenuState::default()
        };
        ui::draw_menu(&mut fb, &settings, &menu);
        save(&fb, "landscape-menu");
        let menu = MenuState {
            folder_edit: Some("my mountains".into()),
            ..menu
        };
        ui::draw_menu(&mut fb, &settings, &menu);
        save(&fb, "mountains-folder");
        for (bytes, name) in [
            (
                include_bytes!("../tests/fixtures/mountain-alpha.png").as_slice(),
                "custom-alpha",
            ),
            (
                include_bytes!("../tests/fixtures/mountain-opaque.png").as_slice(),
                "custom-opaque",
            ),
        ] {
            let mountain = crate::landscape::decode_png(bytes).unwrap();
            Terrain::seeded(&settings.landscape, 7185921, HUD_H, Some(&mountain)).render(&mut fb);
            save(&fb, name);
        }
    }

    fn projectile(weapon: ItemId, kind: PKind) -> Proj {
        Proj {
            pos: vec2(100.0, 80.0),
            vel: vec2(0.0, -1.0),
            owner: 0,
            weapon,
            kind,
            radius: weapon.item().radius,
            scale: 1.0,
            damage: weapon.item().damage(),
            age: 1.0,
            tunneling: false,
            in_dirt: false,
            guidance: None,
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
        assert_eq!(
            aim_repeat_steps(&mut elapsed, true, true, 0.016, false, true),
            1
        );
        for _ in 0..100 {
            assert_eq!(
                aim_repeat_steps(&mut elapsed, false, true, 0.016, false, true),
                0
            );
        }
        assert_eq!(
            aim_repeat_steps(&mut elapsed, false, false, 0.016, false, true),
            0
        );
        assert_eq!(
            aim_repeat_steps(&mut elapsed, true, true, 0.016, false, true),
            1
        );
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
        g.apply_explosion(Explosion::new(
            vec2(100.0, 90.0),
            10.0,
            Damage::Weapon(0),
            0,
            false,
        ));
        assert_eq!(g.tanks[1].health, 100);
        g.apply_explosion(Explosion::new(
            vec2(100.0, 100.0),
            10.0,
            Damage::Weapon(0),
            0,
            false,
        ));
        assert_eq!(g.tanks[1].health, 0);
        assert_eq!(g.tanks[1].power, 0);
        g.apply_explosion(Explosion::new(
            vec2(100.0, 100.0),
            10.0,
            Damage::Weapon(0),
            2,
            false,
        ));
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
    fn empty_items_and_accessories_cannot_fire_or_consume_stock() {
        let mut g = game();
        g.current = 0;
        g.tanks[0].inventory.remove(ItemId::Nuke, 1);
        for id in [ItemId::Nuke, ItemId::Battery] {
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
    fn native_landscape_direction_alternates_across_round_generations() {
        let mut g = Game::new();
        g.settings.landscape.scanned_mountains_percent = 0;
        g.settings.landscape.sky = crate::settings::Sky::Cavern;
        g.start_game();
        assert!(g.landscape_reverse);
        g.start_round();
        assert!(!g.landscape_reverse);
        g.start_round();
        assert!(g.landscape_reverse);
        assert!(g
            .tanks
            .iter()
            .all(|t| t.alive && t.health == 100 && t.supported(&g.terrain)));
    }

    #[test]
    fn hardware_delays_change_animation_speed_with_fixed_projectile_steps() {
        let mut fast = game();
        let mut slow = game();
        fast.wind = 0;
        slow.wind = 0;
        fast.terrain = Terrain::flat(350);
        slow.terrain = Terrain::flat(350);
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
        assert!(g
            .trail
            .iter()
            .all(|&(x, y, _)| (0..W).contains(&x) && (0..H).contains(&y)));
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
        assert!(
            explosion.pos.y >= 260.0,
            "explosion at surface: {}",
            explosion.pos.y
        );
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
            fb.img.bytes = fb
                .img
                .bytes
                .chunks_exact((W * 4) as usize)
                .rev()
                .flatten()
                .copied()
                .collect();
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
        for mode in [
            ScoringMode::Basic,
            ScoringMode::Standard,
            ScoringMode::Greedy,
        ] {
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
                expected += if mode == ScoringMode::Basic {
                    6000
                } else {
                    8500
                };
                assert_eq!(g.tanks[0].account.cash.to_string(), expected.to_string());
                assert_eq!(
                    g.tanks[0].account.interest_earned.to_string(),
                    interest.to_string()
                );
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
        g.apply_explosion(Explosion::new(
            vec2(100.0, 100.0),
            20.0,
            Damage::Weapon(1),
            0,
            false,
        ));
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
            let share = if mode == ScoringMode::Basic {
                1000
            } else {
                1666
            };
            assert!(g
                .tanks
                .iter()
                .all(|t| t.account.cash == crate::economy::Money::from(share)));
            g.finish_round(&[0, 1, 2]);
            assert!(g
                .tanks
                .iter()
                .all(|t| t.account.cash == crate::economy::Money::from(share)));
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
        assert_eq!(
            economy::ranking(&g.tanks, ScoringMode::Basic, &g.prices),
            vec![0, 1, 2]
        );
        assert_eq!(
            economy::ranking(&g.tanks, ScoringMode::Standard, &g.prices),
            vec![0, 1, 2]
        );
        assert_eq!(
            economy::ranking(&g.tanks, ScoringMode::Greedy, &g.prices),
            vec![1, 0, 2]
        );
        assert_eq!(
            economy::net_worth(&g.tanks[1], &g.prices),
            Money::from(900 + 3168 + 9600)
        );
        g.prices[ItemId::Nuke as usize] = 0;
        assert_eq!(
            economy::ranking(&g.tanks, ScoringMode::Greedy, &g.prices),
            vec![0, 1, 2]
        );
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
        assert!(g.phase == Phase::Shop);
        assert_eq!(g.shop.player, 0);
        assert_eq!(g.tanks[0].account.cash, Money::from(20000));
        assert!(shop::buy(
            &mut g.tanks[0],
            ItemId::Missile,
            &g.prices,
            &g.settings,
            &mut g.market
        ));
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), 15);
        g.shop.player += 1;
        g.advance_shop();
        assert_eq!(g.shop.player, 2);
        assert_eq!(g.tanks[1].inventory, Inventory::starter());
        g.shop.player += 1;
        g.advance_shop();
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.tanks[0].account.cash, Money::from(19031));
        assert_eq!(g.tanks[1].account.cash, Money::from(21000));
        assert_eq!(g.tanks[2].account.cash, Money::from(21000));
        g.start_game();
        assert_eq!(g.tanks[0].account.cash, Money::from(20000));
        assert_eq!(g.tanks[0].inventory, Inventory::starter());
        let mut g = Game::new();
        g.settings
            .controllers
            .fill(Ctrl::Ai(crate::ai::AiKind::Shooter));
        g.settings.economics.cash_at_start = 21234;
        g.start_game();
        assert!(g.phase == Phase::Aim);
        // Shield ($20,000), then triggers ($1,000); interest on the $234 left.
        assert_eq!(g.tanks[0].account.cash, Money::from(245));
        assert_eq!(g.tanks[0].inventory.count(ItemId::Shield), 3);
        assert_eq!(g.tanks[0].inventory.count(ItemId::ContactTrigger), 25);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Missile), 10);
    }

    #[test]
    fn shop_rejects_unavailable_or_overstocked_purchases_without_charging() {
        let mut g = game();
        let prices = g.prices;
        let tank = &mut g.tanks[0];
        assert!(shop::offers(tank, &prices, &g.settings, weapons::Category::Weapon).is_empty());
        tank.account = crate::economy::Account::new(100000);
        assert!(!shop::buy(
            tank,
            ItemId::BabyMissile,
            &prices,
            &g.settings,
            &mut g.market
        ));
        assert!(shop::buy(
            tank,
            ItemId::Battery,
            &prices,
            &g.settings,
            &mut g.market
        ));
        assert!(shop::buy(
            tank,
            ItemId::LeapFrog,
            &prices,
            &g.settings,
            &mut g.market
        ));
        tank.inventory.add(ItemId::Missile, 89);
        let cash = tank.account.cash.clone();
        assert!(!shop::buy(
            tank,
            ItemId::Missile,
            &prices,
            &g.settings,
            &mut g.market
        ));
        assert_eq!(tank.account.cash, cash);
        assert_eq!(tank.inventory.count(ItemId::Missile), 99);
    }

    #[test]
    fn ten_round_shop_flow_preserves_stock_banks_interest_once_and_stops_at_final_round() {
        let mut g = Game::new();
        g.settings.players = 3;
        g.settings.controllers.fill(Ctrl::Human);
        g.settings.economics.cash_at_start = 100_000;
        g.settings.economics.computers_buy = false;
        g.settings.economics.free_market = true;
        g.settings.rounds = 10;
        g.start_game();
        for round in 1..=10 {
            assert!(g.phase == Phase::Shop);
            assert_eq!(g.round, round);
            assert_eq!(g.shop.player, 0);
            let prices = g.prices;
            let old_interest = g.tanks[0].account.interest_earned.clone();
            for player in 0..3 {
                assert_eq!(g.shop.player, player);
                assert!(shop::buy(
                    &mut g.tanks[player],
                    ItemId::Missile,
                    &g.prices,
                    &g.settings,
                    &mut g.market
                ));
                g.shop.player += 1;
                g.advance_shop();
                assert_eq!(g.prices, prices);
            }
            assert!(g.phase == Phase::Aim);
            assert_eq!(
                g.tanks[0].inventory.count(ItemId::Missile),
                10 + round as u8 * 5
            );
            assert!(g.tanks[0].account.interest_earned > old_interest);
            let account = g.tanks[0].account.clone();
            g.start_round();
            assert_eq!(g.tanks[0].account, account);
            g.finish_round(&[0]);
            let next_prices = g.prices;
            g.finish_round(&[0]);
            assert_eq!(g.prices, next_prices);
            g.continue_after_round();
        }
        assert!(g.phase == Phase::GameOver);
        let saved_prices = g.prices;
        let saved_market = g.market.prices();
        g.start_game();
        assert_eq!(g.market.prices(), saved_market);
        for i in 0..weapons::ITEM_COUNT {
            if i != ItemId::AutoDefense as usize {
                assert_eq!(g.prices[i], saved_prices[i]);
            }
        }
        assert_eq!(
            g.prices[ItemId::AutoDefense as usize],
            shop::auto_defense_price(
                saved_market[ItemId::AutoDefense as usize],
                g.settings.economics.interest_rate,
                g.settings.rounds
            )
        );
        assert_eq!(g.tanks[0].inventory, Inventory::starter());
    }

    #[test]
    fn inter_round_shop_skips_cashless_players_and_respects_computers_buy() {
        let mut g = Game::new();
        g.settings.players = 3;
        g.settings.economics.computers_buy = false;
        g.start_game();
        g.tanks[1].account.cash = crate::economy::Money::from(100000);
        g.tanks[2].ctrl = Ctrl::Human;
        g.tanks[2].account.cash = crate::economy::Money::from(100000);
        g.tanks[2].alive = false;
        g.finish_round(&[]);
        g.continue_after_round();
        assert!(g.phase == Phase::Shop);
        assert_eq!(g.shop.player, 2); // Dead players also get to buy for next round.
        assert_eq!(g.tanks[1].inventory, Inventory::starter());
        g.shop.player += 1;
        g.advance_shop();
        assert!(g.phase == Phase::Aim);
        g.settings.economics.computers_buy = true;
        g.finish_round(&[]);
        g.continue_after_round();
        assert!(g.tanks[1].inventory.count(ItemId::SuperMag) > 0);
        assert!(g.tanks[1].inventory.count(ItemId::Parachute) > 0);
        assert_eq!(g.shop.player, 2);
    }

    #[test]
    fn parachutes_predict_before_falling_obey_strict_threshold_and_use_fixed_descent_rate() {
        for impact_damage in [true, false] {
            for delay in [0, 10, 1000] {
                let mut g = game();
                g.terrain = Terrain::flat(120);
                for tank in &mut g.tanks {
                    tank.y = 119;
                }
                g.tanks[0].y = 100;
                g.tanks[0].inventory.add(ItemId::Parachute, 2);
                g.settings.physics.impact_damage = impact_damage;
                g.settings.hardware.falling_delay = delay;
                g.action_owner = Some(1);
                assert!(g.update_falling(0.01));
                assert!(g.tanks[0].equipment.chute_open);
                assert_eq!(g.tanks[0].y, 100); // 20 ms per pixel, even at Falling Delay zero.
                assert_eq!(g.tanks[0].inventory.count(ItemId::Parachute), 1);
                g.update_falling(0.01);
                assert_eq!(g.tanks[0].y, 101);
                for _ in 0..40 {
                    g.update_falling(0.01);
                }
                assert_eq!(g.tanks[0].y, 119);
                assert_eq!(g.tanks[0].health, 100);
                assert_eq!(g.tanks[0].inventory.count(ItemId::Parachute), 1);
                assert!(!g.tanks[0].equipment.chute_open);
                assert_eq!(g.tanks[1].account.cash, crate::economy::Money::default());
            }
        }
        for (deployed, threshold, stock) in [(true, 38, 1), (false, 5, 1), (true, 5, 0)] {
            let mut g = game();
            g.terrain = Terrain::flat(120);
            for tank in &mut g.tanks {
                tank.y = 119;
            }
            let t = &mut g.tanks[0];
            t.y = 100;
            t.equipment.parachutes_deployed = deployed;
            t.equipment.parachute_threshold = threshold;
            t.inventory.add(ItemId::Parachute, stock);
            g.settings.hardware.falling_delay = 0;
            assert!(!g.update_falling(0.01));
            assert_eq!(g.tanks[0].health, 62); // predicted == threshold never deploys
            assert_eq!(g.tanks[0].inventory.count(ItemId::Parachute), stock as u8);
        }
    }

    #[test]
    fn parachutes_do_not_cancel_collision_damage_and_enemy_landings_credit_actual_damage() {
        for impact_damage in [true, false] {
            let mut g = game();
            g.terrain = Terrain::flat(131);
            g.settings.physics.impact_damage = impact_damage;
            for tank in &mut g.tanks {
                tank.y = 130;
            }
            g.tanks[0].x = 100;
            g.tanks[0].y = 100;
            g.tanks[1].x = 100;
            g.tanks[0].inventory.add(ItemId::Parachute, 1);
            g.action_owner = Some(2);
            for _ in 0..50 {
                g.update_falling(0.02);
            }
            assert_eq!(g.tanks[0].y, 121);
            assert_eq!(g.tanks[0].health, 69); // half of 42 + 10
            assert_eq!(g.tanks[1].health, 8); // 42 + 50
            assert_eq!(g.tanks[0].inventory.count(ItemId::Parachute), 0);
            assert_eq!(
                g.tanks[0].account.cash,
                crate::economy::Money::from(92 * 30)
            );
            assert_eq!(
                g.tanks[2].account.cash,
                crate::economy::Money::from(31 * 30)
            );
        }
    }

    #[test]
    fn disabled_falls_do_not_consume_parachutes_and_rounds_reset_descent_but_keep_preferences() {
        let mut g = game();
        g.tanks[0].inventory.add(ItemId::Parachute, 2);
        g.tanks[0].equipment.parachute_threshold = 40;
        g.settings.physics.tanks_fall = false;
        assert!(!g.update_falling(10.0));
        assert_eq!(g.tanks[0].inventory.count(ItemId::Parachute), 2);
        g.tanks[0].equipment.chute_open = true;
        g.tanks[0].equipment.fall_started = true;
        g.start_round();
        assert!(!g.tanks[0].equipment.chute_open);
        assert!(!g.tanks[0].equipment.fall_started);
        assert_eq!(g.tanks[0].equipment.parachute_threshold, 40);
        g.start_game();
        assert_eq!(g.tanks[0].equipment.parachute_threshold, 5);
    }

    #[test]
    fn battery_heals_without_waste_or_automatically_changing_chosen_power() {
        let mut g = game();
        let tank = &mut g.tanks[0];
        tank.inventory.add(ItemId::Battery, 3);
        tank.health = 89;
        tank.power = 800;
        assert!(tank.use_battery());
        assert_eq!(tank.health, 99);
        assert_eq!(tank.max_power(), 990);
        assert_eq!(tank.power, 800);
        assert!(tank.use_battery());
        assert_eq!(tank.health, 100);
        assert!(!tank.use_battery());
        assert_eq!(tank.inventory.count(ItemId::Battery), 1);
        tank.health = 0;
        assert!(!tank.use_battery());
        assert_eq!(tank.inventory.count(ItemId::Battery), 1);
    }

    #[test]
    fn retreat_has_no_damage_or_kill_award_and_resumes_spatial_turn_order() {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(101);
        g.tanks[0].last_hit = Some(1);
        let stock = g.tanks[0].inventory.clone();
        g.begin_retreat();
        assert!(g.phase == Phase::Retreat);
        assert!(!g.tanks[0].alive);
        assert_eq!(g.tanks[0].inventory, stock);
        assert!(g.expls.is_empty());
        g.phase_t = 2.4;
        g.update_retreat();
        assert!(g.phase == Phase::Retreat);
        g.phase_t = 2.5;
        g.update_retreat();
        assert!(g.phase == Phase::Resolve);
        g.update_resolve(0.01);
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 1);
        for t in &g.tanks {
            assert_eq!(t.kills, 0);
            assert!(t.account.cash.is_zero());
        }
    }

    #[test]
    fn tank_information_is_numbered_by_position_and_consumes_input() {
        let mut g = game();
        g.tanks[0].x = 500;
        g.tanks[1].x = 100;
        g.tanks[2].x = 300;
        assert!(g.handle_control_action(ControlAction::Info(0)));
        let controls::Panel::Message(lines) = &g.controls.panel else {
            panic!("missing info panel")
        };
        assert!(lines[0].contains(&g.tanks[1].name));
        assert!(g.projs.is_empty());
        g.controls = Controls::default();
        assert!(g.handle_control_action(ControlAction::Info(9)));
        assert!(g.controls.is_closed());
        assert!(g.handle_control_action(ControlAction::Status));
        let controls::Panel::Message(lines) = &g.controls.panel else {
            panic!("missing status panel")
        };
        assert!(lines.iter().any(|line| line == "Walls: None"));
    }

    #[test]
    fn batch_nine_weapons_can_be_bought_selected_and_fired_without_changing_future_item_guards() {
        let mut g = game();
        g.current = 0;
        g.tanks[0].account = crate::economy::Account::new(1_000_000);
        for (id, cost, bundle) in [
            (ItemId::LeapFrog, 10000, 2),
            (ItemId::DeathsHead, 20000, 1),
            (ItemId::SmokeTracer, 500, 10),
            (ItemId::BabyRoller, 5000, 10),
            (ItemId::HeavyRoller, 6750, 2),
        ] {
            let old_cash = g.tanks[0].account.cash.clone();
            assert!(shop::buy(
                &mut g.tanks[0],
                id,
                &g.prices,
                &g.settings,
                &mut g.market
            ));
            assert_eq!(g.tanks[0].inventory.count(id), bundle);
            let mut expected_cash = old_cash;
            expected_cash.subtract(&crate::economy::Money::from(cost));
            assert_eq!(g.tanks[0].account.cash, expected_cash);
            assert_eq!(g.tanks[0].use_item(id), crate::tank::ItemUse::Applied);
            g.fire();
            assert_eq!(g.projs.len(), 1);
            assert_eq!(g.projs[0].weapon, id);
            assert_eq!(g.tanks[0].inventory.count(id), bundle - 1);
            assert_eq!(g.projs[0].kind == PKind::Carrier, id == ItemId::DeathsHead);
            g.projs.clear();
        }
    }

    #[test]
    fn smoke_and_trace_paths_persist_across_shots_and_clear_at_round_boundaries() {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(H);
        for id in [ItemId::Tracer, ItemId::SmokeTracer, ItemId::BabyMissile] {
            g.clear_traces();
            g.projs.clear();
            let mut p = projectile(id, PKind::Shell);
            p.pos = vec2(200.0, 200.0);
            p.vel = vec2(200.0, 0.0);
            g.projs.push(p);
            g.step_projs(0.05);
            if id == ItemId::SmokeTracer {
                let pixels = g.persistent_traces.count();
                assert!(pixels > 5);
                g.projs.clear();
                g.tanks[0].weapon = ItemId::BabyMissile;
                g.fire();
                assert_eq!(g.persistent_traces.count(), pixels);
                g.projs.clear();
            } else {
                assert_eq!(g.persistent_traces.count(), 0);
            }
            if id == ItemId::Tracer {
                assert!(!g.trail.is_empty());
            }
        }
        g.clear_traces();
        g.projs.clear();
        g.settings.weapons.trace_paths = true;
        let mut p = projectile(ItemId::BabyMissile, PKind::Shell);
        p.pos = vec2(200.0, 200.0);
        p.vel = vec2(200.0, 0.0);
        g.projs.push(p);
        g.step_projs(0.05);
        assert!(g.persistent_traces.count() > 5);
        let mut fb = Fb::new();
        g.persistent_traces.draw(&mut fb);
        let index = ((200 * W + 201) * 4) as usize;
        let color = g.tanks[0].color;
        assert_eq!(
            &fb.img.bytes[index..index + 3],
            &[color.0, color.1, color.2]
        );
        g.start_round();
        assert_eq!(g.persistent_traces.count(), 0);
    }

    #[test]
    fn trace_weapons_do_not_damage_tanks_or_terrain_and_funky_final_waits_for_its_chain() {
        let mut g = game();
        for id in [ItemId::Tracer, ItemId::SmokeTracer] {
            let p = projectile(id, PKind::Shell);
            let mut children = Vec::new();
            assert!(weapons::impact(
                p,
                vec2(300.0, 90.0),
                Some(1),
                &g.terrain,
                &mut g.tanks,
                &mut children,
                &mut g.expls
            )
            .is_none());
            assert_eq!(g.tanks[1].health, 100);
            assert!(g.expls.is_empty());
        }
        g.terrain = Terrain::flat(H);
        let p = Proj {
            pos: vec2(200.0, 200.0),
            scale: 3.0,
            ..projectile(ItemId::FunkyBomb, PKind::Shell)
        };
        let _ = weapons::impact(
            p,
            p.pos,
            None,
            &g.terrain,
            &mut g.tanks,
            &mut g.projs,
            &mut g.expls,
        );
        assert!(g.projs.is_empty());
        assert_eq!(g.expls.len(), 1);
        g.expls[0].t = g.expls[0].dur;
        g.settings.physics.tanks_fall = false;
        g.update_resolve(0.0);
        assert!((6..=11).contains(&g.projs.len()));
        assert_eq!(
            g.projs
                .iter()
                .filter(|p| p.kind == PKind::FunkyFinal)
                .count(),
            1
        );
        g.step_projs(SIM_DT);
        assert!(g.projs.iter().any(|p| p.kind == PKind::FunkyFinal));
        g.projs.retain(|p| p.kind == PKind::FunkyFinal);
        g.expls.push(Explosion::new(
            vec2(250.0, 220.0),
            15.0,
            ItemId::FunkyBomb.item().damage(),
            0,
            false,
        ));
        g.step_projs(SIM_DT);
        assert_eq!(g.projs.len(), 1);
        g.expls.clear();
        g.step_projs(SIM_DT);
        assert!(g.projs.is_empty());
        assert_eq!(g.expls.len(), 1);
        assert_eq!(g.expls[0].radius, 120.0);
        assert_eq!(g.expls[0].pos, p.pos);
    }

    #[test]
    fn leap_and_funky_complete_their_whole_chain_before_advancing_the_turn() {
        for id in [ItemId::LeapFrog, ItemId::FunkyBomb] {
            let mut g = game();
            g.current = 0;
            g.settings.physics.tanks_fall = false;
            g.settings.weapons.tunneling = false;
            g.terrain = Terrain::flat(250);
            g.tanks[0].inventory.add(id, 2);
            g.tanks[0].weapon = id;
            let before = g.tanks[0].inventory.count(id);
            g.fire();
            g.projs[0].pos = vec2(200.0, 200.0);
            g.projs[0].vel = vec2(90.0, 120.0);
            let mut radii = Vec::new();
            for _ in 0..2000 {
                for e in &g.expls {
                    if !radii.contains(&(e.radius as i32)) {
                        radii.push(e.radius as i32);
                    }
                }
                g.update_resolve(0.01);
                if g.phase != Phase::Resolve {
                    break;
                }
            }
            assert!(g.phase == Phase::Aim);
            assert_eq!(g.current, 1);
            assert_eq!(g.tanks[0].inventory.count(id), before - 1);
            if id == ItemId::LeapFrog {
                assert_eq!(radii, vec![30, 25, 20]);
            } else {
                assert_eq!(radii[0], 20);
                assert_eq!(*radii.last().unwrap(), 40);
            }
            assert!(g.projs.is_empty() && g.expls.is_empty());
        }
    }

    #[test]
    fn weapon_scale_is_snapshotted_for_carrier_children_and_leap_followups() {
        use crate::settings::ExplosionScale;
        let mut g = game();
        g.current = 0;
        g.settings.weapons.scale = ExplosionScale::Large;
        g.tanks[0].weapon = ItemId::Mirv;
        g.fire();
        assert_eq!(g.projs[0].radius, 60.0);
        g.settings.weapons.scale = ExplosionScale::Normal;
        g.projs[0].pos = vec2(200.0, 80.0);
        g.projs[0].vel = vec2(0.0, -1.0);
        g.terrain = Terrain::flat(H);
        g.step_projs(0.01);
        assert!(g.projs.iter().all(|p| p.radius == 60.0 && p.scale == 3.0));
        g.projs.clear();
        let p = Proj {
            scale: 3.0,
            radius: 90.0,
            vel: vec2(90.0, 120.0),
            ..projectile(ItemId::LeapFrog, PKind::Shell)
        };
        weapons::impact(
            p,
            p.pos,
            None,
            &g.terrain,
            &mut g.tanks,
            &mut g.projs,
            &mut g.expls,
        );
        assert_eq!(g.expls[0].radius, 90.0);
        assert_eq!(g.expls[0].follow_up.as_ref().unwrap().radius, 75.0);
    }

    #[test]
    fn sound_requests_follow_valid_fires_and_harmless_tracers_have_no_explosion_cue() {
        let mut g = game();
        g.current = 0;
        g.take_sound_frame();
        g.tanks[0].weapon = ItemId::Nuke;
        g.tanks[0].inventory.remove(ItemId::Nuke, 1);
        g.fire();
        assert!(g.take_sound_frame().cues.is_empty());
        g.tanks[0].weapon = ItemId::BabyMissile;
        g.fire();
        let frame = g.take_sound_frame();
        assert!(frame.enabled);
        assert_eq!(frame.cues, vec![Cue::Fire]);
        assert_eq!(frame.flight, None); // Default flight sounds are Off.
        assert!(g.take_sound_frame().cues.is_empty());
        for id in [ItemId::Tracer, ItemId::SmokeTracer] {
            let p = projectile(id, PKind::Shell);
            g.impact_projectile(p, vec2(300.0, 90.0), Some(1), &mut Vec::new());
            assert!(g.take_sound_frame().cues.is_empty());
        }
    }

    #[test]
    fn nuke_dirt_riot_digging_shield_and_death_actions_route_to_distinct_cues() {
        for (id, cue) in [
            (ItemId::Nuke, Cue::Nuke),
            (ItemId::BabyNuke, Cue::Nuke),
            (ItemId::Missile, Cue::Explosion),
            (ItemId::DirtBall, Cue::Dirt),
            (ItemId::RiotBomb, Cue::Riot),
            (ItemId::Digger, Cue::Dig),
            (ItemId::DirtCharge, Cue::Dirt),
        ] {
            let mut g = game();
            let p = projectile(id, PKind::Shell);
            g.impact_projectile(p, vec2(200.0, 250.0), None, &mut Vec::new());
            assert!(g.take_sound_frame().cues.contains(&cue));
        }
        for (id, cue) in [
            (ItemId::RiotCharge, Cue::Riot),
            (ItemId::EarthDisrupter, Cue::Disruptor),
        ] {
            let mut g = game();
            g.current = 0;
            g.tanks[0].inventory.add(id, 1);
            g.tanks[0].weapon = id;
            g.fire();
            assert_eq!(g.take_sound_frame().cues, vec![cue]);
        }
        let mut g = game();
        arm(&mut g, 1, ItemId::Shield);
        let mut p = projectile(ItemId::Nuke, PKind::Shell);
        p.pos = vec2(300.0, 81.0);
        p.vel = vec2(0.0, 100.0);
        g.projs.push(p);
        g.step_projs(SIM_DT);
        assert_eq!(g.take_sound_frame().cues, vec![Cue::ShieldHit]);
        assert!(g.expls.is_empty());
        let mut g = game();
        g.terrain = Terrain::flat(101);
        g.tanks[1].health = 0;
        g.update_resolve(0.01);
        assert!(g.take_sound_frame().cues.contains(&Cue::Death));
    }

    #[test]
    fn flight_audio_uses_airborne_shots_and_mute_discards_queued_events() {
        use crate::settings::FlightSound;
        let mut g = game();
        g.phase = Phase::Resolve;
        g.settings.sound.flight = FlightSound::Position;
        let mut p = projectile(ItemId::Missile, PKind::Shell);
        p.pos.y = 100.0;
        p.vel = vec2(100.0, 0.0);
        let mut marker = p;
        marker.kind = PKind::FunkyFinal;
        g.projs = vec![marker, p];
        assert_eq!(
            g.take_sound_frame().flight,
            sound::flight_bin(FlightSound::Position, 100.0, 100.0)
        );
        g.settings.sound.flight = FlightSound::Velocity;
        assert_eq!(
            g.take_sound_frame().flight,
            sound::flight_bin(FlightSound::Velocity, 100.0, 100.0)
        );
        g.settings.sound.enabled = false;
        g.sounds.post(Cue::Fire);
        let frame = g.take_sound_frame();
        assert!(!frame.enabled && frame.flight.is_none() && frame.cues.is_empty());
        g.settings.sound.enabled = true;
        assert!(g.take_sound_frame().cues.is_empty());
        g.projs[1].kind = PKind::Rolling {
            dir: 1,
            timer: 0.0,
            speed: 150.0,
        };
        assert_eq!(g.take_sound_frame().flight, None);
        g.projs[1].kind = PKind::Shell;
        g.phase = Phase::Menu;
        assert_eq!(g.take_sound_frame().flight, None);
    }

    #[test]
    fn sound_settings_do_not_change_projectile_physics() {
        let mut audible = game();
        let mut silent = game();
        for g in [&mut audible, &mut silent] {
            g.current = 0;
            g.wind = 0;
            g.terrain = Terrain::flat(H);
            g.tanks[0].weapon = ItemId::BabyMissile;
            g.tanks[0].angle = 60;
            g.tanks[0].power = 500;
        }
        audible.settings.sound.flight = crate::settings::FlightSound::Velocity;
        silent.settings.sound.enabled = false;
        audible.fire();
        silent.fire();
        for _ in 0..300 {
            audible.step_projs(SIM_DT);
            silent.step_projs(SIM_DT);
            audible.take_sound_frame();
            silent.take_sound_frame();
            assert_eq!(audible.projs.len(), silent.projs.len());
            for (a, b) in audible.projs.iter().zip(&silent.projs) {
                assert_eq!(a.pos, b.pos);
                assert_eq!(a.vel, b.vel);
            }
        }
    }

    fn arm(g: &mut Game, victim: usize, id: ItemId) {
        g.tanks[victim].inventory.add(id, 1);
        assert!(g.tanks[victim].engage_shield(id));
    }

    #[test]
    fn shield_direct_hits_chip_or_reflect_without_explosions_and_score_actual_shield_loss() {
        for id in crate::tank::SHIELDS {
            let mut g = game();
            g.terrain = Terrain::flat(H);
            arm(&mut g, 1, id);
            let strength = crate::shields::spec(id).unwrap().strength;
            let reflective = g.tanks[1]
                .equipment
                .active_shield
                .as_ref()
                .unwrap()
                .reflective();
            let mut p = projectile(ItemId::FunkyBomb, PKind::Shell);
            p.pos = vec2(300.0, 78.0);
            p.vel = vec2(0.0, 200.0);
            g.projs.push(p);
            g.step_projs(0.03);
            let lost = if reflective { 2 } else { 10 };
            assert_eq!(
                g.tanks[1]
                    .equipment
                    .active_shield
                    .as_ref()
                    .unwrap()
                    .strength,
                strength - lost
            );
            assert_eq!(g.tanks[1].health, 100);
            assert_eq!(
                g.tanks[0].account.cash,
                crate::economy::Money::from(lost as u32 * 2)
            );
            assert!(g.expls.is_empty()); // Funky is intercepted before it can start its reaction.
            assert_eq!(g.projs.len(), usize::from(reflective));
            if reflective {
                assert!(g.projs[0].vel.y < 0.0);
            }
        }
        let mut g = game();
        arm(&mut g, 1, ItemId::Shield);
        g.tanks[1].drain_shield(95);
        let mut p = projectile(ItemId::Nuke, PKind::Shell);
        p.pos = vec2(300.0, 81.0);
        p.vel = vec2(0.0, 100.0);
        g.projs.push(p);
        g.step_projs(SIM_DT);
        assert!(g.tanks[1].equipment.active_shield.is_none());
        assert_eq!(g.tanks[1].health, 100); // Direct chips never overflow into health.
        assert!(g.expls.is_empty());
        assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(10));
    }

    #[test]
    fn blasts_absorb_then_overflow_without_random_failure_and_basic_has_no_damage_awards() {
        for mode in [
            crate::settings::ScoringMode::Standard,
            crate::settings::ScoringMode::Basic,
        ] {
            let mut g = game();
            g.settings.economics.scoring = mode;
            arm(&mut g, 1, ItemId::Shield);
            g.apply_explosion(Explosion::new(
                vec2(300.0, 100.0),
                75.0,
                ItemId::Nuke.item().damage(),
                0,
                false,
            ));
            assert!(g.tanks[1].equipment.active_shield.is_none());
            assert_eq!(g.tanks[1].health, 90);
            assert_eq!(
                g.tanks[0].account.cash,
                crate::economy::Money::from(if mode == crate::settings::ScoringMode::Basic {
                    0
                } else {
                    500
                })
            );
        }
        let mut g = game();
        arm(&mut g, 1, ItemId::HeavyShield);
        g.apply_explosion(Explosion::new(
            vec2(300.0, 100.0),
            75.0,
            ItemId::Nuke.item().damage(),
            0,
            false,
        ));
        assert_eq!(g.tanks[1].health, 100);
        assert_eq!(
            g.tanks[1]
                .equipment
                .active_shield
                .as_ref()
                .unwrap()
                .strength,
            40
        );
        g.apply_explosion(Explosion::new(
            vec2(300.0, 100.0),
            75.0,
            ItemId::Nuke.item().damage(),
            0,
            false,
        ));
        assert_eq!(g.tanks[1].health, 30);
        assert!(g.tanks[1].equipment.active_shield.is_none());
        assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(2400));
    }

    #[test]
    fn returning_shots_ignore_the_owners_shield_and_apply_self_damage_and_kill_penalties() {
        let mut g = game();
        g.current = 0;
        g.wind = 0;
        g.settings.physics.tanks_fall = false;
        g.terrain = Terrain::flat(400);
        for tank in &mut g.tanks {
            tank.y = 399;
        }
        arm(&mut g, 0, ItemId::SuperMag);
        g.tanks[0].account.cash = crate::economy::Money::from(10000);
        g.tanks[0].angle = 90;
        g.tanks[0].power = 200;
        g.fire();
        for _ in 0..2000 {
            g.step_projs(SIM_DT);
            if g.projs.is_empty() {
                break;
            }
        }
        assert_eq!(g.tanks[0].health, 0);
        assert_eq!(g.tanks[0].shield_percent(), 100);
        assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(8500));
        g.update_resolve(1.0);
        assert!(!g.tanks[0].alive);
        assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(7000));
    }

    #[test]
    fn sandhog_charges_and_falls_bypass_shields_but_leap_blasts_deplete_them() {
        let mut g = game();
        arm(&mut g, 1, ItemId::SuperMag);
        g.apply_explosion(Explosion::new(
            vec2(300.0, 104.0),
            10.0,
            Damage::Sandhog(ItemId::BabySandhog as usize),
            0,
            false,
        ));
        assert!(g.tanks[1].health <= 0);
        assert_eq!(g.tanks[1].shield_percent(), 100);
        let mut g = game();
        arm(&mut g, 1, ItemId::HeavyShield);
        g.terrain = Terrain::flat(120);
        for t in &mut g.tanks {
            t.y = 119;
        }
        g.tanks[1].y = 100;
        g.settings.hardware.falling_delay = 0;
        g.update_falling(0.1);
        assert_eq!(g.tanks[1].health, 62);
        assert_eq!(g.tanks[1].shield_percent(), 100);
        let mut g = game();
        arm(&mut g, 1, ItemId::Shield);
        for radius in [30.0, 25.0, 20.0] {
            g.apply_explosion(Explosion::new(
                vec2(300.0, 104.0),
                radius,
                ItemId::LeapFrog.item().damage(),
                0,
                false,
            ));
        }
        assert!(g.tanks[1].health <= 0);
        assert!(g.tanks[1].equipment.active_shield.is_none());
    }

    #[test]
    fn a_complete_nearby_leap_frog_chain_beats_a_plain_shield_and_credits_actual_damage() {
        let mut g = game();
        g.current = 0;
        g.wind = 0; // This fixture checks shield/chain damage, not a random wind trajectory.
        g.settings.physics.tanks_fall = false;
        g.settings.physics.suspend_dirt = 100;
        g.settings.weapons.tunneling = false;
        g.terrain = Terrain::flat(120);
        for t in &mut g.tanks {
            t.y = 119;
        }
        arm(&mut g, 1, ItemId::Shield);
        g.tanks[0].inventory.add(ItemId::LeapFrog, 1);
        g.tanks[0].weapon = ItemId::LeapFrog;
        g.fire();
        g.projs[0].pos = vec2(320.0, 119.0);
        g.projs[0].vel = vec2(-60.0, 120.0);
        for _ in 0..2000 {
            g.update_resolve(0.01);
            if g.phase != Phase::Resolve {
                break;
            }
        }
        assert!(!g.tanks[1].alive);
        assert!(g.tanks[1].equipment.active_shield.is_none());
        assert_eq!(g.tanks[0].inventory.count(ItemId::LeapFrog), 0);
        assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(3700));
    }

    #[test]
    fn rollers_traverse_enemy_shields_and_release_without_chipping_or_exploding() {
        for id in [ItemId::BabyRoller, ItemId::Roller, ItemId::HeavyRoller] {
            let mut g = game();
            g.terrain = Terrain::flat(H);
            arm(&mut g, 1, ItemId::Shield);
            let mut p = projectile(id, PKind::Shell);
            p.pos = vec2(300.0, 79.0);
            p.vel = vec2(0.0, 200.0);
            g.projs.push(p);
            g.step_projs(0.02);
            assert!(matches!(
                g.projs[0].kind,
                PKind::ShieldRoll { victim: 1, .. }
            ));
            for _ in 0..1000 {
                g.step_projs(SIM_DT);
                if !matches!(g.projs[0].kind, PKind::ShieldRoll { .. }) {
                    break;
                }
            }
            assert!(g.projs[0].kind == PKind::Shell);
            assert!((g.projs[0].pos.x - 300.0).abs() >= 15.0);
            assert_eq!(g.tanks[1].health, 100);
            assert_eq!(g.tanks[1].shield_percent(), 100);
            assert!(g.expls.is_empty());
        }
    }

    #[test]
    fn force_reflection_at_a_carriers_apex_splits_the_warheads_after_paying_the_shield_cost() {
        let mut g = game();
        g.terrain = Terrain::flat(H);
        arm(&mut g, 1, ItemId::ForceShield);
        let mut p = projectile(ItemId::Mirv, PKind::Carrier);
        p.pos = vec2(300.0, 111.0);
        p.vel = vec2(0.0, -200.0);
        g.projs.push(p);
        g.step_projs(SIM_DT);
        assert_eq!(g.projs.len(), 5);
        assert!(g.projs.iter().all(|p| p.kind == PKind::Bomblet));
        assert_eq!(
            g.tanks[1]
                .equipment
                .active_shield
                .as_ref()
                .unwrap()
                .strength,
            98
        );
    }

    #[test]
    fn magnetic_fields_turn_slow_shots_but_fast_shots_cross_the_field_and_hit_the_shield() {
        let mut outcomes = Vec::new();
        for speed in [100.0, 1000.0] {
            let mut g = game();
            g.terrain = Terrain::flat(H);
            for tank in &mut g.tanks {
                tank.y = 399;
            }
            arm(&mut g, 1, ItemId::MagDeflector);
            let mut p = projectile(ItemId::BabyMissile, PKind::Shell);
            p.pos = vec2(290.0, 290.0);
            p.vel = vec2(10.0, speed);
            g.projs.push(p);
            for _ in 0..180 {
                g.step_projs(SIM_DT);
                if g.projs.is_empty() {
                    break;
                }
            }
            outcomes.push(g.projs.len());
            if speed == 100.0 {
                assert!(g.projs[0].vel.y < 0.0);
                assert!(g.projs[0].pos.y < 300.0);
            } else {
                assert_eq!(
                    g.tanks[1]
                        .equipment
                        .active_shield
                        .as_ref()
                        .unwrap()
                        .strength,
                    45
                );
            }
            assert_eq!(g.tanks[1].health, 100);
        }
        assert_eq!(outcomes, vec![1, 0]);
    }

    #[test]
    fn auto_defense_configures_only_owners_before_combat_and_resets_shields_each_round() {
        let mut g = game();
        g.tanks.iter_mut().for_each(|tank| tank.ctrl = Ctrl::Human);
        for player in [0, 2] {
            g.tanks[player].inventory.add(ItemId::AutoDefense, 1);
            g.tanks[player].inventory.add(ItemId::Shield, 2);
            g.tanks[player].equipment.shield = Some(ItemId::Shield);
        }
        g.start_round();
        assert!(g.phase == Phase::Defense);
        assert_eq!(g.defense_player, 0);
        let first_shooter = g.current;
        assert!(g.controls.defensive_only);
        g.controls.handle(
            &mut g.tanks[0],
            &ui::MenuInput {
                hotkey: Some('E'),
                ..Default::default()
            },
            false,
        );
        assert_eq!(g.tanks[0].shield_percent(), 100);
        assert_eq!(g.tanks[0].inventory.count(ItemId::Shield), 1);
        g.controls.handle(
            &mut g.tanks[0],
            &ui::MenuInput {
                hotkey: Some('R'),
                ..Default::default()
            },
            false,
        );
        assert!(g.controls.panel == controls::Panel::Control); // No retreat/fire in defense setup.
        g.defense_player += 1;
        g.advance_defense();
        assert!(g.phase == Phase::Defense);
        assert_eq!(g.defense_player, 2);
        g.tanks[2].engage_shield(ItemId::Shield);
        g.defense_player += 1;
        g.advance_defense();
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, first_shooter);
        assert!(!g.controls.defensive_only);
        for player in [0, 2] {
            assert_eq!(g.tanks[player].inventory.count(ItemId::AutoDefense), 1);
        }
        g.round += 1;
        g.start_round();
        assert!(g.phase == Phase::Defense);
        assert_eq!(g.tanks[0].shield_percent(), 0);
        assert_eq!(g.tanks[2].shield_percent(), 0);
        assert_eq!(g.tanks[0].inventory.count(ItemId::AutoDefense), 1);
        g.start_game();
        assert!(g
            .tanks
            .iter()
            .all(|t| t.inventory.count(ItemId::AutoDefense) == 0));
    }

    #[test]
    fn computer_auto_defense_uses_an_owned_shield_without_presenting_a_human_panel() {
        let mut g = game();
        for tank in &mut g.tanks {
            tank.ctrl = Ctrl::Ai(crate::ai::AiKind::Shooter);
            tank.inventory.add(ItemId::AutoDefense, 1);
            tank.inventory.add(ItemId::Shield, 1);
            tank.inventory.add(ItemId::SuperMag, 1);
        }
        g.start_round();
        assert!(g.phase == Phase::Aim);
        for tank in &g.tanks {
            assert_eq!(
                tank.equipment.active_shield.as_ref().unwrap().item,
                ItemId::SuperMag
            );
            assert_eq!(tank.inventory.count(ItemId::AutoDefense), 1);
            assert_eq!(tank.inventory.count(ItemId::SuperMag), 0);
            assert_eq!(tank.inventory.count(ItemId::Shield), 1);
        }
    }

    #[test]
    fn auto_defense_price_updates_with_remaining_rounds_and_never_adjusts_twice() {
        let mut g = game();
        g.settings.economics.interest_rate = 0.0;
        g.settings.rounds = 10;
        g.start_game();
        assert_eq!(g.prices[ItemId::AutoDefense as usize], 16500);
        g.refresh_prices();
        assert_eq!(g.prices[ItemId::AutoDefense as usize], 16500);
        g.round = 10;
        g.begin_shop();
        assert_eq!(g.prices[ItemId::AutoDefense as usize], 3000);
    }

    #[test]
    fn batch_ten_catalogue_items_buy_select_and_fire_with_instant_actions_consuming_one() {
        for item in weapons::ITEMS.iter().filter(|item| {
            matches!(
                item.kind,
                ItemKind::Weapon(
                    WKind::Riot
                        | WKind::Wedge
                        | WKind::Tunnel
                        | WKind::DirtCharge
                        | WKind::Disruptor
                )
            ) || matches!(item.id, ItemId::DirtClod | ItemId::TonOfDirt)
        }) {
            let mut g = game();
            g.current = 0;
            g.settings.weapons.useless_items = true;
            g.tanks[0].account = crate::economy::Account::new(1_000_000);
            assert!(shop::buy(
                &mut g.tanks[0],
                item.id,
                &g.prices,
                &g.settings,
                &mut g.market
            ));
            assert_eq!(g.tanks[0].use_item(item.id), crate::tank::ItemUse::Applied);
            let stock = g.tanks[0].inventory.count(item.id);
            g.fire();
            assert_eq!(g.tanks[0].inventory.count(item.id), stock - 1);
            assert!(g.phase == Phase::Resolve);
            if earth::is_instant(item.id) {
                assert!(g.projs.is_empty() && g.earth.active());
            } else {
                assert_eq!(g.projs.len(), 1);
            }
        }
    }

    #[test]
    fn a_buried_tank_can_open_a_path_with_riot_charge_without_firing_or_taking_damage() {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(80);
        g.tanks[0].angle = 90;
        g.tanks[0].weapon = ItemId::RiotCharge;
        g.tanks[0].inventory.add(ItemId::RiotCharge, 1);
        assert!(g.terrain.solid(100, 84));
        g.fire();
        assert!(g.projs.is_empty());
        assert!(!g.terrain.solid(100, 84));
        assert!(!g.terrain.solid(100, 79));
        assert!(g.terrain.solid(100, 101)); // Support below the tank is retained.
        assert_eq!(g.tanks[0].health, 100);
        g.update_resolve(0.6);
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 1);
        assert_eq!(g.tanks[0].inventory.count(ItemId::RiotCharge), 0);
        g.current = 0;
        g.tanks[0].weapon = ItemId::BabyMissile;
        g.fire();
        g.step_projs(SIM_DT);
        assert_eq!(g.projs.len(), 1); // The barrel is no longer embedded in soil.
    }

    #[test]
    fn riot_bomb_has_no_direct_damage_but_awards_actual_fall_damage_to_the_shooter() {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(131);
        for tank in &mut g.tanks {
            tank.y = 130;
        }
        g.settings.hardware.falling_delay = 0;
        g.action_owner = Some(0);
        g.settle_dirt = false;
        let p = projectile(ItemId::RiotBomb, PKind::Shell);
        g.impact_projectile(p, vec2(300.0, 140.0), Some(1), &mut Vec::new());
        assert_eq!(g.tanks[1].health, 100);
        assert!(g.expls.is_empty());
        let (drop, _) = g.predicted_landing(1);
        assert!(drop > 30 && drop < 50);
        g.update_falling(0.01);
        assert_eq!(g.tanks[1].health, 100 - 2 * drop);
        assert_eq!(
            g.tanks[0].account.cash,
            crate::economy::Money::from((2 * drop * 30) as u32)
        );
    }

    #[test]
    fn dirt_spheres_grow_without_lifting_or_damaging_tanks_and_keep_fixed_radii() {
        for id in [ItemId::DirtClod, ItemId::DirtBall, ItemId::TonOfDirt] {
            let mut g = game();
            g.terrain = Terrain::flat(400);
            g.settings.physics.tanks_fall = false;
            g.settle_dirt = false;
            g.phase = Phase::Resolve;
            let p = projectile(id, PKind::Shell);
            let mut children = Vec::new();
            weapons::impact(
                p,
                vec2(300.0, 100.0),
                Some(1),
                &g.terrain,
                &mut g.tanks,
                &mut children,
                &mut g.expls,
            );
            assert_eq!(g.expls[0].radius, id.item().radius);
            g.update_resolve(0.05);
            assert!(g.terrain.solid(300, 100));
            assert!(!g.terrain.solid(300 + id.item().radius as i32 - 1, 100));
            g.update_resolve(2.0);
            assert!(g.terrain.solid(300 + id.item().radius as i32, 100));
            assert_eq!(g.tanks[1].health, 100);
            assert_eq!(g.tanks[1].y, 100);
        }
    }

    #[test]
    fn disrupter_forces_floating_dirt_to_fall_even_at_suspend_one_hundred() {
        let mut g = game();
        g.current = 0;
        g.terrain = Terrain::flat(400);
        for tank in &mut g.tanks {
            tank.y = 399;
        }
        g.terrain.add_dirt(200, 200, 3, 0);
        g.settings.physics.suspend_dirt = 100;
        g.tanks[0].inventory.add(ItemId::EarthDisrupter, 1);
        g.tanks[0].weapon = ItemId::EarthDisrupter;
        g.fire();
        assert!(!g.settle_dirt);
        assert!(g.projs.is_empty());
        g.update_resolve(0.4);
        assert!(g.terrain.solid(200, 200));
        for _ in 0..40 {
            g.update_resolve(0.1);
            if g.phase != Phase::Resolve {
                break;
            }
        }
        assert!(g.settle_dirt);
        assert!(!g.terrain.solid(200, 200));
        assert!(g.phase == Phase::Aim);
        assert_eq!(g.current, 1);
        assert!(g.tanks.iter().all(|t| t.health == 100));
    }

    #[test]
    fn digger_direct_hits_fizzle_and_sandhog_contacts_only_chip_before_the_terminal_blast() {
        for (id, damage) in [(ItemId::Digger, 0), (ItemId::Sandhog, 10)] {
            let mut g = game();
            g.terrain = Terrain::flat(H);
            let mut p = projectile(id, PKind::Shell);
            p.pos = vec2(300.0, 90.0);
            g.projs.push(p);
            g.step_projs(SIM_DT);
            assert_eq!(g.tanks[1].health, 100 - damage);
            assert_eq!(
                g.tanks[0].account.cash,
                crate::economy::Money::from(damage as u32 * 30)
            );
            if damage == 0 {
                assert!(!g.earth.active() && g.expls.is_empty());
            } else {
                assert!(g.earth.active());
            }
        }
        let mut g = game();
        g.apply_explosion(Explosion::new(
            vec2(300.0, 104.0),
            10.0,
            Damage::Sandhog(ItemId::BabySandhog as usize),
            0,
            false,
        ));
        assert_eq!(g.tanks[1].health, -10);
        assert_eq!(g.tanks[1].last_hit, Some(0));
        assert_eq!(g.tanks[0].account.cash, crate::economy::Money::from(3000));
        assert_eq!(explosion_damage(5.01, 10.0, Damage::Sandhog(0)), 50);
        assert_eq!(explosion_damage(5.01, 10.0, Damage::Weapon(0)), 49);
    }

    #[test]
    fn tunnel_and_spray_effects_complete_before_the_turn_advances() {
        for id in [
            ItemId::BabyDigger,
            ItemId::HeavyDigger,
            ItemId::BabySandhog,
            ItemId::HeavySandhog,
            ItemId::DirtCharge,
        ] {
            let mut g = game();
            g.current = 0;
            g.terrain = Terrain::flat(300);
            g.settings.physics.tanks_fall = false;
            g.settings.physics.suspend_dirt = 100;
            g.tanks[0].inventory.add(id, 1);
            g.tanks[0].weapon = id;
            g.fire();
            g.projs[0].pos = vec2(220.0, 299.0);
            g.projs[0].vel = vec2(0.0, 300.0);
            g.step_projs(0.01);
            assert!(g.earth.active());
            assert!(g.phase == Phase::Resolve);
            for _ in 0..1200 {
                g.update_resolve(0.1);
                if g.phase != Phase::Resolve {
                    break;
                }
            }
            assert!(g.phase == Phase::Aim);
            assert!(!g.earth.active());
            assert_eq!(g.tanks[0].inventory.count(id), 0);
        }
    }

    #[test]
    fn random_walls_stay_fixed_for_the_round_and_erratic_walls_for_the_whole_shot() {
        for mode in [WallType::Random, WallType::Erratic] {
            let mut g = game();
            g.settings.physics.walls = mode;
            g.start_round();
            assert!(physics::FIXED_WALLS.contains(&g.wall_type()));
            if mode == WallType::Random {
                let selected = g.wall_type();
                for _ in 0..10 {
                    g.begin_turn();
                    assert_eq!(g.wall_type(), selected);
                }
            } else {
                g.active_walls = WallType::Random; // A new turn must resolve a concrete type.
                g.begin_turn();
                assert!(physics::FIXED_WALLS.contains(&g.wall_type()));
            }
            let selected = g.wall_type();
            g.current = 0;
            g.tanks[0].weapon = ItemId::Mirv;
            g.terrain = Terrain::flat(H);
            g.fire();
            assert_eq!(g.physics().walls, selected);
            g.projs[0].pos = vec2(200.0, 100.0);
            g.projs[0].vel = vec2(0.0, -1.0);
            g.step_projs(0.01);
            assert_eq!(g.projs.len(), 5);
            assert_eq!(g.wall_type(), selected);
            for _ in 0..10 {
                g.step_projs(SIM_DT);
                assert_eq!(g.physics().walls, selected);
            }
            g.active_walls = WallType::Erratic;
            g.start_round();
            assert!(physics::FIXED_WALLS.contains(&g.wall_type()));
        }
    }

    #[test]
    fn concrete_impacts_detonate_rollers_and_status_reports_resolved_walls() {
        let mut g = game();
        g.settings.physics.walls = WallType::Concrete;
        g.terrain = Terrain::flat(H);
        for id in [ItemId::BabyRoller, ItemId::Roller, ItemId::HeavyRoller] {
            g.expls.clear();
            let mut p = projectile(id, PKind::Shell);
            p.pos = vec2(W as f32 - 0.1, 100.0);
            p.vel = vec2(500.0, 0.0);
            g.projs.push(p);
            g.step_projs(0.01);
            assert!(g.projs.is_empty());
            assert_eq!(g.expls.len(), 1);
            assert_eq!(g.expls[0].radius, id.item().radius);
        }
        g.settings.physics.walls = WallType::Random;
        g.active_walls = WallType::Padded;
        assert!(g.handle_control_action(ControlAction::Status));
        let controls::Panel::Message(lines) = &g.controls.panel else {
            panic!("no status")
        };
        assert!(lines.iter().any(|line| line == "Walls: Padded"));
        g.settings.play.status_bar = false;
        assert_eq!(g.physics().ceiling, HUD_H as f32);
        g.settings.play.status_bar = true;
        assert_eq!(g.physics().ceiling, (HUD_H + 22) as f32);
    }

    #[test]
    fn shield_previews_render_all_types_fading_hit_flash_and_auto_defense() {
        let directory = std::env::var("SCORCHED_SHIELD_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        let mut g = game();
        g.current = 0;
        g.settings.play.status_bar = true;
        g.terrain = Terrain::flat(300);
        for tank in &mut g.tanks {
            tank.y = 299;
        }
        for id in crate::tank::SHIELDS {
            arm(&mut g, 0, id);
            g.draw(&mut fb);
            save(&fb, &format!("shield-{}", id.item().name));
        }
        g.tanks[0].drain_shield(100);
        g.tanks[0].equipment.active_shield.as_mut().unwrap().flash = 0.0;
        g.draw(&mut fb);
        save(&fb, "shield-half-strength");
        g.tanks[0].equipment.active_shield.as_mut().unwrap().flash = 0.18;
        g.time = 0.03;
        g.draw(&mut fb);
        save(&fb, "shield-hit-flash");
        g.tanks[0].inventory.add(ItemId::AutoDefense, 1);
        g.tanks[0].inventory.add(ItemId::Shield, 2);
        g.start_round();
        g.tanks[0].engage_shield(ItemId::Shield);
        g.draw(&mut fb);
        save(&fb, "auto-defense-panel");
        g.controls.panel = controls::Panel::Inventory;
        g.draw(&mut fb);
        save(&fb, "auto-defense-inventory");
    }

    #[test]
    fn earth_weapon_previews_render_terrain_and_animated_effects() {
        let directory = std::env::var("SCORCHED_EARTH_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for (id, name) in [
            (ItemId::RiotBlast, "riot-wedge"),
            (ItemId::HeavyRiotBomb, "riot-bomb"),
            (ItemId::HeavyDigger, "digger"),
            (ItemId::HeavySandhog, "sandhog"),
            (ItemId::DirtCharge, "dirt-charge"),
            (ItemId::EarthDisrupter, "disrupter"),
        ] {
            let mut g = game();
            g.current = 0;
            g.phase = Phase::Resolve;
            g.settings.physics.tanks_fall = false;
            g.settle_dirt = false;
            g.terrain = Terrain::flat(260);
            for tank in &mut g.tanks {
                tank.y = 259;
            }
            if earth::is_instant(id) {
                let pos = if id == ItemId::RiotBlast {
                    vec2(320.0, 280.0)
                } else {
                    vec2(0.0, 0.0)
                };
                g.earth.instant(id, pos, 90.0, &mut g.terrain);
            } else {
                let pos = if id == ItemId::DirtCharge {
                    vec2(320.0, 259.0)
                } else {
                    vec2(320.0, 280.0)
                };
                let mut p = projectile(id, PKind::Shell);
                p.pos = pos;
                p.vel = vec2(30.0, 150.0);
                g.impact_projectile(p, pos, None, &mut Vec::new());
            }
            g.update_resolve(0.3);
            g.draw(&mut fb);
            save(&fb, name);
            if id == ItemId::DirtCharge {
                g.earth = earth::Effects::default();
                g.settings.weapons.extra_dirt = true;
                let p = projectile(id, PKind::Shell);
                g.impact_projectile(p, vec2(320.0, 259.0), None, &mut Vec::new());
                g.update_resolve(0.3);
                g.draw(&mut fb);
                save(&fb, "extra-dirt");
            }
        }
        let mut g = game();
        g.current = 0;
        g.phase = Phase::Resolve;
        g.settings.physics.tanks_fall = false;
        g.settle_dirt = false;
        g.terrain = Terrain::flat(350);
        let mut explosion = Explosion::new(
            vec2(320.0, 280.0),
            70.0,
            ItemId::TonOfDirt.item().damage(),
            0,
            true,
        );
        explosion.t = explosion.dur * 0.2;
        g.expls.push(explosion);
        g.draw(&mut fb);
        save(&fb, "ton-of-dirt");
    }

    #[test]
    fn wall_previews_render_each_fixed_type_and_the_physics_menu() {
        let directory = std::env::var("SCORCHED_WALLS_PREVIEW_DIR").ok();
        let mut g = game();
        g.current = 0;
        g.settings.play.status_bar = true;
        g.terrain = Terrain::flat(380);
        for tank in &mut g.tanks {
            tank.y = 379;
        }
        let mut fb = Fb::new();
        for wall in physics::FIXED_WALLS {
            g.settings.physics.walls = wall;
            g.phase = Phase::Aim;
            g.draw(&mut fb);
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/wall-{}.png", wall.label()));
            }
        }
        g.phase = Phase::Menu;
        g.menu.page = Some(ui::Page::Physics);
        g.draw(&mut fb);
        if let Some(directory) = &directory {
            let mut image = fb.img.clone();
            image.bytes = image
                .bytes
                .chunks_exact((W * 4) as usize)
                .rev()
                .flatten()
                .copied()
                .collect();
            image.export_png(&format!("{directory}/physics-menu.png"));
        }
    }

    #[test]
    fn batch_nine_weapon_previews_render_trails_children_and_scaled_blasts() {
        use crate::settings::{BombIcon, ExplosionScale};
        let directory = std::env::var("SCORCHED_WEAPONS_PREVIEW_DIR").ok();
        let mut fb = Fb::new();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        let mut g = game();
        g.current = 0;
        g.phase = Phase::Resolve;
        g.settings.physics.tanks_fall = false;
        g.terrain = Terrain::flat(300);
        for tank in &mut g.tanks {
            tank.y = 299;
        }
        let p = Proj {
            pos: vec2(320.0, 270.0),
            ..projectile(ItemId::FunkyBomb, PKind::Shell)
        };
        weapons::impact(
            p,
            p.pos,
            None,
            &g.terrain,
            &mut g.tanks,
            &mut g.projs,
            &mut g.expls,
        );
        g.expls[0].t = g.expls[0].dur;
        g.update_resolve(0.0);
        for _ in 0..200 {
            g.step_projs(SIM_DT);
        }
        g.draw(&mut fb);
        save(&fb, "funky-trails");
        g.projs.clear();
        g.expls.clear();
        g.clear_traces();
        let p = Proj {
            pos: vec2(320.0, 120.0),
            vel: vec2(0.0, 0.0),
            ..projectile(ItemId::DeathsHead, PKind::Carrier)
        };
        weapons::split_carrier(p, &mut g.projs);
        for _ in 0..100 {
            g.step_projs(SIM_DT);
        }
        g.draw(&mut fb);
        save(&fb, "deaths-head");
        g.projs.clear();
        g.clear_traces();
        g.settings.weapons.trace_paths = true;
        g.projs.push(Proj {
            pos: vec2(150.0, 180.0),
            vel: vec2(300.0, -100.0),
            ..projectile(ItemId::SmokeTracer, PKind::Shell)
        });
        for _ in 0..300 {
            g.step_projs(SIM_DT);
        }
        g.draw(&mut fb);
        save(&fb, "smoke-tracer");
        g.projs.clear();
        g.clear_traces();
        g.settings.weapons.scale = ExplosionScale::Large;
        let mut explosion = Explosion::new(
            vec2(320.0, 260.0),
            90.0,
            ItemId::LeapFrog.item().damage(),
            0,
            false,
        );
        explosion.t = explosion.dur / 2.0;
        g.expls.push(explosion);
        g.draw(&mut fb);
        save(&fb, "scaled-leap-blast");
        g.expls.clear();
        g.projs.push(Proj {
            pos: vec2(320.0, 200.0),
            ..projectile(ItemId::Missile, PKind::Shell)
        });
        for (icon, name) in [
            (BombIcon::Invisible, "bomb-invisible"),
            (BombIcon::Small, "bomb-small"),
            (BombIcon::Big, "bomb-big"),
        ] {
            g.settings.weapons.bomb_icon = icon;
            g.draw(&mut fb);
            save(&fb, name);
        }
    }

    #[test]
    fn battlefield_panels_and_parachutes_render_for_full_inventory() {
        let mut g = game();
        g.current = 0;
        g.settings.play.status_bar = true;
        for item in &weapons::ITEMS {
            g.tanks[0].inventory.add(item.id, 99);
        }
        g.tanks[0].equipment.guidance = Some(ItemId::BallisticGuidance);
        g.tanks[0].equipment.shield = Some(ItemId::SuperMag);
        let mut fb = Fb::new();
        let directory = std::env::var("SCORCHED_CONTROLS_PREVIEW_DIR").ok();
        let save = |fb: &Fb, name: &str| {
            if let Some(directory) = &directory {
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        for (panel, name) in [
            (controls::Panel::Closed, "status-bar"),
            (controls::Panel::Control, "tank-controls"),
            (controls::Panel::Inventory, "inventory"),
            (controls::Panel::Threshold, "threshold"),
            (controls::Panel::Retreat, "retreat-confirm"),
        ] {
            g.controls.panel = panel;
            g.draw(&mut fb);
            save(&fb, name);
        }
        g.controls.panel = controls::Panel::Inventory;
        g.controls.selected = 36;
        g.draw(&mut fb);
        save(&fb, "inventory-accessories");
        g.controls.panel = controls::Panel::Closed;
        g.tanks[0].equipment.chute_open = true;
        g.draw(&mut fb);
        save(&fb, "parachute");
        g.begin_retreat();
        g.phase_t = 0.5;
        g.draw(&mut fb);
        save(&fb, "helicopter");
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
                image.bytes = image
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        save(&fb, "opening-shop");
        g.shop.selected = shop::PAGE_SIZE;
        g.draw(&mut fb);
        save(&fb, "shop-page-2");
        g.shop.category = weapons::Category::Miscellaneous;
        g.shop.selected = 0;
        g.draw(&mut fb);
        save(&fb, "shop-miscellaneous");
        for (panel, name) in [
            (shop::Panel::Inventory, "shop-inventory"),
            (
                shop::Panel::Sell {
                    item: ItemId::BabyNuke,
                    quantity: 2,
                },
                "shop-sell",
            ),
            (shop::Panel::Status, "shop-status"),
        ] {
            g.shop.panel = panel;
            g.draw(&mut fb);
            save(&fb, name);
        }
        let mut huge = crate::economy::Money::from(1);
        for _ in 0..200 {
            huge = huge.scaled(2, 1);
        }
        g.tanks[9].account.cash = huge.clone();
        assert!(crate::economy::money_text(&huge).len() <= 14);
        for mode in [
            ScoringMode::Basic,
            ScoringMode::Standard,
            ScoringMode::Greedy,
        ] {
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
            fb.img.bytes = fb
                .img
                .bytes
                .chunks_exact((W * 4) as usize)
                .rev()
                .flatten()
                .copied()
                .collect();
            fb.img.export_png(&path);
        }
    }
}
