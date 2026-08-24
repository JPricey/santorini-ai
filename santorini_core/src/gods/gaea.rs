//! Gaea, Titan of the Earth.
//!
//! "Setup: place 2 extra Workers of your colour on your God Power card. Any Build: when a Worker
//! builds a dome, Gaea may immediately place a Worker from her God Power card onto a ground level
//! space neighbouring the dome."
//!
//! She is the only god in the engine who wants to act *during* somebody else's turn, and a turn
//! here is one atomic [`GodMove::make_move`]. The way out is to defer: the dome is recorded when it
//! goes up and the Worker is placed at the top of her next turn.
//!
//! The trigger splits in two, and only half of it needs deferring.
//!
//! - **Her own domes resolve inline.** Her build is the last thing her turn does, so "immediately
//!   after the build" *is* the end of her turn. The placement is another field in her own move,
//!   exactly as Jason's `maybe_place_position` is.
//! - **The opponent's domes are deferred.** [`BoardState::on_turn_advanced`] diffs the dome map
//!   across their turn and writes the newly domed squares into her `god_data`; her generator turns
//!   each of them into a placement at the start of her next turn.
//!
//! Deferring is observationally identical to placing immediately whenever the set of free ground
//! level squares beside the dome is unchanged between the dome and the start of her turn. That
//! covers every opponent whose build is the last thing their turn does, and - because this engine's
//! move encodings carry no build *order* - every opponent who builds more than once as well, since
//! doming last is both the only reading the model supports and the one optimal play would pick.
//!
//! It is an approximation against the eight gods who genuinely fix something after the dome:
//! Prometheus, Achilles, Hestia, Ares, Theseus, Medusa, Hydra and Europa. There her placement is
//! computed on the board as it stands at the *end* of their turn, so she loses squares that were
//! built on or stepped onto afterwards and gains squares that were freed afterwards. None of it can
//! touch a win: a placement lands at ground level, so it can never occupy a level 3 square.
//!
//! Points the card leaves open, and which way they go here:
//!
//! - **A dome at any level counts**, not only one that completes a tower - `dome_up` writes a dome
//!   into all four height maps and nothing afterwards can tell the two apart. That is the reading
//!   the official ban list assumes: Atlas and Selene are banned against her precisely because
//!   doming at will would be degenerate, which it only is if partial domes trigger her.
//! - **A trigger she cannot use is not a wasted charge.** No free ground level neighbour means no
//!   Worker placed and no Worker spent, per the published ruling. Declining likewise.
//! - **One placement per dome.** Two domes in one opponent turn owe her two Workers, and the two
//!   placements have to answer to *different* domes - see [`GaeaTurn::domes_can_be_matched`].
//! - **Placements resolve before her move**, which is as close as the model gets to the Worker
//!   landing the instant the opponent domed. It matters: with both her Workers walled in, a Worker
//!   placed at the top of the turn is the one that moves and builds, where a Worker placed at the
//!   end would leave her with no move at all.
//! - **A placement is not a move, but a Worker she placed and then moved is.** Putting a Worker
//!   down does not slide it past Harpies and does not climb for Persephone. Moving it afterwards
//!   does both, and Aphrodite's pull reaches it as well: a Worker resolved onto the board at the
//!   top of her turn is standing in the affinity area when it moves, which is the reading Jason's
//!   placed Worker already gets. See [`gaea_vs_persephone`] for the one ruling that needed making.
//! - **A winning move places nothing.** She has already won, and the turn has no build to trigger
//!   her own dome. Wins are generated only on the no-placement board, which is exact because a
//!   placement can neither create a win - a Worker arriving at ground level cannot climb to level 3
//!   this turn, and Hypnus measures his freeze from level 1 upwards, so a ground level arrival
//!   changes nothing there either - nor block one, since every winning square is at level 3 and
//!   every placement square is at level 0.
//!
//! [`BoardState::on_turn_advanced`]: crate::board::BoardState::on_turn_advanced

use std::{borrow::Cow, collections::HashSet};

use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, PartialAction, StaticGod,
        build_god_power_actions,
        generic::{
            ANY_MOVE_FILTER, GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK,
            MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        move_helpers::{
            build_scored_move, get_generator_prelude_state, get_sized_result,
            get_standard_reach_board, get_worker_end_move_state, get_worker_next_build_state_with_is_matched,
            get_worker_next_move_state, get_worker_start_move_state, is_interact_with_key_squares,
            is_mate_only, modify_prelude_for_checking_workers, push_winning_moves,
        },
    },
    player::Player,
    square::Square,
};

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = MOVE_FROM_POSITION_OFFSET + POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;
const DEFERRED_1_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;
const DEFERRED_2_OFFSET: usize = DEFERRED_1_OFFSET + POSITION_WIDTH;
const OWN_PLACE_OFFSET: usize = DEFERRED_2_OFFSET + POSITION_WIDTH;

/// 25 is not a square, so it is the "no Worker placed here" sentinel - Jason's `NO_PLACEMENT`.
const NO_PLACEMENT: MoveData = 25;

/// Three placement slots take the encoding to bit 29 inclusive, which is exactly the top of
/// `MOVE_DATA_MAIN_SECTION`. The check and win flags at 30 and 31 stay clear.
const _MOVE_FITS_ASSERT: () = assert!(OWN_PLACE_OFFSET + POSITION_WIDTH == 30);

/// Her `god_data`: bits 0-24 are the squares domed on the opponent's last turn and still owed a
/// Worker, bits 25-26 are the Workers already spent off her card.
const GOD_DATA_SPENT_OFFSET: usize = 25;
pub const GAEA_SPENT_WORKER_MASK: GodData = 0b11 << GOD_DATA_SPENT_OFFSET;

/// Two extra Workers on the card. "Twice per game" is a consequence of that supply rather than a
/// clock of its own.
pub const GAEA_EXTRA_WORKERS: u32 = 2;

/// How many Workers she has already taken off her card.
pub fn gaea_workers_spent(god_data: GodData) -> u32 {
    god_data >> GOD_DATA_SPENT_OFFSET
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct GaeaMove(pub MoveData);

impl GaeaMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
        encoded_deferred: MoveData,
    ) -> Self {
        Self(
            ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
                | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
                | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
                | encoded_deferred
                | (NO_PLACEMENT << OWN_PLACE_OFFSET),
        )
    }

    fn with_own_placement(self, own_place_position: Square) -> Self {
        Self(
            (self.0 & !(NO_PLACEMENT << OWN_PLACE_OFFSET))
                | ((own_place_position as MoveData) << OWN_PLACE_OFFSET),
        )
    }

    /// A win ends the turn where it stands: no build, so no dome of her own, and nothing deferred
    /// is resolved either.
    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        Self(
            ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
                | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
                | (NO_PLACEMENT << DEFERRED_1_OFFSET)
                | (NO_PLACEMENT << DEFERRED_2_OFFSET)
                | (NO_PLACEMENT << OWN_PLACE_OFFSET)
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

    fn maybe_square_at(&self, offset: usize) -> Option<Square> {
        let value = (self.0 >> offset) as u8 & LOWER_POSITION_MASK;
        if value == NO_PLACEMENT as u8 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    /// Where the Workers owed by the opponent's domes land, before she moves.
    pub fn deferred_placement_mask(&self) -> BitBoard {
        let mut res = BitBoard::EMPTY;
        if let Some(square) = self.maybe_square_at(DEFERRED_1_OFFSET) {
            res |= BitBoard::as_mask(square);
        }
        if let Some(square) = self.maybe_square_at(DEFERRED_2_OFFSET) {
            res |= BitBoard::as_mask(square);
        }
        res
    }

    /// Where the Worker owed by a dome she raised herself lands, after she builds.
    pub fn maybe_own_place_position(&self) -> Option<Square> {
        self.maybe_square_at(OWN_PLACE_OFFSET)
    }

    /// Every Worker this move puts on the board, whenever in the turn it arrives.
    pub fn placement_mask(&self) -> BitBoard {
        let mut res = self.deferred_placement_mask();
        if let Some(square) = self.maybe_own_place_position() {
            res |= BitBoard::as_mask(square);
        }
        res
    }

    pub fn move_mask(&self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) ^ BitBoard::as_mask(self.move_to_position())
    }

    pub fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl Into<GenericMove> for GaeaMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for GaeaMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl std::fmt::Debug for GaeaMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        for square in self.deferred_placement_mask() {
            write!(f, "+{}", square)?;
        }

        let move_from = self.move_from_position();
        let move_to = self.move_to_position();
        if self.get_is_winning() {
            return write!(f, "{}>{}#", move_from, move_to);
        }

        write!(f, "{}>{}^{}", move_from, move_to, self.build_position())?;

        if let Some(square) = self.maybe_own_place_position() {
            write!(f, "+{}", square)?;
        }

        Ok(())
    }
}

impl GodMove for GaeaMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        let mut res: FullAction = self
            .deferred_placement_mask()
            .all_squares()
            .into_iter()
            .map(PartialAction::HeroActionPlacement)
            .collect();

        res.push(PartialAction::SelectWorker(self.move_from_position()));
        res.push(PartialAction::MoveWorker(self.move_to_position().into()));

        if self.get_is_winning() {
            return vec![res];
        }

        res.push(PartialAction::Build(self.build_position()));

        if let Some(square) = self.maybe_own_place_position() {
            res.push(PartialAction::HeroActionPlacement(square));
        }

        vec![res]
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        let deferred = self.deferred_placement_mask();
        let own_placement = self.maybe_own_place_position();

        // Her card and her memory of the opponent's domes are both settled by the turn as a whole.
        // The pending squares are cleared whether or not she used them: the moment passes, the
        // charges do not.
        let spent = gaea_workers_spent(board.god_data[player as usize])
            + deferred.count_ones()
            + own_placement.is_some() as u32;
        board.set_god_data(player, spent << GOD_DATA_SPENT_OFFSET);

        if deferred.is_not_empty() {
            board.worker_xor(player, deferred);
        }

        board.worker_xor(player, self.move_mask());

        if self.get_is_winning() {
            board.set_winner(player);
            return;
        }

        board.build_up(self.build_position());

        if let Some(square) = own_placement {
            board.worker_xor(player, BitBoard::as_mask(square));
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
        helper.add_maybe_square_with_height(board, self.maybe_square_at(DEFERRED_1_OFFSET));
        helper.add_maybe_square_with_height(board, self.maybe_square_at(DEFERRED_2_OFFSET));
        helper.add_maybe_square_with_height(board, self.maybe_own_place_position());
        helper.get()
    }
}

/// The squares her deferred Workers land on, packed into the move's two deferred slots.
///
/// Bitboard iteration runs from the low square upwards, so the pair is always stored in the same
/// order - the two placements happen at once and the board cannot tell them apart, so without a
/// canonical order the same turn would be emitted twice.
fn encode_deferred(deferred: BitBoard) -> MoveData {
    let mut squares = deferred.all_squares().into_iter();
    let first = squares
        .next()
        .map_or(NO_PLACEMENT, |square| square as MoveData);
    let second = squares
        .next()
        .map_or(NO_PLACEMENT, |square| square as MoveData);

    (first << DEFERRED_1_OFFSET) | (second << DEFERRED_2_OFFSET)
}

/// Everything about her turn that does not depend on which Workers she puts down.
struct GaeaTurn<'a> {
    state: &'a FullGameState,
    player: Player,
    key_squares: BitBoard,

    /// Squares domed on the opponent's last turn, each still owing her a Worker.
    pending_domes: BitBoard,
    /// Workers still on her card.
    charges: u32,
    /// Where a deferred Worker may land, measured before any of them have landed.
    deferred_targets: BitBoard,
}

/// One choice of deferred placements, and the board her turn is then played on.
struct GaeaScenario<'a> {
    state: Cow<'a, FullGameState>,
    deferred: BitBoard,
    encoded_deferred: MoveData,
    /// Workers left on the card once these have landed - what her own dome could still spend.
    charges_left: u32,
}

impl<'a> GaeaTurn<'a> {
    fn new(state: &'a FullGameState, player: Player, key_squares: BitBoard) -> Self {
        let board = &state.board;
        let god_data = board.god_data[player as usize];

        // The mask only ever holds squares that were domed, and nothing can lower a dome, so the
        // intersection is a no-op in play. It keeps the invariant local for hand written FENs.
        let pending_domes = BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0) & board.height_map[3];
        let charges = GAEA_EXTRA_WORKERS.saturating_sub(gaea_workers_spent(god_data));

        let deferred_targets = if charges == 0 || pending_domes.is_empty() {
            BitBoard::EMPTY
        } else {
            let other_god = state.get_god_for_player(!player);
            let occupied = board.workers[0]
                | board.workers[1]
                | other_god.get_frozen_mask(board, !player);

            apply_mapping_to_mask(pending_domes, &NEIGHBOR_MAP)
                & board.exactly_level_0()
                & !occupied
        };

        Self {
            state,
            player,
            key_squares,
            pending_domes,
            charges,
            deferred_targets,
        }
    }

    /// The turn as she plays it if she declines every deferred placement.
    fn base_scenario(&self) -> GaeaScenario<'a> {
        GaeaScenario {
            state: Cow::Borrowed(self.state),
            deferred: BitBoard::EMPTY,
            encoded_deferred: encode_deferred(BitBoard::EMPTY),
            charges_left: self.charges,
        }
    }

    fn scenario(&self, deferred: BitBoard) -> GaeaScenario<'a> {
        let mut state = self.state.clone();
        state.board.worker_xor(self.player, deferred);

        GaeaScenario {
            state: Cow::Owned(state),
            deferred,
            encoded_deferred: encode_deferred(deferred),
            charges_left: self.charges - deferred.count_ones(),
        }
    }

    /// Whether two placements can answer to *different* pending domes, which is what stops her
    /// dropping both Workers beside the one dome that owes her a single Worker.
    ///
    /// Both squares come from [`Self::deferred_targets`], so each already neighbours a pending
    /// dome and the only question left is whether the two can be told apart.
    fn domes_can_be_matched(&self, first: Square, second: Square) -> bool {
        let first_domes = self.pending_domes & NEIGHBOR_MAP[first as usize];
        let second_domes = self.pending_domes & NEIGHBOR_MAP[second as usize];

        (first_domes | second_domes).count_ones() >= 2
    }

    fn placement_scenarios(&self) -> Vec<GaeaScenario<'a>> {
        let mut res = Vec::new();
        if self.deferred_targets.is_empty() {
            return res;
        }

        for first in self.deferred_targets {
            res.push(self.scenario(BitBoard::as_mask(first)));
        }

        if self.charges < 2 {
            return res;
        }

        for first in self.deferred_targets {
            for second in self.deferred_targets {
                if (second as u8) <= (first as u8) {
                    continue;
                }
                if !self.domes_can_be_matched(first, second) {
                    continue;
                }

                res.push(self.scenario(BitBoard::as_mask(first) | BitBoard::as_mask(second)));
            }
        }

        res
    }
}

/// A key for the position a move produces, which is all a duplicate is.
///
/// Two Workers of the same colour are indistinguishable once they are on the board, so the Worker
/// she places and the Worker she moves can trade places - `B2>B1^C1+C2` and `B2>C2^C1+B1` end the
/// turn identically. The same collision happens between a deferred placement and a move
/// destination, and between a deferred placement and one hung off her own dome. Which splits
/// collide depends on adjacency to the dome, to the mover and to the build at once, so they are
/// ruled out by the position they reach rather than by rule.
///
/// Her Workers after the turn fit in the low 25 bits, and the build square rides above them. How
/// many Workers she spent is implied by how many more Workers she has than she started with.
fn resulting_position_key(original_workers: BitBoard, action: GaeaMove) -> u32 {
    let mut workers = (original_workers | action.deferred_placement_mask()) ^ action.move_mask();
    if let Some(square) = action.maybe_own_place_position() {
        workers |= BitBoard::as_mask(square);
    }

    workers.0 | ((action.build_position() as u32) << 25)
}

/// Drop every move that reaches a position an earlier move already reached.
///
/// Only moves that place a Worker can collide - the rest are Mortal's, and Mortal does not repeat
/// itself - so the scan is free in the ordinary case and bounded by the placements in the rest.
/// `retain` keeps the first of each pair, which keeps a winning move last in the list.
fn dedupe_by_resulting_position(turn: &GaeaTurn, result: &mut Vec<ScoredMove>) {
    // Nothing to place, or nowhere a dome could have come from this turn.
    if turn.charges == 0
        || (turn.deferred_targets.is_empty() && turn.state.board.exactly_level_3().is_empty())
    {
        return;
    }

    let original_workers = turn.state.board.workers[turn.player as usize];
    let mut seen = HashSet::new();

    result.retain(|scored| {
        let action = GaeaMove::from(scored.action);
        if action.placement_mask().is_empty() {
            return true;
        }

        seen.insert(resulting_position_key(original_workers, action))
    });
}

pub(super) fn gaea_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if MUST_CLIMB {
        // `gaea_vs_persephone` drives the must-climb generators itself
        unreachable!();
    }

    let turn = GaeaTurn::new(state, player, key_squares);

    if state.get_god_for_player(!player).is_persephone {
        return gaea_vs_persephone::<F>(&turn);
    }

    let mut result = get_sized_result::<F>();
    let stop = add_scenario_moves::<F, false>(&turn, &turn.base_scenario(), &mut result);

    // A placement neither wins nor blocks a win, so a mate search is done with the base board.
    if !stop && !is_mate_only::<F>() {
        for scenario in turn.placement_scenarios() {
            add_scenario_moves::<F, false>(&turn, &scenario, &mut result);
        }
    }

    dedupe_by_resulting_position(&turn, &mut result);
    result
}

/// Persephone's demand is on the Worker that *moves*, and a placement is not a move.
///
/// Jason's shape, one scenario wider. If she can climb without placing anything then every turn she
/// plays has to climb. If she cannot, Persephone may not force her to spend a Worker off her card
/// just to find a climb - so declining stays open - but any placement she does make is judged on
/// the board it produces: if that board offers a climb, she has to take it.
fn gaea_vs_persephone<const F: MoveGenFlags>(turn: &GaeaTurn) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();
    let base = turn.base_scenario();

    if add_scenario_moves::<F, true>(turn, &base, &mut result) {
        dedupe_by_resulting_position(turn, &mut result);
        return result;
    }

    let must_climb = result.len() > 0
        || filtered_out_a_climb::<F, _>(|out| {
            add_scenario_moves::<0, true>(turn, &base, out);
        });

    if is_mate_only::<F>() {
        return result;
    }

    if must_climb {
        for scenario in turn.placement_scenarios() {
            add_scenario_moves::<F, true>(turn, &scenario, &mut result);
        }

        dedupe_by_resulting_position(turn, &mut result);
        return result;
    }

    add_scenario_moves::<F, false>(turn, &base, &mut result);

    for scenario in turn.placement_scenarios() {
        let flat_moves_only = result.len();
        add_scenario_moves::<F, true>(turn, &scenario, &mut result);

        let can_climb = result.len() > flat_moves_only
            || filtered_out_a_climb::<F, _>(|out| {
                add_scenario_moves::<0, true>(turn, &scenario, out);
            });

        if !can_climb {
            add_scenario_moves::<F, false>(turn, &scenario, &mut result);
        }
    }

    dedupe_by_resulting_position(turn, &mut result);
    result
}

/// An empty run under a mate/key-square filter doesn't mean there was no climb to find
/// check if any non-filtered moves were available
fn filtered_out_a_climb<const F: MoveGenFlags, G: FnOnce(&mut Vec<ScoredMove>)>(
    generate_unfiltered: G,
) -> bool {
    if F & ANY_MOVE_FILTER == 0 {
        return false;
    }

    let mut unfiltered = get_sized_result::<0>();
    generate_unfiltered(&mut unfiltered);
    unfiltered.len() > 0
}

/// Mortal's turn, played on the board this scenario's placements have already produced.
///
/// Returns true when the caller should stop - a win was found under `STOP_ON_MATE`.
fn add_scenario_moves<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    turn: &GaeaTurn,
    scenario: &GaeaScenario,
    result: &mut Vec<ScoredMove>,
) -> bool {
    let is_base = scenario.deferred.is_empty();

    let mut prelude =
        get_generator_prelude_state::<F>(&scenario.state, turn.player, turn.key_squares);
    let checkable_mask = prelude.exactly_level_2;
    modify_prelude_for_checking_workers::<F>(checkable_mask, &mut prelude);

    let can_place_own_dome = scenario.charges_left > 0 && !is_mate_only::<F>();

    for worker_start_pos in prelude.acting_workers {
        let worker_start_state = get_worker_start_move_state(&prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(&prelude, &worker_start_state, checkable_mask);

        // A Worker standing on a key square is a block in its own right, so every turn this Worker
        // can play removes the win the key squares came from and none of them needs narrowing.
        // Only a placement she leaves *standing* counts: the Worker she puts down at the top of
        // the turn may be the one that moves, and a square it walks off blocks nothing.
        let placement_blocks = is_interact_with_key_squares::<F>()
            && (scenario.deferred & prelude.key_squares & !worker_start_state.worker_start_mask)
                .is_not_empty();

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;

            // Wins come off the base board only, so that a scenario never emits a win a later
            // scenario would repeat - and so that a win is always the last move in the list.
            if is_base
                && push_winning_moves::<F, GaeaMove, _>(
                    result,
                    worker_start_pos,
                    moves_to_level_3,
                    GaeaMove::new_winning_move,
                )
            {
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
            let worker_next_build_state = get_worker_next_build_state_with_is_matched::<F>(
                &prelude,
                &worker_start_state,
                &worker_end_move_state,
                placement_blocks
                    || (worker_end_move_state.worker_end_mask & prelude.key_squares)
                        .is_not_empty(),
            );
            let reach_board = get_standard_reach_board::<F>(
                &prelude,
                &worker_next_moves,
                &worker_end_move_state,
                worker_next_build_state.unblocked_squares,
            );

            // While a Worker is still on her card, a build that raises a dome is worth scanning
            // even when the narrowing rejected it: the placement it triggers can be the block.
            let mut builds = worker_next_build_state.narrowed_builds;
            if can_place_own_dome && is_interact_with_key_squares::<F>() {
                builds |= worker_next_build_state.all_possible_builds & prelude.exactly_level_3;
            }

            for worker_build_pos in builds {
                let build_mask = BitBoard::as_mask(worker_build_pos);
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    (reach_board & final_level_3).is_not_empty()
                };

                let base_action = GaeaMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                    scenario.encoded_deferred,
                );

                let is_narrowed_build =
                    (worker_next_build_state.narrowed_builds & build_mask).is_not_empty();
                if is_narrowed_build {
                    result.push(build_scored_move::<F, _>(
                        base_action,
                        is_check,
                        worker_end_move_state.is_improving,
                    ));
                }

                if !can_place_own_dome || (prelude.exactly_level_3 & build_mask).is_empty() {
                    continue;
                }

                let mut own_targets = NEIGHBOR_MAP[worker_build_pos as usize]
                    & worker_next_build_state.unblocked_squares
                    & prelude.exactly_level_0;

                if !is_narrowed_build {
                    own_targets &= prelude.key_squares;
                }

                for own_place_pos in own_targets {
                    result.push(build_scored_move::<F, _>(
                        base_action.with_own_placement(own_place_pos),
                        is_check,
                        worker_end_move_state.is_improving,
                    ));
                }
            }
        }
    }

    false
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    if data.is_empty() {
        return Ok(0);
    }

    let splits = data.split('|').collect::<Vec<_>>();
    if splits.len() != 2 {
        return Err("Gaea data must be <workers on her card>|<comma separated dome squares>".into());
    }

    let remaining = splits[0]
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("Failed to parse remaining workers '{}': {}", splits[0], e))?;

    if remaining > GAEA_EXTRA_WORKERS {
        return Err(format!(
            "Gaea only has {} workers on her card",
            GAEA_EXTRA_WORKERS
        ));
    }

    let mut pending = BitBoard::EMPTY;
    if !splits[1].trim().is_empty() {
        for part in splits[1].split(',') {
            let square: Square = part
                .trim()
                .parse()
                .map_err(|e| format!("Failed to parse square {}: {:?}", part, e))?;
            pending |= BitBoard::as_mask(square);
        }
    }

    Ok(pending.0 | ((GAEA_EXTRA_WORKERS - remaining) << GOD_DATA_SPENT_OFFSET))
}

fn stringify_god_data(data: GodData) -> Option<String> {
    match data {
        0 => None,
        x => Some(format!(
            "{}|{}",
            GAEA_EXTRA_WORKERS - (x >> GOD_DATA_SPENT_OFFSET),
            BitBoard(x & BitBoard::MAIN_SECTION_MASK.0)
                .all_squares()
                .iter()
                .map(Square::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )),
    }
}

fn pretty_stringify_god_data(board: &BoardState, player: Player) -> Option<String> {
    let data = board.god_data[player as usize];
    let remaining = GAEA_EXTRA_WORKERS - (data >> GOD_DATA_SPENT_OFFSET);
    let pending = BitBoard(data & BitBoard::MAIN_SECTION_MASK.0);

    let mut res = format!("{} Workers on her card.", remaining);
    if pending.is_not_empty() {
        res.push_str(&format!(
            " Owed a Worker beside {}.",
            pending
                .all_squares()
                .iter()
                .map(Square::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    Some(res)
}

fn flip_horizontal(god_data: GodData) -> GodData {
    BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0)
        .flip_horizontal()
        .0
        | (god_data & GAEA_SPENT_WORKER_MASK)
}

fn flip_vertical(god_data: GodData) -> GodData {
    BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0)
        .flip_vertical()
        .0
        | (god_data & GAEA_SPENT_WORKER_MASK)
}

fn flip_transpose(god_data: GodData) -> GodData {
    BitBoard(god_data & BitBoard::MAIN_SECTION_MASK.0)
        .flip_transpose()
        .0
        | (god_data & GAEA_SPENT_WORKER_MASK)
}

pub const fn build_gaea() -> GodPower {
    god_power(
        GodName::Gaea,
        build_god_power_movers!(gaea_move_gen),
        build_god_power_actions::<GaeaMove>(),
        11417570496126429419,
        6284015103497582141,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_is_gaea()
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
        consistency_checker::consistency_check,
        fen::{game_state_to_fen, parse_fen},
        move_verifier::MoveVerifier,
        square::Square::*,
    };

    fn all_moves(state: &FullGameState) -> Vec<GaeaMove> {
        let god = state.get_active_god();
        god.get_all_moves(state, state.board.current_player)
            .into_iter()
            .map(|m| GaeaMove::from(m.action))
            .collect()
    }

    fn placing_moves(state: &FullGameState) -> Vec<GaeaMove> {
        all_moves(state)
            .into_iter()
            .filter(|m| m.placement_mask().is_not_empty())
            .collect()
    }

    fn apply(state: &FullGameState, action: GaeaMove) -> FullGameState {
        let (active, other) = state.get_active_non_active_gods();
        state.next_state(active, other, action.into())
    }

    /// A turn of theirs that raises a dome, played out through the real hook.
    fn opponent_turn_domeing(state: &FullGameState, build_position: Square) -> FullGameState {
        let (active, other) = state.get_active_non_active_gods();
        let action = active
            .get_all_moves(state, state.board.current_player)
            .into_iter()
            .map(|m| crate::gods::mortal::MortalMove::from(m.action))
            .find(|m| m.build_position() == build_position)
            .expect("the opponent should be able to build there");

        state.next_state(active, other, action.into())
    }

    /// Both halves of her `god_data` ride in the FEN, printed the way Clio prints her coins:
    /// Workers left on the card, then the squares still owed one.
    #[test]
    fn fen_round_trips_the_card_and_the_pending_domes() {
        for fen in [
            "0000000000000000000000000/1/gaea:A5,A4/mortal:E5,E4",
            "0000000000004000000000000/1/gaea[2|C3]:A5,A4/mortal:E5,E4",
            "0000000000404000000000000/1/gaea[1|A3,C3]:A5,A4,B4/mortal:E5,E4",
            "0000000000000000000000000/1/gaea[0|]:A5,A4,B4,C4/mortal:E5,E4",
        ] {
            let state = parse_fen(fen).unwrap();
            assert_eq!(game_state_to_fen(&state), fen);
            state.validate();
        }
    }

    /// The whole point of the god: their dome is remembered, and answered on her next turn.
    #[test]
    fn an_opponent_dome_owes_her_a_worker() {
        // D5 is already at level 3, so their build tops it off.
        let theirs = parse_fen("00030 00000 00000 00000 00000/2/gaea:A1,A2/mortal:C4,E1").unwrap();
        assert_eq!(theirs.board.god_data[0], 0);

        let hers = opponent_turn_domeing(&theirs, D5);
        assert_eq!(
            BitBoard(hers.board.god_data[0] & BitBoard::MAIN_SECTION_MASK.0),
            D5.to_board(),
            "the square they domed is what she is owed a Worker beside"
        );
        hers.validate();
        consistency_check(&hers).unwrap();

        let occupied = hers.board.workers[0] | hers.board.workers[1];
        let expected =
            NEIGHBOR_MAP[D5 as usize] & hers.board.exactly_level_0() & !occupied;
        assert!(expected.is_not_empty());

        let offered = placing_moves(&hers)
            .iter()
            .fold(BitBoard::EMPTY, |acc, m| acc | m.deferred_placement_mask());
        assert_eq!(
            offered, expected,
            "every free ground square beside the dome, and nothing else"
        );
    }

    /// The trigger is "builds a dome", so a build that only raises a tower owes her nothing.
    #[test]
    fn an_ordinary_build_owes_her_nothing() {
        let theirs = parse_fen("00010 00000 00000 00000 00000/2/gaea:A1,A2/mortal:C3,E1").unwrap();
        let hers = opponent_turn_domeing(&theirs, D5);

        assert_eq!(hers.board.god_data[0], 0);
        assert!(placing_moves(&hers).is_empty());
    }

    /// Her own domes never reach the pending mask - the build *is* the end of her turn, so the
    /// placement rides in the same move. Recording it as well would pay her twice for one dome.
    #[test]
    fn her_own_dome_resolves_inside_her_own_move() {
        let state = parse_fen("00000 00000 00300 00000 00000/1/gaea:C2,E5/mortal:A5,A4").unwrap();
        consistency_check(&state).unwrap();

        // Capping C3 makes it a dome, and the Worker that owes her lands beside it.
        let capping: Vec<GaeaMove> = all_moves(&state)
            .into_iter()
            .filter(|m| m.build_position() == C3 && m.maybe_own_place_position().is_some())
            .collect();
        assert!(capping.iter().all(|m| m.deferred_placement_mask().is_empty()));

        // Every free ground square beside the new dome can hold the new Worker - though which end
        // of the pair is the Worker she moved and which the Worker she placed is not something the
        // board records, so several of them are reached by moving there and placing elsewhere.
        let free_ground = NEIGHBOR_MAP[C3 as usize] & state.board.exactly_level_0();
        let reached = capping
            .iter()
            .map(|m| apply(&state, *m).board.workers[0])
            .fold(BitBoard::EMPTY, |acc, workers| acc | workers);
        assert_eq!(free_ground & !reached, BitBoard::EMPTY);

        // Including the square she just stepped off - the Worker arrives after she has built.
        let vacated = capping
            .into_iter()
            .find(|m| m.move_from_position() == C2 && m.maybe_own_place_position() == Some(C2))
            .expect("the square she left is free ground beside the dome");
        let after = apply(&state, vacated);
        assert_eq!(after.board.get_height(C3), 4);
        assert_eq!(
            after.board.god_data[0] & BitBoard::MAIN_SECTION_MASK.0,
            0,
            "the dome she just raised is already paid for"
        );
        assert_eq!(after.board.workers[0].count_ones(), 3);
        after.validate();
    }

    /// She may always decline, and declining costs her nothing but the moment.
    #[test]
    fn declining_keeps_the_worker_and_clears_the_debt() {
        let hers = parse_fen("00040 00000 00000 00000 00000/1/gaea[2|D5]:A1,A2/mortal:C3,E1")
            .unwrap();
        consistency_check(&hers).unwrap();

        let declined = all_moves(&hers)
            .into_iter()
            .find(|m| m.placement_mask().is_empty())
            .expect("declining is always on the table");

        let after = apply(&hers, declined);
        assert_eq!(after.board.workers[0].count_ones(), 2);
        assert_eq!(
            after.board.god_data[0], 0,
            "the card is untouched and the debt has expired"
        );
        after.validate();
    }

    /// Per the published ruling: a dome with nowhere free beside it is not a wasted charge.
    #[test]
    fn a_dome_with_no_room_beside_it_spends_nothing() {
        // Domes at B5 and A4 and their Worker on B4 fill every neighbour of the dome at A5.
        let hers =
            parse_fen("44000 40000 00000 00000 00000/1/gaea[2|A5]:E1,E2/mortal:B4,C1").unwrap();
        consistency_check(&hers).unwrap();

        assert!(placing_moves(&hers).is_empty());

        let after = apply(&hers, all_moves(&hers)[0]);
        assert_eq!(after.board.workers[0].count_ones(), 2);
        assert_eq!(after.board.god_data[0], 0);
    }

    /// Two domes in one turn owe her two Workers, and the two placements have to answer to
    /// different domes - one dome is worth one Worker however many free squares surround it.
    #[test]
    fn each_placement_answers_a_different_dome() {
        // One dome at A5, with B5 and A4 free beside it. Both Workers there would be two Workers
        // off one dome.
        let one_dome = parse_fen("40000 00000 00000 00000 00000/1/gaea[2|A5]:E1,E2/mortal:C3,C1")
            .unwrap();
        consistency_check(&one_dome).unwrap();
        assert!(
            placing_moves(&one_dome)
                .iter()
                .all(|m| m.deferred_placement_mask().count_ones() <= 1),
            "one dome, one Worker"
        );

        // Two domes far enough apart to have their own neighbours.
        let two_domes =
            parse_fen("40004 00000 00000 00000 00000/1/gaea[2|A5,E5]:C1,E1/mortal:C3,B1").unwrap();
        consistency_check(&two_domes).unwrap();
        assert!(
            placing_moves(&two_domes)
                .iter()
                .any(|m| m.deferred_placement_mask().count_ones() == 2),
            "two domes, two Workers"
        );

        let next_states = two_domes.get_next_states_interactive();
        MoveVerifier::new()
            .with_p1_worker_at(B5)
            .with_p1_worker_at(D5)
            .any(&next_states);
    }

    /// Her supply is two Workers for the whole game, not two per trigger.
    #[test]
    fn the_card_runs_out() {
        let spent = parse_fen("40000 00000 00000 00000 00000/1/gaea[0|A5]:C1,E1,B5,A4/mortal:C3,B1")
            .unwrap();
        consistency_check(&spent).unwrap();

        assert!(placing_moves(&spent).is_empty());
        assert!(
            all_moves(&spent)
                .iter()
                .all(|m| m.placement_mask().is_empty())
        );
    }

    /// Once her card is empty the hook stops recording, so one spent-out position does not spread
    /// itself over a different hash for every dome that goes up afterwards.
    #[test]
    fn a_spent_card_records_nothing() {
        let theirs =
            parse_fen("00030 00000 00000 00000 00000/2/gaea[0|]:A1,A2,B1,B2/mortal:C3,E1").unwrap();
        let hers = opponent_turn_domeing(&theirs, D5);

        assert_eq!(gaea_workers_spent(hers.board.god_data[0]), GAEA_EXTRA_WORKERS);
        assert_eq!(
            hers.board.god_data[0] & BitBoard::MAIN_SECTION_MASK.0,
            0,
            "with nothing left to place there is nothing worth remembering"
        );
    }

    /// Placements resolve at the top of her turn, which is what lets the new Worker be the one
    /// that moves - and with both her own Workers walled in, the only one that can.
    #[test]
    fn a_placed_worker_can_take_the_turn() {
        // Her Workers at A1 and A2 wall each other in behind domes; the opponent has just
        // domed E1, which leaves free ground at D1, D2 and E2.
        let hers =
            parse_fen("00000 00000 44000 04000 04004/1/gaea[2|E1]:A1,A2/mortal:C3,C1").unwrap();
        consistency_check(&hers).unwrap();

        let moves = all_moves(&hers);
        assert!(!moves.is_empty(), "the placement is her whole turn");
        assert!(
            moves
                .iter()
                .all(|m| m.deferred_placement_mask().contains_square(m.move_from_position())),
            "every move she has is played with the Worker she just placed"
        );

        let next_states = hers.get_next_states_interactive();
        MoveVerifier::new().with_p1_worker_at(D1).any(&next_states);
    }

    /// A win ends the turn where it stands, so nothing is placed - and the debt expires anyway.
    #[test]
    fn a_winning_move_places_nothing() {
        let hers =
            parse_fen("00000 00000 03000 02000 40000/1/gaea[2|A1]:B2,E5/mortal:E1,E2").unwrap();
        consistency_check(&hers).unwrap();

        let win = all_moves(&hers)
            .into_iter()
            .find(|m| m.get_is_winning())
            .expect("B2 stands on level 2 next to B3");
        assert!(win.placement_mask().is_empty());

        let after = apply(&hers, win);
        assert_eq!(after.get_winner(), Some(Player::One));
        assert_eq!(after.board.workers[0].count_ones(), 2);
    }

    /// Two Workers of the same colour are the same piece once they are down, so the Worker she
    /// places and the Worker she moves can trade places without changing anything.
    #[test]
    fn a_placement_and_a_move_that_trade_places_are_one_turn() {
        let state = parse_fen("00000 00000 00300 00000 00000/1/gaea:C2,E5/mortal:A5,A4").unwrap();
        consistency_check(&state).unwrap();

        let capping: Vec<GaeaMove> = all_moves(&state)
            .into_iter()
            .filter(|m| m.build_position() == C3 && m.maybe_own_place_position().is_some())
            .collect();

        let mut seen = HashSet::new();
        for action in &capping {
            let key =
                resulting_position_key(state.board.workers[0], *action);
            assert!(seen.insert(key), "{:?} repeats a position", action);
        }
    }

    /// A dome that ends the game owes her nothing - there is no turn left to place on.
    #[test]
    fn a_winning_turn_records_no_dome() {
        let theirs =
            parse_fen("00000 00030 00200 00000 00000/2/gaea:A1,A2/mortal:C3,E1").unwrap();
        let (active, other) = theirs.get_active_non_active_gods();
        let win = active
            .get_winning_moves(&theirs, Player::Two)
            .into_iter()
            .next()
            .expect("C3 stands on level 2 beside D4");

        let after = theirs.next_state(active, other, win.action);
        assert_eq!(after.get_winner(), Some(Player::Two));
        assert_eq!(after.board.god_data[0], 0);
    }

    /// The pending squares are Workers on the board once she has answered them, so a folded
    /// position has to fold them too.
    #[test]
    fn the_pending_mask_survives_symmetry_folding() {
        let state =
            parse_fen("40000 00000 00000 00000 00000/1/gaea[1|A5]:C1,E1,B5/mortal:C3,B1").unwrap();

        for permutation in state.get_all_permutations::<true>() {
            let folded = FullGameState::new(permutation, state.gods);
            folded.validate();
            consistency_check(&folded).unwrap();
        }
    }

    /// Drives the searcher's filtered generators - mate only, win blockers - over a whole game.
    #[test]
    fn search_playout_answers_domes() {
        use crate::{
            board::GameStateBuilder,
            search::{SearchContext, get_win_reached_search_terminator, negamax_search},
            search_terminators::DynamicMaxDepthSearchTerminator,
            transposition_table::TranspositionTable,
        };

        for opponent in [
            GodName::Mortal,
            GodName::Pan,
            GodName::Hades,
            GodName::Aphrodite,
            GodName::Hypnus,
            GodName::Harpies,
            GodName::Persephone,
            GodName::Prometheus,
            GodName::Demeter,
        ] {
            let mut state = GameStateBuilder::new(GodName::Gaea, opponent)
                .with_p1_worker(B2)
                .with_p1_worker(D4)
                .with_p2_worker(B4)
                .with_p2_worker(D2)
                .build();

            let mut tt = TranspositionTable::new();

            for _ in 0..30 {
                if state.board.get_winner().is_some() {
                    break;
                }
                consistency_check(&state).unwrap();

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

            assert!(
                state.board.workers[0].count_ones() <= 4,
                "vs {opponent}: she can never hold more than four Workers"
            );
        }
    }
}

