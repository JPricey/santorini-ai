use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    build_god_power,
    gods::{
        FullAction, GodName, GodPower,
        generic::{
            GenericMove, GodMove, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, LOWER_POSITION_MASK,
            MATE_ONLY, MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, NULL_MOVE_DATA,
            POSITION_WIDTH, STOP_ON_MATE, ScoredMove,
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

    pub fn new_hermes_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
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

    fn unmake_move(self, board: &mut BoardState) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(board.current_player, worker_move_mask);

        if self.get_is_winning() {
            board.unset_winner(board.current_player);
            return;
        }

        let build_position = self.build_position();
        board.unbuild(build_position);
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
    board: &BoardState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let current_player_idx = player as usize;
    let mut current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let other_workers = board.workers[1 - current_player_idx] & BitBoard::MAIN_SECTION_MASK;

    let exactly_level_2 = board.exactly_level_2();
    let exactly_level_3 = board.exactly_level_3();

    if F & MATE_ONLY != 0 {
        current_workers &= exactly_level_2
    }
    let capacity = if F & MATE_ONLY != 0 { 1 } else { 128 };

    let mut result: Vec<ScoredMove> = Vec::with_capacity(capacity);
    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height(moving_worker_start_pos);

        let mut neighbor_neighbor = BitBoard::EMPTY;
        if F & INCLUDE_SCORE != 0 {
            let other_checkable_workers =
                (current_workers ^ moving_worker_start_mask) & exactly_level_2;
            for other_pos in other_checkable_workers {
                neighbor_neighbor |= NEIGHBOR_MAP[other_pos as usize];
            }
        }

        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[board.get_worker_climb_height(player, worker_starting_height)]
                | all_workers_mask);
        worker_moves &= !board.exactly_level_n(worker_starting_height);

        if F & MATE_ONLY != 0 || worker_starting_height == 2 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = ScoredMove::new_winning_move(
                    HermesMove::new_hermes_winning_move(
                        moving_worker_start_pos,
                        moving_worker_end_pos,
                    )
                    .into(),
                );
                result.push(winning_move);
                if F & STOP_ON_MATE != 0 {
                    return result;
                }
            }
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let moving_worker_end_mask = BitBoard::as_mask(moving_worker_end_pos);
            let worker_end_height = board.get_height(moving_worker_end_pos);

            let mut worker_builds =
                NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;
            let worker_plausible_next_moves = worker_builds;

            if (F & INTERACT_WITH_KEY_SQUARES) != 0 {
                if (moving_worker_end_mask & key_squares).is_empty() {
                    worker_builds = worker_builds & key_squares;
                }
            }

            let reach_board = neighbor_neighbor
                | (worker_plausible_next_moves
                    & BitBoard::CONDITIONAL_MASK[(worker_end_height == 2) as usize]);

            for worker_build_pos in worker_builds {
                let new_action = HermesMove::new_hermes_single_move(
                    moving_worker_start_pos,
                    moving_worker_end_pos,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let worker_build_mask = BitBoard::as_mask(worker_build_pos);
                    let final_level_3 = (exactly_level_2 & worker_build_mask)
                        | (exactly_level_3 & !worker_build_mask);
                    let check_board = reach_board & final_level_3 & buildable_squares;
                    let is_check = check_board.is_not_empty();
                    if is_check {
                        result.push(ScoredMove::new_checking_move(new_action.into()));
                    } else {
                        let is_improving = worker_end_height > worker_starting_height;
                        if is_improving {
                            result.push(ScoredMove::new_improving_move(new_action.into()));
                        } else {
                            result.push(ScoredMove::new_non_improver(new_action.into()));
                        };
                    }
                } else {
                    result.push(ScoredMove::new_unscored_move(new_action.into()));
                }
            }
        }
    }

    if F & MATE_ONLY != 0 {
        return result;
    }

    let mut worker_iter = current_workers;
    let f1 = worker_iter.next().unwrap();
    let m1 = BitBoard::as_mask(f1);
    let h1 = board.get_height(f1);
    let h1_mask = board.exactly_level_n(h1) & !other_workers;

    let f2 = worker_iter.next().unwrap();
    let m2 = BitBoard::as_mask(f2);

    let c1 = flood_fill(h1_mask, m1);
    let mut c2;
    let h2;
    let is_overlap;
    if (c1 & m2).is_not_empty() {
        is_overlap = true;
        c2 = c1;
        h2 = h1;
    } else {
        is_overlap = false;
        h2 = board.get_height(f2);
        let h2_mask = board.exactly_level_n(h2) & !other_workers;

        c2 = flood_fill(h2_mask, m2);
    }

    let blocked_squares = other_workers | board.height_map[3];

    let l1 = BitBoard::CONDITIONAL_MASK[(h1 == 2) as usize];
    let l2 = BitBoard::CONDITIONAL_MASK[(h2 == 2) as usize];

    for t1 in c1 {
        let t1_mask = BitBoard::as_mask(t1);
        c2 ^= c2 & t1_mask;

        let from_level_2_1 = NEIGHBOR_MAP[t1 as usize] & l1;

        for t2 in c2 {
            let t2_mask = BitBoard::as_mask(t2);
            let both_mask = t1_mask | t2_mask;

            let mut possible_builds = (NEIGHBOR_MAP[t1 as usize] | NEIGHBOR_MAP[t2 as usize])
                & !(blocked_squares | both_mask);
            let worker_plausible_next_moves = possible_builds;

            if F & INTERACT_WITH_KEY_SQUARES != 0 {
                if (both_mask & key_squares).is_empty() {
                    possible_builds &= key_squares;
                }
            }

            let from_level_2_2 = NEIGHBOR_MAP[t2 as usize] & l2;
            let l2_neighbors = from_level_2_1 | from_level_2_2;

            for build in possible_builds {
                let new_action =
                    HermesMove::new_hermes_double_move(f1, t1, f2, t2, build, is_overlap);
                let build_mask = BitBoard::as_mask(build);

                if F & INCLUDE_SCORE != 0 {
                    let is_check = l2_neighbors
                        & worker_plausible_next_moves
                        & (exactly_level_3 & !build_mask | exactly_level_2 & build_mask);
                    if is_check.is_not_empty() {
                        result.push(ScoredMove::new_checking_move(new_action.into()));
                    } else {
                        result.push(ScoredMove::new_non_improver(new_action.into()));
                    }
                } else {
                    result.push(ScoredMove::new_unscored_move(new_action.into()));
                }
            }
        }
    }

    result
}

build_god_power!(
    build_hermes,
    god_name: GodName::Hermes,
    move_type: HermesMove,
    move_gen: hermes_move_gen,
    hash1: 8064494721607657900,
    hash2: 8099092864803375172,
);
