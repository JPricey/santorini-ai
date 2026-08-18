use crate::{
    bitboard::{
        BitBoard, BitboardMapping, DIAGONAL_ONLY_NEIGHBOR_MAP,
        INCLUSIVE_DIAGONAL_ONLY_NEIGHBOR_MAP, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP,
        WIND_AWARE_INCLUSIVE_NEIGHBOR_MAP, WIND_AWARE_NEIGHBOR_MAP,
        WRAPPING_DIAGONAL_ONLY_NEIGHBOR_MAP, WRAPPING_NEIGHBOR_MAP,
        WRAPPING_WIND_AWARE_NEIGHBOR_MAP, apply_mapping_to_mask,
    },
    board::{BoardState, FullGameState},
    direction::direction_idx_to_reverse,
    gods::{
        GodName, StaticGod,
        generic::{
            GenericMove, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, MoveGenFlags,
            STOP_ON_MATE, ScoredMove,
        },
        harpies::{slide_position, slide_position_with_custom_blockers},
        hypnus::hypnus_moveable_worker_filter,
    },
    player::Player,
    square::Square,
};

pub(super) fn is_mate_only<const F: MoveGenFlags>() -> bool {
    F & MATE_ONLY != 0
}

pub(super) fn is_include_score<const F: MoveGenFlags>() -> bool {
    F & INCLUDE_SCORE != 0
}

pub(super) fn is_stop_on_mate<const F: MoveGenFlags>() -> bool {
    F & STOP_ON_MATE != 0
}

pub(super) fn is_interact_with_key_squares<const F: MoveGenFlags>() -> bool {
    F & INTERACT_WITH_KEY_SQUARES != 0
}

pub(super) fn push_winning_moves<
    const F: MoveGenFlags,
    T: Into<GenericMove>,
    B: Fn(Square, Square) -> T,
>(
    result: &mut Vec<ScoredMove>,
    worker_start_pos: Square,
    wins: BitBoard,
    build_move: B,
) -> bool {
    for end_pos in wins {
        let winning_move: T = build_move(worker_start_pos, end_pos);
        result.push(ScoredMove::new_winning_move(winning_move.into()));

        if is_stop_on_mate::<F>() {
            return true;
        }
    }

    false
}

pub(super) fn build_scored_move<const F: MoveGenFlags, T: Into<GenericMove>>(
    action: T,
    is_check: bool,
    is_improving: bool,
) -> ScoredMove {
    if !is_include_score::<F>() {
        ScoredMove::new_unscored_move(action.into())
    } else if is_check {
        ScoredMove::new_checking_move(action.into())
    } else if is_improving {
        ScoredMove::new_improving_move(action.into())
    } else {
        ScoredMove::new_non_improver(action.into())
    }
}

pub(super) fn get_sized_result<const F: MoveGenFlags>() -> Vec<ScoredMove> {
    let capacity = if is_mate_only::<F>() { 1 } else { 128 };
    Vec::with_capacity(capacity)
}

pub(crate) struct GeneratorPreludeState<'a> {
    pub board: &'a BoardState,
    pub key_squares: BitBoard,
    pub other_god: StaticGod,

    pub exactly_level_0: BitBoard,
    pub exactly_level_1: BitBoard,
    pub exactly_level_2: BitBoard,
    pub exactly_level_3: BitBoard,
    pub domes_and_frozen: BitBoard,

    pub standard_neighbor_map: &'static BitboardMapping,

    pub can_climb: bool,

    pub own_workers: BitBoard,
    pub oppo_workers: BitBoard,

    pub all_workers_and_frozen_mask: BitBoard,
    pub win_mask: BitBoard,
    pub build_mask: BitBoard,
    pub affinity_area: BitBoard,

    /// Charybdis' two whirlpools, and only when *both* are on the board - a lone whirlpool is an
    /// ordinary square. Set whichever side owns them, since both players use the portal.
    pub portal_squares: BitBoard,

    /// Squares a worker could win *from* on its next move.
    ///
    /// Normally exactly the level 2 squares. An armed portal whose exit sits on level 3 lets a
    /// worker win from any height, so this widens to cover that - see `portal_mate_sources`.
    /// Widening is always safe: it only decides which workers are *considered*, never which moves
    /// are emitted.
    pub mate_start_mask: BitBoard,

    pub is_against_hypnus: bool,
    pub is_against_harpies: bool,
    pub is_down_prevented: bool,

    pub acting_workers: BitBoard,
}

pub(super) fn get_generator_prelude_state<'a, const F: MoveGenFlags>(
    state: &'a FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> GeneratorPreludeState<'a> {
    let board = &state.board;
    let other_god = state.gods[!player as usize];

    let exactly_level_0 = board.exactly_level_0();
    let exactly_level_1 = board.exactly_level_1();
    let exactly_level_2 = board.exactly_level_2();
    let exactly_level_3 = board.exactly_level_3();
    let domes = board.at_least_level_4();
    let frozen = other_god.get_frozen_mask(&board, !player);

    let own_workers = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
    let oppo_workers = board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK;

    let all_workers_mask = own_workers | oppo_workers;
    let win_mask = other_god.win_mask;

    let build_mask = other_god.get_build_mask(oppo_workers) | exactly_level_3;

    let is_against_hypnus = other_god.is_hypnus();
    let is_against_harpies = other_god.is_harpies();
    let is_against_aphrodite = other_god.is_aphrodite;

    let affinity_area = if is_against_aphrodite {
        apply_mapping_to_mask(oppo_workers, &INCLUSIVE_NEIGHBOR_MAP)
    } else {
        BitBoard::EMPTY
    };

    let acting_workers = if is_against_hypnus {
        hypnus_moveable_worker_filter(&board, own_workers)
    } else {
        own_workers
    };

    let portal_squares = get_portal_squares(state, player);
    // A whirlpool exit sitting on level 3 is a win for whoever can step into the other whirlpool,
    // from any height at all. Crude over-approximation: consider every worker. This only fires in
    // positions that actually have an armed level 3 portal.
    let mate_start_mask = if (portal_squares & exactly_level_3 & win_mask).is_not_empty() {
        BitBoard::MAIN_SECTION_MASK
    } else {
        exactly_level_2
    };

    let can_climb = other_god.can_opponent_climb(board, !player);

    let is_down_prevented = other_god.is_preventing_down;

    let neighbor_map = if other_god.god_name == GodName::Aeolus {
        &WIND_AWARE_NEIGHBOR_MAP[other_god.get_wind_idx(board, !player)]
    } else if other_god.god_name == GodName::Hippolyta {
        &DIAGONAL_ONLY_NEIGHBOR_MAP
    } else {
        &NEIGHBOR_MAP
    };

    GeneratorPreludeState {
        board,
        key_squares,
        other_god,

        exactly_level_0,
        exactly_level_1,
        exactly_level_2,
        exactly_level_3,
        domes_and_frozen: domes | frozen,

        standard_neighbor_map: neighbor_map,

        can_climb,

        own_workers,
        oppo_workers,
        all_workers_and_frozen_mask: all_workers_mask | frozen,
        win_mask,
        build_mask,
        affinity_area,

        portal_squares,
        mate_start_mask,

        is_against_hypnus,
        is_against_harpies,
        is_down_prevented,

        acting_workers,
    }
}

pub(super) fn get_urania_movement_neighbors(
    prelude: &GeneratorPreludeState,
    player: Player,
) -> &'static BitboardMapping {
    if prelude.other_god.god_name == GodName::Aeolus {
        &WRAPPING_WIND_AWARE_NEIGHBOR_MAP[prelude.other_god.get_wind_idx(prelude.board, !player)]
    } else if prelude.other_god.god_name == GodName::Hippolyta {
        &WRAPPING_DIAGONAL_ONLY_NEIGHBOR_MAP
    } else {
        &WRAPPING_NEIGHBOR_MAP
    }
}

pub(super) fn get_inclusive_movement_neighbors(
    prelude: &GeneratorPreludeState,
) -> &'static BitboardMapping {
    if prelude.other_god.god_name == GodName::Aeolus {
        &WIND_AWARE_INCLUSIVE_NEIGHBOR_MAP[prelude
            .other_god
            .get_wind_idx(prelude.board, !prelude.board.current_player)]
    } else if prelude.other_god.god_name == GodName::Hippolyta {
        &INCLUSIVE_DIAGONAL_ONLY_NEIGHBOR_MAP
    } else {
        &INCLUSIVE_NEIGHBOR_MAP
    }
}

pub(super) fn get_reverse_direction_neighbor_map(
    prelude: &GeneratorPreludeState,
) -> &'static BitboardMapping {
    if prelude.other_god.god_name == GodName::Aeolus {
        let wind_direction_idx = prelude
            .other_god
            .get_wind_idx(prelude.board, !prelude.board.current_player);
        let reversed_wind_direction_idx = direction_idx_to_reverse(wind_direction_idx);

        &WIND_AWARE_NEIGHBOR_MAP[reversed_wind_direction_idx]
    } else if prelude.other_god.god_name == GodName::Hippolyta {
        &DIAGONAL_ONLY_NEIGHBOR_MAP
    } else {
        &NEIGHBOR_MAP
    }
}

pub(super) fn modify_prelude_for_checking_workers<const F: MoveGenFlags>(
    checkable_from_mask: BitBoard,
    prelude: &mut GeneratorPreludeState,
) {
    if is_mate_only::<F>() {
        prelude.acting_workers &= checkable_from_mask;
    }
}

pub(super) fn get_basic_acting_workers<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
) -> BitBoard {
    let mut acting_workers = prelude.own_workers;
    if prelude.is_against_hypnus {
        acting_workers = hypnus_moveable_worker_filter(&prelude.board, acting_workers)
    }
    if is_mate_only::<F>() {
        acting_workers &= prelude.mate_start_mask;
    }

    acting_workers
}

pub(super) struct WorkerStartMoveState {
    pub worker_start_pos: Square,
    pub worker_start_mask: BitBoard,
    pub worker_start_height: usize,
    pub other_own_workers: BitBoard,
    pub all_non_moving_workers: BitBoard,

    /// Could this worker win on its next move? `worker_start_height == 2`, plus portal mates.
    pub can_mate: bool,

    /// The level 3 squares this worker actually wins on.
    ///
    /// You have to move *up* onto level 3, so normally only a level 2 worker wins anywhere, and
    /// every other worker wins nowhere - a level 3 worker moving flat to another level 3 square
    /// has not won. The exception is the whirlpool exit, which wins from any height at all, so a
    /// worker that cannot climb to level 3 can still win on that one square.
    pub winnable_squares: BitBoard,
}

pub(super) struct WorkerNextMoveState {
    pub other_threatening_workers: BitBoard,
    pub other_threatening_neighbors: BitBoard,
    pub worker_moves: BitBoard,
}

pub(super) fn get_worker_start_move_state(
    prelude: &GeneratorPreludeState,
    worker_start_pos: Square,
) -> WorkerStartMoveState {
    let worker_start_mask = BitBoard::as_mask(worker_start_pos);
    let worker_start_height = prelude.board.get_height(worker_start_pos);

    let other_own_workers = prelude.own_workers ^ worker_start_mask;
    let non_moving_workers = prelude.oppo_workers | other_own_workers;
    let can_mate = (prelude.mate_start_mask & worker_start_mask).is_not_empty();

    let arrival_mask = if worker_start_height == 2 {
        BitBoard::MAIN_SECTION_MASK
    } else {
        get_active_portal(prelude, worker_start_mask)
    };
    let winnable_squares = prelude.exactly_level_3 & prelude.win_mask & arrival_mask;

    WorkerStartMoveState {
        worker_start_pos,
        worker_start_mask,
        worker_start_height,
        other_own_workers,
        all_non_moving_workers: non_moving_workers,
        can_mate,
        winnable_squares,
    }
}

pub(super) fn get_worker_next_move_state<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    checkable_from_mask: BitBoard,
) -> WorkerNextMoveState {
    let other_threatening_workers = worker_start_state.other_own_workers & checkable_from_mask;
    let other_threatening_neighbors =
        apply_mapping_to_mask(other_threatening_workers, prelude.standard_neighbor_map);
    let worker_moves = get_basic_moves::<MUST_CLIMB>(prelude, worker_start_state);

    WorkerNextMoveState {
        other_threatening_workers,
        other_threatening_neighbors,
        worker_moves,
    }
}

/// Both whirlpools, if Charybdis is in this matchup and both are on the board.
///
/// A lone whirlpool is just an ordinary square, so it is reported as no portal at all.
pub(crate) fn get_portal_squares(state: &FullGameState, player: Player) -> BitBoard {
    for side in [player, !player] {
        let god = state.gods[side as usize];
        if god.is_token_user {
            let tokens = god.get_token_mask(&state.board, side);
            return if tokens.count_ones() == 2 {
                tokens
            } else {
                BitBoard::EMPTY
            };
        }
    }

    BitBoard::EMPTY
}

/// Blocking move generation only keeps moves that touch a key square, which it judges by where a
/// move *ends*. That misses a whirlpool defence: a worker parked on a whirlpool is holding the
/// portal shut, and simply stepping off it arms the portal and flushes the opponent off the square
/// they were about to win on. Rather than teach every generator's narrowing about that, stop
/// narrowing altogether in the rare positions where it applies.
pub(crate) fn widen_key_squares_for_portal(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> BitBoard {
    let portal = get_portal_squares(state, player);
    if portal.is_empty() || (portal & key_squares).is_empty() {
        return key_squares;
    }

    if (portal & state.board.workers[player as usize]).is_not_empty() {
        return BitBoard::MAIN_SECTION_MASK;
    }

    key_squares
}

/// The portal as seen by one specific worker.
///
/// A whirlpool only teleports if the *other* whirlpool is unoccupied. The moving worker's own
/// square does not count as occupied - it is vacated before the teleport resolves - which is what
/// lets a worker standing on one whirlpool step into the other and be sent straight back.
pub(super) fn get_active_portal(
    prelude: &GeneratorPreludeState,
    worker_start_mask: BitBoard,
) -> BitBoard {
    get_active_portal_after_displacement(prelude, worker_start_mask, BitBoard::EMPTY)
}

/// The portal as seen by a worker whose move also relocates other workers.
///
/// "Unoccupied" is judged at the moment the mover *arrives*, which for a displacement god is after
/// the push/swap/pull has already happened - so a whirlpool the move clears becomes usable, and a
/// whirlpool the move fills stops working, all within the same turn. This is why the plain
/// `get_active_portal` (which reads the pre-move board) is only sound for gods that move a single
/// worker and touch nobody else.
///
/// `vacated` is every square a worker leaves during this move - the mover's own start, plus the
/// start square of anyone it displaces. `newly_filled` is every square a displaced worker is
/// pushed *onto*. Both are needed: a whirlpool is free only if no worker sits on it once the dust
/// settles.
pub(super) fn get_active_portal_after_displacement(
    prelude: &GeneratorPreludeState,
    vacated: BitBoard,
    newly_filled: BitBoard,
) -> BitBoard {
    let portal = prelude.portal_squares;
    if portal.is_empty() {
        return BitBoard::EMPTY;
    }

    let occupied = (prelude.all_workers_and_frozen_mask & !vacated) | newly_filled;
    if (portal & occupied).is_empty() {
        portal
    } else {
        BitBoard::EMPTY
    }
}

/// Rewrite a set of destinations so that entering a whirlpool lands on the other one.
///
/// If exactly one whirlpool is reachable, swap it for its partner. If both are reachable the set
/// is unchanged - entering either one lands on the other, so the same two squares are still the
/// possible outcomes. If neither is reachable there is nothing to do; note that a plain xor would
/// be wrong here, which is what the count guards against.
pub(super) fn put_moves_through_portals(moves: BitBoard, portal: BitBoard) -> BitBoard {
    if (moves & portal).count_ones() == 1 {
        moves ^ portal
    } else {
        moves
    }
}

pub(super) struct WorkerEndMoveState {
    pub worker_end_pos: Square,
    pub worker_end_mask: BitBoard,
    pub worker_end_height: usize,
    pub is_improving: bool,
    /// Whether the worker could win from here next move: level 2, or a portal mate.
    pub is_mate_capable: u32,
}

pub(super) fn get_worker_end_move_state<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    mut worker_end_pos: Square,
) -> WorkerEndMoveState {
    if prelude.is_against_harpies {
        worker_end_pos =
            slide_position(prelude, worker_start_state.worker_start_pos, worker_end_pos);
    }

    let worker_end_mask = BitBoard::as_mask(worker_end_pos);
    let worker_end_height = prelude.board.get_height(worker_end_pos);
    let is_improving = worker_end_height > worker_start_state.worker_start_height;
    let is_mate_capable = (prelude.mate_start_mask & worker_end_mask).is_not_empty() as u32;

    WorkerEndMoveState {
        worker_end_pos,
        worker_end_mask,
        worker_end_height,
        is_improving,
        is_mate_capable,
    }
}

pub(super) fn get_worker_end_move_state_with_custom_worker_helper<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    mut worker_end_pos: Square,
    blocker: BitBoard,
) -> WorkerEndMoveState {
    if prelude.is_against_harpies {
        worker_end_pos = slide_position_with_custom_blockers(
            prelude.board,
            worker_start_state.worker_start_pos,
            worker_end_pos,
            blocker,
        );
    }

    let worker_end_mask = BitBoard::as_mask(worker_end_pos);
    let worker_end_height = prelude.board.get_height(worker_end_pos);
    let is_improving = worker_end_height > worker_start_state.worker_start_height;
    let is_mate_capable = (prelude.mate_start_mask & worker_end_mask).is_not_empty() as u32;

    WorkerEndMoveState {
        worker_end_pos,
        worker_end_mask,
        worker_end_height,
        is_improving,
        is_mate_capable,
    }
}

pub(super) fn get_reach_board_when_can_be_level_3<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_move_state: &WorkerNextMoveState,
    has_level_3_others: bool,
    worker_end_pos: Square,
    worker_end_height: usize,
    unblocked_squares: BitBoard,
) -> BitBoard {
    if prelude.is_against_hypnus && prelude.portal_squares.is_empty() {
        let next_turn_moves = NEIGHBOR_MAP[worker_end_pos as usize];

        if worker_end_height == 3 {
            worker_move_state.other_threatening_neighbors & unblocked_squares
        } else if has_level_3_others
            || (worker_move_state.other_threatening_workers.count_ones()
                + (worker_end_height == 2) as u32)
                >= 2
        {
            (worker_move_state.other_threatening_neighbors
                | (next_turn_moves * (worker_end_height == 2) as u32))
                & unblocked_squares
        } else {
            BitBoard::EMPTY
        }
    } else {
        let next_turn_moves = put_moves_through_portals(
            prelude.standard_neighbor_map[worker_end_pos as usize],
            prelude.portal_squares,
        );

        // `worker_end_height` is passed in rather than read off the board because callers like
        // Zeus hand us the height *after* building under themselves. Only the portal case, which
        // does not depend on height at all, comes from the prelude.
        let is_mate_capable = (worker_end_height == 2
            || prelude.portal_squares.is_not_empty()
                && (prelude.mate_start_mask & BitBoard::as_mask(worker_end_pos)).is_not_empty())
            as u32;

        (worker_move_state.other_threatening_neighbors | (next_turn_moves * is_mate_capable))
            & prelude.win_mask
            & unblocked_squares
    }
}

pub(super) fn get_standard_reach_board<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_move_state: &WorkerNextMoveState,
    worker_end_move_state: &WorkerEndMoveState,
    unblocked_squares: BitBoard,
) -> BitBoard {
    get_standard_reach_board_from_parts::<F>(
        prelude,
        worker_move_state.other_threatening_workers,
        worker_move_state.other_threatening_neighbors,
        worker_end_move_state.worker_end_pos,
        worker_end_move_state.is_mate_capable,
        unblocked_squares,
    )
}

pub(super) fn get_standard_reach_board_from_parts<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    other_threatening_workers: BitBoard,
    other_threatening_neighbors: BitBoard,
    worker_end_pos: Square,
    is_mate_capable: u32,
    unblocked_squares: BitBoard,
) -> BitBoard {
    // A single worker's next-turn destinations, so the exact swap applies: stepping into one
    // whirlpool threatens the other, and no longer threatens the one stepped into.
    let next_turn_moves = put_moves_through_portals(
        prelude.standard_neighbor_map[worker_end_pos as usize],
        prelude.portal_squares,
    ) & unblocked_squares;

    // Hypnus can only freeze the single highest worker, so one threat is not enough - unless the
    // threat is a portal mate, which can come from a low worker he has no answer to.
    let reach_board = if prelude.is_against_hypnus
        && prelude.portal_squares.is_empty()
        && (other_threatening_workers.count_ones() + is_mate_capable) < 2
    {
        BitBoard::EMPTY
    } else {
        (other_threatening_neighbors | (next_turn_moves * is_mate_capable))
            & prelude.win_mask
            & unblocked_squares
    };

    reach_board
}

pub(super) fn get_standard_reach_board_with_extra_move_map<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    wind_map: &BitboardMapping,
    worker_move_state: &WorkerNextMoveState,
    worker_end_move_state: &WorkerEndMoveState,
    unblocked_squares: BitBoard,
) -> BitBoard {
    let next_turn_moves = put_moves_through_portals(
        prelude.standard_neighbor_map[worker_end_move_state.worker_end_pos as usize]
            & wind_map[worker_end_move_state.worker_end_pos as usize],
        prelude.portal_squares,
    ) & unblocked_squares;

    let reach_board = if prelude.is_against_hypnus
        && prelude.portal_squares.is_empty()
        && (worker_move_state.other_threatening_workers.count_ones()
            + worker_end_move_state.is_mate_capable)
            < 2
    {
        BitBoard::EMPTY
    } else {
        (worker_move_state.other_threatening_neighbors
            | (next_turn_moves * worker_end_move_state.is_mate_capable))
            & prelude.win_mask
            & unblocked_squares
    };

    reach_board
}

pub(super) struct WorkerNextBuildState {
    pub unblocked_squares: BitBoard,
    pub all_possible_builds: BitBoard,
    pub narrowed_builds: BitBoard,
}

pub(super) fn get_worker_next_build_state_with_is_matched<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    worker_end_move_state: &WorkerEndMoveState,
    is_key_squares_matched: bool,
) -> WorkerNextBuildState {
    let unblocked_squares = !(worker_start_state.all_non_moving_workers
        | worker_end_move_state.worker_end_mask
        | prelude.domes_and_frozen);
    let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
        & unblocked_squares
        & prelude.build_mask;
    let mut narrowed_builds = all_possible_builds;
    if is_interact_with_key_squares::<F>() {
        narrowed_builds &=
            [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_key_squares_matched as usize];
    }

    WorkerNextBuildState {
        unblocked_squares,
        all_possible_builds,
        narrowed_builds,
    }
}

pub(super) fn get_worker_next_build_state<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    worker_end_move_state: &WorkerEndMoveState,
) -> WorkerNextBuildState {
    let unblocked_squares = !(worker_start_state.all_non_moving_workers
        | worker_end_move_state.worker_end_mask
        | prelude.domes_and_frozen);
    let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
        & unblocked_squares
        & prelude.build_mask;
    let mut narrowed_builds = all_possible_builds;
    if is_interact_with_key_squares::<F>() {
        let is_already_matched =
            (worker_end_move_state.worker_end_mask & prelude.key_squares).is_not_empty() as usize;
        narrowed_builds &= [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
    }

    WorkerNextBuildState {
        unblocked_squares,
        all_possible_builds,
        narrowed_builds,
    }
}

pub(super) struct AfterMovePowerGeneratorContext<'a> {
    pub result: &'a mut Vec<ScoredMove>,
    pub prelude: &'a GeneratorPreludeState<'a>,
    pub worker_start_state: &'a WorkerStartMoveState,
    pub worker_end_state: &'a WorkerEndMoveState,
    pub worker_next_build_state: &'a WorkerNextBuildState,
    pub reach_board: BitBoard,
}

pub(super) fn make_build_only_power_generator<
    const F: MoveGenFlags,
    const MUST_CLIMB: bool,
    A: Into<GenericMove>,
    WinningMoveFn: Fn(Square, Square) -> A,
    BuildGeneratorFn: Fn(&mut AfterMovePowerGeneratorContext),
>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
    winning_move_fn: WinningMoveFn,
    build_generator_fn: BuildGeneratorFn,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.mate_start_mask;
    let acting_workers = get_basic_acting_workers::<F>(&prelude);

    for worker_start_pos in acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);
        if is_mate_only::<F>() || worker_start_state.can_mate {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & worker_start_state.winnable_squares;
            if push_winning_moves::<F, A, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                &winning_move_fn,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);
            let worker_next_build_state = get_worker_next_build_state::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            let mut power_context = AfterMovePowerGeneratorContext {
                result: &mut result,
                prelude: &prelude,
                worker_start_state: &worker_start_state,
                worker_end_state: &worker_end_move_state,
                worker_next_build_state: &worker_next_build_state,
                reach_board: reach_board,
            };

            build_generator_fn(&mut power_context);
        }
    }

    result
}

pub(super) fn get_worker_climb_height(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
) -> usize {
    get_worker_climb_height_raw(worker_start_state.worker_start_height, prelude.can_climb)
}

pub(super) fn get_worker_climb_height_raw(worker_start_height: usize, can_climb: bool) -> usize {
    3.min(worker_start_height + can_climb as usize)
}

pub(super) fn restrict_moves_by_affinity_area(
    worker_start_mask: BitBoard,
    worker_moves: BitBoard,
    affinity_area: BitBoard,
) -> BitBoard {
    if (worker_start_mask & affinity_area).is_not_empty() {
        worker_moves & affinity_area
    } else {
        worker_moves
    }
}

pub(super) fn get_basic_moves<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
) -> BitBoard {
    get_basic_moves_from_raw_data::<MUST_CLIMB>(
        prelude,
        worker_start_state.worker_start_pos,
        worker_start_state.worker_start_mask,
        worker_start_state.worker_start_height,
    )
}

pub(super) fn get_basic_moves_from_raw_data<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_pos: Square,
    worker_start_mask: BitBoard,
    worker_start_height: usize,
) -> BitBoard {
    get_basic_moves_from_raw_data_with_custom_blockers::<MUST_CLIMB>(
        prelude,
        worker_start_pos,
        worker_start_mask,
        worker_start_height,
        prelude.all_workers_and_frozen_mask,
    )
}

pub(super) fn get_basic_moves_from_raw_data_with_custom_blockers_no_affinity<
    const MUST_CLIMB: bool,
>(
    prelude: &GeneratorPreludeState,
    worker_start_pos: Square,
    worker_start_height: usize,
    blockers: BitBoard,
) -> BitBoard {
    let move_mask = prelude.standard_neighbor_map[worker_start_pos as usize];

    get_limited_moves_given_move_mask::<MUST_CLIMB, false>(
        prelude,
        move_mask,
        BitBoard::EMPTY,
        worker_start_height,
        blockers,
    )
}

pub(super) fn get_basic_moves_from_raw_data_with_custom_blockers<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_pos: Square,
    worker_start_mask: BitBoard,
    worker_start_height: usize,
    blockers: BitBoard,
) -> BitBoard {
    let move_mask = prelude.standard_neighbor_map[worker_start_pos as usize];

    get_limited_moves_given_move_mask::<MUST_CLIMB, true>(
        prelude,
        move_mask,
        worker_start_mask,
        worker_start_height,
        blockers,
    )
}

pub(super) fn get_basic_moves_from_with_two_movement_maps<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    extra_movement_map: &BitboardMapping,
    worker_start_pos: Square,
    worker_start_mask: BitBoard,
    worker_start_height: usize,
    blockers: BitBoard,
) -> BitBoard {
    let move_mask = prelude.standard_neighbor_map[worker_start_pos as usize]
        & extra_movement_map[worker_start_pos as usize];

    get_limited_moves_given_move_mask::<MUST_CLIMB, true>(
        prelude,
        move_mask,
        worker_start_mask,
        worker_start_height,
        blockers,
    )
}

// Returns all squares below the player - so against hades you also have to !this
#[allow(dead_code)]
pub(super) fn get_down_blocker_mask(
    prelude: &GeneratorPreludeState,
    worker_start_height: usize,
) -> BitBoard {
    if prelude.is_down_prevented && worker_start_height > 0 {
        !prelude.board.height_map[worker_start_height - 1]
    } else {
        BitBoard::EMPTY
    }
}

// Returns all squares not below the player - so against hades you also have to & this
pub(super) fn get_down_allowed_mask(
    prelude: &GeneratorPreludeState,
    worker_start_height: usize,
) -> BitBoard {
    if prelude.is_down_prevented && worker_start_height > 0 {
        prelude.board.height_map[worker_start_height - 1]
    } else {
        BitBoard::MAIN_SECTION_MASK
    }
}

pub(super) fn get_limited_moves_given_move_mask<
    const MUST_CLIMB: bool,
    const APPLY_AFFINITY: bool,
>(
    prelude: &GeneratorPreludeState,
    move_mask: BitBoard,
    worker_start_mask: BitBoard,
    worker_start_height: usize,
    blockers: BitBoard,
) -> BitBoard {
    // Whirlpools are applied *after* the height filters and *before* the affinity filter: only the
    // entry leg of a portal move has a height delta (so Athena, Hades and Persephone all judge the
    // entry), but the worker physically ends up on the exit square (so Aphrodite judges that).
    let portal = get_active_portal(prelude, worker_start_mask);

    if MUST_CLIMB {
        let height_mask = match worker_start_height {
            0 => prelude.exactly_level_1,
            1 => prelude.exactly_level_2,
            2 => prelude.exactly_level_3,
            3 => return BitBoard::EMPTY,
            _ => unreachable!(),
        };

        let worker_moves = move_mask & height_mask & !blockers;
        put_moves_through_portals(worker_moves, portal)
    } else {
        let down_allowed_mask = get_down_allowed_mask(prelude, worker_start_height);

        let climb_height = get_worker_climb_height_raw(worker_start_height, prelude.can_climb);
        let worker_moves =
            move_mask & down_allowed_mask & !(prelude.board.height_map[climb_height] | blockers);
        let worker_moves = put_moves_through_portals(worker_moves, portal);

        if APPLY_AFFINITY {
            restrict_moves_by_affinity_area(worker_start_mask, worker_moves, prelude.affinity_area)
        } else {
            worker_moves
        }
    }
}

pub(super) fn get_basic_moves_from_raw_data_for_hermes<const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    worker_start_pos: Square,
    worker_start_mask: BitBoard,
    worker_start_height: usize,
) -> BitBoard {
    if MUST_CLIMB {
        return get_basic_moves_from_raw_data::<true>(
            prelude,
            worker_start_pos,
            worker_start_mask,
            worker_start_height,
        );
    }

    let down_allowed_mask = get_down_allowed_mask(prelude, worker_start_height);
    let climb_height = get_worker_climb_height_raw(worker_start_height, prelude.can_climb);
    let worker_moves = prelude.standard_neighbor_map[worker_start_pos as usize]
        & down_allowed_mask
        & !(prelude.board.height_map[climb_height]
            | prelude.board.exactly_level_n(worker_start_height)
            | prelude.all_workers_and_frozen_mask);

    restrict_moves_by_affinity_area(worker_start_mask, worker_moves, prelude.affinity_area)
}

#[macro_export]
macro_rules! persephone_check_result {
    (
        $move_gen:ident,
        state: $state:ident,
        player: $player:ident,
        key_squares: $key_squares:ident,
        MUST_CLIMB: $MUST_CLIMB:ident
    ) => {
        if $state.gods[!$player as usize].is_persephone && !MUST_CLIMB {
            let result = $move_gen::<F, true>($state, $player, $key_squares);
            if result.len() > 0 {
                return result;
            }

            // Maybe we couldn't climb because of the other restrictions (must mate, must block)
            // Try again without the restriction. If we can find anything, return the empty result
            // Otherwise, we'll fall back to not climbing
            if F & $crate::gods::generic::ANY_MOVE_FILTER > 0 {
                let unrestricted = $move_gen::<0, true>($state, $player, $key_squares);
                if unrestricted.len() > 0 {
                    return result;
                }
            }

            result
        } else {
            $crate::gods::move_helpers::get_sized_result::<F>()
        }
    };
}
