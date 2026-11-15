//! Tank state, player controllers and tank geometry.
use crate::ai::AiKind;
use crate::economy::Account;
use crate::gfx::Rgb;
use crate::inventory::Inventory;
use crate::settings::MAX_PLAYERS;
use crate::terrain::Terrain;
use crate::weapons::ItemId;
use macroquad::prelude::{vec2, Vec2};

pub const PLAYER_COLORS: [Rgb; MAX_PLAYERS] = [
    Rgb(255, 50, 50),
    Rgb(60, 230, 60),
    Rgb(90, 130, 255),
    Rgb(255, 255, 60),
    Rgb(255, 70, 255),
    Rgb(60, 255, 255),
    Rgb(255, 150, 30),
    Rgb(240, 240, 240),
    Rgb(170, 110, 255),
    Rgb(170, 210, 100),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ctrl {
    Human,
    Ai(AiKind),
}

pub const CTRLS: [Ctrl; 4] =
    [Ctrl::Human, Ctrl::Ai(AiKind::Moron), Ctrl::Ai(AiKind::Shooter), Ctrl::Ai(AiKind::Spoiler)];

impl Ctrl {
    pub fn label(self) -> &'static str {
        match self {
            Ctrl::Human => "Human",
            Ctrl::Ai(AiKind::Moron) => "Moron",
            Ctrl::Ai(AiKind::Shooter) => "Shooter",
            Ctrl::Ai(AiKind::Spoiler) => "Spoiler",
        }
    }
}

pub struct Tank {
    pub name: String,
    pub color: Rgb,
    pub ctrl: Ctrl,
    pub account: Account,
    /// Team assignment is exposed by batch 17.
    pub(crate) team: Option<u8>,
    /// Centre column and bottom row of the tank (terrain starts at y + 1).
    pub x: i32,
    pub y: i32,
    pub angle: i32,
    pub power: i32,
    pub health: i32,
    pub alive: bool,
    pub inventory: Inventory,
    pub weapon: ItemId,
    pub(crate) fall: i32,
    pub(crate) last_hit: Option<usize>,
    pub(crate) kills: i32,
    pub(crate) wins: u32,
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

    pub(crate) fn finish_fall(&mut self, impact_damage: bool) {
        if impact_damage {
            self.health -= self.fall * 2;
        }
        self.fall = 0;
        self.power = self.power.min(self.max_power());
    }

    pub fn supported(&self, terrain: &Terrain) -> bool {
        (-7..=7).filter(|dx| terrain.solid(self.x + dx, self.y + 1)).take(3).count() >= 3
    }
}
