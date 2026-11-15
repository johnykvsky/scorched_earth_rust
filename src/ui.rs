//! Menus, HUD, dialogs and software drawing for game objects.
use crate::gfx::{text_w, Fb, Rgb, H, W};
use crate::settings::{Settings, MAX_PLAYERS};
use crate::tank::{Tank, CTRLS, PLAYER_COLORS};
use crate::weapons::Explosion;
use macroquad::prelude::{
    get_last_key_pressed, is_key_down, is_key_pressed, is_mouse_button_pressed, mouse_position,
    screen_height, screen_width, KeyCode, MouseButton,
};

pub(crate) const HUD_H: i32 = 22;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MenuAction {
    Stay,
    Start,
    Save,
    Quit,
}

/// Key auto-repeat for held keys (call `hit` once per frame per key).
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

pub(crate) fn draw_hud(fb: &mut Fb, t: &Tank, wind: i32) {
    fb.bevel(0, 0, W, HUD_H);
    fb.text(8, 3, &format!("Power: {:4}", t.power), Rgb::BLACK);
    fb.text(112, 3, &format!("Angle: {:2}", display_angle(t.angle)), Rgb::BLACK);
    fb.rect(208, 5, 8, 12, t.color);
    fb.text(224, 3, &t.name.chars().take(14).collect::<String>(), Rgb::BLACK);

    // Small bomb icon, followed by ammunition and the selected weapon.
    fb.rect(352, 8, 8, 8, Rgb::BLACK);
    fb.rect(354, 6, 4, 2, Rgb::BLACK);
    fb.line(356, 6, 360, 3, Rgb::BLACK);
    let ammo = t.inventory.count(t.weapon);
    fb.text(368, 3, &format!("{:2} {}", ammo, t.weapon.item().name), Rgb::BLACK);

    let wind = match wind {
        0 => "No Wind".to_string(),
        v if v > 0 => format!("{} -->", v),
        v => format!("<-- {}", -v),
    };
    fb.text(W - text_w(&wind) - 8, HUD_H + 6, &wind, Rgb(100, 200, 255));
}

pub(crate) fn draw_tank(fb: &mut Fb, t: &Tank) {
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

pub(crate) fn draw_explosion(fb: &mut Fb, e: &Explosion) {
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
            Self::Players => "Player Controllers",
        }
    }
}

#[derive(Default)]
pub(crate) struct MenuState {
    pub(crate) selected: usize,
    pub(crate) page: Option<Page>,
    pub(crate) row: usize,
    pub(crate) notice: Option<String>,
}

#[derive(Default)]
pub(crate) struct MenuInput {
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    page_up: bool,
    page_down: bool,
    activate: bool,
    escape: bool,
    hotkey: Option<char>,
    alt: bool,
    /// Framebuffer coordinates and direction (right mouse button = previous).
    click: Option<(i32, i32, i32)>,
}

impl MenuInput {
    fn poll(keys: &mut KeyRep) -> Self {
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
            up: keys.hit(KeyCode::Up),
            down: keys.hit(KeyCode::Down),
            left: keys.hit(KeyCode::Left),
            right: keys.hit(KeyCode::Right),
            page_up: keys.hit(KeyCode::PageUp),
            page_down: keys.hit(KeyCode::PageDown),
            activate: is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space),
            escape: is_key_pressed(KeyCode::Escape),
            hotkey,
            alt: is_key_down(KeyCode::LeftAlt) || is_key_down(KeyCode::RightAlt),
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
        Self { label: label.into(), key, value: value.to_string(), toggle: None, pending }
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
            p.file_name().unwrap_or(p.as_os_str()).to_string_lossy().chars().take(20).collect()
        })
        .unwrap_or_else(|| default.into())
}

fn rows(page: Page, s: &Settings) -> Vec<MenuRow> {
    use MenuRow as R;
    match page {
        Page::Sound => vec![
            R::toggle("Sound", 'S', s.sound.enabled, 20),
            R::counter("Flight Sounds", 'F', format!("{:?}", s.sound.flight), Some(20)),
        ],
        Page::Hardware => vec![
            R::counter("Firing Delay", 'F', s.hardware.firing_delay, None),
            R::counter("Falling Delay", 'A', s.hardware.falling_delay, None),
        ],
        Page::Economics => vec![
            R::counter("Interest Rate", 'I', format!("{:.2}", s.economics.interest_rate), None),
            R::counter("Cash at Start", 'C', format!("${}", s.economics.cash_at_start), None),
            R::toggle("Computers Buy", 'B', s.economics.computers_buy, 6).enabled(),
            R::counter("Scoring Mode", 'S', format!("{:?}", s.economics.scoring), None),
            R::toggle("Free Market", 'F', s.economics.free_market, 7),
        ],
        Page::Physics => vec![
            R::counter("Air Viscosity", 'A', s.physics.air_viscosity, None),
            R::counter("Gravity", 'G', format!("{:.2}", s.physics.gravity), None),
            R::counter("Suspend Dirt", 'S', format!("{}%", s.physics.suspend_dirt), None),
            R::toggle("Tanks Fall", 'T', s.physics.tanks_fall, 3).enabled(),
            R::counter("Borders Extend", 'B', s.physics.borders_extend, None),
            R::counter("Effect of Walls", 'W', format!("{:?}", s.physics.walls), Some(4)),
        ],
        Page::Landscape => vec![
            R::counter("Sky", 'S', format!("{:?}", s.landscape.sky), Some(19)),
            R::counter("Max. Wind", 'W', s.landscape.max_wind, None),
            R::toggle("Changing Wind", 'C', s.landscape.changing_wind, 3).enabled(),
            R::toggle("Hostile Environment", 'H', s.landscape.hostile_environment, 19),
            R::counter("Bumpiness", 'B', s.landscape.bumpiness, Some(19)),
            R::counter("Slope", 'L', s.landscape.slope, Some(19)),
            R::toggle("Flatten Peaks", 'F', s.landscape.flatten_peaks, 19),
            R::toggle("Random Land", 'R', s.landscape.random_land, 19),
            R::counter(
                "Scanned Mountains",
                'M',
                format!("{}%", s.landscape.scanned_mountains_percent),
                Some(19),
            ),
            R::toggle("Classic Terrain", 'T', s.landscape.classic_terrain, 19),
            R::counter(
                "Mountains Folder",
                'D',
                file_label(&s.landscape.mountains_folder, "Not selected"),
                Some(19),
            ),
        ],
        Page::Play => vec![
            R::counter("Mode", 'M', format!("{:?}", s.play.mode), Some(18)),
            R::counter("Play Order", 'O', format!("{:?}", s.play.order), Some(17)),
            R::counter("Teams", 'T', format!("{:?}", s.play.teams), Some(17)),
            R::toggle("Fast Computers", 'F', s.play.fast_computers, 17),
            R::counter("Talking Tanks", 'A', format!("{:?}", s.play.talking_tanks), Some(17)),
            R::counter("Talk Probability", 'P', format!("{}%", s.play.talk_probability), Some(17)),
            R::toggle("Status Bar", 'S', s.play.status_bar, 8),
            R::counter(
                "Attack File",
                'C',
                file_label(&s.play.attack_comments, "Built-in phrases"),
                Some(17),
            ),
            R::counter(
                "Death File",
                'D',
                file_label(&s.play.death_comments, "Built-in phrases"),
                Some(17),
            ),
        ],
        Page::Weapons => vec![
            R::counter("Arms Level", 'A', s.weapons.arms_level, Some(7)),
            R::counter("Bomb Icon", 'B', format!("{:?}", s.weapons.bomb_icon), Some(9)),
            R::toggle("Tunneling", 'T', s.weapons.tunneling, 5).enabled(),
            R::counter("Scale", 'S', format!("{:?}", s.weapons.scale), Some(9)),
            R::toggle("Trace Paths", 'P', s.weapons.trace_paths, 9),
            R::toggle("Extra Dirt", 'E', s.weapons.extra_dirt, 10),
            R::toggle("Useless Items", 'U', s.weapons.useless_items, 7),
        ],
        Page::Players => (0..s.players)
            .map(|i| {
                R::counter(
                    &format!("Player {}", i + 1),
                    char::from_digit(((i + 1) % 10) as u32, 10).unwrap(),
                    s.controllers[i].label(),
                    None,
                )
            })
            .chain(std::iter::once(R::counter("Done", 'D', "Start game", None)))
            .collect(),
    }
}

const MAIN_KEYS: [char; 11] = ['S', 'P', 'R', 'O', 'H', 'E', 'Y', 'L', 'T', 'W', 'C'];
const MAIN_PAGES: [Option<Page>; 11] = [
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
];

fn cycle(value: i32, direction: i32, min: i32, max: i32) -> i32 {
    min + (value - min + direction).rem_euclid(max - min + 1)
}

fn change_row(s: &mut Settings, page: Page, row: usize, direction: i32) {
    match (page, row) {
        (Page::Hardware, 0) => {
            s.hardware.firing_delay =
                cycle(s.hardware.firing_delay as i32, direction, 0, 500) as u16
        }
        (Page::Hardware, 1) => {
            s.hardware.falling_delay =
                cycle(s.hardware.falling_delay as i32, direction, 0, 1000) as u16
        }
        (Page::Economics, 0) => {
            s.economics.interest_rate =
                cycle((s.economics.interest_rate * 100.0).round() as i32, direction, 0, 30) as f64
                    / 100.0
        }
        (Page::Economics, 1) => {
            s.economics.cash_at_start =
                cycle((s.economics.cash_at_start / 1000) as i32, direction, 0, 1000) as u32 * 1000
        }
        (Page::Economics, 2) => s.economics.computers_buy = !s.economics.computers_buy,
        (Page::Economics, 3) => {
            use crate::settings::ScoringMode::{Basic, Greedy, Standard};
            let modes = [Basic, Standard, Greedy];
            let index = modes.iter().position(|&mode| mode == s.economics.scoring).unwrap();
            s.economics.scoring = modes[cycle(index as i32, direction, 0, 2) as usize];
        }
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
        (Page::Landscape, 1) => {
            s.landscape.max_wind = cycle(s.landscape.max_wind as i32, direction, 0, 500) as u16
        }
        (Page::Landscape, 2) => s.landscape.changing_wind = !s.landscape.changing_wind,
        (Page::Weapons, 2) => s.weapons.tunneling = !s.weapons.tunneling,
        (Page::Players, i) if i < s.players => {
            let index = CTRLS.iter().position(|ctrl| *ctrl == s.controllers[i]).unwrap();
            s.controllers[i] =
                CTRLS[cycle(index as i32, direction, 0, CTRLS.len() as i32 - 1) as usize];
        }
        _ => {}
    }
}

fn dialog_rect(page: Page, count: usize) -> (i32, i32, i32, i32) {
    let height = 72 + count as i32 * 24;
    let width = if page == Page::Players { 400 } else { 472 };
    ((W - width) / 2, (H - height) / 2, width, height)
}

impl MenuState {
    pub(crate) fn handle(&mut self, s: &mut Settings, input: &MenuInput) -> MenuAction {
        if self.notice.is_some() {
            if input.escape || input.activate || input.click.is_some() {
                self.notice = None;
            }
            return MenuAction::Stay;
        }
        if let Some(page) = self.page {
            let options = rows(page, s);
            self.row = self.row.min(options.len() - 1);
            let (x, y, w, h) = dialog_rect(page, options.len());
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
            if change && options[self.row].pending.is_none() {
                if page == Page::Players && self.row == s.players {
                    self.page = None;
                    return MenuAction::Start;
                }
                let jump = if input.page_up || input.page_down { 10 } else { 1 };
                change_row(s, page, self.row, direction * jump);
            }
            return MenuAction::Stay;
        }
        if input.escape {
            return MenuAction::Quit;
        }
        if input.up {
            self.selected = (self.selected + 10) % 11;
        }
        if input.down {
            self.selected = (self.selected + 1) % 11;
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
            if (12..232).contains(&x) && (44..396).contains(&y) && (y - 44) % 32 < 28 {
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
                        direction * if input.page_up || input.page_down { 10 } else { 1 },
                        1,
                        1000,
                    ) as u32
                }
                10 => return MenuAction::Save,
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
) -> MenuAction {
    state.handle(settings, &MenuInput::poll(keys))
}

fn hotkey_text(fb: &mut Fb, x: i32, y: i32, label: &str, key: char, color: Rgb, hot: Rgb) {
    fb.text(x, y, label, color);
    if let Some(index) = label.chars().position(|ch| ch.to_ascii_uppercase() == key) {
        fb.text(x + index as i32 * 8, y, &key.to_string(), hot);
        fb.line(x + index as i32 * 8, y + 16, x + index as i32 * 8 + 6, y + 16, hot);
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
        fb.line(x + 94, y + 218 + offset, x + 132, y + 196 + offset, Rgb(115, 165, 60));
    }
    fb.text_scaled(x + 35, y + 26, "SCORCHED", Rgb::BLACK, 4, 2);
    fb.text_scaled(x + 32, y + 23, "SCORCHED", Rgb(255, 200, 45), 4, 2);
    fb.text_scaled(x + 111, y + 96, "EARTH", Rgb::BLACK, 4, 2);
    fb.text_scaled(x + 108, y + 93, "EARTH", Rgb(255, 120, 20), 4, 2);
    fb.text(x + 70, y + 282, "A RUST TRIBUTE", Rgb(255, 215, 130));
    fb.frame(x, y, w, h, Rgb::DARK);
}

pub(crate) fn draw_menu(fb: &mut Fb, s: &Settings, state: &MenuState) {
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
            if selected { Rgb(255, 255, 70) } else { Rgb(140, 0, 0) },
        );
    }
    fb.text(248, 380, "WENDELL HICKEN'S DOS CLASSIC", Rgb(180, 180, 200));
    fb.text(16, 418, "Arrows select/change  Enter choose  Esc quit", Rgb::WHITE);
    fb.text(16, 442, "Hotkey: next   Alt+hotkey: previous", Rgb(160, 190, 230));
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
            let color = if row.pending.is_some() { Rgb(110, 110, 110) } else { Rgb::BLACK };
            if page == Page::Players && i < s.players {
                fb.rect(x + 16, ry + 3, 9, 10, PLAYER_COLORS[i]);
            }
            let label_x = if page == Page::Players { x + 32 } else { x + 16 };
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
            "Arrows: type  D: start game  Esc: back".into()
        } else {
            "Arrows / hotkeys change; Esc closes".into()
        };
        fb.text(x + 12, y + h - 26, &footer, Rgb::DARK);
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
        fb.text(76, y + height - 22, "Enter / Esc / click to close", Rgb::DARK);
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
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

    fn key(key: char, alt: bool) -> MenuInput {
        MenuInput { hotkey: Some(key), alt, ..Default::default() }
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
        for (key, page) in
            MAIN_KEYS.iter().zip(MAIN_PAGES).filter_map(|(key, page)| page.map(|p| (*key, p)))
        {
            menu.handle(&mut settings, &key_input(key));
            assert_eq!(menu.page, Some(page));
            menu.handle(&mut settings, &MenuInput { escape: true, ..Default::default() });
            assert_eq!(menu.page, None);
        }
        assert_eq!(menu.handle(&mut settings, &key('C', false)), MenuAction::Save);
        assert_eq!(
            menu.handle(&mut settings, &MenuInput { escape: true, ..Default::default() }),
            MenuAction::Quit
        );
    }

    fn key_input(ch: char) -> MenuInput {
        key(ch, false)
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
            for row in rows(page, &settings).into_iter().filter(|row| row.pending.is_some()) {
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
        assert!(rows(Page::Physics, &s).iter().take(5).all(|r| r.pending.is_none()));
        assert!(rows(Page::Physics, &s)[5].pending.is_some()); // Walls are batch 4.
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
    fn economics_controls_cycle_and_leave_free_market_disabled() {
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
        assert!(rows(Page::Economics, &s).iter().take(4).all(|row| row.pending.is_none()));
        menu.handle(&mut s, &key('F', false));
        assert!(!s.economics.free_market);
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
            weapon: ItemId::BabyMissile,
            fall: 0,
            last_hit: None,
            kills: 0,
            wins: 0,
        };
        let prices = crate::economy::base_prices();
        let mut selected = 0;
        assert_eq!(crate::shop::offers(&tank, &prices)[0], ItemId::Missile);
        assert!(!handle_opening_shop(
            &mut tank,
            &prices,
            &mut selected,
            &MenuInput { activate: true, ..Default::default() }
        ));
        assert_eq!(tank.account.cash, Money::from(3125));
        assert_eq!(tank.inventory.count(ItemId::Missile), 15);
        assert!(!handle_opening_shop(
            &mut tank,
            &prices,
            &mut selected,
            &MenuInput { click: Some((70, 140, 1)), ..Default::default() }
        ));
        assert_eq!(tank.account.cash, Money::from(1250));
        assert_eq!(tank.inventory.count(ItemId::Missile), 20);
        let before = tank.account.clone();
        assert!(handle_opening_shop(
            &mut tank,
            &prices,
            &mut selected,
            &MenuInput { escape: true, ..Default::default() }
        ));
        assert_eq!(tank.account, before);
        tank.account.cash = Money::default();
        assert!(handle_opening_shop(
            &mut tank,
            &prices,
            &mut selected,
            &MenuInput { activate: true, ..Default::default() }
        ));
    }

    #[test]
    fn mouse_buttons_change_counters_and_outside_clicks_only_close_dialogs() {
        let mut settings = Settings::default();
        let mut menu = MenuState::default();
        menu.handle(
            &mut settings,
            &MenuInput { click: Some((30, 44 + 4 * 32 + 8, 1)), ..Default::default() },
        );
        assert_eq!(menu.page, Some(Page::Hardware));
        let (x, y, _, _) = dialog_rect(Page::Hardware, 2);
        menu.handle(
            &mut settings,
            &MenuInput { click: Some((x + 40, y + 38, 1)), ..Default::default() },
        );
        assert_eq!(settings.hardware.firing_delay, 101);
        menu.handle(
            &mut settings,
            &MenuInput { click: Some((x + 40, y + 38, -1)), ..Default::default() },
        );
        assert_eq!(settings.hardware.firing_delay, 100);
        assert_eq!(
            menu.handle(
                &mut settings,
                &MenuInput { click: Some((30, 44, 1)), ..Default::default() }
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
        assert_eq!(menu.handle(&mut settings, &key('S', false)), MenuAction::Stay);
        assert_eq!(menu.page, Some(Page::Players));
        let before = settings.controllers[9];
        menu.handle(&mut settings, &key('0', false));
        assert_ne!(settings.controllers[9], before);
        menu.handle(&mut settings, &key('0', true));
        assert_eq!(settings.controllers[9], before);
        assert_eq!(menu.handle(&mut settings, &key('D', false)), MenuAction::Start);
        assert_eq!(menu.page, None);
    }

    #[test]
    fn notices_consume_dismissal_without_activating_the_menu_behind_them() {
        let mut settings = Settings::default();
        let mut menu =
            MenuState { notice: Some("Cannot save settings".into()), ..Default::default() };
        assert_eq!(
            menu.handle(&mut settings, &MenuInput { activate: true, ..Default::default() }),
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
            let menu = MenuState { page, ..Default::default() };
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
                preview.bytes =
                    preview.bytes.chunks_exact((W * 4) as usize).rev().flatten().copied().collect();
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
    let mut lines = vec![title.into()];
    if !message.is_empty() {
        lines.push(message.into());
    }
    lines.push(format!(
        "{:?} - ranked by {}",
        mode,
        if mode == crate::settings::ScoringMode::Greedy { "net worth" } else { "cash" }
    ));
    lines.push(String::new());
    for (rank, &i) in economy::ranking(tanks, mode, prices).iter().enumerate() {
        let tank = &tanks[i];
        let name: String = tank.name.chars().take(12).collect();
        let mut line =
            format!("{:2}. {:12} Cash {:>14}", rank + 1, name, money_text(&tank.account.cash));
        if mode == crate::settings::ScoringMode::Greedy {
            line.push_str(&format!(" Worth {:>14}", money_text(&economy::net_worth(tank, prices))));
        }
        lines.push(line);
    }
    lines.push(String::new());
    lines.push("Press any key".into());
    dialog(fb, &lines);
}

fn handle_opening_shop(
    tank: &mut Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    selected: &mut usize,
    input: &MenuInput,
) -> bool {
    let offers = crate::shop::offers(tank, prices);
    *selected = (*selected).min(offers.len());
    if input.escape || input.hotkey == Some('D') {
        return true;
    }
    if input.up {
        *selected = (*selected + offers.len()) % (offers.len() + 1);
    }
    if input.down {
        *selected = (*selected + 1) % (offers.len() + 1);
    }
    let mut activate = input.activate;
    if let Some((x, y, direction)) = input.click {
        if direction > 0
            && (56..584).contains(&x)
            && y >= 134
            && y < 134 + (offers.len() + 1) as i32 * 24
        {
            *selected = ((y - 134) / 24) as usize;
            activate = true;
        }
    }
    if activate {
        if *selected == offers.len() {
            return true;
        }
        crate::shop::buy(tank, offers[*selected], prices);
        *selected = (*selected).min(crate::shop::offers(tank, prices).len());
    }
    false
}

pub(crate) fn update_opening_shop(
    tank: &mut Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    selected: &mut usize,
    keys: &mut KeyRep,
) -> bool {
    handle_opening_shop(tank, prices, selected, &MenuInput::poll(keys))
}

pub(crate) fn draw_opening_shop(
    fb: &mut Fb,
    tank: &Tank,
    prices: &[u32; crate::weapons::ITEM_COUNT],
    selected: usize,
) {
    use crate::economy::money_text;
    fb.rect(0, 0, W, H, Rgb(20, 20, 45));
    fb.bevel(40, 40, 560, 382);
    fb.text(60, 54, "BUYING BEFORE ROUND 1", Rgb::BLACK);
    fb.text(60, 80, &tank.name, tank.color.scale(0.6));
    fb.text(300, 80, &format!("Cash: {}", money_text(&tank.account.cash)), Rgb::BLACK);
    fb.text(64, 108, "Weapon", Rgb::DARK);
    fb.text(220, 108, "Bundle", Rgb::DARK);
    fb.text(300, 108, "Price", Rgb::DARK);
    fb.text(448, 108, "Owned", Rgb::DARK);
    let offers = crate::shop::offers(tank, prices);
    for (i, &id) in offers.iter().enumerate() {
        let item = id.item();
        let y = 134 + i as i32 * 24;
        let color = if i == selected { Rgb::WHITE } else { Rgb::BLACK };
        if i == selected {
            fb.rect(56, y - 2, 528, 22, Rgb(0, 0, 140));
        }
        fb.text(64, y, item.name, color);
        fb.text(220, y, &format!("x{}", item.bundle_size), color);
        fb.text(300, y, &format!("${}", prices[id as usize]), color);
        fb.text(448, y, &tank.inventory.count(id).to_string(), color);
    }
    let y = 134 + offers.len() as i32 * 24;
    if selected >= offers.len() {
        fb.rect(56, y - 2, 528, 22, Rgb(0, 0, 140));
    }
    fb.text(64, y, "Done", if selected >= offers.len() { Rgb::WHITE } else { Rgb::BLACK });
    fb.text(60, 374, "Space / Enter: buy bundle   D / Esc: done", Rgb::DARK);
    fb.text(60, 396, "Only affordable complete bundles are offered.", Rgb::DARK);
}
