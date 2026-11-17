//! Shared-keyboard bindings and per-player state for concurrent play.
use crate::{
    gfx::{Fb, Rgb},
    settings::{Settings, MAX_PLAYERS},
    tank::Ctrl,
    ui::{self, MenuInput},
    weapons::ItemId,
};
use macroquad::prelude::{KeyCode, Vec2};
use serde::{Deserialize, Serialize};

macro_rules! keys {
    ($($key:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum Key { $($key),+ }
        impl Key {
            pub const ALL: &'static [Self] = &[$(Self::$key),+];
            pub fn code(self) -> KeyCode { match self { $(Self::$key => KeyCode::$key),+ } }
            pub fn from_code(code: KeyCode) -> Option<Self> { match code { $(KeyCode::$key => Some(Self::$key),)+ _ => None } }
            pub fn label(self) -> &'static str { match self { $(Self::$key => stringify!($key)),+ } }
        }
    }
}
// Escape, F1, F11 and modifier keys stay available to the application.
keys!(
    Right,
    Left,
    Up,
    Down,
    Space,
    Tab,
    D,
    A,
    W,
    S,
    Q,
    E,
    B,
    C,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    R,
    T,
    U,
    V,
    X,
    Y,
    Z,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    Comma,
    Period,
    Slash,
    Semicolon,
    Apostrophe,
    LeftBracket,
    RightBracket,
    Backslash,
    Minus,
    Equal,
    GraveAccent,
    Enter,
    Backspace,
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    KpDecimal,
    KpDivide,
    KpMultiply,
    KpSubtract,
    KpAdd,
    KpEnter,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F12
);

pub type Bindings = [Key; 6];
pub const ACTIONS: [&str; 6] = [
    "Clockwise",
    "Counter-clockwise",
    "Power up",
    "Power down",
    "Fire",
    "Change weapon",
];
pub fn defaults() -> [Bindings; MAX_PLAYERS] {
    std::array::from_fn(|player| std::array::from_fn(|action| Key::ALL[player * 6 + action]))
}
pub fn validate(
    maps: &[Bindings; MAX_PLAYERS],
    controllers: &[Ctrl; MAX_PLAYERS],
    players: usize,
) -> Result<(), String> {
    let mut used = Vec::new();
    for (i, map) in maps.iter().enumerate().take(players) {
        if controllers[i] != Ctrl::Human {
            continue;
        }
        for key in map {
            if used.contains(key) {
                return Err(format!("Key {} is assigned more than once", key.label()));
            }
            used.push(*key);
        }
    }
    Ok(())
}

#[derive(Clone, PartialEq, Eq)]
pub struct KeyEditor {
    pub player: usize,
    pub bindings: Bindings,
    pub row: usize,
    pub capture: bool,
    pub error: Option<String>,
}
impl KeyEditor {
    pub fn new(settings: &Settings, player: usize) -> Self {
        Self {
            player,
            bindings: settings.keys[player],
            row: 0,
            capture: false,
            error: None,
        }
    }
    /// True closes the editor. Changes are committed only with Done.
    pub fn handle(&mut self, settings: &mut Settings, input: &MenuInput) -> bool {
        if self.capture {
            if input.escape {
                self.capture = false;
                return false;
            }
            if let Some(code) = input.raw_key {
                if let Some(key) = Key::from_code(code) {
                    let mut maps = settings.keys;
                    maps[self.player] = self.bindings;
                    maps[self.player][self.row] = key;
                    match validate(&maps, &settings.controllers, settings.players) {
                        Ok(()) => {
                            self.bindings[self.row] = key;
                            self.error = None;
                            self.capture = false;
                        }
                        Err(error) => self.error = Some(error),
                    }
                } else {
                    self.error = Some("That key is reserved. Choose another key.".into());
                }
            }
            return false;
        }
        if input.escape || ui::button_index(input, 2) == Some(1) {
            return true;
        }
        if input.up {
            self.row = (self.row + 6) % 7;
        }
        if input.down {
            self.row = (self.row + 1) % 7;
        }
        let mut activate = input.activate;
        if let Some((x, y, 1)) = input.click {
            if (96..544).contains(&x) && (108..304).contains(&y) {
                self.row = ((y - 108) / 28) as usize;
                activate = true;
            }
        }
        if ui::button_index(input, 2) == Some(0) || (activate && self.row == 6) {
            let mut maps = settings.keys;
            maps[self.player] = self.bindings;
            match validate(&maps, &settings.controllers, settings.players) {
                Ok(()) => {
                    settings.keys = maps;
                    return true;
                }
                Err(error) => self.error = Some(error),
            }
        } else if activate && self.row < 6 {
            self.capture = true;
            self.error = None;
        }
        false
    }
    pub fn draw(&self, fb: &mut Fb) {
        fb.bevel(80, 64, 480, 320);
        fb.text(
            96,
            80,
            &format!("PLAYER {} - SIMULTANEOUS KEYS", self.player + 1),
            Rgb::BLACK,
        );
        for row in 0..7 {
            let y = 108 + row as i32 * 28;
            if row == self.row {
                fb.frame(92, y - 2, 456, 24, Rgb::DARK);
            }
            let label = if row == 6 {
                "Done".into()
            } else {
                format!("{}: {}", ACTIONS[row], self.bindings[row].label())
            };
            fb.text(100, y, &label, Rgb::BLACK);
        }
        fb.text(
            96,
            312,
            if self.capture {
                "Press a key (Esc: cancel capture)"
            } else {
                "Arrows: select  Enter/click: assign"
            },
            Rgb::DARK,
        );
        if let Some(error) = &self.error {
            for (i, line) in ui::wrap_text(error, 54).iter().take(2).enumerate() {
                fb.text(96, 338 + i as i32 * 18, line, Rgb(160, 0, 0));
            }
        }
        ui::mouse_buttons(fb, &["Done", "Cancel"]);
    }
}

#[derive(Clone, Copy, Default)]
pub struct Input {
    pub clockwise: i32,
    pub counter_clockwise: i32,
    pub power_up: i32,
    pub power_down: i32,
    pub fire: bool,
    pub weapon: bool,
}
#[derive(Clone, Copy)]
pub struct Shot {
    pub owner: usize,
    pub angle: i32,
    pub power: i32,
    pub weapon: ItemId,
    pub target: Option<Vec2>,
    pub batteries: Option<u8>,
    pub intended: Option<usize>,
}
#[derive(Default)]
pub struct Battle {
    pub queued: Vec<Option<Shot>>,
    pub launching: bool,
    pub ai: Vec<Option<crate::ai::Plan>>,
    pub ai_time: Vec<f32>,
    pub ai_step: Vec<f32>,
    pub flash: Vec<Option<(Rgb, f32)>>,
    pub energy: Vec<(usize, crate::energy::Effect)>,
    pub trails: Vec<Vec<(i32, i32, Rgb)>>,
    pub wind_time: f32,
}
impl Battle {
    pub fn reset(&mut self, count: usize) {
        *self = Self {
            queued: vec![None; count],
            ai: (0..count).map(|_| None).collect(),
            ai_time: vec![0.0; count],
            ai_step: vec![0.0; count],
            flash: vec![None; count],
            trails: vec![Vec::new(); count],
            ..Default::default()
        };
    }
}

pub fn hud_height(players: usize) -> i32 {
    players.div_ceil(2) as i32 * 24
}
pub fn draw_hud(fb: &mut Fb, tanks: &[crate::tank::Tank], busy: &[bool], wind: i32) {
    let height = hud_height(tanks.len());
    fb.rect(0, 0, 640, height, Rgb(18, 18, 28));
    for (i, tank) in tanks.iter().enumerate() {
        let (x, y) = ((i % 2) as i32 * 320 + 4, (i / 2) as i32 * 24 + 3);
        let name: String = tank.name.chars().take(8).collect();
        let weapon: String = tank.weapon.item().name.chars().take(15).collect();
        let text = if tank.alive && tank.health > 0 {
            format!(
                "{} {} P:{:4} {}{}",
                i + 1,
                name,
                tank.power,
                weapon,
                if busy[i] { "*" } else { "" }
            )
        } else {
            format!("{} {} OUT", i + 1, name)
        };
        fb.text(x, y, &text, if tank.alive { tank.color } else { Rgb::DARK });
    }
    let text = if wind == 0 {
        "No Wind".into()
    } else {
        format!("Wind: {wind}")
    };
    fb.text(
        640 - crate::gfx::text_w(&text) - 8,
        height + 6,
        &text,
        Rgb(100, 200, 255),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ten_player_maps_are_unique_persisted_and_reject_conflicts() {
        let mut settings = Settings::default();
        settings.players = MAX_PLAYERS;
        settings.controllers.fill(Ctrl::Human);
        settings.play.mode = crate::settings::PlayMode::Simultaneous;
        settings.validate().unwrap();
        let text = toml::to_string(&settings).unwrap();
        assert_eq!(toml::from_str::<Settings>(&text).unwrap(), settings);
        settings.keys[9][5] = settings.keys[0][0];
        assert!(settings
            .validate()
            .unwrap_err()
            .contains("assigned more than once"));
        settings.controllers[9] = Ctrl::Ai(crate::ai::AiKind::Shooter);
        settings.validate().unwrap();
        assert!(Key::from_code(KeyCode::F1).is_none());
        assert!(Key::from_code(KeyCode::F11).is_none());
        assert!(Key::from_code(KeyCode::Escape).is_none());
    }
    #[test]
    fn key_capture_is_staged_rejects_duplicates_and_can_cancel_or_commit() {
        let mut settings = Settings::default();
        settings.controllers[1] = Ctrl::Human;
        let original = settings.keys;
        let mut editor = KeyEditor::new(&settings, 1);
        editor.handle(
            &mut settings,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
        );
        assert!(editor.capture);
        editor.handle(
            &mut settings,
            &MenuInput {
                raw_key: Some(KeyCode::Space),
                ..Default::default()
            },
        );
        assert!(editor.capture && editor.error.is_some());
        editor.handle(
            &mut settings,
            &MenuInput {
                raw_key: Some(KeyCode::F1),
                ..Default::default()
            },
        );
        assert!(editor.error.as_ref().unwrap().contains("reserved"));
        editor.handle(
            &mut settings,
            &MenuInput {
                raw_key: Some(KeyCode::Kp0),
                ..Default::default()
            },
        );
        assert!(!editor.capture);
        assert_eq!(settings.keys, original);
        assert!(editor.handle(
            &mut settings,
            &MenuInput {
                escape: true,
                ..Default::default()
            }
        ));
        assert_eq!(settings.keys, original);
        let mut editor = KeyEditor::new(&settings, 1);
        editor.handle(
            &mut settings,
            &MenuInput {
                click: Some((120, 110, 1)),
                ..Default::default()
            },
        );
        editor.handle(
            &mut settings,
            &MenuInput {
                raw_key: Some(KeyCode::Kp0),
                ..Default::default()
            },
        );
        assert!(editor.handle(
            &mut settings,
            &MenuInput {
                click: Some((160, 410, 1)),
                ..Default::default()
            }
        ));
        assert_eq!(settings.keys[1][0], Key::Kp0);
        assert_eq!(settings.keys[0], original[0]);
    }
}
