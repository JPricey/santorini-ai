use std::{
    cell::LazyCell,
    collections::{HashMap, HashSet},
};

use rand::{Rng, seq::IndexedRandom};

use crate::{
    gods::{ALL_GODS_BY_ID, GodName, StaticGod},
    player::Player,
};

#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum BannedReason {
    Game,
    Engine,
}
pub const BANNED_MATCHUPS: LazyCell<HashMap<Matchup, BannedReason>> = LazyCell::new(|| {
    let mut set = HashMap::new();
    let mut add_matchup = |g1: GodName, g2: GodName, reason: BannedReason| {
        set.insert(Matchup::new(g1, g2), reason);
        set.insert(Matchup::new(g2, g1), reason);
    };

    // set.insert(
    //     Matchup::new(GodName::Graeae, GodName::Nemesis),
    //     BannedReason::Game,
    // );

    add_matchup(GodName::Harpies, GodName::Hermes, BannedReason::Game);

    // set.insert(
    //     Matchup::new(GodName::Harpies, GodName::Triton),
    //     BannedReason::Game,
    // );
    // set.insert(
    //     Matchup::new(GodName::Urania, GodName::Aphrodite),
    //     BannedReason::Game,
    // );

    // TODO: special move logic
    // add_matchup(GodName::Harpies, GodName::Artemis, BannedReason::Engine);

    set
});

pub fn matchup_banned_reason(matchup: &Matchup) -> Option<BannedReason> {
    BANNED_MATCHUPS.get(matchup).copied()
}

pub fn is_matchup_banned(matchup: &Matchup) -> bool {
    matchup_banned_reason(matchup).is_some()
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Matchup {
    pub gods: [GodName; 2],
}

impl std::fmt::Display for Matchup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} vs {}", self.gods[0], self.gods[1])
    }
}

impl Matchup {
    pub const fn new(g1: GodName, g2: GodName) -> Self {
        Self { gods: [g1, g2] }
    }

    pub const fn new_arr(gods: [GodName; 2]) -> Self {
        Self { gods }
    }

    pub const fn flip(&self) -> Self {
        Self {
            gods: [self.gods[1], self.gods[0]],
        }
    }

    pub const fn is_equal(&self, other: &Self) -> bool {
        self.gods[0].is_equal(other.gods[0]) && self.gods[1].is_equal(other.gods[1])
    }

    pub const fn is_same_gods(&self, other: &Self) -> bool {
        self.is_equal(other) || self.flip().is_equal(other)
    }

    pub const fn god_1(&self) -> StaticGod {
        self.gods[0].to_power()
    }

    pub const fn god_2(&self) -> StaticGod {
        self.gods[1].to_power()
    }

    pub const fn get_gods(&self) -> [StaticGod; 2] {
        [self.gods[0].to_power(), self.gods[1].to_power()]
    }

    pub fn is_mirror(&self) -> bool {
        self.gods[0] == self.gods[1]
    }
}

#[derive(Clone, Debug)]
pub struct MatchupSelector {
    valid_gods: [Vec<GodName>; 2],
    can_swap: bool,
    can_mirror: bool,
}

fn _all_god_names() -> Vec<GodName> {
    ALL_GODS_BY_ID.iter().map(|g| g.god_name).collect()
}

impl Default for MatchupSelector {
    fn default() -> Self {
        Self {
            valid_gods: [_all_god_names(), _all_god_names()],
            can_swap: false,
            can_mirror: true,
        }
    }
}

impl MatchupSelector {
    pub fn get(&self) -> Matchup {
        for _ in 0..1000000 {
            if let Some(res) = self._get() {
                return res;
            }
        }
        panic!("Couldn't find matchup: {:?}", self);
    }

    pub fn get_maybe_flipped(&self) -> Matchup {
        let mut matchup = self.get();
        if self.can_swap && rand::random() {
            matchup = matchup.flip();
        }
        matchup
    }

    pub fn get_all(&self) -> Vec<Matchup> {
        let mut res = HashSet::new();

        for g1 in self.valid_gods[0].iter() {
            for g2 in self.valid_gods[1].iter() {
                if g1 == g2 && !self.can_mirror {
                    continue;
                }

                let m = Matchup::new(*g1, *g2);
                if is_matchup_banned(&m) {
                    continue;
                }

                res.insert(m);
                if self.can_swap {
                    let flipped = m.flip();
                    res.insert(flipped);
                }
            }
        }

        let mut res_vec: Vec<Matchup> = res.into_iter().collect();
        res_vec.sort();
        res_vec
    }

    fn _get(&self) -> Option<Matchup> {
        let mut rng = &mut rand::rng();
        let g1 = self.valid_gods[0].choose(&mut rng).unwrap();
        let g2 = self.valid_gods[1].choose(&mut rng).unwrap();

        let res = Matchup::new(*g1, *g2);

        if BANNED_MATCHUPS.get(&res).is_some() {
            return None;
        }

        Some(if self.can_swap && rng.random() {
            res.flip()
        } else {
            res
        })
    }

    pub fn with_exact_god_for_player(&mut self, player: Player, god_name: GodName) -> &mut Self {
        self.valid_gods[player as usize] = vec![god_name];
        self
    }

    pub fn with_exact_gods_for_player(&mut self, player: Player, gods: Vec<GodName>) -> &mut Self {
        self.valid_gods[player as usize] = gods;
        self
    }

    pub fn minus_god_for_player(&mut self, player: Player, god_name: GodName) -> &mut Self {
        self.valid_gods[player as usize] = self.valid_gods[player as usize]
            .clone()
            .into_iter()
            .filter(|g| *g != god_name)
            .collect();
        self
    }

    pub fn minus_god_for_both(&mut self, god_name: GodName) -> &mut Self {
        self.minus_god_for_player(Player::One, god_name);
        self.minus_god_for_player(Player::Two, god_name)
    }

    pub fn minus_gods_for_player(&mut self, player: Player, gods: &Vec<GodName>) -> &mut Self {
        for god in gods {
            self.minus_god_for_player(player, *god);
        }
        self
    }

    pub fn with_can_swap(&mut self) -> &mut Self {
        self.with_can_swap_option(true)
    }

    pub fn with_no_swap(&mut self) -> &mut Self {
        self.with_can_swap_option(false)
    }

    pub fn with_can_swap_option(&mut self, can_swap: bool) -> &mut Self {
        self.can_swap = can_swap;
        self
    }

    pub fn with_can_mirror_option(&mut self, can_mirror: bool) -> &mut Self {
        self.can_mirror = can_mirror;
        self
    }
}
