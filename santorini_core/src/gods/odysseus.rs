use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, NUM_SQUARES, apply_mapping_to_mask},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_DATA_MAIN_SECTION,
            MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH,
            ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state_with_is_matched,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    search::Heuristic,
    square::Square,
};

use super::PartialAction;

// Odysseus takes an ordinary mortal turn, so bits 0..14 keep the mortal layout verbatim.
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

/// The power ("once per game, at the start of your turn, force any number of opponent workers
/// that neighbor your workers to unoccupied corners") lives in the 15 bits above the mortal move.
///
/// It is encoded per *corner* rather than per worker: there are only ever four corners, so four
/// 3 bit slots covers every possible activation in 12 bits. A slot holds `0` for "nothing was
/// forced here", or `1 + index` into the opponent's workers in ascending square order, read off
/// the *pre-displacement* board.
///
/// The index deliberately spans every opponent worker rather than just the forcible ones. Which
/// workers are forcible depends on the opposing god (Clio's coins sit on squares her own workers
/// can stand on, and those count as domes to us), and `get_blocker_board` has no `StaticGod` to
/// hand — so the cheaper basis is the one that needs nothing but the board.
///
/// Encoding per corner rather than per worker also makes the assignment injective for free — a
/// corner physically cannot hold two workers, and the encoding cannot express it either.
///
/// An all-zero displacement field means the power was not used, so no discriminant bit is needed:
/// a non-power Odysseus move is bit-identical to the equivalent mortal move.
const DISPLACEMENT_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const SLOT_WIDTH: usize = 3;
const SLOT_MASK: MoveData = (1 << SLOT_WIDTH) - 1;
const NUM_CORNERS: usize = 4;
const DISPLACEMENT_MASK: MoveData = ((1 << (SLOT_WIDTH * NUM_CORNERS)) - 1) << DISPLACEMENT_OFFSET;

/// A 3 bit slot can name at most 7 distinct workers. Every god places at most three workers except
/// Hydra, which grows its worker count without bound — hence the banned Odysseus/Hydra matchup in
/// `matchup.rs`. With that ban in place this limit is unreachable.
pub const MAX_ADDRESSABLE_WORKERS: usize = (1 << SLOT_WIDTH) - 1;

/// Slot order. Index `i` in a move's displacement field refers to `CORNER_SQUARES[i]`.
const CORNER_SQUARES: [Square; NUM_CORNERS] = [Square::A5, Square::E5, Square::A1, Square::E1];
const CORNERS: BitBoard = BitBoard(0b10001_00000_00000_00000_10001);

const POWER_USED: GodData = 1;

/// Eval bonus for still holding the power. See `eval_score_modifier`.
const POWER_AVAILABLE_EVAL_BONUS: Heuristic = 500;

const _LAYOUT_ASSERT: () = assert!(DISPLACEMENT_MASK & !MOVE_DATA_MAIN_SECTION == 0);
const _CORNER_ASSERT: () = assert!(
    CORNERS.0
        == (BitBoard::as_mask(Square::A5).0
            | BitBoard::as_mask(Square::E5).0
            | BitBoard::as_mask(Square::A1).0
            | BitBoard::as_mask(Square::E1).0)
);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct OdysseusMove(pub MoveData);

impl GodMove for OdysseusMove {
    fn move_to_actions(
        self,
        board: &BoardState,
        player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = Vec::new();

        let (displacements, count) = self.decode_displacements(board, player);
        for &(from, to) in &displacements[..count] {
            res.push(PartialAction::ForceOpponentWorker(from, to));
        }

        res.push(PartialAction::SelectWorker(self.move_from_position()));
        res.push(PartialAction::MoveWorker(self.move_to_position().into()));

        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        if self.has_displacement() {
            // The forcible-worker list is snapshotted before anything moves, so the whole
            // activation resolves simultaneously: a worker vacating a corner does not open that
            // corner up as a target within the same activation.
            let (displacements, count) = self.decode_displacements(board, player);
            for &(from, to) in &displacements[..count] {
                // One call per worker, never a batched xor: `oppo_worker_xor` re-points the
                // female-worker god_data by xoring it with the whole mask it is given, so a
                // combined mask would corrupt Selene's / Hippolyta's tracking.
                board.oppo_worker_xor(other_god, !player, from.to_board() ^ to.to_board());
            }
            board.set_god_data(player, POWER_USED);
        }

        board.worker_xor(player, self.move_mask());

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, board: &BoardState) -> BitBoard {
        let move_from = self.move_from_position();
        let mut res = BitBoard::as_mask(move_from) | BitBoard::as_mask(self.move_to_position());

        if self.has_displacement() {
            // Blocker boards are queried for the player who is *not* to move, so
            // `board.current_player` is the wrong player here. The mover is whoever owns the
            // worker standing on the move's from-square.
            let player = if board.workers[Player::One as usize].get_bit(move_from) {
                Player::One
            } else {
                Player::Two
            };

            // Only the corners this move actually uses. Covering all four would invite "blocking"
            // moves that dome an unused corner and change nothing.
            //
            // The squares the forced workers are dragged *off* are deliberately left out. Key
            // squares are matched against where a blocking move lands, so listing a vacated square
            // just invites the opponent to step aside and build on their own old square — which
            // does nothing to stop the forcing.
            let (displacements, count) = self.decode_displacements(board, player);
            for &(_, to) in &displacements[..count] {
                res |= to.to_board();
            }
        }

        res
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        // The displacement is intentionally not folded in — the power fires at most once per game,
        // so keying on it would only sparsify the history table.
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

impl Into<GenericMove> for OdysseusMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for OdysseusMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl OdysseusMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        displacement: MoveData,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | displacement;

        Self(data)
    }

    pub fn new_winning_move(
        move_from_position: Square,
        move_to_position: Square,
        displacement: MoveData,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | displacement
            | MOVE_IS_WINNING_MASK;

        Self(data)
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }

    pub fn has_displacement(self) -> bool {
        (self.0 & DISPLACEMENT_MASK) != 0
    }

    fn slot_value(self, slot: usize) -> MoveData {
        (self.0 >> (DISPLACEMENT_OFFSET + slot * SLOT_WIDTH)) & SLOT_MASK
    }

    /// Resolves the displacement field against `board`, which must be the position *before* the
    /// power resolves. Returns the (from, to) pairs and how many of them are populated.
    fn decode_displacements(
        self,
        board: &BoardState,
        player: Player,
    ) -> ([(Square, Square); NUM_CORNERS], usize) {
        let mut res = [(Square::A5, Square::A5); NUM_CORNERS];
        let mut count = 0;

        if !self.has_displacement() {
            return (res, count);
        }

        let (sources, source_count) = get_addressable_workers(board, player);

        for slot in 0..NUM_CORNERS {
            let value = self.slot_value(slot);
            if value == 0 {
                continue;
            }

            let worker_idx = (value - 1) as usize;
            // Transposition table moves are handed to `make_move` without being re-verified
            // against the position, so a hash collision can decode to an index that no longer
            // exists. Drop those rather than reading past the list.
            if worker_idx >= source_count {
                continue;
            }

            res[count] = (sources[worker_idx], CORNER_SQUARES[slot]);
            count += 1;
        }

        (res, count)
    }
}

impl std::fmt::Debug for OdysseusMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        if self.has_displacement() {
            // No board here, so forced workers print as their list index rather than their square.
            write!(f, "(")?;
            let mut is_first = true;
            for slot in 0..NUM_CORNERS {
                let value = self.slot_value(slot);
                if value == 0 {
                    continue;
                }
                if !is_first {
                    write!(f, ",")?;
                }
                is_first = false;
                write!(f, "w{}>{}", value - 1, CORNER_SQUARES[slot])?;
            }
            write!(f, ")")?;
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, self.build_position())
        }
    }
}

/// The opponent's workers in ascending square order — the basis a move's slot values index into.
fn get_addressable_workers(
    board: &BoardState,
    player: Player,
) -> ([Square; MAX_ADDRESSABLE_WORKERS], usize) {
    let mut squares = [Square::A5; MAX_ADDRESSABLE_WORKERS];
    let mut count = 0;

    for square in board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK {
        if count == MAX_ADDRESSABLE_WORKERS {
            break;
        }
        squares[count] = square;
        count += 1;
    }

    (squares, count)
}

/// Opponent workers that may actually be forced: those neighboring one of `player`'s workers.
///
/// Adjacency here is plain physical adjacency, so it deliberately ignores the movement-warping
/// neighbor maps (Aeolus' wind, Hippolyta's diagonals) that the prelude uses.
///
/// Frozen squares are excluded. That covers Europa's Talus, and it means a Clio worker standing on
/// one of her own coins cannot be dragged out — to us that space reads as a dome, so we cannot
/// reach into it.
pub(crate) fn get_forcible_workers(
    board: &BoardState,
    player: Player,
    other_god: StaticGod,
) -> BitBoard {
    let own_workers = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
    let oppo_workers = board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK;
    let frozen = other_god.get_frozen_mask(board, !player);

    oppo_workers & apply_mapping_to_mask(own_workers, &NEIGHBOR_MAP) & !frozen
}

/// The same move with its forcing stripped off, for callers that want to count *distinct* wins.
///
/// One underlying win can appear once per free corner — `(w0>A1)D3>C3#` and `(w0>E1)D3>C3#` are
/// the same threat carried out two ways — so a raw count of winning moves says more about how many
/// corners happen to be open than about how many ways there are to win.
pub(crate) fn strip_displacement(action: GenericMove) -> GenericMove {
    GenericMove::new(action.0 & !DISPLACEMENT_MASK)
}

/// Whether an Odysseus player still holds the power *and* has somewhere to put a worker.
///
/// Used by other gods to decide whether their own blocking tricks survive contact with Odysseus.
/// Frozen squares are not considered, so this is only exact for opponents that have none — which
/// covers every caller today.
pub(super) fn power_is_live(board: &BoardState, odysseus_player: Player) -> bool {
    board.god_data[odysseus_player as usize] == 0
        && (CORNERS & !(board.workers[0] | board.workers[1]) & !board.at_least_level_4())
            .is_not_empty()
}

/// Corners a worker may be forced onto. Domed corners count as occupied, as do corners holding a
/// worker of either colour or a frozen token.
fn get_free_corners(board: &BoardState, player: Player, other_god: StaticGod) -> BitBoard {
    let occupied = board.workers[0]
        | board.workers[1]
        | board.at_least_level_4()
        | other_god.get_frozen_mask(board, !player);

    CORNERS & !occupied & BitBoard::MAIN_SECTION_MASK
}

/// Every non-empty assignment of forcible workers to free corners, as pre-shifted displacement
/// fields. The empty assignment is skipped because it is already covered by the ordinary moves.
///
/// Workers are bits in a bitboard, so two workers of the same kind are interchangeable: sending
/// `w0` to A1 and `w1` to E1 lands on exactly the same position as the reverse. To avoid emitting
/// both, assignments are canonicalised to be monotone in worker index *within a kind* — corner
/// slots are filled left to right, and each newly forced worker of a given kind must have a higher
/// index than the last one of that kind.
///
/// `female_worker_mask` marks (by worker index) the workers that are actually distinguishable,
/// i.e. Selene's and Hippolyta's female worker, whose square is tracked in god_data. Swapping her
/// with a male worker does produce a different position, so those permutations are all kept.
///
/// `forcible_mask` marks (by worker index) which addressable workers may be forced at all.
fn collect_displacements(
    free_corners: BitBoard,
    num_addressable: usize,
    forcible_mask: u32,
    female_worker_mask: u32,
) -> Vec<MoveData> {
    let mut result = Vec::new();
    collect_displacements_inner(
        0,
        free_corners,
        [-1; 2],
        num_addressable,
        forcible_mask,
        female_worker_mask,
        0,
        &mut result,
    );
    result
}

fn collect_displacements_inner(
    slot: usize,
    free_corners: BitBoard,
    last_worker_of_kind: [i32; 2],
    num_addressable: usize,
    forcible_mask: u32,
    female_worker_mask: u32,
    acc: MoveData,
    result: &mut Vec<MoveData>,
) {
    if slot == NUM_CORNERS {
        if acc != 0 {
            result.push(acc);
        }
        return;
    }

    // Leave this corner alone.
    collect_displacements_inner(
        slot + 1,
        free_corners,
        last_worker_of_kind,
        num_addressable,
        forcible_mask,
        female_worker_mask,
        acc,
        result,
    );

    if !free_corners.get_bit(CORNER_SQUARES[slot]) {
        return;
    }

    for worker_idx in 0..num_addressable {
        if forcible_mask & (1 << worker_idx) == 0 {
            continue;
        }

        let kind = ((female_worker_mask >> worker_idx) & 1) as usize;
        // Strict monotonicity within a kind also stops a worker being forced onto two corners.
        if worker_idx as i32 <= last_worker_of_kind[kind] {
            continue;
        }

        let mut next_last_worker_of_kind = last_worker_of_kind;
        next_last_worker_of_kind[kind] = worker_idx as i32;

        let slot_bits = ((worker_idx as MoveData) + 1) << (DISPLACEMENT_OFFSET + slot * SLOT_WIDTH);
        collect_displacements_inner(
            slot + 1,
            free_corners,
            next_last_worker_of_kind,
            num_addressable,
            forcible_mask,
            female_worker_mask,
            acc | slot_bits,
            result,
        );
    }
}

/// Generates the mortal move/build half of the turn from `state`, tagging every emitted move with
/// `displacement`.
///
/// For power moves `state` is the position *after* the forcing has resolved, which is what makes
/// the opponent-god interactions fall out for free: Limus' build mask, Aphrodite's affinity area
/// and Athena's climb flag are all recomputed from the displaced position.
///
/// `powerless_wins` maps a worker's start square to the destinations it already wins on without
/// spending the power. The base pass records into it; the displaced passes subtract it, so a
/// win that was available anyway never gets a redundant "and also fling somebody into a corner"
/// variant. Besides being pointless play, those variants make every corner look like a blocking
/// square to the opponent's search.
///
/// Returns true when the caller should stop generating (a mate was found under `STOP_ON_MATE`).
fn generate_from_state<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
    displacement: MoveData,
    displacement_matched_key_squares: bool,
    powerless_wins: &mut [BitBoard; NUM_SQUARES],
    result: &mut Vec<ScoredMove>,
) -> bool {
    let is_powerless_pass = displacement == 0;
    // A move that spends the power leaves nothing for next turn, so only the powerless pass gets
    // the widened check detection below.
    let power_available_next_turn = is_powerless_pass && state.board.god_data[player as usize] == 0;
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

            let emitted_wins = if is_powerless_pass {
                powerless_wins[worker_start_pos as usize] |= moves_to_level_3;
                moves_to_level_3
            } else {
                moves_to_level_3 & !powerless_wins[worker_start_pos as usize]
            };

            if push_winning_moves::<F, OdysseusMove, _>(
                result,
                worker_start_pos,
                emitted_wins,
                |move_from, move_to| {
                    OdysseusMove::new_winning_move(move_from, move_to, displacement)
                },
            ) {
                return true;
            }
            worker_next_moves.worker_moves ^= moves_to_level_3;
        }

        if is_mate_only::<F>() {
            continue;
        }

        for worker_end_pos in worker_next_moves.worker_moves {
            let worker_end_move_state =
                get_worker_end_move_state::<F>(&prelude, &worker_start_state, worker_end_pos);

            // The forcing itself can be the thing that interacts with a key square (dragging an
            // opponent worker off the square it needed to win from), in which case the move and
            // build are unconstrained.
            let is_key_squares_matched = displacement_matched_key_squares
                || (worker_end_move_state.worker_end_mask & prelude.key_squares).is_not_empty();

            let worker_next_build_state = get_worker_next_build_state_with_is_matched::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
                is_key_squares_matched,
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            // An opponent worker parked on the level 3 square we want is not really in the way
            // while the power is still in hand: next turn we fling it into a corner and step up.
            // So for check detection those workers count as empty space, as long as a corner will
            // still be free for them to land on.
            let (power_reach_board, free_corners_next_turn) = if power_available_next_turn {
                let own_workers_after_move =
                    worker_start_state.other_own_workers | worker_end_move_state.worker_end_mask;
                let forcible_next_turn = prelude.oppo_workers
                    & apply_mapping_to_mask(own_workers_after_move, &NEIGHBOR_MAP)
                    & !prelude.domes_and_frozen;
                let free_corners_next_turn = CORNERS
                    & !(own_workers_after_move | prelude.oppo_workers)
                    & !prelude.domes_and_frozen;

                let power_reach_board = get_standard_reach_board::<F>(
                    &prelude,
                    &worker_next_moves,
                    &worker_end_move_state,
                    worker_next_build_state.unblocked_squares | forcible_next_turn,
                );

                (power_reach_board, free_corners_next_turn)
            } else {
                (reach_board, BitBoard::EMPTY)
            };

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = OdysseusMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    displacement,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);

                    // Capping off a level 3 corner removes it as a landing square.
                    let corners_left =
                        free_corners_next_turn & !(build_mask & prelude.exactly_level_3);
                    let check_board = if corners_left.is_not_empty() {
                        power_reach_board
                    } else {
                        reach_board
                    } & final_level_3;
                    check_board.is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    false
}

fn apply_displacement(
    board: &mut BoardState,
    player: Player,
    other_god: StaticGod,
    displacement: MoveData,
) {
    let action = OdysseusMove(displacement);
    let (displacements, count) = action.decode_displacements(board, player);
    for &(from, to) in &displacements[..count] {
        board.oppo_worker_xor(other_god, !player, from.to_board() ^ to.to_board());
    }
}

pub(super) fn odysseus_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(odysseus_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut powerless_wins = [BitBoard::EMPTY; NUM_SQUARES];
    if generate_from_state::<F, MUST_CLIMB>(
        state,
        player,
        key_squares,
        0,
        false,
        &mut powerless_wins,
        &mut result,
    ) {
        return result;
    }

    if state.board.god_data[player as usize] != 0 {
        return result;
    }

    let other_god = state.gods[!player as usize];
    let forcible = get_forcible_workers(&state.board, player, other_god);
    let free_corners = get_free_corners(&state.board, player, other_god);
    if forcible.is_empty() || free_corners.is_empty() {
        return result;
    }

    debug_assert!(
        (state.board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK).count_ones() as usize
            <= MAX_ADDRESSABLE_WORKERS,
        "Odysseus cannot address more than {} opponent workers (matchup should be banned)",
        MAX_ADDRESSABLE_WORKERS
    );

    let (addressable, num_addressable) = get_addressable_workers(&state.board, player);
    let female_workers = other_god.get_female_worker_mask(&state.board, !player);

    let mut forcible_mask = 0u32;
    let mut female_worker_mask = 0u32;
    for (worker_idx, &square) in addressable[..num_addressable].iter().enumerate() {
        if forcible.get_bit(square) {
            forcible_mask |= 1 << worker_idx;
        }
        if female_workers.get_bit(square) {
            female_worker_mask |= 1 << worker_idx;
        }
    }

    for displacement in collect_displacements(
        free_corners,
        num_addressable,
        forcible_mask,
        female_worker_mask,
    ) {
        let displacement_matched_key_squares = if is_interact_with_key_squares::<F>() {
            let action = OdysseusMove(displacement);
            let (displacements, count) = action.decode_displacements(&state.board, player);
            let mut touched = BitBoard::EMPTY;
            for &(from, to) in &displacements[..count] {
                touched |= from.to_board() | to.to_board();
            }
            (touched & key_squares).is_not_empty()
        } else {
            false
        };

        let mut displaced_state = state.clone();
        apply_displacement(&mut displaced_state.board, player, other_god, displacement);

        if generate_from_state::<F, MUST_CLIMB>(
            &displaced_state,
            player,
            key_squares,
            displacement,
            displacement_matched_key_squares,
            &mut powerless_wins,
            &mut result,
        ) {
            return result;
        }
    }

    result
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        "x" | "X" => Ok(POWER_USED),
        _ => Err(format!("Must be either empty string or x")),
    }
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        _ => Some(format!("x")),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => Some(format!("Power available")),
        _ => Some(format!("Power used")),
    }
}

/// Odysseus proxies Mortal's NNUE weights and contributes no god_data features, so the network
/// cannot see whether the power is still in hand — left alone the search happily spends it on move
/// one for a trivial positional gain. This hand-set bonus stands in for the option value until an
/// NNUE is trained with `god_name_to_nnue_size(Odysseus) = 1`.
fn eval_score_modifier(god_data: GodData) -> Heuristic {
    match god_data {
        0 => POWER_AVAILABLE_EVAL_BONUS,
        _ => 0,
    }
}

pub const fn build_odysseus() -> GodPower {
    god_power(
        GodName::Odysseus,
        build_god_power_movers!(odysseus_move_gen),
        build_god_power_actions::<OdysseusMove>(),
        11398760139457466666,
        8756408457443776940,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_eval_score_modifier_fn(eval_score_modifier)
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::{
        board::GameStateBuilder, fen::parse_fen, matchup::Matchup, matchup::is_matchup_banned,
        move_verifier::MoveVerifier, square::Square::*,
    };

    use super::*;

    #[test]
    fn test_odysseus_displacement_does_not_disturb_the_mortal_move() {
        // The displacement field must live entirely above the mortal from/to/build bits, and
        // entirely below the check/winning flags.
        let plain = OdysseusMove::new_basic_move(A1, B2, C3, 0);
        let displacement = (7 as MoveData) << DISPLACEMENT_OFFSET
            | (7 as MoveData) << (DISPLACEMENT_OFFSET + 3 * SLOT_WIDTH);
        let forced = OdysseusMove::new_basic_move(A1, B2, C3, displacement);

        assert_eq!(forced.move_from_position(), A1);
        assert_eq!(forced.move_to_position(), B2);
        assert_eq!(forced.build_position(), C3);
        assert!(!plain.has_displacement());
        assert!(forced.has_displacement());
        assert_eq!(forced.0 & !DISPLACEMENT_MASK, plain.0);
        assert_eq!(forced.0 & MOVE_DATA_MAIN_SECTION, forced.0);
    }

    #[test]
    fn test_odysseus_wins_by_forcing_a_worker_off_the_winning_square() {
        // B1 is at height 3 with an opponent worker parked on it, and Odysseus sits next to it at
        // height 2. He cannot climb into an occupied space, so the only win is to fling that
        // worker into a corner first.
        let state = GameStateBuilder::new(GodName::Odysseus, GodName::Mortal)
            .with_p1_worker(C1)
            .with_p1_worker(E5)
            .with_p2_worker(B1)
            .with_p2_worker(E3)
            .with_height(C1, 2)
            .with_height(B1, 3)
            .build();

        let next_states = state.get_next_states_interactive();

        MoveVerifier::new()
            .is_winner(Player::One)
            .with_p1_worker_at(B1)
            .any(&next_states);

        // The forced worker has to have gone to a corner, and A1 is the only one both free and
        // not occupied by Odysseus himself (E5 holds his other worker).
        MoveVerifier::new()
            .is_winner(Player::One)
            .with_p1_worker_at(B1)
            .with_p2_worker_at(A1)
            .any(&next_states);
    }

    #[test]
    fn test_odysseus_forces_two_workers_in_one_turn() {
        // Both opponent workers neighbor Odysseus, so both can be flung at once.
        let state = GameStateBuilder::new(GodName::Odysseus, GodName::Mortal)
            .with_p1_worker(C3)
            .with_p1_worker(C1)
            .with_p2_worker(B3)
            .with_p2_worker(D3)
            .build();

        let next_states = state.get_next_states_interactive();

        MoveVerifier::new()
            .with_p2_worker_at(A5)
            .with_p2_worker_at(E5)
            .any(&next_states);

        // ...and forcing only one of them is legal too ("any number").
        MoveVerifier::new()
            .with_p2_worker_at(A5)
            .with_p2_worker_at(D3)
            .any(&next_states);
    }

    #[test]
    fn test_odysseus_only_forces_neighboring_workers() {
        // D1 is nowhere near either Odysseus worker, so it stays put no matter what.
        let state = GameStateBuilder::new(GodName::Odysseus, GodName::Mortal)
            .with_p1_worker(A4)
            .with_p1_worker(A3)
            .with_p2_worker(A5)
            .with_p2_worker(D1)
            .build();

        let next_states = state.get_next_states_interactive();

        MoveVerifier::new()
            .without_p2_worker_at(D1)
            .none(&next_states);
        // The neighboring worker on A5 is already on a corner, but can still be moved to another.
        MoveVerifier::new().with_p2_worker_at(E5).any(&next_states);
    }

    #[test]
    fn test_odysseus_cannot_force_onto_domed_or_occupied_corners() {
        // A5 is domed, E5 and E1 hold Odysseus' own workers, A1 holds the other opponent worker.
        // With no corner free the power cannot be used at all.
        let state = GameStateBuilder::new(GodName::Odysseus, GodName::Mortal)
            .with_p1_worker(E5)
            .with_p1_worker(E1)
            .with_p2_worker(D1)
            .with_p2_worker(A1)
            .with_height(A5, 4)
            .build();

        let next_states = state.get_next_states_interactive();

        for next_state in &next_states {
            assert_eq!(
                next_state.state.board.workers[1], state.board.workers[1],
                "No corner is free, so no opponent worker should ever move"
            );
        }
    }

    #[test]
    fn test_odysseus_power_is_once_per_game() {
        // Same shape as the two-worker test, but god_data already marks the power as spent.
        let state = parse_fen("0000000000000000000000000/1/odysseus[x]:C3,C1/mortal:B3,D3")
            .expect("fen should parse");

        let next_states = state.get_next_states_interactive();

        for next_state in &next_states {
            assert_eq!(
                next_state.state.board.workers[1], state.board.workers[1],
                "The power is spent, so no opponent worker should move"
            );
        }
    }

    #[test]
    fn test_odysseus_god_data_round_trips_through_fen() {
        let fen = "0000000000000000000000000/1/odysseus[x]:C3,C1/mortal:B3,D3";
        let state = parse_fen(fen).expect("fen should parse");
        assert_eq!(state.board.god_data[0], POWER_USED);
        assert_eq!(crate::fen::game_state_to_fen(&state), fen);
    }

    #[test]
    fn test_odysseus_hydra_matchup_is_banned() {
        // Hydra grows past the 7 workers a 3 bit slot can name.
        assert!(is_matchup_banned(&Matchup::new(
            GodName::Odysseus,
            GodName::Hydra
        )));
    }
}
