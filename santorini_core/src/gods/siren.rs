use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    build_god_power_movers,
    gods::{
        BoardSymmetry, FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod,
        build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_DATA_MAIN_SECTION,
            MOVE_IS_WINNING_MASK, MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH,
            ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_build_state, get_worker_next_move_state,
            get_worker_start_move_state, is_interact_with_key_squares, is_mate_only,
            modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    persephone_check_result,
    player::Player,
    square::Square,
};

use super::PartialAction;

// An ordinary Siren turn is a mortal one, so bits 0..14 keep the mortal layout verbatim. On a song
// turn nothing of hers moves, and the from-slot names the worker that builds instead.
const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

/// The to-slot holds 25 - one past the last square - on a song turn. That is the discriminant: an
/// ordinary Siren move is bit-identical to the equivalent mortal move.
const NO_MOVE_SENTINEL: MoveData = 25;

/// Which opponent workers the song drags along, as a bitmask over the opponent's workers in
/// ascending square order, read off the *pre-song* board.
///
/// Naming workers by index rather than by square is what keeps the field this narrow, and the
/// basis needs nothing but the board - no `StaticGod` - which is what `get_blocker_board` and
/// `get_history_idx` have to work with. Every worker is addressable, not just the forcible ones:
/// which ones can actually be dragged depends on the opposing god, since Clio's coins and Europa's
/// Talus read to us as domes.
///
/// The song always blows one square down, so a worker's destination follows from its source and
/// does not need storing.
const FORCE_MASK_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const FORCE_MASK_WIDTH: usize = 5;
const FORCE_MASK: MoveData = ((1 << FORCE_MASK_WIDTH) - 1) << FORCE_MASK_OFFSET;

/// A song names its workers by bit position, so the mask can only address this many. Every god
/// places at most three workers except Hydra, who grows past that without bound - hence the banned
/// Siren/Hydra matchup in `matchup.rs`. With that ban in place the limit is unreachable.
pub const MAX_ADDRESSABLE_WORKERS: usize = FORCE_MASK_WIDTH;

const _LAYOUT_ASSERT: () = assert!(FORCE_MASK & !MOVE_DATA_MAIN_SECTION == 0);

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SirenMove(pub MoveData);

impl Into<GenericMove> for SirenMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for SirenMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl SirenMove {
    pub fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET);

        Self(data)
    }

    pub fn new_song_move(
        builder_position: Square,
        build_position: Square,
        force_mask: MoveData,
    ) -> Self {
        let data: MoveData = ((builder_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | (NO_MOVE_SENTINEL << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | (force_mask << FORCE_MASK_OFFSET);

        Self(data)
    }

    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    /// The worker that acts: the one that steps on an ordinary turn, the one that builds on a song
    /// turn.
    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    /// `None` on a song turn, where none of her workers moves.
    pub fn maybe_move_to_position(&self) -> Option<Square> {
        let value =
            (self.0 >> MOVE_TO_POSITION_OFFSET) as MoveData & LOWER_POSITION_MASK as MoveData;
        if value == NO_MOVE_SENTINEL {
            None
        } else {
            Some(Square::from(value as u8))
        }
    }

    pub fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn force_mask(self) -> MoveData {
        (self.0 & FORCE_MASK) >> FORCE_MASK_OFFSET
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }

    /// Resolves the force mask against `board`, which must be the position *before* the song
    /// resolves. Returns the (from, to) pairs and how many of them are populated.
    fn decode_forced_workers(
        self,
        board: &BoardState,
        player: Player,
    ) -> ([(Square, Square); MAX_ADDRESSABLE_WORKERS], usize) {
        let mut res = [(Square::A5, Square::A5); MAX_ADDRESSABLE_WORKERS];
        let mut count = 0;

        let force_mask = self.force_mask();
        if force_mask == 0 {
            return (res, count);
        }

        let (sources, source_count) = get_addressable_workers(board, player);

        for worker_idx in 0..source_count {
            if force_mask & (1 << worker_idx) == 0 {
                continue;
            }

            let from = sources[worker_idx];
            // Transposition table moves are handed to `make_move` without being re-verified
            // against the position, so a hash collision can decode to a worker standing on the
            // bottom row, with nowhere downwind to go. Drop those rather than xoring a worker off
            // the board.
            let to_mask = from.to_board().shift_south();
            if to_mask.is_empty() {
                continue;
            }

            res[count] = (from, to_mask.lsb());
            count += 1;
        }

        (res, count)
    }
}

impl GodMove for SirenMove {
    fn move_to_actions(
        self,
        board: &BoardState,
        player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res = Vec::new();

        if self.maybe_move_to_position().is_none() {
            let (forced, count) = self.decode_forced_workers(board, player);
            for &(from, to) in &forced[..count] {
                res.push(PartialAction::ForceOpponentWorker(from, to));
            }
        }

        res.push(PartialAction::SelectWorker(self.move_from_position()));
        if let Some(move_to) = self.maybe_move_to_position() {
            res.push(PartialAction::MoveWorker(move_to.into()));

            if self.get_is_winning() {
                return vec![res];
            }
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        if let Some(move_to) = self.maybe_move_to_position() {
            board.worker_xor(
                player,
                BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(move_to),
            );

            if self.get_is_winning() {
                board.set_winner(player);
                return;
            }
        } else {
            // The worker list is snapshotted before anything moves, so the whole song resolves at
            // once: a worker stepping off a square does not open that square up as somewhere to
            // drag the worker behind it.
            let (forced, count) = self.decode_forced_workers(board, player);
            for &(from, to) in &forced[..count] {
                // One call per worker, never a batched xor: `oppo_worker_xor` re-points the
                // female-worker god_data by xoring it with the whole mask it is given, so a
                // combined mask would corrupt Selene's / Hippolyta's tracking.
                board.oppo_worker_xor(other_god, !player, from.to_board() ^ to.to_board());
            }
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        // A song moves none of her workers, so it can never climb and is never the winning move a
        // blocker board gets asked about. Naming squares for one anyway would only invite blocks
        // against a threat that is not there.
        let Some(move_to) = self.maybe_move_to_position() else {
            return BitBoard::EMPTY;
        };

        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(move_to)
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_maybe_square_with_height(board, self.maybe_move_to_position());
        helper.add_square_with_height(board, self.build_position());
        helper.add_value(self.force_mask() as usize, 1 << FORCE_MASK_WIDTH);
        helper.get()
    }
}

impl std::fmt::Debug for SirenMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let Some(move_to) = self.maybe_move_to_position() else {
            // No board here, so the dragged workers print as their list index rather than their
            // square.
            write!(f, "{}(", move_from)?;
            let mut is_first = true;
            for worker_idx in 0..MAX_ADDRESSABLE_WORKERS {
                if self.force_mask() & (1 << worker_idx) == 0 {
                    continue;
                }
                if !is_first {
                    write!(f, ",")?;
                }
                is_first = false;
                write!(f, "w{}", worker_idx)?;
            }
            return write!(f, ")v^{}", self.build_position());
        };

        if self.get_is_winning() {
            write!(f, "{}>{}#", move_from, move_to)
        } else {
            write!(f, "{}>{}^{}", move_from, move_to, self.build_position())
        }
    }
}

/// The opponent's workers in ascending square order - the basis a move's force mask indexes into.
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

fn siren_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(siren_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    // An ordinary turn, identical to a mortal's.
    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, SirenMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                SirenMove::new_winning_move,
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

            for worker_build_pos in worker_next_build_state.narrowed_builds {
                let new_action = SirenMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    (reach_board & final_level_3).is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    // The song never climbs, so it is never a mate, and Persephone never permits it while an
    // ordinary climb is on offer.
    if is_mate_only::<F>() || MUST_CLIMB {
        return result;
    }

    let blocked = prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen;
    let open = !blocked & BitBoard::MAIN_SECTION_MASK;

    // A worker can be dragged if the square directly downwind of it is free. Frozen workers stay
    // put: Clio standing on one of her own coins reads to us as standing on a dome, so the song
    // cannot reach her.
    let forcible = prelude.oppo_workers & !prelude.domes_and_frozen & open.shift_north();
    if forcible.is_empty() {
        return result;
    }

    let (addressable, addressable_count) = get_addressable_workers(&state.board, player);
    let mut forcible_mask: MoveData = 0;
    for worker_idx in 0..addressable_count {
        if forcible.contains_square(addressable[worker_idx]) {
            forcible_mask |= 1 << worker_idx;
        }
    }

    // Her workers do not move, so what she threatens next turn is fixed before the song starts.
    // The "fewer than two" test is how the rest of the engine reads Hypnus: a lone level 2 worker
    // is the highest she has, and the highest worker cannot move.
    let threatening_workers = prelude.own_workers & prelude.exactly_level_2;
    let threatening_neighbors =
        apply_mapping_to_mask(threatening_workers, prelude.standard_neighbor_map);
    let has_threat = !(prelude.is_against_hypnus && threatening_workers.count_ones() < 2);

    let all_builder_neighbors = apply_mapping_to_mask(prelude.own_workers, &NEIGHBOR_MAP);

    // Every non-empty subset of the forcible workers. Each one lands somewhere different - a
    // worker's destination is fixed by its square - so no two subsets reach the same position and
    // there is nothing to canonicalise away.
    let mut subset = forcible_mask;
    while subset != 0 {
        let mut forced_from = BitBoard::EMPTY;
        for worker_idx in 0..addressable_count {
            if subset & (1 << worker_idx) != 0 {
                forced_from |= addressable[worker_idx].to_board();
            }
        }
        let forced_to = forced_from.shift_south();

        let open_after = open ^ forced_from ^ forced_to;
        let new_oppo_workers = prelude.oppo_workers ^ forced_from ^ forced_to;
        // Limus bans building next to *her* workers, and the song has just moved them.
        let build_mask_after =
            prelude.other_god.get_build_mask(new_oppo_workers) | prelude.exactly_level_3;

        let mut narrowed_builds = all_builder_neighbors & open_after & build_mask_after;
        if is_interact_with_key_squares::<F>()
            && (key_squares & (forced_from | forced_to)).is_empty()
        {
            narrowed_builds &= prelude.key_squares;
        }

        let reach_board = if has_threat {
            threatening_neighbors & prelude.win_mask & open_after
        } else {
            BitBoard::EMPTY
        };

        for worker_build_pos in narrowed_builds {
            // Any of her workers may build, and which one it is leaves no trace on the board, so
            // the neighbor with the lowest square is picked as the one to name. Emitting the move
            // once per eligible builder would just be the same position over and over.
            let builder = (prelude.own_workers & NEIGHBOR_MAP[worker_build_pos as usize]).lsb();

            let build_mask = worker_build_pos.to_board();
            let is_check = {
                let final_level_3 = (prelude.exactly_level_2 & build_mask)
                    | (prelude.exactly_level_3 & !build_mask);
                (reach_board & final_level_3).is_not_empty()
            };

            let new_action = SirenMove::new_song_move(builder, worker_build_pos, subset);
            result.push(build_scored_move::<F, _>(new_action, is_check, false));
        }

        subset = (subset - 1) & forcible_mask;
    }

    result
}

/// She borrows Mortal's NNUE weights: her ordinary turn is a mortal one and she carries no
/// `god_data` at all, the song being a thing she may do rather than a thing she accumulates.
pub const fn build_siren() -> GodPower {
    god_power(
        GodName::Siren,
        build_god_power_movers!(siren_move_gen),
        build_god_power_actions::<SirenMove>(),
        10422823607176399451,
        6002115468925304309,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_symmetry(BoardSymmetry::HorizontalOnly)
}

#[cfg(test)]
mod tests {
    use crate::{
        board::GameStateBuilder, consistency_checker::consistency_check, fen::parse_fen,
        move_verifier::MoveVerifier, pretty_board::game_state_with_partial_actions,
        square::Square::*,
    };

    use super::*;

    /// Every set of opponent workers the song is offered as dragging, as (dragged-from) masks.
    fn song_drags_for(state: &FullGameState, player: Player) -> Vec<BitBoard> {
        let siren = GodName::Siren.to_power();
        let mut res = Vec::new();

        for scored in siren.get_all_moves(state, player) {
            let action: SirenMove = scored.action.into();
            if action.maybe_move_to_position().is_some() {
                continue;
            }

            let (forced, count) = action.decode_forced_workers(&state.board, player);
            let mut mask = BitBoard::EMPTY;
            for &(from, _) in &forced[..count] {
                mask |= from.to_board();
            }
            if !res.contains(&mask) {
                res.push(mask);
            }
        }

        res
    }

    #[test]
    fn test_siren_move_fits_in_the_main_section() {
        let plain = SirenMove::new_basic_move(A1, B2, C3);
        assert_eq!(plain.move_from_position(), A1);
        assert_eq!(plain.maybe_move_to_position(), Some(B2));
        assert_eq!(plain.build_position(), C3);
        assert_eq!(plain.force_mask(), 0);
        assert_eq!(plain.0 & MOVE_DATA_MAIN_SECTION, plain.0);

        // An ordinary turn is a mortal turn, and encodes to the same bits.
        let mortal = crate::gods::mortal::MortalMove::new_basic_move(A1, B2, C3);
        assert_eq!(plain.0, mortal.0);

        for force_mask in 1..(1 << FORCE_MASK_WIDTH) {
            let song = SirenMove::new_song_move(A1, C3, force_mask);
            assert_eq!(song.move_from_position(), A1);
            assert_eq!(song.maybe_move_to_position(), None);
            assert_eq!(song.build_position(), C3);
            assert_eq!(song.force_mask(), force_mask);
            assert_eq!(song.0 & MOVE_DATA_MAIN_SECTION, song.0);
        }
    }

    #[test]
    fn test_song_drags_a_worker_one_square_down() {
        // C3 is the only opponent worker within the song's reach that has anywhere to go; E1 is on
        // the bottom row.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(A4)
            .with_p2_worker(C3)
            .with_p2_worker(E1)
            .with_height(C2, 3)
            .build();

        let next_states = state.get_next_states_interactive();

        // Level makes no difference to where the song can put somebody: C2 is three storeys up.
        MoveVerifier::new()
            .without_p2_worker_at(C3)
            .with_p2_worker_at(C2)
            .any(&next_states);

        assert_eq!(song_drags_for(&state, Player::One), vec![C3.to_board()]);
    }

    #[test]
    fn test_song_drags_every_subset_of_the_workers_it_can_reach() {
        let state = GameStateBuilder::new(GodName::Siren, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(A4)
            .with_p2_worker(C3)
            .with_p2_worker(E3)
            .build();

        let mut drags = song_drags_for(&state, Player::One);
        drags.sort_by_key(|b| b.0);

        assert_eq!(
            drags,
            vec![C3.to_board(), E3.to_board(), C3.to_board() | E3.to_board()]
        );
    }

    #[test]
    fn test_song_leaves_workers_with_nowhere_downwind_to_go() {
        // A1 is on the bottom row, B2 has a dome below it, C2 has the Siren's own worker below it,
        // and D3 has the opponent's other worker below it.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Scylla)
            .with_p1_worker(A5)
            .with_p1_worker(C1)
            .with_p2_worker(A1)
            .with_p2_worker(B2)
            .with_p2_worker(D3)
            .with_p2_worker(D2)
            .with_height(B1, 4)
            .build();

        // D2 itself is free to move: D1 is empty. Nothing else is.
        assert_eq!(song_drags_for(&state, Player::One), vec![D2.to_board()]);
    }

    #[test]
    fn test_song_does_not_shuffle_a_column_along() {
        // D3 sits directly above D2, and the whole song resolves at once, so D2 stepping off D2
        // does not open D2 up as somewhere to put D3.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(A4)
            .with_p2_worker(D3)
            .with_p2_worker(D2)
            .build();

        assert_eq!(song_drags_for(&state, Player::One), vec![D2.to_board()]);
    }

    #[test]
    fn test_being_dragged_onto_level_three_does_not_win() {
        // C2 is a level 3 square directly downwind of an opponent worker. Being forced is not
        // moving, so nobody wins by landing there.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(A4)
            .with_p2_worker(C3)
            .with_p2_worker(E1)
            .with_height(C3, 2)
            .with_height(C2, 3)
            .build();

        let siren = GodName::Siren.to_power();
        let mortal = GodName::Mortal.to_power();

        let mut saw_the_drag = false;
        for scored in siren.get_all_moves(&state, Player::One) {
            let action: SirenMove = scored.action.into();
            if action.maybe_move_to_position().is_some() {
                continue;
            }
            saw_the_drag = true;

            let next = state.next_state(siren, mortal, scored.action);
            assert_eq!(next.get_winner(), None, "{:?}", action);
            assert!(!scored.get_is_winning(), "{:?}", action);
        }
        assert!(saw_the_drag);
    }

    #[test]
    fn test_song_builds_with_either_worker() {
        // Her workers stand at opposite corners, so between them they can build anywhere along
        // either corner - and each build square is emitted exactly once.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Mortal)
            .with_p1_worker(A5)
            .with_p1_worker(E1)
            .with_p2_worker(C3)
            .with_p2_worker(C4)
            .build();

        let siren = GodName::Siren.to_power();
        let mut builds_near_a5 = false;
        let mut builds_near_e1 = false;

        for scored in siren.get_all_moves(&state, Player::One) {
            let action: SirenMove = scored.action.into();
            if action.maybe_move_to_position().is_some() {
                continue;
            }

            let build = action.build_position();
            let builder = action.move_from_position();
            assert!(
                NEIGHBOR_MAP[build as usize].contains_square(builder),
                "{:?} builds out of reach of the worker it names",
                action
            );

            builds_near_a5 |= build == B5;
            builds_near_e1 |= build == D1;
        }

        assert!(builds_near_a5 && builds_near_e1);
    }

    #[test]
    fn test_song_can_create_a_check() {
        // Her B2 worker stands on level 2 next to a level 2 square. Building C2 up makes it a
        // level 3 she can step onto next turn, and the song is how she spends the turn.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Mortal)
            .with_p1_worker(B2)
            .with_p1_worker(A5)
            .with_p2_worker(E3)
            .with_p2_worker(E1)
            .with_height(B2, 2)
            .with_height(C2, 2)
            .build();

        assert_eq!(consistency_check(&state), Ok(()));

        let mut saw_check = false;
        for scored in GodName::Siren
            .to_power()
            .get_moves_for_search(&state, Player::One)
        {
            let action: SirenMove = scored.action.into();
            if action.maybe_move_to_position().is_some() || action.build_position() != C2 {
                continue;
            }
            saw_check = true;
            assert!(scored.action.get_is_check(), "{:?}", action);
        }
        assert!(saw_check);
    }

    #[test]
    fn test_song_actions_replay_to_the_same_board() {
        let state = GameStateBuilder::new(GodName::Siren, GodName::Apollo)
            .with_p1_worker(A5)
            .with_p1_worker(A4)
            .with_p2_worker(C3)
            .with_p2_worker(E3)
            .build();

        let siren = GodName::Siren.to_power();
        let apollo = GodName::Apollo.to_power();

        let mut saw_song = false;
        for scored in siren.get_all_moves(&state, Player::One) {
            let action: SirenMove = scored.action.into();
            if action.maybe_move_to_position().is_some() {
                continue;
            }
            saw_song = true;

            let expected = state.next_state(siren, apollo, scored.action);
            for path in siren.get_actions_for_move(&state.board, scored.action, Player::One, apollo)
            {
                let replayed = game_state_with_partial_actions(&state, &path);
                assert_eq!(
                    replayed.board.workers, expected.board.workers,
                    "{:?} replayed to different workers",
                    action
                );
                assert_eq!(
                    replayed.board.height_map, expected.board.height_map,
                    "{:?} replayed to a different board",
                    action
                );
            }
        }
        assert!(saw_song);
    }

    #[test]
    fn test_persephone_takes_the_song_away() {
        // Her A5 worker can climb onto B5, so Persephone obliges her to - and the song, which
        // climbs nothing, is not a way out of it.
        let state = GameStateBuilder::new(GodName::Siren, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p1_worker(E1)
            .with_p2_worker(C3)
            .with_p2_worker(E3)
            .with_height(B5, 1)
            .build();

        assert_eq!(consistency_check(&state), Ok(()));
        assert!(song_drags_for(&state, Player::One).is_empty());

        // Without the climb on offer she gets it back.
        let flat = GameStateBuilder::new(GodName::Siren, GodName::Persephone)
            .with_p1_worker(A5)
            .with_p1_worker(E1)
            .with_p2_worker(C3)
            .with_p2_worker(E3)
            .build();

        assert!(!song_drags_for(&flat, Player::One).is_empty());
    }

    #[test]
    fn test_a_siren_game_only_mirrors_horizontally() {
        // Her song blows one fixed way down the board, so a position is only equivalent to its
        // left-right mirror - not to any of the six turns and flips that would point it elsewhere.
        let state = parse_fen("1000000000000000000000000/1/siren:A1,A2/mortal:E4,E5").unwrap();
        assert_eq!(state.get_all_permutations::<true>().len(), 2);
        assert_eq!(state.get_all_permutations::<false>().len(), 1);

        let mortal_state =
            parse_fen("1000000000000000000000000/1/mortal:A1,A2/mortal:E4,E5").unwrap();
        assert_eq!(mortal_state.get_all_permutations::<true>().len(), 8);

        // The pin applies whichever side is singing.
        let flipped = parse_fen("1000000000000000000000000/1/mortal:A1,A2/siren:E4,E5").unwrap();
        assert_eq!(flipped.get_all_permutations::<true>().len(), 2);
    }

    #[test]
    fn test_opening_placements_are_only_folded_across_the_mirror() {
        // The end of the same story: opening placements are deduplicated by folding a position
        // together with its symmetries, so pinning the orientation leaves far more of them
        // genuinely distinct.
        let count_for = |gods: [GodName; 2]| {
            let powers = [gods[0].to_power(), gods[1].to_power()];
            powers[0]
                .get_unique_placement_actions(powers, &BoardState::default(), Player::One)
                .len()
        };

        assert_eq!(count_for([GodName::Mortal, GodName::Mortal]), 49);
        assert_eq!(count_for([GodName::Siren, GodName::Mortal]), 160);
        assert_eq!(count_for([GodName::Mortal, GodName::Siren]), 160);
    }
}

