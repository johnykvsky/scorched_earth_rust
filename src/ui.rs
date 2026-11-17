//! Menus, HUD, dialogs and software drawing for game objects.
use crate::gfx::{text_w, Fb, Rgb, H, W};
use crate::settings::{Settings, MAX_PLAYERS};
use crate::tank::{Tank, CTRLS, PLAYER_COLORS};
use crate::weapons::Explosion;
use macroquad::prelude::{
    get_char_pressed, get_last_key_pressed, is_key_down, is_key_pressed, is_mouse_button_pressed,
    mouse_position, screen_height, screen_width, KeyCode, MouseButton,
};

pub(crate) const HUD_H: i32 = 22;

/// A click is committed on release so a two-button fire chord cannot open a panel.
#[derive(Default)]
pub(crate) struct BattleMouse {
    previous: (bool, bool),
    origin: Option<(i32, i32, i32)>,
    elapsed: f32,
    repeated: bool,
    chord: bool,
}
pub(crate) enum BattleClick {
    None,
    Click(i32, i32, i32),
    Fire,
}
impl BattleMouse {
    pub fn update(&mut self, left: bool, right: bool, point: (i32, i32), dt: f32) -> BattleClick {
        let mut result = BattleClick::None;
        if left && right && !self.chord {
            self.chord = true;
            self.origin = None;
            if (0..W).contains(&point.0) && (0..H).contains(&point.1) {
                result = BattleClick::Fire;
            }
        } else if !self.chord {
            if self.previous == (false, false) && (left || right) {
                self.origin = Some((point.0, point.1, if left { 1 } else { -1 }));
                self.elapsed = 0.0;
                self.repeated = false;
            }
            if let Some((x, y, direction)) = self.origin {
                if !left && !right {
                    if !self.repeated && (point.0 - x).abs() <= 6 && (point.1 - y).abs() <= 6 {
                        result = BattleClick::Click(x, y, direction);
                    }
                    self.origin = None;
                } else if (0..208).contains(&x) && (0..HUD_H).contains(&y) {
                    self.elapsed += dt;
                    if self.elapsed >= 0.3 {
                        // A held field adjusts 100 units per second after its initial delay.
                        let steps = ((self.elapsed - 0.3) * 100.0).floor() as i32;
                        if steps > 0 {
                            result = BattleClick::Click(x, y, direction * steps);
                            self.elapsed -= steps as f32 / 100.0;
                            self.repeated = true;
                        }
                    }
                }
            }
        }
        if !left && !right {
            self.chord = false;
        }
        self.previous = (left, right);
        result
    }
}

pub(crate) fn button_index(input: &MenuInput, count: usize) -> Option<usize> {
    let (x, y, 1) = input.click? else {
        return None;
    };
    ((80..560).contains(&x) && (400..426).contains(&y))
        .then(|| ((x - 80) as usize * count / 480).min(count - 1))
}
pub(crate) fn mouse_buttons(fb: &mut Fb, labels: &[&str]) {
    let width = 480 / labels.len() as i32;
    for (i, label) in labels.iter().enumerate() {
        let x = 80 + i as i32 * width;
        fb.bevel(x, 400, width, 26);
        fb.text(x + (width - text_w(label)) / 2, 405, label, Rgb::BLACK);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Stay,
    Start,
    Save,
    HallOfFame,
    Quit,
}

/// Key auto-repeat for held keys (call `hit` once per frame per key).
#[derive(Default)]
pub(crate) struct KeyRep {
    pub(crate) held: Vec<(KeyCode, f32, f32)>,
    pub(crate) dt: f32,
}

impl KeyRep {
    /// Original timer rates, accumulated so input speed is independent of frame rate.
    pub(crate) fn aim_steps(&mut self, k: KeyCode, shift: bool, alt: bool) -> i32 {
        let pressed = is_key_pressed(k);
        let down = is_key_down(k);
        let i = match self.held.iter().position(|e| e.0 == k) {
            Some(i) => i,
            None => {
                self.held.push((k, 0.0, 0.0));
                self.held.len() - 1
            }
        };
        aim_repeat_steps(&mut self.held[i].1, pressed, down, self.dt, shift, alt)
    }

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
        let Some(e) = self.held.iter_mut().find(|e| e.0 == k) else {
            return false;
        };
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

pub(crate) fn display_angle(angle: i32) -> i32 {
    90 - (angle - 90).abs()
}

/// One degree / power unit per timer step; PgUp/PgDn apply 15 units per step.
pub(crate) fn aim_repeat_steps(
    elapsed: &mut f32,
    pressed: bool,
    down: bool,
    dt: f32,
    shift: bool,
    alt: bool,
) -> i32 {
    if !down || pressed || alt {
        *elapsed = 0.0;
        return i32::from(pressed);
    }
    let period = if shift { 30.0 } else { 15.0 } / (1_193_182.0 / 327.0);
    *elapsed += dt;
    let steps = (*elapsed / period).floor() as i32;
    *elapsed -= steps as f32 * period;
    steps
}

pub(crate) fn draw_hud(fb: &mut Fb, t: &Tank, wind: i32, status_bar: bool) {
    fb.bevel(0, 0, W, HUD_H);
    fb.text(8, 3, &format!("Power: {:4}", t.power), Rgb::BLACK);
    fb.text(
        112,
        3,
        &format!("Angle: {:2}", display_angle(t.angle)),
        Rgb::BLACK,
    );
    fb.rect(208, 5, 8, 12, t.color);
    fb.text(
        224,
        3,
        &t.name.chars().take(14).collect::<String>(),
        Rgb::BLACK,
    );

    // Small bomb icon, followed by ammunition and the selected weapon.
    fb.rect(352, 8, 8, 8, Rgb::BLACK);
    fb.rect(354, 6, 4, 2, Rgb::BLACK);
    fb.line(356, 6, 360, 3, Rgb::BLACK);
    let ammo = t.inventory.count(t.weapon);
    fb.text(
        368,
        3,
        &format!("{:2} {}", ammo, t.weapon.item().name),
        Rgb::BLACK,
    );

    if t.equipment.active_shield.is_some() {
        fb.text(W - 40, 3, &format!("{:3}%", t.shield_percent()), Rgb::BLACK);
    }

    let wind = match wind {
        0 => "No Wind".to_string(),
        v if v > 0 => format!("{} -->", v),
        v => format!("<-- {}", -v),
    };
    fb.text(
        W - text_w(&wind) - 8,
        HUD_H + if status_bar { 28 } else { 6 },
        &wind,
        Rgb(100, 200, 255),
    );
}

pub(crate) fn draw_tank(fb: &mut Fb, t: &Tank) {
    draw_tank_shape(fb, t.x, t.y, t.angle, t.color, t.shape);
}

pub(crate) fn draw_tank_shape(
    fb: &mut Fb,
    x: i32,
    y: i32,
    angle: i32,
    color: Rgb,
    shape: crate::tank::Shape,
) {
    use crate::tank::Shape;
    let dark = color.scale(0.5);
    match shape {
        Shape::Wheels => {
            fb.rect(x - 6, y - 5, 13, 3, color);
            for wheel in [-5, 0, 5] {
                fb.rect(x + wheel - 1, y - 1, 3, 2, dark);
            }
        }
        Shape::Treads | Shape::Heavy | Shape::Triple => {
            fb.rect(x - 7, y - 1, 15, 2, dark);
            fb.rect(x - 6, y - 3, 13, 2, color);
            if shape == Shape::Heavy {
                fb.rect(x - 6, y - 7, 13, 4, color);
            }
        }
        Shape::Bunker => {
            fb.rect(x - 7, y - 2, 15, 3, dark);
            fb.rect(x - 6, y - 6, 13, 4, color);
        }
        Shape::Tower => {
            fb.rect(x - 7, y - 1, 15, 2, dark);
            fb.rect(x - 2, y - 9, 5, 8, color);
            fb.rect(x - 5, y - 9, 11, 3, color);
        }
        Shape::Dish => {
            fb.rect(x - 7, y - 1, 15, 2, dark);
            fb.rect(x - 1, y - 6, 3, 6, color);
            fb.line(x - 6, y - 9, x - 3, y - 5, color);
            fb.line(x - 3, y - 5, x + 3, y - 5, color);
            fb.line(x + 3, y - 5, x + 6, y - 9, color);
        }
    }
    if matches!(shape, Shape::Wheels | Shape::Treads | Shape::Triple) {
        for dy in -4..=0 {
            for dx in -4..=4 {
                if dx * dx + dy * dy <= 16 {
                    fb.put(x + dx, y - 4 + dy, color);
                }
            }
        }
    }
    for angle in shape.barrel_angles(angle) {
        let a = (angle as f32).to_radians();
        fb.line(
            x,
            y - 6,
            (x as f32 + a.cos() * 10.0).round() as i32,
            ((y - 6) as f32 - a.sin() * 10.0).round() as i32,
            color.scale(1.4),
        );
    }
}

pub(crate) fn draw_walls(fb: &mut Fb, wall: crate::settings::WallType, top: i32) {
    let color = wall.color();
    fb.line(0, top, 0, H - 1, color);
    fb.line(W - 1, top, W - 1, H - 1, color);
    fb.line(0, H - 1, W - 1, H - 1, color);
    if !matches!(
        wall,
        crate::settings::WallType::None | crate::settings::WallType::WrapAround
    ) {
        fb.line(0, top, W - 1, top, color);
    }
}

pub(crate) fn draw_projectile(
    fb: &mut Fb,
    p: &crate::weapons::Proj,
    icon: crate::settings::BombIcon,
) {
    use crate::settings::BombIcon;
    let (x, y) = (p.pos.x as i32, p.pos.y as i32);
    match icon {
        BombIcon::Invisible => {}
        BombIcon::Small => fb.put(x, y, Rgb::WHITE),
        BombIcon::Big => {
            fb.rect(x - 1, y - 1, 3, 3, Rgb::WHITE);
            fb.line(x, y - 2, x + 2, y - 3, Rgb(255, 200, 70));
        }
    }
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

pub(crate) fn draw_explosion(fb: &mut Fb, e: &Explosion) {
    if let Some(death) = e.death {
        crate::presentation::death(fb, e, death);
        return;
    }
    if matches!(e.damage, crate::weapons::Damage::Weapon(2 | 3)) && !e.dirt {
        crate::presentation::nuclear(fb, e);
        return;
    }
    let p = e.t / e.dur;
    let r = if p < 0.4 {
        e.radius * p / 0.4
    } else {
        e.radius
    };
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
pub(crate) fn dialog(fb: &mut Fb, lines: &[String]) {
    let w = lines.iter().map(|l| text_w(l)).max().unwrap_or(0) + 40;
    let h = lines.len() as i32 * 18 + 24;
    let (x, y) = (W / 2 - w / 2, H / 2 - h / 2);
    fb.bevel(x, y, w, h);
    for (i, l) in lines.iter().enumerate() {
        fb.text_centered(y + 12 + i as i32 * 18, l, Rgb::BLACK);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Sound,
    Hardware,
    Economics,
    Physics,
    Landscape,
    Play,
    Weapons,
    Players,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Sound => "Sound",
            Self::Hardware => "Hardware",
            Self::Economics => "Economics",
            Self::Physics => "Physics",
            Self::Landscape => "Landscape",
            Self::Play => "Play Options",
            Self::Weapons => "Weapons",
            Self::Players => "Player Setup",
        }
    }
}

#[derive(Default)]
pub(crate) struct MenuState {
    pub(crate) selected: usize,
    pub(crate) page: Option<Page>,
    pub(crate) row: usize,
    pub(crate) notice: Option<String>,
    pub(crate) folder_edit: Option<String>,
    pub(crate) file_field: Option<FileField>,
    pub(crate) player_edit: Option<PlayerEdit>,
    pub(crate) key_edit: Option<crate::modes::KeyEditor>,
    pub(crate) reassign_only: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileField {
    Mountains,
    Attack,
    Death,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PlayerEdit {
    index: usize,
    name: String,
    ctrl: crate::tank::Ctrl,
    shape: crate::tank::Shape,
    team: u8,
    row: usize,
    last_ai: crate::ai::AiKind,
    select_name: bool,
    error: Option<String>,
}
impl PlayerEdit {
    fn open(s: &Settings, index: usize) -> Self {
        Self {
            index,
            name: s.names[index].clone(),
            ctrl: s.controllers[index],
            shape: s.shapes[index],
            team: s.team_ids[index],
            row: 0,
            last_ai: if let crate::tank::Ctrl::Ai(kind) = s.controllers[index] {
                kind
            } else {
                crate::ai::AiKind::Shooter
            },
            select_name: true,
            error: None,
        }
    }
    fn handle(&mut self, s: &mut Settings, input: &MenuInput, reassign: bool) -> bool {
        let fields = if reassign {
            2
        } else if s.play.teams == crate::settings::TeamMode::None {
            3
        } else {
            4
        };
        if input.up {
            self.row = (self.row + fields - 1) % fields;
        }
        if input.down {
            self.row = (self.row + 1) % fields;
        }
        if let Some((x, y, _)) = input.click {
            if (100..540).contains(&x) && (168..168 + fields as i32 * 28).contains(&y) {
                self.row = ((y - 168) / 28) as usize;
            }
        }
        if self.row == 0 && (self.ctrl == crate::tank::Ctrl::Human || reassign) {
            if input.backspace {
                if self.select_name {
                    self.name.clear();
                } else {
                    self.name.pop();
                }
                self.select_name = false;
            }
            for ch in input.text.chars().filter(|ch| !ch.is_control()) {
                if self.select_name {
                    self.name.clear();
                    self.select_name = false;
                }
                if self.name.chars().count() < 16 {
                    self.name.push(ch);
                }
            }
        }
        if input.tab {
            self.ctrl = if self.ctrl == crate::tank::Ctrl::Human {
                crate::tank::Ctrl::Ai(self.last_ai)
            } else {
                crate::tank::Ctrl::Human
            };
        }
        let change = input.left
            || input.right
            || input
                .click
                .is_some_and(|(x, y, _)| x > 300 && (168..168 + fields as i32 * 28).contains(&y));
        if change {
            let direction = if input.left || input.alt || input.click.is_some_and(|c| c.2 < 0) {
                -1
            } else {
                1
            };
            match self.row {
                1 => {
                    let i = CTRLS.iter().position(|c| *c == self.ctrl).unwrap();
                    self.ctrl =
                        CTRLS[(i as i32 + direction).rem_euclid(CTRLS.len() as i32) as usize];
                }
                2 if !reassign => self.shape = self.shape.cycle(self.ctrl, direction),
                3 if !reassign => {
                    self.team = (i32::from(self.team) + direction).rem_euclid(10) as u8
                }
                _ => {}
            }
        }
        if let crate::tank::Ctrl::Ai(kind) = self.ctrl {
            self.last_ai = kind;
        }
        if self.ctrl == crate::tank::Ctrl::Human && self.shape == crate::tank::Shape::Triple {
            self.shape = crate::tank::Shape::Treads;
        }
        if input.submit || button_index(input, 2) == Some(0) {
            if (self.ctrl == crate::tank::Ctrl::Human || reassign) && self.name.trim().is_empty() {
                self.error = Some("Enter a name before finishing.".into());
                return false;
            }
            if self.ctrl == crate::tank::Ctrl::Human || reassign {
                s.names[self.index] = self.name.trim().to_string();
            }
            s.controllers[self.index] = self.ctrl;
            if !reassign {
                s.shapes[self.index] = self.shape;
                s.team_ids[self.index] = self.team;
            }
            return true;
        }
        false
    }
    fn draw(&self, fb: &mut Fb, s: &Settings, reassign: bool) {
        fb.rect(0, 0, W, H, PLAYER_COLORS[self.index]);
        fb.bevel(80, 108, 480, 260);
        fb.text(100, 124, &format!("PLAYER {}", self.index + 1), Rgb::BLACK);
        let mut lines = vec![
            format!(
                "Name: {}",
                if self.ctrl == crate::tank::Ctrl::Human || reassign {
                    self.name.clone()
                } else {
                    "Computer chooses".into()
                }
            ),
            format!("Controller: {}", self.ctrl.label()),
        ];
        if !reassign {
            lines.push(format!(
                "Icon: {} ({})",
                self.shape.label(),
                if self.shape.mobile() {
                    "mobile"
                } else {
                    "fixed"
                }
            ));
            if s.play.teams != crate::settings::TeamMode::None {
                lines.push(format!("Team: {}", self.team + 1));
            }
        }
        for (row, text) in lines.iter().enumerate() {
            let y = 168 + row as i32 * 28;
            if row == self.row {
                fb.frame(96, y - 2, 448, 24, Rgb::DARK);
            }
            fb.text(104, y, text, Rgb::BLACK);
        }
        if !reassign {
            draw_tank_shape(
                fb,
                512,
                142,
                55,
                PLAYER_COLORS[self.index].scale(0.7),
                self.shape,
            );
        }
        fb.text(
            100,
            296,
            "Tab: human/computer  Arrows: select/change",
            Rgb::DARK,
        );
        fb.text(100, 320, "Enter: Done  Esc: cancel", Rgb::DARK);
        if let Some(error) = &self.error {
            fb.text(100, 344, error, Rgb(160, 0, 0));
        }
        mouse_buttons(fb, &["Done", "Cancel"]);
    }
}

#[derive(Default, Clone)]
pub(crate) struct MenuInput {
    pub(crate) raw_key: Option<KeyCode>,
    pub(crate) up: bool,
    pub(crate) down: bool,
    pub(crate) left: bool,
    pub(crate) right: bool,
    pub(crate) backspace: bool,
    pub(crate) home: bool,
    pub(crate) end: bool,
    pub(crate) tab: bool,
    pub(crate) page_up: bool,
    pub(crate) page_down: bool,
    pub(crate) activate: bool,
    pub(crate) submit: bool,
    pub(crate) text: String,
    pub(crate) escape: bool,
    pub(crate) hotkey: Option<char>,
    pub(crate) alt: bool,
    pub(crate) shift: bool,
    /// Framebuffer coordinates and direction (right mouse button = previous).
    pub(crate) click: Option<(i32, i32, i32)>,
}

impl MenuInput {
    pub(crate) fn poll(keys: &mut KeyRep) -> Self {
        let hotkey = get_last_key_pressed().and_then(|key| match key {
            KeyCode::Key0 => Some('0'),
            KeyCode::Key1 => Some('1'),
            KeyCode::Key2 => Some('2'),
            KeyCode::Key3 => Some('3'),
            KeyCode::Key4 => Some('4'),
            KeyCode::Key5 => Some('5'),
            KeyCode::Key6 => Some('6'),
            KeyCode::Key7 => Some('7'),
            KeyCode::Key8 => Some('8'),
            KeyCode::Key9 => Some('9'),
            KeyCode::A => Some('A'),
            KeyCode::B => Some('B'),
            KeyCode::C => Some('C'),
            KeyCode::D => Some('D'),
            KeyCode::E => Some('E'),
            KeyCode::F => Some('F'),
            KeyCode::G => Some('G'),
            KeyCode::H => Some('H'),
            KeyCode::I => Some('I'),
            KeyCode::J => Some('J'),
            KeyCode::K => Some('K'),
            KeyCode::L => Some('L'),
            KeyCode::M => Some('M'),
            KeyCode::N => Some('N'),
            KeyCode::O => Some('O'),
            KeyCode::P => Some('P'),
            KeyCode::Q => Some('Q'),
            KeyCode::R => Some('R'),
            KeyCode::S => Some('S'),
            KeyCode::T => Some('T'),
            KeyCode::U => Some('U'),
            KeyCode::V => Some('V'),
            KeyCode::W => Some('W'),
            KeyCode::X => Some('X'),
            KeyCode::Y => Some('Y'),
            KeyCode::Z => Some('Z'),
            KeyCode::Minus => Some('-'),
            _ => None,
        });
        let click = if is_mouse_button_pressed(MouseButton::Left) {
            Some(1)
        } else if is_mouse_button_pressed(MouseButton::Right) {
            Some(-1)
        } else {
            None
        };
        let (x, y) = mouse_position();
        let (x, y) = crate::gfx::Viewport::new(screen_width(), screen_height()).point(x, y);
        Self {
            raw_key: get_last_key_pressed(),
            up: keys.hit(KeyCode::Up),
            down: keys.hit(KeyCode::Down),
            left: keys.hit(KeyCode::Left),
            right: keys.hit(KeyCode::Right),
            backspace: is_key_pressed(KeyCode::Backspace),
            home: is_key_pressed(KeyCode::Home),
            end: is_key_pressed(KeyCode::End),
            tab: is_key_pressed(KeyCode::Tab),
            page_up: keys.hit(KeyCode::PageUp),
            page_down: keys.hit(KeyCode::PageDown),
            submit: is_key_pressed(KeyCode::Enter),
            text: std::iter::from_fn(get_char_pressed).collect(),
            activate: is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space),
            escape: is_key_pressed(KeyCode::Escape),
            hotkey,
            alt: is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt),
            shift: is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift),
            click: click.map(|direction| (x, y, direction)),
        }
    }
}

struct MenuRow {
    label: String,
    key: char,
    value: String,
    toggle: Option<bool>,
    /// None means implemented; Some(batch) means disabled until that batch.
    pending: Option<u8>,
}

impl MenuRow {
    fn counter(label: &str, key: char, value: impl ToString, pending: Option<u8>) -> Self {
        Self {
            label: label.into(),
            key,
            value: value.to_string(),
            toggle: None,
            pending,
        }
    }
    fn enabled(mut self) -> Self {
        self.pending = None;
        self
    }

    fn toggle(label: &str, key: char, value: bool, pending: u8) -> Self {
        Self {
            label: label.into(),
            key,
            value: String::new(),
            toggle: Some(value),
            pending: Some(pending),
        }
    }
}

fn file_label(path: &Option<std::path::PathBuf>, default: &str) -> String {
    path.as_ref()
        .map(|p| {
            p.file_name()
                .unwrap_or(p.as_os_str())
                .to_string_lossy()
                .chars()
                .take(20)
                .collect()
        })
        .unwrap_or_else(|| default.into())
}

fn rows(page: Page, s: &Settings) -> Vec<MenuRow> {
    use MenuRow as R;
    match page {
        Page::Sound => vec![
            R::toggle("Sound", 'S', s.sound.enabled, 20).enabled(),
            R::counter(
                "Flight Sounds",
                'F',
                match s.sound.flight {
                    crate::settings::FlightSound::Off => "Off",
                    crate::settings::FlightSound::Position => "POS",
                    crate::settings::FlightSound::Velocity => "VEL",
                },
                None,
            ),
        ],
        Page::Hardware => vec![
            R::counter("Firing Delay", 'F', s.hardware.firing_delay, None),
            R::counter("Falling Delay", 'A', s.hardware.falling_delay, None),
            R::toggle("Fullscreen", 'U', s.hardware.fullscreen, 21).enabled(),
        ],
        Page::Economics => vec![
            R::counter(
                "Interest Rate",
                'I',
                format!("{:.2}", s.economics.interest_rate),
                None,
            ),
            R::counter(
                "Cash at Start",
                'C',
                format!("${}", s.economics.cash_at_start),
                None,
            ),
            R::toggle("Computers Buy", 'B', s.economics.computers_buy, 6).enabled(),
            R::counter(
                "Scoring Mode",
                'S',
                format!("{:?}", s.economics.scoring),
                None,
            ),
            R::toggle("Free Market", 'F', s.economics.free_market, 7).enabled(),
        ],
        Page::Physics => vec![
            R::counter("Air Viscosity", 'A', s.physics.air_viscosity, None),
            R::counter("Gravity", 'G', format!("{:.2}", s.physics.gravity), None),
            R::counter(
                "Suspend Dirt",
                'S',
                format!("{}%", s.physics.suspend_dirt),
                None,
            ),
            R::toggle("Tanks Fall", 'T', s.physics.tanks_fall, 3).enabled(),
            R::counter("Borders Extend", 'B', s.physics.borders_extend, None),
            R::counter("Effect of Walls", 'W', s.physics.walls.label(), None),
        ],
        Page::Landscape => vec![
            R::counter("Sky", 'S', format!("{:?}", s.landscape.sky), None),
            R::counter("Max. Wind", 'W', s.landscape.max_wind, None),
            R::toggle("Changing Wind", 'C', s.landscape.changing_wind, 3).enabled(),
            R::toggle(
                "Hostile Environment",
                'H',
                s.landscape.hostile_environment,
                19,
            )
            .enabled(),
            R::counter("Bumpiness", 'B', s.landscape.bumpiness, None),
            R::counter("Slope", 'L', s.landscape.slope, None),
            R::toggle("Flatten Peaks", 'F', s.landscape.flatten_peaks, 19).enabled(),
            R::toggle("Random Land", 'R', s.landscape.random_land, 19).enabled(),
            R::counter(
                "Scanned Mountains",
                'M',
                format!("{}%", s.landscape.scanned_mountains_percent),
                None,
            ),
            R::toggle("Classic Terrain", 'T', s.landscape.classic_terrain, 19).enabled(),
            R::counter(
                "Mountains Folder",
                'D',
                file_label(&s.landscape.mountains_folder, "Not selected"),
                None,
            ),
        ],
        Page::Play => vec![
            R::counter("Mode", 'M', format!("{:?}", s.play.mode), None),
            R::counter("Play Order", 'O', format!("{:?}", s.play.order), None),
            R::counter("Teams", 'T', format!("{:?}", s.play.teams), None),
            R::toggle("Fast Computers", 'F', s.play.fast_computers, 17).enabled(),
            R::counter(
                "Talking Tanks",
                'A',
                format!("{:?}", s.play.talking_tanks),
                None,
            ),
            R::counter(
                "Talk Probability",
                'P',
                format!("{}%", s.play.talk_probability),
                None,
            ),
            R::toggle("Status Bar", 'S', s.play.status_bar, 8).enabled(),
            R::counter(
                "Attack File",
                'C',
                file_label(&s.play.attack_comments, "Built-in phrases"),
                None,
            ),
            R::counter(
                "Death File",
                'D',
                file_label(&s.play.death_comments, "Built-in phrases"),
                None,
            ),
        ],
        Page::Weapons => vec![
            R::counter("Arms Level", 'A', s.weapons.arms_level, None),
            R::counter("Bomb Icon", 'B', format!("{:?}", s.weapons.bomb_icon), None),
            R::toggle("Tunneling", 'T', s.weapons.tunneling, 5).enabled(),
            R::counter("Scale", 'S', format!("{:?}", s.weapons.scale), None),
            R::toggle("Trace Paths", 'P', s.weapons.trace_paths, 9).enabled(),
            R::toggle("Extra Dirt", 'E', s.weapons.extra_dirt, 10).enabled(),
            R::toggle("Useless Items", 'U', s.weapons.useless_items, 7).enabled(),
        ],
        Page::Players => (0..s.players)
            .map(|i| {
                R::counter(
                    &format!("{} {}", i + 1, s.names[i]),
                    char::from_digit(((i + 1) % 10) as u32, 10).unwrap(),
                    format!(
                        "{} / {}{}",
                        s.controllers[i].label(),
                        s.shapes[i].label(),
                        if s.play.teams == crate::settings::TeamMode::None {
                            String::new()
                        } else {
                            format!(" T{}", s.team_ids[i] + 1)
                        }
                    ),
                    None,
                )
            })
            .chain(std::iter::once(R::counter("Done", 'D', "Start game", None)))
            .collect(),
    }
}

const MAIN_KEYS: [char; 12] = ['S', 'P', 'R', 'O', 'H', 'E', 'Y', 'L', 'T', 'W', 'C', 'F'];
const MAIN_PAGES: [Option<Page>; 12] = [
    None,
    None,
    None,
    Some(Page::Sound),
    Some(Page::Hardware),
    Some(Page::Economics),
    Some(Page::Physics),
    Some(Page::Landscape),
    Some(Page::Play),
    Some(Page::Weapons),
    None,
    None,
];

fn cycle(value: i32, direction: i32, min: i32, max: i32) -> i32 {
    min + (value - min + direction).rem_euclid(max - min + 1)
}

fn change_row(s: &mut Settings, page: Page, row: usize, direction: i32) {
    match (page, row) {
        (Page::Sound, 0) => s.sound.enabled = !s.sound.enabled,
        (Page::Sound, 1) => {
            use crate::settings::FlightSound::{Off, Position, Velocity};
            let modes = [Off, Position, Velocity];
            let i = modes
                .iter()
                .position(|&mode| mode == s.sound.flight)
                .unwrap();
            s.sound.flight = modes[cycle(i as i32, direction, 0, 2) as usize];
        }
        (Page::Hardware, 2) => s.hardware.fullscreen = !s.hardware.fullscreen,
        (Page::Hardware, 0) => {
            s.hardware.firing_delay =
                cycle(s.hardware.firing_delay as i32, direction, 0, 500) as u16
        }
        (Page::Hardware, 1) => {
            s.hardware.falling_delay =
                cycle(s.hardware.falling_delay as i32, direction, 0, 1000) as u16
        }
        (Page::Economics, 0) => {
            s.economics.interest_rate = cycle(
                (s.economics.interest_rate * 100.0).round() as i32,
                direction,
                0,
                30,
            ) as f64
                / 100.0
        }
        (Page::Economics, 1) => {
            s.economics.cash_at_start = cycle(
                (s.economics.cash_at_start / 1000) as i32,
                direction,
                0,
                1000,
            ) as u32
                * 1000
        }
        (Page::Economics, 2) => s.economics.computers_buy = !s.economics.computers_buy,
        (Page::Economics, 3) => {
            use crate::settings::ScoringMode::{Basic, Greedy, Standard};
            let modes = [Basic, Standard, Greedy];
            let index = modes
                .iter()
                .position(|&mode| mode == s.economics.scoring)
                .unwrap();
            s.economics.scoring = modes[cycle(index as i32, direction, 0, 2) as usize];
        }
        (Page::Economics, 4) => s.economics.free_market = !s.economics.free_market,
        (Page::Weapons, 0) => {
            s.weapons.arms_level = cycle(s.weapons.arms_level as i32, direction, 0, 4) as u8
        }
        (Page::Weapons, 6) => s.weapons.useless_items = !s.weapons.useless_items,
        (Page::Physics, 0) => {
            s.physics.air_viscosity = cycle(s.physics.air_viscosity as i32, direction, 0, 20) as u8
        }
        (Page::Physics, 1) => {
            s.physics.gravity =
                cycle((s.physics.gravity * 20.0).round() as i32, direction, 1, 200) as f32 / 20.0
        }
        (Page::Physics, 2) => {
            s.physics.suspend_dirt = cycle(s.physics.suspend_dirt as i32, direction, 0, 100) as u8
        }
        (Page::Physics, 3) => s.physics.tanks_fall = !s.physics.tanks_fall,
        (Page::Physics, 4) => {
            s.physics.borders_extend =
                cycle(s.physics.borders_extend as i32, direction, 0, 10000) as u16
        }
        (Page::Physics, 5) => {
            use crate::settings::WallType::{
                Concrete, Erratic, None, Padded, Random, Rubber, Spring, WrapAround,
            };
            let modes = [
                None, WrapAround, Padded, Rubber, Spring, Concrete, Random, Erratic,
            ];
            let index = modes
                .iter()
                .position(|&mode| mode == s.physics.walls)
                .unwrap();
            s.physics.walls = modes[cycle(index as i32, direction, 0, 7) as usize];
        }
        (Page::Landscape, 0) => {
            use crate::settings::Sky::*;
            let modes = [Plain, Stormy, Stars, Shaded, Sunset, Cavern, Black, Random];
            let index = modes.iter().position(|&v| v == s.landscape.sky).unwrap();
            s.landscape.sky = modes[cycle(index as i32, direction, 0, 7) as usize];
        }
        (Page::Landscape, 3) => s.landscape.hostile_environment = !s.landscape.hostile_environment,
        (Page::Landscape, 4) => {
            s.landscape.bumpiness = cycle(s.landscape.bumpiness as i32, direction, 0, 100) as u8
        }
        (Page::Landscape, 5) => {
            s.landscape.slope = cycle(s.landscape.slope as i32, direction, 0, 100) as u8
        }
        (Page::Landscape, 6) => s.landscape.flatten_peaks = !s.landscape.flatten_peaks,
        (Page::Landscape, 7) => s.landscape.random_land = !s.landscape.random_land,
        (Page::Landscape, 8) => {
            s.landscape.scanned_mountains_percent = cycle(
                s.landscape.scanned_mountains_percent as i32,
                direction,
                0,
                100,
            ) as u8
        }
        (Page::Landscape, 9) => s.landscape.classic_terrain = !s.landscape.classic_terrain,
        (Page::Landscape, 1) => {
            s.landscape.max_wind = cycle(s.landscape.max_wind as i32, direction, 0, 500) as u16
        }
        (Page::Landscape, 2) => s.landscape.changing_wind = !s.landscape.changing_wind,
        (Page::Play, 0) => {
            use crate::settings::PlayMode::*;
            let modes = [Sequential, Synchronous, Simultaneous];
            let i = modes.iter().position(|m| *m == s.play.mode).unwrap();
            s.play.mode = modes[cycle(i as i32, direction, 0, 2) as usize];
        }
        (Page::Play, 1) => {
            let modes = [
                crate::settings::PlayOrder::Random,
                crate::settings::PlayOrder::LosersFirst,
                crate::settings::PlayOrder::WinnersFirst,
                crate::settings::PlayOrder::RoundRobin,
            ];
            let i = modes.iter().position(|m| *m == s.play.order).unwrap();
            s.play.order = modes[cycle(i as i32, direction, 0, 3) as usize];
        }
        (Page::Play, 2) => {
            let modes = [
                crate::settings::TeamMode::None,
                crate::settings::TeamMode::Standard,
                crate::settings::TeamMode::Corporate,
                crate::settings::TeamMode::Vicious,
            ];
            let i = modes.iter().position(|m| *m == s.play.teams).unwrap();
            s.play.teams = modes[cycle(i as i32, direction, 0, 3) as usize];
        }
        (Page::Play, 3) => s.play.fast_computers = !s.play.fast_computers,
        (Page::Play, 4) => {
            let modes = [
                crate::settings::TalkingTanks::Off,
                crate::settings::TalkingTanks::Computers,
                crate::settings::TalkingTanks::All,
            ];
            let i = modes
                .iter()
                .position(|m| *m == s.play.talking_tanks)
                .unwrap();
            s.play.talking_tanks = modes[cycle(i as i32, direction, 0, 2) as usize];
        }
        (Page::Play, 5) => {
            s.play.talk_probability =
                cycle(i32::from(s.play.talk_probability), direction, 0, 100) as u8
        }
        (Page::Play, 6) => s.play.status_bar = !s.play.status_bar,
        (Page::Weapons, 1) => {
            use crate::settings::BombIcon::{Big, Invisible, Small};
            let modes = [Invisible, Small, Big];
            let i = modes
                .iter()
                .position(|&v| v == s.weapons.bomb_icon)
                .unwrap();
            s.weapons.bomb_icon = modes[cycle(i as i32, direction, 0, 2) as usize];
        }
        (Page::Weapons, 3) => {
            use crate::settings::ExplosionScale::{Large, Medium, Normal};
            let modes = [Normal, Medium, Large];
            let i = modes.iter().position(|&v| v == s.weapons.scale).unwrap();
            s.weapons.scale = modes[cycle(i as i32, direction, 0, 2) as usize];
        }
        (Page::Weapons, 4) => s.weapons.trace_paths = !s.weapons.trace_paths,
        (Page::Weapons, 5) => s.weapons.extra_dirt = !s.weapons.extra_dirt,
        (Page::Weapons, 2) => s.weapons.tunneling = !s.weapons.tunneling,
        (Page::Players, i) if i < s.players => {
            let index = CTRLS
                .iter()
                .position(|ctrl| *ctrl == s.controllers[i])
                .unwrap();
            s.controllers[i] =
                CTRLS[cycle(index as i32, direction, 0, CTRLS.len() as i32 - 1) as usize];
            if s.controllers[i] == crate::tank::Ctrl::Human
                && s.shapes[i] == crate::tank::Shape::Triple
            {
                s.shapes[i] = crate::tank::Shape::Treads;
            }
        }
        _ => {}
    }
}

fn dialog_rect(page: Page, count: usize) -> (i32, i32, i32, i32) {
    let height = 72 + count as i32 * 24;
    let width = if page == Page::Players { 560 } else { 472 };
    ((W - width) / 2, (H - height) / 2, width, height)
}

impl MenuState {
    pub(crate) fn handle_with_click(
        &mut self,
        s: &mut Settings,
        input: &MenuInput,
    ) -> (MenuAction, bool) {
        let before = (
            self.selected,
            self.page,
            self.row,
            self.notice.clone(),
            self.folder_edit.clone(),
            self.player_edit.clone(),
            self.key_edit.clone(),
            s.clone(),
        );
        let action = self.handle(s, input);
        let clicked = before
            != (
                self.selected,
                self.page,
                self.row,
                self.notice.clone(),
                self.folder_edit.clone(),
                self.player_edit.clone(),
                self.key_edit.clone(),
                s.clone(),
            )
            || action != MenuAction::Stay;
        (action, clicked)
    }
    pub(crate) fn handle(&mut self, s: &mut Settings, input: &MenuInput) -> MenuAction {
        if let Some(mut editor) = self.key_edit.take() {
            if !editor.handle(s, input) {
                self.key_edit = Some(editor);
            }
            return MenuAction::Stay;
        }
        if let Some(mut editor) = self.player_edit.take() {
            if input.escape || button_index(input, 2) == Some(1) {
                return MenuAction::Stay;
            }
            if !editor.handle(s, input, self.reassign_only) {
                self.player_edit = Some(editor);
            }
            return MenuAction::Stay;
        }
        if self.notice.is_some() {
            if input.escape || input.activate || input.click.is_some() {
                self.notice = None;
            }
            return MenuAction::Stay;
        }
        if let Some(buffer) = &mut self.folder_edit {
            if input.escape || button_index(input, 2) == Some(1) {
                self.folder_edit = None;
                return MenuAction::Stay;
            }
            if input.backspace {
                buffer.pop();
            }
            for ch in input.text.chars().filter(|ch| !ch.is_control()) {
                if buffer.len() < 1024 {
                    buffer.push(ch);
                }
            }
            if input.submit || button_index(input, 2) == Some(0) {
                let value = buffer.trim();
                let path = if value.is_empty() {
                    None
                } else {
                    Some(value.into())
                };
                match self.file_field.unwrap_or(FileField::Mountains) {
                    FileField::Mountains => s.landscape.mountains_folder = path,
                    FileField::Attack => s.play.attack_comments = path,
                    FileField::Death => s.play.death_comments = path,
                }
                self.folder_edit = None;
            }
            return MenuAction::Stay;
        }
        if let Some(page) = self.page {
            let options = rows(page, s);
            self.row = self.row.min(options.len() - 1);
            let (x, y, w, h) = dialog_rect(page, options.len());
            if page == Page::Players
                && self.row < s.players
                && (input.hotkey == Some('K')
                    || input.click.is_some_and(|(mx, my, d)| {
                        d == 1
                            && (x + w - 76..x + w - 12).contains(&mx)
                            && (y + h - 30..y + h - 6).contains(&my)
                    }))
            {
                if s.controllers[self.row] == crate::tank::Ctrl::Human {
                    self.key_edit = Some(crate::modes::KeyEditor::new(s, self.row));
                } else {
                    self.notice = Some("Select a human player to assign keys.".into());
                }
                return MenuAction::Stay;
            }
            if input.escape
                || input
                    .click
                    .is_some_and(|(mx, my, _)| mx < x || mx >= x + w || my < y || my >= y + h)
            {
                self.page = None;
                return MenuAction::Stay;
            }
            if input.up {
                self.row = (self.row + options.len() - 1) % options.len();
            }
            if input.down {
                self.row = (self.row + 1) % options.len();
            }
            let mut direction = if input.alt { -1 } else { 1 };
            let mut change = input.activate;
            if input.left || input.page_down {
                direction = -1;
                change = true;
            }
            if input.right || input.page_up {
                direction = 1;
                change = true;
            }
            if let Some(key) = input.hotkey {
                // The original uses digits for tanks. Keyboard polling also maps them below.
                if let Some(index) = options.iter().position(|row| row.key == key) {
                    self.row = index;
                    change = true;
                }
            }
            if let Some((mx, my, d)) = input.click {
                if mx >= x + 12
                    && mx < x + w - 12
                    && my >= y + 32
                    && my < y + 32 + options.len() as i32 * 24
                {
                    self.row = ((my - y - 32) / 24) as usize;
                    direction = d;
                    change = true;
                }
            }
            if page == Page::Players
                && !self.reassign_only
                && self.row < s.players
                && (input.tab
                    || input.click.is_some_and(|(mx, my, _)| {
                        mx >= x + w - 116
                            && mx < x + w - 12
                            && my >= y + 32
                            && my < y + 32 + s.players as i32 * 24
                    }))
            {
                s.shapes[self.row] = s.shapes[self.row].cycle(s.controllers[self.row], direction);
                return MenuAction::Stay;
            }
            if page == Page::Players
                && self.row < s.players
                && (input.activate
                    || input.hotkey == Some('N')
                    || input.click.is_some_and(|(mx, my, _)| {
                        mx >= x + 12
                            && mx < x + 220
                            && my >= y + 32
                            && my < y + 32 + s.players as i32 * 24
                    }))
            {
                self.player_edit = Some(PlayerEdit::open(s, self.row));
                return MenuAction::Stay;
            }
            if change && options[self.row].pending.is_none() {
                if page == Page::Players && self.row == s.players {
                    self.page = None;
                    return MenuAction::Start;
                }
                if page == Page::Landscape && self.row == 10 {
                    self.file_field = Some(FileField::Mountains);
                    self.folder_edit = Some(
                        s.landscape
                            .mountains_folder
                            .as_ref()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    );
                    return MenuAction::Stay;
                }
                if page == Page::Play && matches!(self.row, 7 | 8) {
                    self.file_field = Some(if self.row == 7 {
                        FileField::Attack
                    } else {
                        FileField::Death
                    });
                    self.folder_edit = Some(
                        if self.row == 7 {
                            &s.play.attack_comments
                        } else {
                            &s.play.death_comments
                        }
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    );
                    return MenuAction::Stay;
                }
                let jump = if input.page_up || input.page_down {
                    10
                } else {
                    1
                };
                change_row(s, page, self.row, direction * jump);
            }
            return MenuAction::Stay;
        }
        if input.escape {
            return MenuAction::Quit;
        }
        if input.up {
            self.selected = (self.selected + MAIN_KEYS.len() - 1) % MAIN_KEYS.len();
        }
        if input.down {
            self.selected = (self.selected + 1) % MAIN_KEYS.len();
        }
        let mut direction = if input.alt { -1 } else { 1 };
        let mut change = input.activate;
        if input.left || input.page_down {
            direction = -1;
            change = true;
        }
        if input.right || input.page_up {
            direction = 1;
            change = true;
        }
        if let Some(key) = input.hotkey {
            if let Some(index) = MAIN_KEYS.iter().position(|&k| k == key) {
                self.selected = index;
                change = true;
            }
        }
        if let Some((x, y, d)) = input.click {
            if (12..232).contains(&x) && (44..428).contains(&y) && (y - 44) % 32 < 28 {
                self.selected = ((y - 44) / 32) as usize;
                direction = d;
                change = true;
            }
        }
        if change {
            match self.selected {
                0 => {
                    self.page = Some(Page::Players);
                    self.row = 0;
                }
                1 => s.players = cycle(s.players as i32, direction, 2, MAX_PLAYERS as i32) as usize,
                2 => {
                    s.rounds = cycle(
                        s.rounds as i32,
                        direction
                            * if input.page_up || input.page_down {
                                10
                            } else {
                                1
                            },
                        1,
                        1000,
                    ) as u32
                }
                10 => return MenuAction::Save,
                11 => return MenuAction::HallOfFame,
                i => {
                    self.page = MAIN_PAGES[i];
                    self.row = 0;
                }
            }
        }
        MenuAction::Stay
    }
}

pub(crate) fn update_menu(
    settings: &mut Settings,
    state: &mut MenuState,
    keys: &mut KeyRep,
) -> (MenuAction, bool) {
    state.handle_with_click(settings, &MenuInput::poll(keys))
}

fn hotkey_text(fb: &mut Fb, x: i32, y: i32, label: &str, key: char, color: Rgb, hot: Rgb) {
    let shortcut = label.chars().position(|ch| ch.to_ascii_uppercase() == key);
    for (index, ch) in label.chars().enumerate() {
        fb.text(
            x + index as i32 * 8,
            y,
            &ch.to_string(),
            if Some(index) == shortcut { hot } else { color },
        );
    }
    if let Some(index) = shortcut {
        fb.line(
            x + index as i32 * 8,
            y + 16,
            x + index as i32 * 8 + 6,
            y + 16,
            hot,
        );
    }
}

fn title_picture(fb: &mut Fb) {
    let (x, y, w, h) = (248, 44, 380, 320);
    for row in 0..h {
        fb.rect(
            x,
            y + row,
            w,
            1,
            Rgb::lerp(Rgb(0, 0, 40), Rgb(180, 65, 15), row as f32 / h as f32),
        );
    }
    // Original artwork for this tribute: mountains, a tank and an artillery burst.
    for col in 0..w {
        let top =
            220 + ((col as f32 * 0.025).sin() * 20.0 + (col as f32 * 0.07).sin() * 8.0) as i32;
        fb.rect(x + col, y + top, 1, h - top, Rgb(65, 42, 18));
    }
    for radius in (1..=42).rev() {
        for dy in -radius..=radius {
            let dx = ((radius * radius - dy * dy) as f32).sqrt() as i32;
            fb.rect(
                x + 280 - dx,
                y + 210 + dy,
                dx * 2 + 1,
                1,
                Rgb::lerp(Rgb(255, 250, 130), Rgb(190, 40, 0), radius as f32 / 42.0),
            );
        }
    }
    fb.rect(x + 58, y + 233, 66, 12, Rgb::BLACK);
    fb.rect(x + 65, y + 225, 52, 10, Rgb(80, 125, 45));
    fb.rect(x + 78, y + 216, 25, 12, Rgb(115, 165, 60));
    for offset in 0..4 {
        fb.line(
            x + 94,
            y + 218 + offset,
            x + 132,
            y + 196 + offset,
            Rgb(115, 165, 60),
        );
    }
    fb.text_scaled(x + 35, y + 26, "SCORCHED", Rgb::BLACK, 4, 2);
    fb.text_scaled(x + 32, y + 23, "SCORCHED", Rgb(255, 200, 45), 4, 2);
    fb.text_scaled(x + 111, y + 96, "EARTH", Rgb::BLACK, 4, 2);
    fb.text_scaled(x + 108, y + 93, "EARTH", Rgb(255, 120, 20), 4, 2);
    fb.text(x + 70, y + 282, "A RUST TRIBUTE", Rgb(255, 215, 130));
    fb.frame(x, y, w, h, Rgb::DARK);
}

pub(crate) fn draw_menu(fb: &mut Fb, s: &Settings, state: &MenuState) {
    if let Some(editor) = &state.player_edit {
        editor.draw(fb, s, state.reassign_only);
        return;
    }
    fb.rect(0, 0, W, H, Rgb(20, 20, 45));
    fb.text(16, 14, "MAIN MENU", Rgb::WHITE);
    title_picture(fb);
    let labels = [
        "Start".into(),
        format!("Players: {}", s.players),
        format!("Rounds: {}", s.rounds),
        "Sound...".into(),
        "Hardware...".into(),
        "Economics...".into(),
        "Physics...".into(),
        "Landscape...".into(),
        "Play Options...".into(),
        "Weapons...".into(),
        "Save Changes".into(),
        "Hall of Fame".into(),
    ];
    for (i, label) in labels.iter().enumerate() {
        let y = 44 + i as i32 * 32;
        fb.bevel(12, y, 220, 28);
        let selected = i == state.selected;
        if selected {
            fb.rect(16, y + 4, 212, 20, Rgb(0, 0, 140));
        }
        hotkey_text(
            fb,
            24,
            y + 6,
            label,
            MAIN_KEYS[i],
            if selected { Rgb::WHITE } else { Rgb::BLACK },
            if selected {
                Rgb(255, 255, 70)
            } else {
                Rgb(140, 0, 0)
            },
        );
    }
    fb.text(248, 380, "WENDELL HICKEN'S DOS CLASSIC", Rgb(180, 180, 200));
    fb.text(
        16,
        434,
        "Arrows select/change  Enter choose  Esc quit",
        Rgb::WHITE,
    );
    fb.text(
        16,
        458,
        "Hotkey: next   Alt+hotkey: previous",
        Rgb(160, 190, 230),
    );
    if let Some(page) = state.page {
        let options = rows(page, s);
        let (x, y, w, h) = dialog_rect(page, options.len());
        fb.bevel(x, y, w, h);
        fb.rect(x + 3, y + 3, w - 6, 24, Rgb(0, 0, 120));
        fb.text(x + 12, y + 6, page.title(), Rgb::WHITE);
        for (i, row) in options.iter().enumerate() {
            let ry = y + 32 + i as i32 * 24;
            let selected = i == state.row;
            if selected {
                fb.frame(x + 8, ry - 2, w - 16, 23, Rgb::DARK);
            }
            let color = if row.pending.is_some() {
                Rgb(110, 110, 110)
            } else {
                Rgb::BLACK
            };
            if page == Page::Players && i < s.players {
                draw_tank_shape(fb, x + 22, ry + 14, 50, PLAYER_COLORS[i], s.shapes[i]);
            }
            let label_x = if page == Page::Players {
                x + 32
            } else {
                x + 16
            };
            hotkey_text(fb, label_x, ry, &row.label, row.key, color, color);
            if let Some(checked) = row.toggle {
                fb.frame(x + w - 40, ry + 2, 12, 12, color);
                if checked {
                    fb.line(x + w - 38, ry + 8, x + w - 35, ry + 11, color);
                    fb.line(x + w - 35, ry + 11, x + w - 30, ry + 4, color);
                }
            } else {
                fb.text(x + w - 16 - text_w(&row.value), ry, &row.value, color);
            }
        }
        let footer = if options[state.row.min(options.len() - 1)].pending.is_some() {
            "Not available yet.  Esc closes".to_string()
        } else if page == Page::Players {
            "N:edit Tab:icon K:keys D:done".into()
        } else {
            "Arrows / hotkeys change; Esc closes".into()
        };
        fb.text(x + 12, y + h - 26, &footer, Rgb::DARK);
        if page == Page::Players {
            fb.bevel(x + w - 76, y + h - 30, 64, 24);
            fb.text(x + w - 60, y + h - 26, "Keys", Rgb::BLACK);
        }
    }
    if let Some(editor) = &state.key_edit {
        editor.draw(fb);
    }
    if let Some(buffer) = &state.folder_edit {
        fb.bevel(60, 180, 520, 120);
        fb.text(
            76,
            192,
            match state.file_field.unwrap_or(FileField::Mountains) {
                FileField::Mountains => "MOUNTAINS FOLDER",
                FileField::Attack => "ATTACK PHRASES FILE",
                FileField::Death => "DEATH PHRASES FILE",
            },
            Rgb(0, 0, 120),
        );
        let tail: String = buffer
            .chars()
            .rev()
            .take(58)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        fb.text(76, 220, &format!("{}|", tail), Rgb::BLACK);
        fb.text(
            76,
            252,
            "Enter: apply  Esc: cancel  Empty: disable",
            Rgb::DARK,
        );
        fb.text(76, 274, "Relative paths use the settings folder", Rgb::DARK);
        mouse_buttons(fb, &["Apply", "Cancel"]);
    }
    if let Some(notice) = &state.notice {
        let lines = wrap_text(notice, 60);
        let height = (lines.len() as i32 * 18 + 64).min(H - 24);
        let y = (H - height) / 2;
        fb.bevel(60, y, 520, height);
        fb.text(76, y + 10, "SETTINGS", Rgb(0, 0, 120));
        for (i, line) in lines.iter().take(((height - 64) / 18) as usize).enumerate() {
            fb.text(76, y + 34 + i as i32 * 18, line, Rgb::BLACK);
        }
        fb.text(
            76,
            y + height - 22,
            "Enter / Esc / click to close",
            Rgb::DARK,
        );
    }
}

pub(crate) fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    // Chunk long filenames as well as prose so errors always fit the dialog.
    for paragraph in text.lines() {
        let chars: Vec<_> = paragraph.chars().collect();
        if chars.is_empty() {
            lines.push(String::new());
        }
        for chunk in chars.chunks(width) {
            lines.push(chunk.iter().collect());
        }
    }
    lines
}

#[cfg(test)]
mod menu_tests {
    use super::*;

    #[test]
    fn player_setup_opens_the_key_editor_and_cancel_preserves_bindings() {
        let mut settings = Settings::default();
        let mut menu = MenuState {
            page: Some(Page::Players),
            ..Default::default()
        };
        menu.handle(
            &mut settings,
            &MenuInput {
                hotkey: Some('K'),
                ..Default::default()
            },
        );
        assert!(menu.key_edit.is_some());
        menu.handle(
            &mut settings,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
        );
        assert!(menu.key_edit.is_none());
        let (x, y, w, h) = dialog_rect(Page::Players, settings.players + 1);
        let (_, clicked) = menu.handle_with_click(
            &mut settings,
            &MenuInput {
                click: Some((x + w - 40, y + h - 20, 1)),
                ..Default::default()
            },
        );
        assert!(clicked && menu.key_edit.is_some());
        menu.handle(
            &mut settings,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
        );
        menu.row = 1;
        menu.handle(
            &mut settings,
            &MenuInput {
                hotkey: Some('K'),
                ..Default::default()
            },
        );
        assert!(menu.key_edit.is_none() && menu.notice.is_some());
    }

    fn key(key: char, alt: bool) -> MenuInput {
        MenuInput {
            hotkey: Some(key),
            alt,
            ..Default::default()
        }
    }

    #[test]
    fn main_hotkeys_cycle_bounds_and_open_every_option_dialog() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut settings, &key('P', true));
        assert_eq!(settings.players, 10);
        menu.handle(&mut settings, &key('P', false));
        assert_eq!(settings.players, 2);
        settings.rounds = 1;
        menu.handle(&mut settings, &key('R', true));
        assert_eq!(settings.rounds, 1000);
        menu.handle(&mut settings, &key('R', false));
        assert_eq!(settings.rounds, 1);
        for (key, page) in MAIN_KEYS
            .iter()
            .zip(MAIN_PAGES)
            .filter_map(|(key, page)| page.map(|p| (*key, p)))
        {
            menu.handle(&mut settings, &key_input(key));
            assert_eq!(menu.page, Some(page));
            menu.handle(
                &mut settings,
                &MenuInput {
                    escape: true,
                    ..Default::default()
                },
            );
            assert_eq!(menu.page, None);
        }
        assert_eq!(
            menu.handle(&mut settings, &key('C', false)),
            MenuAction::Save
        );
        assert_eq!(
            menu.handle(
                &mut settings,
                &MenuInput {
                    escape: true,
                    ..Default::default()
                }
            ),
            MenuAction::Quit
        );
    }

    fn key_input(ch: char) -> MenuInput {
        key(ch, false)
    }

    #[test]
    fn player_editor_types_names_toggles_controller_selects_icons_teams_and_cancels() {
        let mut settings = Settings::default();
        settings.play.teams = crate::settings::TeamMode::Corporate;
        let mut menu = MenuState {
            page: Some(Page::Players),
            ..Default::default()
        };
        menu.handle(
            &mut settings,
            &MenuInput {
                activate: true,
                submit: true,
                ..Default::default()
            },
        );
        assert!(menu.player_edit.is_some());
        menu.handle(
            &mut settings,
            &MenuInput {
                text: "Alice".into(),
                ..Default::default()
            },
        );
        assert_eq!(settings.names[0], "Player 1");
        menu.handle(
            &mut settings,
            &MenuInput {
                tab: true,
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                tab: true,
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                down: true,
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                down: true,
                right: true,
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                down: true,
                right: true,
                ..Default::default()
            },
        );
        let mut fb = Fb::new();
        draw_menu(&mut fb, &settings, &menu);
        assert_eq!(&fb.img.bytes[..3], &[255, 50, 50]);
        menu.handle(
            &mut settings,
            &MenuInput {
                submit: true,
                ..Default::default()
            },
        );
        assert!(menu.player_edit.is_none());
        assert_eq!(settings.names[0], "Alice");
        assert_eq!(settings.controllers[0], crate::tank::Ctrl::Human);
        assert_eq!(settings.shapes[0], crate::tank::Shape::Heavy);
        assert_eq!(settings.team_ids[0], 1);
        let before = settings.clone();
        menu.handle(
            &mut settings,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                text: "Discard".into(),
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
        );
        assert_eq!(settings, before);
    }
    #[test]
    fn play_options_phrase_paths_and_all_three_modes_are_active() {
        let mut settings = Settings::default();
        let mut menu = MenuState {
            page: Some(Page::Play),
            ..Default::default()
        };
        for (index, row) in rows(Page::Play, &settings).iter().enumerate() {
            assert!(row.pending.is_none(), "row {index}");
        }
        menu.handle(&mut settings, &key_input('O'));
        assert_eq!(settings.play.order, crate::settings::PlayOrder::LosersFirst);
        menu.handle(&mut settings, &key_input('T'));
        assert_eq!(settings.play.teams, crate::settings::TeamMode::Standard);
        menu.handle(&mut settings, &key_input('A'));
        assert_eq!(
            settings.play.talking_tanks,
            crate::settings::TalkingTanks::Computers
        );
        menu.handle(&mut settings, &key_input('P'));
        assert_eq!(settings.play.talk_probability, 0);
        menu.handle(&mut settings, &key_input('F'));
        assert!(settings.play.fast_computers);
        menu.handle(&mut settings, &key_input('C'));
        assert!(menu.folder_edit.is_some());
        menu.handle(
            &mut settings,
            &MenuInput {
                text: "my phrases.cfg".into(),
                ..Default::default()
            },
        );
        menu.handle(
            &mut settings,
            &MenuInput {
                submit: true,
                ..Default::default()
            },
        );
        assert_eq!(settings.play.attack_comments, Some("my phrases.cfg".into()));
        assert!(settings.landscape.mountains_folder.is_none());
        menu.handle(&mut settings, &key_input('D'));
        menu.handle(
            &mut settings,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
        );
        assert!(settings.play.death_comments.is_none());
        menu.handle(&mut settings, &key_input('M'));
        assert_eq!(settings.play.mode, crate::settings::PlayMode::Synchronous);
        menu.handle(&mut settings, &key_input('M'));
        assert_eq!(settings.play.mode, crate::settings::PlayMode::Simultaneous);
        menu.handle(&mut settings, &key('M', true));
        assert_eq!(settings.play.mode, crate::settings::PlayMode::Synchronous);
    }

    #[test]
    fn player_shapes_cycle_with_tab_and_mouse_and_triple_is_computer_only() {
        let mut s = Settings::default();
        let mut menu = MenuState {
            page: Some(Page::Players),
            ..Default::default()
        };
        for _ in 0..12 {
            menu.handle(
                &mut s,
                &MenuInput {
                    tab: true,
                    ..Default::default()
                },
            );
            assert_ne!(s.shapes[0], crate::tank::Shape::Triple);
        }
        let before = s.shapes[0];
        menu.handle(
            &mut s,
            &MenuInput {
                tab: true,
                ..Default::default()
            },
        );
        menu.handle(
            &mut s,
            &MenuInput {
                tab: true,
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(s.shapes[0], before);
        let (x, y, w, _) = dialog_rect(Page::Players, s.players + 1);
        menu.handle(
            &mut s,
            &MenuInput {
                click: Some((x + w - 60, y + 38, 1)),
                ..Default::default()
            },
        );
        assert_ne!(s.shapes[0], before);
        menu.row = 1;
        for _ in 0..7 {
            if s.shapes[1] == crate::tank::Shape::Triple {
                break;
            }
            menu.handle(
                &mut s,
                &MenuInput {
                    tab: true,
                    ..Default::default()
                },
            );
        }
        assert_eq!(s.shapes[1], crate::tank::Shape::Triple);
        while s.controllers[1] != crate::tank::Ctrl::Human {
            menu.handle(&mut s, &key('2', false));
        }
        assert_eq!(s.shapes[1], crate::tank::Shape::Treads);
        assert!(s.validate().is_ok());
    }

    #[test]
    fn hardware_counters_work_in_both_directions_and_disabled_options_cannot_change() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut settings, &key('H', false));
        menu.handle(&mut settings, &key('F', false));
        assert_eq!(settings.hardware.firing_delay, 101);
        menu.handle(&mut settings, &key('F', true));
        assert_eq!(settings.hardware.firing_delay, 100);
        settings.hardware.falling_delay = 0;
        menu.handle(&mut settings, &key('A', true));
        assert_eq!(settings.hardware.falling_delay, 1000);
        menu.handle(&mut settings, &key('A', false));
        assert_eq!(settings.hardware.falling_delay, 0);
        for page in [
            Page::Sound,
            Page::Economics,
            Page::Physics,
            Page::Landscape,
            Page::Play,
            Page::Weapons,
        ] {
            menu.page = Some(page);
            let before = settings.clone();
            for row in rows(page, &settings)
                .into_iter()
                .filter(|row| row.pending.is_some())
            {
                menu.handle(&mut settings, &key(row.key, false));
                menu.handle(&mut settings, &key(row.key, true));
            }
            assert_eq!(settings, before, "disabled options changed in {page:?}");
        }
    }

    #[test]
    fn physics_and_wind_options_are_enabled_and_cycle_their_ranges() {
        let mut s = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut s, &key('Y', false));
        menu.handle(&mut s, &key('G', false));
        assert_eq!(s.physics.gravity, 0.25);
        s.physics.gravity = 0.05;
        menu.handle(&mut s, &key('G', true));
        assert_eq!(s.physics.gravity, 10.0);
        menu.handle(&mut s, &key('G', false));
        assert_eq!(s.physics.gravity, 0.05);
        menu.handle(&mut s, &key('A', true));
        assert_eq!(s.physics.air_viscosity, 20);
        menu.handle(&mut s, &key('S', true));
        assert_eq!(s.physics.suspend_dirt, 100);
        s.physics.borders_extend = 0;
        menu.handle(&mut s, &key('B', true));
        assert_eq!(s.physics.borders_extend, 10000);
        menu.handle(&mut s, &key('T', false));
        assert!(!s.physics.tanks_fall);
        assert!(rows(Page::Physics, &s)
            .iter()
            .take(5)
            .all(|r| r.pending.is_none()));
        assert!(rows(Page::Physics, &s)[5].pending.is_none());
        menu.page = Some(Page::Landscape);
        s.landscape.max_wind = 0;
        menu.handle(&mut s, &key('W', true));
        assert_eq!(s.landscape.max_wind, 500);
        menu.handle(&mut s, &key('C', false));
        assert!(s.landscape.changing_wind);
        s.validate().unwrap();
    }

    #[test]
    fn tunneling_menu_toggle_is_active_and_defaults_on() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        assert!(settings.weapons.tunneling);
        menu.handle(&mut settings, &key('W', false));
        assert_eq!(menu.page, Some(Page::Weapons));
        assert!(rows(Page::Weapons, &settings)[2].pending.is_none());
        menu.handle(&mut settings, &key('T', false));
        assert!(!settings.weapons.tunneling);
        menu.handle(&mut settings, &key('T', true));
        assert!(settings.weapons.tunneling);
    }

    #[test]
    fn economics_controls_cycle_and_enable_free_market() {
        use crate::settings::ScoringMode;
        let mut s = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut s, &key('E', false));
        assert_eq!(menu.page, Some(Page::Economics));
        menu.handle(&mut s, &key('I', false));
        assert_eq!(s.economics.interest_rate, 0.06);
        s.economics.interest_rate = 0.30;
        menu.handle(&mut s, &key('I', false));
        assert_eq!(s.economics.interest_rate, 0.0);
        menu.handle(&mut s, &key('I', true));
        assert_eq!(s.economics.interest_rate, 0.30);
        menu.handle(&mut s, &key('C', true));
        assert_eq!(s.economics.cash_at_start, 1_000_000);
        menu.handle(&mut s, &key('C', false));
        assert_eq!(s.economics.cash_at_start, 0);
        menu.handle(&mut s, &key('B', false));
        assert!(!s.economics.computers_buy);
        menu.handle(&mut s, &key('S', false));
        assert_eq!(s.economics.scoring, ScoringMode::Greedy);
        menu.handle(&mut s, &key('S', true));
        assert_eq!(s.economics.scoring, ScoringMode::Standard);
        assert!(rows(Page::Economics, &s)
            .iter()
            .take(4)
            .all(|row| row.pending.is_none()));
        menu.handle(&mut s, &key('F', false));
        assert!(s.economics.free_market);
        s.validate().unwrap();
    }

    #[test]
    fn opening_shop_keyboard_and_mouse_buy_bundles_and_finish_without_extra_charges() {
        use crate::{
            economy::{Account, Money},
            inventory::Inventory,
            weapons::ItemId,
        };
        let mut tank = Tank {
            bank: None,
            ai_state: crate::ai::State::default(),
            shape: crate::tank::Shape::Treads,
            name: "Player 1".into(),
            color: Rgb::WHITE,
            ctrl: crate::tank::Ctrl::Human,
            account: Account::new(5000),
            team: None,
            x: 100,
            y: 100,
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
        };
        let prices = crate::economy::base_prices();
        let mut state = crate::shop::Shop::default();
        let settings = Settings::default();
        let mut market = crate::shop::Market::default();
        assert_eq!(
            crate::shop::offers(&tank, &prices, &settings, state.category)[0],
            ItemId::Missile
        );
        assert!(!handle_shop(
            &mut tank,
            &prices,
            &settings,
            &mut state,
            &mut market,
            &MenuInput {
                activate: true,
                ..Default::default()
            }
        ));
        assert_eq!(tank.account.cash, Money::from(3125));
        assert_eq!(tank.inventory.count(ItemId::Missile), 15);
        assert!(!handle_shop(
            &mut tank,
            &prices,
            &settings,
            &mut state,
            &mut market,
            &MenuInput {
                click: Some((70, 120, 1)),
                ..Default::default()
            }
        ));
        assert_eq!(tank.account.cash, Money::from(1250));
        assert_eq!(tank.inventory.count(ItemId::Missile), 20);
        let before = tank.account.clone();
        assert!(handle_shop(
            &mut tank,
            &prices,
            &settings,
            &mut state,
            &mut market,
            &MenuInput {
                escape: true,
                ..Default::default()
            }
        ));
        assert_eq!(tank.account, before);
        tank.account.cash = Money::default();
        assert!(!handle_shop(
            &mut tank,
            &prices,
            &settings,
            &mut state,
            &mut market,
            &MenuInput {
                activate: true,
                ..Default::default()
            }
        ));
    }

    #[test]
    fn walls_menu_cycles_in_manual_order_and_frame_colours_match_active_walls() {
        use crate::settings::WallType::*;
        let mut s = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut s, &key('Y', false));
        assert!(rows(Page::Physics, &s)[5].pending.is_none());
        for expected in [
            WrapAround, Padded, Rubber, Spring, Concrete, Random, Erratic, None,
        ] {
            menu.handle(&mut s, &key('W', false));
            assert_eq!(s.physics.walls, expected);
        }
        menu.handle(&mut s, &key('W', true));
        assert_eq!(s.physics.walls, Erratic);
        let text = toml::to_string(&s).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);
        for (wall, color) in [
            (None, Rgb::GRAY),
            (WrapAround, Rgb(255, 255, 0)),
            (Padded, Rgb(0, 255, 0)),
            (Rubber, Rgb(255, 0, 0)),
            (Spring, Rgb(0, 255, 255)),
            (Concrete, Rgb::WHITE),
        ] {
            let mut fb = Fb::new();
            draw_walls(&mut fb, wall, HUD_H + 22);
            for (x, y) in [(0, 200), (W - 1, 200), (100, H - 1)] {
                let offset = ((y * W + x) * 4) as usize;
                assert_eq!(
                    &fb.img.bytes[offset..offset + 3],
                    &[color.0, color.1, color.2]
                );
            }
            let offset = (((HUD_H + 22) * W + 100) * 4) as usize;
            let expected = if matches!(wall, None | WrapAround) {
                Rgb::BLACK
            } else {
                color
            };
            assert_eq!(
                &fb.img.bytes[offset..offset + 3],
                &[expected.0, expected.1, expected.2]
            );
        }
    }

    #[test]
    fn sound_menu_is_active_cycles_off_pos_vel_and_emits_clicks_only_on_actions() {
        use crate::settings::FlightSound;
        let mut s = Settings::default();
        let mut menu = MenuState::default();
        let (_, clicked) = menu.handle_with_click(&mut s, &MenuInput::default());
        assert!(!clicked);
        assert!(menu.handle_with_click(&mut s, &key('O', false)).1);
        assert_eq!(menu.page, Some(Page::Sound));
        assert!(rows(Page::Sound, &s)
            .iter()
            .all(|row| row.pending.is_none()));
        assert!(s.sound.enabled);
        assert_eq!(s.sound.flight, FlightSound::Off);
        menu.handle_with_click(&mut s, &key('F', false));
        assert_eq!(s.sound.flight, FlightSound::Position);
        assert_eq!(rows(Page::Sound, &s)[1].label, "Flight Sounds");
        menu.handle_with_click(&mut s, &key('F', false));
        assert_eq!(s.sound.flight, FlightSound::Velocity);
        menu.handle_with_click(&mut s, &key('F', false));
        assert_eq!(s.sound.flight, FlightSound::Off);
        menu.handle_with_click(&mut s, &key('F', true));
        assert_eq!(s.sound.flight, FlightSound::Velocity);
        assert!(menu.handle_with_click(&mut s, &key('S', false)).1);
        assert!(!s.sound.enabled);
        assert!(!menu.handle_with_click(&mut s, &MenuInput::default()).1);
        let text = toml::to_string(&s).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), s);
    }

    #[test]
    fn extra_dirt_menu_control_is_enabled_and_persists_with_weapon_options() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut settings, &key('W', false));
        assert!(rows(Page::Weapons, &settings)[5].pending.is_none());
        menu.handle(&mut settings, &key('E', false));
        assert!(settings.weapons.extra_dirt);
        let text = toml::to_string(&settings).unwrap();
        assert!(
            toml::from_str::<Settings>(&text)
                .unwrap()
                .weapons
                .extra_dirt
        );
        menu.handle(&mut settings, &key('E', true));
        assert!(!settings.weapons.extra_dirt);
    }

    #[test]
    fn weapon_display_options_are_enabled_cycle_both_directions_and_serialize() {
        use crate::settings::{BombIcon, ExplosionScale};
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut settings, &key('W', false));
        assert!([1, 3, 4]
            .into_iter()
            .all(|i| rows(Page::Weapons, &settings)[i].pending.is_none()));
        menu.handle(&mut settings, &key('B', false));
        assert_eq!(settings.weapons.bomb_icon, BombIcon::Invisible);
        menu.handle(&mut settings, &key('B', true));
        assert_eq!(settings.weapons.bomb_icon, BombIcon::Big);
        menu.handle(&mut settings, &key('S', true));
        assert_eq!(settings.weapons.scale, ExplosionScale::Large);
        menu.handle(&mut settings, &key('P', false));
        assert!(settings.weapons.trace_paths);
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
        settings.validate().unwrap();
    }

    #[test]
    fn shield_percentage_is_visible_in_the_main_hud_with_the_status_bar_disabled() {
        let mut tank = crate::shop::tests::tank(0);
        let mut without = Fb::new();
        draw_hud(&mut without, &tank, 0, false);
        tank.inventory.add(crate::weapons::ItemId::Shield, 1);
        tank.engage_shield(crate::weapons::ItemId::Shield);
        let mut with = Fb::new();
        draw_hud(&mut with, &tank, 0, false);
        let mut changed = 0;
        for y in 3..19 {
            for x in W - 40..W - 8 {
                let offset = ((y * W + x) * 4) as usize;
                changed += usize::from(
                    with.img.bytes[offset..offset + 3] != without.img.bytes[offset..offset + 3],
                );
            }
        }
        assert!(changed > 20);
    }

    #[test]
    fn bomb_icons_render_three_distinct_sizes_and_invisible_draws_nothing() {
        use crate::settings::BombIcon;
        use crate::weapons::{ItemId, PKind, Proj};
        let p = Proj {
            pos: macroquad::prelude::vec2(300.0, 200.0),
            vel: macroquad::prelude::vec2(0.0, 0.0),
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
        let mut counts = Vec::new();
        for icon in [BombIcon::Invisible, BombIcon::Small, BombIcon::Big] {
            let mut fb = Fb::new();
            let before = fb.img.bytes.clone();
            draw_projectile(&mut fb, &p, icon);
            counts.push(
                before
                    .chunks_exact(4)
                    .zip(fb.img.bytes.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count(),
            );
        }
        assert_eq!(counts[0], 0);
        assert_eq!(counts[1], 1);
        assert!(counts[2] > counts[1]);
    }

    #[test]
    fn shop_options_are_enabled_and_persist_in_owned_settings() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(&mut settings, &key('W', false));
        menu.handle(&mut settings, &key('A', false));
        assert_eq!(settings.weapons.arms_level, 0);
        menu.handle(&mut settings, &key('A', true));
        assert_eq!(settings.weapons.arms_level, 4);
        menu.handle(&mut settings, &key('U', false));
        assert!(settings.weapons.useless_items);
        menu.handle(
            &mut settings,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
        );
        menu.handle(&mut settings, &key('T', false));
        menu.handle(&mut settings, &key('S', false));
        assert!(settings.play.status_bar);
        settings.validate().unwrap();
    }

    #[test]
    fn shop_pages_categories_inventory_selling_and_status_use_separate_actions() {
        use crate::shop::{self, Market, Panel, Shop, PAGE_SIZE};
        use crate::weapons::{Category, ItemId};
        let mut tank = shop::tests::tank(1_000_000);
        tank.inventory.add(ItemId::Missile, 4);
        let prices = crate::economy::base_prices();
        let settings = Settings::default();
        let mut state = Shop::default();
        let mut market = Market::default();
        let mut act = |input: MenuInput, state: &mut Shop, tank: &mut Tank| {
            handle_shop(tank, &prices, &settings, state, &mut market, &input)
        };
        act(
            MenuInput {
                up: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        let last = shop::offers(&tank, &prices, &settings, Category::Weapon).len() - 1;
        assert_eq!(state.selected, last);
        act(
            MenuInput {
                down: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.selected, 0);
        act(
            MenuInput {
                page_down: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.selected, PAGE_SIZE);
        act(
            MenuInput {
                page_up: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.selected, 0);
        act(
            MenuInput {
                page_up: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.selected, last / PAGE_SIZE * PAGE_SIZE);
        act(
            MenuInput {
                end: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.selected, last);
        act(
            MenuInput {
                home: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.selected, 0);
        act(
            MenuInput {
                tab: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(state.category, Category::Miscellaneous);
        let accessory = shop::offers(&tank, &prices, &settings, state.category)[0];
        act(
            MenuInput {
                activate: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(
            tank.inventory.count(accessory),
            accessory.item().bundle_size
        );
        act(key('I', false), &mut state, &mut tank);
        assert_eq!(state.panel, Panel::Inventory);
        // Missile is first owned item. Rejecting a sale makes no changes.
        let before = tank.account.cash.clone();
        act(
            MenuInput {
                activate: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        assert_eq!(
            state.panel,
            Panel::Sell {
                item: ItemId::Missile,
                quantity: 1
            }
        );
        act(key('Q', false), &mut state, &mut tank);
        act(key('Q', true), &mut state, &mut tank);
        act(key('R', false), &mut state, &mut tank);
        assert_eq!(tank.account.cash, before);
        act(
            MenuInput {
                activate: true,
                ..Default::default()
            },
            &mut state,
            &mut tank,
        );
        for _ in 0..8 {
            act(
                MenuInput {
                    up: true,
                    ..Default::default()
                },
                &mut state,
                &mut tank,
            );
        }
        assert_eq!(
            state.panel,
            Panel::Sell {
                item: ItemId::Missile,
                quantity: 4
            }
        );
        act(key('A', false), &mut state, &mut tank);
        assert_eq!(tank.inventory.count(ItemId::Missile), 0);
        assert!(tank.account.cash > before);
        assert!(!act(
            MenuInput {
                escape: true,
                ..Default::default()
            },
            &mut state,
            &mut tank
        ));
        assert_eq!(state.panel, Panel::Buy);
        act(key('U', false), &mut state, &mut tank);
        assert_eq!(state.panel, Panel::Status);
        assert!(!act(
            MenuInput {
                escape: true,
                ..Default::default()
            },
            &mut state,
            &mut tank
        ));
        assert_eq!(state.panel, Panel::Buy);
        assert!(act(
            MenuInput {
                escape: true,
                ..Default::default()
            },
            &mut state,
            &mut tank
        ));
    }

    #[test]
    fn mouse_buttons_change_counters_and_outside_clicks_only_close_dialogs() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(
            &mut settings,
            &MenuInput {
                click: Some((30, 44 + 4 * 32 + 8, 1)),
                ..Default::default()
            },
        );
        assert_eq!(menu.page, Some(Page::Hardware));
        let (x, y, _, _) = dialog_rect(Page::Hardware, 2);
        menu.handle(
            &mut settings,
            &MenuInput {
                click: Some((x + 40, y + 38, 1)),
                ..Default::default()
            },
        );
        assert_eq!(settings.hardware.firing_delay, 101);
        menu.handle(
            &mut settings,
            &MenuInput {
                click: Some((x + 40, y + 38, -1)),
                ..Default::default()
            },
        );
        assert_eq!(settings.hardware.firing_delay, 100);
        assert_eq!(
            menu.handle(
                &mut settings,
                &MenuInput {
                    click: Some((30, 44, 1)),
                    ..Default::default()
                }
            ),
            MenuAction::Stay
        );
        assert_eq!(menu.page, None);
        assert_eq!(settings.players, 2);
    }

    #[test]
    fn ten_player_setup_edits_last_controller_and_starts_only_on_done() {
        let mut settings = Settings::default();
        settings.players = 10;
        let mut menu = MenuState::default();
        assert_eq!(
            menu.handle(&mut settings, &key('S', false)),
            MenuAction::Stay
        );
        assert_eq!(menu.page, Some(Page::Players));
        let before = settings.controllers[9];
        menu.handle(&mut settings, &key('0', false));
        assert_ne!(settings.controllers[9], before);
        menu.handle(&mut settings, &key('0', true));
        assert_eq!(settings.controllers[9], before);
        assert_eq!(
            menu.handle(&mut settings, &key('D', false)),
            MenuAction::Start
        );
        assert_eq!(menu.page, None);
    }

    #[test]
    fn notices_consume_dismissal_without_activating_the_menu_behind_them() {
        let mut settings = Settings::default();
        let mut menu = MenuState {
            notice: Some("Cannot save settings".into()),
            ..Default::default()
        };
        assert_eq!(
            menu.handle(
                &mut settings,
                &MenuInput {
                    activate: true,
                    ..Default::default()
                }
            ),
            MenuAction::Stay
        );
        assert!(menu.notice.is_none());
        assert!(menu.page.is_none());
    }

    #[test]
    fn menu_and_all_dialogs_fit_and_render() {
        let mut settings = Settings::default();
        settings.players = 10;
        let mut fb = Fb::new();
        let directory = std::env::var("SCORCHED_MENU_PREVIEW_DIR").ok();
        for page in [
            None,
            Some(Page::Sound),
            Some(Page::Hardware),
            Some(Page::Economics),
            Some(Page::Physics),
            Some(Page::Landscape),
            Some(Page::Play),
            Some(Page::Weapons),
            Some(Page::Players),
        ] {
            let menu = MenuState {
                page,
                ..Default::default()
            };
            if let Some(page) = page {
                let options = rows(page, &settings);
                let (x, y, w, h) = dialog_rect(page, options.len());
                assert!(x >= 0 && y >= 0 && x + w <= W && y + h <= H);
                let mut used = Vec::new();
                for row in options {
                    assert!(!used.contains(&row.key), "duplicate hotkey in {page:?}");
                    used.push(row.key);
                    assert!(
                        text_w(&row.label) + text_w(&row.value) + 64 < w,
                        "row overflows: {}",
                        row.label
                    );
                }
            }
            draw_menu(&mut fb, &settings, &menu);
            if let Some(directory) = &directory {
                let mut preview = fb.img.clone();
                preview.bytes = preview
                    .bytes
                    .chunks_exact((W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                preview.export_png(&format!("{directory}/menu-{page:?}.png"));
            }
        }
    }
}

pub(crate) fn draw_standings(
    fb: &mut Fb,
    tanks: &[Tank],
    prices: &[u32; crate::weapons::ITEM_COUNT],
    mode: crate::settings::ScoringMode,
    title: &str,
    message: &str,
) {
    use crate::economy::{self, money_text};
    use crate::settings::ScoringMode;
    let mut lines = vec![title.into()];
    if !message.is_empty() {
        lines.push(message.into());
    }
    lines.push(format!(
        "{:?} - ranked by {}",
        mode,
        if mode == crate::settings::ScoringMode::Greedy {
            "net worth"
        } else {
            "cash"
        }
    ));
    lines.push(String::new());
    if tanks.iter().any(|t| t.bank.is_some()) {
        let mut groups = Vec::new();
        let mut seen = Vec::new();
        for (i, tank) in tanks.iter().enumerate() {
            let key = tank.bank.map_or(100 + i, usize::from);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let mut worth = tank.account.cash.clone();
            if mode == ScoringMode::Greedy {
                for (_, member) in tanks
                    .iter()
                    .enumerate()
                    .filter(|(j, t)| t.bank.map_or(100 + *j, usize::from) == key)
                {
                    worth.add(&economy::inventory_value(&member.inventory, prices));
                }
            }
            let label = tank.bank.map_or_else(
                || tank.name.chars().take(12).collect::<String>(),
                |team| format!("Team {}", team + 1),
            );
            groups.push((label, tank.account.cash.clone(), worth));
        }
        groups.sort_by(|a, b| b.2.cmp(&a.2));
        for (position, (name, cash, worth)) in groups.into_iter().enumerate() {
            lines.push(format!(
                "{}. {} Bank {}{}",
                position + 1,
                name,
                money_text(&cash),
                if mode == ScoringMode::Greedy {
                    format!(" Worth {}", money_text(&worth))
                } else {
                    String::new()
                }
            ));
        }
        lines.push(String::new());
        lines.push("Click or press any key".into());
        dialog(fb, &lines);
        return;
    }
    for (rank, &i) in economy::ranking(tanks, mode, prices).iter().enumerate() {
        let tank = &tanks[i];
        let name: String = tank.team.map_or_else(
            || tank.name.chars().take(12).collect(),
            |team| {
                format!(
                    "{} T{}",
                    tank.name.chars().take(8).collect::<String>(),
                    team + 1
                )
            },
        );
        let mut line = format!(
            "{:2}. {:12} Cash {:>14}",
            rank + 1,
            name,
            money_text(&tank.account.cash)
        );
        if mode == crate::settings::ScoringMode::Greedy {
            line.push_str(&format!(
                " Worth {:>14}",
                money_text(&economy::net_worth(tank, prices))
            ));
        }
        lines.push(line);
    }
    lines.push(String::new());
    lines.push("Click or press any key".into());
    dialog(fb, &lines);
}

fn shop_items(
    tank: &Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    settings: &Settings,
    state: &crate::shop::Shop,
) -> Vec<crate::weapons::ItemId> {
    if state.panel == crate::shop::Panel::Inventory {
        crate::shop::inventory(tank)
    } else {
        crate::shop::offers(tank, prices, settings, state.category)
    }
}

fn handle_shop(
    tank: &mut Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    settings: &Settings,
    state: &mut crate::shop::Shop,
    market: &mut crate::shop::Market,
    input: &MenuInput,
) -> bool {
    use crate::shop::{self, Panel, PAGE_SIZE};
    use crate::weapons::Category;
    // Translate the mouse toolbar into the same actions used by the keyboard.
    let mut mouse_input;
    let input = if let Some(button) = button_index(
        input,
        if matches!(state.panel, Panel::Sell { .. }) {
            4
        } else {
            5
        },
    ) {
        mouse_input = input.clone();
        mouse_input.click = None;
        if matches!(state.panel, Panel::Sell { .. }) {
            match button {
                0 => mouse_input.down = true,
                1 => mouse_input.up = true,
                2 => mouse_input.hotkey = Some('A'),
                _ => mouse_input.hotkey = Some('R'),
            }
        } else {
            match button {
                0 => mouse_input.page_up = true,
                1 => mouse_input.page_down = true,
                2 => mouse_input.tab = true,
                3 => mouse_input.hotkey = Some('I'),
                _ => mouse_input.hotkey = Some('D'),
            }
        }
        &mouse_input
    } else {
        input
    };
    match state.panel {
        Panel::Status => {
            if input.escape || input.activate || input.hotkey.is_some() || input.click.is_some() {
                state.panel = Panel::Buy;
            }
            return false;
        }
        Panel::Sell { item, mut quantity } => {
            if input.escape || input.hotkey == Some('R') {
                state.panel = Panel::Inventory;
            } else if input.hotkey == Some('A') {
                shop::sell(tank, item, quantity, prices, settings.economics.free_market);
                state.panel = Panel::Inventory;
            } else {
                if input.up || (input.hotkey == Some('Q') && !input.alt) {
                    quantity = quantity.saturating_add(1);
                }
                if input.down || (input.hotkey == Some('Q') && input.alt) {
                    quantity = quantity.saturating_sub(1);
                }
                quantity = quantity.clamp(1, tank.inventory.count(item).max(1));
                state.panel = Panel::Sell { item, quantity };
            }
            return false;
        }
        _ => {}
    }
    if input.escape || input.hotkey == Some('D') {
        if state.panel == Panel::Inventory {
            state.panel = Panel::Buy;
            state.selected = 0;
            return false;
        }
        return true;
    }
    if state.panel == Panel::Buy && input.hotkey == Some('U') {
        state.panel = Panel::Status;
        return false;
    }
    if input.hotkey == Some('I') {
        state.panel = if state.panel == Panel::Buy {
            Panel::Inventory
        } else {
            Panel::Buy
        };
        state.selected = 0;
        return false;
    }
    if input.tab && state.panel == Panel::Buy {
        state.category = if state.category == Category::Weapon {
            Category::Miscellaneous
        } else {
            Category::Weapon
        };
        state.selected = 0;
    }
    let items = shop_items(tank, prices, settings, state);
    if items.is_empty() {
        state.selected = 0;
    } else {
        state.selected = state.selected.min(items.len() - 1);
        if input.up {
            state.selected = (state.selected + items.len() - 1) % items.len();
        }
        if input.down {
            state.selected = (state.selected + 1) % items.len();
        }
        if input.home {
            state.selected = 0;
        }
        if input.end {
            state.selected = items.len() - 1;
        }
        let pages = items.len().div_ceil(PAGE_SIZE);
        if input.page_down || input.page_up {
            let page = state.selected / PAGE_SIZE;
            let next = (page + if input.page_down { 1 } else { pages - 1 }) % pages;
            state.selected = (next * PAGE_SIZE + state.selected % PAGE_SIZE).min(items.len() - 1);
        }
    }
    let mut activate = input.activate;
    if let Some((x, y, direction)) = input.click {
        if direction > 0 {
            if (536..608).contains(&x) && (70..94).contains(&y) {
                if state.panel == Panel::Buy {
                    return true;
                }
                state.panel = Panel::Buy;
                state.selected = 0;
                return false;
            }
            let index = state.selected / PAGE_SIZE * PAGE_SIZE + ((y - 114).max(0) / 20) as usize;
            if (40..600).contains(&x) && (114..354).contains(&y) && index < items.len() {
                state.selected = index;
                activate = true;
            }
        }
    }
    if activate {
        if let Some(&id) = items.get(state.selected) {
            if state.panel == Panel::Buy {
                shop::buy(tank, id, prices, settings, market);
            } else {
                state.panel = Panel::Sell {
                    item: id,
                    quantity: 1,
                };
            }
        }
        let count = shop_items(tank, prices, settings, state).len();
        state.selected = state.selected.min(count.saturating_sub(1));
    }
    false
}

pub(crate) fn update_shop(
    tank: &mut Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    settings: &Settings,
    state: &mut crate::shop::Shop,
    market: &mut crate::shop::Market,
    keys: &mut KeyRep,
) -> bool {
    handle_shop(
        tank,
        prices,
        settings,
        state,
        market,
        &MenuInput::poll(keys),
    )
}

pub(crate) fn draw_shop(
    fb: &mut Fb,
    tank: &Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    settings: &Settings,
    state: &crate::shop::Shop,
    round: u32,
) {
    use crate::economy::money_text;
    use crate::shop::{self, Panel, PAGE_SIZE};
    fb.rect(0, 0, W, H, Rgb(20, 20, 45));
    fb.bevel(24, 16, 592, 450);
    fb.text(
        40,
        28,
        &format!("BUYING BEFORE ROUND {round} / {}", settings.rounds),
        Rgb::BLACK,
    );
    fb.text(40, 52, &tank.name, tank.color.scale(0.6));
    fb.text(
        320,
        52,
        &format!(
            "{}: {}",
            if tank.bank.is_some() {
                "Team bank"
            } else {
                "Cash"
            },
            money_text(&tank.account.cash)
        ),
        Rgb::BLACK,
    );
    let inventory = matches!(state.panel, Panel::Inventory | Panel::Sell { .. });
    let title = if inventory {
        "INVENTORY - select to sell"
    } else {
        match state.category {
            crate::weapons::Category::Weapon => "WEAPONS",
            _ => "MISCELLANEOUS",
        }
    };
    fb.text(40, 78, title, Rgb::BLACK);
    fb.text(544, 78, "Done", Rgb::BLACK);
    fb.text(72, 98, "Item", Rgb::DARK);
    fb.text(296, 98, "Qty", Rgb::DARK);
    fb.text(
        360,
        98,
        if inventory { "Offer" } else { "Price" },
        Rgb::DARK,
    );
    fb.text(520, 98, "Owned", Rgb::DARK);
    let mut list_state = crate::shop::Shop::default();
    list_state.selected = state.selected;
    list_state.category = state.category;
    list_state.panel = if matches!(state.panel, Panel::Inventory | Panel::Sell { .. }) {
        Panel::Inventory
    } else {
        Panel::Buy
    };
    let items = shop_items(tank, prices, settings, &list_state);
    let start = state.selected / PAGE_SIZE * PAGE_SIZE;
    for (i, &id) in items.iter().enumerate().skip(start).take(PAGE_SIZE) {
        let y = 116 + (i - start) as i32 * 20;
        let selected = i == state.selected;
        if selected {
            fb.rect(36, y - 2, 568, 20, Rgb(0, 0, 140));
        }
        let color = if selected { Rgb::WHITE } else { Rgb::BLACK };
        // Distinct weapon and supply icons, coloured by the item's arms level.
        let ink = [
            Rgb(60, 100, 40),
            Rgb(40, 80, 160),
            Rgb(140, 70, 10),
            Rgb(140, 30, 90),
            Rgb(160, 20, 20),
        ][id.item().arms_level as usize];
        if id.item().category == crate::weapons::Category::Weapon {
            fb.rect(48, y + 4, 10, 9, ink);
            fb.line(52, y + 4, 58, y, ink);
        } else {
            fb.rect(46, y + 1, 14, 14, ink);
            fb.line(49, y + 7, 57, y + 7, Rgb::WHITE);
            fb.line(53, y + 3, 53, y + 11, Rgb::WHITE);
        }
        fb.text(72, y, id.item().name, color);
        let (quantity, value) = if inventory {
            (
                tank.inventory.count(id),
                shop::sell_offer(
                    id,
                    tank.inventory.count(id),
                    prices,
                    settings.economics.free_market,
                ),
            )
        } else {
            let (q, cost) = shop::purchase(tank, id, prices).unwrap();
            (q, cost)
        };
        fb.text(296, y, &format!("x{quantity}"), color);
        fb.text(360, y, &money_text(&value), color);
        fb.text(536, y, &tank.inventory.count(id).to_string(), color);
    }
    if items.is_empty() {
        fb.text(
            48,
            140,
            if inventory {
                "No sellable items."
            } else {
                "No affordable items in this category."
            },
            Rgb::DARK,
        );
    }
    let pages = items.len().div_ceil(PAGE_SIZE).max(1);
    fb.text(
        40,
        360,
        &format!(
            "Page {}/{}   Arms: {}   Market: {}",
            start / PAGE_SIZE + 1,
            pages,
            settings.weapons.arms_level,
            if settings.economics.free_market {
                "Free"
            } else {
                "Fixed"
            }
        ),
        Rgb::DARK,
    );
    if let Some(&id) = items.get(state.selected) {
        let future = !id.item().is_usable_weapon()
            && !matches!(
                id,
                crate::weapons::ItemId::Battery
                    | crate::weapons::ItemId::Parachute
                    | crate::weapons::ItemId::AutoDefense
                    | crate::weapons::ItemId::MagDeflector
                    | crate::weapons::ItemId::Shield
                    | crate::weapons::ItemId::ForceShield
                    | crate::weapons::ItemId::HeavyShield
                    | crate::weapons::ItemId::SuperMag
                    | crate::weapons::ItemId::HeatGuidance
                    | crate::weapons::ItemId::BallisticGuidance
                    | crate::weapons::ItemId::HorizontalGuidance
                    | crate::weapons::ItemId::VerticalGuidance
                    | crate::weapons::ItemId::LazyBoy
                    | crate::weapons::ItemId::ContactTrigger
                    | crate::weapons::ItemId::FuelTank
            );
        let detail = if id == crate::weapons::ItemId::AutoDefense {
            "Buy once: defensive setup before every round."
        } else if future {
            "Stored in inventory; effect in a later batch."
        } else if !inventory
            && shop::purchase(tank, id, prices).is_some_and(|(q, _)| q < id.item().bundle_size)
        {
            "Partial bundle: remaining slots, 20% unit markup."
        } else {
            "Stock carries between rounds. Maximum: 99."
        };
        fb.text(40, 382, detail, Rgb::DARK);
    }
    mouse_buttons(fb, &["Prev", "Next", "Category", "Inventory", "Done"]);
    fb.text(
        40,
        426,
        if inventory {
            "Space: sell  I/Esc: back to buying"
        } else {
            "Tab: category  Space: buy  I: inventory"
        },
        Rgb::BLACK,
    );
    fb.text(40, 444, "U: update/status  D/Esc: done", Rgb::BLACK);
    match state.panel {
        Panel::Status => dialog(
            fb,
            &[
                format!("{} - Update", tank.name),
                format!("Rounds left: {}", settings.rounds.saturating_sub(round) + 1),
                format!(
                    "Interest earned: {}",
                    money_text(&tank.account.interest_earned)
                ),
                format!(
                    "{}: {}",
                    if tank.bank.is_some() {
                        "Team bank"
                    } else {
                        "Cash"
                    },
                    money_text(&tank.account.cash)
                ),
                "Enter / Esc: close".into(),
            ],
        ),
        Panel::Sell { item, quantity } => dialog(
            fb,
            &[
                format!("Sell {}", item.item().name),
                format!("Quantity: {quantity} / {}", tank.inventory.count(item)),
                format!(
                    "Offer: {}",
                    money_text(&shop::sell_offer(
                        item,
                        quantity,
                        prices,
                        settings.economics.free_market
                    ))
                ),
                "Q / Up: more   Alt-Q / Down: less".into(),
                "A: accept   R / Esc: reject".into(),
            ],
        ),
        _ => {}
    }
    if matches!(state.panel, Panel::Sell { .. }) {
        mouse_buttons(fb, &["-", "+", "Sell", "Cancel"]);
    }
}

#[cfg(test)]
mod landscape_tests {
    use super::*;
    #[test]
    fn all_landscape_options_work_and_folder_editor_accepts_spaces_cancel_and_clear() {
        let mut s = Settings::default();
        let mut menu = MenuState {
            page: Some(Page::Landscape),
            ..MenuState::default()
        };
        assert!(rows(Page::Landscape, &s)
            .iter()
            .all(|r| r.pending.is_none()));
        for row in [0, 3, 4, 5, 6, 7, 8, 9] {
            let before = s.clone();
            menu.row = row;
            menu.handle(
                &mut s,
                &MenuInput {
                    right: true,
                    ..MenuInput::default()
                },
            );
            assert_ne!(s, before);
        }
        s.landscape.bumpiness = 100;
        menu.row = 4;
        menu.handle(
            &mut s,
            &MenuInput {
                right: true,
                ..MenuInput::default()
            },
        );
        assert_eq!(s.landscape.bumpiness, 0);
        menu.row = 10;
        menu.handle(
            &mut s,
            &MenuInput {
                activate: true,
                ..MenuInput::default()
            },
        );
        assert!(menu.folder_edit.is_some());
        menu.handle(
            &mut s,
            &MenuInput {
                text: "my mountains!".into(),
                ..MenuInput::default()
            },
        );
        menu.handle(
            &mut s,
            &MenuInput {
                backspace: true,
                ..MenuInput::default()
            },
        );
        menu.handle(
            &mut s,
            &MenuInput {
                submit: true,
                ..MenuInput::default()
            },
        );
        assert_eq!(s.landscape.mountains_folder, Some("my mountains".into()));
        menu.handle(
            &mut s,
            &MenuInput {
                activate: true,
                ..MenuInput::default()
            },
        );
        menu.handle(
            &mut s,
            &MenuInput {
                text: "cancel".into(),
                escape: true,
                ..MenuInput::default()
            },
        );
        assert_eq!(s.landscape.mountains_folder, Some("my mountains".into()));
        menu.folder_edit = Some(String::new());
        menu.handle(
            &mut s,
            &MenuInput {
                submit: true,
                ..MenuInput::default()
            },
        );
        assert_eq!(s.landscape.mountains_folder, None);
    }
}

#[cfg(test)]
mod presentation_input_tests {
    use super::*;
    fn click(x: i32, y: i32) -> MenuInput {
        MenuInput {
            click: Some((x, y, 1)),
            ..Default::default()
        }
    }
    #[test]
    fn mouse_player_editor_apply_cancel_and_fullscreen_round_trip() {
        let mut settings = Settings::default();
        let mut state = MenuState {
            player_edit: Some(PlayerEdit::open(&settings, 0)),
            ..Default::default()
        };
        state.handle(&mut settings, &click(500, 202)); // Controller forward.
        state.handle(&mut settings, &click(100, 410)); // Done.
        assert!(state.player_edit.is_none());
        assert_ne!(settings.controllers[0], crate::tank::Ctrl::Human);
        let before = settings.clone();
        state.player_edit = Some(PlayerEdit::open(&settings, 0));
        state.handle(&mut settings, &click(500, 202));
        state.handle(&mut settings, &click(500, 410)); // Cancel.
        assert_eq!(settings, before);
        change_row(&mut settings, Page::Hardware, 2, 1);
        assert!(settings.hardware.fullscreen);
        let saved = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&saved).unwrap(), settings);
    }

    #[test]
    fn fire_chord_waits_for_both_buttons_and_never_commits_a_single_click() {
        for (left, right) in [(true, false), (false, true)] {
            let mut mouse = BattleMouse::default();
            assert!(matches!(
                mouse.update(left, right, (240, 10), 0.02),
                BattleClick::None
            ));
            assert!(matches!(
                mouse.update(true, true, (240, 10), 0.02),
                BattleClick::Fire
            ));
            assert!(matches!(
                mouse.update(true, true, (240, 10), 0.5),
                BattleClick::None
            ));
            assert!(matches!(
                mouse.update(left, right, (240, 10), 0.02),
                BattleClick::None
            ));
            assert!(matches!(
                mouse.update(false, false, (240, 10), 0.02),
                BattleClick::None
            ));
            assert!(matches!(
                mouse.update(true, true, (240, 10), 0.02),
                BattleClick::Fire
            ));
        }
        let mut mouse = BattleMouse::default();
        assert!(matches!(
            mouse.update(true, true, (-10, 10), 0.02),
            BattleClick::None
        ));
    }
    #[test]
    fn single_clicks_allow_small_motion_and_held_fields_repeat_without_release_step() {
        let mut mouse = BattleMouse::default();
        mouse.update(false, true, (250, 10), 0.02);
        assert!(matches!(
            mouse.update(false, false, (252, 12), 0.02),
            BattleClick::Click(250, 10, -1)
        ));
        mouse.update(true, false, (20, 10), 0.1);
        let mut steps = 0;
        for _ in 0..9 {
            if let BattleClick::Click(_, _, n) = mouse.update(true, false, (20, 10), 0.1) {
                steps += n;
            }
        }
        assert!((68..=70).contains(&steps), "{steps}");
        assert!(matches!(
            mouse.update(false, false, (20, 10), 0.02),
            BattleClick::None
        ));
        assert_eq!(button_index(&click(-100, 405), 4), None);
        assert_eq!(button_index(&click(559, 425), 4), Some(3));
        assert_eq!(button_index(&click(560, 425), 4), None);
    }
    #[test]
    fn shop_toolbar_reaches_supplies_all_pages_and_sell_confirmation_with_mouse() {
        use crate::{
            shop::{Market, Panel, Shop},
            weapons::{Category, ItemId, ITEMS},
        };
        let mut tank = crate::shop::tests::tank(0);
        tank.account = crate::economy::Account::new(1_000_000);
        let settings = Settings::default();
        let prices = crate::economy::base_prices();
        let mut market = Market::default();
        let mut shop = Shop::default();
        let mut act = |input: MenuInput, shop: &mut Shop, tank: &mut Tank| {
            handle_shop(tank, &prices, &settings, shop, &mut market, &input)
        };
        act(click(224, 410), &mut shop, &mut tank); // Next page.
        assert!(shop.selected > 0);
        act(click(128, 410), &mut shop, &mut tank);
        assert_eq!(shop.selected, 0);
        act(click(320, 410), &mut shop, &mut tank);
        assert_eq!(shop.category, Category::Miscellaneous);
        act(click(160, 118), &mut shop, &mut tank);
        assert!(ITEMS
            .iter()
            .any(|i| i.category == Category::Miscellaneous && tank.inventory.count(i.id) > 0));
        tank.inventory.add(ItemId::Missile, 3);
        act(click(416, 410), &mut shop, &mut tank);
        assert!(shop.panel == Panel::Inventory);
        let items = shop_items(&tank, &prices, &settings, &shop);
        let row = items.iter().position(|id| *id == ItemId::Missile).unwrap();
        act(click(160, 114 + row as i32 * 20), &mut shop, &mut tank);
        assert!(matches!(shop.panel, Panel::Sell { .. }));
        act(click(260, 410), &mut shop, &mut tank); // Quantity +.
        act(click(380, 410), &mut shop, &mut tank); // Sell two.
        assert_eq!(tank.inventory.count(ItemId::Missile), 1);
        act(click(512, 410), &mut shop, &mut tank); // Back to buying.
        assert!(shop.panel == Panel::Buy);
        assert!(act(click(512, 410), &mut shop, &mut tank));
    }
}
