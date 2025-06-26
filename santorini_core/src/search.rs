use std::{
    marker::PhantomData,
    sync::{Arc, atomic::AtomicBool},
};

use serde::{Deserialize, Serialize};

use crate::{
    board::{FullGameState, get_all_permutations_for_pair},
    gods::{
        GodPower,
        generic::{is_move_winning, mortal_add_score_to_move, mortal_get_score},
    },
    move_container::GenericMove,
    nnue::evaluate,
    player::Player,
    transposition_table::{SearchScoreType, TTValue},
};

use super::{board::BoardState, transposition_table::TranspositionTable};

pub type Hueristic = i32;
pub const WINNING_SCORE: Hueristic = 10_000;
pub const WINNING_SCORE_BUFFER: Hueristic = 9000;
pub static mut NUM_SEARCHES: usize = 0;

/// Trait to check if a search should stop at some static boundary
pub trait StaticSearchTerminator {
    fn should_stop(search_state: &SearchState) -> bool;
}

pub struct NoopStaticSearchTerminator {}

impl StaticSearchTerminator for NoopStaticSearchTerminator {
    fn should_stop(_search_state: &SearchState) -> bool {
        false
    }
}

pub struct MaxDepthStaticSearchTerminator<const N: usize> {}
impl<const N: usize> StaticSearchTerminator for MaxDepthStaticSearchTerminator<N> {
    fn should_stop(search_state: &SearchState) -> bool {
        search_state.last_fully_completed_depth >= N
    }
}

pub struct NodesVisitedStaticSearchTerminator<const N: usize> {}
impl<const N: usize> StaticSearchTerminator for NodesVisitedStaticSearchTerminator<N> {
    fn should_stop(search_state: &SearchState) -> bool {
        search_state.nodes_visited >= N
    }
}

pub struct AndStaticSearchTerminator<A: StaticSearchTerminator, B: StaticSearchTerminator> {
    a_type: PhantomData<A>,
    b_type: PhantomData<B>,
}
impl<A: StaticSearchTerminator, B: StaticSearchTerminator> StaticSearchTerminator
    for AndStaticSearchTerminator<A, B>
{
    fn should_stop(search_state: &SearchState) -> bool {
        A::should_stop(search_state) && B::should_stop(search_state)
    }
}

pub struct OrStaticSearchTerminator<A: StaticSearchTerminator, B: StaticSearchTerminator> {
    a_type: PhantomData<A>,
    b_type: PhantomData<B>,
}
impl<A: StaticSearchTerminator, B: StaticSearchTerminator> StaticSearchTerminator
    for OrStaticSearchTerminator<A, B>
{
    fn should_stop(search_state: &SearchState) -> bool {
        A::should_stop(search_state) || B::should_stop(search_state)
    }
}

/*
pub fn judge_non_terminal_state(
    state: &BoardState,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
) -> Hueristic {
    (p1_god.player_advantage_fn)(state, Player::One)
        - (p2_god.player_advantage_fn)(state, Player::Two)
}
*/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BestMoveTrigger {
    StopFlag,
    EndOfLine,
    Improvement,
    Saved,
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

pub struct SearchContext<'a> {
    pub tt: &'a mut TranspositionTable,
    pub stop_flag: Arc<AtomicBool>,
    pub new_best_move_callback: Box<dyn FnMut(NewBestMove)>,
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub last_fully_completed_depth: usize,
    pub best_move: Option<NewBestMove>,
    pub nodes_visited: usize,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            last_fully_completed_depth: 0,
            best_move: None,
            nodes_visited: 0,
        }
    }
}

impl<'a> SearchContext<'a> {
    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn new(tt: &'a mut TranspositionTable) -> Self {
        let new_best_move_callback = Box::new(|_new_best_move: NewBestMove| {
            // eprintln!("{:?}", _new_best_move);
        });

        SearchContext {
            tt,
            new_best_move_callback,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

pub fn search_with_state<T>(
    search_context: &mut SearchContext,
    root_state: &FullGameState,
) -> SearchState
where
    T: StaticSearchTerminator,
{
    // root_state.print_to_console();
    // let eval = evaluate(&root_state.board);
    // eprintln!("nnue eval: {}", eval);

    let mut root_board = root_state.board.clone();
    let color = root_board.current_player.color();

    let mut search_state = SearchState::default();

    if root_board.get_winner().is_some() {
        panic!("Should not search in an already terminal state");
    }

    let starting_depth = {
        if let Some(tt_entry) = search_context.tt.fetch(&root_state.board) {
            let mut best_child_state = root_board.clone();
            let active_god = root_state.get_active_god();
            (active_god.make_move)(&mut best_child_state, tt_entry.best_action);

            let new_best_move = NewBestMove::new(
                FullGameState::new(best_child_state, root_state.gods[0], root_state.gods[1]),
                tt_entry.score,
                tt_entry.search_depth as usize,
                BestMoveTrigger::Saved,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
            tt_entry.search_depth + 1
        } else {
            3
        }
    } as usize;

    for depth in starting_depth.. {
        if search_context.should_stop() || T::should_stop(&search_state) {
            // eprintln!(
            //     "Stopping search. Last completed depth {}. Duration: {} seconds",
            //     search_state.last_fully_completed_depth,
            //     start_time.elapsed().as_secs_f32(),
            // );
            if let Some(best_move) = &mut search_state.best_move {
                best_move.trigger = BestMoveTrigger::StopFlag;
                (search_context.new_best_move_callback)(best_move.clone());
            }
            break;
        }

        let score = _inner_search::<T>(
            search_context,
            &mut search_state,
            root_state.gods[0],
            root_state.gods[1],
            &mut root_board,
            0,
            depth,
            color,
            Hueristic::MIN + 1,
            Hueristic::MAX,
        );

        if score.abs() > WINNING_SCORE_BUFFER
            && !(search_context.should_stop() || T::should_stop(&search_state))
        {
            // eprintln!("Mate found, ending search early");
            let mut best_move = search_state.best_move.clone().unwrap();
            best_move.trigger = BestMoveTrigger::EndOfLine;
            (search_context.new_best_move_callback)(best_move);
            break;
        }
    }

    search_state
}

fn _q_extend(
    state: &mut BoardState,
    search_state: &mut SearchState,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    color: Hueristic,
    depth: Hueristic,
    q_depth: u32,
) -> Hueristic {
    search_state.nodes_visited += 1;

    let (active_god, other_god) = match state.current_player {
        Player::One => (p1_god, p2_god),
        Player::Two => (p2_god, p1_god),
    };

    // If we have a win right now, just take it
    if (active_god.get_win)(state, state.current_player).len() > 0 {
        let score = WINNING_SCORE - depth - 1;
        return score;
    }

    // If opponent isn't threatening a win, take the current score
    if (other_god.get_win)(state, !state.current_player).len() == 0 {
        return evaluate(state);
        // return color * judge_non_terminal_state(state, p1_god, p2_god);
    }

    // Opponent is threatening a win right now. Keep looking to confirm if we can block it
    let mut best_score = Hueristic::MIN;
    let child_moves = (active_god.get_moves)(state, state.current_player);
    for child_move in &child_moves {
        (active_god.make_move)(state, *child_move);

        let child_score = _q_extend(
            state,
            search_state,
            p1_god,
            p2_god,
            -color,
            depth + 1,
            q_depth + 1,
        );
        if child_score > best_score {
            best_score = child_score;
        }

        (active_god.unmake_move)(state, *child_move);
    }

    best_score
}

/*
 * Ideally we want to order states from most -> least promising.
 * Unfortunately, it's slow to identify the promise of a state, and it's slow to run a classic
 * sort.
 *
 * Instead we pseudo-sort:
 * - Take the "advantage fn" of the current god for the current state, and use that as a baseline
 * score.
 * - Any state with a better score than this goes in the top half of the list.
 * - Any state with worse score goes in the bottom half of the list.
 * - Sort back to front, because because swaps are expensive and moves often hurt more than help
 *
 * - Why not run both advantage functions?
 *  - After some testing, this proved to be worse. not sure if it's because of the wasted time or
 *  numbers are actually less meaningful
 *
 */
/*
fn _order_states(
    states: &mut [BoardState],
    current_god: &'static GodPower,
    player: Player,
    baseline_score: Hueristic,
) {
    if states.len() <= 1 {
        return;
    }
    let mut losing = states.len() - 1;
    let mut back = states.len() - 1;
    let mut front = 0;
    let mut best_score = Hueristic::MIN;

    while front < back {
        if states[back].get_winner().is_some() {
            if back != losing {
                states.swap(back, losing);
            }
            losing -= 1;
            back -= 1;
            continue;
        }

        let score = (current_god.player_advantage_fn)(&states[back], player);

        if score <= baseline_score {
            back -= 1;
        } else {
            if score > best_score {
                best_score = score;
            }
            states.swap(back, front);
            front += 1;
        }
    }

    if best_score > baseline_score {
        front = 0;
    } else {
        back = losing;
    }

    if back > 0 && states[back].get_winner().is_some() {
        if back != losing {
            states.swap(back, losing);
        }
        back -= 1;
    }

    while front < back {
        let score = (current_god.player_advantage_fn)(&states[back], player);
        if score == best_score {
            states.swap(back, front);
            front += 1;
        } else {
            back -= 1;
        }
    }
}
*/

fn _select_next_action(actions: &mut Vec<GenericMove>, start_index: usize) {
    let mut best_index = start_index;
    let mut best_score = mortal_get_score(actions[start_index]);
    let mut i = start_index + 1;
    while i < actions.len() {
        let score = mortal_get_score(actions[i]);
        if score > best_score {
            best_score = score;
            best_index = i;
        }

        i += 1
    }

    // eprintln!("best_score: {best_score}");

    if best_index != start_index {
        actions.swap(start_index, best_index);
    }
}

fn _inner_search<T>(
    search_context: &mut SearchContext,
    search_state: &mut SearchState,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    state: &mut BoardState,
    depth: Hueristic,
    remaining_depth: usize,
    color: Hueristic,
    mut alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic
where
    T: StaticSearchTerminator,
{
    let active_god = match state.current_player {
        Player::One => p1_god,
        Player::Two => p2_god,
    };

    if let Some(winner) = state.get_winner() {
        search_state.nodes_visited += 1;
        return if winner == state.current_player {
            WINNING_SCORE - depth
        } else {
            -(WINNING_SCORE - depth)
        };
    } else if remaining_depth == 0 {
        return _q_extend(state, search_state, p1_god, p2_god, color, depth, 0);
    } else {
        search_state.nodes_visited += 1;
    }

    // Old: check if we have a win to quit early.
    // This got replaced with short circuiting move gen once we spot a win, and checking for it
    // if (active_god.has_win)(state, state.current_player) {
    //     let score = WINNING_SCORE - depth - 1;

    //     if depth == 0 {
    //         let children = (active_god.next_states)(state, state.current_player);
    //         for child in children.into_iter().rev() {
    //             if let Some(winner) = child.get_winner() {
    //                 if winner == state.current_player {
    //                     let new_best_move = NewBestMove::new(
    //                         FullGameState::new(child, p1_god, p2_god),
    //                         score,
    //                         remaining_depth,
    //                         BestMoveTrigger::EndOfLine,
    //                     );
    //                     search_state.best_move = Some(new_best_move.clone());
    //                     (search_state.new_best_move_callback)(new_best_move);
    //                     return score;
    //                 }
    //             }
    //         }

    //         let full_state = FullGameState::new(state.clone(), p1_god, p2_god);
    //         panic!(
    //             "Was promised an immediate win but didn't find it? {:?}",
    //             full_state
    //         );
    //     }

    //     return score;
    // }

    let mut track_used = false;
    let mut track_unused = false;
    let tt_entry = search_context.tt.fetch(state);
    if let Some(tt_value) = tt_entry {
        if tt_value.search_depth >= remaining_depth as u8 {
            if TranspositionTable::IS_TRACKING_STATS {
                track_used = true;
            }

            match tt_value.score_type {
                SearchScoreType::Exact => {
                    return tt_value.score;
                }
                SearchScoreType::LowerBound => {
                    if tt_value.score >= beta {
                        return tt_value.score;
                    }
                }
                SearchScoreType::UpperBound => {
                    if tt_value.score <= alpha {
                        return tt_value.score;
                    }
                }
            }
        } else if TranspositionTable::IS_TRACKING_STATS {
            track_unused = true;
        }
    }

    let alpha_orig = alpha;

    // let mut children = (active_god.next_states)(state, state.current_player);
    let mut child_moves = (active_god.get_moves)(state, state.current_player);
    if child_moves.len() == 0 {
        let score = WINNING_SCORE - depth - 1;
        return -score;
    }

    // get_moves stops accululating once it sees a win, so if there is a win it'll be last
    if is_move_winning(child_moves[child_moves.len() - 1]) {
        let score = WINNING_SCORE - depth - 1;
        if depth == 0 {
            let mut winning_board = state.clone();
            (active_god.make_move)(&mut winning_board, child_moves[child_moves.len() - 1]);

            let new_best_move = NewBestMove::new(
                FullGameState::new(winning_board, p1_god, p2_god),
                score,
                remaining_depth,
                BestMoveTrigger::EndOfLine,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
        }

        return score;
    }

    // let baseline_score = (active_god.player_advantage_fn)(&state, state.current_player);
    if let Some(tt_value) = tt_entry {
        for i in 0..child_moves.len() {
            if child_moves[i] == tt_value.best_action {
                mortal_add_score_to_move(&mut child_moves[i], u8::MAX);
                break;
            }
        }
    }

    if track_used {
        search_context.tt.stats.used_value += 1;
    } else if track_unused {
        search_context.tt.stats.unused_value += 1;
    }

    let mut best_action = child_moves[0];
    let mut best_score = Hueristic::MIN;

    let mut child_action_index = 0;
    while child_action_index < child_moves.len() {
        _select_next_action(&mut child_moves, child_action_index);
        let child_action = child_moves[child_action_index];
        child_action_index += 1;

        (active_god.make_move)(state, child_action);

        let score = -_inner_search::<T>(
            search_context,
            search_state,
            p1_god,
            p2_god,
            state,
            depth + 1,
            remaining_depth - 1,
            -color,
            -beta,
            -alpha,
        );

        let should_stop = search_context.should_stop() || T::should_stop(&search_state);

        if score > best_score {
            best_score = score;
            best_action = child_action;

            if depth == 0 && !should_stop {
                let new_best_move = NewBestMove::new(
                    FullGameState::new(state.clone(), p1_god, p2_god),
                    score,
                    remaining_depth,
                    BestMoveTrigger::Improvement,
                );
                search_state.best_move = Some(new_best_move.clone());
                (search_context.new_best_move_callback)(new_best_move);
            }

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    (active_god.unmake_move)(state, child_action);
                    break;
                }
            }
        }

        if should_stop {
            (active_god.unmake_move)(state, child_action);
            break;
        }

        (active_god.unmake_move)(state, child_action);
    }

    if !(search_context.should_stop() || T::should_stop(&search_state)) {
        let tt_score_type = if best_score <= alpha_orig {
            SearchScoreType::UpperBound
        } else if best_score >= beta {
            SearchScoreType::LowerBound
        } else {
            SearchScoreType::Exact
        };

        // Early on in the game, add all permutations of a board state to the TT, to help
        // deduplicate identical searches
        // TODO: bring this back??
        // if state.height_map[0].0.count_ones() <= 3 {
        //     for (base, child) in get_all_permutations_for_pair(state, &best_board) {
        //         let tt_value = TTValue {
        //             best_action: best_action,
        //             search_depth: remaining_depth as u8,
        //             score_type: tt_score_type,
        //             score: best_score,
        //         };

        //         search_context.tt.insert(&base, tt_value);
        //     }
        let tt_value = TTValue {
            best_action: best_action,
            search_depth: remaining_depth as u8,
            score_type: tt_score_type,
            score: best_score,
        };

        search_context.tt.insert(state, tt_value);
    }

    if depth == 0 {
        search_state.last_fully_completed_depth = remaining_depth;
    }

    best_score
}
