use std::{cell::LazyCell, collections::HashMap};

use rand::{Rng, seq::IndexedRandom};

use crate::{
    gods::{ALL_GODS_BY_ID, GodName, StaticGod},
    player::Player,
};

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub enum BannedReason {
    Game,
    Engine,
}
pub const BANNED_MATCHUPS: LazyCell<HashMap<Matchup, BannedReason>> = LazyCell::new(|| {
    let mut set = HashMap::new();
    let mut _add_matchup = |g1: GodName, g2: GodName, reason: BannedReason| {
        set.insert(Matchup::new(g1, g2), reason);
        set.insert(Matchup::new(g2, g1), reason);
    };

    // set.insert(
    //     Matchup::new(GodName::Graeae, GodName::Nemesis),
    //     BannedReason::Game,
    // );
    //
    // set.insert(
    //     Matchup::new(GodName::Harpies, GodName::Hermes),
    //     BannedReason::Game,
    // );
    // set.insert(
    //     Matchup::new(GodName::Harpies, GodName::Triton),
    //     BannedReason::Game,
    // );
    // set.insert(
    //     Matchup::new(GodName::Urania, GodName::Aphrodite),
    //     BannedReason::Game,
    // );

    set
});

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
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
}

#[derive(Clone, Debug)]
pub struct MatchupSelector {
    valid_gods: [Vec<GodName>; 2],
    can_swap: bool,
}

fn _all_god_names() -> Vec<GodName> {
    ALL_GODS_BY_ID.iter().map(|g| g.god_name).collect()
}

impl Default for MatchupSelector {
    fn default() -> Self {
        Self {
            valid_gods: [_all_god_names(), _all_god_names()],
            can_swap: false,
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

    pub fn minus_god_for_player(&mut self, player: Player, god_name: GodName) -> &mut Self {
        self.valid_gods[player as usize] = self.valid_gods[player as usize]
            .clone()
            .into_iter()
            .filter(|g| *g != god_name)
            .collect();
        self
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
}
