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

pub(crate) fn support_mask(tanks: &[Tank], terrain: &Terrain) -> [bool; MAX_PLAYERS] {
    std::array::from_fn(|i| {
        tanks
            .get(i)
            .is_some_and(|t| t.alive && t.health > 0 && t.supported(terrain))
    })
}
/// Remember who removed support before several owners' shots share a resolver.
pub(crate) fn record_undermining(
    tanks: &mut [Tank],
    terrain: &Terrain,
    before: [bool; MAX_PLAYERS],
    owner: Option<usize>,
) {
    for (i, tank) in tanks.iter_mut().enumerate() {
        if before[i] && tank.health > 0 && !tank.supported(terrain) {
            tank.last_hit = owner;
            tank.environmental_hit = owner.is_none();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ctrl {
    Human,
    Ai(AiKind),
}

pub const CTRLS: [Ctrl; 9] = [
    Ctrl::Human,
    Ctrl::Ai(AiKind::Moron),
    Ctrl::Ai(AiKind::Shooter),
    Ctrl::Ai(AiKind::Poolshark),
    Ctrl::Ai(AiKind::Tosser),
    Ctrl::Ai(AiKind::Chooser),
    Ctrl::Ai(AiKind::Spoiler),
    Ctrl::Ai(AiKind::Cyborg),
    Ctrl::Ai(AiKind::Unknown),
];

impl Ctrl {
    pub fn label(self) -> &'static str {
        match self {
            Ctrl::Human => "Human",
            Ctrl::Ai(kind) => kind.label(),
        }
    }
}

pub const SHIELDS: [ItemId; 5] = [
    ItemId::MagDeflector,
    ItemId::Shield,
    ItemId::ForceShield,
    ItemId::HeavyShield,
    ItemId::SuperMag,
];
pub const GUIDANCE: [ItemId; 5] = [
    ItemId::HeatGuidance,
    ItemId::BallisticGuidance,
    ItemId::HorizontalGuidance,
    ItemId::VerticalGuidance,
    ItemId::LazyBoy,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Wheels,
    #[default]
    Treads,
    Heavy,
    Bunker,
    Tower,
    Dish,
    Triple,
}
impl Shape {
    pub const ALL: [Self; 7] = [
        Self::Wheels,
        Self::Treads,
        Self::Heavy,
        Self::Bunker,
        Self::Tower,
        Self::Dish,
        Self::Triple,
    ];
    pub fn mobile(self) -> bool {
        matches!(
            self,
            Self::Wheels | Self::Treads | Self::Heavy | Self::Triple
        )
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Wheels => "Wheels",
            Self::Treads => "Treads",
            Self::Heavy => "Heavy",
            Self::Bunker => "Bunker",
            Self::Tower => "Tower",
            Self::Dish => "Dish",
            Self::Triple => "Triple",
        }
    }
    pub fn cycle(self, ctrl: Ctrl, direction: i32) -> Self {
        let choices: Vec<_> = Self::ALL
            .into_iter()
            .filter(|s| *s != Self::Triple || ctrl != Ctrl::Human)
            .collect();
        let index = choices.iter().position(|s| *s == self).unwrap_or(0) as i32;
        choices[(index + direction).rem_euclid(choices.len() as i32) as usize]
    }
    pub fn barrel_angles(self, angle: i32) -> Vec<i32> {
        if self == Self::Triple {
            vec![(angle - 5).max(0), angle, (angle + 5).min(180)]
        } else {
            vec![angle]
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Equipment {
    /// Units left in the opened fuel tank; unopened tanks stay in inventory.
    pub fuel: u8,
    pub parachutes_deployed: bool,
    pub parachute_threshold: u8,
    pub shield: Option<ItemId>,
    pub active_shield: Option<crate::shields::ActiveShield>,
    pub guidance: Option<ItemId>,
    pub triggers: bool,
    pub chute_open: bool,
    pub fall_started: bool,
    pub descent_time: f32,
}
impl Default for Equipment {
    fn default() -> Self {
        Self {
            fuel: 0,
            parachutes_deployed: true,
            parachute_threshold: 5,
            shield: None,
            active_shield: None,
            guidance: None,
            triggers: false,
            chute_open: false,
            fall_started: false,
            descent_time: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemUse {
    Applied,
    Unavailable,
    Later(u8),
}

#[derive(Clone)]
pub struct Tank {
    pub ai_state: crate::ai::State,
    pub shape: Shape,
    pub name: String,
    pub color: Rgb,
    pub ctrl: Ctrl,
    pub account: Account,
    /// Shared Corporate account identifier; account values are synchronized per transaction.
    pub(crate) bank: Option<u8>,
    pub(crate) team: Option<u8>,
    /// Centre column and bottom row of the tank (terrain starts at y + 1).
    pub x: i32,
    pub y: i32,
    pub angle: i32,
    pub power: i32,
    pub health: i32,
    pub alive: bool,
    pub inventory: Inventory,
    pub equipment: Equipment,
    pub weapon: ItemId,
    pub(crate) fall: i32,
    pub(crate) last_hit: Option<usize>,
    pub(crate) environmental_hit: bool,
    pub(crate) kills: i32,
    pub(crate) wins: u32,
}

impl Tank {
    pub fn fuel_available(&self) -> u16 {
        u16::from(self.equipment.fuel) + u16::from(self.inventory.count(ItemId::FuelTank)) * 10
    }
    pub fn spend_fuel(&mut self, amount: u16) -> bool {
        if !self.shape.mobile() || self.fuel_available() < amount {
            return false;
        }
        let remaining = self.fuel_available() - amount;
        let unopened = remaining / 10;
        let used = u16::from(self.inventory.count(ItemId::FuelTank)) - unopened;
        self.inventory.remove(ItemId::FuelTank, u32::from(used));
        self.equipment.fuel = (remaining % 10) as u8;
        true
    }
    pub fn engage_shield(&mut self, id: ItemId) -> bool {
        if !self.alive || self.health <= 0 || self.inventory.count(id) == 0 {
            return false;
        }
        let Some(shield) = crate::shields::ActiveShield::new(id) else {
            return false;
        };
        self.inventory.remove(id, 1);
        self.equipment.shield = Some(id);
        self.equipment.active_shield = Some(shield);
        true
    }

    /// Return actual shield points removed; direct chip/reflection costs never overflow.
    pub fn drain_shield(&mut self, damage: i32) -> i32 {
        let Some(shield) = &mut self.equipment.active_shield else {
            return 0;
        };
        let lost = damage.max(0).min(shield.strength.max(0));
        shield.strength -= lost;
        if lost > 0 {
            shield.flash = 0.18;
        }
        if shield.strength <= 0 {
            self.equipment.active_shield = None;
        }
        lost
    }

    pub fn shield_percent(&self) -> i32 {
        self.equipment
            .active_shield
            .as_ref()
            .map_or(0, |shield| shield.percent())
    }

    pub fn use_battery(&mut self) -> bool {
        if !self.alive
            || self.health <= 0
            || self.health >= 100
            || self.inventory.count(ItemId::Battery) == 0
        {
            return false;
        }
        self.inventory.remove(ItemId::Battery, 1);
        self.health = (self.health + 10).min(100);
        true
    }

    pub fn cycle_supply(&mut self, shields: bool, backwards: bool) {
        let choices = if shields { &SHIELDS } else { &GUIDANCE };
        let current = if shields {
            self.equipment.shield
        } else {
            self.equipment.guidance
        };
        let mut options = vec![None];
        options.extend(
            choices
                .iter()
                .copied()
                .filter(|&id| self.inventory.count(id) > 0)
                .map(Some),
        );
        let index = options.iter().position(|&id| id == current).unwrap_or(0);
        let next = options[(index + if backwards { options.len() - 1 } else { 1 }) % options.len()];
        if shields {
            self.equipment.shield = next;
        } else {
            self.equipment.guidance = next;
        }
    }

    pub fn use_item(&mut self, id: ItemId) -> ItemUse {
        if self.inventory.count(id) == 0 {
            return ItemUse::Unavailable;
        }
        if self.inventory.can_select(id) {
            self.weapon = id;
            return ItemUse::Applied;
        }
        match id {
            ItemId::Battery => {
                if self.use_battery() {
                    ItemUse::Applied
                } else {
                    ItemUse::Unavailable
                }
            }
            ItemId::Parachute => {
                self.equipment.parachutes_deployed = !self.equipment.parachutes_deployed;
                ItemUse::Applied
            }
            ItemId::ContactTrigger => {
                self.equipment.triggers = !self.equipment.triggers;
                ItemUse::Applied
            }
            id if SHIELDS.contains(&id) => {
                if self.engage_shield(id) {
                    ItemUse::Applied
                } else {
                    ItemUse::Unavailable
                }
            }
            id if GUIDANCE.contains(&id) => {
                self.equipment.guidance = Some(id);
                ItemUse::Applied
            }
            ItemId::FuelTank => ItemUse::Unavailable, // Movement is a battlefield action.
            ItemId::AutoDefense => ItemUse::Applied,
            _ => ItemUse::Later(10),
        }
    }

    pub fn reset_descent(&mut self) {
        self.fall = 0;
        self.equipment.chute_open = false;
        self.equipment.fall_started = false;
        self.equipment.descent_time = 0.0;
    }

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
        if impact_damage && !self.equipment.chute_open {
            self.health -= self.fall * 2;
        }
        self.reset_descent();
        self.power = self.power.min(self.max_power());
    }

    pub fn supported(&self, terrain: &Terrain) -> bool {
        (-7..=7)
            .filter(|dx| terrain.solid(self.x + dx, self.y + 1))
            .take(3)
            .count()
            >= 3
    }
}
