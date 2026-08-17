//! Human-readable descriptions of each god's power.
//!
//! Text follows the published card wording where one exists, adjusted to match
//! what this engine actually implements. Gods suffixed with `V2` are the
//! Pantheon Edition reworks of the original card, and the `Chronus*T` variants
//! are Chronus with a non-standard number of complete towers needed to win.

use crate::gods::GodName;

impl GodName {
    /// A one or two sentence description of this god's power.
    pub const fn description(self) -> &'static str {
        match self {
            GodName::Mortal => {
                "No special power."
            }
            GodName::Pan => "You also win if your Worker moves down two or more levels.",
            GodName::Artemis => {
                "Your Worker may move one additional time, but not back to its initial space."
            }
            GodName::Hephaestus => {
                "Your Worker may build one additional block (not dome) on top of your first block."
            }
            GodName::Atlas => "Your Worker may build a dome at any level.",
            GodName::Athena => {
                "If one of your Workers moved up on your last turn, opponent Workers cannot move \
                 up this turn."
            }
            GodName::Minotaur => {
                "Your Worker may move into an opponent Worker's space, if their Worker can be \
                 forced one space straight backwards to an unoccupied space at any level."
            }
            GodName::Demeter => {
                "Your Worker may build one additional time, but not on the same space."
            }
            GodName::Apollo => {
                "Your Worker may move into a neighboring opponent Worker's space by forcing their \
                 Worker to the space yours just vacated."
            }
            GodName::Hermes => {
                "If your Workers do not move up or down, they may each move any number of times \
                 (even zero), and then one of them builds."
            }
            GodName::Prometheus => {
                "If your Worker does not move up, it may build both before and after moving."
            }
            GodName::Urania => {
                "When your Worker moves or builds, treat opposite edges and corners of the board \
                 as adjacent, so that every space has eight neighbors."
            }
            GodName::Graeae => {
                "Place three Workers. After moving, build with one of your Workers that did not \
                 move."
            }
            GodName::Hera => "An opponent cannot win by moving into a perimeter space.",
            GodName::Limus => {
                "Opponent Workers cannot build on spaces neighboring your Workers, unless building \
                 a dome to create a complete tower."
            }
            GodName::Hypnus => {
                "If one of your opponent's Workers is higher than all of their others, it cannot \
                 move."
            }
            GodName::Harpies => {
                "Each time an opponent's Worker moves, it is forced space by space in the same \
                 direction until the next space is at a higher level or is obstructed."
            }
            GodName::Aphrodite => {
                "If an opponent Worker starts its turn neighboring one of your Workers, its last \
                 move must be to a space neighboring one of your Workers."
            }
            GodName::Persephone => {
                "On your opponent's turn, if possible, at least one of their Workers must move up."
            }
            GodName::Hades => "Opponent Workers cannot move down.",
            GodName::Morpheus => {
                "At the start of your turn, add a block to your god power card. Your Worker cannot \
                 build as normal; instead it may spend any number of stored blocks, building that \
                 many times."
            }
            GodName::Aeolus => {
                "On your turn, set the wind to any direction (or to none). Workers cannot move \
                 directly into the wind."
            }
            GodName::Hestia => {
                "Your Worker may build one additional time, but this cannot be on a perimeter \
                 space."
            }
            GodName::Europa => {
                "Europa & Talus: all players treat the space containing the Talus token as if it \
                 contains only a dome. After your turn, you may move the Talus token to a space \
                 neighboring your moved Worker."
            }
            GodName::Bia => {
                "Place your Workers on perimeter spaces. If your Worker moves into a space and the \
                 next space in the same direction is occupied by an opponent Worker, that Worker \
                 is removed from the game."
            }
            GodName::Clio => {
                "Place a coin on each of the first three blocks your Workers build. Opponents \
                 treat spaces containing your coins as if they contain only a dome."
            }
            GodName::Maenads => {
                "If your Workers ever neighbor an opponent's Worker on opposite sides, that \
                 opponent loses the game."
            }
            GodName::Zeus => "Your Worker may build a block under itself.",
            GodName::Ares => {
                "You may remove an unoccupied block (not dome) neighboring your unmoved Worker."
            }
            GodName::Eros => {
                "Place your Workers on opposite perimeter spaces. You also win if one of your \
                 Workers moves to a space neighboring your other Worker and both are on the first \
                 level."
            }
            GodName::Selene => {
                "One of your Workers is female. Your female Worker may build a dome at any level, \
                 regardless of which Worker moved."
            }
            GodName::Hippolyta => {
                "One of your Workers is female. All Workers except your female Worker may only \
                 move diagonally."
            }
            GodName::Scylla => {
                "If your Worker moves from a space neighboring an opponent's Worker, you may force \
                 that Worker into the space yours just vacated."
            }
            GodName::Charon => {
                "Before your Worker moves, you may force a neighboring opponent Worker to the \
                 space directly on the other side of your Worker, if that space is unoccupied."
            }
            GodName::Pegasus => {
                "Your Worker may move up more than one level, but cannot win the game by doing so."
            }
            GodName::Proteus => {
                "Place three Workers. After your Worker moves, if possible, force one of your \
                 other Workers into the space it just vacated."
            }
            GodName::Asteria => {
                "If one of your Workers moved down this turn, you may also build a dome on any \
                 unoccupied space."
            }
            GodName::Hydra => {
                "At the end of your turn, if none of your Workers neighbor each other, add a \
                 Worker to a lowest unoccupied space neighboring your moved Worker. Otherwise, \
                 remove one of your Workers."
            }
            GodName::ApolloV2 => {
                "Pantheon Apollo: your Worker may move into a neighboring opponent Worker's space \
                 by forcing their Worker to the space yours just vacated, but only if their Worker \
                 is not higher than yours."
            }
            GodName::Medusa => {
                "If possible, your Workers build in lower neighboring spaces that are occupied by \
                 opponent Workers, removing those Workers from the game."
            }
            GodName::Iris => {
                "If a Worker neighbors your Worker and the space directly on the other side of it \
                 is unoccupied, your Worker may move to that space regardless of its level."
            }
            GodName::Castor => {
                "Castor & Pollux: instead of your normal turn, you may either move with both of \
                 your Workers, or build with both of your Workers."
            }
            GodName::CharonV2 => {
                "Pantheon Charon: instead of moving, you may force a neighboring opponent Worker \
                 to the space directly on the other side of your Worker, if that space is \
                 unoccupied. Then build as normal."
            }
            GodName::Polyphemus => {
                "Once per game, your Worker builds up to two domes at any level on any unoccupied \
                 spaces on the board."
            }
            GodName::Nike => {
                "If one of your Workers moved down on your last turn, opponent Workers cannot move \
                 up this turn."
            }
            GodName::Nemesis => {
                "If none of an opponent's Workers neighbor yours, you may force your Workers and \
                 the opponent's Workers to swap spaces."
            }
            GodName::Poseidon => {
                "If your unmoved Worker is on the ground level, it may build up to three times."
            }
            GodName::Bellerophon => "Once per game, your Worker moves up two levels.",
            GodName::Chronus => {
                "You also win when there are at least five complete towers on the board."
            }
            GodName::Theseus => {
                "Once per game, if one of your Workers is exactly two levels below a neighboring \
                 opponent Worker, remove that opponent Worker from play."
            }
            GodName::Jason => {
                "Once per game, before moving, place your extra Worker on an unoccupied \
                 ground-level perimeter space, then take your turn with that Worker."
            }
            GodName::Achilles => {
                "Once per game, your Worker builds both before and after moving."
            }
            GodName::Stymphalians => {
                "Place three Workers. Your Worker must move two or three times, and cannot end its \
                 move on a space neighboring where it started."
            }
            GodName::Chronus4T => {
                "Chronus variant: you also win when there are at least four complete towers on the \
                 board."
            }
            GodName::Chronus3T => {
                "Chronus variant: you also win when there are at least three complete towers on \
                 the board."
            }
            GodName::Terpsichore => "All of your Workers must move, and then all must build.",
            GodName::Odysseus => {
                "Once per game at the start of your turn, force to unoccupied corner spaces any \
                 number of opponent Workers that neighbor your Workers."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::gods::ALL_GODS_BY_ID;

    #[test]
    fn every_god_has_a_description() {
        for god in ALL_GODS_BY_ID.iter() {
            assert!(
                !god.god_name.description().is_empty(),
                "{:?} has an empty description",
                god.god_name
            );
        }
    }
}
