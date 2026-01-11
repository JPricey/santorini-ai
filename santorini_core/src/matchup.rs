use std::{
    cell::LazyCell,
    collections::{HashMap, HashSet},
};

use rand::{Rng, seq::IndexedRandom};

use crate::{
    board::GodPair,
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

    // Gets stuck in infinite loop
    add_matchup(GodName::Ares, GodName::Ares, BannedReason::Engine);

    add_matchup(GodName::Hades, GodName::Pan, BannedReason::Game);
    add_matchup(GodName::Hades, GodName::Asteria, BannedReason::Game);
    add_matchup(GodName::Hades, GodName::Nike, BannedReason::Game);

    add_matchup(GodName::Aphrodite, GodName::Urania, BannedReason::Game);

    add_matchup(GodName::Harpies, GodName::Hermes, BannedReason::Game);

    // Harpies/Maenads seems fine? Well we implemented it, anyway

    add_matchup(GodName::Nemesis, GodName::Aphrodite, BannedReason::Game);
    add_matchup(GodName::Nemesis, GodName::Bia, BannedReason::Game);
    add_matchup(GodName::Nemesis, GodName::Clio, BannedReason::Game);
    // add_matchup(GodName::Nemesis, GodName::Gaea, BannedReason::Game);
    add_matchup(GodName::Nemesis, GodName::Graeae, BannedReason::Game);
    add_matchup(GodName::Nemesis, GodName::Medusa, BannedReason::Game);
    // add_matchup(GodName::Nemesis, GodName::Terpsichore, BannedReason::Game);
    // add_matchup(GodName::Nemesis, GodName::Theseus, BannedReason::Game);

    // We don't represent non-complete domes, so ban any domer
    add_matchup(GodName::Chronus, GodName::Atlas, BannedReason::Engine);
    add_matchup(GodName::Chronus, GodName::Selene, BannedReason::Engine);
    add_matchup(GodName::Chronus, GodName::Asteria, BannedReason::Engine);
    add_matchup(GodName::Chronus, GodName::Polyphemus, BannedReason::Engine);

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

    pub const fn get_gods(&self) -> GodPair {
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

    pub fn with_exact_god_for_player(mut self, player: Player, god_name: GodName) -> Self {
        self.valid_gods[player as usize] = vec![god_name];
        self
    }

    pub fn with_exact_gods_for_player(mut self, player: Player, gods: &[GodName]) -> Self {
        self.valid_gods[player as usize] = gods.iter().cloned().collect();
        self
    }

    pub fn minus_god_for_player(mut self, player: Player, god_name: GodName) -> Self {
        self.valid_gods[player as usize] = self.valid_gods[player as usize]
            .clone()
            .into_iter()
            .filter(|g| *g != god_name)
            .collect();
        self
    }

    pub fn minus_god_for_both(self, god_name: GodName) -> Self {
        self.minus_god_for_player(Player::One, god_name)
            .minus_god_for_player(Player::Two, god_name)
    }

    pub fn minus_gods_for_player(self, player: Player, gods: &Vec<GodName>) -> Self {
        let mut res = self;
        for god in gods {
            res = res.minus_god_for_player(player, *god);
        }
        res
    }

    pub fn with_can_swap(self) -> Self {
        self.with_can_swap_option(true)
    }

    pub fn with_no_swap(self) -> Self {
        self.with_can_swap_option(false)
    }

    pub fn with_can_swap_option(mut self, can_swap: bool) -> Self {
        self.can_swap = can_swap;
        self
    }

    pub fn with_can_mirror_option(mut self, can_mirror: bool) -> Self {
        self.can_mirror = can_mirror;
        self
    }
}
