use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            WorkerNextMoveState, build_scored_move, get_basic_moves_from_raw_data_for_hermes,
            get_generator_prelude_state, get_sized_result, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves, restrict_moves_by_affinity_area,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

pub const HERMES_MOVE_FROM_POSITION_OFFSET: usize = 0;
pub const HERMES_MOVE_TO_POSITION_OFFSET: usize = HERMES_MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_BUILD_POSITION_OFFSET: usize = HERMES_MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_MOVE2_FROM_POSITION_OFFSET: usize = HERMES_BUILD_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_MOVE2_TO_POSITION_OFFSET: usize =
    HERMES_MOVE2_FROM_POSITION_OFFSET + POSITION_WIDTH;

pub const HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_OFFSET: usize =
    HERMES_MOVE2_TO_POSITION_OFFSET + POSITION_WIDTH;
pub const HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_MASK: MoveData =
    1 << HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_OFFSET;

pub const HERMES_NOT_DOING_SPECIAL_MOVE_VALUE: MoveData = 25 << HERMES_MOVE2_FROM_POSITION_OFFSET;
pub const HERMES_NO_MOVE_MASK: BitBoard = BitBoard::as_mask_u8(0);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct HermesMove(pub MoveData);

impl Into<GenericMove> for HermesMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for HermesMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl HermesMove {
    pub fn new_hermes_no_move(build_position: Square) -> Self {
        let data: MoveData = ((build_position as MoveData) << HERMES_BUILD_POSITION_OFFSET)
            | HERMES_NOT_DOING_SPECIAL_MOVE_VALUE;

        Self(data)
    }

    pub fn new_hermes_single_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HERMES_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HERMES_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << HERMES_BUILD_POSITION_OFFSET)
            | HERMES_NOT_DOING_SPECIAL_MOVE_VALUE;

        Self(data)
    }

    pub fn new_hermes_double_move(
        move_from_position: Square,
        move_to_position: Square,
        move_from2_position: Square,
        move_to2_position: Square,
        build_position: Square,
        is_overlap: bool,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HERMES_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HERMES_MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << HERMES_BUILD_POSITION_OFFSET)
            | ((move_from2_position as MoveData) << HERMES_MOVE2_FROM_POSITION_OFFSET)
            | ((move_to2_position as MoveData) << HERMES_MOVE2_TO_POSITION_OFFSET)
            | (is_overlap as MoveData) << HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_OFFSET;

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << HERMES_MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << HERMES_MOVE_TO_POSITION_OFFSET)
            | HERMES_NOT_DOING_SPECIAL_MOVE_VALUE
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
        Square::from((self.0 >> HERMES_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_from_position2(&self) -> Option<Square> {
        let value = (self.0 >> HERMES_MOVE2_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    // WARNING: only returns usable values when move_from_position2 has returned a value
    pub fn move_to_position2(self) -> Square {
        Square::from((self.0 >> HERMES_MOVE2_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn are_double_moves_overlapping(self) -> bool {
        self.0 & HERMES_ARE_DOUBLE_MOVES_OVERLAPPING_MASK != 0
    }

    pub fn move_mask(self) -> BitBoard {
        if let Some(move2) = self.move_from_position2() {
            BitBoard::as_mask(self.move_from_position())
                ^ BitBoard::as_mask(self.move_to_position())
                ^ BitBoard::as_mask(move2)
                ^ BitBoard::as_mask(self.move_to_position2())
        } else {
            BitBoard::as_mask(self.move_from_position())
                ^ BitBoard::as_mask(self.move_to_position())
        }
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for HermesMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            return write!(f, "{}>{}#", move_from, move_to);
        }

        let mut moves_from_mask = BitBoard::as_mask(move_from);
        let mut moves_to_mask = BitBoard::as_mask(move_to);

        if let Some(move_from_2) = self.move_from_position2() {
            let move_to_2 = self.move_to_position2();
            moves_from_mask |= BitBoard::as_mask(move_from_2);
            moves_to_mask |= BitBoard::as_mask(move_to_2);
        }
        let move_delta = moves_from_mask ^ moves_to_mask;

        let all_squares_from = (moves_from_mask & move_delta).all_squares();
        let all_squares_to = (moves_to_mask & move_delta).all_squares();

        assert_eq!(all_squares_from.len(), all_squares_to.len());
        match all_squares_from.len() {
            0 => write!(f, "^{}", build),
            1 => write!(f, "{}>{}^{}", all_squares_from[0], all_squares_to[0], build),
            2 => {
                write!(
                    f,
                    "({},{})>({},{})^{}",
                    all_squares_from[0],
                    all_squares_from[1],
                    all_squares_to[0],
                    all_squares_to[1],
                    build
                )
            }
            _ => unreachable!(),
        }
    }
}

impl GodMove for HermesMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
            ]];
        }
        let build_position = self.build_position();

        if let Some(from2) = self.move_from_position2() {
            let s = PartialAction::SelectWorker;
            let m = PartialAction::MoveWorker;

            let mut res = vec![];
            let f1 = self.move_from_position();
            let t1 = self.move_to_position();
            let f2 = from2;
            let t2 = self.move_to_position2();
            let build = PartialAction::Build(self.build_position());

            res.push(vec![s(f1), m(t1), s(f2), m(t2), build]);
            res.push(vec![s(f2), m(t2), s(f1), m(t1), build]);
            if f1 == t1 {
                res.push(vec![s(f2), m(t2), build]);
            }
            if f2 == t2 {
                res.push(vec![s(f1), m(t1), build]);
            }
            if f1 == t1 && f2 == t2 {
                res.push(vec![build]);
            }

            if self.are_double_moves_overlapping() {
                res.push(vec![s(f1), m(t2), s(f2), m(t1), build]);
                res.push(vec![s(f2), m(t1), s(f1), m(t2), build]);

                if f1 == t2 {
                    res.push(vec![s(f2), m(t1), build]);
                }

                if f2 == t1 {
                    res.push(vec![s(f1), m(t2), build]);
                }

                if f1 == t2 && f2 == t1 {
                    res.push(vec![build]);
                }
            }

            res
        } else {
            vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position()),
                PartialAction::Build(build_position),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.build_up(build_position);
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let (is_special, from) = if self.move_from_position2().is_some() {
            (1, self.move_to_position2())
        } else {
            (0, self.move_from_position())
        };

        let to = self.move_to_position();
        let build = self.build_position();

        let from_height = board.get_height(from);
        let to_height = board.get_height(to);
        let build_height = board.get_height(build);

        let fu = from as usize;
        let tu = to as usize;
        let bu = build as usize;

        let mut res = 4 * fu + from_height;
        res = res * 100 + 4 * tu + to_height;
        res = res * 100 + 4 * bu + build_height;
        res = res * 2 + is_special as usize;

        res
    }
}

fn flood_fill(walkable_squares: BitBoard, origin: BitBoard) -> BitBoard {
    let mut result = origin;
    let mut queue = origin;

    loop {
        if queue.is_empty() {
            break;
        }

        let square = queue.lsb();
        queue.0 &= queue.0 - 1;

        let new = NEIGHBOR_MAP[square as usize] & walkable_squares & !result;
        queue |= new;
        result |= new;
    }

    result
}

fn hermes_move_gen<const F: MoveGenFlags>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers.into_iter() {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);

        let other_threatening_workers = (worker_start_state.other_own_workers) & checkable_mask;
        let other_threatening_neighbors =
            apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);

        let mut worker_moves = get_basic_moves_from_raw_data_for_hermes(
            &prelude,
            worker_start_pos,
            worker_start_state.worker_start_mask,
            worker_start_state.worker_start_height,
        );

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 = worker_moves & prelude.exactly_level_3 & prelude.win_mask;

            if push_winning_moves::<F, _, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                HermesMove::new_winning_move,
            ) {
                return result;
            }
            worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let unblocked_squares = !(worker_start_state.all_non_moving_workers | prelude.domes);

        for worker_end_pos in worker_moves.into_iter() {
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
                unblocked_squares,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = HermesMove::new_hermes_single_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );

                let is_check = {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_3 = (prelude.exactly_level_2 & worker_build_mask)
                        | (prelude.exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3 & unblocked_squares;
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

    if is_mate_only::<F>() {
        return result;
    }

    let mut worker_iter = prelude.acting_workers;
    let f1 = worker_iter.next().unwrap();
    let m1 = BitBoard::as_mask(f1);
    let h1 = prelude.board.get_height(f1);
    let h1_mask = prelude.board.exactly_level_n(h1) & !prelude.oppo_workers;
    let mut c1 = flood_fill(h1_mask, m1);

    let Some(f2) = worker_iter.next() else {
        c1 = restrict_moves_by_affinity_area(m1, c1, prelude.affinity_area);
        // There's only 1 hermes worker
        let non_selected_workers = prelude.all_workers_mask ^ m1;
        let buildable_squares = !(non_selected_workers | prelude.domes);

        for t1 in c1 {
            let worker_end_mask = BitBoard::as_mask(t1);

            let all_possible_builds = NEIGHBOR_MAP[t1 as usize] & buildable_squares;
            let mut narrowed_builds = all_possible_builds & prelude.build_mask;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (worker_end_mask & prelude.key_squares).is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            let worker_plausible_next_moves = NEIGHBOR_MAP[t1 as usize];
            let reach_board = (worker_plausible_next_moves
                & BitBoard::CONDITIONAL_MASK[(h1 == 2) as usize])
                & prelude.win_mask;

            for worker_build_pos in narrowed_builds {
                let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                let new_action = HermesMove::new_hermes_single_move(f1, t1, worker_build_pos);

                let is_check = {
                    let check_board = (prelude.exactly_level_2 & worker_build_mask
                        | prelude.exactly_level_3 & !worker_build_mask)
                        & reach_board
                        & buildable_squares;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(new_action, is_check, false))
            }
        }

        return result;
    };

    // There's 2 hermes workers
    let m2 = BitBoard::as_mask(f2);

    let mut c2;
    let h2;
    let is_overlap;
    if (c1 & m2).is_not_empty() {
        is_overlap = true;
        c2 = restrict_moves_by_affinity_area(m2, c1, prelude.affinity_area);
        c1 = restrict_moves_by_affinity_area(m1, c1, prelude.affinity_area);
        h2 = h1;
    } else {
        is_overlap = false;
        h2 = prelude.board.get_height(f2);
        let h2_mask = prelude.board.exactly_level_n(h2) & !prelude.oppo_workers;

        c1 = restrict_moves_by_affinity_area(m1, c1, prelude.affinity_area);
        c2 = restrict_moves_by_affinity_area(m2, flood_fill(h2_mask, m2), prelude.affinity_area);
    }

    let blocked_squares = prelude.oppo_workers | prelude.domes;

    let l1 = BitBoard::CONDITIONAL_MASK[(h1 == 2) as usize];
    let l2 = BitBoard::CONDITIONAL_MASK[(h2 == 2) as usize];

    for t1 in c1 {
        let t1_mask = BitBoard::as_mask(t1);
        c2 &= !t1_mask;

        let t1_ns = NEIGHBOR_MAP[t1 as usize];
        let from_level_2_1 = t1_ns & l1;

        for t2 in c2 {
            let t2_mask = BitBoard::as_mask(t2);
            let both_mask = t1_mask | t2_mask;

            let t2_ns = NEIGHBOR_MAP[t2 as usize];
            let mut possible_builds = (t1_ns | t2_ns) & !(blocked_squares | both_mask);
            let worker_plausible_next_moves = possible_builds & prelude.win_mask;
            possible_builds &= prelude.build_mask;

            if is_interact_with_key_squares::<F>() {
                if (both_mask & key_squares).is_empty() {
                    possible_builds &= key_squares;
                }
            }

            let from_level_2_2 = t2_ns & l2;
            let l2_neighbors = from_level_2_1 | from_level_2_2;

            for build in possible_builds {
                let new_action =
                    HermesMove::new_hermes_double_move(f1, t1, f2, t2, build, is_overlap);
                let worker_build_mask = BitBoard::as_mask(build);

                // dont need hypnus check here. if you can move both workers, it's because they are
                // both on the same level
                let is_check = {
                    let check_board = l2_neighbors
                        & worker_plausible_next_moves
                        & (prelude.exactly_level_3 & !worker_build_mask
                            | prelude.exactly_level_2 & worker_build_mask);
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(new_action, is_check, false))
            }
        }
    }

    result
}

pub const fn build_hermes() -> GodPower {
    god_power(
        GodName::Hermes,
        build_god_power_movers!(hermes_move_gen),
        build_god_power_actions::<HermesMove>(),
        8064494721607657900,
        8099092864803375172,
    )
}
