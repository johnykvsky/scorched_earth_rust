//! Optional weighted text-line speech; empty entries deliberately mean silence.
use crate::{
    settings::{PlaySettings, TalkingTanks},
    tank::Ctrl,
};
use std::{io::Read, path::Path};
const ATTACK: [&str; 8] = [
    "Incoming!",
    "Special delivery!",
    "Duck and cover!",
    "Say goodnight.",
    "Fore!",
    "This one's for you.",
    "Catch!",
    "Knock knock.",
];
const DEATH: [&str; 6] = [
    "Not like this!",
    "Tell mom I tried.",
    "I'll be back... maybe.",
    "Ouch.",
    "Was it something I said?",
    "Medic!",
];
pub struct Speech {
    pub attack: Vec<String>,
    pub death: Vec<String>,
}
impl Default for Speech {
    fn default() -> Self {
        Self {
            attack: ATTACK.iter().map(|s| s.to_string()).collect(),
            death: DEATH.iter().map(|s| s.to_string()).collect(),
        }
    }
}
impl Speech {
    pub fn load(settings: &PlaySettings, base: &Path) -> (Self, Vec<String>) {
        let mut speech = Self::default();
        let mut errors = Vec::new();
        for (path, destination) in [
            (&settings.attack_comments, &mut speech.attack),
            (&settings.death_comments, &mut speech.death),
        ] {
            if let Some(path) = path {
                let path = if path.is_absolute() {
                    path.clone()
                } else {
                    base.join(path)
                };
                let result: Result<Vec<String>, String> = (|| {
                    let mut bytes = Vec::new();
                    std::fs::File::open(&path)
                        .map_err(|e| e.to_string())?
                        .take(1_048_577)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    if bytes.len() > 1_048_576 {
                        return Err("file exceeds 1 MiB".into());
                    }
                    let text = String::from_utf8(bytes)
                        .map_err(|_| "file must contain UTF-8/ASCII text".to_string())?;
                    Ok(text
                        .lines()
                        .map(|line| line.chars().take(72).collect())
                        .collect::<Vec<String>>())
                })();
                match result {
                    Ok(lines) => *destination = lines,
                    Err(error) => errors.push(format!(
                        "Cannot load {}: {error}. Using built-in phrases.",
                        path.display()
                    )),
                }
            }
        }
        (speech, errors)
    }
    pub fn phrase(
        &self,
        ctrl: Ctrl,
        settings: &PlaySettings,
        death: bool,
        roll: u8,
        choice: usize,
    ) -> Option<String> {
        if settings.talking_tanks == TalkingTanks::Off
            || (settings.talking_tanks == TalkingTanks::Computers && ctrl == Ctrl::Human)
            || roll >= settings.talk_probability
        {
            return None;
        }
        let lines = if death { &self.death } else { &self.attack };
        if lines.is_empty() {
            return None;
        }
        let line = &lines[choice % lines.len()];
        (!line.trim().is_empty()).then(|| line.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    #[test]
    fn weighted_lines_blanks_and_probability_modes_are_preserved() {
        let folder = std::env::temp_dir().join(format!(
            "scorched-talk-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("talk.cfg"), "Hello\r\nHello\r\n\r\nBye\n").unwrap();
        let mut settings = crate::settings::Settings::default().play;
        settings.attack_comments = Some("talk.cfg".into());
        settings.talking_tanks = TalkingTanks::All;
        let (speech, errors) = Speech::load(&settings, &folder);
        assert!(errors.is_empty());
        assert_eq!(speech.attack, vec!["Hello", "Hello", "", "Bye"]);
        assert_eq!(
            speech.phrase(Ctrl::Human, &settings, false, 99, 0),
            Some("Hello".into())
        );
        assert!(speech.phrase(Ctrl::Human, &settings, false, 0, 2).is_none());
        settings.talk_probability = 0;
        assert!(speech.phrase(Ctrl::Human, &settings, false, 0, 0).is_none());
        settings.talk_probability = 100;
        settings.talking_tanks = TalkingTanks::Computers;
        assert!(speech.phrase(Ctrl::Human, &settings, false, 0, 0).is_none());
        assert!(speech
            .phrase(Ctrl::Ai(crate::ai::AiKind::Moron), &settings, false, 0, 0)
            .is_some());
        settings.attack_comments = Some("missing.cfg".into());
        let (fallback, errors) = Speech::load(&settings, &folder);
        assert_eq!(errors.len(), 1);
        assert_eq!(fallback.attack, Speech::default().attack);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
