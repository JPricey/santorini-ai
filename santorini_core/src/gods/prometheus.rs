use crate::{
    bitboard::{
        BETWEEN_MAPPING, BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, NEIGHBOR_MAP,
        apply_mapping_to_mask,
    },
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::prometheus_slide,
        move_helpers::{
            WorkerNextMoveState, build_scored_move, get_basic_moves, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
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
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const PRE_BUILD_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const NO_PRE_BUILD_VALUE: MoveData = 25 << PRE_BUILD_POSITION_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
struct PrometheusMove(pub MoveData);

impl Into<GenericMove> for PrometheusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for PrometheusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl PrometheusMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | NO_PRE_BUILD_VALUE;

        Self(data)
    }

    pub fn new_pre_build_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        pre_build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((pre_build_position as MoveData) << PRE_BUILD_POSITION_OFFSET);

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
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn pre_build_position(self) -> Option<Square> {
        let value = (self.0 >> PRE_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for PrometheusMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            write!(f, "{}>{}#", move_from, move_to)
        } else if let Some(pre_build) = self.pre_build_position() {
            write!(f, "^{} {}>{}^{}", pre_build, move_from, move_to, build)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for PrometheusMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(move_from),
                PartialAction::MoveWorker(move_to.into()),
            ]];
        }

        let build_position = self.build_position();

        if let Some(pre_build_position) = self.pre_build_position() {
            let mut res = vec![vec![
                PartialAction::Build(pre_build_position),
                PartialAction::SelectWorker(move_from),
                PartialAction::MoveWorker(move_to.into()),
                PartialAction::Build(build_position),
            ]];

            let from_neighbors = NEIGHBOR_MAP[move_from as usize];
            let to_neighbors = NEIGHBOR_MAP[move_to as usize];
            let both_neighbors = from_neighbors & to_neighbors;

            // Harpies can push the worker through a square on its way to move_to. Building on
            // that square before the move can block the step onto it or change where the push
            // ends, so a build there is not interchangeable. Pushes of more than one square
            // leave nothing adjacent to both ends, so the single between square covers it.
            let pushed_through_mask = BETWEEN_MAPPING[move_from as usize][move_to as usize]
                .map_or(BitBoard::EMPTY, BitBoard::as_mask);

            let pre_build_mask = BitBoard::as_mask(pre_build_position);
            let build_mask = BitBoard::as_mask(build_position);
            let are_builds_interchangeable = (both_neighbors & pre_build_mask).is_not_empty()
                && (both_neighbors & build_mask).is_not_empty()
                && ((pre_build_mask | build_mask) & pushed_through_mask).is_empty();

            if are_builds_interchangeable {
                res.push(vec![
                    PartialAction::Build(build_position),
                    PartialAction::SelectWorker(move_from),
                    PartialAction::MoveWorker(move_to.into()),
                    PartialAction::Build(pre_build_position),
                ]);
            }

            res
        } else {
            vec![vec![
                PartialAction::SelectWorker(move_from),
                PartialAction::MoveWorker(move_to.into()),
                PartialAction::Build(build_position),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        {
            let build_position = self.build_position();
            board.build_up(build_position);
        }

        if let Some(build_position) = self.pre_build_position() {
            board.build_up(build_position);
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_maybe_square_with_height(board, self.pre_build_position());
        helper.get()
    }
}

pub fn prometheus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if state.gods[!player as usize].is_harpies() {
        _prometheus_move_gen::<F, MUST_CLIMB, true>(state, player, key_squares)
    } else {
        _prometheus_move_gen::<F, MUST_CLIMB, false>(state, player, key_squares)
    }
}

fn _prometheus_move_gen<
    const F: MoveGenFlags,
    const MUST_CLIMB: bool,
    const AGAINST_HARPIES: bool,
>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(prometheus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.mate_start_mask;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let neighbor_moves_map = prelude.standard_neighbor_map;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let mut worker_moves = get_basic_moves::<MUST_CLIMB>(&prelude, &worker_start_state);

        if is_mate_only::<F>() || worker_start_state.can_mate {
            let moves_to_level_3 = worker_moves & worker_start_state.winnable_squares;
            if push_winning_moves::<F, _, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                PrometheusMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_threatening_workers =
            (worker_start_state.other_own_workers) & prelude.exactly_level_2;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &neighbor_moves_map);

        let unblocked_squares =
            !(worker_start_state.all_non_moving_workers | prelude.domes_and_frozen);
        let pre_build_locations =
            NEIGHBOR_MAP[worker_start_pos as usize] & unblocked_squares & prelude.build_mask;

        if !MUST_CLIMB {
            // Can't go up
            let pre_build_worker_moves =
                worker_moves & !prelude.board.height_map[worker_start_state.worker_start_height];

            let moveable_ontop_of_prebuild = if worker_start_state.worker_start_height == 0 {
                BitBoard::EMPTY
            } else {
                // can't go even, if we prebuilt there
                let mut moveable_ontop_of_prebuild = pre_build_worker_moves
                    & !prelude.board.height_map[worker_start_state.worker_start_height - 1];
                if prelude.is_down_prevented {
                    // If down is allowed, then add back downwards open moves
                    match worker_start_state.worker_start_height {
                        1 => {
                            moveable_ontop_of_prebuild |=
                                prelude.exactly_level_0 & !prelude.all_workers_and_frozen_mask
                        }
                        2 => {
                            moveable_ontop_of_prebuild |=
                                prelude.exactly_level_1 & !prelude.all_workers_and_frozen_mask
                        }
                        3 => {
                            moveable_ontop_of_prebuild |=
                                prelude.exactly_level_2 & !prelude.all_workers_and_frozen_mask
                        }
                        _ => (),
                    }
                }
                moveable_ontop_of_prebuild
            };

            // Destinations the worker can step to whether or not the pre-build landed there
            let dests_reachable_either_way = pre_build_worker_moves & moveable_ontop_of_prebuild;

            // Precompute harpies slides for each neighboring dest, whether or not the pre-build
            // was there
            let mut slide_ends = [(Square::A5, Square::A5); 25];
            if AGAINST_HARPIES {
                for dest in pre_build_worker_moves | moveable_ontop_of_prebuild {
                    let dest_height = prelude.board.get_height(dest);
                    slide_ends[dest as usize] = (
                        prometheus_slide(&prelude, worker_start_pos, dest, dest_height),
                        prometheus_slide(&prelude, worker_start_pos, dest, dest_height + 1),
                    );
                }
            }

            // If we pre-build
            for pre_build_pos in pre_build_locations {
                let pre_build_mask = BitBoard::as_mask(pre_build_pos);

                let worker_moves_with_pre_build = pre_build_worker_moves & !pre_build_mask
                    | pre_build_mask & moveable_ontop_of_prebuild;

                for worker_dest_pos in worker_moves_with_pre_build.into_iter() {
                    let mut worker_end_pos = worker_dest_pos;
                    let mut worker_end_mask = BitBoard::as_mask(worker_end_pos);
                    let mut worker_end_height = prelude.board.get_height(worker_end_pos)
                        + ((worker_end_pos == pre_build_pos) as usize);

                    // Squares that can be built on interchangeably before or after the move.
                    // Only harpies can break that for the destination square, by pushing the
                    // worker on further depending on whether it was built up first.
                    let mut interchangeable_builds = pre_build_locations;

                    if AGAINST_HARPIES {
                        let dest_mask = BitBoard::as_mask(worker_dest_pos);
                        let (plain_end, raised_end) = slide_ends[worker_dest_pos as usize];

                        worker_end_pos = if worker_dest_pos == pre_build_pos {
                            raised_end
                        } else {
                            plain_end
                        };
                        worker_end_mask = BitBoard::as_mask(worker_end_pos);
                        worker_end_height = prelude.board.get_height(worker_end_pos)
                            + ((worker_end_pos == pre_build_pos) as usize);

                        // Building on the destination before the move is only interchangeable
                        // with building on it after if the worker gets pushed to the same square
                        // either way, and is allowed to step there either way.
                        let is_dest_interchangeable = plain_end == raised_end
                            && (dests_reachable_either_way & dest_mask).is_not_empty();

                        if !is_dest_interchangeable {
                            interchangeable_builds &= !dest_mask;
                        }
                    }

                    let is_now_lvl_2 = (worker_end_height == 2) as usize;

                    let post_build_locations = NEIGHBOR_MAP[worker_end_pos as usize]
                        & unblocked_squares
                        & prelude.build_mask;
                    let worker_plausible_next_moves =
                        neighbor_moves_map[worker_end_pos as usize] & unblocked_squares;

                    // Can't dome the pre-build square and then build on it again
                    let mut worker_builds =
                        post_build_locations & !(pre_build_mask & prelude.exactly_level_3);

                    // An interchangeable pair of builds reaches the same position in either order,
                    // so only keep the ordering where the pre-build square sorts last. The
                    // pre-build square has to be interchangeable itself for that, otherwise the
                    // swapped ordering never gets generated and dropping this one loses the move.
                    if (post_build_locations & interchangeable_builds & pre_build_mask)
                        .is_not_empty()
                    {
                        let both_buildable = worker_builds & interchangeable_builds;
                        worker_builds ^=
                            both_buildable & LOWER_SQUARES_EXCLUSIVE_MASK[pre_build_pos as usize];
                    }

                    if is_interact_with_key_squares::<F>() {
                        if ((worker_end_mask | pre_build_mask) & key_squares).is_empty() {
                            worker_builds &= key_squares;
                        }
                    }

                    let own_final_workers = worker_start_state.other_own_workers | worker_end_mask;
                    let reach_board = if prelude.is_against_hypnus
                        && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2
                    {
                        BitBoard::EMPTY
                    } else {
                        (other_threatening_neighbors
                            | (worker_plausible_next_moves
                                & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                            & prelude.win_mask
                            & !own_final_workers
                    };

                    for worker_build_pos in worker_builds {
                        let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                        let is_double_build = pre_build_pos == worker_build_pos;

                        let new_action = PrometheusMove::new_pre_build_move(
                            worker_start_pos,
                            worker_end_pos,
                            worker_build_pos,
                            pre_build_pos,
                        );

                        let is_check = {
                            let final_level_3 = if is_double_build {
                                prelude.exactly_level_1 & pre_build_mask
                                    | prelude.exactly_level_3 & !pre_build_mask
                            } else {
                                let both_build_mask = pre_build_mask | worker_build_mask;
                                prelude.exactly_level_2 & both_build_mask
                                    | prelude.exactly_level_3 & !both_build_mask
                            };
                            let check_board = reach_board & final_level_3 & unblocked_squares;
                            check_board.is_not_empty()
                        };

                        result.push(build_scored_move::<F, _>(new_action, is_check, false))
                    }
                }
            }
        }

        // We dont pre-build
        for worker_end_pos in worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &WorkerNextMoveState {
                    other_threatening_workers,
                    other_threatening_neighbors,
                    worker_moves,
                },
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = PrometheusMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2
                        & BitBoard::as_mask(worker_build_pos))
                        | (prelude.exactly_level_3 & !BitBoard::as_mask(worker_build_pos));
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use crate::{
        fen::parse_fen, gods::PartialAction, move_verifier::MoveVerifier, square::Square::*,
    };

    // The pre-build and the post-move build are interchangeable only when both squares can be
    // built on before *and* after the move, so only then may one of the two orderings be
    // dropped. Regression: the ordering was pruned by square index alone, which silently lost
    // every build square shared by the start and end squares whenever the pre-build square was
    // out of reach after the move.
    #[test]
    fn test_pre_build_keeps_shared_build_squares() {
        let state = parse_fen("0000000000000000000000000/1/prometheus:C3,A1/mortal:E5,E4").unwrap();

        let next_states = state.get_next_states_interactive();

        // Pre-build on B4 (only reachable from C3), step C3 -> C2, then build on any neighbour
        // of C2 -- including the squares that neighbour both C3 and C2.
        for second_build in [B3, D3, B2, D2] {
            MoveVerifier::new()
                .with_p1_worker_at(C2)
                .without_p1_worker_at(C3)
                .with_height_at(B4, 1)
                .with_height_at(second_build, 1)
                .no_winner()
                .any(&next_states);
        }

        crate::consistency_checker::consistency_check(&state).unwrap();
    }

    // Harpies pushes the C1 worker through D1 on its way to E1. Building D1 first would leave
    // the height 0 worker unable to step onto it, so that ordering must not be offered even
    // though D1 neighbours both ends of the move.
    #[test]
    fn test_no_interchangeable_build_on_pushed_through_square() {
        let state =
            parse_fen("0000000000000000011001000/1/prometheus:C4,C1/harpies:D3,B2").unwrap();

        let illegal_ordering = vec![
            PartialAction::Build(D1),
            PartialAction::SelectWorker(C1),
            PartialAction::MoveWorker(E1.into()),
            PartialAction::Build(D2),
        ];

        assert!(
            !state
                .get_next_states_interactive()
                .iter()
                .any(|s| s.actions == illegal_ordering)
        );
    }

    // The same pair of builds must not be emitted in both orders when either order is legal.
    #[test]
    fn test_interchangeable_builds_are_not_duplicated() {
        let state = parse_fen("0000000000000000000000000/1/prometheus:C3,A1/mortal:E5,E4").unwrap();

        let all_states = state.get_active_god().get_all_next_states(&state);
        let distinct = all_states.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(all_states.len(), distinct.len());
    }
}

pub const fn build_prometheus() -> GodPower {
    god_power(
        GodName::Prometheus,
        build_god_power_movers!(prometheus_move_gen),
        build_god_power_actions::<PrometheusMove>(),
        7255800742029900355,
        11420172211286930201,
    )
}
