use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, NEIGHBOR_MAP},
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
            GeneratorPreludeState, build_scored_move, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    placement::PlacementType,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const NEMESIS_SWAP_1: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const NEMESIS_SWAP_2: usize = NEMESIS_SWAP_1 + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct NemesisMove(pub MoveData);

fn _nemesis_add_swap_actions(res: &mut Vec<PartialAction>, w: Square, s: Square) {
    res.push(PartialAction::SelectWorker(w));
    res.push(PartialAction::new_move_with_displace(s, w, s));
}

fn _nemesis_f_move_actions(
    base: Vec<PartialAction>,
    w1: Square,
    w2: Square,
    s1: Square,
    s2: Square,
) -> Vec<Vec<PartialAction>> {
    let mut res = Vec::new();
    {
        let mut action_seq = base.clone();
        _nemesis_add_swap_actions(&mut action_seq, w1, s1);
        _nemesis_add_swap_actions(&mut action_seq, w2, s2);
        res.push(action_seq);
    }
    {
        let mut action_seq = base.clone();
        _nemesis_add_swap_actions(&mut action_seq, w2, s2);
        _nemesis_add_swap_actions(&mut action_seq, w1, s1);
        res.push(action_seq);
    }
    res
}

fn _nemesis_2x2_move_actions(
    base: Vec<PartialAction>,
    w1: Square,
    w2: Square,
    s1: Square,
    s2: Square,
) -> Vec<Vec<PartialAction>> {
    let mut res = Vec::new();
    {
        let mut action_seq = base.clone();
        _nemesis_add_swap_actions(&mut action_seq, w1, s1);
        _nemesis_add_swap_actions(&mut action_seq, w2, s2);
        res.push(action_seq);
    }
    {
        let mut action_seq = base.clone();
        _nemesis_add_swap_actions(&mut action_seq, w1, s2);
        _nemesis_add_swap_actions(&mut action_seq, w2, s1);
        res.push(action_seq);
    }
    {
        let mut action_seq = base.clone();
        _nemesis_add_swap_actions(&mut action_seq, w2, s2);
        _nemesis_add_swap_actions(&mut action_seq, w1, s1);
        res.push(action_seq);
    }
    {
        let mut action_seq = base.clone();
        _nemesis_add_swap_actions(&mut action_seq, w2, s1);
        _nemesis_add_swap_actions(&mut action_seq, w1, s2);
        res.push(action_seq);
    }

    res
}

impl GodMove for NemesisMove {
    fn move_to_actions(
        self,
        board: &BoardState,
        player: Player,
        other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }
        res.push(PartialAction::Build(self.build_position()));

        if let Some(s1) = self.maybe_swap_1() {
            if let Some(s2) = self.maybe_swap_2() {
                let other_worker_pos =
                    (board.workers[player as usize] ^ self.move_from_position().to_board()).lsb();

                if s1 == self.move_to_position() {
                    _nemesis_add_swap_actions(&mut res, other_worker_pos, s2);
                    return vec![res];
                }

                if other_god.placement_type == PlacementType::FemaleWorker {
                    return _nemesis_f_move_actions(
                        res,
                        self.move_to_position(),
                        other_worker_pos,
                        s1,
                        s2,
                    );
                } else {
                    return _nemesis_2x2_move_actions(
                        res,
                        self.move_to_position(),
                        other_worker_pos,
                        s1,
                        s2,
                    );
                }
            } else {
                _nemesis_add_swap_actions(&mut res, self.move_to_position(), s1);
                return vec![res];
            }
        }

        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        let move_from = self.move_from_position().to_board();
        let move_to = self.move_to_position().to_board();
        let worker_move_mask = move_from ^ move_to;

        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());

        if let Some(swap_1) = self.maybe_swap_1() {
            if let Some(swap_2) = self.maybe_swap_2() {
                let own_1 = self.move_to_position().to_board();
                let own_2 = (board.workers[player as usize] ^ own_1).lsb().to_board();
                let swap_mask = swap_1.to_board() ^ swap_2.to_board() ^ own_1 ^ own_2;

                if other_god.placement_type == PlacementType::FemaleWorker {
                    board.worker_xor(player, swap_mask);
                    board.oppo_worker_xor(other_god, !player, own_1 ^ swap_1.to_board());
                    board.oppo_worker_xor(other_god, !player, own_2 ^ swap_2.to_board());
                } else {
                    board.worker_xor(player, swap_mask);
                    board.oppo_worker_xor(other_god, !player, swap_mask);
                }
            } else {
                let swap_mask = swap_1.to_board() ^ self.move_to_position().to_board();
                board.worker_xor(player, swap_mask);
                board.oppo_worker_xor(other_god, !player, swap_mask);
            }
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        if let Some(swap_1) = self.maybe_swap_1() {
            helper.add_known_maybe_square_with_height(board, swap_1);
            if let Some(swap_2) = self.maybe_swap_2() {
                helper.add_known_maybe_square_with_height(board, swap_2);
            }
        }
        helper.get()
    }
}

impl Into<GenericMove> for NemesisMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for NemesisMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl NemesisMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << NEMESIS_SWAP_1);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn new_single_swap(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        swap_1: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((swap_1 as MoveData) << NEMESIS_SWAP_1)
            | ((25 as MoveData) << NEMESIS_SWAP_2);
        Self(data)
    }

    fn new_double_swap(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        swap_1: Square,
        swap_2: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((swap_1 as MoveData) << NEMESIS_SWAP_1)
            | ((swap_2 as MoveData) << NEMESIS_SWAP_2);
        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn maybe_swap_1(self) -> Option<Square> {
        let swap_1 = (self.0 >> NEMESIS_SWAP_1) as u8 & LOWER_POSITION_MASK;
        if swap_1 == 25 {
            None
        } else {
            Some(Square::from(swap_1))
        }
    }

    fn maybe_swap_2(self) -> Option<Square> {
        let swap_2 = (self.0 >> NEMESIS_SWAP_2) as u8 & LOWER_POSITION_MASK;
        if swap_2 == 25 {
            None
        } else {
            Some(Square::from(swap_2))
        }
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for NemesisMove {
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
        } else {
            if let Some(swap_1) = self.maybe_swap_1() {
                if let Some(swap_2) = self.maybe_swap_2() {
                    return write!(
                        f,
                        "{}>{}^{}[{},{}]",
                        move_from, move_to, build, swap_1, swap_2
                    );
                } else {
                    return write!(f, "{}>{}^{}[{}]", move_from, move_to, build, swap_1);
                }
            }
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

fn _add_swap_moves<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    end_mask: BitBoard,
    other_own_workers: BitBoard,
    other_threatening_neighbors: BitBoard,
    all_possible_builds: BitBoard,
    unblocked_squares: BitBoard,
    key_squares: BitBoard,
) {
    if prelude.other_god.placement_type == PlacementType::FemaleWorker {
        let mut oppo_iter = prelude.oppo_workers.into_iter();
        let oppo_worker_1 = oppo_iter.next().unwrap();
        let oppo_worker_2 = oppo_iter.next().unwrap();

        let mut reach_board = BitBoard::EMPTY;
        if prelude.board.get_height(oppo_worker_1) == 2 {
            reach_board |= prelude.standard_neighbor_map[oppo_worker_1 as usize];
        }
        if prelude.board.get_height(oppo_worker_2) == 2 {
            reach_board |= prelude.standard_neighbor_map[oppo_worker_2 as usize];
        }
        reach_board &= unblocked_squares;

        let other_worker = other_own_workers.lsb();
        let prev_height =
            prelude.board.get_height(start_pos) + prelude.board.get_height(other_worker);
        let oppo_height =
            prelude.board.get_height(oppo_worker_1) + prelude.board.get_height(oppo_worker_2);
        let is_improving = oppo_height > prev_height;

        for build_pos in all_possible_builds {
            let build_mask = build_pos.to_board();
            let final_level_3 =
                (prelude.exactly_level_2 & build_mask) | (prelude.exactly_level_3 & !build_mask);
            let is_check = (reach_board & final_level_3).is_not_empty();

            {
                let swap_action = NemesisMove::new_double_swap(
                    start_pos,
                    end_pos,
                    build_pos,
                    oppo_worker_1,
                    oppo_worker_2,
                );
                result.push(build_scored_move::<F, _>(
                    swap_action,
                    is_check,
                    is_improving,
                ));
            }

            {
                let swap_action = NemesisMove::new_double_swap(
                    start_pos,
                    end_pos,
                    build_pos,
                    oppo_worker_2,
                    oppo_worker_1,
                );
                result.push(build_scored_move::<F, _>(
                    swap_action,
                    is_check,
                    is_improving,
                ));
            }
        }
        return;
    }

    let swap_count = prelude.own_workers.count_ones();
    match swap_count {
        1 => {
            let prev_height = prelude.board.get_height(start_pos);

            for oppo_worker_pos in prelude.oppo_workers {
                let new_height = prelude.board.get_height(oppo_worker_pos);

                let swapped_reach = if new_height == 2 {
                    prelude.standard_neighbor_map[oppo_worker_pos as usize]
                        & prelude.win_mask
                        & unblocked_squares
                } else {
                    BitBoard::EMPTY
                };

                let narrowed_builds = if is_interact_with_key_squares::<F>() {
                    if ((end_mask | oppo_worker_pos.to_board()) & key_squares).is_empty() {
                        all_possible_builds & key_squares
                    } else {
                        all_possible_builds
                    }
                } else {
                    all_possible_builds
                };

                let is_improving = new_height > prev_height;

                for build_pos in narrowed_builds {
                    let swap_action = NemesisMove::new_single_swap(
                        start_pos,
                        end_pos,
                        build_pos,
                        oppo_worker_pos,
                    );

                    let build_mask = build_pos.to_board();
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let is_check = (final_level_3 & swapped_reach).is_not_empty();
                    result.push(build_scored_move::<F, _>(
                        swap_action,
                        is_check,
                        is_improving,
                    ));
                }
            }
        }
        2 => {
            let other_worker = other_own_workers.lsb();
            let other_worker_height = prelude.board.get_height(other_worker);

            match prelude.oppo_workers.count_ones() {
                0 => {}
                1 => {
                    let oppo_worker_pos = prelude.oppo_workers.lsb();

                    let moving_start_height = prelude.board.get_height(start_pos);
                    let moving_end_height = prelude.board.get_height(end_pos);
                    let oppo_height = prelude.board.get_height(oppo_worker_pos);

                    let swap_moving_improver = oppo_height > moving_start_height;
                    let swap_standing_improver = moving_end_height > moving_start_height
                        || other_worker_height > other_worker_height;

                    let moving_worker_reach = if moving_end_height == 2 {
                        prelude.standard_neighbor_map[end_pos as usize]
                    } else {
                        BitBoard::EMPTY
                    };

                    let swapped_reach = if oppo_height == 2 {
                        prelude.standard_neighbor_map[oppo_worker_pos as usize]
                    } else {
                        BitBoard::EMPTY
                    };

                    let moving_worker_full_reach = (other_threatening_neighbors | swapped_reach)
                        & unblocked_squares
                        & prelude.win_mask;
                    let other_worker_full_reach = (moving_worker_reach | swapped_reach)
                        & unblocked_squares
                        & prelude.win_mask;

                    for build_pos in all_possible_builds {
                        let build_mask = build_pos.to_board();
                        let final_level_3 = (prelude.exactly_level_2 & build_mask)
                            | (prelude.exactly_level_3 & !build_mask);

                        {
                            // Swap moving worker
                            let swap_action = NemesisMove::new_single_swap(
                                start_pos,
                                end_pos,
                                build_pos,
                                oppo_worker_pos,
                            );
                            result.push(build_scored_move::<F, _>(
                                swap_action,
                                (moving_worker_full_reach & final_level_3).is_not_empty(),
                                swap_moving_improver,
                            ));
                        }

                        {
                            // Swap other worker
                            let swap_action = NemesisMove::new_double_swap(
                                start_pos,
                                end_pos,
                                build_pos,
                                end_pos,
                                oppo_worker_pos,
                            );
                            result.push(build_scored_move::<F, _>(
                                swap_action,
                                (other_worker_full_reach & final_level_3).is_not_empty(),
                                swap_standing_improver,
                            ));
                        }
                    }
                }
                2 => {
                    let mut oppo_iter = prelude.oppo_workers.into_iter();
                    let oppo_worker_1 = oppo_iter.next().unwrap();
                    let oppo_worker_2 = oppo_iter.next().unwrap();

                    let mut reach_board = BitBoard::EMPTY;
                    if prelude.is_against_hypnus {
                        if prelude.board.get_height(oppo_worker_1) == 2
                            && prelude.board.get_height(oppo_worker_2) == 2
                        {
                            reach_board = prelude.standard_neighbor_map[oppo_worker_1 as usize]
                                | prelude.standard_neighbor_map[oppo_worker_2 as usize];
                        }
                    } else {
                        if prelude.board.get_height(oppo_worker_1) == 2 {
                            reach_board |= prelude.standard_neighbor_map[oppo_worker_1 as usize];
                        }
                        if prelude.board.get_height(oppo_worker_2) == 2 {
                            reach_board |= prelude.standard_neighbor_map[oppo_worker_2 as usize];
                        }
                    }

                    reach_board &= unblocked_squares & prelude.win_mask;

                    let prev_height = prelude.board.get_height(start_pos) + other_worker_height;
                    let oppo_height = prelude.board.get_height(oppo_worker_1)
                        + prelude.board.get_height(oppo_worker_2);
                    let is_improving = oppo_height > prev_height;

                    for build_pos in all_possible_builds {
                        let build_mask = build_pos.to_board();
                        let final_level_3 = (prelude.exactly_level_2 & build_mask)
                            | (prelude.exactly_level_3 & !build_mask);
                        let is_check = (reach_board & final_level_3).is_not_empty();

                        let swap_action = NemesisMove::new_double_swap(
                            start_pos,
                            end_pos,
                            build_pos,
                            oppo_worker_1,
                            oppo_worker_2,
                        );

                        result.push(build_scored_move::<F, _>(
                            swap_action,
                            is_check,
                            is_improving,
                        ));
                    }
                }
                _ => {
                    let starting_height = prelude.board.get_height(start_pos);

                    for oppo_worker_1 in prelude.oppo_workers {
                        let mut reach_board = BitBoard::EMPTY;
                        if prelude.board.get_height(oppo_worker_1) == 2 {
                            reach_board |= prelude.standard_neighbor_map[oppo_worker_1 as usize];
                        }
                        let op_1 = oppo_worker_1;
                        let op_1_height = prelude.board.get_height(op_1);
                        let op_1 = op_1.to_board();

                        for oppo_worker_2 in prelude.oppo_workers
                            & LOWER_SQUARES_EXCLUSIVE_MASK[oppo_worker_1 as usize]
                        {
                            let op_2 = oppo_worker_2;
                            let op_2_height = prelude.board.get_height(op_2);
                            let op_2 = op_2.to_board();

                            let is_improver =
                                op_1_height + op_2_height > starting_height + other_worker_height;

                            let mut reach_board = reach_board;

                            if prelude.board.get_height(oppo_worker_2) == 2 {
                                reach_board |=
                                    prelude.standard_neighbor_map[oppo_worker_2 as usize];
                            }
                            reach_board &= unblocked_squares & prelude.win_mask;

                            let narrowed_builds = if is_interact_with_key_squares::<F>()
                                && ((op_1 | op_2 | end_mask) & key_squares).is_empty()
                            {
                                all_possible_builds & key_squares
                            } else {
                                all_possible_builds
                            };

                            for build_pos in narrowed_builds {
                                let swap_action = NemesisMove::new_double_swap(
                                    start_pos,
                                    end_pos,
                                    build_pos,
                                    oppo_worker_1,
                                    oppo_worker_2,
                                );

                                let build_mask = build_pos.to_board();
                                let final_level_3 = (prelude.exactly_level_2 & build_mask)
                                    | (prelude.exactly_level_3 & !build_mask);
                                let is_check = (reach_board & final_level_3).is_not_empty();
                                result.push(build_scored_move::<F, _>(
                                    swap_action,
                                    is_check,
                                    is_improver,
                                ));
                            }
                        }
                    }
                }
            }
        }
        _ => unreachable!("Invalid number of nemesis workers"),
    }
}

pub(super) fn nemesis_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(nemesis_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, NemesisMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                NemesisMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let mut can_neighbors_use_power = true;
        for n_pos in worker_start_state.other_own_workers {
            if (NEIGHBOR_MAP[n_pos as usize] & prelude.oppo_workers).is_not_empty() {
                can_neighbors_use_power = false;
                break;
            }
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            let can_use_power = can_neighbors_use_power
                && (NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                    & prelude.oppo_workers)
                    .is_empty();

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);

            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;

            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            // No power moves
            let narrowed_builds = if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (worker_end_move_state.worker_end_mask & prelude.key_squares).is_not_empty();
                if is_already_matched {
                    all_possible_builds
                } else {
                    all_possible_builds & key_squares
                }
            } else {
                all_possible_builds
            };

            for worker_build_pos in narrowed_builds {
                let build_mask = worker_build_pos.to_board();
                let new_action = NemesisMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
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

            if can_use_power {
                _add_swap_moves::<F>(
                    &prelude,
                    &mut result,
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_end_move_state.worker_end_mask,
                    worker_start_state.other_own_workers,
                    worker_next_moves.other_threatening_neighbors,
                    all_possible_builds,
                    unblocked_squares,
                    key_squares,
                );
            }
        }
    }

    result
}

pub const fn build_nemesis() -> GodPower {
    god_power(
        GodName::Nemesis,
        build_god_power_movers!(nemesis_move_gen),
        build_god_power_actions::<NemesisMove>(),
        13902774959503976241,
        8706614857531094214,
    )
    .with_nnue_god_name(GodName::Mortal)
}
