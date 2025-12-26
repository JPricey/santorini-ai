use itertools::Itertools;

use crate::{
    bitboard::{BitBoard, LOWER_SQUARES_EXCLUSIVE_MASK, NEIGHBOR_MAP, UPPER_SPACES_INCLUSIVE_MAP},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            GeneratorPreludeState, build_scored_move, get_generator_prelude_state,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const B1_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const B2_OFFSET: usize = B1_OFFSET + POSITION_WIDTH;
const B3_OFFSET: usize = B2_OFFSET + POSITION_WIDTH;

#[derive(Copy, Clone, PartialEq, Eq)]
struct PoseidonMove(pub MoveData);

fn _perms_no_dupes(squares: &Vec<Square>) -> Vec<Vec<Square>> {
    let result: Vec<Vec<Square>> = squares
        .iter()
        .cloned()
        .permutations(squares.len())
        .unique()
        .collect();
    result
}

fn _get_poseidon_build_orders(
    moving_worker: Square,
    standing_worker: Square,
    builds: Vec<Square>,
) -> Vec<Vec<Square>> {
    let mut results = Vec::new();

    let mut all_builds = BitBoard::EMPTY;
    for build in &builds {
        all_builds |= build.to_board();
    }

    let moving_worker_builds = NEIGHBOR_MAP[moving_worker as usize];
    let standing_worker_builds = NEIGHBOR_MAP[standing_worker as usize];

    let can_go_first = moving_worker_builds & all_builds;
    let must_go_first = all_builds & !standing_worker_builds;
    assert!(must_go_first.count_ones() <= 1);

    let go_first_builds = if must_go_first.count_ones() == 0 {
        can_go_first
    } else {
        must_go_first
    };

    for first_build in go_first_builds {
        let mut this_builds = builds.clone();
        let idx: usize = this_builds
            .iter()
            .cloned()
            .find_position(|b| *b == first_build)
            .unwrap()
            .0;
        this_builds.remove(idx);

        let mut rest_builds = _perms_no_dupes(&this_builds);
        for suffix in &mut rest_builds {
            suffix.insert(0, first_build);
        }
        results.extend(rest_builds);
    }

    results
}

impl GodMove for PoseidonMove {
    fn move_to_actions(
        self,
        board: &BoardState,
        player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        if let Some(b1) = self.maybe_b1() {
            let standing_worker =
                (board.workers[player as usize] & !self.move_from_position().to_board()).lsb();
            let mut builds = Vec::with_capacity(4);
            builds.push(self.build_position());

            builds.push(b1);
            if let Some(b2) = self.maybe_b2() {
                builds.push(b2);
                if let Some(b3) = self.maybe_b3() {
                    builds.push(b3);
                }
            }

            let all_build_orders =
                _get_poseidon_build_orders(self.move_to_position(), standing_worker, builds);

            all_build_orders
                .iter()
                .map(|build_orders| {
                    let mut res = res.clone();
                    for build in build_orders {
                        res.push(PartialAction::Build(*build));
                    }
                    res
                })
                .collect_vec()
        } else {
            res.push(PartialAction::Build(self.build_position()));
            vec![res]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let worker_move_mask = self.move_mask();
        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());

        if let Some(b1) = self.maybe_b1() {
            board.build_up(b1);
            if let Some(b2) = self.maybe_b2() {
                board.build_up(b2);
                if let Some(b3) = self.maybe_b3() {
                    board.build_up(b3);
                }
            }
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        if let Some(b1) = self.maybe_b1() {
            helper.add_known_maybe_square_with_height(board, b1);

            if let Some(b2) = self.maybe_b2() {
                helper.add_known_maybe_square_with_height(board, b2);

                if let Some(b3) = self.maybe_b3() {
                    helper.add_known_maybe_square_with_height(board, b3);
                }
            }
        }
        helper.get()
    }
}

impl Into<GenericMove> for PoseidonMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for PoseidonMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl PoseidonMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((25 as MoveData) << B1_OFFSET);

        Self(data)
    }

    fn new_b1_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        b1_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((b1_position as MoveData) << B1_OFFSET)
            | ((25 as MoveData) << B2_OFFSET);

        Self(data)
    }

    fn new_b2_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        b1_position: Square,
        b2_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((b1_position as MoveData) << B1_OFFSET)
            | ((b2_position as MoveData) << B2_OFFSET)
            | ((25 as MoveData) << B3_OFFSET);

        Self(data)
    }

    fn new_b3_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        b1_position: Square,
        b2_position: Square,
        b3_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((b1_position as MoveData) << B1_OFFSET)
            | ((b2_position as MoveData) << B2_OFFSET)
            | ((b3_position as MoveData) << B3_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    fn move_to_position(&self) -> Square {
        Square::from((self.0 >> POSITION_WIDTH) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn maybe_b1(self) -> Option<Square> {
        let value = (self.0 >> B1_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn maybe_b2(self) -> Option<Square> {
        let value = (self.0 >> B2_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn maybe_b3(self) -> Option<Square> {
        let value = (self.0 >> B3_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == 25 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for PoseidonMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        let build = self.build_position();
        let is_win = self.get_is_winning();

        if is_win {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            if let Some(b1) = self.maybe_b1() {
                if let Some(b2) = self.maybe_b2() {
                    if let Some(b3) = self.maybe_b3() {
                        return write!(
                            f,
                            "{}>{}^{} ^{}^{}^{}",
                            move_from, move_to, build, b1, b2, b3
                        );
                    }
                    return write!(f, "{}>{}^{} ^{}^{}", move_from, move_to, build, b1, b2);
                }
                return write!(f, "{}>{}^{} ^{}", move_from, move_to, build, b1);
            }
            return write!(f, "{}>{}^{}", move_from, move_to, build);
        }
    }
}

fn _add_b3_continuation<const F: MoveGenFlags>(
    _prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    b0: Square,
    b1: Square,
    b2: Square,
    b2_height: usize,
    checking_reach: BitBoard,
    already_checking: BitBoard,
) {
    if b2_height >= 4 {
        return;
    }
    let b2_mask = b2.to_board();

    let action = PoseidonMove::new_b3_move(start_pos, end_pos, b0, b1, b2, b2);

    let already_checking = if b2_height == 2 {
        already_checking | checking_reach & b2_mask
    } else if b2_height == 3 {
        already_checking & !b2_mask
    } else {
        already_checking
    };

    result.push(build_scored_move::<F, _>(
        action,
        already_checking.is_not_empty(),
        false,
    ));
}

fn _add_b3<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    b0: Square,
    b1: Square,
    b2: Square,
    possible_ground_builds: BitBoard,
    checking_reach: BitBoard,
    already_checking: BitBoard,
    key_squares: BitBoard,
    did_already_block: bool,
) {
    let possible_builds = if is_interact_with_key_squares::<F>() && !did_already_block {
        possible_ground_builds & key_squares
    } else {
        possible_ground_builds
    };

    for b3 in possible_builds {
        let b3_mask = b3.to_board();
        let already_checking =
            already_checking & !b3_mask | checking_reach & prelude.exactly_level_2 & b3_mask;

        let action = PoseidonMove::new_b3_move(start_pos, end_pos, b0, b1, b2, b3);
        result.push(build_scored_move::<F, _>(
            action,
            already_checking.is_not_empty(),
            false,
        ));
    }
}

fn _add_b2_continuation<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    b0: Square,
    b1: Square,
    b1_height: usize,
    possible_ground_builds: BitBoard,
    checking_reach: BitBoard,
    already_checking: BitBoard,
    key_squares: BitBoard,
    did_already_block: bool,
) {
    if b1_height >= 4 {
        return;
    }

    let b1_mask = b1.to_board();
    let already_checking = if b1_height == 2 {
        already_checking | checking_reach & b1_mask
    } else if b1_height == 3 {
        already_checking & !b1_mask
    } else {
        already_checking
    };

    if !is_interact_with_key_squares::<F>() || did_already_block {
        let action = PoseidonMove::new_b2_move(start_pos, end_pos, b0, b1, b1);
        result.push(build_scored_move::<F, _>(
            action,
            already_checking.is_not_empty(),
            false,
        ));

        _add_b3_continuation::<F>(
            prelude,
            result,
            start_pos,
            end_pos,
            b0,
            b1,
            b1,
            b1_height + 1,
            checking_reach,
            already_checking,
        );
    }

    _add_b3::<F>(
        prelude,
        result,
        start_pos,
        end_pos,
        b0,
        b1,
        b1,
        possible_ground_builds,
        checking_reach,
        already_checking,
        key_squares,
        did_already_block,
    );
}

fn _add_b2<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    b0: Square,
    b1: Square,
    possible_ground_builds: BitBoard,
    checking_reach: BitBoard,
    already_checking: BitBoard,
    key_squares: BitBoard,
    did_already_block: bool,
) {
    for b2 in possible_ground_builds {
        let b2_height = prelude.board.get_height(b2);

        let b2_mask = b2.to_board();
        let already_checking =
            already_checking & !b2_mask | checking_reach & prelude.exactly_level_2 & b2_mask;

        let did_already_block = did_already_block || (key_squares & b2_mask).is_not_empty();

        if !is_interact_with_key_squares::<F>() || did_already_block {
            let action = PoseidonMove::new_b2_move(start_pos, end_pos, b0, b1, b2);
            result.push(build_scored_move::<F, _>(
                action,
                already_checking.is_not_empty(),
                false,
            ));

            _add_b3_continuation::<F>(
                prelude,
                result,
                start_pos,
                end_pos,
                b0,
                b1,
                b2,
                b2_height + 1,
                checking_reach,
                already_checking,
            );
        }
    }
}

fn _add_b1_continuation<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    b0: Square,
    b0_height: usize,
    possible_ground_builds: BitBoard,
    checking_reach: BitBoard,
    already_checking: BitBoard,
    key_squares: BitBoard,
    did_already_block: bool,
) {
    if b0_height >= 4 {
        return;
    }
    let b0_mask = b0.to_board();

    let already_checking = if b0_height == 2 {
        already_checking | checking_reach & b0_mask
    } else if b0_height == 3 {
        already_checking & !b0_mask
    } else {
        already_checking
    };

    if !is_interact_with_key_squares::<F>() || did_already_block {
        let action = PoseidonMove::new_b1_move(start_pos, end_pos, b0, b0);
        result.push(build_scored_move::<F, _>(
            action,
            already_checking.is_not_empty(),
            false,
        ));
    }

    _add_b2_continuation::<F>(
        prelude,
        result,
        start_pos,
        end_pos,
        b0,
        b0,
        b0_height + 1,
        possible_ground_builds,
        checking_reach,
        already_checking,
        key_squares,
        did_already_block,
    );

    _add_b2::<F>(
        prelude,
        result,
        start_pos,
        end_pos,
        b0,
        b0,
        possible_ground_builds,
        checking_reach,
        already_checking,
        key_squares,
        did_already_block,
    );
}

fn _add_b1<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    b0: Square,
    possible_ground_builds: BitBoard,
    checking_reach: BitBoard,
    already_checking: BitBoard,
    key_squares: BitBoard,
    did_already_block: bool,
) {
    for b1 in possible_ground_builds {
        let b1_height = prelude.board.get_height(b1);

        let next_builds = possible_ground_builds & LOWER_SQUARES_EXCLUSIVE_MASK[b1 as usize];

        let b1_mask = b1.to_board();
        let already_checking =
            already_checking & !b1_mask | checking_reach & prelude.exactly_level_2 & b1_mask;

        let did_already_block = did_already_block || (b1_mask & key_squares).is_not_empty();

        if !is_interact_with_key_squares::<F>() || did_already_block {
            let action = PoseidonMove::new_b1_move(start_pos, end_pos, b0, b1);
            result.push(build_scored_move::<F, _>(
                action,
                already_checking.is_not_empty(),
                false,
            ));
        }

        _add_b2_continuation::<F>(
            prelude,
            result,
            start_pos,
            end_pos,
            b0,
            b1,
            b1_height + 1,
            next_builds,
            checking_reach,
            already_checking,
            key_squares,
            did_already_block,
        );

        _add_b2::<F>(
            prelude,
            result,
            start_pos,
            end_pos,
            b0,
            b1,
            next_builds,
            checking_reach,
            already_checking,
            key_squares,
            did_already_block,
        );
    }
}

fn _add_poseidon_special_builds<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
    start_pos: Square,
    end_pos: Square,
    ground_builder: Square,
    unblocked_squares: BitBoard,
    all_possible_builds: BitBoard,
    checking_reach: BitBoard,
    key_squares: BitBoard,
    did_already_block: bool,
) {
    let possible_ground_builds =
        NEIGHBOR_MAP[ground_builder as usize] & unblocked_squares & prelude.build_mask;

    let both_builds = possible_ground_builds & all_possible_builds;
    let exclusive_first_builds = all_possible_builds ^ both_builds;

    for b0 in both_builds {
        let b0_height = prelude.board.get_height(b0);
        let next_builds =
            possible_ground_builds & !(both_builds & UPPER_SPACES_INCLUSIVE_MAP[b0 as usize]);

        let b0_mask = b0.to_board();
        let exactly_level_3 =
            prelude.exactly_level_2 & b0_mask | prelude.exactly_level_3 & !b0_mask;
        let already_checking = checking_reach & exactly_level_3;

        let did_already_block = did_already_block || (key_squares & b0_mask).is_not_empty();

        _add_b1::<F>(
            prelude,
            result,
            start_pos,
            end_pos,
            b0,
            next_builds,
            checking_reach,
            already_checking,
            key_squares,
            did_already_block,
        );

        _add_b1_continuation::<F>(
            prelude,
            result,
            start_pos,
            end_pos,
            b0,
            b0_height + 1,
            next_builds & !b0.to_board(),
            checking_reach,
            already_checking,
            key_squares,
            did_already_block,
        );
    }

    for b0 in exclusive_first_builds {
        let b0_mask = b0.to_board();
        let exactly_level_3 =
            prelude.exactly_level_2 & b0_mask | prelude.exactly_level_3 & !b0_mask;
        let already_checking = checking_reach & exactly_level_3;

        let did_already_block = did_already_block || (key_squares & b0_mask).is_not_empty();

        _add_b1::<F>(
            prelude,
            result,
            start_pos,
            end_pos,
            b0,
            possible_ground_builds,
            checking_reach,
            already_checking,
            key_squares,
            did_already_block,
        );
    }
}

fn poseidon_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(poseidon_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, PoseidonMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                PoseidonMove::new_winning_move,
            ) {
                return result;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        let other_ground_workers = worker_start_state.other_own_workers & prelude.exactly_level_0;
        let maybe_poseidon_special_build = other_ground_workers.maybe_lsb();

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

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = PoseidonMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(new_action, is_check, false))
            }

            if let Some(ground_builder) = maybe_poseidon_special_build {
                _add_poseidon_special_builds::<F>(
                    &prelude,
                    &mut result,
                    worker_start_state.worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    ground_builder,
                    worker_next_build_state.unblocked_squares,
                    worker_next_build_state.all_possible_builds,
                    reach_board,
                    key_squares,
                    (key_squares
                        & (worker_start_state.worker_start_mask
                            | worker_end_move_state.worker_end_mask))
                        .is_not_empty(),
                );
            }
        }
    }

    result
}

pub(crate) const fn build_poseidon() -> GodPower {
    god_power(
        GodName::Poseidon,
        build_god_power_movers!(poseidon_move_gen),
        build_god_power_actions::<PoseidonMove>(),
        39626542716481940,
        12412485317668298438,
    )
    .with_nnue_god_name(GodName::Mortal)
}
