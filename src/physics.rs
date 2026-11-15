//! Shared fixed-step ballistics and the original wind/settling rules.
use crate::gfx::W;
use crate::settings::PhysicsSettings;
use macroquad::prelude::Vec2;
use macroquad::rand::gen_range;

pub const SIM_DT: f32 = 1.0 / 600.0;
pub const MAX_FLIGHT_TIME: f32 = 120.0;

#[derive(Clone, Copy, Debug)]
pub struct Physics {
    pub gravity: f32,
    pub wind: f32,
    pub air_viscosity: u8,
    pub borders_extend: u16,
}

impl Physics {
    pub fn new(settings: &PhysicsSettings, wind: i32) -> Self {
        Self {
            gravity: 2500.0 * settings.gravity,
            wind: 1.25 * wind as f32,
            air_viscosity: settings.air_viscosity,
            borders_extend: settings.borders_extend,
        }
    }

    /// Original order: cap speed, move, drag, gravity, wind (MECHANICS.md §4.1).
    #[cfg(test)]
    pub fn advance(&self, pos: &mut Vec2, vel: &mut Vec2, dt: f32) {
        Self::cap_speed(vel);
        *pos += *vel * dt;
        self.air_forces(vel, dt, dt);
    }

    pub fn cap_speed(vel: &mut Vec2) {
        let speed_squared = vel.length_squared();
        if speed_squared > 1_000_000.0 {
            *vel *= 1000.0 / speed_squared.sqrt();
        }
    }

    /// Only the time spent in air contributes forces when a step crosses dirt.
    pub fn air_forces(&self, vel: &mut Vec2, air_time: f32, step_time: f32) {
        if air_time <= 0.0 {
            return;
        }
        let drag = 1.0 - self.air_viscosity as f32 / 10000.0;
        *vel *= if air_time >= step_time { drag } else { drag.powf(air_time / step_time) };
        vel.y += self.gravity * air_time;
        vel.x += self.wind * air_time;
    }

    /// The top stays open for high arcs; the lateral margin lets wind return shots.
    pub fn beyond_border(&self, x: f32) -> bool {
        x < -(self.borders_extend as f32) || x >= W as f32 + self.borders_extend as f32
    }
}

pub fn round_wind(max: u16) -> i32 {
    round_wind_with(max, |bound| gen_range(0, bound))
}

fn round_wind_with(max: u16, mut random: impl FnMut(i32) -> i32) -> i32 {
    let max = i32::from(max);
    if max < 2 {
        return 0;
    } // Avoid random(0) for the smallest wind limits.
    let mut wind = random(max / 2) - max / 4;
    if random(100) < 20 {
        wind *= 2;
        if random(100) < 40 {
            wind *= 2;
        }
    }
    wind
}

pub fn changed_wind(wind: i32, max: u16, roll: i32) -> i32 {
    (wind + roll - 5).clamp(-i32::from(max), i32::from(max))
}

/// Exactly 100-suspend of the possible rolls cause dirt to fall.
pub fn dirt_falls(suspend: u8, roll: u8) -> bool {
    roll < 100 - suspend
}

#[cfg(test)]
mod tests {
    use super::*;
    use macroquad::prelude::vec2;

    #[test]
    fn original_units_speed_limit_drag_and_euler_order() {
        let mut settings = PhysicsSettings::default();
        settings.air_viscosity = 20;
        let physics = Physics::new(&settings, 200);
        assert_eq!(physics.gravity, 500.0);
        assert_eq!(physics.wind, 250.0);
        let mut pos = vec2(10.0, 20.0);
        let mut vel = vec2(2000.0, 0.0);
        physics.advance(&mut pos, &mut vel, SIM_DT);
        assert!((pos.x - (10.0 + 1000.0 * SIM_DT)).abs() < 0.0001);
        assert_eq!(pos.y, 20.0); // Movement precedes gravity.
        assert!((vel.x - (998.0 + 250.0 * SIM_DT)).abs() < 0.0001);
        assert!((vel.y - 500.0 * SIM_DT).abs() < 0.0001);
    }

    fn landing_x(gravity: f32, wind: i32, viscosity: u8) -> f32 {
        let mut settings = PhysicsSettings::default();
        settings.gravity = gravity;
        settings.air_viscosity = viscosity;
        let physics = Physics::new(&settings, wind);
        let mut pos = vec2(100.0, 100.0);
        let mut vel = vec2(200.0, -200.0);
        for _ in 0..10000 {
            physics.advance(&mut pos, &mut vel, SIM_DT);
            if pos.y >= 300.0 {
                return pos.x;
            }
        }
        panic!("trajectory failed to land");
    }

    #[test]
    fn gravity_wind_and_viscosity_change_the_trajectory() {
        let still = landing_x(0.2, 0, 0);
        assert!(landing_x(0.4, 0, 0) < still);
        assert!(landing_x(0.2, 0, 20) < still);
        assert!(landing_x(0.2, -100, 0) < still);
        assert!(landing_x(0.2, 100, 0) > still);
    }

    #[test]
    fn round_wind_uses_original_rolls_and_conditional_doubling() {
        for (rolls, expected) in [
            (vec![75, 20], 25),
            (vec![75, 19, 40], 50),
            (vec![75, 19, 39], 100),
            (vec![0, 0, 0], -200),
        ] {
            let mut rolls = rolls.into_iter();
            assert_eq!(round_wind_with(200, |_| rolls.next().unwrap()), expected);
            assert!(rolls.next().is_none());
        }
        for max in [0, 1] {
            assert_eq!(round_wind_with(max, |_| panic!("random(0)")), 0);
        }
        for max in [2, 3, 7, 200, 500] {
            for first in 0..i32::from(max) / 2 {
                let mut first_roll = Some(first);
                let wind = round_wind_with(max, |_| first_roll.take().unwrap_or(0));
                assert!(wind.abs() <= i32::from(max));
            }
        }
    }

    #[test]
    fn changing_wind_is_a_clamped_random_walk_and_settling_is_a_percentage() {
        assert_eq!(changed_wind(499, 500, 10), 500);
        assert_eq!(changed_wind(-499, 500, 0), -500);
        for roll in 0..11 {
            assert_eq!(changed_wind(0, 200, roll), roll - 5);
            assert_eq!(changed_wind(0, 0, roll), 0);
        }
        for suspend in 0..=100 {
            assert_eq!(
                (0..100).filter(|&roll| dirt_falls(suspend, roll)).count(),
                (100 - suspend) as usize
            );
        }
    }
}
