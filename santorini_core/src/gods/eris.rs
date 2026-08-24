//! Eris, Goddess of Strife.
//!
//! "Your Turn: you may move and build with an opponent's Worker this turn instead of your own, as
//! long as it is not their Worker that they most recently moved."
//!
//! Two rulings shape the whole implementation. Eris *moves* Workers rather than forcing them, so
//! the puppeted Worker takes a real turn - anything triggered by moving triggers. But she cannot
//! use the opponent's power, so it takes that turn under Mortal rules: a puppeted Pegasus climbs
//! one level like anybody else.
//!
//! Two families of move come out of one generator. Her own moves are Mortal's, bit for bit. A
//! puppet move is Mortal's shape too, started from an opponent Worker instead: the square it
//! leaves frees up, every other Worker on the board stays an obstacle, and the build is an
//! ordinary build from wherever it lands.
//!
//! The Workers she may not touch live as a square mask in *her* `god_data`, rewritten after each
//! of the opponent's turns by [`BoardState::on_turn_advanced`]. A mask rather than a single square
//! is what settles the multi-Worker movers - Hermes, Castor, Terpsichore - which the card, written
//! for one Worker, says nothing about. Protecting all of them needs no invented rule, at the cost
//! of letting those gods switch her off whenever they move both.
//!
//! Points the card leaves open, and which way they go here:
//!
//! - **A puppet reaching level 3 wins for nobody.** The opponent winning would make the move
//!   unplayable; Eris winning contradicts "your Worker". Santorini's general principle is that a
//!   Worker arriving on level 3 other than by its owner's volition does not win, and the engine
//!   already carries Workers standing on level 3 for Hera and for forced moves.
//! - **The opponent's restrictive passives follow the mover, not the Worker.** Athena's climb ban,
//!   Limus' build restriction and Aeolus' wind all constrain a puppeted Worker exactly as they
//!   constrain one of hers. One rule instead of four, and it matches "Eris cannot use the
//!   opponent's power" - she gets no benefit from it and no exemption from it either. The price is
//!   paid in full against Limus, who bans building beside his Workers: a puppet is standing on the
//!   square it would build next to, so it has no legal build at all and her power goes quiet for
//!   the whole game. See `tests::limus_leaves_a_puppet_nowhere_to_build`.
//! - **Hypnus and Aphrodite are the exception, because they name Workers by owner.** "Your
//!   opponent's Workers" means hers; a puppet is one of *his*, so his own freeze and her own pull
//!   have no referent on it. Extending them would mean inventing which set of Workers to measure
//!   the height or the affinity against.
//! - **Harpies does not slide a puppet.** Her song catches "an opponent's Worker", and from
//!   Harpies' point of view the Worker Eris is moving is her own. Eris moving one of her own
//!   Workers does slide.
//! - **A puppet move writes nothing to the owner's `god_data`.** Moving Athena's Worker up is not
//!   Athena moving up. The one exception is Selene's and Hippolyta's female flag, which names a
//!   *square* rather than recording a turn, so it has to travel with the Worker it describes.
//!
//! [`BoardState::on_turn_advanced`]: crate::board::BoardState::on_turn_advanced

use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, PartialAction, StaticGod,
        build_god_power_actions,
        generic::{
            GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK, MoveData,
            MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            GeneratorPreludeState, WorkerEndMoveState, WorkerStartMoveState, build_scored_move,
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity,
            get_generator_prelude_state, get_standard_reach_board,
            get_standard_reach_board_from_parts, get_worker_end_move_state,
            get_worker_next_build_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, modify_prelude_for_checking_workers,
            push_winning_moves,
        },
    },
    persephone_check_result,
    placement::PlacementType,
    player::Player,
    square::Square,
};

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const IS_PUPPET_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const IS_PUPPET_MASK: MoveData = 1 << IS_PUPPET_OFFSET;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct ErisMove(pub MoveData);

impl ErisMove {
    pub fn new_own_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        Self(
            ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
                | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
                | ((build_position as MoveData) << BUILD_POSITION_OFFSET),
        )
    }

    pub fn new_puppet_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        Self(
            Self::new_own_move(move_from_position, move_to_position, build_position).0
                | IS_PUPPET_MASK,
        )
    }

    /// Only her own Workers ever win, so a winning move is never a puppet one.
    pub fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        Self(
            ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
                | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
                | MOVE_IS_WINNING_MASK,
        )
    }

    pub fn move_from_position(&self) -> Square {
        Square::from((self.0 >> MOVE_FROM_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn build_position(&self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn move_mask(&self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_puppet(&self) -> bool {
        (self.0 & IS_PUPPET_MASK) != 0
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl Into<GenericMove> for ErisMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for ErisMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl std::fmt::Debug for ErisMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();

        if self.get_is_winning() {
            return write!(f, "{}>{}#", move_from, move_to);
        }

        let marker = if self.get_is_puppet() { "~" } else { "" };
        write!(f, "{marker}{}>{}^{}", move_from, move_to, self.build_position())
    }
}

impl GodMove for ErisMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        if self.get_is_puppet() {
            return vec![vec![
                PartialAction::ForceOpponentWorker(
                    self.move_from_position(),
                    self.move_to_position(),
                ),
                PartialAction::Build(self.build_position()),
            ]];
        }

        let mut res = vec![
            PartialAction::SelectWorker(self.move_from_position()),
            PartialAction::MoveWorker(self.move_to_position().into()),
        ];
        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));
        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, other_god: StaticGod) {
        let worker_move_mask = self.move_mask();

        if self.get_is_puppet() {
            let victim = !player;

            // Selene's and Hippolyta's female flag names a square, so it has to travel with the
            // Worker it describes or the position stops validating. This is deliberately not
            // `oppo_worker_xor`, which *clears* the flag: that is for a displacement, where the
            // Worker is knocked aside, and a puppeted Worker is still theirs and still female.
            if other_god.placement_type == PlacementType::FemaleWorker
                && (board.god_data[victim as usize] & worker_move_mask.0) != 0
            {
                board.xor_god_data(victim, worker_move_mask.0);
            }

            board.worker_xor(victim, worker_move_mask);
            board.build_up(self.build_position());
            return;
        }

        board.worker_xor(player, worker_move_mask);

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        helper.add_square_with_height(board, self.move_from_position());
        helper.add_square_with_height(board, self.move_to_position());
        helper.add_square_with_height(board, self.build_position());
        // Without this a puppet move and an own move over the same three squares - which the board
        // makes perfectly possible - would share a history slot and score each other's successes.
        helper.add_bool(self.get_is_puppet());
        helper.get()
    }
}

/// The opponent Workers Eris may take over this turn.
///
/// Two exclusions, both mask intersections. The Workers they moved on their own most recent turn
/// are off limits by the card. Workers standing on a frozen square are off limits because the
/// square is: Clio's ruling that Eris cannot puppet a Worker standing on coins is the same
/// exclusion the rest of the generator already applies to every square a coin sits on.
fn get_puppetable_workers(prelude: &GeneratorPreludeState, player: Player) -> BitBoard {
    let off_limits = BitBoard(prelude.board.god_data[player as usize]);
    prelude.oppo_workers & !off_limits & !prelude.domes_and_frozen
}

pub(super) fn eris_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    let mut result = persephone_check_result!(eris_move_gen, state: state, player: player, key_squares: key_squares, MUST_CLIMB: MUST_CLIMB);

    let mut prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    // Her own moves are a Mortal's, and come first so that a win short-circuits the whole
    // generator before any puppet work is done.
    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, ErisMove, _>(
                &mut result,
                worker_start_pos,
                moves_to_level_3,
                ErisMove::new_winning_move,
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
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    (reach_board & final_level_3).is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    ErisMove::new_own_move(
                        worker_start_pos,
                        worker_end_move_state.worker_end_pos,
                        worker_build_pos,
                    ),
                    is_check,
                    worker_end_move_state.is_improving,
                ))
            }
        }
    }

    // No puppet move ever wins, so a mate search is done. `MUST_CLIMB` is Persephone's demand that
    // one of the acting player's own Workers climb, and a puppet turn moves none of them - the
    // reason that matchup is banned outright rather than merely awkward.
    if is_mate_only::<F>() || MUST_CLIMB {
        return result;
    }

    generate_puppet_moves::<F>(&prelude, player, &mut result);

    result
}

fn generate_puppet_moves<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    player: Player,
    result: &mut Vec<ScoredMove>,
) {
    let puppetable = get_puppetable_workers(prelude, player);
    if puppetable.is_empty() {
        return;
    }

    let all_workers = prelude.own_workers | prelude.oppo_workers;

    // None of her own Workers move on a puppet turn, so what she threatens next turn is settled
    // before the puppet takes a step: her level 2 Workers, and wherever they can reach. All that
    // varies per move is which squares are still free once the Worker has landed and built.
    let own_threatening_workers = prelude.own_workers & prelude.exactly_level_2;
    let own_threatening_neighbors =
        apply_mapping_to_mask(own_threatening_workers, prelude.standard_neighbor_map);

    // Limus measures his ban from where his Workers stand at the end of the turn, and a puppet
    // move is Eris relocating one of them. Recomputing costs a mapping per destination, so it is
    // skipped for the gods that do not restrict builds at all.
    let build_mask_is_dynamic = prelude.build_mask != BitBoard::MAIN_SECTION_MASK;

    for puppet_start_pos in puppetable {
        let puppet_start_mask = BitBoard::as_mask(puppet_start_pos);
        let puppet_start_height = prelude.board.get_height(puppet_start_pos);

        let worker_start_state = WorkerStartMoveState {
            worker_start_pos: puppet_start_pos,
            worker_start_mask: puppet_start_mask,
            worker_start_height: puppet_start_height,
            // Every one of her own Workers stays put, so every one of them still threatens.
            other_own_workers: prelude.own_workers,
            all_non_moving_workers: all_workers ^ puppet_start_mask,
        };

        // No affinity: Aphrodite's pull governs where *her opponent's* Workers may go, and the one
        // being moved here is Aphrodite's own. No Harpies slide either, for the mirror reason.
        let puppet_moves =
            get_basic_moves_from_raw_data_with_custom_blockers_no_affinity::<false>(
                prelude,
                puppet_start_pos,
                puppet_start_height,
                prelude.all_workers_and_frozen_mask ^ puppet_start_mask,
            );

        // Vacating a key square is a block in its own right - it is how she takes the Worker that
        // was about to win off its launch pad - and the square a move *leaves* is not something
        // the standard narrowing looks at.
        let start_matches_key_squares = is_interact_with_key_squares::<F>()
            && (puppet_start_mask & prelude.key_squares).is_not_empty();

        for puppet_end_pos in puppet_moves {
            let puppet_end_mask = BitBoard::as_mask(puppet_end_pos);
            let worker_end_move_state = WorkerEndMoveState {
                worker_end_pos: puppet_end_pos,
                worker_end_mask: puppet_end_mask,
                worker_end_height: prelude.board.get_height(puppet_end_pos),
                // Sending their Worker uphill is not an improvement of hers, and it threatens
                // nothing: a puppet on level 2 cannot be walked onto level 3 for a win.
                is_improving: false,
                is_now_lvl_2: 0,
            };

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | puppet_end_mask
                | prelude.domes_and_frozen);

            let final_build_mask = if build_mask_is_dynamic {
                let final_oppo_workers =
                    prelude.oppo_workers ^ puppet_start_mask ^ puppet_end_mask;
                prelude.other_god.get_build_mask(final_oppo_workers) | prelude.exactly_level_3
            } else {
                prelude.build_mask
            };

            let mut builds = NEIGHBOR_MAP[puppet_end_pos as usize]
                & unblocked_squares
                & final_build_mask;

            if is_interact_with_key_squares::<F>()
                && !start_matches_key_squares
                && (puppet_end_mask & prelude.key_squares).is_empty()
            {
                builds &= prelude.key_squares;
            }

            if builds.is_empty() {
                continue;
            }

            let reach_board = get_standard_reach_board_from_parts::<F>(
                prelude,
                own_threatening_workers,
                own_threatening_neighbors,
                puppet_end_pos,
                worker_end_move_state.is_now_lvl_2,
                unblocked_squares,
            );

            for build_pos in builds {
                let build_mask = build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    (reach_board & final_level_3).is_not_empty()
                };

                result.push(build_scored_move::<F, _>(
                    ErisMove::new_puppet_move(puppet_start_pos, puppet_end_pos, build_pos),
                    is_check,
                    false,
                ));
            }
        }
    }
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    if data.is_empty() {
        return Ok(0);
    }

    let mut res = BitBoard::EMPTY;
    for part in data.split(',') {
        let square: Square = part
            .trim()
            .parse()
            .map_err(|e| format!("Failed to parse square {}: {:?}", part, e))?;
        res |= BitBoard::as_mask(square);
    }

    Ok(res.0 as GodData)
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        x => Some(
            BitBoard(x)
                .all_squares()
                .iter()
                .map(Square::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    match board.god_data[player as usize] {
        0 => Some("Every opponent Worker is available".to_string()),
        x => Some(format!(
            "Cannot control the Worker(s) at {}",
            BitBoard(x)
                .all_squares()
                .iter()
                .map(Square::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn flip_horizontal(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_horizontal().0 as GodData
}

fn flip_vertical(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_vertical().0 as GodData
}

fn flip_transpose(god_data: GodData) -> GodData {
    BitBoard(god_data).flip_transpose().0 as GodData
}

pub const fn build_eris() -> GodPower {
    god_power(
        GodName::Eris,
        build_god_power_movers!(eris_move_gen),
        build_god_power_actions::<ErisMove>(),
        2974201680550869065,
        17177741703695918135,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_is_eris()
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
    .with_flip_god_data_horizontal_fn(flip_horizontal)
    .with_flip_god_data_vertical_fn(flip_vertical)
    .with_flip_god_data_transpose_fn(flip_transpose)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bitboard::WIND_AWARE_NEIGHBOR_MAP,
        consistency_checker::consistency_check,
        fen::{game_state_to_fen, parse_fen},
    };

    fn all_moves(state: &FullGameState) -> Vec<ErisMove> {
        let god = state.get_active_god();
        god.get_all_moves(state, state.board.current_player)
            .into_iter()
            .map(|m| ErisMove::from(m.action))
            .collect()
    }

    fn puppet_moves(state: &FullGameState) -> Vec<ErisMove> {
        all_moves(state)
            .into_iter()
            .filter(|m| m.get_is_puppet())
            .collect()
    }

    fn apply(state: &FullGameState, action: ErisMove) -> FullGameState {
        let (active, other) = state.get_active_non_active_gods();
        state.next_state(active, other, action.into())
    }

    /// The record of what the opponent last moved rides in the FEN like any other god data.
    #[test]
    fn fen_round_trips_the_memory() {
        for fen in [
            "0000000000000000000000000/1/eris:A5,A4/mortal:E5,E4",
            "0000000000000000000000000/1/eris[E5]:A5,A4/mortal:E5,E4",
            "0000000000000000000000000/1/eris[E5,E4]:A5,A4/mortal:E5,E4",
        ] {
            let state = parse_fen(fen).unwrap();
            assert_eq!(game_state_to_fen(&state), fen);
            state.validate();
        }
    }

    /// The memory is written by the opponent's own turn and by nothing else. A puppet turn moves
    /// their Worker, but they did not move it, so it leaves the record exactly as it found it -
    /// otherwise she could take a Worker over and immediately lock herself out of it.
    #[test]
    fn the_memory_records_only_the_opponents_own_turn() {
        let theirs = parse_fen("0000000000000000000000000/2/eris:A5,A4/mortal:E5,E1").unwrap();
        assert_eq!(theirs.board.god_data[0], 0);

        let (active, other) = theirs.get_active_non_active_gods();
        let their_move = active
            .get_all_moves(&theirs, Player::Two)
            .into_iter()
            .map(|m| crate::gods::mortal::MortalMove::from(m.action))
            .find(|m| m.move_from_position() == Square::E5 && m.move_to_position() == Square::D5)
            .expect("E5 > D5 should be legal");
        let hers = theirs.next_state(active, other, their_move.into());

        assert_eq!(
            BitBoard(hers.board.god_data[0]),
            Square::D5.to_board(),
            "the record names where their Worker ended up, not where it started"
        );
        hers.validate();

        // She takes over the Worker they did *not* move, which changes nothing about what they
        // most recently moved.
        let puppet = puppet_moves(&hers)
            .into_iter()
            .find(|m| m.move_from_position() == Square::E1)
            .expect("their other Worker is available");
        let after = apply(&hers, puppet);
        assert_eq!(
            BitBoard(after.board.god_data[0]),
            Square::D5.to_board(),
            "a puppet turn is not a turn of theirs"
        );
        after.validate();
    }

    /// The Worker named by the memory is untouchable, and only that one.
    #[test]
    fn off_limits_workers_cannot_be_puppeted() {
        let state = parse_fen("0000000000000000000000000/1/eris[E5]:A5,A4/mortal:E5,E1").unwrap();

        let starts: Vec<Square> = puppet_moves(&state)
            .iter()
            .map(|m| m.move_from_position())
            .collect();
        assert!(!starts.is_empty());
        assert!(starts.iter().all(|s| *s == Square::E1));

        // With the memory cleared, both of their Workers are hers to use.
        let free = parse_fen("0000000000000000000000000/1/eris:A5,A4/mortal:E5,E1").unwrap();
        let free_starts: std::collections::HashSet<Square> = puppet_moves(&free)
            .iter()
            .map(|m| m.move_from_position())
            .collect();
        assert_eq!(free_starts.len(), 2);
    }

    /// A Worker she walks onto level 3 wins for nobody. Reading it as the owner's win would make
    /// the move unplayable; reading it as hers contradicts "your Worker". The move is generated
    /// as an ordinary one - she may legitimately want somebody parked up there - and simply does
    /// not end the game.
    #[test]
    fn a_puppet_reaching_level_3_wins_for_nobody() {
        let state = parse_fen("00000 00300 00200 00000 00000/1/eris:A1,B1/mortal:C3,E1").unwrap();

        let climb = puppet_moves(&state)
            .into_iter()
            .find(|m| m.move_from_position() == Square::C3 && m.move_to_position() == Square::C4)
            .expect("she should be able to walk their Worker up onto level 3");
        assert!(!climb.get_is_winning());

        let after = apply(&state, climb);
        assert_eq!(after.get_winner(), None);
        assert_eq!(after.board.get_height(Square::C4), 3);
        after.validate();
    }

    /// Athena's ban is on the mover, not on the Worker: while it is in force nothing Eris moves
    /// climbs, her own Workers and theirs alike. She gets no benefit from their power and no
    /// exemption from it either.
    #[test]
    fn athenas_climb_ban_reaches_a_puppeted_worker() {
        let banned =
            parse_fen("00000 00000 01000 00000 00000/1/eris:A1,A2/athena[^]:C3,E1").unwrap();
        assert!(
            puppet_moves(&banned)
                .iter()
                .all(|m| banned.board.get_height(m.move_to_position())
                    <= banned.board.get_height(m.move_from_position())),
            "no puppet may climb while Athena's flag is up"
        );

        let allowed = parse_fen("00000 00000 01000 00000 00000/1/eris:A1,A2/athena:C3,E1").unwrap();
        assert!(
            puppet_moves(&allowed)
                .iter()
                .any(|m| m.move_from_position() == Square::C3
                    && m.move_to_position() == Square::B3),
            "with the flag down the same climb is available"
        );
    }

    /// Moving Athena's Worker up is not Athena moving up. Her flag governs what *she* did on her
    /// own turn, so a puppet turn must leave the whole of the owner's state alone.
    #[test]
    fn a_puppet_move_writes_none_of_the_owners_state() {
        let state = parse_fen("00000 00000 01000 00000 00000/1/eris:A1,A2/athena:C3,E1").unwrap();
        assert_eq!(state.board.god_data[1], 0);

        let climb = puppet_moves(&state)
            .into_iter()
            .find(|m| m.move_from_position() == Square::C3 && m.move_to_position() == Square::B3)
            .expect("B3 is one level up from C3");

        let after = apply(&state, climb);
        assert_eq!(
            after.board.god_data[1], 0,
            "Athena's flag must not be set by a climb she did not choose"
        );
        after.validate();
    }

    /// Selene's female flag names a *square* rather than recording a turn, so it is the one piece
    /// of the owner's state that has to travel with a puppeted Worker. It is also not cleared:
    /// this is not a displacement, and the Worker is still theirs and still female.
    #[test]
    fn the_female_flag_travels_with_a_puppeted_worker() {
        let state = parse_fen("0000000000000000000000000/1/eris:A1,A2/selene[E5]:E5,E1").unwrap();
        assert_eq!(BitBoard(state.board.god_data[1]), Square::E5.to_board());

        let puppet = puppet_moves(&state)
            .into_iter()
            .find(|m| m.move_from_position() == Square::E5 && m.move_to_position() == Square::D5)
            .expect("their female Worker can step to D5");

        let after = apply(&state, puppet);
        assert_eq!(
            BitBoard(after.board.god_data[1]),
            Square::D5.to_board(),
            "the flag follows the Worker it describes"
        );
        after.validate();

        // Moving their *other* Worker leaves the flag where it is.
        let other = puppet_moves(&state)
            .into_iter()
            .find(|m| m.move_from_position() == Square::E1)
            .expect("their male Worker is available too");
        let after_other = apply(&state, other);
        assert_eq!(BitBoard(after_other.board.god_data[1]), Square::E5.to_board());
        after_other.validate();
    }

    /// Harpies' song catches "an opponent's Worker", and the Worker Eris is moving is Harpies'
    /// own - so it takes one step and stops. Her own Workers are dragged the full distance.
    #[test]
    fn harpies_does_not_slide_a_puppet() {
        let state = parse_fen("0000000000000000000000000/1/eris:C3,A1/harpies:C1,E5").unwrap();

        for m in puppet_moves(&state) {
            assert!(
                NEIGHBOR_MAP[m.move_from_position() as usize]
                    .contains_square(m.move_to_position()),
                "a puppeted Worker takes a single step: {:?}",
                m
            );
        }

        assert!(
            all_moves(&state).iter().any(|m| !m.get_is_puppet()
                && !NEIGHBOR_MAP[m.move_from_position() as usize]
                    .contains_square(m.move_to_position())),
            "her own Workers are still being slid, or this position proves nothing"
        );
    }

    /// Clio's ruling - no puppeting a Worker that is standing on coins - is the exclusion the rest
    /// of the generator already applies to every coin square, so it costs one mask intersection.
    #[test]
    fn coins_protect_the_worker_standing_on_them() {
        let state =
            parse_fen("00000 00000 00100 00000 00000/1/eris:A1,A2/clio[2|C3]:C3,E1").unwrap();

        let starts: Vec<Square> = puppet_moves(&state)
            .iter()
            .map(|m| m.move_from_position())
            .collect();
        assert!(!starts.is_empty());
        assert!(
            starts.iter().all(|s| *s == Square::E1),
            "the Worker on the coin is out of reach: {:?}",
            starts
        );
    }

    /// Aeolus' wind constrains where a puppeted Worker may step exactly as it constrains hers -
    /// the same "follows the mover" rule as Athena's ban. Referenced by
    /// `aeolus::tests::test_all_gods_respect_aeolus`, which counts only her own moves for it.
    #[test]
    fn wind_applies_to_puppet_moves() {
        let state = parse_fen("0000000000000000000000000/1/eris:A1,A2/aeolus[n]:C3,E1").unwrap();
        let wind_idx = state.gods[1].get_wind_idx(&state.board, Player::Two);
        let allowed = WIND_AWARE_NEIGHBOR_MAP[wind_idx][Square::C3 as usize];

        assert!(allowed != NEIGHBOR_MAP[Square::C3 as usize], "wind is blowing");

        let destinations: Vec<Square> = puppet_moves(&state)
            .iter()
            .filter(|m| m.move_from_position() == Square::C3)
            .map(|m| m.move_to_position())
            .collect();
        assert!(!destinations.is_empty());
        for destination in destinations {
            assert!(
                allowed.contains_square(destination),
                "puppet stepped across the wind to {}",
                destination
            );
        }
    }

    /// A god that moves both of its Workers puts both of them out of reach, and Eris has an
    /// ordinary Mortal turn instead. Hermes and Castor can do this whenever they like, which is a
    /// strategic dimension rather than a bug; Terpsichore has to do it every turn, which is why
    /// that matchup is banned.
    #[test]
    fn moving_both_workers_switches_her_off() {
        let state = parse_fen("0000000000000000000000000/2/eris:A5,A4/hermes:E5,E1").unwrap();
        let (active, other) = state.get_active_non_active_gods();

        let double = active
            .get_all_moves(&state, Player::Two)
            .into_iter()
            .map(|m| state.next_state(active, other, m.action))
            .find(|next| (next.board.workers[1] & state.board.workers[1]).is_empty())
            .expect("Hermes should be able to move both Workers");

        assert_eq!(BitBoard(double.board.god_data[0]).count_ones(), 2);
        assert!(
            puppet_moves(&double).is_empty(),
            "both of their Workers are off limits"
        );
        assert!(!all_moves(&double).is_empty(), "she still has her own turn");
        double.validate();
    }

    /// A full search playing both sides, with the consistency checker run on every position the
    /// game passes through. The opponents are the ones whose powers the puppet branch has to
    /// answer for: a female Worker that has to travel, a wind and a climb ban that follow the
    /// mover, a build restriction measured from where their Workers end up, and the two gods that
    /// name Workers by owner and so do not reach a puppet at all.
    #[test]
    fn search_playout_against_the_interesting_powers() {
        for opponent in [
            GodName::Selene,
            GodName::Aeolus,
            GodName::Athena,
            GodName::Hypnus,
            GodName::Harpies,
            GodName::Clio,
        ] {
            run_search_playout(opponent, true);
        }
    }

    /// Limus is the one power that "the restriction follows the mover" switches her off entirely,
    /// and it is worth saying so out loud rather than discovering it as a silent absence.
    ///
    /// His ban is on building beside his Workers, and a puppeted Worker is one of his - standing
    /// on the very square it would be building next to. So every build a puppet could make is
    /// banned, bar a dome capping a level 3 square, and a turn with no legal build is no turn.
    /// She still has her own Mortal moves; it is only the power that goes quiet.
    #[test]
    fn limus_leaves_a_puppet_nowhere_to_build() {
        let flat = parse_fen("0000000000000000000000000/1/eris:A1,A2/limus:C3,E1").unwrap();
        assert!(
            puppet_moves(&flat).is_empty(),
            "every square beside where the puppet lands is beside a Limus Worker"
        );
        assert!(!all_moves(&flat).is_empty(), "her own turn is untouched");

        // The level 3 exemption is his own, and it is the one build a puppet still has.
        let capped = parse_fen("00000 00000 00030 00000 00000/1/eris:A1,A2/limus:C3,E1").unwrap();
        assert!(
            puppet_moves(&capped)
                .iter()
                .any(|m| m.build_position() == Square::D3),
            "capping the level 3 square is still allowed"
        );

        run_search_playout(GodName::Limus, false);
    }

    fn run_search_playout(opponent: GodName, expect_puppets_offered: bool) {
        use crate::{
            board::GameStateBuilder,
            search::{SearchContext, get_win_reached_search_terminator, negamax_search},
            search_terminators::DynamicMaxDepthSearchTerminator,
            square::Square::*,
            transposition_table::TranspositionTable,
        };

        let mut state = GameStateBuilder::new(GodName::Eris, opponent)
            .with_p1_worker(B2)
            .with_p1_worker(D4)
            .with_p2_worker(B4)
            .with_p2_worker(D2)
            .build();

        let mut tt = TranspositionTable::new();
        let mut saw_puppet_offered = false;

        for _ in 0..14 {
            if state.board.get_winner().is_some() {
                break;
            }
            consistency_check(&state).unwrap();

            if state.board.current_player == Player::One {
                saw_puppet_offered |= !puppet_moves(&state).is_empty();
            }

            let mut search_context = SearchContext {
                tt: &mut tt,
                new_best_move_callback: Box::new(|_| {}),
                terminator: DynamicMaxDepthSearchTerminator::new(3),
            };
            let search_state = negamax_search(
                &mut search_context,
                state.clone(),
                get_win_reached_search_terminator(),
            );

            let best_move = search_state.best_move.expect("Search found no move");
            let (active_god, oppo_god) = state.get_active_non_active_gods();
            state = state.next_state(active_god, oppo_god, best_move.action);
        }

        // Not an assertion about the search's taste, only that the power was on the table at some
        // point - otherwise the playout never exercised the branch it was written for.
        assert_eq!(
            saw_puppet_offered, expect_puppets_offered,
            "vs {opponent}: puppet moves offered: {saw_puppet_offered}"
        );
    }
}
