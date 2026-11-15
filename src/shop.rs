//! Opening purchases for Cash at Start. The full between-round shop is batch 7.
use crate::economy::Money;
use crate::tank::Tank;
use crate::weapons::{ItemId, ITEMS, ITEM_COUNT};

#[derive(Default)]
pub struct OpeningShop {
    pub player: usize,
    pub selected: usize,
}

pub fn offers(tank: &Tank, prices: &[u32; ITEM_COUNT]) -> Vec<ItemId> {
    ITEMS
        .iter()
        .filter(|item| {
            item.id != ItemId::BabyMissile
                && item.is_usable_weapon()
                && u16::from(tank.inventory.count(item.id)) + u16::from(item.bundle_size) <= 99
                && tank.account.cash >= Money::from(prices[item.id as usize])
        })
        .map(|item| item.id)
        .collect()
}

pub fn buy(tank: &mut Tank, id: ItemId, prices: &[u32; ITEM_COUNT]) -> bool {
    if !offers(tank, prices).contains(&id) {
        return false;
    }
    let item = id.item();
    let quantity = tank.inventory.add(id, u32::from(item.bundle_size));
    debug_assert_eq!(quantity, item.bundle_size);
    tank.account.cash.subtract(&Money::from(prices[id as usize]));
    true
}

pub fn computer_buys(tank: &mut Tank, prices: &[u32; ITEM_COUNT]) {
    // A bounded starter policy; personality-specific purchasing arrives in batch 16.
    for id in [
        ItemId::Nuke,
        ItemId::BabyNuke,
        ItemId::Missile,
        ItemId::Roller,
        ItemId::Mirv,
        ItemId::FunkyBomb,
    ] {
        buy(tank, id, prices);
    }
}
