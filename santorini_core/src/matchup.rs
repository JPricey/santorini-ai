use std::{
    cell::{LazyCell, OnceCell},
    collections::{HashMap, HashSet},
};

use rand::seq::IndexedRandom;

use crate::{
    board::GodPair,
    gods::{ALL_GODS_BY_ID, GodName, StaticGod, WIP_GODS},
    player::Player,
};

/// A CLI argument value that is either a single god name or "wip" (expands to all WIP gods).
#[derive(Clone, Debug)]
pub enum GodSelector {
    Single(GodName),
    Wip,
}

impl std::str::FromStr for GodSelector {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("wip") {
            Ok(GodSelector::Wip)
        } else {
            s.parse::<GodName>().map(GodSelector::Single)
        }
    }
}

/// Resolve a list of `GodSelector` values into a flat list of `GodName`s,
/// expanding `Wip` into all WIP gods.
fn resolve_god_selectors(selectors: &[GodSelector]) -> Vec<GodName> {
    selectors
        .iter()
        .flat_map(|s| match s {
            GodSelector::Single(g) => vec![*g],
            GodSelector::Wip => WIP_GODS.to_vec(),
        })
        .collect()
}

/// A CLI argument value representing an explicit matchup, parsed as "god1:god2".
#[derive(Clone, Debug)]
pub struct MatchupPair(pub Matchup);

impl std::str::FromStr for MatchupPair {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (a, b) = s
            .split_once(':')
            .ok_or_else(|| format!("expected god1:god2, got '{s}'"))?;
        let g1: GodName = a.parse().map_err(|e| format!("bad god1 '{a}': {e}"))?;
        let g2: GodName = b.parse().map_err(|e| format!("bad god2 '{b}': {e}"))?;
        Ok(MatchupPair(Matchup::new(g1, g2)))
    }
}

#[derive(clap::Args, Clone, Debug, Default)]
pub struct MatchupArgs {
    /// Restrict Player 1 to these gods (accepts god names or "wip")
    #[arg(long, num_args = 1..)]
    pub p1: Vec<GodSelector>,

    /// Restrict Player 2 to these gods (accepts god names or "wip")
    #[arg(long, num_args = 1..)]
    pub p2: Vec<GodSelector>,

    /// Restrict both players to these gods (accepts god names or "wip")
    #[arg(long, num_args = 1..)]
    pub gods: Vec<GodSelector>,

    /// Add specific matchups as god1:god2 (e.g. --matchup apollo:pan)
    #[arg(long, num_args = 1..)]
    pub matchup: Vec<MatchupPair>,

    /// Exclude these gods for both players
    #[arg(long, num_args = 1..)]
    pub exclude: Vec<GodName>,

    /// Disallow mirror matchups (default: mirrors allowed)
    #[arg(long)]
    pub no_mirror: bool,

    /// Disallow swapped matchups (default: swaps allowed)
    #[arg(long)]
    pub no_swap: bool,
}

impl MatchupArgs {
    /// Build a `MatchupSelector` from these CLI arguments.
    /// Always excludes Mortal. `--p1`/`--p2` override `--gods` per-player.
    /// `--exclude` is applied after god selection.
    pub fn to_selector(&self) -> MatchupSelector {
        let mut selector = MatchupSelector::default()
            .with_can_mirror_option(!self.no_mirror)
            .with_can_swap_option(!self.no_swap);

        if !self.gods.is_empty() {
            let gods = resolve_god_selectors(&self.gods);
            selector = selector
                .with_exact_gods_for_player(Player::One, &gods)
                .with_exact_gods_for_player(Player::Two, &gods);
        }

        if !self.p1.is_empty() {
            selector =
                selector.with_exact_gods_for_player(Player::One, &resolve_god_selectors(&self.p1));
        }
        if !self.p2.is_empty() {
            selector =
                selector.with_exact_gods_for_player(Player::Two, &resolve_god_selectors(&self.p2));
        }

        for pair in &self.matchup {
            selector = selector.with_extra_matchup(pair.0);
        }

        for god in &self.exclude {
            selector = selector.minus_god_for_both(*god);
        }

        selector = selector.minus_god_for_both(GodName::Mortal);

        selector
    }
}

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

    // Gets stuck in infinite loop
    add_matchup(GodName::Ares, GodName::Ares, BannedReason::Engine);

    // We don't represent non-complete domes, so ban any domer vs chronus
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
    extra_matchups: Vec<Matchup>,
    // Can gods for p1 & p2 be swapped? (ex: allowing Apollo vs Pan also allows Pan vs Apollo)
    can_swap: bool,
    // Are mirrir matches allowed (ex: Apollo vs Apollo)?
    can_mirror: bool,
    all_matchups: OnceCell<Vec<Matchup>>,
}

fn _all_god_names() -> Vec<GodName> {
    ALL_GODS_BY_ID.iter().map(|g| g.god_name).collect()
}

impl Default for MatchupSelector {
    fn default() -> Self {
        Self {
            valid_gods: [_all_god_names(), _all_god_names()],
            extra_matchups: Vec::new(),
            can_swap: false,
            can_mirror: true,
            all_matchups: OnceCell::new(),
        }
    }
}

impl MatchupSelector {
    fn computed_matchups(&self) -> &[Matchup] {
        self.all_matchups.get_or_init(|| {
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
                        res.insert(m.flip());
                    }
                }
            }

            for m in &self.extra_matchups {
                res.insert(*m);
            }

            let mut res_vec: Vec<Matchup> = res.into_iter().collect();
            res_vec.sort();
            res_vec
        })
    }

    pub fn get(&self) -> Matchup {
        *self
            .computed_matchups()
            .choose(&mut rand::rng())
            .expect("no valid matchups in selector")
    }

    pub fn get_all(&self) -> Vec<Matchup> {
        self.computed_matchups().to_vec()
    }

    pub fn with_exact_gods_for_player(mut self, player: Player, gods: &[GodName]) -> Self {
        self.valid_gods[player as usize] = gods.iter().cloned().collect();
        self.all_matchups.take();
        self
    }

    pub fn with_extra_matchup(mut self, matchup: Matchup) -> Self {
        self.extra_matchups.push(matchup);
        self.all_matchups.take();
        self
    }

    pub fn minus_god_for_player(mut self, player: Player, god_name: GodName) -> Self {
        self.valid_gods[player as usize] = self.valid_gods[player as usize]
            .clone()
            .into_iter()
            .filter(|g| *g != god_name)
            .collect();
        self.all_matchups.take();
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
        self.all_matchups.take();
        self
    }

    pub fn with_can_mirror_option(mut self, can_mirror: bool) -> Self {
        self.can_mirror = can_mirror;
        self.all_matchups.take();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_gods_for_both_players() {
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Apollo, GodName::Pan])
            .with_exact_gods_for_player(Player::Two, &[GodName::Artemis]);

        let all = selector.get_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&Matchup::new(GodName::Apollo, GodName::Artemis)));
        assert!(all.contains(&Matchup::new(GodName::Pan, GodName::Artemis)));
    }

    #[test]
    fn no_mirror_excludes_same_god_pairs() {
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Apollo, GodName::Pan])
            .with_exact_gods_for_player(Player::Two, &[GodName::Apollo, GodName::Pan])
            .with_can_mirror_option(false);

        let all = selector.get_all();
        for m in &all {
            assert_ne!(m.gods[0], m.gods[1], "mirror matchup found: {m}");
        }
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn can_swap_includes_flipped() {
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Apollo])
            .with_exact_gods_for_player(Player::Two, &[GodName::Pan])
            .with_can_swap();

        let all = selector.get_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&Matchup::new(GodName::Apollo, GodName::Pan)));
        assert!(all.contains(&Matchup::new(GodName::Pan, GodName::Apollo)));
    }

    #[test]
    fn minus_god_removes_from_pool() {
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Apollo, GodName::Pan])
            .with_exact_gods_for_player(Player::Two, &[GodName::Artemis])
            .minus_god_for_player(Player::One, GodName::Apollo);

        let all = selector.get_all();
        assert_eq!(all, vec![Matchup::new(GodName::Pan, GodName::Artemis)]);
    }

    #[test]
    fn banned_matchups_excluded() {
        // Hades vs Pan is banned
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Hades])
            .with_exact_gods_for_player(Player::Two, &[GodName::Pan]);

        assert!(selector.get_all().is_empty());
    }

    #[test]
    fn extra_matchup_included() {
        let extra = Matchup::new(GodName::Apollo, GodName::Pan);
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Artemis])
            .with_exact_gods_for_player(Player::Two, &[GodName::Artemis])
            .with_extra_matchup(extra);

        let all = selector.get_all();
        assert!(all.contains(&Matchup::new(GodName::Artemis, GodName::Artemis)));
        assert!(all.contains(&extra));
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn cache_invalidated_by_builder() {
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Apollo])
            .with_exact_gods_for_player(Player::Two, &[GodName::Pan]);

        assert_eq!(selector.get_all().len(), 1);

        // Builder after read should recompute
        let selector = selector.with_extra_matchup(Matchup::new(GodName::Artemis, GodName::Atlas));
        assert_eq!(selector.get_all().len(), 2);
    }

    #[test]
    fn get_returns_valid_matchup() {
        let selector = MatchupSelector::default()
            .with_exact_gods_for_player(Player::One, &[GodName::Apollo])
            .with_exact_gods_for_player(Player::Two, &[GodName::Pan]);

        let m = selector.get();
        assert_eq!(m, Matchup::new(GodName::Apollo, GodName::Pan));
    }

    #[test]
    fn god_selector_parses_wip() {
        let sel: GodSelector = "wip".parse().unwrap();
        assert!(matches!(sel, GodSelector::Wip));

        let sel: GodSelector = "WIP".parse().unwrap();
        assert!(matches!(sel, GodSelector::Wip));
    }

    #[test]
    fn god_selector_parses_god_name() {
        let sel: GodSelector = "apollo".parse().unwrap();
        assert!(matches!(sel, GodSelector::Single(GodName::Apollo)));
    }

    #[test]
    fn resolve_wip_expands() {
        let selectors = vec![GodSelector::Wip];
        let resolved = resolve_god_selectors(&selectors);
        assert_eq!(resolved.len(), WIP_GODS.len());
        for g in &WIP_GODS {
            assert!(resolved.contains(g));
        }
    }

    #[test]
    fn resolve_mixed_selectors() {
        let selectors = vec![GodSelector::Single(GodName::Apollo), GodSelector::Wip];
        let resolved = resolve_god_selectors(&selectors);
        assert_eq!(resolved.len(), 1 + WIP_GODS.len());
        assert!(resolved.contains(&GodName::Apollo));
    }

    #[test]
    fn matchup_pair_parses() {
        let pair: MatchupPair = "apollo:pan".parse().unwrap();
        assert_eq!(pair.0, Matchup::new(GodName::Apollo, GodName::Pan));
    }

    #[test]
    fn matchup_pair_rejects_bad_format() {
        assert!("apollo".parse::<MatchupPair>().is_err());
        assert!("apollo:notagod".parse::<MatchupPair>().is_err());
    }
}
