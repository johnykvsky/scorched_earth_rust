//! Local top-ten final scores, kept separately for each scoring mode.
use crate::{
    economy::{self, Money},
    settings::ScoringMode,
    tank::Tank,
    weapons::ITEM_COUNT,
};
use serde::{Deserialize, Serialize};
use std::{io::Read, path::Path};

pub const MODES: [ScoringMode; 3] = [
    ScoringMode::Basic,
    ScoringMode::Standard,
    ScoringMode::Greedy,
];
const LIMIT: usize = 10;
const MAX_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub score: Money,
    pub mode: ScoringMode,
    pub rounds: u32,
    pub players: usize,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HallOfFame {
    version: u32,
    entries: Vec<Entry>,
}

impl Default for HallOfFame {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl HallOfFame {
    pub fn entries(&self, mode: ScoringMode) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(move |entry| entry.mode == mode)
    }

    pub fn record(
        &mut self,
        tanks: &[Tank],
        prices: &[u32; ITEM_COUNT],
        mode: ScoringMode,
        rounds: u32,
    ) {
        let mut banks = Vec::new();
        for tank in tanks {
            if let Some(bank) = tank.bank {
                if banks.contains(&bank) {
                    continue;
                }
                banks.push(bank);
            }
            let mut score = tank.account.cash.clone();
            if mode == ScoringMode::Greedy {
                if let Some(bank) = tank.bank {
                    for member in tanks.iter().filter(|t| t.bank == Some(bank)) {
                        score.add(&economy::inventory_value(&member.inventory, prices));
                    }
                } else {
                    score = economy::net_worth(tank, prices);
                }
            }
            self.entries.push(Entry {
                name: tank
                    .bank
                    .map_or_else(|| tank.name.clone(), |bank| format!("Team {}", bank + 1)),
                score,
                mode,
                rounds,
                players: tanks.len(),
            });
        }
        // Stable sorting keeps older records ahead when scores tie.
        self.entries.sort_by(|a, b| b.score.cmp(&a.score));
        let mut counts = [0; 3];
        self.entries.retain(|entry| {
            let i = MODES.iter().position(|&mode| mode == entry.mode).unwrap();
            counts[i] += 1;
            counts[i] <= LIMIT
        });
    }

    fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported hall of fame version".into());
        }
        for mode in MODES {
            let entries: Vec<_> = self.entries(mode).collect();
            if entries.len() > LIMIT || entries.windows(2).any(|w| w[0].score < w[1].score) {
                return Err("Invalid hall of fame ranking".into());
            }
        }
        if self.entries.iter().any(|e| {
            e.name.trim().is_empty()
                || e.name.chars().count() > 32
                || e.name.chars().any(char::is_control)
                || !(1..=1000).contains(&e.rounds)
                || !(2..=10).contains(&e.players)
        }) {
            return Err("Invalid hall of fame entry".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("Cannot read hall of fame: {e}")),
        };
        let mut text = String::new();
        file.take(MAX_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|e| format!("Cannot read hall of fame: {e}"))?;
        if text.len() as u64 > MAX_BYTES {
            return Err("Hall of fame exceeds 256 KiB".into());
        }
        let hall: Self =
            toml::from_str(&text).map_err(|_| "Invalid hall of fame TOML".to_string())?;
        hall.validate()?;
        Ok(hall)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        crate::settings::save_atomic(path, &text)
    }
}

pub fn draw(fb: &mut crate::gfx::Fb, hall: &HallOfFame, mode: ScoringMode, notice: Option<&str>) {
    use crate::gfx::{Rgb, H, W};
    fb.rect(0, 0, W, H, Rgb(20, 20, 45));
    fb.bevel(16, 16, W - 32, 376);
    fb.text_centered(26, "HALL OF FAME", Rgb::BLACK);
    fb.text_centered(
        50,
        &format!(
            "{mode:?}: best final {}",
            if mode == ScoringMode::Greedy {
                "net worth"
            } else {
                "cash"
            }
        ),
        Rgb::BLACK,
    );
    for (x, label) in [
        (32, "Rank"),
        (80, "Player / bank"),
        (336, "Score"),
        (416, "Rounds"),
        (488, "Players"),
    ] {
        fb.text(x, 78, label, Rgb::BLACK);
    }
    for (i, entry) in hall.entries(mode).enumerate() {
        let y = 106 + i as i32 * 23;
        fb.text(32, y, &format!("{:2}", i + 1), Rgb::BLACK);
        fb.text(
            80,
            y,
            &entry.name.chars().take(20).collect::<String>(),
            Rgb::BLACK,
        );
        fb.text(
            264,
            y,
            &format!("{:>14}", economy::money_text(&entry.score)),
            Rgb::BLACK,
        );
        fb.text(432, y, &format!("{:4}", entry.rounds), Rgb::BLACK);
        fb.text(512, y, &format!("{:2}", entry.players), Rgb::BLACK);
    }
    if hall.entries(mode).next().is_none() {
        fb.text_centered(
            200,
            "Finish a match to record its final scores.",
            Rgb::BLACK,
        );
    }
    if let Some(notice) = notice {
        for (i, line) in crate::ui::wrap_text(notice, 70).iter().take(2).enumerate() {
            fb.text(32, 348 + i as i32 * 18, line, Rgb(140, 0, 0));
        }
    }
    crate::ui::mouse_buttons(fb, &["Previous mode", "Next mode", "Back"]);
    fb.text_centered(
        442,
        "Left/Right: scoring mode   Enter/Esc: back",
        Rgb::WHITE,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tanks() -> Vec<Tank> {
        (0..10)
            .map(|i| {
                let mut tank = crate::shop::tests::tank(1000 + i * 100);
                tank.name = format!("Player {}", i + 1);
                tank
            })
            .collect()
    }

    #[test]
    fn hall_of_fame_top_ten_per_mode_exact_scores_and_stable_ties() {
        let mut hall = HallOfFame::default();
        let prices = economy::base_prices();
        let mut tanks = tanks();
        let huge = Money::from_decimal(&"9".repeat(200)).unwrap();
        tanks[0].account.cash = huge.clone();
        for mode in MODES {
            hall.record(&tanks, &prices, mode, 1000);
            let first = hall.entries(mode).next().unwrap();
            assert_eq!(first.name, "Player 1");
            assert_eq!(
                first.score,
                if mode == ScoringMode::Greedy {
                    economy::net_worth(&tanks[0], &prices)
                } else {
                    huge.clone()
                }
            );
        }
        tanks[0].name = "Later tie".into();
        hall.record(&tanks, &prices, ScoringMode::Standard, 1);
        let entries: Vec<_> = hall.entries(ScoringMode::Standard).collect();
        assert_eq!(entries.len(), 10);
        assert_eq!(entries[0].name, "Player 1");
        assert_eq!(entries[1].name, "Later tie");
        assert_eq!(hall.entries(ScoringMode::Basic).count(), 10);
        assert_eq!(hall.entries(ScoringMode::Greedy).count(), 10);
        hall.validate().unwrap();
        let restored: HallOfFame = toml::from_str(&toml::to_string(&hall).unwrap()).unwrap();
        assert_eq!(restored.entries, hall.entries);
    }

    #[test]
    fn hall_of_fame_corporate_banks_count_cash_once_and_all_member_inventory() {
        let prices = economy::base_prices();
        let mut tanks = tanks();
        for (i, tank) in tanks.iter_mut().enumerate() {
            tank.team = Some((i % 2) as u8);
        }
        economy::configure_banks(&mut tanks, true);
        let mut hall = HallOfFame::default();
        for mode in MODES {
            hall.record(&tanks, &prices, mode, 5);
        }
        for mode in MODES {
            assert_eq!(hall.entries(mode).count(), 2);
            for entry in hall.entries(mode) {
                let bank = if entry.name == "Team 1" { 0 } else { 1 };
                let mut expected = tanks[bank as usize].account.cash.clone();
                if mode == ScoringMode::Greedy {
                    for tank in tanks.iter().filter(|t| t.bank == Some(bank)) {
                        expected.add(&economy::inventory_value(&tank.inventory, &prices));
                    }
                }
                assert_eq!(entry.score, expected);
            }
        }
    }

    #[test]
    fn hall_of_fame_rejects_invalid_versions_entries_rankings_and_oversized_money() {
        let mut hall = HallOfFame::default();
        hall.record(&tanks(), &economy::base_prices(), ScoringMode::Standard, 5);
        let valid = toml::to_string(&hall).unwrap();
        for text in [
            valid.replace("version = 1", "version = 2"),
            valid.replace("rounds = 5", "rounds = 0"),
            valid.replace("players = 10", "players = 11"),
            valid.replace("Player 10", ""),
            valid.replace(
                "score = \"1900\"",
                &format!("score = \"{}\"", "1".repeat(4097)),
            ),
        ] {
            assert!(toml::from_str::<HallOfFame>(&text)
                .and_then(|h| h.validate().map_err(serde::de::Error::custom))
                .is_err());
        }
        hall.entries.swap(0, 9);
        assert!(hall.validate().is_err());
    }

    #[test]
    fn hall_of_fame_previews() {
        let mut fb = crate::gfx::Fb::new();
        let mut hall = HallOfFame::default();
        let directory = std::env::var("SCORCHED_HALL_PREVIEW_DIR").ok();
        let save = |fb: &crate::gfx::Fb, name: &str| {
            if let Some(directory) = &directory {
                std::fs::create_dir_all(directory).unwrap();
                let mut image = fb.img.clone();
                image.bytes = image
                    .bytes
                    .chunks_exact((crate::gfx::W * 4) as usize)
                    .rev()
                    .flatten()
                    .copied()
                    .collect();
                image.export_png(&format!("{directory}/{name}.png"));
            }
        };
        draw(&mut fb, &hall, ScoringMode::Standard, None);
        save(&fb, "empty");
        let mut tanks = tanks();
        tanks[0].account.cash = Money::from_decimal(&"9".repeat(4096)).unwrap();
        tanks[0].name = "SixteenCharName!!".into();
        for mode in MODES {
            hall.record(&tanks, &economy::base_prices(), mode, 1000);
            draw(&mut fb, &hall, mode, None);
            save(&fb, &format!("{mode:?}"));
        }
        draw(
            &mut fb,
            &hall,
            ScoringMode::Standard,
            Some("Scores kept for this session. Cannot save hall of fame: permission denied."),
        );
        save(&fb, "error");
        crate::ui::draw_menu(
            &mut fb,
            &crate::settings::Settings::default(),
            &crate::ui::MenuState::default(),
        );
        save(&fb, "menu");
    }
}
