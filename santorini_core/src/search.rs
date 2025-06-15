use std::sync::{Arc, atomic::AtomicBool};

use serde::{Deserialize, Serialize};

use crate::{
    board::FullGameState,
    gods::GodPower,
    transposition_table::{SearchScore, TTValue},
};

use super::{
    board::{BoardState, Player},
    transposition_table::TranspositionTable,
};

pub type Hueristic = i32;
pub const WINNING_SCORE: Hueristic = 1000;
pub const WINNING_SCORE_BUFFER: Hueristic = 900;
pub static mut NUM_SEARCHES: usize = 0;

pub fn judge_state(
    state: &BoardState,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    depth: Hueristic,
) -> Hueristic {
    if let Some(winner) = state.get_winner() {
        let new_score = winner.color() * (WINNING_SCORE - depth as Hueristic);
        return new_score;
    }

    (p1_god.player_advantage_fn)(state, Player::One)
        - (p2_god.player_advantage_fn)(state, Player::Two)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BestMoveTrigger {
    StopFlag,
    EndOfLine,
    Improvement,
}

#[derive(Clone, Debug)]
pub struct NewBestMove {
    pub state: FullGameState,
    pub score: Hueristic,
    pub depth: usize,
    pub trigger: BestMoveTrigger,
}

impl NewBestMove {
    pub fn new(
        state: FullGameState,
        score: Hueristic,
        depth: usize,
        trigger: BestMoveTrigger,
    ) -> Self {
        NewBestMove {
            state,
            score,
            depth,
            trigger,
        }
    }
}

pub struct SearchState<'a> {
    pub tt: &'a mut TranspositionTable,
    pub stop_flag: Arc<AtomicBool>,
    pub new_best_move_callback: Box<dyn FnMut(NewBestMove)>,
    pub last_fully_completed_depth: usize,
    pub best_move: Option<NewBestMove>,
}

impl<'a> SearchState<'a> {
    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn new(tt: &'a mut TranspositionTable) -> Self {
        let new_best_move_callback =
            Box::new(|new_best_move: NewBestMove| eprintln!("{:?}", new_best_move));

        SearchState {
            tt,
            new_best_move_callback,
            stop_flag: Arc::new(AtomicBool::new(false)),
            last_fully_completed_depth: 0,
            best_move: None,
        }
    }
}

pub fn search_with_state(search_state: &mut SearchState, root_state: &FullGameState) {
    let start_time = std::time::Instant::now();
    let p1_god = root_state.p1_god;
    let p2_god = root_state.p2_god;
    let root_board = &root_state.board;
    let color = root_board.current_player.color();

    if root_board.get_winner().is_some() {
        panic!("Can't search on a terminal node");
    }

    let starting_depth = 3;

    for depth in starting_depth.. {
        if search_state.should_stop() {
            eprintln!(
                "Stopping search. Last completed depth {}. Duration: {} seconds",
                search_state.last_fully_completed_depth,
                start_time.elapsed().as_secs_f32(),
            );
            if let Some(best_move) = &mut search_state.best_move {
                best_move.trigger = BestMoveTrigger::StopFlag;
                (search_state.new_best_move_callback)(best_move.clone());
            }
            break;
        }

        let score = _inner_search(
            search_state,
            p1_god,
            p2_god,
            root_board,
            0,
            depth,
            color,
            Hueristic::MIN + 1,
            Hueristic::MAX,
        );

        if score.abs() > WINNING_SCORE_BUFFER && !search_state.should_stop() {
            eprintln!("Mate found, ending search early");
            let mut best_move = search_state.best_move.clone().unwrap();
            best_move.trigger = BestMoveTrigger::EndOfLine;
            (search_state.new_best_move_callback)(best_move);
            break;
        }
    }
}

fn _q_extend(
    state: &BoardState,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    color: Hueristic,
    depth: Hueristic,
    q_depth: u32,
) -> Hueristic {
    let (active_god, other_god) = match state.current_player {
        Player::One => (p1_god, p2_god),
        Player::Two => (p2_god, p1_god),
    };

    // If we have a win right now, just take it
    if (active_god.has_win)(state, state.current_player) {
        let score = WINNING_SCORE - depth - 1;
        return score;
    }

    // If opponent isn't threatening a win, take the current score
    if !(other_god.has_win)(state, state.current_player.other()) {
        return color * judge_state(state, p1_god, p2_god, depth);
    }

    // Opponent is threatening a win right now. Keep looking to confirm if we can block it
    let mut best_score = Hueristic::MIN;
    let children = (active_god.next_states)(state, state.current_player);
    for child in &children {
        let child_score = _q_extend(child, p1_god, p2_god, -color, depth + 1, q_depth + 1);
        if child_score > best_score {
            best_score = child_score;
        }
    }

    best_score
}

fn _inner_search(
    search_state: &mut SearchState,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    state: &BoardState,
    depth: Hueristic,
    remaining_depth: usize,
    color: Hueristic,
    mut alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic {
    if state.get_winner().is_some() {
        return color * judge_state(state, p1_god, p2_god, depth);
    }

    let active_god = match state.current_player {
        Player::One => p1_god,
        Player::Two => p2_god,
    };

    if remaining_depth == 0 {
        return _q_extend(state, p1_god, p2_god, color, depth, 0);
    }

    // TODO: should this only be done at max depth?
    // Move ordering should solve this for us
    if (active_god.has_win)(state, state.current_player) {
        let score = WINNING_SCORE - depth - 1;

        if depth == 0 {
            let children = (active_god.next_states)(state, state.current_player);
            for child in children {
                if let Some(winner) = child.get_winner() {
                    if winner == state.current_player {
                        let new_best_move = NewBestMove::new(
                            FullGameState::new(child, p1_god, p2_god),
                            score,
                            remaining_depth,
                            BestMoveTrigger::EndOfLine,
                        );
                        search_state.best_move = Some(new_best_move.clone());
                        (search_state.new_best_move_callback)(new_best_move);
                        return score;
                    }
                }
            }

            let full_state = FullGameState::new(state.clone(), p1_god, p2_god);
            panic!(
                "Was promised an immediate win but didn't find it? {:?}",
                full_state
            );
        }

        return score;
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

    let mut children = (active_god.next_states)(state, state.current_player);

    if let Some(tt_value) = tt_entry {
        for i in 0..children.len() {
            if children[i] == tt_value.best_child {
                if i == 0 {
                    break;
                } else {
                    children.swap(0, i);
                    break;
                }
            }
        }
    }

    /*
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
    */

    if track_used {
        search_state.tt.stats.used_value += 1;
    } else if track_unused {
        search_state.tt.stats.unused_value += 1;
    }

    let mut best_board = &children[0];
    let mut best_score = Hueristic::MIN;

    for child in &children {
        let score = -_inner_search(
            search_state,
            p1_god,
            p2_god,
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
                let new_best_move = NewBestMove::new(
                    FullGameState::new(best_board.clone(), p1_god, p2_god),
                    score,
                    remaining_depth,
                    BestMoveTrigger::Improvement,
                );
                search_state.best_move = Some(new_best_move.clone());
                (search_state.new_best_move_callback)(new_best_move);
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
