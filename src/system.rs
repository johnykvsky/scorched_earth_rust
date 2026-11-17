//! Pausing System Menu. World mutations are executed by Game after confirmation.
use crate::{
    gfx::{Fb, Rgb},
    ui::MenuInput,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Clear,
    MassKill,
    Players,
    Teams,
    Sound,
    Save,
    Restore,
    NewGame,
    Quit,
}
impl Command {
    pub const ALL: [Self; 9] = [
        Self::Clear,
        Self::MassKill,
        Self::Players,
        Self::Teams,
        Self::Sound,
        Self::Save,
        Self::Restore,
        Self::NewGame,
        Self::Quit,
    ];
    fn label(self) -> &'static str {
        match self {
            Self::Clear => "C Clear Screen",
            Self::MassKill => "M Mass Kill",
            Self::Players => "P Reassign Players",
            Self::Teams => "T Reassign Teams",
            Self::Sound => "S Sound on/off",
            Self::Save => "V Save Game",
            Self::Restore => "R Restore Game",
            Self::NewGame => "N New Game",
            Self::Quit => "Q Quit",
        }
    }
    pub fn enabled(self, game: bool, round: bool, buying: bool) -> bool {
        match self {
            Self::Save => buying,
            Self::MassKill => round,
            Self::Clear | Self::Players | Self::Teams => game,
            _ => true,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Panel {
    #[default]
    Menu,
    Confirm(Command),
    Players,
    Teams,
    Message(String),
    File {
        command: Command,
        path: String,
    },
    Overwrite {
        path: String,
    },
}
#[derive(Default)]
pub struct System {
    pub panel: Panel,
    pub row: usize,
    pub can_save: bool,
}
pub enum Request {
    None,
    Close,
    Run(Command),
}
impl System {
    pub fn handle(&mut self, input: &MenuInput, game: bool, round: bool) -> Request {
        if let Panel::Confirm(command) = self.panel {
            if input.hotkey == Some('Y') || crate::ui::button_index(input, 2) == Some(0) {
                self.panel = Panel::Menu;
                return Request::Run(command);
            }
            if input.escape
                || input.hotkey == Some('N')
                || crate::ui::button_index(input, 2) == Some(1)
            {
                self.panel = Panel::Menu;
            }
            return Request::None;
        }
        if matches!(self.panel, Panel::Message(_)) {
            if input.escape || input.activate || input.hotkey.is_some() || input.click.is_some() {
                self.panel = Panel::Menu;
            }
            return Request::None;
        }
        if input.escape || input.hotkey == Some('D') {
            return Request::Close;
        }
        self.row = self.row.min(9);
        if input.up {
            self.row = (self.row + 9) % 10;
        }
        if input.down {
            self.row = (self.row + 1) % 10;
        }
        let mut activate = input.activate;
        if let Some(key) = input.hotkey {
            if let Some(row) = ['C', 'M', 'P', 'T', 'S', 'V', 'R', 'N', 'Q']
                .iter()
                .position(|&k| k == key)
            {
                self.row = row;
                activate = true;
            }
        }
        if let Some((x, y, 1)) = input.click {
            if (104..536).contains(&x) && (104..344).contains(&y) {
                self.row = ((y - 104) / 24) as usize;
                activate = true;
            }
        }
        if !activate {
            return Request::None;
        }
        if self.row == 9 {
            return Request::Close;
        }
        let command = Command::ALL[self.row];
        if !command.enabled(game, round, self.can_save) {
            self.panel = Panel::Message(
                if command == Command::Save {
                    "Save Game is available only from the buying screen."
                } else {
                    "This action is not available here."
                }
                .into(),
            );
            return Request::None;
        }
        if matches!(
            command,
            Command::MassKill | Command::NewGame | Command::Quit
        ) {
            self.panel = Panel::Confirm(command);
            Request::None
        } else {
            Request::Run(command)
        }
    }
    pub fn draw(&self, fb: &mut Fb, game: bool, round: bool) {
        match &self.panel {
            Panel::Menu => {
                fb.bevel(96, 64, 448, 324);
                fb.text(112, 76, "SYSTEM MENU", Rgb::BLACK);
                for row in 0..10 {
                    let y = 104 + row as i32 * 24;
                    if row == self.row {
                        fb.frame(104, y - 2, 432, 23, Rgb::DARK);
                    }
                    let enabled = row == 9 || Command::ALL[row].enabled(game, round, self.can_save);
                    fb.text(
                        112,
                        y,
                        if row == 9 {
                            "D Done / Resume"
                        } else {
                            Command::ALL[row].label()
                        },
                        if enabled {
                            Rgb::BLACK
                        } else {
                            Rgb(110, 110, 110)
                        },
                    );
                }
                fb.text(112, 364, "Esc/F1: resume", Rgb::DARK);
            }
            Panel::Confirm(command) => crate::ui::dialog(
                fb,
                &[
                    match command {
                        Command::MassKill => "Mass kill everyone?",
                        Command::NewGame => "Start a new game?",
                        _ => "Quit the game?",
                    }
                    .into(),
                    "Y: confirm   N/Esc: cancel".into(),
                ],
            ),
            Panel::Message(text) => {
                let mut lines = crate::ui::wrap_text(text, 58);
                lines.truncate(18);
                lines.push("Press a key to continue.".into());
                crate::ui::dialog(fb, &lines)
            }
            Panel::File { command, path } => {
                fb.bevel(60, 140, 520, 168);
                fb.text(
                    76,
                    156,
                    if *command == Command::Save {
                        "SAVE GAME"
                    } else {
                        "RESTORE GAME"
                    },
                    Rgb::BLACK,
                );
                fb.text(76, 184, "File (Home: clear):", Rgb::DARK);
                fb.bevel(72, 208, 496, 28);
                let tail: String = path
                    .chars()
                    .rev()
                    .take(58)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                fb.text(76, 218, &format!("{tail}|"), Rgb::BLACK);
                fb.text(76, 252, "Enter: apply   Esc: cancel", Rgb::DARK);
                fb.text(76, 276, "Relative paths use the saves folder.", Rgb::DARK);
                crate::ui::mouse_buttons(fb, &["Apply", "Cancel"]);
            }
            Panel::Overwrite { path } => {
                let mut lines = vec!["Overwrite this saved game?".into()];
                lines.extend(crate::ui::wrap_text(path, 58).into_iter().take(3));
                lines.push("Y: overwrite   N/Esc: cancel".into());
                crate::ui::dialog(fb, &lines);
                crate::ui::mouse_buttons(fb, &["Overwrite", "Cancel"]);
            }
            Panel::Players | Panel::Teams => {}
        }
        if matches!(self.panel, Panel::Confirm(_)) {
            crate::ui::mouse_buttons(fb, &["Confirm", "Cancel"]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(c: char) -> MenuInput {
        MenuInput {
            hotkey: Some(c),
            ..Default::default()
        }
    }
    #[test]
    fn destructive_commands_require_confirmation_and_cancel_without_running() {
        for (keycode, command) in [
            ('M', Command::MassKill),
            ('N', Command::NewGame),
            ('Q', Command::Quit),
        ] {
            let mut menu = System::default();
            assert!(matches!(
                menu.handle(&key(keycode), true, true),
                Request::None
            ));
            assert_eq!(menu.panel, Panel::Confirm(command));
            assert!(matches!(menu.handle(&key('N'), true, true), Request::None));
            assert_eq!(menu.panel, Panel::Menu);
            menu.handle(&key(keycode), true, true);
            assert!(matches!(menu.handle(&key('Y'),true,true),Request::Run(c) if c==command));
        }
    }
    #[test]
    fn save_requires_buying_restore_is_always_available_and_mass_kill_requires_a_round() {
        let mut menu = System::default();
        assert!(matches!(menu.handle(&key('V'), true, true), Request::None));
        assert!(matches!(menu.panel, Panel::Message(_)));
        menu.panel = Panel::Menu;
        assert!(matches!(
            menu.handle(&key('R'), false, false),
            Request::Run(Command::Restore)
        ));
        menu.can_save = true;
        assert!(matches!(
            menu.handle(&key('V'), true, false),
            Request::Run(Command::Save)
        ));
        let mut menu = System::default();
        assert!(matches!(menu.handle(&key('M'), true, false), Request::None));
        menu.panel = Panel::Menu;
        assert!(matches!(
            menu.handle(&key('S'), true, true),
            Request::Run(Command::Sound)
        ));
        assert!(matches!(
            menu.handle(
                &MenuInput {
                    escape: true,
                    ..Default::default()
                },
                true,
                true
            ),
            Request::Close
        ));
    }
}
