use crate::{
    bitboard::BitBoard,
    board::{BoardState, IS_WINNER_MASK, NEIGHBOR_MAP},
    gods::{BoardStateWithAction, FullAction, PartialAction},
    move_container::{self, ChildMoveContainer, GenericMove},
    player::Player,
    square::Square,
    utils::grid_position_builder,
};

// TODO: bitflags?
type MoveGenFlags = u8;
pub const STOP_ON_MATE: MoveGenFlags = 1 << 0;
pub const MATE_ONLY: MoveGenFlags = 1 << 2;
pub const INCLUDE_SCORE: MoveGenFlags = 1 << 3;
pub const ANY_MATE_CHECK: MoveGenFlags = STOP_ON_MATE | MATE_ONLY;
// const INCLUDE_QUIET: MoveGenFlags = 1 << 1;

const LOWER_POSITION_MASK: u8 = 0b11111;
const POSITION_WIDTH: usize = 5;
const MORTAL_BUILD_POSITION_OFFSET: usize = 25;
const MORTAL_MOVE_IS_WINNING_OFFSET: usize = 63;
const MORTAL_MOVE_IS_WINNING_MASK: u64 = 1 << MORTAL_MOVE_IS_WINNING_OFFSET;

const MORTAL_SCORE_OFFSET: usize = MORTAL_BUILD_POSITION_OFFSET + POSITION_WIDTH;

const GRID_POSITION_SCORES: [u8; 25] = grid_position_builder(0, 1, 2, 3, 4, 5);
const WORKER_HEIGHT_SCORES: [u8; 4] = [0, 10, 25, 10];

pub fn mortal_add_score_to_move(action: &mut GenericMove, score: u8) {
    action.0 |= (score as u64) << MORTAL_SCORE_OFFSET;
}

pub fn mortal_get_score(action: GenericMove) -> u8 {
    (action.0 >> MORTAL_SCORE_OFFSET) as u8
}

fn build_mortal_winning_move(move_from_mask: BitBoard, move_to_mask: BitBoard) -> GenericMove {
    let data: u64 = (move_from_mask.0 | move_to_mask.0) as u64 | MORTAL_MOVE_IS_WINNING_MASK;
    GenericMove(data)
}

pub fn is_move_winning(action: GenericMove) -> bool {
    action.0 & MORTAL_MOVE_IS_WINNING_MASK != 0
}

fn build_mortal_move(
    move_from_mask: BitBoard,
    move_to_mask: BitBoard,
    build_position: Square,
) -> GenericMove {
    let mut data: u64 = (move_from_mask.0 | move_to_mask.0) as u64;
    data |= (build_position as u64) << MORTAL_BUILD_POSITION_OFFSET;

    GenericMove(data)
}

pub fn mortal_move_to_actions(board: &BoardState, action: GenericMove) -> Vec<FullAction> {
    let current_player = board.current_player;
    let worker_move_mask: u32 = (action.0 as u32) & BitBoard::MAIN_SECTION_MASK.0;
    let current_workers = board.workers[current_player as usize];

    let moving_worker_mask = current_workers.0 & worker_move_mask;
    let result_worker_mask = worker_move_mask ^ moving_worker_mask;

    if action.0 & MORTAL_MOVE_IS_WINNING_MASK > 0 {
        return vec![vec![
            PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
            PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        ]];
    }

    let build_position = (action.0 >> MORTAL_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
    return vec![vec![
        PartialAction::SelectWorker(Square::from(moving_worker_mask.trailing_zeros() as usize)),
        PartialAction::MoveWorker(Square::from(result_worker_mask.trailing_zeros() as usize)),
        PartialAction::Build(Square::from(build_position as usize)),
    ]];
}

pub fn mortal_make_move(board: &mut BoardState, action: GenericMove) {
    let current_player = board.current_player;
    board.flip_current_player();
    let worker_move_mask: u32 = (action.0 as u32) & BitBoard::MAIN_SECTION_MASK.0;
    board.workers[current_player as usize].0 ^= worker_move_mask;

    if action.0 & MORTAL_MOVE_IS_WINNING_MASK > 0 {
        board.set_winner(current_player);
        return;
    }

    let build_position = (action.0 >> MORTAL_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
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
    let worker_move_mask: u32 = (action.0 as u32) & BitBoard::MAIN_SECTION_MASK.0;
    board.workers[board.current_player as usize].0 ^= worker_move_mask;

    if action.0 & MORTAL_MOVE_IS_WINNING_MASK > 0 {
        board.unset_winner(board.current_player);
        return;
    }

    let build_position = (action.0 >> MORTAL_BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
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
                let mut winning_move = build_mortal_winning_move(
                    moving_worker_start_mask,
                    BitBoard::as_mask(moving_worker_end_pos),
                );
                if F & INCLUDE_SCORE != 0 {
                    mortal_add_score_to_move(&mut winning_move, u8::MAX);
                }
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
                let mut new_action = build_mortal_move(
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
                    mortal_add_score_to_move(&mut new_action, baseline_score + check_count * 30);
                }
                result.push(new_action);
            }
        }
    }

    result
}
