use std::{array, fmt::Debug};

use arrayvec::ArrayVec;
use serde::{Deserialize, Serialize};

use crate::{
    bitboard::BitBoard,
    board::FullGameState,
    gods::generic::{GenericMove, GodMove, KILLER_MATCH_SCORE, MoveScore, WorkerPlacement},
    move_picker::{MovePicker, MovePickerStage},
    nnue::LabeledAccumulator,
    placement::{PlacementState, get_placement_actions, get_starting_placement_state},
    search_terminators::SearchTerminator,
    transposition_table::SearchScoreType,
    utils::{hash_u64, timestamp_string},
};

use super::transposition_table::TranspositionTable;

pub const MAX_PLY: usize = 127;

pub type Hueristic = i16;
pub const WINNING_SCORE: Hueristic = 10_000;
pub const INFINITY: Hueristic = WINNING_SCORE * 2;
pub const WINNING_SCORE_BUFFER: Hueristic = 9000;

pub const fn win_at_ply(ply: usize) -> Hueristic {
    WINNING_SCORE - ply as Hueristic
}

const HALF_USIZE: u32 = size_of::<usize>() as u32 / 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BestMoveTrigger {
    StopFlag,
    EndOfLine,
    Improvement,
    Saved,
    Seed,
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
        is_placement_action: bool,
        score: Hueristic,
        depth: usize,
        nodes_visited: usize,
        trigger: BestMoveTrigger,
    ) -> Self {
        let action_str = if is_placement_action {
            let placement: WorkerPlacement = action.into();
            format!("{:?}", placement)
        } else {
            state.get_other_god().stringify_move(action)
        };

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
    pub new_best_move_callback: Box<dyn FnMut(BestSearchResult)>,
    pub terminator: T,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStackEntry {
    pub eval: Hueristic,
    pub is_null_move: bool,
    pub move_hash: usize,
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

const GLOBAL_MOVE_HISTORY_MAX: HistoryDelta = 1024;
const PER_PLY_HISTORY_MAX: HistoryDelta = 4096;
const RESPONSE_HISTORY_MAX: HistoryDelta = 8192;
const FOLLOW_HISTORY_MAX: HistoryDelta = 8192;

pub const BASE_MOVE_HISTORY_TABLE_SIZE: usize = 999_983;
pub const MOVE_HISTORY_BY_DEPTH_SIZE: usize = 100_001;
pub const MAX_MOVE_HISTORY_DEPTH: usize = 32;
pub const RESPONSE_HISTORY_SIZE: usize = 999_983;
pub const FOLLOW_HISTORY_SIZE: usize = 999_983;

type HistoryDelta = i32;

pub fn update_history_value(val: &mut MoveScore, bonus: HistoryDelta, max: HistoryDelta) {
    let current = HistoryDelta::from(*val);
    *val += bonus as MoveScore - (current * bonus.abs() / max) as MoveScore;
}

pub fn set_min_history_value(val: &mut MoveScore, new_val: HistoryDelta) {
    *val = (*val).max(new_val as MoveScore);
}

pub struct Histories {
    pub global_move_history: Vec<MoveScore>,
    pub move_history_by_ply: [Vec<MoveScore>; MAX_MOVE_HISTORY_DEPTH],
    pub response_history: Vec<MoveScore>,
    pub follow_history: Vec<MoveScore>,
}

impl Histories {
    pub fn get_move_score(
        &self,
        move_idx: usize,
        ply: usize,
        prev_move_hash: Option<usize>,
        follow_move_hash: Option<usize>,
    ) -> MoveScore {
        let mut res = 0;
        res += self.global_move_history[move_idx % BASE_MOVE_HISTORY_TABLE_SIZE];
        res += self.move_history_by_ply[Self::_move_history_ply(ply)]
            [move_idx % MOVE_HISTORY_BY_DEPTH_SIZE];

        if let Some(prev_move_idx) = prev_move_hash {
            res +=
                self.response_history[hash_u64(move_idx.rotate_left(HALF_USIZE) ^ prev_move_idx)
                    % RESPONSE_HISTORY_SIZE];
        }

        if let Some(follow_move_idx) = follow_move_hash {
            res +=
                self.follow_history[hash_u64(move_idx.rotate_left(HALF_USIZE) ^ follow_move_idx)
                    % FOLLOW_HISTORY_SIZE];
        }

        res
    }

    // If ply > our history limit, use the last entry for that player instead
    fn _move_history_ply(ply: usize) -> usize {
        if ply < MAX_MOVE_HISTORY_DEPTH {
            ply
        } else {
            MAX_MOVE_HISTORY_DEPTH - 2 + ((ply - MAX_MOVE_HISTORY_DEPTH) % 2)
        }
    }

    pub fn update_move(
        &mut self,
        move_idx: usize,
        ply: usize,
        magnitude: HistoryDelta,
        prev_move_idx: Option<usize>,
        follow_move_idx: Option<usize>,
    ) {
        update_history_value(
            &mut self.global_move_history[move_idx % BASE_MOVE_HISTORY_TABLE_SIZE],
            magnitude,
            GLOBAL_MOVE_HISTORY_MAX,
        );
        update_history_value(
            &mut self.move_history_by_ply[Self::_move_history_ply(ply)]
                [move_idx % MOVE_HISTORY_BY_DEPTH_SIZE],
            magnitude,
            PER_PLY_HISTORY_MAX,
        );

        if let Some(prev_move_idx) = prev_move_idx {
            update_history_value(
                &mut self.response_history
                    [hash_u64(move_idx.rotate_left(32) ^ prev_move_idx) % RESPONSE_HISTORY_SIZE],
                magnitude,
                RESPONSE_HISTORY_MAX,
            );
        }

        if let Some(follow_move_idx) = follow_move_idx {
            update_history_value(
                &mut self.follow_history
                    [hash_u64(move_idx.rotate_left(32) ^ follow_move_idx) % FOLLOW_HISTORY_SIZE],
                magnitude,
                FOLLOW_HISTORY_MAX,
            );
        }
    }

    pub fn set_move_min(
        &mut self,
        move_idx: usize,
        ply: usize,
        magnitude: HistoryDelta,
        prev_move_idx: Option<usize>,
        follow_move_idx: Option<usize>,
    ) {
        set_min_history_value(
            &mut self.global_move_history[move_idx % BASE_MOVE_HISTORY_TABLE_SIZE],
            magnitude,
        );
        set_min_history_value(
            &mut self.move_history_by_ply[Self::_move_history_ply(ply)]
                [move_idx % MOVE_HISTORY_BY_DEPTH_SIZE],
            magnitude,
        );

        if let Some(prev_move_idx) = prev_move_idx {
            set_min_history_value(
                &mut self.response_history
                    [hash_u64(move_idx.rotate_left(32) ^ prev_move_idx) % RESPONSE_HISTORY_SIZE],
                magnitude,
            );
        }

        if let Some(follow_move_idx) = follow_move_idx {
            set_min_history_value(
                &mut self.follow_history
                    [hash_u64(move_idx.rotate_left(32) ^ follow_move_idx) % FOLLOW_HISTORY_SIZE],
                magnitude,
            );
        }
    }
}

impl Default for Histories {
    fn default() -> Self {
        Self {
            global_move_history: vec![0; BASE_MOVE_HISTORY_TABLE_SIZE],
            move_history_by_ply: array::from_fn(|_| vec![0; MOVE_HISTORY_BY_DEPTH_SIZE]),
            response_history: vec![0; RESPONSE_HISTORY_SIZE],
            follow_history: vec![0; RESPONSE_HISTORY_SIZE],
        }
    }
}

pub struct SearchState {
    pub last_fully_completed_depth: usize,
    pub best_move: Option<BestSearchResult>,
    pub nodes_visited: usize,
    pub killer_move_table: [Option<GenericMove>; MAX_PLY],
    pub search_stack: [SearchStackEntry; MAX_PLY],
    pub history: [Histories; 2],
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
            search_stack: array::from_fn(|_| Default::default()),
            history: Default::default(),
        }
    }
}

impl<'a, T: SearchTerminator> SearchContext<'a, T> {
    pub fn should_stop(&mut self, state: &SearchState) -> bool {
        self.terminator.should_stop(state)
    }

    pub fn new(tt: &'a mut TranspositionTable, terminator: T) -> Self {
        let new_best_move_callback = Box::new(|_new_best_move: BestSearchResult| {
            // eprintln!("{:?}", _new_best_move);
        });

        SearchContext {
            tt,
            new_best_move_callback,
            terminator,
        }
    }
}

pub fn negamax_search<T>(
    search_context: &mut SearchContext<T>,
    mut root_state: FullGameState,
) -> SearchState
where
    T: SearchTerminator,
{
    let mut search_state = SearchState::default();

    root_state.validate();
    if root_state.get_winner().is_some() {
        panic!(
            "Should not search in an already terminal state: {:?}",
            root_state
        );
    }

    let starting_mode = get_starting_placement_state(&root_state.board, root_state.gods).unwrap();

    if let Some(tt_entry) = search_context.tt.fetch(&root_state, 0)
        && tt_entry.best_action != GenericMove::NULL_MOVE
    {
        let mut best_child_state = root_state.clone();

        if let Some(starting_mode) = starting_mode {
            let placement: WorkerPlacement = tt_entry.best_action.into();
            placement.make_move(&mut best_child_state.board, starting_mode.next_placement);
        } else {
            let active_god = root_state.get_active_god();
            active_god.make_move(&mut best_child_state.board, tt_entry.best_action);
        }

        let new_best_move = BestSearchResult::new(
            best_child_state,
            tt_entry.best_action,
            starting_mode.is_some(),
            tt_entry.score,
            tt_entry.search_depth as usize,
            0,
            BestMoveTrigger::Saved,
        );
        search_state.best_move = Some(new_best_move.clone());
        (search_context.new_best_move_callback)(new_best_move);
    } else {
        // Pick a random move to start with, to make sure we don't fail to find any move
        let all_next_states = root_state.get_all_next_states_with_actions();

        if let Some((next_state, next_action)) = all_next_states.last() {
            let new_best_move = BestSearchResult::new(
                next_state.clone(),
                next_action.clone(),
                starting_mode.is_some(),
                -INFINITY,
                0,
                0,
                BestMoveTrigger::Seed,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
        }
    }

    let start_depth = starting_mode.is_none() as usize;

    let mut nnue_acc = LabeledAccumulator::new_from_scratch(
        &root_state.board,
        root_state.gods[0].model_god_name,
        root_state.gods[1].model_god_name,
    );

    for depth in start_depth.. {
        if search_context.should_stop(&search_state) {
            if let Some(best_move) = &mut search_state.best_move {
                best_move.trigger = BestMoveTrigger::StopFlag;
                (search_context.new_best_move_callback)(best_move.clone());
            }
            break;
        }

        let score = _root_search(
            search_context,
            &mut search_state,
            &mut root_state,
            &mut nnue_acc,
            depth,
        );

        search_state.last_fully_completed_depth = depth;

        if search_state.best_move.is_none() && !search_context.should_stop(&search_state) {
            // We didn't find _any_ move. Could be:
            // 1. We got smothered.
            // 2. There's a bug
            // This is rare & cheap enough to do a full check for, instead of assuming the smother
            let active_god = root_state.get_active_god();
            let moves =
                active_god.get_moves_for_search(&root_state, root_state.board.current_player);

            if moves.len() > 0 {
                root_state.print_to_console();
                panic!(
                    "{} Moves were available, but didn't make any: depth: {}, {:?}, {:?}. {:?}",
                    timestamp_string(),
                    depth,
                    root_state.board,
                    moves,
                    search_state
                );
            }

            // There's actually no moves to make. Report the loss
            let mut losing_board = root_state.clone();
            losing_board
                .board
                .set_winner(!root_state.board.current_player);

            let empty_losing_move = BestSearchResult::new(
                losing_board,
                GenericMove::NULL_MOVE,
                false,
                -win_at_ply(0),
                0,
                0,
                BestMoveTrigger::EndOfLine,
            );
            search_state.best_move = Some(empty_losing_move.clone());
            (search_context.new_best_move_callback)(empty_losing_move.clone());
            break;
        }

        if score.abs() > WINNING_SCORE_BUFFER && !search_context.should_stop(&search_state) {
            // If we see a win/loss, maybe the refutation was pruned out. Keep searching a bit further
            // to confirm, but there's no need to search forever
            let win_depth = WINNING_SCORE - score.abs();
            if depth as Hueristic > 1 * (win_depth + 1) {
                let mut best_move = search_state.best_move.clone().unwrap();
                best_move.trigger = BestMoveTrigger::EndOfLine;
                (search_context.new_best_move_callback)(best_move);
                break;
            }
        }
    }

    search_state
}

fn _root_search<T>(
    search_context: &mut SearchContext<T>,
    search_state: &mut SearchState,
    state: &FullGameState,
    nnue_acc: &mut LabeledAccumulator,
    remaining_depth: usize,
) -> Hueristic
where
    T: SearchTerminator,
{
    if let Some(starting_mode) = get_starting_placement_state(&state.board, state.gods).unwrap() {
        _placement_search::<T, Root>(
            search_context,
            search_state,
            state,
            nnue_acc,
            starting_mode,
            0,
            remaining_depth,
            -INFINITY,
            INFINITY,
        )
    } else {
        _start_inner_search::<T, Root>(
            search_context,
            search_state,
            state,
            nnue_acc,
            0,
            remaining_depth,
            -INFINITY,
            INFINITY,
        )
    }
}

fn _start_inner_search<T, NT>(
    search_context: &mut SearchContext<T>,
    search_state: &mut SearchState,
    state: &FullGameState,
    nnue_acc: &mut LabeledAccumulator,
    ply: usize,
    remaining_depth: usize,
    alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic
where
    T: SearchTerminator,
    NT: NodeType,
{
    let (_active_god, other_god) = state.get_active_non_active_gods();

    let is_in_check = other_god
        .get_winning_moves(&state, !state.board.current_player)
        .len()
        > 0;

    _inner_search::<T, NT>(
        search_context,
        search_state,
        state,
        nnue_acc,
        is_in_check,
        ply,
        0,
        remaining_depth as i32,
        alpha,
        beta,
        false,
    )
}

fn _placement_search<T, NT>(
    search_context: &mut SearchContext<T>,
    search_state: &mut SearchState,
    state: &FullGameState,
    nnue_acc: &mut LabeledAccumulator,
    placement_mode: PlacementState,
    ply: usize,
    remaining_depth: usize,
    mut alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic
where
    T: SearchTerminator,
    NT: NodeType,
{
    debug_assert!(state.validation_err().is_ok());
    // if let Err(err) = state.validation_err() {
    //     panic!("{}", err);
    // }

    search_state.search_stack[ply].eval = -INFINITY;
    search_state.nodes_visited += 1;
    let mut best_score = -INFINITY;

    let alpha_orig = alpha;
    let mut should_stop = false;

    let mut placements = get_placement_actions::<true>(&state, placement_mode);
    let mut best_action = placements[0];

    let tt_entry = search_context.tt.fetch(&state, ply);
    if let Some(tt_entry) = tt_entry {
        let tt_move: WorkerPlacement = tt_entry.best_action.into();
        for i in 1..placements.len() {
            if placements[i] == tt_move {
                placements.swap(0, i);
                break;
            }
        }
    }

    let next_mode = placement_mode.next();
    let turn_switch_score_mult = [-1, 1][placement_mode.is_swapped as usize];

    search_state.search_stack[ply].eval = -WINNING_SCORE_BUFFER;
    for action in placements {
        search_state.search_stack[ply].move_hash = hash_u64(action.get_history_idx(&state.board));
        let child_state = action.make_on_clone(state, placement_mode.next_placement);

        let score = if let Some(next_mode) = next_mode {
            -_placement_search::<T, NT::Next>(
                search_context,
                search_state,
                &child_state,
                nnue_acc,
                next_mode,
                ply + 1,
                remaining_depth,
                -beta,
                -alpha,
            )
        } else {
            turn_switch_score_mult
                * _start_inner_search::<T, NT::Next>(
                    search_context,
                    search_state,
                    &child_state,
                    nnue_acc,
                    ply + 1,
                    remaining_depth,
                    -beta,
                    -alpha,
                )
        };

        should_stop = search_context.should_stop(&search_state);

        if score > best_score {
            best_score = score;
            best_action = action;

            if NT::ROOT && (!should_stop || should_stop && search_state.best_move.is_none()) {
                let new_best_move = BestSearchResult::new(
                    child_state.clone(),
                    best_action.into(),
                    true,
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
                    break;
                }
            }
        }

        should_stop = search_context.should_stop(&search_state);
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

        search_context.tt.insert(
            &state,
            best_action.into(),
            remaining_depth as u8,
            tt_score_type,
            best_score,
            -INFINITY,
            ply,
        );
    }

    best_score
}

fn _q_extend<T>(
    search_context: &mut SearchContext<T>,
    search_state: &mut SearchState,
    state: &FullGameState,
    nnue_acc: &mut LabeledAccumulator,
    ply: usize,
    q_depth: u32,
    mut alpha: Hueristic,
    beta: Hueristic,
) -> Hueristic
where
    T: SearchTerminator,
{
    search_state.nodes_visited += 1;

    let tt_entry = search_context.tt.fetch(&state, ply);
    if let Some(tt_value) = &tt_entry {
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
    }
    let alpha_orig = alpha;

    let (active_god, other_god) = state.get_active_non_active_gods();

    // If we have a win right now, just take it
    if active_god
        .get_winning_moves(&state, state.board.current_player)
        .len()
        > 0
    {
        let score = win_at_ply(ply);
        return score;
    }

    if q_depth > 20 || ply >= MAX_PLY {
        // Give up at some max depth
        nnue_acc.replace_from_state(&state);
        return nnue_acc.evaluate().min(beta);
    }

    let eval;
    let child_moves;
    let opponent_wins = other_god.get_winning_moves(&state, !state.board.current_player);

    // Don't bother taking the current eval if we're in check - we have to respond to it.
    if opponent_wins.len() > 0 {
        eval = -win_at_ply(ply + 1);
        let mut blocker_board = BitBoard::EMPTY;
        for action in &opponent_wins {
            blocker_board |= other_god.get_blocker_board(&state.board, action.action);
        }
        child_moves = active_god.get_unscored_blocker_moves(
            &state,
            state.board.current_player,
            blocker_board,
        );
    } else {
        nnue_acc.replace_from_state(&state);
        eval = nnue_acc.evaluate();

        return eval.min(beta);
    }

    if eval >= beta {
        return beta;
    }

    alpha = alpha.max(eval);

    let mut best_score = eval;
    let mut best_action = GenericMove::NULL_MOVE;

    let mut should_stop = false;
    for child_move in child_moves.iter().rev() {
        let child_state = state.next_state(active_god, child_move.action);

        let score = -_q_extend(
            search_context,
            search_state,
            &child_state,
            nnue_acc,
            ply + 1,
            q_depth + 1,
            -beta,
            -alpha,
        );
        if score > best_score {
            best_score = score;
            best_action = child_move.action;

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    break;
                }
            }
        }

        should_stop = search_context.should_stop(&search_state);
        if should_stop {
            break;
        }
    }

    if q_depth < 2 && !should_stop {
        let tt_score_type = if best_score <= alpha_orig {
            SearchScoreType::UpperBound
        } else if best_score >= beta {
            SearchScoreType::LowerBound
        } else {
            SearchScoreType::Exact
        };

        search_context
            .tt
            .insert(&state, best_action, 0, tt_score_type, best_score, eval, ply);
    }

    best_score
}

fn _inner_search<T, NT>(
    search_context: &mut SearchContext<T>,
    search_state: &mut SearchState,
    state: &FullGameState,
    nnue_acc: &mut LabeledAccumulator,
    is_in_check: bool,
    ply: usize,
    carry_reduction: i32,
    mut remaining_depth: i32,
    mut alpha: Hueristic,
    mut beta: Hueristic,
    is_cut_node: bool,
) -> Hueristic
where
    T: SearchTerminator,
    NT: NodeType,
{
    debug_assert!(state.validation_err().is_ok());
    // if let Err(err) = state.validation_err() {
    //     state.print_to_console();
    //     panic!("{}", err);
    // }

    let current_player_idx = state.board.current_player as usize;
    let other_player_idx = !state.board.current_player;

    let (active_god, other_god) = state.get_active_non_active_gods();

    if !NT::ROOT
        && let Some(winner) = state.get_winner()
    {
        search_state.nodes_visited += 1;
        return if winner == state.board.current_player {
            win_at_ply(ply)
        } else {
            -win_at_ply(ply)
        };
    } else if remaining_depth <= 0 {
        return _q_extend(
            search_context,
            search_state,
            state,
            nnue_acc,
            ply,
            0,
            alpha,
            beta,
        );
    } else if !NT::ROOT {
        search_state.nodes_visited += 1;

        // Worst possible outcome is losing right now (due to a smother)
        // Best possible outcome is winning right now
        alpha = alpha.max(-win_at_ply(ply));
        beta = beta.min(win_at_ply(ply));
        if alpha >= beta {
            return alpha;
        }
    } else {
        search_state.nodes_visited += 1;
    }

    let mut track_used = false;
    let mut track_unused = false;
    let tt_entry = search_context.tt.fetch(&state, ply);

    if !NT::ROOT {
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
    }

    let alpha_orig = alpha;

    // internal iterative reduction
    // reduce depth on a tt miss
    if !NT::ROOT && remaining_depth >= 4 && tt_entry.is_none() {
        remaining_depth -= 1;
    }

    let key_squares = if is_in_check {
        let passed_state = state.next_state_passing(active_god);
        let other_wins = other_god.get_winning_moves(&passed_state, other_player_idx);

        if other_wins.len() == 0 {
            // TODO: fix all these?
            // Or maybe it's not worth being so precise, if these are rare cases
            // I think this breaks if you walk from level 3 to level 3?
            // ...checks should just be a test in move gen, no need for the threats only stuff i
            // think
            // eprintln!(
            //     "claimed to be in check but wasn't?: {:?}",
            //     state.as_basic_game_state()
            // );
            // state.print_to_console();
            // assert_ne!(other_wins.len(), 0);
            None
        } else {
            let mut key_squares = BitBoard::EMPTY;
            for action in &other_wins {
                key_squares |= other_god.get_blocker_board(&state.board, action.action);
            }

            remaining_depth += 1;

            Some(key_squares)
        }
    } else {
        None
    };

    let mut move_picker = MovePicker::new(
        state.board.current_player,
        active_god,
        tt_entry.as_ref().map(|e| e.best_action),
        search_state.killer_move_table[ply as usize],
        key_squares,
    );

    if !move_picker.has_any_moves(&state) {
        // If this is root, we need to pick a move
        if NT::ROOT {
            let moves = active_god.get_moves_for_search(&state, state.board.current_player);
            if moves.len() == 0 {
                // There's actually no moves so we don't have to pick one
                return -win_at_ply(ply);
            } else {
                let score = -win_at_ply(ply + 1);
                let best_action = moves[0].action;
                let child_state = state.next_state(active_god, best_action);

                let new_best_move = BestSearchResult::new(
                    child_state.clone(),
                    best_action,
                    false,
                    score,
                    remaining_depth.max(0) as usize,
                    search_state.nodes_visited,
                    BestMoveTrigger::EndOfLine,
                );

                search_state.best_move = Some(new_best_move.clone());
                (search_context.new_best_move_callback)(new_best_move);

                return score;
            }
        }

        // If we're in check, assume that we're not smothered and are losing on the next turn
        if key_squares.is_some() {
            return -win_at_ply(ply + 1);
        } else {
            return -win_at_ply(ply);
        }
    }

    if let Some(winning_action) = move_picker.get_winning_move(&state) {
        let score = win_at_ply(ply);
        if NT::ROOT {
            let winning_state = state.next_state(active_god, winning_action);
            debug_assert!(winning_state.get_winner() == Some(state.board.current_player));

            let new_best_move = BestSearchResult::new(
                winning_state,
                winning_action,
                false,
                score,
                remaining_depth.max(0) as usize,
                search_state.nodes_visited,
                BestMoveTrigger::EndOfLine,
            );
            search_state.best_move = Some(new_best_move.clone());
            (search_context.new_best_move_callback)(new_best_move);
        }

        return score;
    }

    nnue_acc.replace_from_state(&state);
    let eval = if let Some(tt_value) = &tt_entry {
        tt_value.eval
    } else {
        nnue_acc.evaluate()
    };

    if ply >= MAX_PLY - 1 {
        return eval;
    }

    let ss = &mut search_state.search_stack;
    ss[ply].eval = eval;

    let (improving, eval_delta) = if ply >= 2 {
        let delta = eval - ss[ply - 2].eval;
        (delta > 0, delta)
    } else if ply >= 4 {
        let delta = eval - ss[ply - 4].eval;
        (delta > 0, delta)
    } else {
        (true, 0)
    };

    // let mut child_nnue_acc = nnue_acc.clone();
    if !NT::ROOT && !NT::PV && key_squares.is_none() {
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
            let nmp_reduction = (4 + remaining_depth / 4).min(remaining_depth);

            search_state.search_stack[ply].is_null_move = true;
            #[cfg(target_pointer_width = "32")]
            const NULL_MOVE_HASH: usize = 2140012677;
            #[cfg(target_pointer_width = "64")]
            const NULL_MOVE_HASH: usize = 71369690056371976;
            search_state.search_stack[ply].move_hash = NULL_MOVE_HASH;

            let null_move_child_state = state.next_state_passing(active_god);

            let null_value = -_inner_search::<T, OffPV>(
                search_context,
                search_state,
                &null_move_child_state,
                nnue_acc,
                false,
                ply + 1,
                carry_reduction,
                remaining_depth - nmp_reduction,
                -beta,
                -beta + 1,
                !is_cut_node,
            );
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
    let next_depth = remaining_depth - 1;

    let lmp_d = remaining_depth.max(1) + improving as i32;
    let _lmp_cutoff = if improving {
        (10 + 3 * lmp_d * lmp_d).max(0) as usize
    } else {
        (6 + lmp_d * lmp_d).max(0) as usize
    };

    let low_score_cutoff = -750 * (remaining_depth.max(0) + 1) as MoveScore;

    let _nd2 = ((next_depth + 1) * (next_depth + 1)) as HistoryDelta;
    let nd1 = (next_depth + 1) as HistoryDelta;
    let nd = nd1;
    let prev_move_idx = if ply > 0 {
        Some(search_state.search_stack[ply - 1].move_hash)
    } else {
        None
    };

    let follow_move_idx = if ply > 1 {
        Some(search_state.search_stack[ply - 2].move_hash)
    } else {
        None
    };

    while let Some(child_scored_action) = move_picker.next(
        &state,
        &search_state.history[current_player_idx],
        ply,
        prev_move_idx,
        follow_move_idx,
    ) {
        let move_score = child_scored_action.score;
        let child_action = child_scored_action.action;

        let child_is_check = child_action.get_is_check();
        move_idx += 1;

        let history_move_hash = active_god.get_history_hash(&state.board, child_action);
        search_state.search_stack[ply].move_hash = history_move_hash;

        let mut move_score_adjustment = 0;

        let mut score;
        let child_state;
        if move_idx == 1 {
            child_state = state.next_state(active_god, child_action);

            score = -_inner_search::<T, NT::Next>(
                search_context,
                search_state,
                &child_state,
                nnue_acc,
                child_is_check,
                ply + 1,
                carry_reduction,
                next_depth,
                -beta,
                -alpha,
                !is_cut_node,
            )
        } else {
            let mut reduction = 0;
            if remaining_depth > 2 {
                reduction += search_context
                    .tt
                    .lmr_table
                    .get(remaining_depth as usize, move_idx);
                if !NT::PV {
                    reduction += 1024;
                }

                reduction += (is_cut_node && remaining_depth >= 6) as i32 * 1024;
                reduction -= (eval_delta as i32) / 2;
                reduction -= key_squares.is_some() as i32 * 2048;
                reduction -= child_action.get_is_check() as i32 * 1024;
                reduction -= (move_score == KILLER_MATCH_SCORE) as i32 * 1024;
                reduction = reduction.max(0);
            }

            let used_reduction = reduction / 1024;
            let remaining_reduction = reduction % 1024;
            let next_depth = (remaining_depth - 1).max(0);
            let reduced_depth = (next_depth - used_reduction).clamp(0, next_depth);

            // Prune quiet moves once move scores get very low
            if move_score < low_score_cutoff
                && move_picker.stage == MovePickerStage::YieldNonImprovers
                && key_squares.is_none()
            {
                break;
            }

            // Soft qs on the last ply
            if ply >= 2
                && next_depth <= 0
                && key_squares.is_none()
                && move_idx > 12
                && move_picker.stage == MovePickerStage::YieldNonImprovers
            {
                break;
            }

            child_state = state.next_state(active_god, child_action);

            // Try a 0-window search
            score = -_inner_search::<T, OffPV>(
                search_context,
                search_state,
                &child_state,
                nnue_acc,
                child_is_check,
                ply + 1,
                remaining_reduction,
                reduced_depth,
                -alpha - 1,
                -alpha,
                true,
            );

            // If we improve alpha and there was a reduction, try again without that reduction
            if score > alpha && used_reduction >= 1 && next_depth > reduced_depth {
                score = -_inner_search::<T, OffPV>(
                    search_context,
                    search_state,
                    &child_state,
                    nnue_acc,
                    child_is_check,
                    ply + 1,
                    0,
                    next_depth,
                    -alpha - 1,
                    -alpha,
                    !is_cut_node,
                );
            }

            // The search failed, try again
            if score > alpha && score < beta {
                score = -_inner_search::<T, NT::Next>(
                    search_context,
                    search_state,
                    &child_state,
                    nnue_acc,
                    child_is_check,
                    ply + 1,
                    0,
                    next_depth,
                    -beta,
                    -alpha,
                    false,
                )
            }
        };

        should_stop = search_context.should_stop(&search_state);

        if score > best_score {
            best_score = score;
            best_action = child_action;

            if NT::ROOT && (!should_stop || should_stop && search_state.best_move.is_none()) {
                let new_best_move = BestSearchResult::new(
                    child_state.clone(),
                    best_action,
                    false,
                    score,
                    remaining_depth.max(0) as usize,
                    search_state.nodes_visited,
                    BestMoveTrigger::Improvement,
                );

                search_state.best_move = Some(new_best_move.clone());
                (search_context.new_best_move_callback)(new_best_move);
            }

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    if move_picker.stage == MovePickerStage::YieldNonImprovers {
                        search_state.killer_move_table[ply] = Some(child_action);
                    }

                    move_score_adjustment += 75 * nd;
                    search_state.history[current_player_idx].update_move(
                        history_move_hash,
                        ply,
                        move_score_adjustment,
                        prev_move_idx,
                        follow_move_idx,
                    );
                    break;
                }
            }
        }

        let mut delta_scaled = (score - best_score) as HistoryDelta;
        delta_scaled /= 60;
        let delta = delta_scaled.clamp(-4 * nd, 0 * nd);
        move_score_adjustment += delta;

        search_state.history[current_player_idx].update_move(
            move_idx,
            ply,
            move_score_adjustment,
            prev_move_idx,
            follow_move_idx,
        );

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

        let history_move_hash = active_god.get_history_hash(&state.board, best_action);
        search_state.history[current_player_idx].update_move(
            history_move_hash,
            ply,
            3 * nd,
            prev_move_idx,
            follow_move_idx,
        );

        // Early on in the game, add all permutations of a board state to the TT, to help
        // deduplicate identical searches
        if state.board.height_map[0].count_ones() <= 1 {
            for base in state
                .board
                .get_all_permutations::<false>(state.gods, state.base_hash())
            {
                search_context.tt.conditionally_insert(
                    &FullGameState::new(base, state.gods),
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
            &state,
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

    use crate::search_terminators::DynamicMaxDepthSearchTerminator;

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
            terminator: DynamicMaxDepthSearchTerminator::new(2),
        };

        let search_state = negamax_search(&mut search_context, full_state);

        let _best_move = search_state.best_move.unwrap();
        // assert!(best_move.score > -WINNING_SCORE_BUFFER);
        // assert!(orig_loss_counter.borrow().clone() <= 1);
    }
}
