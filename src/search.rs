use crate::transposition_table::{SearchScore, TTValue};

use super::{
    board::{BitmapType, IS_WINNER_MASK, NEIGHBOR_MAP, Player, SantoriniState},
    transposition_table::TranspositionTable,
};

struct MortalAgent {}

pub type Hueristic = i32;
pub const WINNING_SCORE: Hueristic = 1000;

impl MortalAgent {
    pub fn hueristic(state: &SantoriniState, player: Player) -> Hueristic {
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
            result += 10 * height as Hueristic;
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
}

pub struct AlphaBetaSearch {}

pub static mut NUM_SEARCHES: usize = 0;

pub fn judge_state(state: &SantoriniState, depth_bonus: usize) -> Hueristic {
    if let Some(winner) = state.get_winner() {
        let new_score = winner.color() * (WINNING_SCORE + depth_bonus as Hueristic);
        return new_score;
    }

    MortalAgent::hueristic(state, Player::One) - MortalAgent::hueristic(state, Player::Two)
}

impl AlphaBetaSearch {
    pub fn search(root: &SantoriniState, depth: usize) -> (SantoriniState, Hueristic) {
        let mut tt = TranspositionTable::new();
        let color = root.current_player.color();

        if root.get_winner().is_some() {
            println!("Passed an already won state?");
            return (root.clone(), color * judge_state(root, 100));
        }

        let res = Self::_inner_search(
            root,
            &mut tt,
            depth,
            color,
            Hueristic::MIN + 1,
            Hueristic::MAX,
        );

        println!("TT stats: {:?}", tt.stats);

        res
    }

    fn _inner_search(
        state: &SantoriniState,
        tt: &mut TranspositionTable,
        remaining_depth: usize,
        color: Hueristic,
        mut alpha: Hueristic,
        beta: Hueristic,
    ) -> (SantoriniState, Hueristic) {
        if remaining_depth == 0 || state.get_winner().is_some() {
            return (state.clone(), color * judge_state(state, remaining_depth));
        }

        let mut track_used = false;
        let tt_entry = tt.fetch(state);
        if let Some(tt_value) = tt_entry {
            if tt_value.search_depth >= remaining_depth as u8 {
                if TranspositionTable::IS_TRACKING_STATS {
                    track_used = true;
                }

                match tt_value.score {
                    SearchScore::Exact(score) => {
                        return (state.clone(), score);
                    }
                    SearchScore::LowerBound(score) => {
                        if score >= beta {
                            return (state.clone(), score);
                        }
                    }
                    SearchScore::UpperBound(score) => {
                        if score <= alpha {
                            return (state.clone(), score);
                        }
                    }
                }
            } else if TranspositionTable::IS_TRACKING_STATS {
                tt.stats.unused_value += 1;
            }
        }

        if track_used {
            tt.stats.used_value += 1;
        }

        let alpha_orig = alpha;

        let mut children = state.get_next_states_with_scores();
        if color == 1 {
            children.sort_by(|a, b| (b.1).partial_cmp(&a.1).unwrap());
        } else {
            children.sort_by(|a, b| (a.1).partial_cmp(&b.1).unwrap());
        }

        // for (child, child_guess) in &children {
        //     child.print_to_console();
        //     println!("score^ {}", child_guess);
        // }

        let mut best_board = &children[0].0;
        let mut best_score = Hueristic::MIN;

        for (child, _) in &children {
            let (_, score) =
                Self::_inner_search(child, tt, remaining_depth - 1, -color, -beta, -alpha);
            let score = -score;

            if score > best_score {
                best_score = score;
                best_board = child;

                if score > alpha {
                    alpha = score;

                    if alpha >= beta {
                        // println!("prune");
                        break;
                    }
                }
            }
        }

        let tt_score = if best_score <= alpha_orig {
            SearchScore::UpperBound(best_score)
        } else if best_score >= beta {
            SearchScore::LowerBound(best_score)
        } else {
            SearchScore::Exact(best_score)
        };

        let tt_value = TTValue {
            search_depth: remaining_depth as u8,
            score: tt_score,
        };

        tt.insert(state, tt_value);

        (best_board.clone(), best_score)
    }
}
