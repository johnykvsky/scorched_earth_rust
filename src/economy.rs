//! Exact cash arithmetic, combat awards, interest and economic rankings.
use crate::inventory::Inventory;
use crate::settings::ScoringMode;
use crate::tank::Tank;
use crate::weapons::{ITEMS, ITEM_COUNT};
use std::{cmp::Ordering, fmt};

const BASE: u64 = 1_000_000_000;

/// Unsigned arbitrary-size money, stored as little-endian base-1e9 digits.
/// Interest in a 1000-round game can exceed fixed-width integer limits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Money(Vec<u32>);

impl From<u32> for Money {
    fn from(value: u32) -> Self {
        let mut value = u64::from(value);
        let mut digits = Vec::new();
        while value > 0 {
            digits.push((value % BASE) as u32);
            value /= BASE;
        }
        Self(digits)
    }
}
impl Ord for Money {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.iter().rev().cmp(other.0.iter().rev()))
    }
}
impl PartialOrd for Money {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((last, rest)) = self.0.split_last() else {
            return write!(f, "0");
        };
        write!(f, "{last}")?;
        for digit in rest.iter().rev() {
            write!(f, "{digit:09}")?;
        }
        Ok(())
    }
}
impl Money {
    pub(crate) fn from_decimal(text: &str) -> Result<Self, String> {
        if text.is_empty() || text.len() > 4096 || !text.bytes().all(|b| b.is_ascii_digit()) {
            return Err("Money must be a nonnegative decimal integer (up to 4096 digits)".into());
        }
        let mut value = Self::default();
        for chunk in text.as_bytes().rchunks(9) {
            value
                .0
                .push(std::str::from_utf8(chunk).unwrap().parse().unwrap());
        }
        value.normalize();
        Ok(value)
    }
    pub fn is_zero(&self) -> bool {
        self.0.is_empty()
    }
    pub fn add(&mut self, other: &Self) {
        let mut carry = 0u64;
        self.0.resize(self.0.len().max(other.0.len()), 0);
        for (i, digit) in self.0.iter_mut().enumerate() {
            let sum = u64::from(*digit) + u64::from(*other.0.get(i).unwrap_or(&0)) + carry;
            *digit = (sum % BASE) as u32;
            carry = sum / BASE;
        }
        if carry > 0 {
            self.0.push(carry as u32);
        }
    }
    pub fn subtract(&mut self, other: &Self) {
        if &*self < other {
            self.0.clear();
            return;
        }
        let mut borrow = 0i64;
        for (i, digit) in self.0.iter_mut().enumerate() {
            let value = i64::from(*digit) - i64::from(*other.0.get(i).unwrap_or(&0)) - borrow;
            *digit = value.rem_euclid(BASE as i64) as u32;
            borrow = i64::from(value < 0);
        }
        self.normalize();
    }
    /// Exact floor(self * numerator / denominator), without intermediate overflow.
    pub fn scaled(&self, numerator: u32, denominator: u32) -> Self {
        assert!(denominator > 0);
        let mut result = self.clone();
        let mut carry = 0u64;
        for digit in &mut result.0 {
            let product = u64::from(*digit) * u64::from(numerator) + carry;
            *digit = (product % BASE) as u32;
            carry = product / BASE;
        }
        while carry > 0 {
            result.0.push((carry % BASE) as u32);
            carry /= BASE;
        }
        carry = 0;
        for digit in result.0.iter_mut().rev() {
            let value = carry * BASE + u64::from(*digit);
            *digit = (value / u64::from(denominator)) as u32;
            carry = value % u64::from(denominator);
        }
        result.normalize();
        result
    }
    fn normalize(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    pub fn award(&mut self, amount: i32) {
        let value = Self::from(amount.unsigned_abs());
        if amount < 0 {
            self.subtract(&value);
        } else {
            self.add(&value);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub cash: Money,
    pub interest_earned: Money,
}

impl serde::Serialize for Money {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
impl<'de> serde::Deserialize<'de> for Money {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        Self::from_decimal(&text).map_err(serde::de::Error::custom)
    }
}
impl Account {
    pub fn new(cash: u32) -> Self {
        Self {
            cash: cash.into(),
            ..Self::default()
        }
    }
    pub fn pay_interest(&mut self, rate: f64) {
        let interest = self.cash.scaled((rate * 10000.0).trunc() as u32, 10000);
        self.cash.add(&interest);
        self.interest_earned.add(&interest);
    }
}

/// Corporate account replicas are synchronized at each transaction boundary.
pub fn sync_bank(tanks: &mut [Tank], source: usize) {
    let Some(bank) = tanks[source].bank else {
        return;
    };
    let account = tanks[source].account.clone();
    for tank in tanks.iter_mut().filter(|t| t.bank == Some(bank)) {
        tank.account = account.clone();
    }
}
pub fn award_cash(tanks: &mut [Tank], owner: usize, amount: i32) {
    tanks[owner].account.cash.award(amount);
    sync_bank(tanks, owner);
}
pub fn add_cash(tanks: &mut [Tank], owner: usize, amount: &Money) {
    tanks[owner].account.cash.add(amount);
    sync_bank(tanks, owner);
}
pub fn interest(tanks: &mut [Tank], rate: f64) {
    let mut paid = Vec::new();
    for i in 0..tanks.len() {
        if let Some(bank) = tanks[i].bank {
            if paid.contains(&bank) {
                continue;
            }
            paid.push(bank);
        }
        tanks[i].account.pay_interest(rate);
        sync_bank(tanks, i);
    }
}
/// Split old shared funds before regrouping, preserving every dollar, including remainders.
pub fn configure_banks(tanks: &mut [Tank], corporate: bool) {
    for i in 0..tanks.len() {
        let Some(bank) = tanks[i].bank else {
            continue;
        };
        let members: Vec<_> = (0..tanks.len())
            .filter(|&j| tanks[j].bank == Some(bank))
            .collect();
        let account = tanks[i].account.clone();
        let share = account.cash.scaled(1, members.len() as u32);
        let earned = account.interest_earned.scaled(1, members.len() as u32);
        let mut cash_left = account.cash;
        let mut interest_left = account.interest_earned;
        for (position, &j) in members.iter().enumerate() {
            let last = position + 1 == members.len();
            tanks[j].account = Account {
                cash: if last {
                    cash_left.clone()
                } else {
                    share.clone()
                },
                interest_earned: if last {
                    interest_left.clone()
                } else {
                    earned.clone()
                },
            };
            cash_left.subtract(&tanks[j].account.cash);
            interest_left.subtract(&tanks[j].account.interest_earned);
            tanks[j].bank = None;
        }
    }
    if !corporate {
        return;
    }
    let mut joined = Vec::new();
    for i in 0..tanks.len() {
        let Some(team) = tanks[i].team else {
            continue;
        };
        if joined.contains(&team) {
            continue;
        }
        joined.push(team);
        let members: Vec<_> = (0..tanks.len())
            .filter(|&j| tanks[j].team == Some(team))
            .collect();
        let mut account = Account::default();
        for &j in &members {
            account.cash.add(&tanks[j].account.cash);
            account
                .interest_earned
                .add(&tanks[j].account.interest_earned);
        }
        for j in members {
            tanks[j].bank = Some(team);
            tanks[j].account = account.clone();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageTarget {
    Tank,
    Shield,
}
#[derive(Clone, Copy)]
pub struct DamageEvent {
    pub owner: usize,
    pub victim: usize,
    pub amount: i32,
    pub target: DamageTarget,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Enemy,
    Teammate,
    Yourself,
}

pub fn relation(tanks: &[Tank], owner: usize, victim: usize) -> Relation {
    if owner == victim {
        Relation::Yourself
    } else if tanks[owner].team.is_some() && tanks[owner].team == tanks[victim].team {
        Relation::Teammate
    } else {
        Relation::Enemy
    }
}

pub fn damage_award(
    mode: ScoringMode,
    relation: Relation,
    amount: i32,
    target: DamageTarget,
) -> i32 {
    if mode == ScoringMode::Basic {
        return 0;
    }
    let rate = match relation {
        Relation::Enemy => {
            if target == DamageTarget::Shield {
                2
            } else {
                30
            }
        }
        Relation::Teammate | Relation::Yourself => -15,
    };
    rate * amount.max(0)
}
pub fn kill_award(mode: ScoringMode, relation: Relation) -> i32 {
    match relation {
        Relation::Enemy => {
            if mode == ScoringMode::Basic {
                4000
            } else {
                500
            }
        }
        Relation::Teammate => -2000,
        Relation::Yourself => -1500,
    }
}
pub fn award_damage(tanks: &mut [Tank], mode: ScoringMode, event: DamageEvent) {
    record_damage(tanks, Some(mode), event);
}

pub fn record_damage(tanks: &mut [Tank], mode: Option<ScoringMode>, event: DamageEvent) {
    if event.amount > 0 && relation(tanks, event.owner, event.victim) == Relation::Enemy {
        let old = tanks[event.victim].ai_state.grudges[event.owner];
        tanks[event.victim].ai_state.grudges[event.owner] = old.saturating_add(event.amount as u32);
    }
    if let Some(mode) = mode {
        let amount = damage_award(
            mode,
            relation(tanks, event.owner, event.victim),
            event.amount,
            event.target,
        );
        award_cash(tanks, event.owner, amount);
    }
}
pub fn survival_pool(mode: ScoringMode, players: usize) -> u32 {
    if mode == ScoringMode::Basic {
        players as u32 * 1000
    } else {
        5000
    }
}

pub fn base_prices() -> [u32; ITEM_COUNT] {
    std::array::from_fn(|i| ITEMS[i].cost)
}

pub fn inventory_value(inventory: &Inventory, prices: &[u32; ITEM_COUNT]) -> Money {
    let mut total = Money::default();
    for item in &ITEMS {
        // Match the 80% resale valuation: round once per held item type.
        let denominator = u32::from(item.bundle_size) * 100;
        let mut numerator = Money::from(prices[item.id as usize])
            .scaled(u32::from(inventory.count(item.id)) * 80, 1);
        numerator.add(&Money::from(denominator / 2));
        total.add(&numerator.scaled(1, denominator));
    }
    total
}
pub fn net_worth(tank: &Tank, prices: &[u32; ITEM_COUNT]) -> Money {
    let mut value = tank.account.cash.clone();
    value.add(&inventory_value(&tank.inventory, prices));
    value
}
pub fn ranking(tanks: &[Tank], mode: ScoringMode, prices: &[u32; ITEM_COUNT]) -> Vec<usize> {
    let scores: Vec<_> = tanks
        .iter()
        .map(|t| {
            if mode == ScoringMode::Greedy {
                net_worth(t, prices)
            } else {
                t.account.cash.clone()
            }
        })
        .collect();
    let mut order: Vec<_> = (0..tanks.len()).collect();
    order.sort_by(|&a, &b| scores[b].cmp(&scores[a]).then(a.cmp(&b)));
    order
}
pub fn money_text(money: &Money) -> String {
    let text = money.to_string();
    if text.len() <= 12 {
        format!("${text}")
    } else {
        format!("${}.{}e{}", &text[..1], &text[1..7], text.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corporate_awards_penalties_and_interest_touch_one_shared_balance() {
        let mut tanks: Vec<_> = (0..4)
            .map(|i| {
                let mut t = crate::shop::tests::tank(1000);
                t.team = Some(i % 2);
                t
            })
            .collect();
        configure_banks(&mut tanks, true);
        assert_eq!(tanks[0].account.cash, Money::from(2000));
        award_damage(
            &mut tanks,
            ScoringMode::Standard,
            DamageEvent {
                owner: 0,
                victim: 1,
                amount: 10,
                target: DamageTarget::Tank,
            },
        );
        assert_eq!(tanks[2].account.cash, Money::from(2300));
        award_damage(
            &mut tanks,
            ScoringMode::Standard,
            DamageEvent {
                owner: 0,
                victim: 2,
                amount: 5,
                target: DamageTarget::Tank,
            },
        );
        assert_eq!(tanks[0].account.cash, Money::from(2225));
        award_cash(&mut tanks, 2, -2000);
        interest(&mut tanks, 0.1);
        assert_eq!(tanks[0].account.cash, Money::from(247));
        assert_eq!(tanks[0].account, tanks[2].account);
        assert_eq!(tanks[0].account.interest_earned, Money::from(22));
        assert_eq!(tanks[1].account.cash, Money::from(2200));
    }
    #[test]
    fn regrouping_and_leaving_corporate_mode_preserve_every_dollar() {
        let mut tanks: Vec<_> = (0..4)
            .map(|i| {
                let mut t = crate::shop::tests::tank(100 + i);
                t.team = Some((i % 2) as u8);
                t
            })
            .collect();
        configure_banks(&mut tanks, true);
        for (i, tank) in tanks.iter_mut().enumerate() {
            tank.team = Some((i / 2) as u8);
        }
        configure_banks(&mut tanks, true);
        assert_eq!(tanks[0].account.cash, Money::from(203));
        assert_eq!(tanks[2].account.cash, Money::from(203));
        configure_banks(&mut tanks, false);
        let mut total = Money::default();
        for tank in &tanks {
            assert!(tank.bank.is_none());
            total.add(&tank.account.cash);
        }
        assert_eq!(total, Money::from(406));
    }

    #[test]
    fn money_handles_carries_borrows_zero_floor_and_large_values_exactly() {
        let mut cash = Money::from(u32::MAX);
        cash.add(&Money::from(1));
        assert_eq!(cash.to_string(), "4294967296");
        cash.subtract(&Money::from(u32::MAX));
        assert_eq!(cash, Money::from(1));
        cash.award(-1500);
        assert!(cash.is_zero());
        let mut power = Money::from(1);
        for _ in 0..200 {
            power = power.scaled(2, 1);
        }
        assert_eq!(
            power.to_string(),
            "1606938044258990275541962092341162602522202993782792835301376"
        );
        assert!(power > Money::from(u32::MAX));
        for _ in 0..200 {
            power = power.scaled(1, 2);
        }
        assert_eq!(power, Money::from(1));
        assert_eq!(power.scaled(0, 1), Money::default());
        let mut seed = 7831u32;
        for _ in 0..100 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let a = seed;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let b = seed;
            let divisor = (seed / 31).max(1);
            assert_eq!(
                Money::from(a).scaled(b, divisor).to_string(),
                (u128::from(a) * u128::from(b) / u128::from(divisor)).to_string()
            );
            let mut sum = Money::from(a);
            sum.add(&Money::from(b));
            assert_eq!(sum.to_string(), (u64::from(a) + u64::from(b)).to_string());
            sum.subtract(&Money::from(a));
            assert_eq!(sum, Money::from(b));
        }
    }

    #[test]
    fn interest_truncates_rate_to_basis_points_and_compounds_without_overflow() {
        let mut account = Account::new(99999);
        account.pay_interest(0.0123456);
        assert_eq!(account.cash, Money::from(101228));
        assert_eq!(account.interest_earned, Money::from(1229));
        account.pay_interest(0.05);
        assert_eq!(account.cash, Money::from(106289));
        assert_eq!(account.interest_earned, Money::from(6290));
        let before = account.clone();
        account.pay_interest(0.0);
        assert_eq!(account, before);
        let mut account = Account::new(1_000_000);
        for _ in 0..1000 {
            account.pay_interest(0.30);
        }
        assert_eq!(account.cash.to_string().len(), 120);
        let mut earned = account.cash.clone();
        earned.subtract(&Money::from(1_000_000));
        assert_eq!(earned, account.interest_earned);
    }

    #[test]
    fn damage_and_kill_rules_include_shields_and_friendly_fire() {
        for mode in [
            ScoringMode::Basic,
            ScoringMode::Standard,
            ScoringMode::Greedy,
        ] {
            assert_eq!(
                kill_award(mode, Relation::Enemy),
                if mode == ScoringMode::Basic {
                    4000
                } else {
                    500
                }
            );
            assert_eq!(kill_award(mode, Relation::Yourself), -1500);
            assert_eq!(kill_award(mode, Relation::Teammate), -2000);
            for (relation, target, expected) in [
                (Relation::Enemy, DamageTarget::Tank, 600),
                (Relation::Enemy, DamageTarget::Shield, 40),
                (Relation::Yourself, DamageTarget::Tank, -300),
                (Relation::Teammate, DamageTarget::Shield, -300),
            ] {
                assert_eq!(
                    damage_award(mode, relation, 20, target),
                    if mode == ScoringMode::Basic {
                        0
                    } else {
                        expected
                    }
                );
            }
            assert_eq!(
                survival_pool(mode, 10),
                if mode == ScoringMode::Basic {
                    10000
                } else {
                    5000
                }
            );
        }
    }

    #[test]
    fn inventory_valuation_uses_current_bundle_prices_and_rounds_per_item() {
        let mut inventory = Inventory::default();
        let mut prices = base_prices();
        let baby_value = 3168; // The always-displayed 99 Baby Missiles use the same table valuation.
        assert_eq!(
            inventory_value(&inventory, &prices),
            Money::from(baby_value)
        );
        inventory.add(crate::weapons::ItemId::Nuke, 1);
        assert_eq!(
            inventory_value(&inventory, &prices),
            Money::from(baby_value + 9600)
        );
        prices[crate::weapons::ItemId::Nuke as usize] = 15000;
        assert_eq!(
            inventory_value(&inventory, &prices),
            Money::from(baby_value + 12000)
        );
        inventory.add(crate::weapons::ItemId::HeatGuidance, 2);
        assert_eq!(
            inventory_value(&inventory, &prices),
            Money::from(baby_value + 12000 + 2667)
        );
        prices[crate::weapons::ItemId::Nuke as usize] = u32::MAX;
        inventory.add(crate::weapons::ItemId::Nuke, 98);
        assert!(inventory_value(&inventory, &prices) > Money::from(u32::MAX));
    }
}
