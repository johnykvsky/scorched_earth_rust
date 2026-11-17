//! One-pixel driving steps. Fuel is spent only after geometry and collision checks.
use crate::{gfx::W, tank::Tank, terrain::Terrain};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveResult {
    Blocked,
    Moved,
    Slip,
}

pub fn drive(
    tanks: &mut [Tank],
    current: usize,
    terrain: &Terrain,
    direction: i32,
    top: i32,
) -> MoveResult {
    let tank = &tanks[current];
    if !tank.alive
        || tank.health <= 0
        || !tank.shape.mobile()
        || tank.fuel_available() == 0
        || direction == 0
    {
        return MoveResult::Blocked;
    }
    let x = tank.x + direction.signum();
    if !(8..W - 8).contains(&x) {
        return MoveResult::Blocked;
    }
    let clear = |y| {
        y - 10 >= top && (x - 7..=x + 7).all(|xx| (y - 10..=y).all(|yy| !terrain.solid(xx, yy)))
    };
    // While driving a leading wheel/track can support a climb. Normal three-pixel
    // settling resumes when movement ends; don't turn a one-pixel rise into a slip.
    let support = |y| (x - 7..=x + 7).any(|xx| terrain.solid(xx, y + 1));
    let Some(rise) = (0..=3).find(|rise| clear(tank.y - rise)) else {
        return MoveResult::Blocked;
    };
    let mut y = tank.y - rise;
    let mut slip = false;
    if rise == 0 && !support(y) {
        if let Some(drop) = (1..=3).find(|drop| clear(y + drop) && support(y + drop)) {
            y += drop;
        } else {
            slip = true;
        }
    }
    if tanks.iter().enumerate().any(|(i, t)| {
        i != current
            && t.alive
            && t.health > 0
            && (t.x - x).abs() <= 14
            && y >= t.y - 10
            && y - 10 <= t.y
    }) {
        return MoveResult::Blocked;
    }
    let cost = 1 + rise as u16;
    let tank = &mut tanks[current];
    if !tank.spend_fuel(cost) {
        return MoveResult::Blocked;
    }
    tank.x = x;
    tank.y = y;
    tank.reset_descent();
    if slip {
        MoveResult::Slip
    } else {
        MoveResult::Moved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tank::Shape, weapons::ItemId};

    fn driver() -> Vec<Tank> {
        let mut tank = crate::shop::tests::tank(0);
        tank.x = 100;
        tank.y = 299;
        tank.inventory.add(ItemId::FuelTank, 2);
        vec![tank]
    }

    #[test]
    fn driving_consumes_units_opens_tanks_only_as_needed_and_is_reversible() {
        let mut tanks = driver();
        let terrain = Terrain::flat(300);
        for _ in 0..11 {
            assert_eq!(drive(&mut tanks, 0, &terrain, 1, 22), MoveResult::Moved);
        }
        assert_eq!((tanks[0].x, tanks[0].y), (111, 299));
        assert_eq!(tanks[0].fuel_available(), 9);
        assert_eq!(tanks[0].inventory.count(ItemId::FuelTank), 0);
        assert_eq!(tanks[0].equipment.fuel, 9);
        assert_eq!(drive(&mut tanks, 0, &terrain, -1, 22), MoveResult::Moved);
        assert_eq!(tanks[0].x, 110);
        assert_eq!(tanks[0].fuel_available(), 8);
        assert_eq!(tanks[0].health, 100);
    }

    #[test]
    fn slopes_cost_one_plus_climb_and_four_pixel_walls_are_blocked() {
        for rise in 1..=4 {
            let mut tanks = driver();
            let mut terrain = Terrain::flat(300);
            // New leading edge meets a vertical step.
            for y in 300 - rise..300 {
                terrain.add_dirt(108, y, 0, 22);
            }
            let result = drive(&mut tanks, 0, &terrain, 1, 22);
            if rise <= 3 {
                assert_eq!(result, MoveResult::Moved);
                assert_eq!(tanks[0].y, 299 - rise);
                assert_eq!(tanks[0].fuel_available(), 20 - 1 - rise as u16);
            } else {
                assert_eq!(result, MoveResult::Blocked);
                assert_eq!(tanks[0].x, 100);
                assert_eq!(tanks[0].fuel_available(), 20);
            }
        }
    }

    #[test]
    fn downhill_three_pixels_is_a_step_and_four_is_a_slip() {
        for drop in [1, 2, 3, 4, 20] {
            let mut tanks = driver();
            let terrain = Terrain::flat(300 + drop);
            let result = drive(&mut tanks, 0, &terrain, 1, 22);
            assert_eq!(
                result,
                if drop <= 3 {
                    MoveResult::Moved
                } else {
                    MoveResult::Slip
                }
            );
            assert_eq!(tanks[0].y, if drop <= 3 { 299 + drop } else { 299 });
            assert_eq!(tanks[0].fuel_available(), 19);
        }
    }

    #[test]
    fn unaffordable_climbs_bounds_other_tanks_and_fixed_shapes_spend_nothing() {
        let mut tanks = driver();
        tanks[0].inventory.remove(ItemId::FuelTank, 2);
        tanks[0].equipment.fuel = 1;
        let mut terrain = Terrain::flat(300);
        terrain.add_dirt(108, 299, 0, 22);
        assert_eq!(drive(&mut tanks, 0, &terrain, 1, 22), MoveResult::Blocked);
        assert_eq!(tanks[0].equipment.fuel, 1);
        assert_eq!(tanks[0].x, 100);
        let terrain = Terrain::flat(300);
        let mut other = tanks[0].clone();
        other.x = 115;
        tanks.push(other);
        assert_eq!(drive(&mut tanks, 0, &terrain, 1, 22), MoveResult::Blocked);
        tanks.pop();
        for x in [8, W - 9] {
            tanks[0].x = x;
            assert_eq!(
                drive(&mut tanks, 0, &terrain, if x == 8 { -1 } else { 1 }, 22),
                MoveResult::Blocked
            );
        }
        tanks[0].x = 100;
        for shape in [Shape::Bunker, Shape::Tower, Shape::Dish] {
            tanks[0].shape = shape;
            assert_eq!(drive(&mut tanks, 0, &terrain, 1, 22), MoveResult::Blocked);
            assert_eq!(tanks[0].equipment.fuel, 1);
        }
    }

    #[test]
    fn tank_can_drive_up_a_crater_wall_without_changing_terrain() {
        let mut tanks = driver();
        tanks[0].inventory.add(ItemId::FuelTank, 10);
        let mut terrain = Terrain::flat(300);
        terrain.carve(100, 260, 50);
        tanks[0].y = 309;
        let start = tanks[0].y;
        for _ in 0..40 {
            match drive(&mut tanks, 0, &terrain, 1, 22) {
                MoveResult::Moved => {}
                result => panic!("crater exit blocked at {}: {result:?}", tanks[0].x),
            }
        }
        assert!(tanks[0].y < start);
        assert_eq!(tanks[0].y, 299);
        assert!(!terrain.solid(100, 300));
    }
}
