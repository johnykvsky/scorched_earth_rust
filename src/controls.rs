//! Modal battlefield panels and the common accessory actions behind every input path.
use crate::gfx::{Fb, Rgb, W};
use crate::tank::{ItemUse, Tank, GUIDANCE, SHIELDS};
use crate::ui::{dialog, KeyRep, MenuInput, HUD_H};
use crate::weapons::{ItemId, ITEMS};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Closed,
    Control,
    Inventory,
    Threshold,
    Message(Vec<String>),
    Retreat,
}
#[derive(Default)]
pub struct Controls {
    pub panel: Panel,
    pub selected: usize,
    pub threshold: String,
    pub defensive_only: bool,
    pub clicked: bool,
    resume: Option<Panel>,
    keys: KeyRep,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Consumed,
    Status,
    Info(usize),
    Retreat,
    Move,
}

const ROW_KEYS: [char; 10] = ['B', 'P', 'V', 'S', 'E', 'G', '-', 'F', 'I', 'D'];

pub fn owned(tank: &Tank) -> Vec<ItemId> {
    ITEMS
        .iter()
        .filter(|item| {
            tank.inventory.count(item.id) > 0
                || (item.id == ItemId::FuelTank && tank.fuel_available() > 0)
        })
        .map(|item| item.id)
        .collect()
}
impl Controls {
    pub fn is_closed(&self) -> bool {
        self.panel == Panel::Closed
    }
    pub fn message(&mut self, lines: Vec<String>) {
        self.resume = Some(self.panel.clone());
        self.panel = Panel::Message(lines);
    }
    pub fn items(&self, tank: &Tank) -> Vec<ItemId> {
        owned(tank)
            .into_iter()
            .filter(|id| {
                !self.defensive_only
                    || SHIELDS.contains(id)
                    || matches!(
                        id,
                        ItemId::Battery | ItemId::Parachute | ItemId::AutoDefense
                    )
            })
            .collect()
    }
    pub fn update(&mut self, tank: &mut Tank, dt: f32, status_bar: bool) -> Action {
        self.keys.dt = dt;
        let input = MenuInput::poll(&mut self.keys);
        self.handle_with_click(tank, &input, status_bar)
    }
    pub(crate) fn handle_with_click(
        &mut self,
        tank: &mut Tank,
        input: &MenuInput,
        status_bar: bool,
    ) -> Action {
        let before = (self.panel.clone(), self.selected, self.threshold.clone());
        let supplies = (tank.inventory.clone(), tank.equipment.clone(), tank.health);
        let action = self.handle(tank, input, status_bar);
        self.clicked = before != (self.panel.clone(), self.selected, self.threshold.clone())
            || supplies != (tank.inventory.clone(), tank.equipment.clone(), tank.health)
            || matches!(
                action,
                Action::Status | Action::Info(_) | Action::Retreat | Action::Move
            );
        action
    }
    pub(crate) fn handle(
        &mut self,
        tank: &mut Tank,
        input: &MenuInput,
        status_bar: bool,
    ) -> Action {
        match &self.panel {
            Panel::Message(_) => {
                if input.escape || input.activate || input.hotkey.is_some() || input.click.is_some()
                {
                    self.panel = self.resume.take().unwrap_or(Panel::Closed);
                }
                return Action::Consumed;
            }
            Panel::Retreat => {
                if input.hotkey == Some('Y') || crate::ui::button_index(input, 2) == Some(0) {
                    self.panel = Panel::Closed;
                    return Action::Retreat;
                }
                if input.escape
                    || input.hotkey == Some('N')
                    || crate::ui::button_index(input, 2) == Some(1)
                {
                    self.panel = Panel::Closed;
                }
                return Action::Consumed;
            }
            Panel::Threshold => {
                let button = crate::ui::button_index(input, 4);
                if input.escape || button == Some(3) {
                    self.panel = Panel::Control;
                    return Action::Consumed;
                }
                if input.backspace {
                    self.threshold.pop();
                }
                if let Some(c @ '0'..='9') = input.hotkey {
                    if self.threshold.len() < 3 {
                        self.threshold.push(c);
                    }
                }
                if input.left
                    || input.down
                    || input.right
                    || input.up
                    || matches!(button, Some(0 | 1))
                {
                    let value = self
                        .threshold
                        .parse::<i32>()
                        .unwrap_or(i32::from(tank.equipment.parachute_threshold));
                    let delta = if input.left || input.down || button == Some(0) {
                        -1
                    } else {
                        1
                    };
                    self.threshold = (value + delta).clamp(1, 100).to_string();
                }
                if input.activate || button == Some(2) {
                    if self.threshold.is_empty() {
                        self.panel = Panel::Control;
                    } else if let Ok(value @ 1..=100) = self.threshold.parse::<u8>() {
                        tank.equipment.parachute_threshold = value;
                        self.panel = Panel::Control;
                    }
                }
                return Action::Consumed;
            }
            _ => {}
        }
        if !self.is_closed() && (input.escape || input.hotkey == Some('D')) {
            self.panel = if self.defensive_only && self.panel == Panel::Inventory {
                Panel::Control
            } else {
                Panel::Closed
            };
            return Action::Consumed;
        }
        if self.panel == Panel::Inventory {
            let button = crate::ui::button_index(input, 3);
            if button == Some(2) {
                self.panel = if self.defensive_only {
                    Panel::Control
                } else {
                    Panel::Closed
                };
                return Action::Consumed;
            }
            let items = self.items(tank);
            if items.is_empty() {
                self.panel = Panel::Control;
                return Action::Consumed;
            }
            self.selected = self.selected.min(items.len() - 1); // Baby Missiles are always owned.
            if input.up {
                self.selected = (self.selected + items.len() - 1) % items.len();
            }
            if input.down {
                self.selected = (self.selected + 1) % items.len();
            }
            if input.home {
                self.selected = 0;
            }
            if input.end {
                self.selected = items.len() - 1;
            }
            let pages = items.len().div_ceil(12);
            if input.page_up || input.page_down || matches!(button, Some(0 | 1)) {
                let page = (self.selected / 12
                    + if input.page_down || button == Some(1) {
                        1
                    } else {
                        pages - 1
                    })
                    % pages;
                self.selected = (page * 12 + self.selected % 12).min(items.len() - 1);
            }
            let mut activate = input.activate;
            if let Some((x, y, 1)) = input.click {
                let index = self.selected / 12 * 12 + ((y - 100).max(0) / 22) as usize;
                if (64..576).contains(&x) && (100..364).contains(&y) && index < items.len() {
                    self.selected = index;
                    activate = true;
                }
            }
            if activate {
                let id = items[self.selected];
                if id == ItemId::FuelTank {
                    return self.request_movement(tank);
                }
                self.apply(tank, id);
                if tank.inventory.can_select(id) {
                    self.panel = Panel::Closed;
                }
            }
            if input.hotkey == Some('I') {
                self.panel = if self.defensive_only {
                    Panel::Control
                } else {
                    Panel::Closed
                };
            }
            return Action::Consumed;
        }
        let mut key = input.hotkey;
        if self.panel == Panel::Control {
            if let Some(button) =
                crate::ui::button_index(input, if self.defensive_only { 1 } else { 3 })
            {
                key = Some(if self.defensive_only {
                    'D'
                } else {
                    ['R', 'U', 'D'][button]
                });
            }
            self.selected = self.selected.min(ROW_KEYS.len() - 1);
            if input.up {
                self.selected = (self.selected + ROW_KEYS.len() - 1) % ROW_KEYS.len();
            }
            if input.down {
                self.selected = (self.selected + 1) % ROW_KEYS.len();
            }
            if input.activate || input.left || input.right {
                key = Some(ROW_KEYS[self.selected]);
            }
            if let Some((x, y, 1)) = input.click {
                if (80..560).contains(&x) && (116..336).contains(&y) {
                    self.selected = ((y - 116) / 22) as usize;
                    key = Some(ROW_KEYS[self.selected]);
                }
            }
            if key == Some('T') {
                key = Some('-');
            }
        } else if status_bar {
            if let Some((x, y, 1)) = input.click {
                if (HUD_H..HUD_H + 22).contains(&y) {
                    key = match x {
                        100..=171 => Some('B'),
                        172..=243 => Some('P'),
                        244..=315 => Some('S'),
                        316..=363 => Some('E'),
                        364..=435 => Some('G'),
                        436..=507 => Some('-'),
                        508..=639 => Some('F'),
                        _ => None,
                    };
                }
            }
        }
        if self.defensive_only
            && key.is_some_and(|key| !matches!(key, 'B' | 'P' | 'V' | 'S' | 'E' | 'I' | 'D'))
        {
            return Action::Consumed;
        }
        match key {
            Some('T') => {
                self.panel = Panel::Control;
                self.selected = 0;
            }
            Some('I') => {
                self.panel = Panel::Inventory;
                self.selected = 0;
            }
            Some('B') => {
                self.apply(tank, ItemId::Battery);
            }
            Some('P') => {
                self.apply(tank, ItemId::Parachute);
            }
            Some('V') if self.panel == Panel::Control => {
                self.panel = Panel::Threshold;
                self.threshold.clear();
            }
            Some('S') => {
                tank.cycle_supply(true, input.alt || input.left || input.shift);
                if self.is_closed() {
                    self.panel = Panel::Control;
                    self.selected = 3;
                }
            }
            Some('G') => {
                tank.cycle_supply(false, input.alt || input.left || input.shift);
                if self.is_closed() {
                    self.panel = Panel::Control;
                    self.selected = 5;
                }
            }
            Some('E') => {
                if let Some(id) = tank.equipment.shield {
                    self.apply(tank, id);
                }
            }
            Some('-') => {
                self.apply(tank, ItemId::ContactTrigger);
            }
            Some('F') => {
                return self.request_movement(tank);
            }
            Some('D') if self.panel == Panel::Control => self.panel = Panel::Closed,
            Some('U') => return Action::Status,
            Some('K') => self.message(vec![
                "No kibitzing, please.".into(),
                "Press a key to continue.".into(),
            ]),
            Some('R') => self.panel = Panel::Retreat,
            Some(c @ '0'..='9') => {
                return Action::Info(if c == '0' {
                    9
                } else {
                    c as usize - '1' as usize
                })
            }
            _ => {
                return if self.is_closed() {
                    Action::None
                } else {
                    Action::Consumed
                }
            }
        }
        Action::Consumed
    }
    fn apply(&mut self, tank: &mut Tank, id: ItemId) {
        if id == ItemId::AutoDefense && tank.inventory.count(id) > 0 {
            self.message(vec![
                "Auto Defense enabled".into(),
                "Defensive controls open before each round.".into(),
                "This item lasts for the whole game.".into(),
            ]);
            return;
        }
        if let ItemUse::Later(batch) = tank.use_item(id) {
            self.message(vec![
                id.item().name.into(),
                format!("Effect arrives in batch {batch}."),
                "Item kept in inventory.".into(),
                "Press a key to continue.".into(),
            ]);
        }
    }
    fn request_movement(&mut self, tank: &Tank) -> Action {
        if !self.defensive_only && tank.shape.mobile() && tank.fuel_available() > 0 {
            self.panel = Panel::Closed;
            Action::Move
        } else {
            self.message(vec![
                if tank.shape.mobile() {
                    "No fuel available.".into()
                } else {
                    "Fixed emplacements cannot move.".into()
                },
                "Press a key to continue.".into(),
            ]);
            Action::Consumed
        }
    }
}

fn selection(id: Option<ItemId>) -> &'static str {
    id.map_or("None", |id| id.item().name)
}
pub fn draw_status(fb: &mut Fb, tank: &Tank) {
    let count = |ids: &[ItemId]| {
        ids.iter()
            .map(|&id| u32::from(tank.inventory.count(id)))
            .sum::<u32>()
    };
    fb.bevel(0, HUD_H, W, 22);
    let fields = [
        (4, format!("Max:{:4}", tank.max_power())),
        (100, format!("{}[b]", tank.inventory.count(ItemId::Battery))),
        (
            172,
            format!(
                "{}[p]{}",
                tank.inventory.count(ItemId::Parachute),
                if tank.equipment.parachutes_deployed {
                    "+"
                } else {
                    "-"
                }
            ),
        ),
        (244, format!("{}[s]", count(&SHIELDS))),
        (316, format!("{}%e", tank.shield_percent())),
        (364, format!("{}[g]", count(&GUIDANCE))),
        (
            436,
            format!(
                "{}[-]{}",
                tank.inventory.count(ItemId::ContactTrigger),
                if tank.equipment.triggers { "+" } else { "-" }
            ),
        ),
        (508, format!("{}[f]", tank.fuel_available())),
    ];
    for (x, text) in fields {
        fb.text(x, HUD_H + 3, &text, Rgb::BLACK);
    }
}

pub fn draw(fb: &mut Fb, tank: &Tank, state: &Controls) {
    match &state.panel {
        Panel::Closed => {}
        Panel::Control => {
            fb.bevel(72, 70, 496, 300);
            fb.text(
                88,
                80,
                if state.defensive_only {
                    "AUTO DEFENSE - BEFORE COMBAT"
                } else {
                    "TANK CONTROL PANEL"
                },
                Rgb::BLACK,
            );
            fb.text(
                88,
                100,
                &format!("{}   Health: {}", tank.name, tank.health),
                Rgb::BLACK,
            );
            let count = |id| tank.inventory.count(id);
            let rows = [
                (
                    format!("B Battery: {} (+10 health)", count(ItemId::Battery)),
                    count(ItemId::Battery) > 0 && tank.health < 100,
                ),
                (
                    format!(
                        "P Parachutes: {} ({})",
                        count(ItemId::Parachute),
                        if tank.equipment.parachutes_deployed {
                            "deployed"
                        } else {
                            "passive"
                        }
                    ),
                    count(ItemId::Parachute) > 0,
                ),
                (
                    format!("V Safety threshold: {}", tank.equipment.parachute_threshold),
                    true,
                ),
                (
                    format!("S Shield: {}", selection(tank.equipment.shield)),
                    SHIELDS.iter().any(|&id| count(id) > 0),
                ),
                (
                    if let Some(shield) = &tank.equipment.active_shield {
                        format!(
                            "E Engage - {} {}%{}",
                            shield.item.item().name,
                            shield.percent(),
                            if shield.laser_immune() {
                                " (laser-proof)"
                            } else {
                                ""
                            }
                        )
                    } else {
                        "E Engage selected shield".into()
                    },
                    tank.equipment.shield.is_some_and(|id| count(id) > 0),
                ),
                (
                    format!("G Guidance: {}", selection(tank.equipment.guidance)),
                    !state.defensive_only && GUIDANCE.iter().any(|&id| count(id) > 0),
                ),
                (
                    format!(
                        "T Triggers: {}",
                        if tank.equipment.triggers {
                            "armed"
                        } else {
                            "off"
                        }
                    ),
                    !state.defensive_only && count(ItemId::ContactTrigger) > 0,
                ),
                (
                    format!("F Move: {} fuel", tank.fuel_available()),
                    !state.defensive_only && tank.shape.mobile() && tank.fuel_available() > 0,
                ),
                ("I Inventory".into(), true),
                ("D Done".into(), true),
            ];
            for (i, (text, enabled)) in rows.iter().enumerate() {
                let y = 118 + i as i32 * 22;
                if state.selected == i {
                    fb.rect(80, y - 2, 480, 22, Rgb(0, 0, 140));
                }
                let color = if !enabled {
                    Rgb::DARK
                } else if state.selected == i {
                    Rgb::WHITE
                } else {
                    Rgb::BLACK
                };
                fb.text(88, y, text, color);
            }
            fb.text(88, 348, "Arrows/Enter or hotkey   Esc: close", Rgb::DARK);
        }
        Panel::Inventory => {
            fb.bevel(56, 62, 528, 356);
            fb.text(72, 76, "INVENTORY - select an item to use", Rgb::BLACK);
            let items = state.items(tank);
            let start = state.selected / 12 * 12;
            for (i, &id) in items.iter().enumerate().skip(start).take(12) {
                let y = 102 + (i - start) as i32 * 22;
                if state.selected == i {
                    fb.rect(64, y - 2, 512, 22, Rgb(0, 0, 140));
                }
                let color = if state.selected == i {
                    Rgb::WHITE
                } else {
                    Rgb::BLACK
                };
                draw_item_icon(fb, id, 76, y + 8, tank.color.scale(0.7));
                fb.text(96, y, id.item().name, color);
                fb.text(
                    440,
                    y,
                    &if id == ItemId::FuelTank {
                        format!("{}u", tank.fuel_available())
                    } else {
                        format!("{:2}", tank.inventory.count(id))
                    },
                    color,
                );
            }
            fb.text(
                72,
                374,
                &format!(
                    "Page {}/{}   Space/Enter: use",
                    start / 12 + 1,
                    items.len().div_ceil(12)
                ),
                Rgb::BLACK,
            );
        }
        Panel::Threshold => dialog(
            fb,
            &[
                "Parachute safety threshold (1-100)".into(),
                format!(
                    "Damage: {}",
                    if state.threshold.is_empty() {
                        tank.equipment.parachute_threshold.to_string()
                    } else {
                        state.threshold.clone()
                    }
                ),
                "Type a number or use arrows.".into(),
                "Enter: apply   Esc: cancel".into(),
            ],
        ),
        Panel::Message(lines) => dialog(fb, lines),
        Panel::Retreat => dialog(
            fb,
            &[
                "Retreat from this round?".into(),
                "Y: retreat   N / Esc: cancel".into(),
            ],
        ),
    }
    match state.panel {
        Panel::Control if !state.defensive_only => {
            crate::ui::mouse_buttons(fb, &["Retreat", "Status", "Close"])
        }
        Panel::Control => crate::ui::mouse_buttons(fb, &["Close"]),
        Panel::Inventory => crate::ui::mouse_buttons(fb, &["Prev page", "Next page", "Close"]),
        Panel::Threshold => crate::ui::mouse_buttons(fb, &["-", "+", "Apply", "Cancel"]),
        Panel::Retreat => crate::ui::mouse_buttons(fb, &["Retreat", "Cancel"]),
        _ => {}
    }
}

fn draw_item_icon(fb: &mut Fb, id: ItemId, x: i32, y: i32, color: Rgb) {
    if id == ItemId::Battery {
        fb.rect(x - 5, y - 6, 10, 12, color);
        fb.line(x - 3, y, x + 3, y, Rgb::WHITE);
        fb.line(x, y - 3, x, y + 3, Rgb::WHITE);
    } else if id == ItemId::Parachute {
        fb.line(x - 7, y - 2, x + 7, y - 2, color);
        fb.line(x - 7, y - 2, x, y - 7, color);
        fb.line(x + 7, y - 2, x, y - 7, color);
        fb.line(x - 7, y - 2, x, y + 6, color);
        fb.line(x + 7, y - 2, x, y + 6, color);
    } else if SHIELDS.contains(&id) {
        fb.frame(x - 5, y - 7, 11, 10, color);
        fb.line(x - 5, y + 3, x, y + 7, color);
        fb.line(x + 5, y + 3, x, y + 7, color);
    } else if GUIDANCE.contains(&id) {
        fb.line(x - 7, y, x + 7, y, color);
        fb.line(x, y - 7, x, y + 7, color);
        fb.frame(x - 4, y - 4, 9, 9, color);
    } else {
        fb.rect(x - 5, y - 3, 10, 9, color);
        fb.line(x, y - 3, x + 5, y - 7, color);
    }
}

pub fn draw_parachute(fb: &mut Fb, tank: &Tank) {
    let (x, y) = (tank.x, tank.y - 28);
    for dx in -14..=14 {
        let top = y - ((196 - dx * dx) as f32).sqrt() as i32 / 2;
        fb.line(x + dx, top, x + dx, y, Rgb::WHITE);
    }
    for dx in [-14, -7, 7, 14] {
        fb.line(x + dx, y, x, tank.y - 8, tank.color);
    }
}

pub fn draw_retreat(fb: &mut Fb, tank: &Tank, time: f32) {
    let x = tank.x + (time * 150.0) as i32;
    let y = tank.y - 28 - (time * 65.0) as i32;
    fb.rect(x - 15, y - 5, 30, 12, Rgb(70, 110, 60));
    fb.rect(x + 5, y - 3, 8, 6, Rgb(100, 200, 255));
    fb.line(x - 15, y, x - 32, y - 5, Rgb::WHITE);
    fb.line(x - 32, y - 5, x - 32, y - 12, Rgb::WHITE);
    fb.line(x, y - 5, x, y - 12, Rgb::WHITE);
    let rotor = if (time * 30.0) as i32 % 2 == 0 {
        27
    } else {
        13
    };
    fb.line(x - rotor, y - 13, x + rotor, y - 13, Rgb::WHITE);
    fb.line(x - 16, y + 10, x + 16, y + 10, Rgb::WHITE);
    if time < 1.0 {
        let blast = crate::weapons::Explosion::new(
            tank.center(),
            30.0,
            crate::weapons::Damage::Death,
            0,
            false,
        );
        let mut blast = blast;
        blast.t = time * blast.dur;
        crate::ui::draw_explosion(fb, &blast);
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
    fn mouse_controls_apply_threshold_page_inventory_and_confirm_retreat() {
        let mut tank = crate::shop::tests::tank(0);
        for item in &ITEMS {
            tank.inventory.add(item.id, 1);
        }
        let click = |x, y| MenuInput {
            click: Some((x, y, 1)),
            ..Default::default()
        };
        let mut state = Controls {
            panel: Panel::Control,
            ..Default::default()
        };
        state.handle(&mut tank, &click(150, 116 + 2 * 22), false);
        assert_eq!(state.panel, Panel::Threshold);
        let before = tank.equipment.parachute_threshold;
        state.handle(&mut tank, &click(100, 410), false);
        state.handle(&mut tank, &click(380, 410), false);
        assert_eq!(
            tank.equipment.parachute_threshold,
            before.saturating_sub(1).max(1)
        );
        assert_eq!(state.panel, Panel::Control);
        state.handle(&mut tank, &click(150, 116 + 8 * 22), false);
        assert_eq!(state.panel, Panel::Inventory);
        state.handle(&mut tank, &click(320, 410), false);
        assert_eq!(state.selected, 12);
        state.handle(&mut tank, &click(500, 410), false);
        assert!(state.is_closed());
        state.panel = Panel::Control;
        state.handle(&mut tank, &click(100, 410), false);
        assert_eq!(state.panel, Panel::Retreat);
        assert_eq!(
            state.handle(&mut tank, &click(500, 410), false),
            Action::Consumed
        );
        assert!(state.is_closed());
        state.panel = Panel::Retreat;
        assert_eq!(
            state.handle(&mut tank, &click(100, 410), false),
            Action::Retreat
        );
        state.panel = Panel::Control;
        state.defensive_only = true;
        state.handle(&mut tank, &click(100, 410), false);
        assert!(state.is_closed());
    }

    #[test]
    fn panel_click_feedback_is_quiet_on_idle_and_detects_navigation_and_supply_use() {
        let mut tank = crate::shop::tests::tank(0);
        let mut state = Controls::default();
        state.handle_with_click(&mut tank, &MenuInput::default(), false);
        assert!(!state.clicked);
        state.handle_with_click(&mut tank, &key('T'), false);
        assert!(state.clicked);
        state.handle_with_click(&mut tank, &MenuInput::default(), false);
        assert!(!state.clicked);
        state.handle_with_click(
            &mut tank,
            &MenuInput {
                down: true,
                ..Default::default()
            },
            false,
        );
        assert!(state.clicked);
        tank.health = 80;
        tank.inventory.add(ItemId::Battery, 1);
        state.handle_with_click(&mut tank, &key('B'), false);
        assert!(state.clicked);
        assert_eq!(tank.health, 90);
        state.handle_with_click(&mut tank, &key('B'), false);
        assert!(!state.clicked); // Empty stock produces no action.
    }

    #[test]
    fn battery_parachute_and_trigger_actions_share_panel_inventory_and_status_paths() {
        let mut tank = crate::shop::tests::tank(0);
        tank.health = 75;
        tank.inventory.add(ItemId::Battery, 4);
        tank.inventory.add(ItemId::Parachute, 2);
        tank.inventory.add(ItemId::ContactTrigger, 2);
        let mut state = Controls::default();
        assert_eq!(state.handle(&mut tank, &key('B'), false), Action::Consumed);
        assert_eq!(tank.health, 85);
        state.handle(&mut tank, &key('T'), false);
        assert_eq!(state.panel, Panel::Control);
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.health, 95);
        state.handle(&mut tank, &key('P'), false);
        assert!(!tank.equipment.parachutes_deployed);
        state.handle(&mut tank, &key('T'), false); // panel T means triggers
        assert!(tank.equipment.triggers);
        assert_eq!(tank.inventory.count(ItemId::ContactTrigger), 2);
        state.handle(&mut tank, &key('I'), false);
        assert_eq!(state.panel, Panel::Inventory);
        state.selected = owned(&tank)
            .iter()
            .position(|&id| id == ItemId::Battery)
            .unwrap();
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.health, 100);
        assert_eq!(tank.inventory.count(ItemId::Battery), 1);
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.inventory.count(ItemId::Battery), 1); // No waste at full health.
        state.handle(
            &mut tank,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
            false,
        );
        assert!(state.is_closed());
        state.handle(
            &mut tank,
            &MenuInput {
                click: Some((180, HUD_H + 3, 1)),
                ..Default::default()
            },
            true,
        );
        assert!(tank.equipment.parachutes_deployed);
        assert_eq!(tank.inventory.count(ItemId::Parachute), 2);
        tank.health = 90;
        state.handle(
            &mut tank,
            &MenuInput {
                click: Some((110, HUD_H + 3, 1)),
                ..Default::default()
            },
            true,
        );
        assert_eq!(tank.health, 100);
        assert_eq!(tank.inventory.count(ItemId::Battery), 0);
    }

    #[test]
    fn owned_selection_future_effects_and_modal_input_preserve_stock() {
        let mut tank = crate::shop::tests::tank(0);
        tank.inventory.add(ItemId::Shield, 2);
        tank.inventory.add(ItemId::SuperMag, 1);
        tank.inventory.add(ItemId::HeatGuidance, 6);
        tank.inventory.add(ItemId::FuelTank, 10);
        let mut state = Controls::default();
        state.handle(&mut tank, &key('S'), false);
        assert_eq!(tank.equipment.shield, Some(ItemId::Shield));
        state.handle(&mut tank, &key('S'), false);
        assert_eq!(tank.equipment.shield, Some(ItemId::SuperMag));
        state.handle(&mut tank, &key('E'), false);
        assert_eq!(state.panel, Panel::Control);
        assert_eq!(tank.inventory.count(ItemId::SuperMag), 0);
        assert_eq!(
            tank.equipment.active_shield.as_ref().unwrap().item,
            ItemId::SuperMag
        );
        // Activating a panel row is consumed rather than passed through to fire.
        assert_eq!(
            state.handle(
                &mut tank,
                &MenuInput {
                    activate: true,
                    ..Default::default()
                },
                false
            ),
            Action::Consumed
        );
        assert_eq!(state.panel, Panel::Control);
        state.handle(&mut tank, &key('G'), false);
        assert_eq!(tank.equipment.guidance, Some(ItemId::HeatGuidance));
        state.handle(&mut tank, &key('G'), false);
        assert_eq!(tank.equipment.guidance, None);
        assert_eq!(state.handle(&mut tank, &key('F'), false), Action::Move);
        assert!(state.is_closed());
        assert_eq!(tank.inventory.count(ItemId::FuelTank), 10);
        state.handle(&mut tank, &key('T'), false);
        assert_eq!(state.panel, Panel::Control);
        state.handle(
            &mut tank,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(state.handle(&mut tank, &key('R'), false), Action::Consumed);
        assert_eq!(state.panel, Panel::Retreat);
        assert_eq!(state.handle(&mut tank, &key('N'), false), Action::Consumed);
        assert!(state.is_closed());
        state.handle(&mut tank, &key('R'), false);
        assert_eq!(state.handle(&mut tank, &key('Y'), false), Action::Retreat);
    }

    #[test]
    fn shift_s_reverses_shield_selection_without_consuming_it() {
        let mut tank = crate::shop::tests::tank(0);
        tank.inventory.add(ItemId::Shield, 1);
        tank.inventory.add(ItemId::SuperMag, 1);
        let mut controls = Controls::default();
        controls.handle(&mut tank, &key('S'), false);
        assert_eq!(tank.equipment.shield, Some(ItemId::Shield));
        controls.handle(
            &mut tank,
            &MenuInput {
                hotkey: Some('S'),
                shift: true,
                ..Default::default()
            },
            false,
        );
        assert!(tank.equipment.shield.is_none());
        controls.handle(
            &mut tank,
            &MenuInput {
                hotkey: Some('S'),
                shift: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.equipment.shield, Some(ItemId::SuperMag));
        assert_eq!(tank.inventory.count(ItemId::SuperMag), 1);
    }

    #[test]
    fn movement_opens_from_shortcut_panel_inventory_and_status_without_spending_stock() {
        for path in 0..4 {
            let mut tank = crate::shop::tests::tank(0);
            tank.inventory.add(ItemId::FuelTank, 1);
            let mut state = Controls::default();
            let input = match path {
                0 => key('F'),
                1 => {
                    state.panel = Panel::Control;
                    state.selected = 7;
                    MenuInput {
                        activate: true,
                        ..Default::default()
                    }
                }
                2 => {
                    state.panel = Panel::Inventory;
                    state.selected = state
                        .items(&tank)
                        .iter()
                        .position(|id| *id == ItemId::FuelTank)
                        .unwrap();
                    MenuInput {
                        activate: true,
                        ..Default::default()
                    }
                }
                _ => MenuInput {
                    click: Some((520, HUD_H + 5, 1)),
                    ..Default::default()
                },
            };
            assert_eq!(state.handle(&mut tank, &input, true), Action::Move);
            assert!(state.is_closed());
            assert_eq!(tank.fuel_available(), 10);
        }
        let mut tank = crate::shop::tests::tank(0);
        tank.equipment.fuel = 7;
        let mut state = Controls::default();
        assert!(state.items(&tank).contains(&ItemId::FuelTank));
        assert_eq!(state.handle(&mut tank, &key('F'), false), Action::Move);
        tank.shape = crate::tank::Shape::Bunker;
        assert_eq!(state.handle(&mut tank, &key('F'), false), Action::Consumed);
        assert!(matches!(state.panel, Panel::Message(_)));
        assert_eq!(tank.equipment.fuel, 7);
    }

    #[test]
    fn threshold_edits_validate_apply_and_cancel_without_changing_aim() {
        let mut tank = crate::shop::tests::tank(0);
        let mut state = Controls::default();
        state.handle(&mut tank, &key('T'), false);
        state.handle(&mut tank, &key('V'), false);
        for c in ['1', '0', '0'] {
            state.handle(&mut tank, &key(c), false);
        }
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.equipment.parachute_threshold, 100);
        state.handle(&mut tank, &key('V'), false);
        state.handle(&mut tank, &key('0'), false);
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(state.panel, Panel::Threshold);
        state.handle(
            &mut tank,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.equipment.parachute_threshold, 100);
        assert_eq!(tank.power, 400);
        assert_eq!(tank.angle, 90);
        state.handle(&mut tank, &key('V'), false);
        state.handle(
            &mut tank,
            &MenuInput {
                down: true,
                ..Default::default()
            },
            false,
        );
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.equipment.parachute_threshold, 99);
    }

    #[test]
    fn inventory_pages_wrap_select_weapons_and_missing_supplies_do_nothing() {
        let mut tank = crate::shop::tests::tank(0);
        let mut state = Controls::default();
        state.handle(&mut tank, &key('B'), false);
        state.handle(&mut tank, &key('P'), false);
        assert_eq!(tank.health, 100);
        assert!(tank.equipment.parachutes_deployed);
        for item in &ITEMS {
            tank.inventory.add(item.id, 1);
        }
        state.handle(&mut tank, &key('I'), false);
        state.handle(
            &mut tank,
            &MenuInput {
                page_up: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(state.selected, 36);
        state.handle(
            &mut tank,
            &MenuInput {
                page_down: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(state.selected, 0);
        state.handle(
            &mut tank,
            &MenuInput {
                down: true,
                ..Default::default()
            },
            false,
        );
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.weapon, ItemId::Missile);
        assert_eq!(tank.inventory.count(ItemId::Missile), 1);
        assert!(state.is_closed());
    }
}

#[cfg(test)]
mod defense_tests {
    use super::*;
    #[test]
    fn precombat_controls_filter_inventory_and_block_offensive_actions_without_consuming_items() {
        let mut tank = crate::shop::tests::tank(0);
        for item in &ITEMS {
            tank.inventory.add(item.id, 1);
        }
        let mut state = Controls::default();
        state.defensive_only = true;
        state.panel = Panel::Control;
        for key in ['G', '-', 'F', 'R', '1', 'K'] {
            assert_eq!(
                state.handle(
                    &mut tank,
                    &MenuInput {
                        hotkey: Some(key),
                        ..Default::default()
                    },
                    false
                ),
                Action::Consumed
            );
            assert_eq!(state.panel, Panel::Control);
        }
        assert!(tank.equipment.guidance.is_none() && !tank.equipment.triggers);
        state.handle(
            &mut tank,
            &MenuInput {
                hotkey: Some('I'),
                ..Default::default()
            },
            false,
        );
        assert_eq!(state.panel, Panel::Inventory);
        let items = state.items(&tank);
        assert!(!items.contains(&ItemId::BabyMissile));
        assert!(!items.contains(&ItemId::FuelTank));
        assert!(items.contains(&ItemId::Shield) && items.contains(&ItemId::Battery));
        state.selected = items.iter().position(|&id| id == ItemId::Shield).unwrap();
        state.handle(
            &mut tank,
            &MenuInput {
                activate: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(tank.shield_percent(), 100);
        assert_eq!(tank.inventory.count(ItemId::Shield), 0);
        state.handle(
            &mut tank,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
            false,
        );
        assert_eq!(state.panel, Panel::Control);
        state.handle(
            &mut tank,
            &MenuInput {
                escape: true,
                ..Default::default()
            },
            false,
        );
        assert!(state.is_closed());
        assert_eq!(tank.inventory.count(ItemId::AutoDefense), 1);
    }
}
