use std::{
    marker::PhantomData,
    sync::{Arc, atomic::AtomicBool},
};

use serde::{Deserialize, Serialize};

use crate::{
    board::FullGameState,
    gods::{
        GodPower,
        generic::{GenericMove, TT_MATCH_SCORE},
    },
    nnue::{self, LabeledAccumulator},
    player::Player,
    search,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BestMoveTrigger {
    StopFlag,
    EndOfLine,
    Improvement,
    Saved,
}

#[derive(Clone, Debug)]
pub struct BestSearchResult {
    pub child_state: FullGameState,
    pub score: Hueristic,
    pub depth: usize,
    pub nodes_visited: usize,
    pub trigger: BestMoveTrigger,
}

impl BestSearchResult {
    pub fn new(
        state: FullGameState,
        score: Hueristic,
        depth: usize,
        nodes_visited: usize,
        trigger: BestMoveTrigger,
    ) -> Self {
        BestSearchResult {
            child_state: state,
            score,
            depth,
            nodes_visited,
            trigger,
        }
    }
}

pub struct SearchContext<'a> {
    pub tt: &'a mut TranspositionTable,
    pub stop_flag: Arc<AtomicBool>,
    pub new_best_move_callback: Box<dyn FnMut(BestSearchResult)>,
}

#[derive(Debug, Clone)]
pub struct SearchState {
    pub last_fully_completed_depth: usize,
    pub best_move: Option<BestSearchResult>,
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
        let new_best_move_callback = Box::new(|_new_best_move: BestSearchResult| {
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
    let mut root_board = root_state.board.clone();
    let mut search_state = SearchState::default();

    if root_board.get_winner().is_some() {
        panic!("Should not search in an already terminal state");
    }

    let starting_depth = {
        if let Some(tt_entry) = search_context.tt.fetch(&root_state.board) {
            let mut best_child_state = root_board.clone();
            let active_god = root_state.get_active_god();
            active_god.make_move(&mut best_child_state, tt_entry.best_action);

            let new_best_move = BestSearchResult::new(
                FullGameState::new(best_child_state, root_state.gods[0], root_state.gods[1]),
                tt_entry.score,
                tt_entry.search_depth as usize,
                0,
                BestMoveTrigger::Saved,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
            tt_entry.search_depth + 1
        } else {
            3
        }
    } as usize;

    let mut nnue_acc = LabeledAccumulator::new_from_scratch(&root_board);

    for depth in starting_depth.. {
        if search_context.should_stop() || T::should_stop(&search_state) {
            if let Some(best_move) = &mut search_state.best_move {
                best_move.trigger = BestMoveTrigger::StopFlag;
                (search_context.new_best_move_callback)(best_move.clone());
            }
            break;
        }

        let score = _inner_search::<T>(
            search_context,
            &mut search_state,
            &mut nnue_acc,
            root_state.gods[0],
            root_state.gods[1],
            &mut root_board,
            0,
            depth,
            Hueristic::MIN + 1,
            Hueristic::MAX,
        );

        if search_state.best_move.is_none()
            && !(search_context.should_stop() || T::should_stop(&search_state))
        {
            // We didn't find _any_ move. Could be:
            // 1. There's a bug
            // 2. We got smothered.
            // This is rare enough to bother doing a full check for
            let active_god = root_state.get_active_god();
            let moves = (active_god.get_moves)(&root_board, root_board.current_player);

            if moves.len() > 0 {
                panic!(
                    "Moves were available, but didn't make any: {:?}",
                    root_board
                );
            }

            // There's actually no moves to make. Report the loss
            let mut losing_board = root_state.clone();
            losing_board.board.set_winner(!root_board.current_player);

            let empty_losing_move = BestSearchResult::new(
                losing_board,
                -WINNING_SCORE,
                0,
                0,
                BestMoveTrigger::EndOfLine,
            );
            search_state.best_move = Some(empty_losing_move.clone());
            (search_context.new_best_move_callback)(empty_losing_move.clone());
            break;
        }

        if score.abs() > WINNING_SCORE_BUFFER
            && !(search_context.should_stop() || T::should_stop(&search_state))
        {
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
    nnue_acc: &mut LabeledAccumulator,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    depth: Hueristic,
    q_depth: u32,
    mut alpha: Hueristic,
    beta: Hueristic,
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
        nnue_acc.replace_from_board(state);
        return nnue_acc.evaluate(state.current_player);
    }

    // Opponent is threatening a win right now. Keep looking to confirm if we can block it
    let mut best_score = WINNING_SCORE - depth - 1;
    let child_moves = (active_god.get_moves)(state, state.current_player);
    // Go back to front because wins will be last
    // TODO: should we do full sorting here?
    for child_move in child_moves.iter().rev() {
        active_god.make_move(state, *child_move);

        let score = -_q_extend(
            state,
            search_state,
            nnue_acc,
            p1_god,
            p2_god,
            depth + 1,
            q_depth + 1,
            -beta,
            -alpha,
        );
        if score > best_score {
            best_score = score;

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    active_god.unmake_move(state, *child_move);
                    break;
                }
            }
        }

        active_god.unmake_move(state, *child_move);
    }

    best_score
}

fn _select_next_action(actions: &mut Vec<GenericMove>, start_index: usize) {
    let mut best_index = start_index;
    let mut best_score = actions[start_index].get_score();
    let mut i = start_index + 1;
    while i < actions.len() {
        let score = actions[i].get_score();
        if score > best_score {
            best_score = score;
            best_index = i;
        }

        i += 1
    }

    if best_index != start_index {
        actions.swap(start_index, best_index);
    }
}

fn _inner_search<T>(
    search_context: &mut SearchContext,
    search_state: &mut SearchState,
    nnue_acc: &mut LabeledAccumulator,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    state: &mut BoardState,
    depth: Hueristic,
    remaining_depth: usize,
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
        return _q_extend(
            state,
            search_state,
            nnue_acc,
            p1_god,
            p2_god,
            depth,
            0,
            alpha,
            beta,
        );
    } else {
        search_state.nodes_visited += 1;
    }

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
        // TODO: need to do something smarter about losing on smothering
        let score = WINNING_SCORE - depth - 1;
        return -score;
    }

    // get_moves stops running once it sees a win, so if there is a win it'll be last
    if child_moves[child_moves.len() - 1].get_is_winning() {
        let score = WINNING_SCORE - depth - 1;
        if depth == 0 {
            let mut winning_board = state.clone();
            active_god.make_move(&mut winning_board, child_moves[child_moves.len() - 1]);
            assert!(winning_board.get_winner() == Some(state.current_player));

            let new_best_move = BestSearchResult::new(
                FullGameState::new(winning_board, p1_god, p2_god),
                score,
                remaining_depth,
                search_state.nodes_visited,
                BestMoveTrigger::EndOfLine,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
        }

        return score;
    }

    if let Some(tt_value) = tt_entry {
        for i in 0..child_moves.len() {
            if child_moves[i] == tt_value.best_action {
                child_moves[i].set_score(TT_MATCH_SCORE);
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

    nnue_acc.replace_from_board(state);
    let mut child_nnue_acc = nnue_acc.clone();

    let mut child_action_index = 0;
    while child_action_index < child_moves.len() {
        _select_next_action(&mut child_moves, child_action_index);
        let child_action = child_moves[child_action_index];
        child_action_index += 1;

        active_god.make_move(state, child_action);

        let score = -_inner_search::<T>(
            search_context,
            search_state,
            &mut child_nnue_acc,
            p1_god,
            p2_god,
            state,
            depth + 1,
            remaining_depth - 1,
            -beta,
            -alpha,
        );

        let should_stop = search_context.should_stop() || T::should_stop(&search_state);

        if score > best_score {
            best_score = score;
            best_action = child_action;

            if depth == 0 && !should_stop {
                let new_best_move = BestSearchResult::new(
                    FullGameState::new(state.clone(), p1_god, p2_god),
                    score,
                    remaining_depth,
                    search_state.nodes_visited,
                    BestMoveTrigger::Improvement,
                );
                search_state.best_move = Some(new_best_move.clone());
                (search_context.new_best_move_callback)(new_best_move);
            }

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    active_god.unmake_move(state, child_action);
                    break;
                }
            }
        }

        if should_stop {
            active_god.unmake_move(state, child_action);
            break;
        }

        active_god.unmake_move(state, child_action);
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

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;

    #[test]
    fn test_tt_lookup_pv_regression() {
        // Guard against this regression scenario:
        // 1. P1 performs move ordering, and puts a bad move up first that gets mated on the spot. Because this is the first searched move, this becomes the PV.
        // 2. Later in the move ordering, a better move is found to become the PV.
        // 3. We get to a further search depth
        // 4. TT lookup fails for some reason, and we search the bad move from 1 again, and go back
        //    to thinking we're losing temporarily
        let state_string = "0000001440001220222204421/2/mortal:13,18/mortal:14,17";
        let full_state = FullGameState::try_from(state_string).unwrap();
        let orig_loss_counter = Rc::new(RefCell::new(0));
        let loss_counter = orig_loss_counter.clone();
        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            stop_flag: Arc::new(AtomicBool::new(false)),
            new_best_move_callback: Box::new(move |new_best_move| {
                if new_best_move.score < -WINNING_SCORE_BUFFER {
                    // increment loss counter
                    *loss_counter.borrow_mut() += 1;
                }
            }),
        };

        let search_state = search_with_state::<MaxDepthStaticSearchTerminator<5>>(
            &mut search_context,
            &full_state,
        );

        let best_move = search_state.best_move.unwrap();
        assert!(best_move.score > -WINNING_SCORE_BUFFER);
        assert!(orig_loss_counter.borrow().clone() <= 1);
    }
}
