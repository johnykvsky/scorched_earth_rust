//! Turn ordering, team outcomes and round awards.
use crate::{
    economy::{self, Money},
    settings::{PlayOrder, ScoringMode, TeamMode},
    tank::Tank,
};
use macroquad::rand::{gen_range, ChooseRandom};

#[derive(Default)]
pub struct Order {
    pub circle: Vec<usize>,
    pub active: Vec<usize>,
}
impl Order {
    pub fn sequential(players: usize) -> Self {
        let circle: Vec<_> = (0..players).collect();
        Self {
            active: circle.clone(),
            circle,
        }
    }
    pub fn reset(&mut self, players: usize) {
        *self = Self::sequential(players);
        self.circle.shuffle();
    }
    pub fn begin(
        &mut self,
        tanks: &[Tank],
        order: PlayOrder,
        round: u32,
        scoring: ScoringMode,
        prices: &[u32; crate::weapons::ITEM_COUNT],
    ) {
        self.active = self.circle.clone();
        match order {
            PlayOrder::Random => self.active.shuffle(),
            PlayOrder::RoundRobin => {
                let n = self.active.len();
                if n > 0 {
                    self.active.rotate_left((round as usize - 1) % n);
                }
            }
            PlayOrder::WinnersFirst | PlayOrder::LosersFirst => {
                let scores: Vec<_> = tanks
                    .iter()
                    .map(|t| {
                        if scoring == ScoringMode::Greedy {
                            economy::net_worth(t, prices)
                        } else {
                            t.account.cash.clone()
                        }
                    })
                    .collect();
                self.active.sort_by(|&a, &b| {
                    let comparison = if order == PlayOrder::WinnersFirst {
                        scores[b].cmp(&scores[a])
                    } else {
                        scores[a].cmp(&scores[b])
                    };
                    comparison.then(a.cmp(&b))
                });
            }
        }
    }
    pub fn next(&self, tanks: &[Tank], current: usize, order: PlayOrder) -> usize {
        if order == PlayOrder::Random {
            let eligible: Vec<_> = tanks
                .iter()
                .enumerate()
                .filter(|(i, t)| *i != current && t.alive && t.health > 0)
                .map(|(i, _)| i)
                .collect();
            return if eligible.is_empty() {
                current
            } else {
                eligible[gen_range(0, eligible.len())]
            };
        }
        let start = self.active.iter().position(|&i| i == current).unwrap_or(0);
        (1..=self.active.len())
            .map(|offset| self.active[(start + offset) % self.active.len()])
            .find(|&i| tanks[i].alive && tanks[i].health > 0)
            .unwrap_or(current)
    }
}

pub fn alive(tanks: &[Tank]) -> Vec<usize> {
    tanks
        .iter()
        .enumerate()
        .filter(|(_, t)| t.alive && t.health > 0)
        .map(|(i, _)| i)
        .collect()
}
fn group(tanks: &[Tank], index: usize) -> usize {
    tanks[index].team.map_or(100 + index, usize::from)
}
/// Vicious clears runtime teams; configured assignments return at the next round.
pub fn outcome(tanks: &mut [Tank], mode: TeamMode, split: &mut bool) -> Option<Vec<usize>> {
    let live = alive(tanks);
    if live.is_empty() {
        return Some(live);
    }
    if mode == TeamMode::None || *split {
        return if live.len() <= 1 { Some(live) } else { None };
    }
    let one_team = live
        .iter()
        .all(|&i| group(tanks, i) == group(tanks, live[0]));
    if !one_team {
        return None;
    }
    if mode == TeamMode::Vicious {
        for tank in tanks {
            tank.team = None;
        }
        *split = true;
        if live.len() <= 1 {
            Some(live)
        } else {
            None
        }
    } else {
        Some(live)
    }
}

pub fn survival(
    tanks: &mut [Tank],
    survivors: &[usize],
    mode: TeamMode,
    scoring: ScoringMode,
    split: bool,
) {
    if survivors.is_empty() {
        return;
    }
    let pool = economy::survival_pool(scoring, tanks.len());
    if mode != TeamMode::None && !split && tanks[survivors[0]].team.is_some() {
        let winners: Vec<_> = (0..tanks.len())
            .filter(|&i| group(tanks, i) == group(tanks, survivors[0]))
            .collect();
        let weight: u32 = winners
            .iter()
            .map(|i| if survivors.contains(i) { 2 } else { 1 })
            .sum();
        for winner in winners {
            let units = if survivors.contains(&winner) { 2 } else { 1 };
            economy::award_cash(tanks, winner, (pool * units / weight) as i32);
            tanks[winner].wins += 1;
        }
    } else {
        for &winner in survivors {
            economy::award_cash(tanks, winner, (pool / survivors.len() as u32) as i32);
        }
        tanks[survivors[0]].wins += 1;
    }
}
pub fn mass_kill_share(tanks: &[Tank], scoring: ScoringMode) -> Money {
    let live = alive(tanks);
    if live.is_empty() {
        return Money::default();
    }
    let n = live.len() as u32;
    let pool = if scoring == ScoringMode::Basic {
        1000 * tanks.len() as u32 + 4000 * n
    } else {
        5000 + 500 * n
            + live
                .iter()
                .map(|&i| {
                    30 * tanks[i].health.max(0) as u32
                        + 2 * tanks[i]
                            .equipment
                            .active_shield
                            .as_ref()
                            .map_or(0, |s| s.strength.max(0)) as u32
                })
                .sum::<u32>()
    };
    Money::from(pool / n)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tanks() -> Vec<Tank> {
        (0..4)
            .map(|i| {
                let mut t = crate::shop::tests::tank((i + 1) * 100);
                t.team = Some((i % 2) as u8);
                t
            })
            .collect()
    }
    #[test]
    fn orders_use_round_scores_rotating_random_circle_and_live_next_players() {
        let mut tanks = tanks();
        let prices = economy::base_prices();
        let mut order = Order::sequential(4);
        order.circle = vec![2, 0, 3, 1];
        order.begin(
            &tanks,
            PlayOrder::LosersFirst,
            1,
            ScoringMode::Standard,
            &prices,
        );
        assert_eq!(order.active, vec![0, 1, 2, 3]);
        order.begin(
            &tanks,
            PlayOrder::WinnersFirst,
            1,
            ScoringMode::Standard,
            &prices,
        );
        assert_eq!(order.active, vec![3, 2, 1, 0]);
        order.begin(
            &tanks,
            PlayOrder::RoundRobin,
            2,
            ScoringMode::Standard,
            &prices,
        );
        assert_eq!(order.active, vec![0, 3, 1, 2]);
        order.begin(
            &tanks,
            PlayOrder::RoundRobin,
            3,
            ScoringMode::Standard,
            &prices,
        );
        assert_eq!(order.active, vec![3, 1, 2, 0]);
        tanks[1].alive = false;
        assert_eq!(order.next(&tanks, 3, PlayOrder::RoundRobin), 2);
        for _ in 0..100 {
            let next = order.next(&tanks, 0, PlayOrder::Random);
            assert!(next == 2 || next == 3);
        }
        assert_eq!(order.circle, vec![2, 0, 3, 1]);
    }
    #[test]
    fn standard_teams_end_with_two_survivors_and_dead_members_get_half_weight() {
        let mut tanks = tanks();
        for t in &mut tanks {
            t.account = crate::economy::Account::default();
        }
        tanks[1].alive = false;
        tanks[3].alive = false;
        let mut split = false;
        assert_eq!(
            outcome(&mut tanks, TeamMode::Standard, &mut split),
            Some(vec![0, 2])
        );
        tanks[2].alive = false;
        survival(
            &mut tanks,
            &[0],
            TeamMode::Standard,
            ScoringMode::Standard,
            false,
        );
        assert_eq!(tanks[0].account.cash, Money::from(3333));
        assert_eq!(tanks[2].account.cash, Money::from(1666));
        assert_eq!(tanks[0].wins, 1);
        assert_eq!(tanks[2].wins, 1);
        assert!(tanks[1].account.cash.is_zero());
    }
    #[test]
    fn vicious_last_team_becomes_enemies_before_an_individual_wins() {
        let mut tanks = tanks();
        let mut split = false;
        assert!(outcome(&mut tanks, TeamMode::Vicious, &mut split).is_none());
        tanks[1].alive = false;
        tanks[3].alive = false;
        assert!(outcome(&mut tanks, TeamMode::Vicious, &mut split).is_none());
        assert!(split);
        assert_eq!(economy::relation(&tanks, 0, 2), economy::Relation::Enemy);
        assert_eq!(
            crate::ai::target_for(crate::ai::AiKind::Shooter, &tanks, 0),
            Some(2)
        );
        tanks[2].alive = false;
        assert_eq!(
            outcome(&mut tanks, TeamMode::Vicious, &mut split),
            Some(vec![0])
        );
    }
    #[test]
    fn mass_kill_uses_remaining_health_shields_and_basic_mode_constants() {
        let mut tanks = tanks();
        tanks[2].alive = false;
        tanks[3].alive = false;
        tanks[1].health = 50;
        tanks[0].equipment.active_shield =
            crate::shields::ActiveShield::new(crate::weapons::ItemId::Shield);
        assert_eq!(
            mass_kill_share(&tanks, ScoringMode::Standard),
            Money::from(5350)
        );
        assert_eq!(
            mass_kill_share(&tanks, ScoringMode::Basic),
            Money::from(6000)
        );
        tanks[0].alive = false;
        tanks[1].alive = false;
        assert!(mass_kill_share(&tanks, ScoringMode::Greedy).is_zero());
    }
}
