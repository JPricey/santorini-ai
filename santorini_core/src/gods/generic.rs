use crate::{
    bitboard::BitBoard,
    board::{BoardState, IS_WINNER_MASK, NEIGHBOR_MAP},
    move_container::{self, ChildMoveContainer, GenericMove},
    player::Player,
    square::Square,
};

// TODO: bitflags?
type MoveGenFlags = u8;
const STOP_ON_MATE: MoveGenFlags = 1 << 0;
const MATE_ONLY: MoveGenFlags = 1 << 2;
const ANY_MATE_CHECK: MoveGenFlags = STOP_ON_MATE | MATE_ONLY;
// const INCLUDE_QUIET: MoveGenFlags = 1 << 1;

const LOWER_POSITION_MASK: u8 = 0b11111;
const MORTAL_BUILD_POSITION_OFFSET: usize = 32;
const MORTAL_MOVE_IS_WINNING_OFFSET: usize = 63;
const MORTAL_MOVE_IS_WINNING_MASK: u64 = 1 << MORTAL_MOVE_IS_WINNING_OFFSET;
fn build_mortal_winning_move(move_from_mask: BitBoard, move_to_mask: BitBoard) -> GenericMove {
    let data: u64 = (move_from_mask.0 | move_to_mask.0) as u64 | MORTAL_MOVE_IS_WINNING_MASK;
    GenericMove(data)
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

pub fn make_move(board: &mut BoardState, action: GenericMove) {
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

pub fn unmake_move(board: &mut BoardState, action: GenericMove) {
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

// TODO: accept a move accumulator and use that instead of returning a vec
pub fn mortal_move_gen<const F: MoveGenFlags>(
    move_container: &mut ChildMoveContainer,
    board: &BoardState,
    player: Player,
) {
    let current_player_idx = player as usize;
    let starting_current_workers = board.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let current_workers = starting_current_workers;

    let all_workers_mask = board.workers[0] | board.workers[1];

    for moving_worker_start_pos in current_workers.into_iter() {
        let moving_worker_start_mask = BitBoard::as_mask(moving_worker_start_pos);

        let worker_starting_height = board.get_height_for_worker(moving_worker_start_mask);

        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos as usize]
            & !(board.height_map[too_high] | all_workers_mask);

        if worker_starting_height != 3 {
            let moves_to_level_3 = worker_moves & board.height_map[2];
            worker_moves ^= moves_to_level_3;

            for moving_worker_end_pos in moves_to_level_3.into_iter() {
                move_container.push(build_mortal_winning_move(
                    moving_worker_start_mask,
                    BitBoard::as_mask(moving_worker_end_pos),
                ));
                if F & STOP_ON_MATE != 0 {
                    return;
                }
            }
        }

        if F & MATE_ONLY != 0 {
            continue;
        }

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let buildable_squares = !(non_selected_workers | board.height_map[3]);

        for moving_worker_end_pos in worker_moves.into_iter() {
            let worker_builds = NEIGHBOR_MAP[moving_worker_end_pos as usize] & buildable_squares;

            for worker_build_pos in worker_builds {
                move_container.push(build_mortal_move(
                    moving_worker_start_mask,
                    BitBoard::as_mask(moving_worker_end_pos),
                    worker_build_pos,
                ));
            }
        }
    }
}
