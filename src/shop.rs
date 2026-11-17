//! Buying, selling and the persistent per-item free market.
use crate::economy::{base_prices, Money};
use crate::settings::Settings;
use crate::tank::Tank;
use crate::weapons::{Category, ItemId, ITEMS, ITEM_COUNT};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const PAGE_SIZE: usize = 12;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Buy,
    Inventory,
    Sell {
        item: ItemId,
        quantity: u8,
    },
    Status,
}

pub struct Shop {
    pub player: usize,
    pub selected: usize,
    pub category: Category,
    pub panel: Panel,
}
impl Default for Shop {
    fn default() -> Self {
        Self {
            player: 0,
            selected: 0,
            category: Category::Weapon,
            panel: Panel::Buy,
        }
    }
}

/// Full bundles use the advertised price. Remaining spaces cost 120% per unit,
/// rounded to the nearest dollar, rather than charging for unusable surplus.
pub fn purchase(tank: &Tank, id: ItemId, prices: &[u32; ITEM_COUNT]) -> Option<(u8, Money)> {
    if id == ItemId::FuelTank && !tank.shape.mobile() {
        return None;
    }
    if id == ItemId::BabyMissile || (id == ItemId::AutoDefense && tank.inventory.count(id) > 0) {
        return None;
    }
    let item = id.item();
    let space = 99 - tank.inventory.count(id);
    if space == 0 {
        return None;
    }
    let quantity = space.min(item.bundle_size);
    let price = Money::from(prices[id as usize]);
    let cost = if quantity == item.bundle_size {
        price
    } else {
        let denominator = u32::from(item.bundle_size) * 5;
        let mut cost = price.scaled(u32::from(quantity) * 6, 1);
        cost.add(&Money::from(denominator / 2));
        cost.scaled(1, denominator)
    };
    Some((quantity, cost))
}

/// Whole-game entitlement: base plus an annuity for the remaining rounds.
/// Use exp_m1/ln_1p to avoid cancellation for very small positive rates.
pub fn auto_defense_price(base: u32, rate: f64, rounds_left: u32) -> u32 {
    let factor = if rate == 0.0 {
        f64::from(rounds_left) + 1.0
    } else {
        1.0 - (-f64::from(rounds_left) * rate.ln_1p()).exp_m1() / rate
    };
    (f64::from(base) * factor)
        .round()
        .clamp(1.0, f64::from(u32::MAX)) as u32
}

pub fn useful(id: ItemId, settings: &Settings) -> bool {
    match id {
        ItemId::SmokeTracer => !settings.weapons.trace_paths,
        ItemId::Parachute => settings.physics.tanks_fall,
        ItemId::EarthDisrupter => settings.physics.suspend_dirt > 0,
        ItemId::ContactTrigger => settings.weapons.tunneling,
        _ => true,
    }
}

pub fn offers(
    tank: &Tank,
    prices: &[u32; ITEM_COUNT],
    settings: &Settings,
    category: Category,
) -> Vec<ItemId> {
    ITEMS
        .iter()
        .filter(|item| {
            item.category == category
                && item.arms_level <= settings.weapons.arms_level
                && (settings.weapons.useless_items || useful(item.id, settings))
                && purchase(tank, item.id, prices)
                    .is_some_and(|(_, cost)| tank.account.cash >= cost)
        })
        .map(|item| item.id)
        .collect()
}

pub fn inventory(tank: &Tank) -> Vec<ItemId> {
    ITEMS
        .iter()
        .filter(|item| item.id != ItemId::BabyMissile && tank.inventory.count(item.id) > 0)
        .map(|item| item.id)
        .collect()
}

pub fn buy(
    tank: &mut Tank,
    id: ItemId,
    prices: &[u32; ITEM_COUNT],
    settings: &Settings,
    market: &mut Market,
) -> bool {
    if !offers(tank, prices, settings, id.item().category).contains(&id) {
        return false;
    }
    let (quantity, cost) = purchase(tank, id, prices).unwrap();
    tank.inventory.add(id, u32::from(quantity));
    tank.account.cash.subtract(&cost);
    if settings.economics.free_market {
        // Demand counts bundle purchases, including a final partial bundle.
        market.entries[id as usize].buys = market.entries[id as usize].buys.saturating_add(1);
    }
    true
}

pub fn sell_offer(
    id: ItemId,
    quantity: u8,
    prices: &[u32; ITEM_COUNT],
    free_market: bool,
) -> Money {
    if id == ItemId::BabyMissile {
        return Money::default();
    }
    let denominator = u32::from(id.item().bundle_size) * 100;
    let numerator = u32::from(quantity) * if free_market { 65 } else { 80 };
    let mut value = Money::from(prices[id as usize]).scaled(numerator * 2, 1);
    value.add(&Money::from(denominator));
    value.scaled(1, denominator * 2)
}

pub fn sell(
    tank: &mut Tank,
    id: ItemId,
    quantity: u8,
    prices: &[u32; ITEM_COUNT],
    free_market: bool,
) -> bool {
    if quantity == 0 || id == ItemId::BabyMissile || !tank.inventory.remove(id, u32::from(quantity))
    {
        return false;
    }
    tank.account
        .cash
        .add(&sell_offer(id, quantity, prices, free_market));
    if !tank.inventory.can_select(tank.weapon) {
        tank.weapon = ItemId::BabyMissile;
    }
    true
}

pub fn computer_buys(
    tank: &mut Tank,
    prices: &[u32; ITEM_COUNT],
    settings: &Settings,
    market: &mut Market,
) {
    if !settings.economics.computers_buy {
        return;
    }
    let crate::tank::Ctrl::Ai(selected) = tank.ctrl else {
        return;
    };
    if selected == crate::ai::AiKind::Unknown && tank.ai_state.resolved.is_none() {
        tank.ai_state = crate::ai::State::new(tank.ctrl);
    }
    let kind = tank.ai_state.effective(selected);
    if kind == crate::ai::AiKind::Moron {
        for _ in 0..32 {
            let choices: Vec<_> = [Category::Weapon, Category::Miscellaneous]
                .into_iter()
                .flat_map(|category| offers(tank, prices, settings, category))
                .collect();
            if choices.is_empty() {
                break;
            }
            let weight = |id: ItemId| match id {
                ItemId::Missile | ItemId::BabyNuke | ItemId::RiotCharge => 6,
                ItemId::Parachute | ItemId::Battery | ItemId::Shield => 4,
                ItemId::Laser | ItemId::PlasmaBlast | ItemId::AutoDefense => 1,
                _ => 2,
            };
            let mut roll =
                macroquad::rand::gen_range(0, choices.iter().map(|&id| weight(id)).sum::<i32>());
            for id in choices {
                roll -= weight(id);
                if roll < 0 {
                    buy(tank, id, prices, settings, market);
                    break;
                }
            }
        }
        return;
    }
    // Recovered priority skeleton; the extended order/stock caps are local policy.
    if !crate::tank::SHIELDS
        .iter()
        .any(|&id| tank.inventory.count(id) > 0)
    {
        for &id in crate::tank::SHIELDS.iter().rev() {
            if buy(tank, id, prices, settings, market) {
                break;
            }
        }
    }
    for (id, minimum) in [
        (ItemId::Parachute, 3),
        (ItemId::Missile, 5),
        (ItemId::Battery, 5),
        (ItemId::RiotCharge, 2),
        (ItemId::ContactTrigger, 5),
    ] {
        if tank.inventory.count(id) < minimum {
            buy(tank, id, prices, settings, market);
        }
    }
    for id in [
        ItemId::BabyNuke,
        ItemId::Nuke,
        ItemId::Mirv,
        ItemId::HeavyRoller,
        ItemId::Napalm,
        ItemId::Laser,
        ItemId::PlasmaBlast,
        ItemId::LazyBoy,
        ItemId::HeatGuidance,
        ItemId::FuelTank,
        ItemId::AutoDefense,
    ] {
        if tank.inventory.count(id) == 0 {
            buy(tank, id, prices, settings, market);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarketEntry {
    price: u32,
    demand: f64,
    ratio: f64,
    buys: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Market {
    entries: Vec<MarketEntry>,
}
impl Default for Market {
    fn default() -> Self {
        Self {
            entries: base_prices()
                .into_iter()
                .map(|price| MarketEntry {
                    price,
                    demand: 0.1,
                    ratio: 0.1,
                    buys: 0,
                })
                .collect(),
        }
    }
}
impl Market {
    pub fn prices(&self) -> [u32; ITEM_COUNT] {
        std::array::from_fn(|i| self.entries[i].price)
    }

    pub fn advance(&mut self, players: usize, rate: f64) {
        for (entry, item) in self.entries.iter_mut().zip(&ITEMS) {
            entry.demand =
                0.7 * entry.demand + 0.3 * (f64::from(entry.buys) / players.max(1) as f64);
            entry.ratio = 0.7 * entry.ratio
                + 0.3 * (f64::from(entry.price) / f64::from(item.cost)).powi(2) / 10.0;
            entry.price = (f64::from(entry.price) * (1.0 + rate * (entry.demand - entry.ratio)))
                .trunc()
                .clamp(1.0, f64::from(u32::MAX)) as u32;
            entry.buys = 0;
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("Cannot load market {}: {e}", path.display())),
        };
        let market: Self =
            toml::from_str(&text).map_err(|e| format!("Invalid market {}: {e}", path.display()))?;
        market
            .validate()
            .map_err(|e| format!("{e} in {}", path.display()))?;
        Ok(market)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.entries.len() != ITEM_COUNT
            || self.entries.iter().any(|e| {
                e.price == 0
                    || !e.demand.is_finite()
                    || e.demand < 0.0
                    || !e.ratio.is_finite()
                    || e.ratio < 0.0
            })
        {
            return Err("Invalid market data".into());
        }
        Ok(())
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        crate::settings::save_atomic(path, &text)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{economy::Account, gfx::Rgb, inventory::Inventory, tank::Ctrl};

    pub(crate) fn tank(cash: u32) -> Tank {
        Tank {
            bank: None,
            ai_state: crate::ai::State::default(),
            shape: crate::tank::Shape::Treads,
            name: "Buyer".into(),
            color: Rgb::WHITE,
            ctrl: Ctrl::Human,
            account: Account::new(cash),
            team: None,
            x: 0,
            y: 0,
            angle: 90,
            power: 400,
            health: 100,
            alive: true,
            inventory: Inventory::default(),
            equipment: crate::tank::Equipment::default(),
            weapon: ItemId::BabyMissile,
            fall: 0,
            last_hit: None,
            environmental_hit: false,
            kills: 0,
            wins: 0,
        }
    }

    #[test]
    fn smart_buyers_prioritize_defenses_and_ammo_and_buy_switch_is_authoritative() {
        let mut settings = Settings::default();
        settings.weapons.arms_level = 4;
        for kind in crate::ai::AiKind::KNOWN {
            let mut tank = tank(100_000);
            tank.ctrl = Ctrl::Ai(kind);
            tank.ai_state = crate::ai::State::new(tank.ctrl);
            let mut market = Market::default();
            let prices = base_prices();
            settings.economics.computers_buy = false;
            let before = tank.inventory.clone();
            computer_buys(&mut tank, &prices, &settings, &mut market);
            assert_eq!(tank.inventory, before);
            assert_eq!(tank.account.cash, Money::from(100_000));
            settings.economics.computers_buy = true;
            computer_buys(&mut tank, &prices, &settings, &mut market);
            assert!(tank.account.cash < Money::from(100_000));
            if kind != crate::ai::AiKind::Moron {
                assert!(tank.inventory.count(ItemId::SuperMag) > 0);
                assert!(tank.inventory.count(ItemId::Parachute) > 0);
                assert!(tank.inventory.count(ItemId::Missile) >= 5);
                assert!(tank.inventory.count(ItemId::Battery) >= 5);
            }
            assert!(ITEMS.iter().all(|item| tank.inventory.count(item.id) <= 99));
        }
    }

    #[test]
    fn buyers_obey_arms_filters_and_market_demand_matches_exact_spending() {
        let mut settings = Settings::default();
        settings.weapons.arms_level = 0;
        settings.economics.free_market = true;
        for kind in [
            crate::ai::AiKind::Moron,
            crate::ai::AiKind::Shooter,
            crate::ai::AiKind::Unknown,
        ] {
            let mut tank = tank(30_000);
            tank.ctrl = Ctrl::Ai(kind);
            tank.ai_state = crate::ai::State::new(tank.ctrl);
            let mut market = Market::default();
            let prices = market.prices();
            computer_buys(&mut tank, &prices, &settings, &mut market);
            let spent: u32 = ITEMS
                .iter()
                .map(|item| {
                    if item.id == ItemId::BabyMissile {
                        return 0;
                    }
                    let units = u32::from(tank.inventory.count(item.id));
                    let bundle = u32::from(item.bundle_size);
                    let price = prices[item.id as usize];
                    let remainder = units % bundle;
                    units / bundle * price
                        + if remainder == 0 {
                            0
                        } else {
                            (price * remainder * 6 + bundle * 5 / 2) / (bundle * 5)
                        }
                })
                .sum();
            assert_eq!(tank.account.cash, Money::from(30_000 - spent));
            assert!(ITEMS
                .iter()
                .filter(|item| item.arms_level > 0)
                .all(|item| tank.inventory.count(item.id) == 0));
            for (item, entry) in ITEMS.iter().zip(&market.entries) {
                if item.id != ItemId::BabyMissile {
                    assert_eq!(
                        u32::from(tank.inventory.count(item.id))
                            .div_ceil(u32::from(item.bundle_size)),
                        entry.buys
                    );
                }
            }
        }
    }

    #[test]
    fn fuel_is_never_offered_or_sold_to_fixed_emplacements_even_with_useless_items_on() {
        let mut settings = Settings::default();
        settings.weapons.arms_level = 4;
        settings.weapons.useless_items = true;
        let prices = base_prices();
        for shape in crate::tank::Shape::ALL {
            let mut t = tank(100_000);
            t.shape = shape;
            let offer =
                offers(&t, &prices, &settings, Category::Miscellaneous).contains(&ItemId::FuelTank);
            assert_eq!(offer, shape.mobile());
            assert_eq!(
                purchase(&t, ItemId::FuelTank, &prices).is_some(),
                shape.mobile()
            );
            let mut market = Market::default();
            assert_eq!(
                buy(&mut t, ItemId::FuelTank, &prices, &settings, &mut market),
                shape.mobile()
            );
            assert_eq!(
                t.inventory.count(ItemId::FuelTank),
                if shape.mobile() { 10 } else { 0 }
            );
        }
    }

    #[test]
    fn purchases_enforce_filters_affordability_and_partial_bundle_markup_atomically() {
        let mut t = tank(100_000);
        let mut settings = Settings::default();
        let mut market = Market::default();
        let prices = base_prices();
        t.inventory.add(ItemId::Missile, 97);
        assert_eq!(
            purchase(&t, ItemId::Missile, &prices),
            Some((2, Money::from(900)))
        );
        assert!(buy(
            &mut t,
            ItemId::Missile,
            &prices,
            &settings,
            &mut market
        ));
        assert_eq!(t.inventory.count(ItemId::Missile), 99);
        assert_eq!(t.account.cash, Money::from(99_100));
        assert!(!buy(
            &mut t,
            ItemId::Missile,
            &prices,
            &settings,
            &mut market
        ));
        settings.weapons.arms_level = 0;
        assert!(!buy(&mut t, ItemId::Nuke, &prices, &settings, &mut market));
        settings.weapons.arms_level = 4;
        settings.physics.tanks_fall = false;
        assert!(
            !offers(&t, &prices, &settings, Category::Miscellaneous).contains(&ItemId::Parachute)
        );
        settings.weapons.useless_items = true;
        assert!(buy(
            &mut t,
            ItemId::Parachute,
            &prices,
            &settings,
            &mut market
        ));
        assert_eq!(t.inventory.count(ItemId::Parachute), 8);
        assert!(buy(
            &mut t,
            ItemId::LeapFrog,
            &prices,
            &settings,
            &mut market
        ));
        t.account.cash = Money::from(1);
        let before = t.inventory.clone();
        assert!(!buy(
            &mut t,
            ItemId::Tracer,
            &prices,
            &settings,
            &mut market
        ));
        assert_eq!(t.account.cash, Money::from(1));
        assert_eq!(t.inventory, before);
        assert!(!buy(
            &mut t,
            ItemId::BabyMissile,
            &prices,
            &settings,
            &mut market
        ));
        // Buying at the cap uses exact cash even if a partial price exceeds u32.
        let mut prices = prices;
        prices[ItemId::ContactTrigger as usize] = u32::MAX;
        t.inventory.add(ItemId::ContactTrigger, 75);
        t.account.cash = Money::from(u32::MAX).scaled(2, 1);
        let (q, cost) = purchase(&t, ItemId::ContactTrigger, &prices).unwrap();
        assert_eq!(q, 24);
        assert!(cost > Money::from(u32::MAX));
        assert!(buy(
            &mut t,
            ItemId::ContactTrigger,
            &prices,
            &settings,
            &mut market
        ));
        assert_eq!(t.inventory.count(ItemId::ContactTrigger), 99);
    }

    #[test]
    fn selling_rounds_once_never_sells_unlimited_stock_and_reveals_affordable_items() {
        let mut t = tank(0);
        let settings = Settings::default();
        let prices = base_prices();
        t.inventory.add(ItemId::BabyNuke, 3);
        t.inventory.add(ItemId::Missile, 2);
        t.weapon = ItemId::Missile;
        assert!(offers(&t, &prices, &settings, Category::Weapon).is_empty());
        assert_eq!(
            sell_offer(ItemId::BabyNuke, 1, &prices, false),
            Money::from(2667)
        );
        assert_eq!(
            sell_offer(ItemId::BabyNuke, 1, &prices, true),
            Money::from(2167)
        );
        assert!(sell(&mut t, ItemId::BabyNuke, 1, &prices, false));
        assert!(offers(&t, &prices, &settings, Category::Weapon).contains(&ItemId::Missile));
        let before = t.account.cash.clone();
        for (id, qty) in [
            (ItemId::BabyMissile, 1),
            (ItemId::BabyNuke, 3),
            (ItemId::Missile, 0),
        ] {
            assert!(!sell(&mut t, id, qty, &prices, false));
            assert_eq!(t.account.cash, before);
        }
        assert!(sell(&mut t, ItemId::Missile, 2, &prices, true));
        assert_eq!(t.weapon, ItemId::BabyMissile);
        assert!(!inventory(&t).contains(&ItemId::BabyMissile));
    }

    #[test]
    fn useless_items_respect_settings_and_arms_limits_apply_to_both_groups() {
        let mut settings = Settings::default();
        let t = tank(1_000_000);
        let prices = base_prices();
        settings.weapons.trace_paths = true;
        settings.weapons.tunneling = false;
        settings.physics.tanks_fall = false;
        settings.physics.suspend_dirt = 0;
        for id in [
            ItemId::SmokeTracer,
            ItemId::ContactTrigger,
            ItemId::Parachute,
            ItemId::EarthDisrupter,
        ] {
            assert!(!offers(&t, &prices, &settings, id.item().category).contains(&id));
            settings.weapons.useless_items = true;
            assert!(offers(&t, &prices, &settings, id.item().category).contains(&id));
            settings.weapons.useless_items = false;
        }
        for level in 0..=4 {
            settings.weapons.arms_level = level;
            for category in [Category::Weapon, Category::Miscellaneous] {
                assert!(offers(&t, &prices, &settings, category)
                    .iter()
                    .all(|id| id.item().arms_level <= level));
            }
        }
    }

    #[test]
    fn market_tracks_buys_keeps_round_quotes_fixed_and_uses_configured_rate() {
        let mut market = Market::default();
        let mut settings = Settings::default();
        settings.economics.free_market = true;
        let mut t = tank(1_000_000);
        let prices = market.prices();
        let quote = sell_offer(ItemId::Missile, 5, &prices, true);
        for _ in 0..10 {
            assert!(buy(
                &mut t,
                ItemId::Missile,
                &prices,
                &settings,
                &mut market
            ));
        }
        assert_eq!(market.prices(), prices);
        assert_eq!(sell_offer(ItemId::Missile, 5, &prices, true), quote);
        let mut unchanged = market.clone();
        unchanged.advance(2, 0.0);
        assert_eq!(unchanged.prices(), prices);
        market.advance(2, 0.30);
        let expected = (1875.0_f64 * (1.0 + 0.30 * ((0.7 * 0.1 + 0.3 * 5.0) - 0.1))).trunc() as u32;
        assert_eq!(market.prices()[ItemId::Missile as usize], expected);
        assert!(market.prices()[ItemId::Nuke as usize] < prices[ItemId::Nuke as usize]);
        assert_eq!(market.entries[ItemId::Missile as usize].buys, 0);
        let mut hostile = Market::default();
        hostile.entries[0].price = u32::MAX;
        for _ in 0..1000 {
            hostile.advance(10, 0.30);
        }
        assert!(hostile.prices().iter().all(|&p| p > 0));
    }

    #[test]
    fn market_round_trips_pending_demand_and_rejects_bad_files_without_overwriting() {
        let dir = std::env::temp_dir().join(format!("scorched-market-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("market.toml");
        assert_eq!(Market::load(&path).unwrap().prices(), base_prices());
        let mut market = Market::default();
        market.entries[1].buys = 7;
        market.advance(3, 0.05);
        market.entries[2].buys = 2;
        market.save(&path).unwrap();
        let mut loaded = Market::load(&path).unwrap();
        assert_eq!(loaded.prices(), market.prices());
        loaded.advance(3, 0.05);
        market.advance(3, 0.05);
        assert_eq!(loaded.prices(), market.prices());
        for text in [
            "not toml",
            "entries = []",
            &toml::to_string(&Market {
                entries: vec![
                    MarketEntry {
                        price: 0,
                        demand: f64::NAN,
                        ratio: 0.1,
                        buys: 0
                    };
                    ITEM_COUNT
                ],
            })
            .unwrap(),
        ] {
            std::fs::write(&path, text).unwrap();
            assert!(Market::load(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
        assert!(market.save(&dir).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod auto_defense_tests {
    use super::*;
    #[test]
    fn annuity_handles_zero_small_and_positive_rates_without_repeated_price_adjustment() {
        assert_eq!(auto_defense_price(1500, 0.0, 10), 16500);
        assert_eq!(auto_defense_price(1500, 0.25, 1), 2700);
        assert_eq!(auto_defense_price(1500, 0.25, 2), 3660);
        assert_eq!(auto_defense_price(1500, 1e-12, 10), 16500);
        assert_eq!(auto_defense_price(1500, 0.0, 1000), 1501500);
        assert!(auto_defense_price(1500, 0.05, 10) > auto_defense_price(1500, 0.05, 1));
        assert_eq!(auto_defense_price(u32::MAX, 0.0, 1000), u32::MAX);
    }
    #[test]
    fn auto_defense_is_bought_once_and_use_keeps_the_token() {
        let mut t = tests::tank(100000);
        let settings = Settings::default();
        let mut prices = base_prices();
        prices[ItemId::AutoDefense as usize] = auto_defense_price(1500, 0.0, 10);
        let mut market = Market::default();
        assert!(buy(
            &mut t,
            ItemId::AutoDefense,
            &prices,
            &settings,
            &mut market
        ));
        assert_eq!(t.inventory.count(ItemId::AutoDefense), 1);
        assert_eq!(t.account.cash, Money::from(83500));
        assert!(!buy(
            &mut t,
            ItemId::AutoDefense,
            &prices,
            &settings,
            &mut market
        ));
        assert_eq!(
            t.use_item(ItemId::AutoDefense),
            crate::tank::ItemUse::Applied
        );
        assert_eq!(t.inventory.count(ItemId::AutoDefense), 1);
    }
}
