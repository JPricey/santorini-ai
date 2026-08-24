//! Circe, Divine Enchantress.
//!
//! "At the start of your turn, if an opponent's Workers do not neighbor each other, you alone have
//! use of their power until your next turn."
//!
//! Circe has no moves of her own - while she is not stealing she plays exactly as a Mortal, so
//! this module borrows Mortal's generator and move encoding wholesale. The interesting part of
//! Circe lives outside this file:
//!
//! - [`FullGameState::effective_gods`] swaps the two `GodPower` pointers while a steal is active,
//!   so the borrowed power arrives complete - not just its move generator, but its `win_mask`,
//!   its `placement_type`, and the `is_persephone` / `is_aphrodite` / build-mask passives that the
//!   move generation prelude reads straight off the struct.
//! - [`BoardState::update_circe_steal`] recomputes the steal at the top of each Circe turn.
//! - [`BoardState::data_player`] answers "whose slot holds the power's state", which is how the
//!   borrowed power keeps reading and writing its own `god_data` while Circe is the one using it.
//!
//! [`FullGameState::effective_gods`]: crate::board::FullGameState::effective_gods
//! [`BoardState::update_circe_steal`]: crate::board::BoardState::update_circe_steal
//! [`BoardState::data_player`]: crate::board::BoardState::data_player

use crate::{
    board::{BoardState, GodData},
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions, god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
    player::Player,
};

/// Bit 31 of Circe's `god_data`, set while she is holding the opponent's power.
///
/// Circe's slot holds this bit and nothing else - the stolen power's own state stays in its
/// owner's slot for the whole game. That is what makes the bit self-describing: no other god
/// writes bit 31, so the slot carrying it *is* Circe's and a steal *is* in progress, which lets
/// [`BoardState::data_player`] redirect the borrowed power's reads and writes without knowing
/// anything about who is playing which god.
///
/// It also means the bit cannot be clobbered. A stolen `set_god_data` redirects to the owner's
/// slot, so it never touches Circe's u32.
///
/// [`BoardState::data_player`]: crate::board::BoardState::data_player
pub const CIRCE_STEAL_BIT: GodData = 1 << 31;

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        "stealing" => Ok(CIRCE_STEAL_BIT),
        _ => Err(format!("Unknown god data format: {}", data)),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data & CIRCE_STEAL_BIT {
        0 => None,
        _ => Some("stealing".to_string()),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] & CIRCE_STEAL_BIT {
        0 => None,
        _ => Some("Holding the opponent's power".to_string()),
    }
}

pub const fn build_circe() -> GodPower {
    god_power(
        GodName::Circe,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        14640178244090551025,
        7535102153977020819,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_is_circe()
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{fen::parse_fen, gods::GodName, player::Player, square::Square};

    /// Circe holding Hydra's power grows her own side, and keeps the Workers when it goes back.
    #[test]
    fn steals_hydras_worker_growth() {
        use crate::{fen::parse_fen, player::Player};

        // Both sides' Workers are apart: Circe holds the power, and her own Workers being
        // non-neighbouring is what makes Hydra's power add one rather than remove one.
        let mut state =
            parse_fen("00000 00000 00000 00000 00000/1/circe:A1,E1/hydra:A5,E5").unwrap();
        state.board.update_circe_steal(state.gods);
        assert_eq!(state.get_active_god().god_name, GodName::Hydra);
        assert_eq!(state.board.workers[0].count_ones(), 2);

        let (active, other) = state.get_active_non_active_gods();
        let grown = active
            .get_all_moves(&state, Player::One)
            .into_iter()
            .map(|m| state.next_state(active, other, m.action))
            .find(|next| next.board.workers[0].count_ones() == 3)
            .expect("Circe should be able to grow a third Worker");

        grown.validate();
        assert_eq!(
            grown.board.god_data[0], CIRCE_STEAL_BIT,
            "growing Workers must not leave borrowed state in Circe's slot"
        );

        // The extra Worker is hers to keep: Hydra reunites, the power goes home, and a
        // three-Worker Circe is still a legal position. That is why the Worker cap in
        // `_validate_player` follows the power she *can* hold, not the one she holds now.
        let mut returned = grown.clone();
        returned.board.workers[1] = crate::square::Square::A5.to_board()
            | crate::square::Square::B5.to_board();
        returned.board.current_player = Player::One;
        returned.recalculate_internals();
        returned.board.update_circe_steal(returned.gods);
        assert_eq!(returned.board.god_data[0], 0, "power handed back");
        assert_eq!(returned.get_active_god().god_name, GodName::Circe);
        returned.validate();
    }

    /// Every god Circe may actually face, printed so the ban list is legible at a glance rather
    /// than reconstructed from `matchup.rs`.
    #[test]
    fn ban_list_leaves_a_playable_field() {
        use crate::{
            gods::ALL_GODS_BY_ID,
            matchup::{Matchup, is_matchup_banned},
        };

        let (allowed, banned): (Vec<_>, Vec<_>) = ALL_GODS_BY_ID
            .iter()
            .map(|god| god.god_name)
            .partition(|name| !is_matchup_banned(&Matchup::new(GodName::Circe, *name)));

        eprintln!("Circe may face ({}): {:?}", allowed.len(), allowed);
        eprintln!("Circe is banned against ({}): {:?}", banned.len(), banned);

        // Every god carrying `god_data` that Circe can still meet must have had its slot remapped
        // through `data_player`. Pinning that set here means adding a stateful god without
        // handling it fails loudly rather than corrupting a borrowed power in some deep search.
        let stateful_and_allowed: Vec<_> = allowed
            .iter()
            .copied()
            .filter(|n| {
                matches!(
                    n,
                    GodName::Athena
                        | GodName::Nike
                        | GodName::Aeolus
                        | GodName::Europa
                        | GodName::Morpheus
                )
            })
            .collect();
        assert_eq!(
            stateful_and_allowed.len(),
            5,
            "the stateful five: {stateful_and_allowed:?}"
        );
        assert!(banned.contains(&GodName::Circe), "the mirror must be banned");
    }

    /// Workers apart: Circe holds the power and the owner is reduced to a Mortal.
    /// Workers together: both sides play their own god.
    #[test]
    fn steal_follows_worker_adjacency() {
        // Athena's workers on A1 and E1 - far apart.
        let mut apart =
            parse_fen("00000 00000 00000 00000 00000/1/circe:C3,C4/athena:A1,E1").unwrap();
        apart.board.update_circe_steal(apart.gods);
        let (active, other) = apart.get_active_non_active_gods();
        assert_eq!(
            active.god_name,
            GodName::Athena,
            "Circe should hold Athena's power"
        );
        assert_eq!(
            other.god_name,
            GodName::Mortal,
            "Athena should be powerless"
        );

        // Athena's workers on A1 and A2 - neighbouring.
        let mut together =
            parse_fen("00000 00000 00000 00000 00000/1/circe:C3,C4/athena:A1,A2").unwrap();
        together.board.update_circe_steal(together.gods);
        let (active, other) = together.get_active_non_active_gods();
        assert_eq!(active.god_name, GodName::Circe);
        assert_eq!(other.god_name, GodName::Athena);
    }

    /// The apparatus stays in its owner's slot even while Circe is the one using it.
    #[test]
    fn stolen_state_stays_in_the_owners_slot() {
        let fen = "00000 01000 00000 00000 00000/1/circe:B4,C4/morpheus:A1,E1";
        let mut state = parse_fen(fen).unwrap();
        state.board.update_circe_steal(state.gods);
        assert_eq!(
            state.board.god_data[0], CIRCE_STEAL_BIT,
            "only the steal bit"
        );

        // Circe, wielding Morpheus, reads and writes Morpheus' pile.
        assert_eq!(state.board.data_player(Player::One), Player::Two);
        assert_eq!(state.board.data_player(Player::Two), Player::Two);

        let (active, other) = state.get_active_non_active_gods();
        let next = state.next_state(
            active,
            other,
            active.get_all_moves(&state, Player::One)[0].action,
        );
        assert_eq!(
            next.board.god_data[0], CIRCE_STEAL_BIT,
            "Circe's slot must never pick up borrowed state"
        );
    }

    /// Athena's "moved up last turn" flag is cleared when the power changes hands - it describes
    /// a turn its new holder did not play.
    #[test]
    fn timed_flag_resets_on_handoff() {
        // Athena has moved up (flag set) but her workers are apart, so Circe takes the power.
        let mut state =
            parse_fen("00000 00000 00000 00000 00000/1/circe:C3,C4/athena[^]:A1,E1").unwrap();
        assert_eq!(state.board.god_data[1], 1, "Athena's flag starts set");
        state.board.update_circe_steal(state.gods);
        assert_eq!(state.board.god_data[1], 0, "flag cleared on handoff");
        assert_eq!(state.board.god_data[0], CIRCE_STEAL_BIT);
    }

    /// Circe holding Chronus' power wins the way he does - on the state of the board, not on
    /// where she moves. Four towers already up, so completing the fifth wins on the build; and
    /// with five already up she wins whatever she does.
    #[test]
    fn wins_on_towers_with_chronus_power() {
        // Four domes, and D5 sitting at level 3 ready to be capped. Chronus' Workers are apart,
        // so Circe holds his power.
        let mut state =
            parse_fen("0004344400000000000000000/1/circe:D4,A1/chronus:A5,E1").unwrap();
        state.board.update_circe_steal(state.gods);
        assert_eq!(state.get_active_god().god_name, GodName::Chronus);

        let wins = state
            .get_active_god()
            .get_winning_moves(&state, Player::One);
        assert!(
            !wins.is_empty(),
            "Circe should win by capping the fifth tower: {:?}",
            state
        );

        let (active, other) = state.get_active_non_active_gods();
        let won = state.next_state(active, other, wins[0].action);
        assert_eq!(won.get_winner(), Some(Player::One));

        // And with the towers already up, every move she has is a win.
        let mut already_won =
            parse_fen("0044344400000000000000000/1/circe:D4,A1/chronus:A5,E1").unwrap();
        already_won.board.update_circe_steal(already_won.gods);
        assert_eq!(already_won.board.height_map[3].count_ones(), 5);
        assert!(
            !already_won
                .get_active_god()
                .get_winning_moves(&already_won, Player::One)
                .is_empty(),
            "five towers already up is a win for whoever holds the power"
        );

        // Without the steal she is a Mortal and towers mean nothing to her.
        let no_steal = parse_fen("0044344400000000000000000/1/circe:D4,A1/chronus:A5,B5").unwrap();
        assert_eq!(no_steal.get_active_god().god_name, GodName::Circe);
        assert!(
            no_steal
                .get_active_god()
                .get_winning_moves(&no_steal, Player::One)
                .is_empty(),
            "a Circe with no power does not win on towers"
        );
    }

    /// A full search playing both sides, with the consistency checker run on every position the
    /// game passes through. Morpheus is the sharpest opponent for this: his coins are the one
    /// piece of borrowed apparatus that accumulates, so the search has to keep reading and
    /// writing them out of his slot while Circe is the one spending them.
    #[test]
    fn search_playout_vs_morpheus() {
        run_search_playout(GodName::Morpheus);
    }

    /// The rest of the stateful five, plus the opponents that stress the parts of the design the
    /// move-gen fuzzer cannot reach: a displacer (Circe moving opponent Workers changes the very
    /// adjacency her steal is read from), and the opponent-facing passives whose applicability
    /// flips with the steal.
    #[test]
    fn search_playout_vs_stateful_and_passive_gods() {
        for opponent in [
            // The rest of the stateful five - each a different shape of borrowed apparatus.
            GodName::Athena,
            GodName::Aeolus,
            GodName::Europa,
            // A displacer: Circe moving opponent Workers changes the very adjacency her steal is
            // read from two plies later.
            GodName::Apollo,
            // Opponent-facing passives, which change hands wholesale along with the power.
            GodName::Hypnus,
            GodName::Persephone,
        ] {
            run_search_playout(opponent);
        }
    }

    fn run_search_playout(opponent: GodName) {
        use crate::{
            board::GameStateBuilder,
            consistency_checker::consistency_check,
            search::{SearchContext, get_win_reached_search_terminator, negamax_search},
            search_terminators::DynamicMaxDepthSearchTerminator,
            square::Square::*,
            transposition_table::TranspositionTable,
        };

        let mut state = GameStateBuilder::new(GodName::Circe, opponent)
            .with_p1_worker(B2)
            .with_p1_worker(D4)
            .with_p2_worker(B4)
            .with_p2_worker(D2)
            .build();

        let mut tt = TranspositionTable::new();
        let mut saw_steal = false;
        let mut saw_no_steal = false;

        for _ in 0..14 {
            if state.board.get_winner().is_some() {
                break;
            }
            consistency_check(&state).unwrap();

            // Circe's slot never accumulates borrowed state, whatever Morpheus' pile is doing.
            assert!(
                state.board.god_data[0] & !CIRCE_STEAL_BIT == 0,
                "vs {opponent}: Circe's slot picked up borrowed state: {:?}",
                state
            );
            if state.board.current_player == Player::One {
                let stealing = state.board.god_data[0] & CIRCE_STEAL_BIT != 0;
                saw_steal |= stealing;
                saw_no_steal |= !stealing;
                assert_eq!(
                    state.get_active_god().god_name,
                    if stealing {
                        opponent
                    } else {
                        GodName::Circe
                    },
                );
            }

            let mut search_context = SearchContext {
                tt: &mut tt,
                new_best_move_callback: Box::new(|_| {}),
                terminator: DynamicMaxDepthSearchTerminator::new(3),
            };
            let search_state = negamax_search(
                &mut search_context,
                state.clone(),
                get_win_reached_search_terminator(),
            );

            let best_move = search_state.best_move.expect("Search found no move");
            let (active_god, oppo_god) = state.get_active_non_active_gods();
            state = state.next_state(active_god, oppo_god, best_move.action);
        }

        assert!(
            saw_steal && saw_no_steal,
            "vs {opponent}: the steal never toggled (steal seen: {saw_steal}, no-steal seen: \
             {saw_no_steal}) - the handoff was not exercised"
        );
    }

    /// Morpheus' coins are accumulated apparatus with no expiry, so they survive the handoff.
    #[test]
    fn accumulated_apparatus_survives_handoff() {
        let mut state =
            parse_fen("00000 00000 00000 00000 00000/1/circe:C3,C4/morpheus[4]:A1,E1").unwrap();
        assert_eq!(state.board.god_data[1], 4);
        state.board.update_circe_steal(state.gods);
        assert_eq!(
            state.board.god_data[1], 4,
            "coins are not reset by the steal"
        );
    }
}
