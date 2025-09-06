use crate::{
    bitboard::{apply_mapping_to_mask, BitBoard, NEIGHBOR_MAP},
    board::{BoardState, FullGameState},
    gods::{
        generic::{
            GenericMove, MoveGenFlags, ScoredMove, INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, STOP_ON_MATE
        },
        harpies::slide_position,
        hypnus::hypnus_moveable_worker_filter, StaticGod,
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

pub(super) struct GeneratorPreludeState<'a> {
    pub board: &'a BoardState,
    pub player: Player,
    pub key_squares: BitBoard,
    pub other_god: StaticGod,

    pub exactly_level_0: BitBoard,
    pub exactly_level_1: BitBoard,
    pub exactly_level_2: BitBoard,
    pub exactly_level_3: BitBoard,
    pub domes: BitBoard,

    pub own_workers: BitBoard,
    pub oppo_workers: BitBoard,

    pub all_workers_mask: BitBoard,
    pub win_mask: BitBoard,
    pub build_mask: BitBoard,

    pub is_against_hypnus: bool,
    pub is_against_harpies: bool,

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

    let own_workers = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
    let oppo_workers = board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK;

    let all_workers_mask = own_workers | oppo_workers;
    let win_mask = other_god.win_mask;

    let build_mask = other_god.get_build_mask(oppo_workers) | exactly_level_3;
    let is_against_hypnus = other_god.is_hypnus();
    let is_against_harpies = other_god.is_harpies();

    let acting_workers = if is_against_hypnus {
        hypnus_moveable_worker_filter(&board, own_workers)
    } else {
        own_workers
    };

    GeneratorPreludeState {
        board,
        player,
        key_squares,
        other_god,

        exactly_level_0,
        exactly_level_1,
        exactly_level_2,
        exactly_level_3,
        domes,
        own_workers,
        oppo_workers,
        all_workers_mask,
        win_mask,
        build_mask,
        is_against_hypnus,
        is_against_harpies,

        acting_workers,
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
        acting_workers &= prelude.exactly_level_2;
    }

    acting_workers
}

pub(super) struct WorkerStartMoveState {
    pub worker_start_pos: Square,
    pub worker_start_mask: BitBoard,
    pub worker_start_height: usize,
    pub other_own_workers: BitBoard,
    pub all_non_moving_workers: BitBoard,
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

    WorkerStartMoveState {
        worker_start_pos,
        worker_start_mask,
        worker_start_height,
        other_own_workers,
        all_non_moving_workers: non_moving_workers,
    }
}

pub(super) fn get_worker_next_move_state(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    checkable_from_mask: BitBoard,
) -> WorkerNextMoveState {
    let other_threatening_workers = worker_start_state.other_own_workers & checkable_from_mask;
    let other_threatening_neighbors =
        apply_mapping_to_mask(other_threatening_workers, &NEIGHBOR_MAP);
    let worker_moves = NEIGHBOR_MAP[worker_start_state.worker_start_pos as usize]
        & !(prelude.board.height_map[prelude
            .board
            .get_worker_climb_height(prelude.player, worker_start_state.worker_start_height)]
            | prelude.all_workers_mask);

    WorkerNextMoveState {
        other_threatening_workers,
        other_threatening_neighbors,
        worker_moves,
    }
}

pub(super) struct WorkerEndMoveState {
    pub worker_end_pos: Square,
    pub worker_end_mask: BitBoard,
    pub worker_end_height: usize,
    pub is_improving: bool,
    pub is_now_lvl_2: u32,
}

pub(super) fn get_worker_end_move_state<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_start_state: &WorkerStartMoveState,
    mut worker_end_pos: Square,
) -> WorkerEndMoveState {
    if prelude.is_against_harpies {
        worker_end_pos = slide_position(
            prelude.board,
            worker_start_state.worker_start_pos,
            worker_end_pos,
        );
    }

    let worker_end_mask = BitBoard::as_mask(worker_end_pos);
    let worker_end_height = prelude.board.get_height(worker_end_pos);
    let is_improving = worker_end_height > worker_start_state.worker_start_height;
    let is_now_lvl_2 = (worker_end_height == 2) as u32;

    WorkerEndMoveState {
        worker_end_pos,
        worker_end_mask,
        worker_end_height,
        is_improving,
        is_now_lvl_2,
    }
}

pub(super) fn get_standard_reach_board<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    worker_move_state: &WorkerNextMoveState,
    worker_end_move_state: &WorkerEndMoveState,
    unblocked_squares: BitBoard,
) -> BitBoard {
    let next_turn_moves =
        NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize] & unblocked_squares;

    let reach_board = if prelude.is_against_hypnus
        && (worker_move_state.other_threatening_workers.count_ones()
            + worker_end_move_state.is_now_lvl_2)
            < 2
    {
        BitBoard::EMPTY
    } else {
        (worker_move_state.other_threatening_neighbors
            | (next_turn_moves * worker_end_move_state.is_now_lvl_2))
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
        | prelude.domes);
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
        | prelude.domes);
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
    let checkable_mask = prelude.exactly_level_2;
    let acting_workers = get_basic_acting_workers::<F>(&prelude);

    for worker_start_pos in acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
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
