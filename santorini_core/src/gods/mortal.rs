use crate::{
    bitboard::BitBoard,
    board::{BoardState, IS_WINNER_MASK, NEIGHBOR_MAP},
    player::Player,
    search::Hueristic,
    square::Square,
    utils::{grid_position_builder, move_all_workers_one_include_original_workers},
};

use super::{
    BoardStateWithAction, FullChoiceMapper, GodName, GodPower, PartialAction, StateOnlyMapper,
};

const POSITION_BONUS: [Hueristic; 25] = grid_position_builder(-1, 0, 0, 2, 1, 1);
const WORKER_HEIGHT_SCORES: [i32; 4] = [0, 10, 40, 10];

pub fn mortal_player_advantage(state: &BoardState, player: Player) -> Hueristic {
    let player_index = player as usize;

    let mut result: Hueristic = 0;
    let mut current_workers = state.workers[player_index].0;
    let non_worker_mask = !(state.workers[0] | state.workers[1]);

    let mut total_moves_count = 0;

    while current_workers != 0 {
        let worker_pos = current_workers.trailing_zeros();
        let worker_mask = 1 << worker_pos;
        current_workers ^= worker_mask;

        let height = state.get_height_for_worker(BitBoard(worker_mask));
        result += WORKER_HEIGHT_SCORES[height];
        result += POSITION_BONUS[worker_pos as usize];

        let too_high = std::cmp::min(3, height + 1);
        let worker_moves_mask =
            NEIGHBOR_MAP[worker_pos as usize] & !state.height_map[too_high] & non_worker_mask;

        let worker_moves_count = worker_moves_mask.0.count_zeros();
        total_moves_count += worker_moves_count;
        if worker_moves_count == 0 {
            result -= 9;
        } else if worker_moves_count >= 3 {
            result += 9
        };

        // Huge bonus for being able to move to multiple 3's. This is likely winning
        if (state.height_map[2] & worker_moves_mask).0.count_ones() > 1 {
            result += 100;
        }

        // Bonus for being next to 2's
        if (state.height_map[1] & worker_moves_mask).0 > 0 {
            result += 4;
        }

        // Bonus for height of adjacent tiles??
        // for h in (0..too_high).rev() {
        //     let mult = if h == 2 { 10 } else { h + 1 };
        //     result +=
        //         ((state.height_map[h] & worker_moves_mask).count_ones() * mult as u32) as Hueristic;
        // }
    }

    if total_moves_count < 2 {
        result -= 25;
    }

    result
}

pub fn mortal_next_states<T, M, const SHORT_CIRCUIT_WINS: bool>(
    state: &BoardState,
    player: Player,
) -> Vec<T>
where
    M: super::ResultsMapper<T>,
{
    let mut result: Vec<T> = Vec::with_capacity(128);

    let current_player_idx = player as usize;
    let starting_current_workers = state.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;
    let mut current_workers = starting_current_workers;

    let all_workers_mask = state.workers[0] | state.workers[1];

    while current_workers.0 != 0 {
        let moving_worker_start_pos = current_workers.0.trailing_zeros() as usize;
        let moving_worker_start_mask = BitBoard(1 << moving_worker_start_pos);
        current_workers ^= moving_worker_start_mask;

        let mut mapper = M::new();
        mapper.add_action(PartialAction::SelectWorker(Square::from(
            moving_worker_start_pos,
        )));

        let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
        let worker_starting_height = state.get_height_for_worker(moving_worker_start_mask);

        // Remember that actual height map is offset by 1
        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos]
            & !state.height_map[too_high]
            & !non_selected_workers;

        while worker_moves.0 != 0 {
            let worker_move_pos = worker_moves.0.trailing_zeros() as usize;
            let worker_move_mask = BitBoard(1 << worker_move_pos);
            worker_moves ^= worker_move_mask;

            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(Square::from(worker_move_pos)));

            if worker_starting_height != 3 && (state.height_map[2] & worker_move_mask).0 > 0 {
                let mut winning_next_state = state.clone();
                winning_next_state.workers[current_player_idx] ^=
                    moving_worker_start_mask | worker_move_mask | IS_WINNER_MASK;
                winning_next_state.flip_current_player();
                result.push(mapper.map_result(winning_next_state));
                if SHORT_CIRCUIT_WINS {
                    return result;
                }

                continue;
            }

            let mut worker_builds =
                NEIGHBOR_MAP[worker_move_pos] & !non_selected_workers & !state.height_map[3];

            while worker_builds.0 != 0 {
                let worker_build_pos = worker_builds.0.trailing_zeros() as usize;
                let worker_build_mask = 1 << worker_build_pos;
                worker_builds ^= BitBoard(worker_build_mask);

                let mut mapper = mapper.clone();
                mapper.add_action(PartialAction::Build(Square::from(worker_build_pos)));

                let mut next_state = state.clone();
                next_state.flip_current_player();
                for height in 0.. {
                    if next_state.height_map[height].0 & worker_build_mask == 0 {
                        next_state.height_map[height] |= BitBoard(worker_build_mask);
                        break;
                    }
                }
                next_state.workers[current_player_idx] ^=
                    moving_worker_start_mask | worker_move_mask;
                result.push(mapper.map_result(next_state))
            }
        }
    }

    if result.len() == 0 {
        // Lose due to no moves
        let mut next_state = state.clone();
        next_state.workers[1 - current_player_idx] |= IS_WINNER_MASK;
        next_state.flip_current_player();
        let mut mapper = M::new();
        mapper.add_action(PartialAction::NoMoves);
        result.push(mapper.map_result(next_state));
    }

    result
}

pub fn mortal_has_win(state: &BoardState, player: Player) -> bool {
    let current_player_idx = player as usize;
    let starting_current_workers = state.workers[current_player_idx] & BitBoard::MAIN_SECTION_MASK;

    let level_2_workers = starting_current_workers & (state.height_map[1] & !state.height_map[2]);
    let moves_from_lvl_2 = move_all_workers_one_include_original_workers(level_2_workers);
    let open_spaces = !(state.workers[0] | state.workers[1] | state.height_map[3]);
    let exactly_level_3 = state.height_map[2] & open_spaces;
    let level_3_moves = moves_from_lvl_2 & exactly_level_3;

    level_3_moves.0 != 0
}

pub const fn build_mortal() -> GodPower {
    GodPower {
        god_name: GodName::Mortal,
        player_advantage_fn: mortal_player_advantage,
        next_states: mortal_next_states::<BoardState, StateOnlyMapper, true>,
        // next_state_with_scores_fn: get_next_states_custom::<StateWithScore, HueristicMapper>,
        next_states_interactive: mortal_next_states::<BoardStateWithAction, FullChoiceMapper, false>,
        has_win: mortal_has_win,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        board::{FullGameState, Player},
        gods::tests::assert_has_win_consistency,
    };

    #[test]
    fn test_mortal_win_checking() {
        {
            let state_str = "00000 00000 00230 00000 00030/1/mortal:12/mortal:24";
            let mut state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, true);
            state.board.current_player = Player::Two;
            assert_has_win_consistency(&state, false);
        }

        {
            // level 3 is next, but it's blocked by a worker
            let state_str = "00000 00000 00230 00000 00030/1/mortal:12/mortal:13";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, false);
        }

        {
            // level 3 is next, but you're already on level 3
            let state_str = "00000 00000 00330 00000 00030/1/mortal:12/mortal:24";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, false);
        }

        {
            let state_str = "2300000000000000000000000/2/mortal:2,13/mortal:0,17";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, true);
        }

        {
            let state_str = "2144330422342221044000400/2/mortal:1,13/mortal:8,9";
            let state = FullGameState::try_from(state_str).unwrap();

            assert_has_win_consistency(&state, true);
        }
    }
}
