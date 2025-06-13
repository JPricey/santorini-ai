use crate::{
    board::{
        BitmapType, IS_WINNER_MASK, MAIN_SECTION_MASK, NEIGHBOR_MAP, Player, SantoriniState,
        position_to_coord,
    },
    search::{Hueristic, WINNING_SCORE},
};

use super::{FullChoice, FullChoiceMapper, GodPower, PartialAction, StateOnlyMapper};

fn player_advantage(state: &SantoriniState, player: Player) -> Hueristic {
    let player_index = player as usize;

    if state.workers[player_index] & IS_WINNER_MASK > 0 {
        // panic!("not possible?");
        return WINNING_SCORE;
    }

    let mut result: Hueristic = 0;
    let mut current_workers = state.workers[player_index];
    while current_workers != 0 {
        let worker_pos = current_workers.trailing_zeros() as usize;
        let worker_mask: BitmapType = 1 << worker_pos;
        current_workers ^= worker_mask;

        let height = state.get_height_for_worker(worker_mask);
        result += 10 * height as i32;
        if height == 2 {
            result += 10;
        }

        let too_high = std::cmp::min(3, height + 1);
        let worker_moves = NEIGHBOR_MAP[worker_pos] & !state.height_map[too_high];
        for h in (0..too_high).rev() {
            let mult = if h == 2 { 10 } else { h + 1 };
            result +=
                ((state.height_map[h] & worker_moves).count_ones() * mult as u32) as Hueristic;
        }
    }

    result
}

fn get_next_states_custom<T, M>(state: &SantoriniState, player: Player) -> Vec<T>
where
    M: super::ResultsMapper<T>,
{
    let mut result: Vec<T> = Vec::with_capacity(128);

    let current_player_idx = player as usize;
    let starting_current_workers = state.workers[current_player_idx] & MAIN_SECTION_MASK;
    let mut current_workers = starting_current_workers;

    let all_workers_mask = state.workers[0] | state.workers[1];

    while current_workers != 0 {
        let moving_worker_start_pos = current_workers.trailing_zeros() as usize;
        let moving_worker_start_mask: BitmapType = 1 << moving_worker_start_pos;
        current_workers ^= moving_worker_start_mask;

        let mut mapper = M::new();
        mapper.add_action(PartialAction::SelectWorker(position_to_coord(
            moving_worker_start_pos,
        )));

        let all_stable_workers = all_workers_mask ^ moving_worker_start_mask;
        let worker_starting_height = state.get_height_for_worker(moving_worker_start_mask);

        // Remember that actual height map is offset by 1
        let too_high = std::cmp::min(3, worker_starting_height + 1);
        let mut worker_moves = NEIGHBOR_MAP[moving_worker_start_pos]
            & !state.height_map[too_high]
            & !all_stable_workers;

        while worker_moves != 0 {
            let worker_move_pos = worker_moves.trailing_zeros() as usize;
            let worker_move_mask: BitmapType = 1 << worker_move_pos;
            worker_moves ^= worker_move_mask;

            let mut mapper = mapper.clone();
            mapper.add_action(PartialAction::MoveWorker(position_to_coord(
                worker_move_pos,
            )));

            if state.height_map[2] & worker_move_mask > 0 {
                let mut winning_next_state = state.clone();
                winning_next_state.workers[current_player_idx] ^=
                    moving_worker_start_mask | worker_move_mask | IS_WINNER_MASK;
                winning_next_state.flip_current_player();
                result.push(mapper.map_result(winning_next_state));
                continue;
            }

            let mut worker_builds =
                NEIGHBOR_MAP[worker_move_pos] & !all_stable_workers & !state.height_map[3];

            while worker_builds != 0 {
                let worker_build_pos = worker_builds.trailing_zeros() as usize;
                let worker_build_mask = 1 << worker_build_pos;
                worker_builds ^= worker_build_mask;

                let mut mapper = mapper.clone();
                mapper.add_action(PartialAction::Build(position_to_coord(worker_build_pos)));

                let mut next_state = state.clone();
                next_state.flip_current_player();
                for height in 0.. {
                    if next_state.height_map[height] & worker_build_mask == 0 {
                        next_state.height_map[height] |= worker_build_mask;
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

pub const fn get_mortal_god() -> GodPower {
    GodPower {
        player_advantage_fn: player_advantage,
        next_states: get_next_states_custom::<SantoriniState, StateOnlyMapper>,
        // next_state_with_scores_fn: get_next_states_custom::<StateWithScore, HueristicMapper>,
        next_states_interactive: get_next_states_custom::<FullChoice, FullChoiceMapper>,
    }
}
