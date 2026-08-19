//! Charybdis - the whirlpool monster.
//!
//! At the end of her turn Charybdis may place one of her two whirlpool tokens on any unoccupied
//! space. Whirlpools that get built on (or dug out from under) return to her supply.
//!
//! Once *both* whirlpools are on the board they form a portal that **either** player may use: a
//! worker that moves onto one whirlpool is forced through to the other. The entry leg is an
//! ordinary move and obeys every normal restriction; the exit leg has no height delta at all
//! (see `move_helpers::put_moves_through_portals`). A worker can never win on the entry square,
//! but wins on the exit square "as if it had moved up".
//!
//! Because the exit square is where the worker actually ends up, moves encode the *outcome*: the
//! `move_to_position` of a portal move is the exit whirlpool, never the entry. That keeps every
//! other god's move encoding untouched, and makes "no win on the entry" fall out for free.

use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const TOKEN_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

/// Token position of 25 means "did not place a whirlpool this turn".
const NO_TOKEN: MoveData = 25;

/// Charybdis has exactly two whirlpool tokens; anything on the board is not in her supply.
pub const MAX_WHIRLPOOLS: u32 = 2;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CharybdisMove(pub MoveData);

impl GodMove for CharybdisMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));

        if let Some(token) = self.maybe_token_position() {
            res.push(PartialAction::PlaceWhirlpool(token));
        }

        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);

        // Her own build returns any whirlpool on that square to the supply, and she may then place
        // a token - possibly onto the very square she just built. Resolving both through a single
        // `set_god_data` keeps that ordering unambiguous, and is why the generic post-move sweep in
        // `GodPower::make_move` only cleans up the *opponent's* tokens.
        let mut tokens = BitBoard(board.god_data[player as usize]) & !build_position.to_board();
        if let Some(token) = self.maybe_token_position() {
            tokens |= token.to_board();
        }
        board.set_god_data(player, tokens.0 as GodData);
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        // The portal squares themselves are added generically for every god by
        // `GodPower::get_blocker_board`.
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for CharybdisMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for CharybdisMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl CharybdisMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | (NO_TOKEN << TOKEN_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_token_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        token_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((token_position as MoveData) << TOKEN_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_token_position(self) -> Option<Square> {
        let value = (self.0 >> TOKEN_POSITION_OFFSET) & (LOWER_POSITION_MASK as MoveData);
        if value >= NO_TOKEN {
            None
        } else {
            Some(Square::from(value as u8))
        }
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for CharybdisMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            return write!(f, "{}>{}#", move_from, move_to);
        }

        let build = self.build_position();
        match self.maybe_token_position() {
            Some(token) => write!(f, "{}>{}^{} W{}", move_from, move_to, build, token),
            None => write!(f, "{}>{}^{}", move_from, move_to, build),
        }
    }
}

pub(crate) fn whirlpools(board: &BoardState, player: Player) -> BitBoard {
    BitBoard(board.god_data[player as usize]) & BitBoard::MAIN_SECTION_MASK
}

pub(super) fn charybdis_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(charybdis_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.mate_start_mask;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let tokens = whirlpools(&prelude.board, player);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.can_mate {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & worker_start_state.winnable_squares;
            if push_winning_moves::<F, CharybdisMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                CharybdisMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            // Building on one of her own whirlpools returns it to her supply, so even with both
            // out on the board a build can free one up again - that is the only way she ever gets
            // to relocate a whirlpool.
            let may_place = tokens.count_ones() < MAX_WHIRLPOOLS
                || (worker_next_build_state.all_possible_builds & tokens).is_not_empty();

            let builds = if is_interact_with_key_squares::<F>() && may_place {
                worker_next_build_state.all_possible_builds
            } else {
                worker_next_build_state.narrowed_builds
            };

            for worker_build_pos in builds {
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    (reach_board & final_level_3).is_not_empty()
                };

                let is_narrowed_build =
                    (worker_next_build_state.narrowed_builds & build_mask).is_not_empty();

                if is_narrowed_build {
                    result.push(build_scored_move::<F, _>(
                        CharybdisMove::new_basic_move(
                            worker_start_pos,
                            worker_end_move_state.worker_end_pos,
                            worker_build_pos,
                        ),
                        is_check,
                        worker_end_move_state.is_improving,
                    ));
                }

                let remaining = tokens & !build_mask;
                if remaining.count_ones() >= MAX_WHIRLPOOLS {
                    continue;
                }

                // "Any unoccupied space", evaluated *after* the move and build: the worker has
                // vacated its start square and arrived at its end square, the build may have just
                // completed a dome, and a whirlpool that was built on is back in her hand.
                let occupied_after = (prelude.all_workers_and_frozen_mask
                    ^ worker_start_state.worker_start_mask)
                    | worker_end_move_state.worker_end_mask
                    | prelude.board.at_least_level_4()
                    | (prelude.exactly_level_3 & build_mask)
                    | remaining;
                let mut token_squares = !occupied_after & BitBoard::MAIN_SECTION_MASK;

                if is_interact_with_key_squares::<F>() && !is_narrowed_build {
                    // This build does not touch a key square, so the move only survives if the
                    // *placement* is what defuses the win. That works by arming the portal: a
                    // worker that lands on either whirlpool is flushed over to the other one, so
                    // the win evaporates. A lone whirlpool teleports nobody and can never block.
                    if remaining.count_ones() != MAX_WHIRLPOOLS - 1 {
                        token_squares = BitBoard::EMPTY;
                    } else if (remaining & key_squares).is_empty() {
                        // The whirlpool already out there is not on a square anybody wants, so the
                        // new one has to be the square that gets covered.
                        token_squares &= key_squares;
                    }
                    // Otherwise the standing whirlpool is the one being landed on, and *any*
                    // placement arms the portal that flushes them off it.
                }

                for token_pos in token_squares {
                    result.push(build_scored_move::<F, _>(
                        CharybdisMove::new_token_move(
                            worker_start_pos,
                            worker_end_move_state.worker_end_pos,
                            worker_build_pos,
                            token_pos,
                        ),
                        is_check,
                        worker_end_move_state.is_improving,
                    ));
                }
            }
        }
    }

    result
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Ok(0);
    }

    let mut res = BitBoard::EMPTY;
    for part in trimmed.split(',') {
        let square: Square = part
            .trim()
            .parse()
            .map_err(|e| format!("Failed to parse square {}: {:?}", part, e))?;
        res |= BitBoard::as_mask(square);
    }

    if res.count_ones() > MAX_WHIRLPOOLS {
        return Err(format!(
            "Charybdis has only {} whirlpools, got {}",
            MAX_WHIRLPOOLS,
            res.count_ones()
        ));
    }

    Ok(res.0 as GodData)
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        x => Some(
            BitBoard(x)
                .all_squares()
                .iter()
                .map(Square::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => None,
        x => Some(format!(
            "Whirlpools at {}",
            BitBoard(x)
                .all_squares()
                .iter()
                .map(Square::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn get_token_mask(board: &BoardState, player: Player) -> BitBoard {
    whirlpools(board, player)
}

fn flip_horizontal(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_horizontal().0 as GodData
}

fn flip_vertical(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_vertical().0 as GodData
}

fn flip_transpose(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_transpose().0 as GodData
}

pub(super) const fn build_charybdis() -> GodPower {
    god_power(
        GodName::Charybdis,
        build_god_power_movers!(charybdis_move_gen),
        build_god_power_actions::<CharybdisMove>(),
        9029134172295705300,
        3707725897804464561,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_get_token_mask_fn(get_token_mask)
    .with_flip_god_data_horizontal_fn(flip_horizontal)
    .with_flip_god_data_vertical_fn(flip_vertical)
    .with_flip_god_data_transpose_fn(flip_transpose)
}

#[cfg(test)]
mod tests {
    use crate::{
        board::GameStateBuilder,
        fen::{game_state_to_fen, parse_fen},
        gods::mortal::MortalMove,
        search::{
            SearchContext, WINNING_SCORE_BUFFER, get_win_reached_search_terminator, negamax_search,
        },
        search_terminators::DynamicMaxDepthSearchTerminator,
        square::Square::*,
        transposition_table::TranspositionTable,
    };

    use super::*;

    fn with_whirlpools(mut state: FullGameState, player: Player, squares: &[Square]) -> FullGameState {
        let mut mask = BitBoard::EMPTY;
        for square in squares {
            mask |= BitBoard::as_mask(*square);
        }
        state.board.set_god_data(player, mask.0 as GodData);
        state
    }

    /// Where can the worker on `from` actually end up? Every god in this test module encodes
    /// from/to in the same low bits, so reading them back as a `MortalMove` is safe.
    fn destinations_from(state: &FullGameState, player: Player, from: Square) -> BitBoard {
        let mut res = BitBoard::EMPTY;
        for scored in state.gods[player as usize].get_all_moves(state, player) {
            let action: MortalMove = scored.action.into();
            if action.move_from_position() == from {
                res |= BitBoard::as_mask(action.move_to_position());
            }
        }
        res
    }

    fn winning_destinations(state: &FullGameState, player: Player) -> BitBoard {
        let mut res = BitBoard::EMPTY;
        for scored in state.gods[player as usize].get_winning_moves(state, player) {
            let action: MortalMove = scored.action.into();
            res |= BitBoard::as_mask(action.move_to_position());
        }
        res
    }

    #[test]
    fn test_charybdis_fen_round_trip() {
        for squares in [vec![], vec![C4], vec![C4, E1]] {
            let state = with_whirlpools(
                GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                    .with_p1_worker(A1)
                    .with_p1_worker(A2)
                    .with_p2_worker(C3)
                    .with_p2_worker(E5)
                    .build(),
                Player::One,
                &squares,
            );

            let fen = game_state_to_fen(&state);
            let parsed = parse_fen(&fen).unwrap();
            assert_eq!(parsed, state, "round trip failed for {}", fen);
        }
    }

    #[test]
    fn test_a_lone_whirlpool_is_an_ordinary_square() {
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4],
        );

        let destinations = destinations_from(&state, Player::Two, C3);
        assert!(destinations.contains_square(C4));
    }

    #[test]
    fn test_the_opponent_is_pulled_through_the_portal() {
        // C4 is next to the opponent's worker; E1 is nowhere near it.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let destinations = destinations_from(&state, Player::Two, C3);
        assert!(
            destinations.contains_square(E1),
            "stepping into C4 should surface at E1"
        );
        assert!(
            !destinations.contains_square(C4),
            "the worker is forced off the whirlpool it entered"
        );
    }

    #[test]
    fn test_an_occupied_partner_disables_the_portal() {
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(E1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let destinations = destinations_from(&state, Player::Two, C3);
        assert!(destinations.contains_square(C4));
        assert!(!destinations.contains_square(E1));
    }

    #[test]
    fn test_the_exit_wins_from_any_height() {
        // The mate that every "you must be on level 2" fast path would miss: a worker standing on
        // the ground steps into a whirlpool and surfaces on level 3.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_height(E1, 3)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E1],
        );

        assert!(
            winning_destinations(&state, Player::Two).contains_square(E1),
            "surfacing on a level 3 whirlpool wins"
        );
    }

    #[test]
    fn test_no_win_on_the_whirlpool_you_entered() {
        // A whirlpool on level 3 is a trap, not a win: the climber is flushed straight back down.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_height(C3, 2)
                .with_height(C4, 3)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E1],
        );

        assert!(
            winning_destinations(&state, Player::Two).is_empty(),
            "climbing onto a whirlpool is never a win"
        );
        assert!(destinations_from(&state, Player::Two, C3).contains_square(E1));
    }

    #[test]
    fn test_a_level_3_worker_does_not_win_by_moving_sideways() {
        // Regression: an armed level 3 portal widens mate detection to every worker, and a worker
        // already standing on level 3 must not have its flat move tagged as a win.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C2)
                .with_p2_worker(E5)
                .with_height(C2, 3)
                .with_height(C3, 3)
                .with_height(D1, 3)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[B1, D1],
        );

        let wins = winning_destinations(&state, Player::Two);
        assert!(
            !wins.contains_square(C3),
            "level 3 to level 3 is not moving up"
        );
        // ...but the same worker can still win by dropping into B1 and surfacing on D1.
        assert!(
            wins.contains_square(D1),
            "the portal exit wins even for a worker coming down from level 3"
        );
    }

    #[test]
    fn test_a_worker_can_be_flushed_back_to_where_it_started() {
        // Standing on one whirlpool and stepping into the other sends the worker straight back:
        // a legal move that ends where it began, and therefore a free build.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C4)
                .with_p2_worker(E5)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, C5],
        );

        let destinations = destinations_from(&state, Player::Two, C4);
        assert!(
            destinations.contains_square(C4),
            "the worker is sent back to its own square"
        );
        assert!(!destinations.contains_square(C5));
    }

    #[test]
    fn test_athena_restricts_the_entry_and_not_the_exit() {
        // Athena stops the climb *into* a whirlpool, but the exit is not a climb at all.
        let mut state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Athena)
                .with_p1_worker(C3)
                .with_p1_worker(A2)
                .with_p2_worker(E5)
                .with_p2_worker(E4)
                .with_height(E1, 3)
                .with_current_player(Player::One)
                .build(),
            Player::One,
            &[C4, E1],
        );
        state.board.set_god_data(Player::Two, 1);

        assert!(
            destinations_from(&state, Player::One, C3).contains_square(E1),
            "a flat entry is legal under Athena, however high the exit is"
        );

        let raised = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Athena)
                .with_p1_worker(C3)
                .with_p1_worker(A2)
                .with_p2_worker(E5)
                .with_p2_worker(E4)
                .with_height(C4, 1)
                .with_height(E1, 3)
                .with_current_player(Player::One)
                .build(),
            Player::One,
            &[C4, E1],
        );
        let mut raised = raised;
        raised.board.set_god_data(Player::Two, 1);

        assert!(
            !destinations_from(&raised, Player::One, C3).contains_square(E1),
            "Athena forbids climbing into the whirlpool"
        );
    }

    #[test]
    fn test_pan_does_not_win_by_falling_through_a_whirlpool() {
        // Using a whirlpool counts as moving up, so Pan gets nothing from surfacing on the ground.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Pan)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_height(C3, 2)
                .with_height(C4, 2)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let wins = winning_destinations(&state, Player::Two);
        assert!(
            !wins.contains_square(E1),
            "a whirlpool exit is not a two level drop"
        );
        // Control: the same worker still wins by walking down to an ordinary ground square.
        assert!(wins.contains_square(B2));
    }

    #[test]
    fn test_building_on_a_whirlpool_returns_it_to_the_supply() {
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_current_player(Player::Two)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let mortal = GodName::Mortal.to_power();
        let charybdis = GodName::Charybdis.to_power();

        let mut checked = false;
        for scored in mortal.get_all_moves(&state, Player::Two) {
            let action: MortalMove = scored.action.into();
            if action.build_position() != C4 {
                continue;
            }

            let next = state.next_state(mortal, charybdis, scored.action);
            assert!(
                !whirlpools(&next.board, Player::One).contains_square(C4),
                "building on a whirlpool returns it to the supply"
            );
            assert_eq!(next.board.god_data[Player::One as usize].count_ones(), 1);
            checked = true;
        }
        assert!(checked, "expected at least one build onto C4");
    }

    #[test]
    fn test_charybdis_places_at_most_one_whirlpool_on_a_free_square() {
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .with_height(E4, 4)
                .build(),
            Player::One,
            &[C4],
        );

        let charybdis = GodName::Charybdis.to_power();
        let mut placements = BitBoard::EMPTY;

        for scored in charybdis.get_all_moves(&state, Player::One) {
            let action: CharybdisMove = scored.action.into();
            let Some(token) = action.maybe_token_position() else {
                continue;
            };

            // Builds that land on C4 return that whirlpool to her hand, which makes C4 itself a
            // legal placement again - see the test below. Only look at the others here.
            if action.build_position() != C4 {
                placements |= BitBoard::as_mask(token);
            }

            let next = state.next_state(charybdis, GodName::Mortal.to_power(), scored.action);
            let after = whirlpools(&next.board, Player::One);
            assert!(
                after.contains_square(token),
                "{:?} should leave a whirlpool where it placed one",
                action
            );
            assert!(
                after.count_ones() <= MAX_WHIRLPOOLS,
                "{:?} left too many whirlpools",
                action
            );
        }

        assert!(!placements.contains_square(C4), "never onto the other whirlpool");
        assert!(!placements.contains_square(E4), "never onto a dome");
        assert!(!placements.contains_square(C3), "never onto a worker");
        assert!(!placements.contains_square(E5), "never onto a worker");
        assert!(placements.contains_square(D3), "ordinary free squares are fine");
    }

    #[test]
    fn test_building_on_her_own_whirlpool_frees_it_to_be_replaced() {
        // Her only way to relocate a whirlpool: build under one, which returns it to her supply,
        // then place it somewhere else at the end of the same turn.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(C3)
                .with_p1_worker(A1)
                .with_p2_worker(E5)
                .with_p2_worker(E4)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let charybdis = GodName::Charybdis.to_power();
        let mut found = false;

        for scored in charybdis.get_all_moves(&state, Player::One) {
            let action: CharybdisMove = scored.action.into();
            let Some(token) = action.maybe_token_position() else {
                continue;
            };

            let build = action.build_position();
            assert!(
                build == C4 || build == E1,
                "{:?}: with both whirlpools out, only a build that frees one lets her place",
                action
            );

            let next = state.next_state(charybdis, GodName::Mortal.to_power(), scored.action);
            let after = whirlpools(&next.board, Player::One);
            assert_eq!(after.count_ones(), MAX_WHIRLPOOLS);
            assert!(after.contains_square(token));
            // The whirlpool that was not built on stays exactly where it was.
            let untouched = if build == C4 { E1 } else { C4 };
            assert!(after.contains_square(untouched));
            found |= build == C4;
        }

        assert!(found, "expected a build on C4 followed by a placement");
    }

    #[test]
    fn test_no_placements_once_both_whirlpools_are_out() {
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .build(),
            Player::One,
            &[C4, E1],
        );

        for scored in GodName::Charybdis.to_power().get_all_moves(&state, Player::One) {
            let action: CharybdisMove = scored.action.into();
            if action.maybe_token_position().is_some() {
                // The only exception: a build that returns one of them to her supply.
                assert!(
                    action.build_position() == C4 || action.build_position() == E1,
                    "{:?} placed a whirlpool she does not have",
                    action
                );
            }
        }
    }

    #[test]
    fn test_search_finds_a_whirlpool_mate() {
        // Drives the whole search path - move ordering, the token placements in the move list, the
        // eval - and makes it find a win that no amount of climbing would produce.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(C3)
                .with_p1_worker(A1)
                .with_p2_worker(E5)
                .with_p2_worker(E4)
                .with_height(E1, 3)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            new_best_move_callback: Box::new(move |_new_best_move| {}),
            terminator: DynamicMaxDepthSearchTerminator::new(2),
        };
        let search_state = negamax_search(
            &mut search_context,
            state,
            get_win_reached_search_terminator(),
        );

        let best = search_state.best_move.unwrap();
        assert!(best.score > WINNING_SCORE_BUFFER, "search should see the mate");

        let action: CharybdisMove = best.action.into();
        assert_eq!(action.move_from_position(), C3);
        assert_eq!(action.move_to_position(), E1);
    }

    #[test]
    fn test_whirlpools_are_reported_as_tokens_for_rendering() {
        // The UI draws whatever `get_token_squares` reports, so the whirlpools have to be in it or
        // they are invisible on the board.
        let state = with_whirlpools(
            GameStateBuilder::new(GodName::Charybdis, GodName::Mortal)
                .with_p1_worker(A1)
                .with_p1_worker(A2)
                .with_p2_worker(C3)
                .with_p2_worker(E5)
                .build(),
            Player::One,
            &[C4, E1],
        );

        let (p1_tokens, p2_tokens) = state.get_token_squares();
        assert!(p1_tokens.contains_square(C4));
        assert!(p1_tokens.contains_square(E1));
        assert_eq!(p1_tokens.count_ones(), 2);
        assert!(p2_tokens.is_empty());

        let pretty = crate::pretty_board::state_to_pretty_board(&state);
        let json = serde_json::to_string(&pretty).unwrap();
        assert!(json.contains("C4"), "whirlpools must reach the web UI too: {json}");
    }

    /// Where a given worker can end up, read off the board rather than the move encoding.
    ///
    /// Every god packs its move struct differently, so decoding `move_to_position` only works for
    /// the gods that happen to share Mortal's layout. Applying the move and diffing the worker
    /// masks works for all of them.
    fn outcomes_for_worker(state: &FullGameState, player: Player, from: Square) -> BitBoard {
        let active = state.gods[player as usize];
        let oppo = state.gods[!player as usize];
        let before = state.board.workers[player as usize];
        let whirlpools_before = state.board.god_data[!player as usize];

        let mut res = BitBoard::EMPTY;
        for scored in active.get_all_moves(state, player) {
            let next = state.next_state(active, oppo, scored.action);
            let after = next.board.workers[player as usize];

            // Only moves that leave both whirlpools standing say anything about the portal. A god
            // that builds before it moves (Prometheus, Achilles) can hand a token back and then
            // legitimately stand on the square that used to teleport, which is a different
            // question from whether its destinations route through the portal at all.
            if next.board.god_data[!player as usize] != whirlpools_before {
                continue;
            }

            let vacated = before & !after;
            let arrived = after & !before;

            if vacated == BitBoard::as_mask(from) && arrived.count_ones() == 1 {
                res |= arrived;
            }
        }

        res
    }

    #[test]
    fn test_every_audited_opponent_routes_moves_through_the_portal() {
        // A whirlpool is only real if the *opponent's* generator sends its destinations through
        // it. Gods that build their move list by hand can silently skip that, and no amount of
        // fuzzing will notice: the move list stays perfectly self-consistent, it is just missing
        // the teleport. So assert it directly for every god allowed to face her.
        use crate::matchup::{Matchup, is_matchup_banned};

        let mut checked = 0;
        let mut unattributable = Vec::new();

        for god in crate::gods::ALL_GODS_BY_ID.iter() {
            if is_matchup_banned(&Matchup::new(GodName::Charybdis, god.god_name)) {
                continue;
            }

            let state = with_whirlpools(
                GameStateBuilder::new(GodName::Charybdis, god.god_name)
                    .with_p1_worker(A1)
                    .with_p1_worker(A2)
                    .with_p2_worker(C3)
                    .with_p2_worker(E5)
                    .with_current_player(Player::Two)
                    .build(),
                Player::One,
                &[D4, E1],
            );

            let outcomes = outcomes_for_worker(&state, Player::Two, C3);

            if outcomes.is_empty() {
                // No move moved exactly this one worker and nothing else, so there is nothing to
                // read. Collected and asserted below rather than skipped quietly, so that a god
                // that stops being checkable has to be noticed.
                unattributable.push(god.god_name);
                continue;
            }

            assert!(
                outcomes.contains_square(E1),
                "{:?} does not route its moves through the whirlpool - stepping into D4 has to \
                 surface at E1. Its generator probably builds destinations by hand instead of \
                 going through get_limited_moves_given_move_mask. Outcomes: {}",
                god.god_name,
                outcomes
            );
            assert!(
                !outcomes.contains_square(D4),
                "{:?} left a worker standing on the whirlpool it entered",
                god.god_name
            );

            checked += 1;
        }

        // Hydra's moves can turn a worker into a tower, so a worker mask diff cannot attribute
        // them to a single move. She reaches her destinations through the shared funnel, which is
        // what actually matters here.
        assert_eq!(
            unattributable,
            vec![GodName::Hydra],
            "the set of gods this test cannot read has changed"
        );
        assert!(checked >= 30, "expected to check most gods, got {checked}");
    }
}
