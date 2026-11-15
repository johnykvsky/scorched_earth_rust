//! Bounded stock for every item, independent of round/battlefield state.

use crate::weapons::{ItemId, ITEMS, ITEM_COUNT};

pub const MAX_STOCK: u8 = 99;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inventory {
    counts: [u8; ITEM_COUNT],
}

impl Default for Inventory {
    fn default() -> Self {
        let mut counts = [0; ITEM_COUNT];
        counts[ItemId::BabyMissile as usize] = MAX_STOCK;
        Self { counts }
    }
}

impl Inventory {
    pub fn starter() -> Self {
        let mut inventory = Self::default();
        for item in &ITEMS {
            inventory.add(item.id, item.starting_ammo.into());
        }
        inventory
    }

    pub fn count(&self, item: ItemId) -> u8 {
        self.counts[item as usize]
    }

    /// Return the quantity actually added, allowing callers to charge for partial bundles.
    pub fn add(&mut self, item: ItemId, quantity: u32) -> u8 {
        if item == ItemId::BabyMissile {
            return 0;
        }
        let count = &mut self.counts[item as usize];
        let added = quantity.min(u32::from(MAX_STOCK - *count)) as u8;
        *count += added;
        added
    }

    /// Atomic removal: insufficient stock leaves the inventory unchanged.
    pub fn remove(&mut self, item: ItemId, quantity: u32) -> bool {
        if item == ItemId::BabyMissile {
            return true;
        }
        let count = &mut self.counts[item as usize];
        if quantity > u32::from(*count) {
            return false;
        }
        *count -= quantity as u8;
        true
    }

    pub fn can_select(&self, item: ItemId) -> bool {
        item.item().is_usable_weapon() && self.count(item) > 0
    }

    pub fn cycle_weapon(&self, current: ItemId, backwards: bool) -> ItemId {
        let mut index = current as usize;
        loop {
            index = (index + if backwards { ITEM_COUNT - 1 } else { 1 }) % ITEM_COUNT;
            let candidate = ITEMS[index].id;
            if self.can_select(candidate) {
                return candidate;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_caps_partial_additions_and_rejects_overdraw_without_changes() {
        for item in &ITEMS {
            if item.id == ItemId::BabyMissile {
                continue;
            }
            let mut stock = Inventory::default();
            assert_eq!(stock.add(item.id, 90), 90);
            assert_eq!(stock.add(item.id, 20), 9);
            assert_eq!(stock.add(item.id, u32::MAX), 0);
            assert_eq!(stock.count(item.id), 99);
            assert!(!stock.remove(item.id, 100));
            assert_eq!(stock.count(item.id), 99);
            assert!(stock.remove(item.id, 99));
            assert!(!stock.remove(item.id, 1));
            assert_eq!(stock.count(item.id), 0);
        }
    }

    #[test]
    fn baby_missiles_are_always_available_and_display_as_99() {
        let mut stock = Inventory::default();
        assert!(stock.remove(ItemId::BabyMissile, u32::MAX));
        assert_eq!(stock.add(ItemId::BabyMissile, u32::MAX), 0);
        assert_eq!(stock.count(ItemId::BabyMissile), 99);
        assert!(stock.can_select(ItemId::BabyMissile));
    }

    #[test]
    fn selection_wraps_and_skips_accessories_pending_weapons_and_empty_stock() {
        let mut stock = Inventory::default();
        stock.add(ItemId::Battery, 5);
        stock.add(ItemId::LeapFrog, 5);
        stock.add(ItemId::DirtBall, 1);
        stock.add(ItemId::Tracer, 1);
        assert_eq!(stock.cycle_weapon(ItemId::BabyMissile, false), ItemId::Tracer);
        assert_eq!(stock.cycle_weapon(ItemId::BabyMissile, true), ItemId::DirtBall);
        assert_eq!(stock.cycle_weapon(ItemId::DirtBall, false), ItemId::BabyMissile);
        stock.remove(ItemId::Tracer, 1);
        assert_eq!(stock.cycle_weapon(ItemId::BabyMissile, false), ItemId::DirtBall);
        stock.remove(ItemId::DirtBall, 1);
        assert_eq!(stock.cycle_weapon(ItemId::BabyMissile, false), ItemId::BabyMissile);
        assert_eq!(stock.cycle_weapon(ItemId::BabyMissile, true), ItemId::BabyMissile);
    }
}
