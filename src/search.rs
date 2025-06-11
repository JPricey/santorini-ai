use std::time::Duration;

use rand::seq::SliceRandom;

use crate::{
    board::StateWithScore,
    transposition_table::{SearchScore, TTValue},
};

use super::{
    board::{BitmapType, IS_WINNER_MASK, NEIGHBOR_MAP, Player, SantoriniState},
    transposition_table::TranspositionTable,
};

struct MortalAgent {}

pub type Hueristic = i32;
pub const WINNING_SCORE: Hueristic = 1000;
pub const WINNING_SCORE_BUFFER: Hueristic = 900;

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

pub static mut NUM_SEARCHES: usize = 0;

pub fn judge_state(state: &SantoriniState, depth: Hueristic) -> Hueristic {
    if let Some(winner) = state.get_winner() {
        let new_score = winner.color() * (WINNING_SCORE - depth as Hueristic);
        return new_score;
    }

    MortalAgent::hueristic(state, Player::One) - MortalAgent::hueristic(state, Player::Two)
}

struct SearchState {
    tt: TranspositionTable,
    start_time: std::time::Instant,
    duration: Duration,
    last_fully_completed_depth: usize,
    best_move: Option<StateWithScore>,
}

impl SearchState {
    pub fn should_stop(&self) -> bool {
        self.start_time.elapsed() > self.duration
    }
}

pub fn santorini_search(root: &SantoriniState, duration_secs: f32) -> StateWithScore {
    let mut search_state = SearchState {
        tt: TranspositionTable::new(),
        start_time: std::time::Instant::now(),
        duration: Duration::from_secs_f32(duration_secs),
        last_fully_completed_depth: 0,
        best_move: None,
    };

    search_with_state(&mut search_state, root)
}

fn search_with_state(search_state: &mut SearchState, root: &SantoriniState) -> StateWithScore {
    let color = root.current_player.color();

    if root.get_winner().is_some() {
        println!("Passed an already won state?");
        return (root.clone(), color * judge_state(root, 0));
    }

    let start_time = std::time::Instant::now();
    let starting_depth = 3;

    for depth in starting_depth.. {
        if search_state.should_stop() {
            println!(
                "Stopping search. Last completed depth {}. Duration: {} seconds",
                search_state.last_fully_completed_depth,
                start_time.elapsed().as_secs_f32()
            );
            break;
        }

        let score = _inner_search(
            search_state,
            root,
            0,
            depth,
            color,
            Hueristic::MIN + 1,
            Hueristic::MAX,
        );

        if score.abs() > WINNING_SCORE_BUFFER {
            println!("Mate found, ending search early");
            break;
        }
    }

    search_state.best_move.clone().unwrap()
}

fn _inner_search(
    search_state: &mut SearchState,
    state: &SantoriniState,
    depth: Hueristic,
    remaining_depth: usize,
    color: Hueristic,
    mut alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic {
    if remaining_depth == 0 || state.get_winner().is_some() {
        return color * judge_state(state, depth);
    }

    let mut track_used = false;
    let mut track_unused = false;
    let tt_entry = search_state.tt.fetch(state);
    if let Some(tt_value) = tt_entry {
        if tt_value.search_depth >= remaining_depth as u8 {
            if TranspositionTable::IS_TRACKING_STATS {
                track_used = true;
            }

            match tt_value.score {
                SearchScore::Exact(score) => {
                    return score;
                }
                SearchScore::LowerBound(score) => {
                    if score >= beta {
                        return score;
                    }
                }
                SearchScore::UpperBound(score) => {
                    if score <= alpha {
                        return score;
                    }
                }
            }
        } else if TranspositionTable::IS_TRACKING_STATS {
            track_unused = true;
        }
    }

    let alpha_orig = alpha;

    let mut children = state.get_next_states_with_scores();

    if let Some(tt_value) = tt_entry {
        children.sort_by(|a, b| {
            if a.0 == tt_value.best_child {
                std::cmp::Ordering::Less
            } else if b.0 == tt_value.best_child {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
            .then((color * b.1).partial_cmp(&(color * a.1)).unwrap())
        });
    } else {
        children.sort_by(|a, b| (color * b.1).partial_cmp(&(color * a.1)).unwrap())
    }

    if track_used {
        search_state.tt.stats.used_value += 1;
    } else if track_unused {
        search_state.tt.stats.unused_value += 1;
    }

    let mut best_board = &children[0].0;
    let mut best_score = Hueristic::MIN;

    for (child, _) in &children {
        let score = -_inner_search(
            search_state,
            child,
            depth + 1,
            remaining_depth - 1,
            -color,
            -beta,
            -alpha,
        );

        let should_stop = search_state.should_stop();

        if score > best_score {
            best_score = score;
            best_board = child;

            if depth == 0 && !should_stop {
                search_state.best_move = Some((best_board.clone(), score));
                println!(
                    "Setting next best move: {:?} ({}). depth: {}",
                    best_board, score, remaining_depth
                );
            }

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    break;
                }
            }
        }

        if should_stop {
            break;
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
        best_child: best_board.clone(),
        search_depth: remaining_depth as u8,
        score: tt_score,
    };

    search_state.tt.insert(state, tt_value);

    if depth == 0 {
        search_state.last_fully_completed_depth = remaining_depth;
    }

    best_score
}
