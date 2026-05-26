use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            GeneratorPreludeState, build_scored_move,
            get_basic_moves_from_raw_data_with_custom_blockers, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET_1: usize = 0;
const MOVE_TO_POSITION_OFFSET_1: usize = MOVE_FROM_POSITION_OFFSET_1 + POSITION_WIDTH;
const BUILD_POSITION_1: usize = MOVE_TO_POSITION_OFFSET_1 + POSITION_WIDTH;

const MOVE_FROM_POSITION_OFFSET_2: usize = BUILD_POSITION_1 + POSITION_WIDTH;
const MOVE_TO_POSITION_OFFSET_2: usize = MOVE_FROM_POSITION_OFFSET_2 + POSITION_WIDTH;
const BUILD_POSITION_2: usize = MOVE_TO_POSITION_OFFSET_2 + POSITION_WIDTH;

const NONE_SENTINEL: MoveData = 25;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct TerpsichoreMove(pub MoveData);

impl GodMove for TerpsichoreMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let from1 = self.move_from_position_1();
        let to1 = self.move_to_position_1();

        if self.get_is_winning() {
            if let Some(from2) = self.maybe_move_from_position_2() {
                let to2 = self.move_to_position_2();
                return vec![vec![
                    PartialAction::SelectWorker(from1),
                    PartialAction::MoveWorker(to1.into()),
                    PartialAction::SelectWorker(from2),
                    PartialAction::MoveWorker(to2.into()),
                ]];
            }
            return vec![vec![
                PartialAction::SelectWorker(from1),
                PartialAction::MoveWorker(to1.into()),
            ]];
        }

        if let Some(from2) = self.maybe_move_from_position_2() {
            let to2 = self.move_to_position_2();
            let b1 = self.build_position_1();
            let b2 = self.build_position_2();

            let w1_first_ok = to1 != from2;
            let w2_first_ok = to2 != from1;

            let move_orderings: Vec<Vec<PartialAction>> = {
                let w1_then_w2 = vec![
                    PartialAction::SelectWorker(from1),
                    PartialAction::MoveWorker(to1.into()),
                    PartialAction::SelectWorker(from2),
                    PartialAction::MoveWorker(to2.into()),
                ];
                let w2_then_w1 = vec![
                    PartialAction::SelectWorker(from2),
                    PartialAction::MoveWorker(to2.into()),
                    PartialAction::SelectWorker(from1),
                    PartialAction::MoveWorker(to1.into()),
                ];
                match (w1_first_ok, w2_first_ok) {
                    (true, true) => vec![w1_then_w2, w2_then_w1],
                    (true, false) => vec![w1_then_w2],
                    (false, true) => vec![w2_then_w1],
                    (false, false) => vec![],
                }
            };

            let build_orderings: Vec<(Square, Square)> = if b1 == b2 {
                vec![(b1, b2)]
            } else {
                vec![(b1, b2), (b2, b1)]
            };

            let mut results = Vec::with_capacity(move_orderings.len() * build_orderings.len());
            for moves in &move_orderings {
                for (bb1, bb2) in &build_orderings {
                    let mut seq = moves.clone();
                    seq.push(PartialAction::Build(*bb1));
                    seq.push(PartialAction::Build(*bb2));
                    results.push(seq);
                }
            }
            results
        } else {
            let build = self.build_position_1();
            vec![vec![
                PartialAction::SelectWorker(from1),
                PartialAction::MoveWorker(to1.into()),
                PartialAction::Build(build),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let from1 = self.move_from_position_1();
        let to1 = self.move_to_position_1();
        let mut move_mask = BitBoard::as_mask(from1) ^ BitBoard::as_mask(to1);

        if self.get_is_winning() {
            if let Some(from2) = self.maybe_move_from_position_2() {
                let to2 = self.move_to_position_2();
                move_mask ^= BitBoard::as_mask(from2) ^ BitBoard::as_mask(to2);
            }
            board.worker_xor(player, move_mask);
            board.set_winner(player);
            return;
        }

        if let Some(from2) = self.maybe_move_from_position_2() {
            let to2 = self.move_to_position_2();
            move_mask ^= BitBoard::as_mask(from2) ^ BitBoard::as_mask(to2);
            board.worker_xor(player, move_mask);
            board.build_up(self.build_position_1());
            board.build_up(self.build_position_2());
        } else {
            board.worker_xor(player, move_mask);
            board.build_up(self.build_position_1());
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        let mut mask = BitBoard::as_mask(self.move_from_position_1())
            | BitBoard::as_mask(self.move_to_position_1());
        if let Some(from2) = self.maybe_move_from_position_2() {
            mask |= BitBoard::as_mask(from2) | BitBoard::as_mask(self.move_to_position_2());
        }
        mask
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position_1());
        helper.add_square_with_height(board, self.move_to_position_1());

        if self.get_is_winning() {
            if let Some(from2) = self.maybe_move_from_position_2() {
                helper.add_square_with_height(board, from2);
                helper.add_square_with_height(board, self.move_to_position_2());
            }
        } else if let Some(from2) = self.maybe_move_from_position_2() {
            helper.add_square_with_height(board, from2);
            helper.add_square_with_height(board, self.move_to_position_2());
            helper.add_square_with_height(board, self.build_position_1());
            helper.add_square_with_height(board, self.build_position_2());
        } else {
            helper.add_square_with_height(board, self.build_position_1());
        }

        helper.get()
    }
}

impl Into<GenericMove> for TerpsichoreMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for TerpsichoreMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl TerpsichoreMove {
    pub fn new_basic_move(from: Square, to: Square, build: Square) -> Self {
        let data: MoveData = ((from as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((build as MoveData) << BUILD_POSITION_1)
            | (NONE_SENTINEL << MOVE_FROM_POSITION_OFFSET_2)
            | (NONE_SENTINEL << BUILD_POSITION_2);
        Self(data)
    }

    pub fn new_winning_single_move(from: Square, to: Square) -> Self {
        let data: MoveData = ((from as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | (NONE_SENTINEL << MOVE_FROM_POSITION_OFFSET_2)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn new_double_move(
        from1: Square,
        to1: Square,
        b1: Square,
        from2: Square,
        to2: Square,
        b2: Square,
    ) -> Self {
        let data: MoveData = ((from1 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to1 as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((b1 as MoveData) << BUILD_POSITION_1)
            | ((from2 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((to2 as MoveData) << MOVE_TO_POSITION_OFFSET_2)
            | ((b2 as MoveData) << BUILD_POSITION_2);
        Self(data)
    }

    pub fn new_winning_double_move(
        vacator_from: Square,
        vacator_to: Square,
        winner_from: Square,
        winner_to: Square,
    ) -> Self {
        let data: MoveData = ((vacator_from as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((vacator_to as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((winner_from as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((winner_to as MoveData) << MOVE_TO_POSITION_OFFSET_2)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn move_from_position_1(&self) -> Square {
        Square::from((self.0 >> MOVE_FROM_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_to_position_1(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_move_from_position_2(&self) -> Option<Square> {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_2) as u8 & LOWER_POSITION_MASK;
        if value as MoveData == NONE_SENTINEL {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn move_to_position_2(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET_2) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position_1(&self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_1) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position_2(&self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_2) as u8 & LOWER_POSITION_MASK)
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for TerpsichoreMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }
        let from1 = self.move_from_position_1();
        let to1 = self.move_to_position_1();

        if self.get_is_winning() {
            if let Some(from2) = self.maybe_move_from_position_2() {
                return write!(
                    f,
                    "{}>{} {}>{}#",
                    from1,
                    to1,
                    from2,
                    self.move_to_position_2(),
                );
            }
            return write!(f, "{}>{}#", from1, to1);
        }

        if let Some(from2) = self.maybe_move_from_position_2() {
            write!(
                f,
                "{}>{}^{} {}>{}^{}",
                from1,
                to1,
                self.build_position_1(),
                from2,
                self.move_to_position_2(),
                self.build_position_2(),
            )
        } else {
            write!(f, "{}>{}^{}", from1, to1, self.build_position_1())
        }
    }
}

fn emit_double_move_with_builds<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    w1_start: Square,
    w1_end: Square,
    w2_start: Square,
    w2_end: Square,
    start_height_1: usize,
    start_height_2: usize,
    end_height_1: usize,
    end_height_2: usize,
    w1_end_mask: BitBoard,
    w2_end_mask: BitBoard,
    non_own_worker_blockers: BitBoard,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) {
    let workers_after_mask = w1_end_mask | w2_end_mask;
    let unblocked = !(workers_after_mask | non_own_worker_blockers);

    let possible_builds_1 = NEIGHBOR_MAP[w1_end as usize] & unblocked;
    let possible_builds_2 = NEIGHBOR_MAP[w2_end as usize] & unblocked;

    if possible_builds_1.is_empty() || possible_builds_2.is_empty() {
        return;
    }

    // Squares either worker could build. When both b1 and b2 fall in this set, (b1, b2) and (b2, b1)
    // produce identical board states — keep only the canonical (lower-square-first) ordering.
    let build_overlap = possible_builds_1 & possible_builds_2;

    let reach1 = if end_height_1 == 2 {
        prelude.standard_neighbor_map[w1_end as usize]
    } else {
        BitBoard::EMPTY
    };
    let reach2 = if end_height_2 == 2 {
        prelude.standard_neighbor_map[w2_end as usize]
    } else {
        BitBoard::EMPTY
    };
    let reach = reach1 | reach2;

    let moves_hit_key = (workers_after_mask & key_squares).is_not_empty();
    let is_improving = (end_height_1 == 1 && end_height_1 > start_height_1)
        || (end_height_2 == 2 && end_height_2 > start_height_2);

    for b1 in possible_builds_1 {
        let b1_mask = BitBoard::as_mask(b1);
        let b1_was_lvl_3 = (b1_mask & prelude.exactly_level_3).is_not_empty();
        let b1_in_overlap = (b1_mask & build_overlap).is_not_empty();

        // b1 builds first conceptually; if b1 is at lvl 3 it becomes a dome and b2 can't go there.
        let b2_base = if b1_was_lvl_3 {
            possible_builds_2 & !b1_mask
        } else {
            possible_builds_2
        };

        let b2_options = if is_interact_with_key_squares::<F>()
            && !moves_hit_key
            && (b1_mask & key_squares).is_empty()
        {
            b2_base & key_squares
        } else {
            b2_base
        };

        for b2 in b2_options {
            let b2_mask = BitBoard::as_mask(b2);

            // Dedup swapped assignments when both builds are reachable by either worker.
            if b1_in_overlap && (b2_mask & build_overlap).is_not_empty() && (b2 as u8) < (b1 as u8)
            {
                continue;
            }

            let both_mask = b1_mask | b2_mask;

            // Check detection allows own workers to sit on lvl-3 win squares — we assume
            // a vacate-and-climb is feasible without verifying the vacator has a legal
            // move out. Cheaper, and false positives are tolerated by the search.
            let is_check = {
                let (final_lvl_3, new_domes) = if b1 == b2 {
                    let lvl_3 =
                        (prelude.exactly_level_3 & !b1_mask) | (prelude.exactly_level_1 & b1_mask);
                    let domes = prelude.exactly_level_2 & b1_mask;
                    (lvl_3, domes)
                } else {
                    let lvl_3 = (prelude.exactly_level_3 & !both_mask)
                        | (prelude.exactly_level_2 & both_mask);
                    let domes = prelude.exactly_level_3 & both_mask;
                    (lvl_3, domes)
                };
                let obstacles = non_own_worker_blockers | new_domes;
                let reachable = final_lvl_3 & !obstacles & prelude.win_mask;
                (reach & reachable).is_not_empty()
            };

            let new_action =
                TerpsichoreMove::new_double_move(w1_start, w1_end, b1, w2_start, w2_end, b2);

            result.push(build_scored_move::<F, _>(
                new_action,
                is_check,
                is_improving,
            ));
        }
    }
}

pub(super) fn terpsichore_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(terpsichore_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    let non_own_worker_blockers = prelude.domes_and_frozen | prelude.oppo_workers;
    let unblocked_non_own_workers = !non_own_worker_blockers;

    for worker_start_pos in prelude.acting_workers & prelude.exactly_level_2 {
        let worker_start_mask = BitBoard::as_mask(worker_start_pos);
        let worker_moves = get_basic_moves_from_raw_data_with_custom_blockers::<false>(
            &prelude,
            worker_start_pos,
            worker_start_mask,
            2,
            non_own_worker_blockers,
        );

        let winning_moves = worker_moves & prelude.exactly_level_3 & prelude.win_mask;
        let blocked_by_own = winning_moves & prelude.own_workers;
        let direct_wins = winning_moves ^ blocked_by_own;

        if push_winning_moves::<F, TerpsichoreMove, _>(
            &mut result,
            worker_start_pos,
            direct_wins,
            TerpsichoreMove::new_winning_single_move,
        ) {
            return result;
        }

        for blocker_pos in blocked_by_own {
            let blocker_mask = BitBoard::as_mask(blocker_pos);
            let vacator_moves = get_basic_moves_from_raw_data_with_custom_blockers::<false>(
                &prelude,
                blocker_pos,
                blocker_mask,
                3,
                non_own_worker_blockers | worker_start_mask,
            );
            for vacator_to in vacator_moves & !worker_start_mask {
                let action = TerpsichoreMove::new_winning_double_move(
                    blocker_pos,
                    vacator_to,
                    worker_start_pos,
                    blocker_pos,
                );
                result.push(ScoredMove::new_winning_move(action.into()));
                if is_stop_on_mate::<F>() {
                    return result;
                }
            }
        }
    }

    if is_mate_only::<F>() {
        return result;
    }

    let mut own_workers_iter = prelude.own_workers.into_iter();
    let Some(worker_start_1) = own_workers_iter.next() else {
        return result;
    };

    if let Some(worker_start_2) = own_workers_iter.next() {
        // TODO!
        // if prelude.is_against_harpies {
        // }
        let start_height_1 = prelude.board.get_height(worker_start_1);
        let start_height_2 = prelude.board.get_height(worker_start_2);
        let start_mask_1 = BitBoard::as_mask(worker_start_1);
        let start_mask_2 = BitBoard::as_mask(worker_start_2);

        let moves_1 = get_basic_moves_from_raw_data_with_custom_blockers::<false>(
            &prelude,
            worker_start_1,
            start_mask_1,
            start_height_1,
            non_own_worker_blockers,
        );
        let moves_2 = get_basic_moves_from_raw_data_with_custom_blockers::<false>(
            &prelude,
            worker_start_2,
            start_mask_2,
            start_height_2,
            non_own_worker_blockers,
        );

        let win_squares = prelude.exactly_level_3 & prelude.win_mask;
        let non_win_moves_1 = if start_height_1 == 2 {
            moves_1 & !win_squares
        } else {
            moves_1
        };
        let non_win_moves_2 = if start_height_2 == 2 {
            moves_2 & !win_squares
        } else {
            moves_2
        };

        for to1 in non_win_moves_1 {
            let end_mask_1 = BitBoard::as_mask(to1);
            let end_height_1 = prelude.board.get_height(to1);
            let to1_reachable_by_w2 = (end_mask_1 & non_win_moves_2).is_not_empty();
            let mut moves_2_filtered = non_win_moves_2 & !end_mask_1;
            if to1 == worker_start_2 {
                // Workers can't simultaneously swap squares: if w1 takes w2's start,
                // w2 can't take w1's start (whichever moves first walks into the other).
                moves_2_filtered &= !start_mask_1;
            }

            for to2 in moves_2_filtered {
                let end_mask_2 = BitBoard::as_mask(to2);

                if is_stop_on_mate::<F>() {
                    let worker_swap_exists =
                        to1_reachable_by_w2 && (end_mask_2 & non_win_moves_1).is_not_empty();
                    if worker_swap_exists && (to2 as u8) < (to1 as u8) {
                        continue;
                    }
                }

                let end_height_2 = prelude.board.get_height(to2);

                emit_double_move_with_builds::<F>(
                    &prelude,
                    worker_start_1,
                    to1,
                    worker_start_2,
                    to2,
                    start_height_1,
                    start_height_2,
                    end_height_1,
                    end_height_2,
                    end_mask_1,
                    end_mask_2,
                    non_own_worker_blockers,
                    key_squares,
                    &mut result,
                );
            }
        }
    } else {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_1);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        // Strip winning moves (already emitted above).
        if worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            worker_next_moves.worker_moves ^= moves_to_level_3;
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
                unblocked_non_own_workers,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = TerpsichoreMove::new_basic_move(
                    worker_start_1,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ));
            }
        }
    }

    result
}

pub const fn build_terpsichore() -> GodPower {
    god_power(
        GodName::Terpsichore,
        build_god_power_movers!(terpsichore_move_gen),
        build_god_power_actions::<TerpsichoreMove>(),
        412529686953457099,
        5297169764087678351,
    )
    .with_nnue_god_name(GodName::Castor)
}

#[cfg(test)]
mod tests {
    use crate::{board::GameStateBuilder, move_verifier::MoveVerifier, square::Square::*};

    use super::*;

    #[test]
    fn test_terpsichore_does_not_emit_simultaneous_worker_swap() {
        // Workers adjacent at A1 and B1. The "swap" — w1 A1→B1 while w2 B1→A1 — is
        // physically impossible (whichever moves first walks into the other). The only
        // states that would leave both A1 and B1 occupied after a Terpsi turn are swap
        // states, so we assert none exist. We also sanity-check that the filter isn't
        // over-restricting: at least one move has w1 entering w2's start (B1) while w1
        // vacates A1.
        let state = GameStateBuilder::new(GodName::Terpsichore, GodName::Mortal)
            .with_p1_worker(A1)
            .with_p1_worker(B1)
            .with_p2_worker(E4)
            .with_p2_worker(E5)
            .build();

        let next_states = state.get_next_states_interactive();

        MoveVerifier::new()
            .with_p1_worker_at(A1)
            .with_p1_worker_at(B1)
            .none(&next_states);

        MoveVerifier::new()
            .with_p1_worker_at(B1)
            .without_p1_worker_at(A1)
            .any(&next_states);
    }

    #[test]
    fn test_terpsichore_wins_via_vacate_and_climb() {
        // A1 at height 3 (terpsi worker on top), B1 at height 2 (terpsi worker adjacent).
        // The h3 worker can't normally win — it's already there — and the h2 worker can't
        // climb to A1 because A1 is occupied. But for Terpsi: vacator (h3 worker) moves
        // away, then the h2 worker climbs to A1 and wins. The state arises via
        // displacement / win-prevention gods in practice.
        let state = GameStateBuilder::new(GodName::Terpsichore, GodName::Mortal)
            .with_p1_worker(A1)
            .with_p1_worker(B1)
            .with_p2_worker(E4)
            .with_p2_worker(E5)
            .with_height(A1, 3)
            .with_height(B1, 2)
            .build();

        let next_states = state.get_next_states_interactive();

        // A vacate-and-climb win: p1 wins, worker ends on the climbed-to A1 (h3), but the
        // other worker is no longer at B1 (it vacated and climbed onto A1 itself, while
        // the originally-h3 worker moved away).
        MoveVerifier::new()
            .is_winner(Player::One)
            .with_p1_worker_at(A1)
            .without_p1_worker_at(B1)
            .any(&next_states);
    }
}
