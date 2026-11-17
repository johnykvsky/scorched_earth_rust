//! Versioned buying-screen checkpoints. Battlefield state and options are excluded.
use crate::{
    ai::{AiKind, State},
    economy::Account,
    inventory::Inventory,
    settings::{Settings, MAX_PLAYERS},
    shop::Market,
    tank::{Ctrl, Equipment, Shape, Tank, GUIDANCE, PLAYER_COLORS, SHIELDS},
    weapons::{ItemId, ITEMS, ITEM_COUNT},
};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

const FORMAT: &str = "scorched-rust-save";
const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedGame {
    format: String,
    version: u32,
    pub round: u32,
    pub rounds: u32,
    pub interest_round: u32,
    pub buyer: usize,
    pub circle: Vec<usize>,
    pub players: Vec<Player>,
    pub market: Market,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Player {
    pub name: String,
    pub ctrl: Ctrl,
    pub shape: Shape,
    pub team: u8,
    account: Account,
    bank: Option<u8>,
    inventory: Vec<u8>,
    fuel: u8,
    parachutes_deployed: bool,
    parachute_threshold: u8,
    shield: Option<ItemId>,
    guidance: Option<ItemId>,
    triggers: bool,
    kills: i32,
    wins: u32,
    personality: Option<AiKind>,
    grudges: Vec<u32>,
}

impl SavedGame {
    pub fn capture(
        tanks: &[Tank],
        settings: &Settings,
        round: u32,
        interest_round: u32,
        buyer: usize,
        circle: &[usize],
        market: &Market,
    ) -> Self {
        Self {
            format: FORMAT.into(),
            version: 1,
            round,
            rounds: settings.rounds,
            interest_round,
            buyer,
            circle: circle.to_vec(),
            market: market.clone(),
            players: tanks
                .iter()
                .enumerate()
                .map(|(i, t)| Player {
                    name: t.name.clone(),
                    ctrl: t.ctrl,
                    shape: t.shape,
                    team: settings.team_ids[i],
                    account: t.account.clone(),
                    bank: t.bank,
                    inventory: ITEMS
                        .iter()
                        .map(|item| t.inventory.count(item.id))
                        .collect(),
                    fuel: t.equipment.fuel,
                    parachutes_deployed: t.equipment.parachutes_deployed,
                    parachute_threshold: t.equipment.parachute_threshold,
                    shield: t.equipment.shield,
                    guidance: t.equipment.guidance,
                    triggers: t.equipment.triggers,
                    kills: t.kills,
                    wins: t.wins,
                    personality: t.ai_state.resolved,
                    grudges: t.ai_state.grudges.to_vec(),
                })
                .collect(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        let invalid = |reason: &str| Err(format!("Invalid saved game: {reason}"));
        if self.format != FORMAT || self.version != 1 {
            return invalid("unsupported format or version");
        }
        let n = self.players.len();
        if !(2..=MAX_PLAYERS).contains(&n) || self.buyer >= n {
            return invalid("player count or current buyer");
        }
        if self.players[self.buyer].ctrl != Ctrl::Human {
            return invalid("current buyer must be human");
        }
        if !(1..=1000).contains(&self.rounds)
            || !(1..=self.rounds).contains(&self.round)
            || self.interest_round != self.round - 1
        {
            return invalid("round counters");
        }
        let mut circle = self.circle.clone();
        circle.sort_unstable();
        if circle != (0..n).collect::<Vec<_>>() {
            return invalid("turn order");
        }
        for p in &self.players {
            if p.name.trim().is_empty()
                || p.name.chars().count() > 16
                || p.name.chars().any(char::is_control)
                || p.team >= MAX_PLAYERS as u8
                || (p.ctrl == Ctrl::Human && p.shape == Shape::Triple)
            {
                return invalid("player identity");
            }
            if p.inventory.len() != ITEM_COUNT
                || p.inventory.iter().any(|&q| q > 99)
                || p.inventory[ItemId::BabyMissile as usize] != 99
                || p.fuel > 9
                || p.parachute_threshold > 100
                || p.shield.is_some_and(|id| !SHIELDS.contains(&id))
                || p.guidance.is_some_and(|id| !GUIDANCE.contains(&id))
            {
                return invalid("inventory or equipment");
            }
            if p.grudges.len() != MAX_PLAYERS
                || p.wins > self.round - 1
                || p.kills.unsigned_abs() > 10000
                || p.personality == Some(AiKind::Unknown)
                || match p.ctrl {
                    Ctrl::Human => p.personality.is_some(),
                    Ctrl::Ai(AiKind::Unknown) => p.personality.is_none(),
                    Ctrl::Ai(kind) => p.personality != Some(kind),
                }
            {
                return invalid("player history or computer personality");
            }
            if let Some(bank) = p.bank {
                if bank >= MAX_PLAYERS as u8
                    || bank != p.team
                    || self
                        .players
                        .iter()
                        .any(|q| q.bank == Some(bank) && q.account != p.account)
                {
                    return invalid("shared account");
                }
            }
        }
        self.market.validate()
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("Cannot read saved game: {e}"))?;
        let mut text = String::new();
        file.take(MAX_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|e| format!("Cannot read saved game: {e}"))?;
        if text.len() as u64 > MAX_BYTES {
            return Err("Saved game exceeds 1 MiB".into());
        }
        let saved: Self =
            toml::from_str(&text).map_err(|_| "Invalid saved game TOML".to_string())?;
        saved.validate()?;
        Ok(saved)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        crate::settings::save_atomic(path, &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checkpoint() -> SavedGame {
        let mut settings = Settings::default();
        settings.players = 2;
        let mut tanks = vec![
            crate::shop::tests::tank(12345),
            crate::shop::tests::tank(98765),
        ];
        tanks[0].equipment.fuel = 7;
        tanks[0].inventory.add(ItemId::Nuke, 3);
        tanks[0].account.cash = crate::economy::Money::from_decimal(
            "12345678901234567890123456789012345678901234567890",
        )
        .unwrap();
        SavedGame::capture(&tanks, &settings, 3, 2, 0, &[1, 0], &Market::default())
    }
    #[test]
    fn checkpoint_toml_round_trips_large_money_stock_and_persistent_equipment() {
        let saved = checkpoint();
        saved.validate().unwrap();
        let text = toml::to_string_pretty(&saved).unwrap();
        let restored: SavedGame = toml::from_str(&text).unwrap();
        restored.validate().unwrap();
        assert_eq!(restored.players[0].account, saved.players[0].account);
        let tank = restored.players[0].tank(0);
        assert_eq!(tank.equipment.fuel, 7);
        assert_eq!(tank.inventory.count(ItemId::Nuke), 3);
        assert_eq!(restored.circle, [1, 0]);
        assert!(!text.contains("gravity"));
        assert!(!text.contains("cash_at_start"));
        assert!(!text.contains("terrain"));
    }
    #[test]
    fn malformed_counters_stock_equipment_order_personality_and_banks_are_rejected() {
        let mutations: &[fn(&mut SavedGame)] = &[
            |s| s.version = 999,
            |s| s.format = "other-game".into(),
            |s| s.round = 0,
            |s| s.rounds = 1001,
            |s| s.interest_round = s.round,
            |s| s.buyer = 2,
            |s| s.players.clear(),
            |s| s.circle = vec![0, 0],
            |s| s.players[0].name = "\n".into(),
            |s| s.players[0].inventory.pop().map(|_| ()).unwrap(),
            |s| s.players[0].inventory[1] = 100,
            |s| s.players[0].inventory[0] = 0,
            |s| s.players[0].shield = Some(ItemId::Missile),
            |s| s.players[0].guidance = Some(ItemId::Shield),
            |s| s.players[0].fuel = 10,
            |s| s.players[0].shape = Shape::Triple,
            |s| s.players[0].personality = Some(AiKind::Unknown),
            |s| s.players[0].grudges.clear(),
            |s| s.players[0].bank = Some(99),
            |s| {
                s.players[0].bank = Some(0);
                s.players[1].bank = Some(0);
            },
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let mut saved = checkpoint();
            mutate(&mut saved);
            assert!(saved.validate().is_err(), "mutation {index}");
        }
        let text = toml::to_string_pretty(&checkpoint()).unwrap();
        for broken in [
            text.replace("version = 1", "version = 1\noptions = true"),
            text.replace(
                "cash = \"12345678901234567890123456789012345678901234567890\"",
                "cash = \"-1\"",
            ),
        ] {
            assert!(toml::from_str::<SavedGame>(&broken).is_err());
        }
        for value in ["", "-1", "1.5", "1e12", "NaN"] {
            assert!(crate::economy::Money::from_decimal(value).is_err());
        }
        assert_eq!(
            crate::economy::Money::from_decimal("0000").unwrap(),
            Default::default()
        );
    }
}

impl Player {
    pub fn tank(&self, index: usize) -> Tank {
        let mut inventory = Inventory::default();
        for (item, &quantity) in ITEMS.iter().zip(&self.inventory) {
            inventory.add(item.id, u32::from(quantity));
        }
        Tank {
            name: self.name.clone(),
            ctrl: self.ctrl,
            shape: self.shape,
            color: PLAYER_COLORS[index],
            account: self.account.clone(),
            bank: self.bank,
            team: Some(self.team),
            inventory,
            equipment: Equipment {
                fuel: self.fuel,
                parachutes_deployed: self.parachutes_deployed,
                parachute_threshold: self.parachute_threshold,
                shield: self.shield,
                guidance: self.guidance,
                triggers: self.triggers,
                ..Default::default()
            },
            ai_state: State {
                resolved: self.personality,
                grudges: std::array::from_fn(|i| self.grudges[i]),
                ..Default::default()
            },
            kills: self.kills,
            wins: self.wins,
            x: 0,
            y: 0,
            angle: 90,
            power: 400,
            health: 100,
            alive: true,
            weapon: ItemId::BabyMissile,
            fall: 0,
            last_hit: None,
            environmental_hit: false,
        }
    }
}
