use serde_json::map::Iter;

use crate::{
    bitboard::BitBoard,
    board::{BoardState, NEIGHBOR_MAP},
    gods::{FullAction, PartialAction},
    move_container::{GenericMove, MoveData},
    player::Player,
    square::Square,
    utils::grid_position_builder,
};

// TODO: bitflags?
type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const RETURN_FIRST_MATE: MoveGenFlags = STOP_ON_MATE | MATE_ONLY;
// const INCLUDE_QUIET: MoveGenFlags = 1 << 1;

const LOWER_POSITION_MASK: u8 = 0b11111;
#[allow(dead_code)]
const POSITION_WIDTH: usize = 5;
const MORTAL_BUILD_POSITION_OFFSET: usize = 25;

const MORTAL_SCORE_WIDTH: usize = 8;
const MORTAL_SCORE_OFFSET: usize = 64 - MORTAL_SCORE_WIDTH;
const MORTAL_SCORE_ANTI_MASK: u64 = (1 << MORTAL_SCORE_OFFSET) - 1;

const MORTAL_MOVE_IS_WINNING_OFFSET: usize = MORTAL_SCORE_OFFSET - 1;
const MORTAL_MOVE_IS_WINNING_MASK: u64 = 1 << MORTAL_MOVE_IS_WINNING_OFFSET;

const GRID_POSITION_SCORES: [u8; 25] = grid_position_builder(0, 1, 2, 3, 4, 5);
const WORKER_HEIGHT_SCORES: [u8; 4] = [0, 10, 25, 10];

/*
struct MortalMoveIterator<'a> {
    pub workers_to_move_iter: BitBoard,
    pub current_moves_iter: BitBoard,
    pub board: &'a BoardState,
}

impl<'a> MortalMoveIterator<'a> {
    pub fn new(board: &'a BoardState, workers_to_move: BitBoard) -> Self {
        Self {
            workers_to_move_iter: workers_to_move.into_iter(),
            board,
        }
    }
}

impl<'a> Iterator for MortalMoveIterator<'a> {
    type Item = Square;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}
*/

impl GenericMove {
    fn new_mortal_move(
        move_from_mask: BitBoard,
        move_to_mask: BitBoard,
        build_position: Square,
    ) -> GenericMove {
        let mut data: MoveData = (move_from_mask.0 | move_to_mask.0) as MoveData;
        data |= (build_position as MoveData) << MORTAL_BUILD_POSITION_OFFSET;

        Self::new(data)
    }

    fn new_mortal_winning_move(move_from_mask: BitBoard, move_to_mask: BitBoard) -> GenericMove {
        let data: MoveData = (move_from_mask.0 | move_to_mask.0) as MoveData;
        Self::new_winning_move(data)
    }

    fn worker_move_mask(&self) -> u32 {
        (self.data as u32) & BitBoard::MAIN_SECTION_MASK.0 as u32
    }

    fn build_position(&self) -> u8 {
        ((self.data >> MORTAL_BUILD_POSITION_OFFSET) as u8) & LOWER_POSITION_MASK
    }
}

pub fn mortal_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let current_player = board.current_player;
    let worker_move_mask = action.worker_move_mask();
    let current_workers = board.workers[current_player as usize];

    let moving_worker_mask = current_workers.0 & worker_move_mask;
    let result_worker_mask = worker_move_mask ^ moving_worker_mask;

    if action.get_is_winning() {
        return vec![vec![
            PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
            PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        ]];
    }

    let build_position = action.build_position();
    return vec![vec![
        PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
        PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        PartialAction::Build(Square::from(build_position as usize)),
    ]];
}

pub fn mortal_make_move(board: &mut BoardState, action: GenericMove) {
    let current_player = board.current_player;
    board.flip_current_player();
    let worker_move_mask = action.worker_move_mask();
    board.workers[current_player as usize].0 ^= worker_move_mask;

    if action.get_is_winning() {
        board.set_winner(current_player);
        return;
    }

    let build_position = action.build_position();
    let build_mask = BitBoard::as_mask_u8(build_position);

    for height in 0..4 {
        if (board.height_map[height] & build_mask).is_empty() {
            board.height_map[height] ^= build_mask;
            return;
        }
    }
    panic!("Expected to build, but couldn't")
}

pub fn mortal_unmake_move(board: &mut BoardState, action: GenericMove) {
    board.flip_current_player();
    let worker_move_mask = action.worker_move_mask();
    board.workers[board.current_player as usize].0 ^= worker_move_mask;

    if action.get_is_winning() {
        board.unset_winner(board.current_player);
        return;
    }

    let build_position = action.build_position();
    let build_mask = BitBoard::as_mask_u8(build_position);

    for height in (0..4).rev() {
        if (board.height_map[height] & build_mask).is_not_empty() {
            board.height_map[height] ^= build_mask;
            break;
        }
    }
}

/*
 * Score calculation:
 * Mate: 255
 * Baseline: 128
 * For each check move that this adds: +50
 * If we went up: +13
 * If we went down: -13
 * + our new GRID_POSITION_SCORES
 * - our old GRID_POSITION_SCORES
 */
pub fn mortal_move_gen<const F: MoveGenFlags>(
    board: &BoardState,
    player: Player,
) -> Vec<GenericMove> {
    let mut result = Vec::with_capacity(128);

    let current_player_idx = player as usize;
    let starting_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let current_workers = starting_current_workers;

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);
        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let baseline_score = 50
            - GRID_POSITION_SCORES[moving_worker_start_pos as usize]
            - WORKER_HEIGHT_SCORES[worker_starting_height];

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[too_high] | all_workers_mask);

        if worker_starting_height != 3 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                let winning_move = GenericMove::new_mortal_winning_move(
                    moving_worker_start_mask,
                    BitBoard::as_mask(moving_worker_end_pos),
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
            let worker_end_height = board.get_height_for_worker(moving_worker_end_mask);

            let baseline_score = baseline_score
                + GRID_POSITION_SCORES[moving_worker_end_pos as usize]
                + WORKER_HEIGHT_SCORES[worker_end_height as usize];

            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            let (check_count, builds_that_result_in_checks, build_that_remove_checks) =
                if worker_end_height == 2 {
                    let exactly_level_2 = board.height_map[1] & !board.height_map[2];
                    let level_3 = board.height_map[2];
                    // (worker_builds & exactly_level_2)
                    let check_count = (worker_builds & level_3).0.count_ones();
                    let builds_that_result_in_checks = worker_builds & exactly_level_2;
                    let builds_that_remove_checks = worker_builds & level_3;
                    (
                        check_count as u8,
                        builds_that_result_in_checks,
                        builds_that_remove_checks,
                    )
                } else {
                    (0, BitBoard::EMPTY, BitBoard::EMPTY)
                };

            for worker_build_pos in worker_builds {
                let mut new_action = GenericMove::new_mortal_move(
                    moving_worker_start_mask,
                    moving_worker_end_mask,
                    worker_build_pos,
                );
                if F & INCLUDE_SCORE != 0 {
                    let check_count = check_count
                        + ((builds_that_result_in_checks & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as u8)
                        - ((build_that_remove_checks & BitBoard::as_mask(worker_build_pos))
                            .is_not_empty() as u8);
                    new_action.set_score(baseline_score + check_count * 30);
                }
                result.push(new_action);
            }
        }
    }

    result
}
