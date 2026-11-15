//! Session options. Later batches expose these in dialogs and wire up their effects.
//! DOS-only hardware switches are intentionally omitted (TODO batch 2).
#![allow(dead_code)]

use crate::ai::AiKind;
use crate::tank::Ctrl;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::{fs, io::Write};

pub const MAX_PLAYERS: usize = 10;

macro_rules! option_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }
    };
}
option_enum!(FlightSound { Off, Position, Velocity });
option_enum!(ScoringMode { Basic, Standard, Greedy });
option_enum!(WallType { None, WrapAround, Padded, Rubber, Spring, Concrete, Random, Erratic });
option_enum!(Sky { Plain, Stormy, Stars, Shaded, Sunset, Cavern, Black, Random });
option_enum!(PlayMode { Sequential, Synchronous, Simultaneous });
option_enum!(PlayOrder { Random, LosersFirst, WinnersFirst, RoundRobin });
option_enum!(TeamMode { None, Standard, Corporate, Vicious });
option_enum!(TalkingTanks { Off, Computers, All });
option_enum!(BombIcon { Invisible, Small, Big });
option_enum!(ExplosionScale { Normal, Medium, Large });

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub players: usize,
    pub rounds: u32,
    pub controllers: [Ctrl; MAX_PLAYERS],
    pub sound: SoundSettings,
    pub hardware: HardwareSettings,
    pub economics: EconomicsSettings,
    pub physics: PhysicsSettings,
    pub landscape: LandscapeSettings,
    pub play: PlaySettings,
    pub weapons: WeaponSettings,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SoundSettings {
    pub enabled: bool,
    pub flight: FlightSound,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HardwareSettings {
    pub firing_delay: u16,
    pub falling_delay: u16,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EconomicsSettings {
    pub interest_rate: f64,
    pub cash_at_start: u32,
    pub computers_buy: bool,
    pub free_market: bool,
    pub scoring: ScoringMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhysicsSettings {
    pub air_viscosity: u8,
    pub gravity: f32,
    pub suspend_dirt: u8,
    pub tanks_fall: bool,
    pub borders_extend: u16,
    pub impact_damage: bool,
    pub walls: WallType,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LandscapeSettings {
    pub sky: Sky,
    pub max_wind: u16,
    pub changing_wind: bool,
    pub hostile_environment: bool,
    pub bumpiness: u8,
    pub slope: u8,
    pub flatten_peaks: bool,
    pub random_land: bool,
    pub scanned_mountains_percent: u8,
    pub mountains_folder: Option<PathBuf>,
    pub classic_terrain: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlaySettings {
    pub mode: PlayMode,
    pub order: PlayOrder,
    pub teams: TeamMode,
    pub fast_computers: bool,
    pub talking_tanks: TalkingTanks,
    pub talk_probability: u8,
    pub talk_delay: u16,
    pub attack_comments: Option<PathBuf>,
    pub death_comments: Option<PathBuf>,
    pub status_bar: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeaponSettings {
    pub arms_level: u8,
    pub bomb_icon: BombIcon,
    pub tunneling: bool,
    pub scale: ExplosionScale,
    pub trace_paths: bool,
    pub extra_dirt: bool,
    pub useless_items: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let mut controllers = [Ctrl::Ai(AiKind::Shooter); MAX_PLAYERS];
        controllers[0] = Ctrl::Human;
        Self {
            players: 2,
            rounds: 10,
            controllers,
            sound: SoundSettings { enabled: true, flight: FlightSound::Off },
            hardware: HardwareSettings { firing_delay: 100, falling_delay: 10 },
            economics: EconomicsSettings {
                interest_rate: 0.05,
                cash_at_start: 0,
                computers_buy: true,
                free_market: false,
                scoring: ScoringMode::Standard,
            },
            physics: PhysicsSettings {
                air_viscosity: 0,
                gravity: 0.2,
                suspend_dirt: 0,
                tanks_fall: true,
                borders_extend: 75,
                impact_damage: true,
                walls: WallType::None,
            },
            landscape: LandscapeSettings {
                sky: Sky::Random,
                max_wind: 200,
                changing_wind: false,
                hostile_environment: true,
                bumpiness: 20,
                slope: 20,
                flatten_peaks: false,
                random_land: false,
                scanned_mountains_percent: 20,
                mountains_folder: None,
                classic_terrain: false,
            },
            play: PlaySettings {
                mode: PlayMode::Sequential,
                order: PlayOrder::Random,
                teams: TeamMode::None,
                fast_computers: false,
                talking_tanks: TalkingTanks::Off,
                talk_probability: 100,
                talk_delay: 18,
                attack_comments: None,
                death_comments: None,
                status_bar: false,
            },
            weapons: WeaponSettings {
                arms_level: 4,
                bomb_icon: BombIcon::Big,
                tunneling: true,
                scale: ExplosionScale::Normal,
                trace_paths: false,
                extra_dirt: false,
                useless_items: false,
            },
        }
    }
}

macro_rules! section_default {
    ($($ty:ident => $field:ident),* $(,)?) => {$(
        impl Default for $ty {
            fn default() -> Self { Settings::default().$field }
        }
    )*};
}
section_default!(SoundSettings => sound, HardwareSettings => hardware,
    EconomicsSettings => economics, PhysicsSettings => physics,
    LandscapeSettings => landscape, PlaySettings => play, WeaponSettings => weapons);

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        macro_rules! range {
            ($value:expr, $min:expr, $max:expr, $name:literal) => {
                if !($min..=$max).contains(&$value) {
                    return Err(format!("{} must be between {} and {}", $name, $min, $max));
                }
            };
        }
        range!(self.players, 2, MAX_PLAYERS, "Players");
        range!(self.rounds, 1, 1000, "Rounds");
        range!(self.hardware.firing_delay, 0, 500, "Firing Delay");
        range!(self.hardware.falling_delay, 0, 1000, "Falling Delay");
        range!(self.economics.interest_rate, 0.0, 0.30, "Interest Rate");
        range!(self.economics.cash_at_start, 0, 1_000_000, "Cash at Start");
        range!(self.physics.air_viscosity, 0, 20, "Air Viscosity");
        range!(self.physics.gravity, 0.05, 10.0, "Gravity");
        range!(self.physics.suspend_dirt, 0, 100, "Suspend Dirt");
        range!(self.physics.borders_extend, 0, 10000, "Borders Extend");
        range!(self.landscape.max_wind, 0, 500, "Max Wind");
        range!(self.landscape.bumpiness, 0, 100, "Bumpiness");
        range!(self.landscape.slope, 0, 100, "Slope");
        range!(self.landscape.scanned_mountains_percent, 0, 100, "Scanned Mountains");
        range!(self.play.talk_probability, 0, 100, "Talk Probability");
        range!(self.weapons.arms_level, 0, 4, "Arms Level");
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("Cannot read {}: {e}", path.display())),
        };
        let settings: Self = toml::from_str(&text)
            .map_err(|e| format!("Invalid settings in {}: {e}", path.display()))?;
        settings.validate().map_err(|e| format!("Invalid settings in {}: {e}", path.display()))?;
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;
        let filename = path.file_name().ok_or("Settings path needs a filename")?;
        let mut temporary = filename.to_os_string();
        temporary.push(format!(".{}.tmp", std::process::id()));
        let temporary = parent.join(temporary);
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| format!("Cannot write {}: {e}", temporary.display()))?;
        let result = (|| {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|e| format!("Cannot save {}: {e}", path.display()))
    }
}

pub fn config_path(args: impl IntoIterator<Item = std::ffi::OsString>) -> Result<PathBuf, String> {
    let mut args = args.into_iter();
    let first = args.next();
    if args.next().is_some() {
        return Err("Usage: scorched [settings.toml]".into());
    }
    if let Some(path) = first {
        return Ok(path.into());
    }
    let directory = dirs::config_dir()
        .ok_or("Cannot find the configuration directory; pass a settings filename")?;
    Ok(directory.join("scorched/settings.toml"))
}

impl HardwareSettings {
    /// Fixed physics steps keep trajectories unchanged as drawing speed changes.
    pub fn shot_time(&self, dt: f32) -> f32 {
        if self.firing_delay == 0 {
            30.0
        } else {
            dt * 100.0 / self.firing_delay as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);
    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "scorched-settings-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn explicit_save_round_trips_all_settings_and_load_never_writes() {
        let directory = TestDirectory::new();
        let path = directory.0.join("nested/custom.toml");
        assert_eq!(Settings::load(&path).unwrap(), Settings::default());
        assert!(!path.exists());
        let mut settings = Settings::default();
        settings.players = 10;
        settings.rounds = 1000;
        settings.controllers[9] = Ctrl::Human;
        settings.economics.cash_at_start = 987654;
        settings.economics.interest_rate = 0.0123456;
        settings.economics.computers_buy = false;
        settings.economics.scoring = ScoringMode::Greedy;
        settings.weapons.tunneling = false;
        settings.hardware.firing_delay = 0;
        settings.hardware.falling_delay = 1000;
        settings.landscape.mountains_folder = Some(PathBuf::from("my mountains"));
        settings.play.attack_comments = Some(PathBuf::from("talk.txt"));
        settings.physics.walls = WallType::WrapAround;
        settings.play.mode = PlayMode::Simultaneous;
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), settings);
        // Session edits remain in memory until a second explicit save.
        settings.rounds = 17;
        assert_eq!(Settings::load(&path).unwrap().rounds, 1000);
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap().rounds, 17);
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn partial_files_inherit_defaults_and_invalid_files_report_errors() {
        let directory = TestDirectory::new();
        let path = directory.0.join("partial.toml");
        fs::write(&path, "players = 10\n[hardware]\nfalling_delay = 0\n").unwrap();
        let settings = Settings::load(&path).unwrap();
        assert_eq!(settings.players, 10);
        assert_eq!(settings.rounds, 10);
        assert_eq!(settings.hardware.firing_delay, 100);
        assert_eq!(settings.hardware.falling_delay, 0);
        for text in [
            "players = 11",
            "rounds = 0",
            "rounds = 1001",
            "[hardware]\nfiring_delay = 501",
            "[hardware]\nfalling_delay = 1001",
            "[physics]\ngravity = nan",
            "[economics]\ninterest_rate = inf",
            "[physics]\ngravity = 0.0",
            "[weapons]\narms_level = 5",
            "typo = true",
            "[hardware]\nfalling_delay = 'oops'",
            "[[invalid",
        ] {
            fs::write(&path, text).unwrap();
            assert!(Settings::load(&path).is_err(), "accepted invalid config: {text}");
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
    }

    #[test]
    fn failed_save_keeps_existing_config_and_reports_io_errors() {
        let directory = TestDirectory::new();
        let path = directory.0.join("settings.toml");
        Settings::default().save(&path).unwrap();
        let before = fs::read(&path).unwrap();
        let mut invalid = Settings::default();
        invalid.players = 0;
        assert!(invalid.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(Settings::default().save(&directory.0).is_err());
        assert!(Settings::load(&directory.0).is_err());
    }

    #[test]
    fn command_line_selects_exactly_one_config_file() {
        let path = config_path([std::ffi::OsString::from("other game.toml")]).unwrap();
        assert_eq!(path, PathBuf::from("other game.toml"));
        assert!(config_path(["one.toml".into(), "two.toml".into()]).is_err());
        assert!(config_path(std::iter::empty()).unwrap().ends_with("scorched/settings.toml"));
    }
}
