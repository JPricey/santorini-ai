use arrayvec::ArrayVec;

use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PUSH_MAPPING},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::basic_slide_from_unblocked,
        move_helpers::{
            GeneratorPreludeState, build_scored_move,
            get_basic_moves_from_raw_data_with_custom_blockers, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, is_stop_on_mate, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET_1: usize = 0;
const MOVE_TO_POSITION_OFFSET_1: usize = MOVE_FROM_POSITION_OFFSET_1 + POSITION_WIDTH;

const MOVE_FROM_POSITION_OFFSET_2: usize = MOVE_TO_POSITION_OFFSET_1 + POSITION_WIDTH;
const MOVE_TO_POSITION_OFFSET_2: usize = MOVE_FROM_POSITION_OFFSET_2 + POSITION_WIDTH;

const BUILD_POSITION_1: usize = MOVE_TO_POSITION_OFFSET_2 + POSITION_WIDTH;
const BUILD_POSITION_2: usize = BUILD_POSITION_1 + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct CastorMove(pub MoveData);

impl GodMove for CastorMove {
    fn move_to_actions(self, _board: &BoardState) -> Vec<FullAction> {
        if let Some(from1) = self.maybe_move_from_position_1() {
            let to1 = self.move_to_position_1();
            let mut res = vec![
                PartialAction::SelectWorker(from1),
                PartialAction::MoveWorker(to1.into()),
            ];

            if let Some(from2) = self.maybe_move_from_position_2() {
                let to2 = self.move_to_position_2();

                if to1 == from2 {
                    return vec![vec![
                        PartialAction::SelectWorker(from2),
                        PartialAction::MoveWorker(to2.into()),
                        PartialAction::SelectWorker(from1),
                        PartialAction::MoveWorker(to1.into()),
                    ]];
                } else if to2 == from1 {
                    return vec![vec![
                        PartialAction::SelectWorker(from1),
                        PartialAction::MoveWorker(to1.into()),
                        PartialAction::SelectWorker(from2),
                        PartialAction::MoveWorker(to2.into()),
                    ]];
                } else {
                    return vec![
                        vec![
                            PartialAction::SelectWorker(from1),
                            PartialAction::MoveWorker(to1.into()),
                            PartialAction::SelectWorker(from2),
                            PartialAction::MoveWorker(to2.into()),
                        ],
                        vec![
                            PartialAction::SelectWorker(from2),
                            PartialAction::MoveWorker(to2.into()),
                            PartialAction::SelectWorker(from1),
                            PartialAction::MoveWorker(to1.into()),
                        ],
                    ];
                }
            } else if let Some(build) = self.maybe_build_position_1() {
                res.push(PartialAction::Build(build));
                return vec![res];
            } else {
                return vec![res];
            }
        } else {
            // Double build
            let b1 = self.definite_build_position_1();

            if let Some(build2) = self.maybe_build_position_2() {
                return vec![
                    vec![PartialAction::Build(b1), PartialAction::Build(build2)],
                    vec![PartialAction::Build(build2), PartialAction::Build(b1)],
                ];
            } else {
                return vec![vec![PartialAction::Build(b1)]];
            }
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        if let Some(move_from_1) = self.maybe_move_from_position_1() {
            let mut move_mask =
                BitBoard::as_mask(move_from_1) ^ BitBoard::as_mask(self.move_to_position_1());

            if let Some(from2) = self.maybe_move_from_position_2() {
                move_mask ^=
                    BitBoard::as_mask(from2) ^ BitBoard::as_mask(self.move_to_position_2());
                board.worker_xor(player, move_mask);

                if board.workers[player as usize].is_empty() {
                    eprintln!("move made board empty: {:?}", self);
                }

                if self.get_is_winning() {
                    board.set_winner(player);
                }
            } else {
                board.worker_xor(player, move_mask);

                if self.get_is_winning() {
                    board.set_winner(player);
                } else if let Some(build) = self.maybe_build_position_1() {
                    board.build_up(build);
                }
            }
        } else {
            board.build_up(self.definite_build_position_1());
            if let Some(build2) = self.maybe_build_position_2() {
                board.build_up(build2);
            }
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        if let Some(mf_2) = self.maybe_move_from_position_2() {
            BitBoard::as_mask(self.definite_move_from_position_1())
                | BitBoard::as_mask(self.move_to_position_1())
                | BitBoard::as_mask(mf_2)
                | BitBoard::as_mask(self.move_to_position_2())
        } else {
            BitBoard::as_mask(self.definite_move_from_position_1())
                | BitBoard::as_mask(self.move_to_position_1())
        }
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        if let Some(move1) = self.maybe_move_from_position_1() {
            helper.add_square_with_height(board, move1);
            helper.add_square_with_height(board, self.move_to_position_1());

            if let Some(move2) = self.maybe_move_from_position_2() {
                helper.add_square_with_height(board, move2);
                helper.add_square_with_height(board, self.move_to_position_2());
            } else if let Some(build) = self.maybe_build_position_1() {
                helper.add_square_with_height(board, build);
            }
        } else {
            helper.add_value(1, 2);
            helper.add_square_with_height(board, self.definite_build_position_1());

            if let Some(build2) = self.maybe_build_position_2() {
                helper.add_square_with_height(board, build2);
            }
        }

        helper.get()
    }
}

impl Into<GenericMove> for CastorMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for CastorMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl CastorMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((build_position as MoveData) << BUILD_POSITION_1);

        Self(data)
    }

    pub fn new_winning_single_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((25 as MoveData) << BUILD_POSITION_1)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub fn new_double_move(from1: Square, to1: Square, from2: Square, to2: Square) -> Self {
        debug_assert_ne!(to1, to2);

        let data: MoveData = ((from1 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to1 as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((from2 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((to2 as MoveData) << MOVE_TO_POSITION_OFFSET_2)
            | ((25 as MoveData) << BUILD_POSITION_1);

        Self(data)
    }

    pub fn new_winning_double_move(from1: Square, to1: Square, from2: Square, to2: Square) -> Self {
        let data: MoveData = ((from1 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((to1 as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((from2 as MoveData) << MOVE_FROM_POSITION_OFFSET_2)
            | ((to2 as MoveData) << MOVE_TO_POSITION_OFFSET_2)
            | ((25 as MoveData) << BUILD_POSITION_1)
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    pub fn new_double_build(build_1: Square, build_2: Square) -> Self {
        let data: MoveData = ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((build_1 as MoveData) << BUILD_POSITION_1)
            | ((build_2 as MoveData) << BUILD_POSITION_2);
        Self(data)
    }

    pub fn new_single_build(build_1: Square) -> Self {
        let data: MoveData = ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((build_1 as MoveData) << BUILD_POSITION_1)
            | ((25 as MoveData) << BUILD_POSITION_2);
        Self(data)
    }

    pub fn new_single_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET_1)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET_1)
            | ((25 as MoveData) << BUILD_POSITION_1)
            | ((25 as MoveData) << MOVE_FROM_POSITION_OFFSET_2);
        Self(data)
    }

    pub fn maybe_move_from_position_1(&self) -> Option<Square> {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn definite_move_from_position_1(&self) -> Square {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK;
        Square::from(value)
    }

    // Only call when we know we're doing this kind of move
    pub fn move_to_position_1(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET_1) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_move_from_position_2(&self) -> Option<Square> {
        let value = (self.0 >> MOVE_FROM_POSITION_OFFSET_2) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    // Only call when we know we're doing this kind of move
    pub fn move_to_position_2(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET_2) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_build_position_1(&self) -> Option<Square> {
        let value = (self.0 >> BUILD_POSITION_1) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn definite_build_position_1(&self) -> Square {
        let value = (self.0 >> BUILD_POSITION_1) as u8 & LOWER_POSITION_MASK;
        Square::from(value)
    }

    pub fn maybe_build_position_2(&self) -> Option<Square> {
        let value = (self.0 >> BUILD_POSITION_2) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for CastorMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        if let Some(move1) = self.maybe_move_from_position_1() {
            let mut res = format!("{}>{}", move1, self.move_to_position_1());

            if let Some(move2) = self.maybe_move_from_position_2() {
                res += &format!(" {}>{}", move2, self.move_to_position_2());
            } else if let Some(build) = self.maybe_build_position_1() {
                res += &format!(" ^{}", build);
            }

            if self.get_is_winning() {
                res += "#";
            }

            write!(f, "{}", res)
        } else {
            if let Some(build2) = self.maybe_build_position_2() {
                write!(f, "^{} ^{}", self.definite_build_position_1(), build2)
            } else {
                write!(f, "^{}", self.definite_build_position_1())
            }
        }
    }
}

fn handle_persephone_double_moves_for_move_masks<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    non_own_worker_blockers: BitBoard,
    worker_start_1: Square,
    worker_start_2: Square,
    start_mask_1: BitBoard,
    w1_moves: BitBoard,
    w2_moves: BitBoard,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) {
    for to1 in w1_moves {
        let end_mask_1 = BitBoard::as_mask(to1);

        let mut final_moves_2 = w2_moves & !end_mask_1;
        if to1 == worker_start_2 {
            final_moves_2 &= !start_mask_1;
        }

        if is_interact_with_key_squares::<F>() && (key_squares & end_mask_1).is_empty() {
            final_moves_2 &= key_squares;
        }

        let end_height_1 = prelude.board.get_height(to1);
        let reach1 = if end_height_1 == 2 {
            NEIGHBOR_MAP[to1 as usize]
        } else {
            BitBoard::EMPTY
        };

        for to2 in final_moves_2 {
            let end_height_2 = prelude.board.get_height(to2);
            let end_mask_2 = BitBoard::as_mask(to2);
            let end_masks = end_mask_1 | end_mask_2;

            let new_action = CastorMove::new_double_move(worker_start_1, to1, worker_start_2, to2);

            let reach2 = if end_height_2 == 2 {
                NEIGHBOR_MAP[to2 as usize]
            } else {
                BitBoard::EMPTY
            };

            let is_check = {
                let check_board = (reach1 | reach2)
                    & !(non_own_worker_blockers | end_masks)
                    & prelude.exactly_level_3;
                check_board.is_not_empty()
            };

            result.push(build_scored_move::<F, _>(new_action, is_check, true));
        }
    }
}

fn handle_must_climb_double_moves<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) {
    // don't bother calculating winning moves, since any double winning move vs persephone will be possible with
    // a single move anyway
    if is_mate_only::<F>() {
        return;
    }

    let mut own_workers = prelude.own_workers.into_iter();
    let non_own_worker_blockers = prelude.domes_and_frozen | prelude.oppo_workers;

    let worker_start_1 = own_workers
        .next()
        .expect("Castor must have 2 workers vs persephone");
    let worker_start_2 = own_workers
        .next()
        .expect("Castor must have 2 workers vs persephone");

    let start_height_1 = prelude.board.get_height(worker_start_1);
    let start_height_2 = prelude.board.get_height(worker_start_2);

    let start_mask_1 = BitBoard::as_mask(worker_start_1);

    let max_height_1 = 2.min(start_height_1 + 1);
    let max_height_2 = 2.min(start_height_2 + 1);

    let all_moves_1 = NEIGHBOR_MAP[worker_start_1 as usize]
        & !(non_own_worker_blockers | prelude.board.height_map[max_height_1]);
    let all_moves_2 = NEIGHBOR_MAP[worker_start_2 as usize]
        & !(non_own_worker_blockers | prelude.board.height_map[max_height_2]);

    let improvers_1 = all_moves_1
        & match start_height_1 {
            0 => prelude.exactly_level_1,
            1 => prelude.exactly_level_2,
            2 => prelude.exactly_level_3,
            _ => unreachable!(
                "Castor should only have workers on level 0-2 vs persephone: {} {:?}",
                start_height_1,
                FullGameState::new(
                    prelude.board.clone(),
                    [GodName::Mortal.to_power(), GodName::Mortal.to_power()]
                )
            ),
        };

    let improvers_2 = all_moves_2
        & match start_height_2 {
            0 => prelude.exactly_level_1,
            1 => prelude.exactly_level_2,
            2 => prelude.exactly_level_3,
            _ => unreachable!(
                "Castor should only have workers on level 0-2 vs persephone: {} {:?}",
                start_height_2,
                FullGameState::new(
                    prelude.board.clone(),
                    [GodName::Mortal.to_power(), GodName::Mortal.to_power()]
                )
            ),
        };

    handle_persephone_double_moves_for_move_masks::<F>(
        prelude,
        non_own_worker_blockers,
        worker_start_1,
        worker_start_2,
        start_mask_1,
        improvers_1,
        all_moves_2,
        key_squares,
        result,
    );

    handle_persephone_double_moves_for_move_masks::<F>(
        prelude,
        non_own_worker_blockers,
        worker_start_1,
        worker_start_2,
        start_mask_1,
        all_moves_1 ^ improvers_1,
        improvers_2,
        key_squares,
        result,
    );
}

fn _harpy_slide_with_coverage(
    prelude: &GeneratorPreludeState,
    unblocked_squares: BitBoard,
    mut from: Square,
    mut to: Square,
) -> HarpyWorkerSlideState {
    let dir = to;
    let mut current_height = prelude.board.get_height(to);
    let mut cover = from.to_board() | to.to_board();

    loop {
        let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
            break;
        };

        let next_mask = next_spot.to_board();
        if (unblocked_squares & next_mask).is_empty() {
            break;
        }

        let new_height = prelude.board.get_height(next_spot);
        if new_height > current_height {
            // Can't climb
            break;
        }

        cover |= next_mask;

        from = to;
        to = next_spot;
        current_height = new_height;
    }

    HarpyWorkerSlideState {
        dir,
        dest: to,
        cover,
        height: current_height,
    }
}

// Returns w2's final position, and if it collided with w1's worker
fn _harpy_w2_slide(
    prelude: &GeneratorPreludeState,
    unblocked_non_own_workers: BitBoard,
    worker_start_2: Square,
    to: Square,
    w1_pos: Square,
) -> (Square, usize, bool) {
    let mut current_from = worker_start_2;
    let mut current_pos = to;
    let mut current_height = prelude.board.get_height(current_pos);

    loop {
        let Some(next_spot) = PUSH_MAPPING[current_from as usize][current_pos as usize] else {
            return (current_pos, current_height, false);
        };

        let next_mask = next_spot.to_board();
        if (unblocked_non_own_workers & next_mask).is_empty() {
            // Hit wall
            return (current_pos, current_height, false);
        }

        let new_height = prelude.board.get_height(next_spot);
        if new_height > current_height {
            // Can't climb
            return (current_pos, current_height, false);
        }

        if next_spot == w1_pos {
            return (current_pos, current_height, true);
        }

        current_from = current_pos;
        current_pos = next_spot;
        current_height = new_height;
    }
}

#[derive(Copy, Clone)]
struct HarpyWorkerSlideState {
    dir: Square,
    dest: Square,
    height: usize,
    cover: BitBoard,
}

fn _push_harpy_double_move<const F: MoveGenFlags>(
    w1_start: Square,
    w2_start: Square,
    w1_dest: Square,
    w2_dest: Square,
    w1_start_height: usize,
    w2_start_height: usize,
    w1_end_height: usize,
    w2_end_height: usize,
    checkable_squares: BitBoard,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) {
    let new_action = CastorMove::new_double_move(w1_start, w1_dest, w2_start, w2_dest);

    if is_interact_with_key_squares::<F>() {
        if ((w1_dest.to_board() | w2_dest.to_board()) & key_squares).is_empty() {
            return;
        }
    }

    let is_check = {
        let is_w1_check = if w1_end_height == 2 {
            (NEIGHBOR_MAP[w1_dest as usize] & checkable_squares).is_not_empty()
        } else {
            false
        };

        is_w1_check
            || if w2_end_height == 2 {
                (NEIGHBOR_MAP[w2_dest as usize] & checkable_squares).is_not_empty()
            } else {
                false
            }
    };

    let improver = (w1_end_height > w1_start_height) || (w2_end_height > w2_start_height);

    result.push(build_scored_move::<F, _>(new_action, is_check, improver));
}

fn _handle_harpy_move_did_force_w1_move_first<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    unblocked_non_own_workers: BitBoard,
    worker_start_1: Square,
    worker_start_2: Square,
    w1_mask: BitBoard,
    w2_mask: BitBoard,
    start_height_1: usize,
    start_height_2: usize,
    w1: &HarpyWorkerSlideState,
    w2: &HarpyWorkerSlideState,
    checkable_squares: BitBoard,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) -> bool {
    // w1 is forced to move first when:
    // - w1 start is in w2s way, and we want w1 to move out of the way first
    // - w1 end is in w2s way, and we want to see how that collision works
    // - w2 start is in w1s way

    if (w1.cover & w2_mask).is_not_empty() {
        // W2 starts in W1's way
        if w1.dir == worker_start_2 {
            // can't move w1 first at all, skip, but record that there was an interaction
            return true;
        }

        let (w1_dest, w1_dest_height) = basic_slide_from_unblocked(
            prelude,
            unblocked_non_own_workers & !w2_mask,
            worker_start_1,
            w1.dir,
        );

        // W1 moved and now blocks w2. Skip, but record that there was an interaction
        if w1_dest == w2.dir {
            return true;
        }

        // W2 moves as if nothing was in the way now
        _push_harpy_double_move::<F>(
            worker_start_1,
            worker_start_2,
            w1_dest,
            w2.dest,
            start_height_1,
            start_height_2,
            w1_dest_height,
            w2.height,
            checkable_squares,
            key_squares,
            result,
        );
        return true;
    }

    let w1_end_in_the_way = (w2.cover & w1.dest.to_board()).is_not_empty();
    if w1_end_in_the_way {
        // w1 blocks w2 entirely
        if w1.dest == w2.dir {
            return true;
        }

        let (w2_dest, w2_dest_height) = basic_slide_from_unblocked(
            prelude,
            unblocked_non_own_workers & !w1.dest.to_board(),
            worker_start_2,
            w2.dir,
        );

        _push_harpy_double_move::<F>(
            worker_start_1,
            worker_start_2,
            w1.dest,
            w2_dest,
            start_height_1,
            start_height_2,
            w1.height,
            w2_dest_height,
            checkable_squares,
            key_squares,
            result,
        );
        return true;
    }

    let w1_start_in_the_way = (w2.cover & w1_mask).is_not_empty();
    if w1_start_in_the_way {
        // W1 moves out of w2s way to start
        // But we know that w1 doesn't end in w2s way

        _push_harpy_double_move::<F>(
            worker_start_1,
            worker_start_2,
            w1.dest,
            w2.dest,
            start_height_1,
            start_height_2,
            w1.height,
            w2.height,
            checkable_squares,
            key_squares,
            result,
        );
    }

    false
}

fn handle_harpies_double_moves<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    unblocked_non_own_workers: BitBoard,
    key_squares: BitBoard,
    result: &mut Vec<ScoredMove>,
) {
    // don't bother calculating winning moves, since any double winning move vs harpies will be possible with
    // a single move anyway
    if is_mate_only::<F>() {
        return;
    }

    let mut own_workers = prelude.own_workers.into_iter();
    let worker_start_1 = own_workers
        .next()
        .expect("Castor must have 2 workers vs harpies");
    let worker_start_2 = own_workers
        .next()
        .expect("Castor must have 2 workers vs harpies");
    let w1_mask = worker_start_1.to_board();
    let w2_mask = worker_start_2.to_board();

    let start_height_1 = prelude.board.get_height(worker_start_1);
    let start_height_2 = prelude.board.get_height(worker_start_2);
    let max_height_1 = 2.min(start_height_1 + 1);
    let max_height_2 = 2.min(start_height_2 + 1);

    let all_moves_1 = NEIGHBOR_MAP[worker_start_1 as usize]
        & unblocked_non_own_workers
        & !(prelude.board.height_map[max_height_1]);
    let all_moves_2 = NEIGHBOR_MAP[worker_start_2 as usize]
        & unblocked_non_own_workers
        & !(prelude.board.height_map[max_height_2]);

    let mut w1_moves: ArrayVec<HarpyWorkerSlideState, 8> = Default::default();
    for dir in all_moves_1 {
        w1_moves.push(_harpy_slide_with_coverage(
            prelude,
            unblocked_non_own_workers,
            worker_start_1,
            dir,
        ));
    }

    let checkable_squares = prelude.exactly_level_3 & unblocked_non_own_workers;

    for dir2 in all_moves_2 {
        let w2_move =
            _harpy_slide_with_coverage(prelude, unblocked_non_own_workers, worker_start_2, dir2);

        // Let's try to break down the cases....
        // 1. w1 goes first then w2
        //  - maybe because w1 hits w2 before it moves (w2 start in w1 coverage)
        //  - maybe because w2 hits w1 after it moves (w1 end in w2 coverage)
        //  - but never both
        // 2. w2 goes first then w1
        //  - maybe because w2 hits w1 before it moves (w1 start in w2 coverage)
        //  - maybe because w1 hits w2 after it moves (w2 end in w1 coverage)
        //  - but never both
        // 3. if we haven't emit a move yet, it's because they are independant, so we can safely
        //    emit them both
        // 2/3 are weird because maybe you want to go first to "get there" first, or maybe you want
        // to go first to collide with the other worker first
        //
        // recalculate first if coverage line hits other worker start
        // recalculate second if coverage line hits other worker dest
        // force order when dir == other worker start or dest

        for w1_move in &w1_moves {
            let did_move_w1_first = _handle_harpy_move_did_force_w1_move_first::<F>(
                prelude,
                unblocked_non_own_workers,
                worker_start_1,
                worker_start_2,
                w1_mask,
                w2_mask,
                start_height_1,
                start_height_2,
                w1_move,
                &w2_move,
                checkable_squares,
                key_squares,
                result,
            );

            let did_move_w2_first = _handle_harpy_move_did_force_w1_move_first::<F>(
                prelude,
                unblocked_non_own_workers,
                worker_start_2,
                worker_start_1,
                w2_mask,
                w1_mask,
                start_height_2,
                start_height_1,
                &w2_move,
                w1_move,
                checkable_squares,
                key_squares,
                result,
            );

            if !did_move_w1_first && !did_move_w2_first {
                // independent moves
                _push_harpy_double_move::<F>(
                    worker_start_1,
                    worker_start_2,
                    w1_move.dest,
                    w2_move.dest,
                    start_height_1,
                    start_height_2,
                    w1_move.height,
                    w2_move.height,
                    checkable_squares,
                    key_squares,
                    result,
                );
            }
        }
    }
}

pub(super) fn castor_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(castor_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);
    let non_own_worker_blockers = prelude.domes_and_frozen | prelude.oppo_workers;
    let unblocked_non_own_workers = !non_own_worker_blockers;
    let check_mask = unblocked_non_own_workers & prelude.exactly_level_3 & prelude.win_mask;

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, CastorMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                CastorMove::new_winning_single_move,
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
                unblocked_non_own_workers,
            );

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = CastorMove::new_basic_move(
                    worker_start_pos,
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
                ))
            }
        }
    }

    // Double moves
    if MUST_CLIMB {
        handle_must_climb_double_moves::<F>(&prelude, key_squares, &mut result);
    } else if prelude.is_against_harpies {
        handle_harpies_double_moves::<F>(
            &prelude,
            unblocked_non_own_workers,
            key_squares,
            &mut result,
        );
    } else {
        let mut own_workers = prelude.own_workers.into_iter();
        let Some(worker_start_1) = own_workers.next() else {
            return result;
        };

        if prelude.is_against_hypnus
            && (prelude.own_workers & prelude.exactly_level_2) != prelude.own_workers
        {
            // Stuck vs hypnus
        } else {
            let start_height_1 = prelude.board.get_height(worker_start_1);
            let start_mask_1 = BitBoard::as_mask(worker_start_1);
            let mut moves_1 = get_basic_moves_from_raw_data_with_custom_blockers::<false>(
                &prelude,
                worker_start_1,
                start_mask_1,
                start_height_1,
                non_own_worker_blockers,
            );

            let wins_1 = if start_height_1 == 2 {
                moves_1 & prelude.exactly_level_3 & prelude.win_mask
            } else {
                BitBoard::EMPTY
            };
            moves_1 ^= wins_1;

            if let Some(worker_start_2) = own_workers.next() {
                let start_height_2 = prelude.board.get_height(worker_start_2);
                let start_mask_2 = BitBoard::as_mask(worker_start_2);

                let mut moves_2 = get_basic_moves_from_raw_data_with_custom_blockers::<false>(
                    &prelude,
                    worker_start_2,
                    start_mask_2,
                    start_height_2,
                    non_own_worker_blockers,
                );

                let wins_2 = if start_height_2 == 2 {
                    moves_2 & prelude.exactly_level_3 & prelude.win_mask
                } else {
                    BitBoard::EMPTY
                };
                moves_2 ^= wins_2;

                if (wins_2 & start_mask_1).is_not_empty() {
                    for to1 in moves_1 & !start_mask_2 {
                        let new_action = CastorMove::new_winning_double_move(
                            worker_start_1,
                            to1,
                            worker_start_2,
                            worker_start_1,
                        );
                        result.push(ScoredMove::new_winning_move(new_action.into()));
                        if is_stop_on_mate::<F>() {
                            return result;
                        }
                    }
                }

                if (wins_1 & start_mask_2).is_not_empty() {
                    for to2 in moves_2 & !start_mask_1 {
                        let new_action = CastorMove::new_winning_double_move(
                            worker_start_2,
                            to2,
                            worker_start_1,
                            worker_start_2,
                        );
                        result.push(ScoredMove::new_winning_move(new_action.into()));
                        if is_stop_on_mate::<F>() {
                            return result;
                        }
                    }
                }

                if is_mate_only::<F>() {
                    // NOOP - can't mate here anymore
                } else {
                    for to1 in moves_1 {
                        let end_mask_1 = BitBoard::as_mask(to1);
                        let end_height_1 = prelude.board.get_height(to1);
                        let reach1 = if end_height_1 == 2 {
                            prelude.standard_neighbor_map[to1 as usize]
                        } else {
                            BitBoard::EMPTY
                        };

                        let mut final_moves_2 = moves_2 & !end_mask_1;
                        if to1 == worker_start_2 {
                            final_moves_2 &= !start_mask_1;
                        }

                        if is_interact_with_key_squares::<F>()
                            && (key_squares & end_mask_1).is_empty()
                        {
                            final_moves_2 &= key_squares;
                        }

                        for to2 in final_moves_2 {
                            let end_height_2 = prelude.board.get_height(to2);

                            let new_action = CastorMove::new_double_move(
                                worker_start_1,
                                to1,
                                worker_start_2,
                                to2,
                            );

                            let reach2 = if end_height_2 == 2 {
                                prelude.standard_neighbor_map[to2 as usize]
                            } else {
                                BitBoard::EMPTY
                            };

                            let is_check = {
                                if prelude.is_against_hypnus
                                    && (end_height_1 != 2 || end_height_2 != 2)
                                {
                                    false
                                } else {
                                    let check_board = (reach1 | reach2) & check_mask;
                                    check_board.is_not_empty()
                                }
                            };

                            result.push(build_scored_move::<F, _>(
                                new_action,
                                is_check,
                                end_height_1 > start_height_1 || end_height_2 > start_height_2,
                            ));
                        }
                    }
                }
            } else {
                if is_mate_only::<F>() {
                    // NOOP - can't mate here anymore
                } else {
                    let final_moves_1 = if is_interact_with_key_squares::<F>() {
                        moves_1 & key_squares
                    } else {
                        moves_1
                    };

                    for to1 in final_moves_1 {
                        let height = prelude.board.get_height(to1);
                        let is_check = height == 2
                            && (prelude.standard_neighbor_map[to1 as usize] & check_mask)
                                .is_not_empty();

                        let new_action = CastorMove::new_single_move(worker_start_1, to1);

                        result.push(build_scored_move::<F, _>(
                            new_action,
                            is_check,
                            height > start_height_1,
                        ));
                    }
                }
            }
        }
    }

    if is_mate_only::<F>() || MUST_CLIMB {
        return result;
    }

    let unblocked_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);

    let mut own_workers = prelude.own_workers.into_iter();
    let worker_start_1 = own_workers.next().unwrap();

    // Double builds
    let worker_start_state = get_worker_start_move_state(&prelude, worker_start_1);

    let possible_builds_1 =
        NEIGHBOR_MAP[worker_start_1 as usize] & unblocked_squares & prelude.build_mask;

    let mut reach = if worker_start_state.worker_start_height == 2 {
        prelude.standard_neighbor_map[worker_start_1 as usize]
    } else {
        BitBoard::EMPTY
    };

    if let Some(worker_start_2) = worker_start_state.other_own_workers.into_iter().next() {
        if prelude.is_against_hypnus
            && (prelude.own_workers & prelude.exactly_level_2) != prelude.own_workers
        {
            reach = BitBoard::EMPTY;
        } else {
            if prelude.board.get_height(worker_start_2) == 2 {
                reach |= prelude.standard_neighbor_map[worker_start_2 as usize];
            }
            reach &= unblocked_non_own_workers & prelude.win_mask;
        }

        let possible_builds_2 =
            NEIGHBOR_MAP[worker_start_2 as usize] & unblocked_squares & prelude.build_mask;

        let overlap = possible_builds_1 & possible_builds_2;
        let not_overlap = !overlap;

        for b1 in possible_builds_1 {
            let b1_mask = BitBoard::as_mask(b1);

            let b2_builds =
                if is_interact_with_key_squares::<F>() && (key_squares & b1_mask).is_empty() {
                    possible_builds_2 & key_squares
                } else {
                    possible_builds_2 & !(prelude.exactly_level_3 & b1_mask)
                };

            for b2 in b2_builds {
                let b2_mask = BitBoard::as_mask(b2);
                let both_mask = b1_mask | b2_mask;
                if (both_mask & not_overlap).is_empty() {
                    if (b2 as u8) > (b1 as u8) {
                        continue;
                    }
                }

                let is_check = {
                    let final_lvl_3 = if b1 == b2 {
                        (prelude.exactly_level_3 & !both_mask)
                            | (prelude.exactly_level_1 & both_mask)
                    } else {
                        (prelude.exactly_level_3 & !both_mask)
                            | (prelude.exactly_level_2 & both_mask)
                    };
                    (final_lvl_3 & reach).is_not_empty()
                };
                let new_action = CastorMove::new_double_build(b1, b2);
                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    } else {
        reach &= unblocked_squares & prelude.win_mask;

        let narrowed_builds = if is_interact_with_key_squares::<F>() {
            possible_builds_1 & key_squares
        } else {
            possible_builds_1
        };

        for b1 in narrowed_builds {
            let b1_mask = BitBoard::as_mask(b1);
            let is_check = {
                let final_lvl_3 =
                    (prelude.exactly_level_3 & !b1_mask) | (prelude.exactly_level_2 & b1_mask);

                (final_lvl_3 & reach).is_not_empty()
            };
            let new_action = CastorMove::new_single_build(b1);
            result.push(build_scored_move::<F, _>(new_action, is_check, false));
        }
    }

    result
}

pub const fn build_castor() -> GodPower {
    god_power(
        GodName::Castor,
        build_god_power_movers!(castor_move_gen),
        build_god_power_actions::<CastorMove>(),
        2979614850588903286,
        362356524330526493,
    )
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::fen::parse_fen;

    use super::*;

    #[test]
    fn test_castor_wins_move_out_of_eachothers_way_1() {
        let fen = "0000000000002300000000000/1/castor:C3,D3/mortal:A1,B1";
        let state = parse_fen(fen).unwrap();
        let castor = GodName::Castor.to_power();

        let next_moves = castor.get_moves_for_search(&state, Player::One);
        for m in next_moves {
            if m.action.get_is_winning() {
                return;
            }
        }

        assert!(false, "Could not find expected win");
    }

    #[test]
    fn test_castor_wins_move_out_of_eachothers_way_2() {
        let fen = "0000000000003200000000000/1/castor:C3,D3/mortal:A1,B1";
        let state = parse_fen(fen).unwrap();
        let castor = GodName::Castor.to_power();

        let next_moves = castor.get_moves_for_search(&state, Player::One);
        for m in next_moves {
            if m.action.get_is_winning() {
                return;
            }
        }

        assert!(false, "Could not find expected win");
    }

    #[test]
    fn test_castor_debug() {
        let fen = "0000000000000000000000000/1/castor:D5,A3/castor:C4";
        let state = parse_fen(fen).unwrap();
        let castor = GodName::Castor.to_power();
        let mortal = GodName::Mortal.to_power();

        let next_moves = castor.get_moves_for_search(&state, Player::One);
        for m in next_moves {
            let action = m.action;
            if !action.get_is_winning() {
                continue;
            }
            let next_state = state.next_state(castor, mortal, action);
            next_state.print_to_console();
            eprintln!("{} / {:0b}", castor.stringify_move(action), action.0);
        }
    }
}
