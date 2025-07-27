use std::{
    array,
    fmt::Debug,
    sync::{Arc, atomic::AtomicBool},
};

use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};

use crate::{
    bitboard::BitBoard,
    board::FullGameState,
    gods::{GodPower, generic::GenericMove},
    move_picker::{MovePicker, MovePickerStage},
    nnue::LabeledAccumulator,
    player::Player,
    search_terminators::SearchTerminator,
    transposition_table::SearchScoreType,
};

use super::{board::BoardState, transposition_table::TranspositionTable};

pub const MAX_PLY: usize = 127;

pub type Hueristic = i32;
pub const WINNING_SCORE: Hueristic = 10_000;
pub const WINNING_SCORE_BUFFER: Hueristic = 9000;
pub static mut NUM_SEARCHES: usize = 0;

pub const fn win_at_depth(depth: usize) -> Hueristic {
    WINNING_SCORE - depth as Hueristic
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
    pub action: GenericMove,
    pub action_str: String,
    pub score: Hueristic,
    pub depth: usize,
    pub nodes_visited: usize,
    pub trigger: BestMoveTrigger,
}

impl BestSearchResult {
    pub fn new(
        state: FullGameState,
        action: GenericMove,
        score: Hueristic,
        depth: usize,
        nodes_visited: usize,
        trigger: BestMoveTrigger,
    ) -> Self {
        let action_str = state.get_other_god().stringify_move(action);
        BestSearchResult {
            child_state: state,
            action,
            action_str,
            score,
            depth,
            nodes_visited,
            trigger,
        }
    }
}

pub struct SearchContext<'a, T: SearchTerminator> {
    pub tt: &'a mut TranspositionTable,
    pub stop_flag: Arc<AtomicBool>,
    pub new_best_move_callback: Box<dyn FnMut(BestSearchResult)>,
    pub terminator: T,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStackEntry {
    pub eval: Hueristic,
    pub is_null_move: bool,
}

// Examples only store pv for reporting. I'm much less interested now
#[derive(Clone, Debug)]
pub struct PVariation {
    pub moves: ArrayVec<GenericMove, MAX_PLY>,
}

impl PVariation {
    const EMPTY: Self = Self {
        moves: ArrayVec::new_const(),
    };

    pub fn moves(&self) -> &[GenericMove] {
        &self.moves
    }

    pub const fn default_const() -> Self {
        Self::EMPTY
    }

    pub fn load_from(&mut self, m: GenericMove, rest: &Self) {
        self.moves.clear();
        self.moves.push(m);
        self.moves
            .try_extend_from_slice(&rest.moves)
            .expect("attempted to construct a PV longer than MAX_PLY.");
    }
}

pub trait NodeType {
    const PV: bool;
    const ROOT: bool;
    type Next: NodeType;
}

struct Root;
struct OnPV;
struct OffPV;
// struct CheckForced;

impl NodeType for Root {
    const PV: bool = true;
    const ROOT: bool = true;
    type Next = OnPV;
}
impl NodeType for OnPV {
    const PV: bool = true;
    const ROOT: bool = false;
    type Next = Self;
}
impl NodeType for OffPV {
    const PV: bool = false;
    const ROOT: bool = false;
    type Next = Self;
}
// impl NodeType for CheckForced {
//     const PV: bool = false;
//     const ROOT: bool = true;
//     type Next = OffPV;
// }

#[derive(Clone)]
pub struct SearchState {
    pub last_fully_completed_depth: usize,
    pub best_move: Option<BestSearchResult>,
    pub nodes_visited: usize,
    pub killer_move_table: [Option<GenericMove>; MAX_PLY],
    pub search_stack: [SearchStackEntry; MAX_PLY],
    // pub max_q_depth: u32,
}

impl Debug for SearchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchState")
            .field(
                "last_fully_completed_depth",
                &self.last_fully_completed_depth,
            )
            .field("best_move", &self.best_move)
            .field("nodes_visited", &self.nodes_visited)
            // .field("killer_move_table", &self.killer_move_table)
            // .field("search_stack", &self.search_stack)
            .finish()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            last_fully_completed_depth: 0,
            best_move: None,
            nodes_visited: 0,
            killer_move_table: [None; MAX_PLY],
            search_stack: array::from_fn(|_| SearchStackEntry::default()),
            // max_q_depth: 5,
        }
    }
}

impl<'a, T: SearchTerminator> SearchContext<'a, T> {
    pub fn should_stop(&self, state: &SearchState) -> bool {
        self.stop_flag.load(std::sync::atomic::Ordering::Relaxed)
            || self.terminator.should_stop(state)
    }

    pub fn new(tt: &'a mut TranspositionTable, terminator: T) -> Self {
        let new_best_move_callback = Box::new(|_new_best_move: BestSearchResult| {
            // eprintln!("{:?}", _new_best_move);
        });

        SearchContext {
            tt,
            new_best_move_callback,
            stop_flag: Arc::new(AtomicBool::new(false)),
            terminator,
        }
    }
}

pub fn negamax_search<T>(
    search_context: &mut SearchContext<T>,
    root_state: &FullGameState,
) -> SearchState
where
    T: SearchTerminator,
{
    search_context
        .tt
        .age(root_state.gods[0].god_name, root_state.gods[1].god_name);

    let mut root_board = root_state.board.clone();
    let mut search_state = SearchState::default();

    if root_board.get_winner().is_some() {
        panic!("Should not search in an already terminal state");
    }

    let starting_depth = {
        if let Some(tt_entry) = search_context.tt.fetch(&root_state.board, 0)
            && tt_entry.best_action != GenericMove::NULL_MOVE
        {
            let mut best_child_state = root_board.clone();
            let active_god = root_state.get_active_god();
            active_god.make_move(&mut best_child_state, tt_entry.best_action);

            let new_best_move = BestSearchResult::new(
                FullGameState::new(best_child_state, root_state.gods[0], root_state.gods[1]),
                tt_entry.best_action,
                tt_entry.score,
                tt_entry.search_depth as usize,
                0,
                BestMoveTrigger::Saved,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
            // tt_entry.search_depth + 1
            2
        } else {
            2
        }
    } as usize;

    let mut nnue_acc = LabeledAccumulator::new_from_scratch(&root_board);

    let is_in_check = root_state
        .get_other_god()
        .get_winning_moves(&root_board, !root_board.current_player)
        .len()
        > 0;

    for depth in starting_depth.. {
        if search_context.should_stop(&search_state) {
            if let Some(best_move) = &mut search_state.best_move {
                best_move.trigger = BestMoveTrigger::StopFlag;
                (search_context.new_best_move_callback)(best_move.clone());
            }
            break;
        }

        let score = _inner_search::<T, Root>(
            search_context,
            &mut search_state,
            &mut nnue_acc,
            root_state.gods[0],
            root_state.gods[1],
            &mut root_board,
            is_in_check,
            0,
            depth,
            Hueristic::MIN + 1,
            Hueristic::MAX,
        );

        search_state.last_fully_completed_depth = depth;

        if search_state.best_move.is_none() && !search_context.should_stop(&search_state) {
            // We didn't find _any_ move. Could be:
            // 1. There's a bug
            // 2. We got smothered.
            // This is rare enough to bother doing a full check for
            let active_god = root_state.get_active_god();
            let moves = active_god.get_moves_for_search(&root_board, root_board.current_player);

            if moves.len() > 0 {
                root_board.print_to_console();
                panic!(
                    "Moves were available, but didn't make any: {:?}, {:?}. {:?}",
                    root_board, moves, search_state
                );
            }

            // There's actually no moves to make. Report the loss
            let mut losing_board = root_state.clone();
            losing_board.board.set_winner(!root_board.current_player);

            let empty_losing_move = BestSearchResult::new(
                losing_board,
                GenericMove::NULL_MOVE,
                -WINNING_SCORE,
                0,
                0,
                BestMoveTrigger::EndOfLine,
            );
            search_state.best_move = Some(empty_losing_move.clone());
            (search_context.new_best_move_callback)(empty_losing_move.clone());
            break;
        }

        if score.abs() > WINNING_SCORE_BUFFER && !search_context.should_stop(&search_state) {
            let mut best_move = search_state.best_move.clone().unwrap();
            best_move.trigger = BestMoveTrigger::EndOfLine;
            (search_context.new_best_move_callback)(best_move);
            break;
        }
    }

    search_state
}

fn _q_extend<T>(
    search_context: &mut SearchContext<T>,
    state: &mut BoardState,
    search_state: &mut SearchState,
    nnue_acc: &mut LabeledAccumulator,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    depth: usize,
    q_depth: u32,
    mut alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic
where
    T: SearchTerminator,
{
    search_state.nodes_visited += 1;

    // if q_depth > search_state.max_q_depth {
    //     search_state.max_q_depth = q_depth;
    //     eprintln!("New max q depth: {}", q_depth);
    // }

    let (active_god, other_god) = match state.current_player {
        Player::One => (p1_god, p2_god),
        Player::Two => (p2_god, p1_god),
    };

    // If we have a win right now, just take it
    if active_god
        .get_winning_moves(state, state.current_player)
        .len()
        > 0
    {
        let score = win_at_depth(depth) - 1;
        return score;
    }

    let eval;
    let child_moves;
    let opponent_wins = other_god.get_winning_moves(state, !state.current_player);

    // If opponent is threatening a win, we must respond to it. Don't bother taking the current
    // eval, just know that we're losing
    if opponent_wins.len() > 0 {
        eval = -(WINNING_SCORE - 1);
        let mut blocker_board = BitBoard::EMPTY;
        for action in &opponent_wins {
            blocker_board |= other_god.get_blocker_board(action.action);
        }
        child_moves = active_god.get_blocker_moves(state, state.current_player, blocker_board);
    } else {
        // If qs is going on for too long, just return the current eval
        nnue_acc.replace_from_board(state);
        eval = nnue_acc.evaluate();

        // TODO: test this
        if q_depth > 2 {
            return eval;
        }

        child_moves = active_god.get_improver_moves(state, state.current_player);
    }

    // check standing pat
    if eval >= beta {
        return beta;
    }
    alpha = alpha.max(eval);

    let mut best_score = eval;

    for child_move in child_moves.iter().rev() {
        active_god.make_move(state, child_move.action);

        let score = -_q_extend(
            search_context,
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
                    active_god.unmake_move(state, child_move.action);
                    break;
                }
            }
        }

        active_god.unmake_move(state, child_move.action);

        if search_context.should_stop(&search_state) {
            break;
        }
    }

    best_score
}

fn _inner_search<T, NT>(
    search_context: &mut SearchContext<T>,
    search_state: &mut SearchState,
    nnue_acc: &mut LabeledAccumulator,
    p1_god: &'static GodPower,
    p2_god: &'static GodPower,
    state: &mut BoardState,
    is_in_check: bool,
    ply: usize,
    mut remaining_depth: usize,
    mut alpha: Hueristic,
    mut beta: Hueristic,
) -> Hueristic
where
    T: SearchTerminator,
    NT: NodeType,
{
    let active_god = match state.current_player {
        Player::One => p1_god,
        Player::Two => p2_god,
    };

    if !NT::ROOT
        && let Some(winner) = state.get_winner()
    {
        search_state.nodes_visited += 1;
        return if winner == state.current_player {
            win_at_depth(ply)
        } else {
            -win_at_depth(ply)
        };
    } else if remaining_depth == 0 {
        return _q_extend(
            search_context,
            state,
            search_state,
            nnue_acc,
            p1_god,
            p2_god,
            ply,
            0,
            alpha,
            beta,
        );
    } else if !NT::ROOT {
        search_state.nodes_visited += 1;

        // Worst possible outcome is losing right now (due to a smother)
        // Best possible outcome is winning right now
        alpha = alpha.max(-win_at_depth(ply));
        beta = beta.min(win_at_depth(ply));
        if alpha >= beta {
            return alpha;
        }
    } else {
        search_state.nodes_visited += 1;
    }

    let mut track_used = false;
    let mut track_unused = false;
    let tt_entry = search_context.tt.fetch(state, ply);
    if let Some(tt_value) = &tt_entry {
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

    // internal iterative reduction
    // reduce repth on a tt miss
    // my variant: exclude PV lines from this rule
    if !NT::ROOT && !NT::PV && remaining_depth >= 4 && tt_entry.is_none() {
        remaining_depth -= 1;
    }

    let mut move_picker = MovePicker::new(
        state.current_player,
        active_god,
        tt_entry.as_ref().map(|e| e.best_action),
        search_state.killer_move_table[ply as usize],
    );

    if !move_picker.has_any_moves(&state) {
        // TODO: need to do something smarter about losing on smothering
        let score = win_at_depth(ply) - 1;
        return -score;
    }

    if let Some(winning_action) = move_picker.get_winning_move(&state) {
        let score = win_at_depth(ply) - 1;
        if NT::ROOT {
            let mut winning_board = state.clone();
            active_god.make_move(&mut winning_board, winning_action);
            assert!(winning_board.get_winner() == Some(state.current_player));

            let new_best_move = BestSearchResult::new(
                FullGameState::new(winning_board, p1_god, p2_god),
                winning_action,
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

    let eval = if let Some(tt_value) = &tt_entry {
        tt_value.eval
    } else {
        nnue_acc.replace_from_board(state);
        nnue_acc.evaluate()
    };

    let ss = &mut search_state.search_stack;
    ss[ply].eval = eval;

    let improving = if ply >= 2 {
        eval > ss[ply - 2].eval
    } else if ply >= 4 {
        eval > ss[ply - 4].eval
    } else {
        true
    };

    let mut child_nnue_acc = nnue_acc.clone();
    if !NT::ROOT && !NT::PV && !is_in_check {
        // Reverse Futility Pruning
        if remaining_depth <= 8 {
            let rfp_margin =
                150 + 100 * remaining_depth as Hueristic - (improving as Hueristic) * 80;
            if eval - rfp_margin >= beta {
                return beta;
            }
        }

        // Null move pruning
        if remaining_depth > 3
            && eval + 45 * (improving as Hueristic) >= beta
            && !ss[ply - 1].is_null_move
        {
            let reduction = (4 + remaining_depth / 4).min(remaining_depth);

            search_state.search_stack[ply].is_null_move = true;
            state.flip_current_player();
            let null_value = -_inner_search::<T, OffPV>(
                search_context,
                search_state,
                &mut child_nnue_acc,
                p1_god,
                p2_god,
                state,
                false,
                ply + 1,
                remaining_depth - reduction,
                -beta,
                -beta + 1,
            );
            state.flip_current_player();
            search_state.search_stack[ply].is_null_move = false;

            // cutoff above beta
            if null_value >= beta {
                return beta;
            }
        }
    }

    if track_used {
        search_context.tt.stats.used_value += 1;
    } else if track_unused {
        search_context.tt.stats.unused_value += 1;
    }

    let mut best_action = GenericMove::NULL_MOVE;
    let mut best_score = Hueristic::MIN;

    let mut should_stop = false;
    let mut move_idx = 0;
    let mut best_action_idx = 0;
    while let Some(child_action) = move_picker.next(&state) {
        let child_is_check = child_action.get_is_check();
        move_idx += 1;
        active_god.make_move(state, child_action);

        // check extension
        let mut next_depth = if child_is_check {
            // eprintln!("check ext: ply {ply}");
            remaining_depth
        } else {
            remaining_depth - 1
        };

        let mut score;
        if move_idx == 1 {
            score = -_inner_search::<T, NT::Next>(
                search_context,
                search_state,
                &mut child_nnue_acc,
                p1_god,
                p2_god,
                state,
                child_is_check,
                ply + 1,
                next_depth,
                -beta,
                -alpha,
            )
        } else {
            if next_depth > 1 && move_idx >= 200 {
                next_depth -= 1;
            }

            // Stop considering non-improvers eventually
            if ply >= 2
                && remaining_depth < 6
                && move_idx > 300
                && !improving
                && move_picker.stage == MovePickerStage::YieldNonImprovers
            {
                active_god.unmake_move(state, child_action);
                break;
            }

            // Try a 0-window search
            score = -_inner_search::<T, OffPV>(
                search_context,
                search_state,
                &mut child_nnue_acc,
                p1_god,
                p2_god,
                state,
                child_is_check,
                ply + 1,
                next_depth,
                -alpha - 1,
                -alpha,
            );

            // The search failed, try again
            if score > alpha && score < beta {
                score = -_inner_search::<T, NT::Next>(
                    search_context,
                    search_state,
                    &mut child_nnue_acc,
                    p1_god,
                    p2_god,
                    state,
                    child_is_check,
                    ply + 1,
                    next_depth,
                    -beta,
                    -alpha,
                )
            }
        };

        should_stop = search_context.should_stop(&search_state);

        if score > best_score {
            best_score = score;
            best_action = child_action;
            best_action_idx = move_idx - 1;

            // if move_idx > 1000 {
            //     eprintln!("{move_idx}: {}", active_god.stringify_move(child_action));
            //     state.print_to_console();
            // }

            if NT::ROOT && !should_stop {
                let new_best_move = BestSearchResult::new(
                    FullGameState::new(state.clone(), p1_god, p2_god),
                    best_action,
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

        active_god.unmake_move(state, child_action);

        if should_stop {
            break;
        }
    }

    if !should_stop {
        let tt_score_type = if best_score <= alpha_orig {
            SearchScoreType::UpperBound
        } else if best_score >= beta {
            SearchScoreType::LowerBound
        } else {
            SearchScoreType::Exact
        };

        if alpha != alpha_orig && best_action_idx > 1 {
            search_state.killer_move_table[ply as usize] = Some(best_action);
        }

        // Early on in the game, add all permutations of a board state to the TT, to help
        // deduplicate identical searches
        if state.height_map[0].count_ones() <= 1 {
            for base in state.get_all_permutations::<false>() {
                search_context.tt.conditionally_insert(
                    &base,
                    GenericMove::NULL_MOVE,
                    remaining_depth as u8,
                    tt_score_type,
                    best_score,
                    eval,
                    ply,
                );
            }
        }

        search_context.tt.insert(
            state,
            best_action,
            remaining_depth as u8,
            tt_score_type,
            best_score,
            eval,
            ply,
        );
    }

    best_score
}

#[cfg(test)]
mod tests {
    use core::panic;
    use std::{cell::RefCell, rc::Rc};

    use crate::{gods::pan, search_terminators::DynamicMaxDepthSearchTerminator};

    use super::*;

    #[test]
    fn test_tt_lookup_pv_regression() {
        // Guard against this regression scenario:
        // 1. P1 performs move ordering, and puts a bad move up first that gets mated on the spot. Because this is the first searched move, this becomes the PV.
        // 2. Later in the move ordering, a better move is found to become the PV.
        // 3. We get to a further search depth
        // 4. TT lookup fails for some reason, and we search the bad move from 1 again, and go back
        //    to thinking we're losing temporarily
        let state_string = "0000001440001220222204421/2/mortal:D2,D3/mortal:C2,E3";
        let full_state = FullGameState::try_from(state_string).unwrap();
        let orig_loss_counter = Rc::new(RefCell::new(0));
        let orig_win_since_loss_counter = Rc::new(RefCell::new(0));
        let loss_counter = orig_loss_counter.clone();
        let win_since_loss_counter = orig_win_since_loss_counter.clone();
        let mut tt = TranspositionTable::new();
        let mut search_context = SearchContext {
            tt: &mut tt,
            stop_flag: Arc::new(AtomicBool::new(false)),
            new_best_move_callback: Box::new(move |new_best_move| {
                if new_best_move.score < -WINNING_SCORE_BUFFER {
                    // increment loss counter
                    *loss_counter.borrow_mut() += 1;
                    if *win_since_loss_counter.borrow() > 0 {
                        panic!("Reverted back to loss");
                    }
                } else {
                    if *loss_counter.borrow() > 0 {
                        *win_since_loss_counter.borrow_mut() += 1;
                    }
                }
            }),
            terminator: DynamicMaxDepthSearchTerminator::new(5),
        };

        let search_state = negamax_search(&mut search_context, &full_state);

        let best_move = search_state.best_move.unwrap();
        // assert!(best_move.score > -WINNING_SCORE_BUFFER);
        // assert!(orig_loss_counter.borrow().clone() <= 1);
    }
}
