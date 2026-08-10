use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, PERIMETER_SPACES_MASK, apply_mapping_to_mask},
    board::{BoardState, FullGameState, GodData},
    build_god_power_movers,
    gods::{
        FullAction, GodName, GodPower, HistoryIdxHelper, StaticGod, build_god_power_actions,
        generic::{
            ANY_MOVE_FILTER, GenericMove, GodMove, LOWER_POSITION_MASK, MOVE_IS_WINNING_MASK,
            MoveData, MoveGenFlags, NULL_MOVE_DATA, POSITION_WIDTH, ScoredMove,
        },
        god_power,
        harpies::slide_position,
        move_helpers::{
            GeneratorPreludeState, build_scored_move, get_basic_moves_from_raw_data,
            get_generator_prelude_state, get_sized_result, get_standard_reach_board,
            get_worker_end_move_state, get_worker_next_move_state, get_worker_start_move_state,
            is_interact_with_key_squares, is_mate_only, push_winning_moves,
        },
    },
    player::Player,
    square::Square,
};

use super::PartialAction;

const MOVE_FROM_POSITION_OFFSET: usize = 0;
const MOVE_TO_POSITION_OFFSET: usize = POSITION_WIDTH;
const BUILD_POSITION_OFFSET: usize = MOVE_TO_POSITION_OFFSET + POSITION_WIDTH;

const PLACE_POSITION_OFFSET: usize = BUILD_POSITION_OFFSET + POSITION_WIDTH;

const NO_PLACEMENT: MoveData = 25;

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct JasonMove(pub MoveData);

impl Into<GenericMove> for JasonMove {
    fn into(self) -> GenericMove {
        unsafe { std::mem::transmute(self) }
    }
}

impl From<GenericMove> for JasonMove {
    fn from(value: GenericMove) -> Self {
        unsafe { std::mem::transmute(value) }
    }
}

impl JasonMove {
    fn new_basic_move(
        move_from_position: Square,
        move_to_position: Square,
        build_position: Square,
    ) -> Self {
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | (NO_PLACEMENT << PLACE_POSITION_OFFSET);

        Self(data)
    }

    fn new_power_move(place_position: Square, build_position: Square) -> Self {
        let data: MoveData = 0
            | ((build_position as MoveData) << BUILD_POSITION_OFFSET)
            | ((place_position as MoveData) << PLACE_POSITION_OFFSET);

        Self(data)
    }

    fn new_winning_move(move_from_position: Square, move_to_position: Square) -> Self {
        // A placed worker starts on the ground, so it can never reach level 3 in one move
        let data: MoveData = ((move_from_position as MoveData) << MOVE_FROM_POSITION_OFFSET)
            | ((move_to_position as MoveData) << MOVE_TO_POSITION_OFFSET)
            | (NO_PLACEMENT << PLACE_POSITION_OFFSET)
            | MOVE_IS_WINNING_MASK;
        Self(data)
    }

    pub(crate) fn move_from_position(&self) -> Square {
        Square::from((self.0 as u8) & LOWER_POSITION_MASK)
    }

    pub(crate) fn move_to_position(&self) -> Square {
        Square::from((self.0 >> MOVE_TO_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    fn build_position(self) -> Square {
        Square::from((self.0 >> BUILD_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK)
    }

    pub fn maybe_place_position(self) -> Option<Square> {
        let value = (self.0 >> PLACE_POSITION_OFFSET) as u8 & LOWER_POSITION_MASK;
        if value == NO_PLACEMENT as u8 {
            None
        } else {
            Some(Square::from(value))
        }
    }

    fn move_mask(self) -> BitBoard {
        BitBoard::as_mask(self.move_from_position()) | BitBoard::as_mask(self.move_to_position())
    }

    fn get_is_winning(&self) -> bool {
        (self.0 & MOVE_IS_WINNING_MASK) != 0
    }
}

impl std::fmt::Debug for JasonMove {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 == NULL_MOVE_DATA {
            return write!(f, "NULL");
        }

        let is_win = self.get_is_winning();

        if is_win {
            let move_from = self.move_from_position();
            let move_to = self.move_to_position();
            write!(f, "{}>{}#", move_from, move_to)
        } else if let Some(place_pos) = self.maybe_place_position() {
            let move_to = self.move_to_position();
            let build = self.build_position();
            write!(f, "+{}>{}^{}", place_pos, move_to, build)
        } else {
            let move_from = self.move_from_position();
            let move_to = self.move_to_position();
            let build = self.build_position();
            write!(f, "{}>{}^{}", move_from, move_to, build)
        }
    }
}

impl GodMove for JasonMove {
    fn move_to_actions(
        self,
        _board: &BoardState,
        _player: Player,
        _other_god: StaticGod,
    ) -> Vec<FullAction> {
        if self.get_is_winning() {
            return vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position().into()),
            ]];
        }

        if let Some(place_pos) = self.maybe_place_position() {
            vec![vec![
                PartialAction::HeroActionPlacement(place_pos),
                PartialAction::Build(self.build_position()),
            ]]
        } else {
            vec![vec![
                PartialAction::SelectWorker(self.move_from_position()),
                PartialAction::MoveWorker(self.move_to_position().into()),
                PartialAction::Build(self.build_position()),
            ]]
        }
    }

    fn make_move(self, board: &mut BoardState, player: Player, _other_god: StaticGod) {
        if self.get_is_winning() {
            let worker_move_mask = self.move_mask();
            board.worker_xor(player, worker_move_mask);
            board.set_winner(player);
            return;
        }

        if let Some(place_pos) = self.maybe_place_position() {
            board.set_god_data(player, 1);
            board.worker_xor(player, place_pos.to_board());
            board.build_up(self.build_position());
        } else {
            let worker_move_mask = self.move_mask();
            board.worker_xor(player, worker_move_mask);
            board.build_up(self.build_position());
        }
    }

    fn get_blocker_board(self, _board: &BoardState) -> BitBoard {
        self.move_mask()
    }

    fn get_history_idx(self, board: &BoardState) -> usize {
        let mut helper = HistoryIdxHelper::new();
        if let Some(place_position) = self.maybe_place_position() {
            helper.add_square_with_height(board, place_position);
            helper.add_square_with_height(board, self.move_to_position());
        } else {
            helper.add_square_with_height(board, self.move_from_position());
            helper.add_square_with_height(board, self.move_to_position());
        }
        helper.add_square_with_height(board, self.build_position());
        helper.get()
    }
}

fn jason_move_gen<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    state: &FullGameState,
    player: Player,
    key_squares: BitBoard,
) -> Vec<ScoredMove> {
    if MUST_CLIMB {
        // `jason_vs_persephone` drives the must-climb generators itself
        unreachable!();
    }

    let prelude = get_generator_prelude_state::<F>(state, player, key_squares);
    let has_power_available = state.board.god_data[player as usize] == 0;

    if prelude.other_god.is_persephone {
        return jason_vs_persephone::<F>(&prelude, has_power_available);
    }

    let mut result = get_sized_result::<F>();
    if add_standard_moves::<F, false>(&prelude, &mut result) {
        return result;
    }

    if has_power_available && !is_mate_only::<F>() {
        if prelude.is_against_harpies {
            add_hero_power_moves_vs_harpies::<F>(&prelude, &mut result);
        } else {
            add_hero_power_moves_vs_non_harpies::<F, false>(&prelude, &mut result);
        }
    }

    result
}

// If a regular work can climb it must. You can power this turn, but that worker must also climb
// If a regular worker can't climb, then you can power this turn, but if there's a placement that
// allows that worker to climb, it must.
fn jason_vs_persephone<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    has_power_available: bool,
) -> Vec<ScoredMove> {
    let mut result = get_sized_result::<F>();

    if add_standard_moves::<F, true>(prelude, &mut result) {
        return result;
    }

    let must_climb = result.len() > 0
        || filtered_out_a_climb::<F, _>(|out| {
            add_standard_moves::<0, true>(prelude, out);
        });

    if must_climb {
        if has_power_available && !is_mate_only::<F>() {
            add_hero_power_moves_vs_non_harpies::<F, true>(prelude, &mut result);
        }
        return result;
    }

    if add_standard_moves::<F, false>(prelude, &mut result) {
        return result;
    }

    if has_power_available && !is_mate_only::<F>() {
        let flat_moves_only = result.len();
        add_hero_power_moves_vs_non_harpies::<F, true>(prelude, &mut result);

        let power_can_climb = result.len() > flat_moves_only
            || filtered_out_a_climb::<F, _>(|out| {
                add_hero_power_moves_vs_non_harpies::<0, true>(prelude, out);
            });

        if !power_can_climb {
            add_hero_power_moves_vs_non_harpies::<F, false>(prelude, &mut result);
        }
    }

    result
}

// An empty run under a mate/key-square filter doesn't mean there was no climb to find
// check if any non-filtered moves were available
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

fn add_standard_moves<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
) -> bool {
    let checkable_mask = prelude.exactly_level_2;
    let acting_workers = if is_mate_only::<F>() {
        prelude.acting_workers & checkable_mask
    } else {
        prelude.acting_workers
    };

    for worker_start_pos in acting_workers {
        let worker_start_state = get_worker_start_move_state(prelude, worker_start_pos);
        let mut worker_next_moves =
            get_worker_next_move_state::<MUST_CLIMB>(prelude, &worker_start_state, checkable_mask);

        if is_mate_only::<F>() || worker_start_state.worker_start_height == 2 {
            let moves_to_level_3 =
                worker_next_moves.worker_moves & prelude.exactly_level_3 & prelude.win_mask;
            if push_winning_moves::<F, JasonMove, _>(
                result,
                worker_start_pos,
                moves_to_level_3,
                JasonMove::new_winning_move,
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
                get_worker_end_move_state::<F>(prelude, &worker_start_state, worker_end_pos);

            let unblocked_squares = !(worker_start_state.all_non_moving_workers
                | worker_end_move_state.worker_end_mask
                | prelude.domes_and_frozen);
            let reach_board = get_standard_reach_board::<F>(
                prelude,
                &worker_next_moves,
                &worker_end_move_state,
                unblocked_squares,
            );

            let all_possible_builds = NEIGHBOR_MAP[worker_end_move_state.worker_end_pos as usize]
                & unblocked_squares
                & prelude.build_mask;

            let mut narrowed_builds = all_possible_builds;
            if is_interact_with_key_squares::<F>() {
                let is_already_matched = (worker_end_move_state.worker_end_mask
                    & prelude.key_squares)
                    .is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            for worker_build_pos in narrowed_builds {
                let build_mask = worker_build_pos.to_board();
                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };
                let new_action = JasonMove::new_basic_move(
                    worker_start_pos,
                    worker_end_move_state.worker_end_pos,
                    worker_build_pos,
                );

                result.push(build_scored_move::<F, _>(
                    new_action,
                    is_check,
                    worker_end_move_state.is_improving,
                ));
            }
        }
    }

    false
}

fn add_hero_power_moves_vs_non_harpies<const F: MoveGenFlags, const MUST_CLIMB: bool>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
) {
    let unblocked_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);
    let buildable_squares = unblocked_squares & prelude.build_mask;
    let valid_placements = PERIMETER_SPACES_MASK & prelude.exactly_level_0 & unblocked_squares;

    let threatening_workers = prelude.own_workers & prelude.exactly_level_2;
    let reach_board = if prelude.is_against_hypnus && threatening_workers.count_ones() < 2 {
        BitBoard::EMPTY
    } else {
        apply_mapping_to_mask(threatening_workers, prelude.standard_neighbor_map)
            & unblocked_squares
            & prelude.win_mask
    };

    let mut seen_move_to = BitBoard::EMPTY;

    for init_pos in valid_placements {
        let place_mask = init_pos.to_board();

        let worker_moves =
            get_basic_moves_from_raw_data::<MUST_CLIMB>(prelude, init_pos, place_mask, 0);

        for move_to in worker_moves {
            let move_to_mask = move_to.to_board();

            if (move_to_mask & seen_move_to).is_not_empty() {
                continue;
            }
            seen_move_to |= move_to_mask;

            let all_possible_builds = NEIGHBOR_MAP[move_to as usize] & buildable_squares;
            let mut narrowed_builds = all_possible_builds;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (move_to_mask & prelude.key_squares).is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            for build_pos in narrowed_builds {
                let build_mask = build_pos.to_board();

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                let new_action = JasonMove::new_power_move(move_to, build_pos);

                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    }
}

fn add_hero_power_moves_vs_harpies<const F: MoveGenFlags>(
    prelude: &GeneratorPreludeState,
    result: &mut Vec<ScoredMove>,
) {
    let unblocked_squares = !(prelude.all_workers_and_frozen_mask | prelude.domes_and_frozen);
    let valid_placements = PERIMETER_SPACES_MASK & prelude.exactly_level_0 & unblocked_squares;

    let threatening_workers = prelude.own_workers & prelude.exactly_level_2;
    let reach_board = apply_mapping_to_mask(threatening_workers, &NEIGHBOR_MAP) & unblocked_squares;

    let mut seen_move_to = BitBoard::EMPTY;

    for init_pos in valid_placements {
        let place_mask = init_pos.to_board();

        let worker_moves = get_basic_moves_from_raw_data::<false>(prelude, init_pos, place_mask, 0);

        for direction_target in worker_moves {
            let move_to = slide_position(prelude, init_pos, direction_target);
            let move_to_mask = move_to.to_board();

            if (move_to_mask & seen_move_to).is_not_empty() {
                continue;
            }
            seen_move_to |= move_to_mask;

            let all_possible_builds = NEIGHBOR_MAP[move_to as usize] & unblocked_squares;
            let mut narrowed_builds = all_possible_builds;

            if is_interact_with_key_squares::<F>() {
                let is_already_matched =
                    (move_to_mask & prelude.key_squares).is_not_empty() as usize;
                narrowed_builds &=
                    [prelude.key_squares, BitBoard::MAIN_SECTION_MASK][is_already_matched];
            }

            for build_pos in narrowed_builds {
                let build_mask = build_pos.to_board();

                let is_check = {
                    let final_level_3 = (prelude.exactly_level_2 & build_mask)
                        | (prelude.exactly_level_3 & !build_mask);
                    let check_board = reach_board & final_level_3;
                    check_board.is_not_empty()
                };

                let new_action = JasonMove::new_power_move(move_to, build_pos);

                result.push(build_scored_move::<F, _>(new_action, is_check, false));
            }
        }
    }
}

fn parse_god_data(data: &str) -> Result<GodData, String> {
    match data {
        "" => Ok(0),
        "x" | "X" => Ok(1),
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

pub const fn build_jason() -> GodPower {
    god_power(
        GodName::Jason,
        build_god_power_movers!(jason_move_gen),
        build_god_power_actions::<JasonMove>(),
        7892341056789234105,
        14567890123456789012,
    )
    .with_parse_god_data_fn(parse_god_data)
    .with_stringify_god_data_fn(stringify_god_data)
    .with_pretty_stringify_god_data_fn(pretty_stringify_god_data)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::{
        board::GameStateBuilder,
        consistency_checker::consistency_check,
        fen::parse_fen,
        gods::generic::GenericMove,
        move_verifier::MoveVerifier,
        square::Square::{self, *},
    };

    fn jason_move_climbs(state: &FullGameState, action: GenericMove) -> bool {
        let board = &state.board;
        let jason_move: JasonMove = action.into();

        if action.get_is_winning() {
            return true;
        }

        if let Some(end_pos) = jason_move.maybe_place_position() {
            // The placed worker always starts on the ground
            board.get_height(end_pos) > 0
        } else {
            board.get_height(jason_move.move_to_position())
                > board.get_height(jason_move.move_from_position())
        }
    }

    fn assert_all_moves_climb(state: &FullGameState) {
        let jason = GodName::Jason.to_power();
        let moves = jason.get_all_moves(state, state.board.current_player);
        assert!(!moves.is_empty());

        for scored_move in &moves {
            assert!(
                jason_move_climbs(state, scored_move.action),
                "Move didn't climb: {}",
                jason.stringify_move(scored_move.action)
            );
        }
    }

    // A `BoardState` minus the bookkeeping the brute force would have to maintain by hand
    type BoardSignature = (BitBoard, BitBoard, [BitBoard; 4], [GodData; 2], bool);

    fn board_signature(board: &BoardState) -> BoardSignature {
        (
            board.workers[0] & BitBoard::MAIN_SECTION_MASK,
            board.workers[1] & BitBoard::MAIN_SECTION_MASK,
            std::array::from_fn(|i| board.height_map[i] & BitBoard::MAIN_SECTION_MASK),
            board.god_data,
            board.get_winner().is_some(),
        )
    }

    // Every position Jason can legally reach, worked out from first principles
    fn bruteforce_next_boards(state: &FullGameState, player: Player) -> HashSet<BoardSignature> {
        let board = &state.board;
        let own_workers = board.workers[player as usize] & BitBoard::MAIN_SECTION_MASK;
        let oppo_workers = board.workers[!player as usize] & BitBoard::MAIN_SECTION_MASK;
        let all_workers = own_workers | oppo_workers;
        let domes = board.at_least_level_4();

        let mut ordinary_climbing = HashSet::new();
        let mut ordinary_flat = HashSet::new();
        let mut power_climbing = HashSet::new();
        let mut power_flat = HashSet::new();

        fn add_builds(
            bucket: &mut HashSet<BoardSignature>,
            moved: &BoardState,
            end: Square,
            blocked: BitBoard,
        ) {
            for build in NEIGHBOR_MAP[end as usize] & !blocked {
                let mut next = moved.clone();
                next.build_up(build);
                bucket.insert(board_signature(&next));
            }
        }

        // Ordinary move + build
        for start in own_workers {
            let start_height = board.get_height(start);

            for end in NEIGHBOR_MAP[start as usize] & !(all_workers | domes) {
                let end_height = board.get_height(end);
                if end_height > start_height + 1 {
                    continue;
                }

                let did_climb = end_height > start_height;

                let mut moved = board.clone();
                moved.worker_xor(player, start.to_board() | end.to_board());

                // Only climbing onto level 3 wins - a worker already up there can walk across
                if end_height == 3 && did_climb {
                    moved.set_winner(player);
                    ordinary_climbing.insert(board_signature(&moved));
                    continue;
                }

                let blocked = (all_workers & !start.to_board()) | end.to_board() | domes;
                let bucket = if did_climb {
                    &mut ordinary_climbing
                } else {
                    &mut ordinary_flat
                };
                add_builds(bucket, &moved, end, blocked);
            }
        }

        // Place the extra worker on the perimeter, then move + build with it
        if board.god_data[player as usize] == 0 {
            let placements = PERIMETER_SPACES_MASK & board.exactly_level_0() & !all_workers;

            for place in placements {
                for end in NEIGHBOR_MAP[place as usize] & !(all_workers | domes) {
                    let end_height = board.get_height(end);
                    if end_height > 1 {
                        continue;
                    }

                    let mut moved = board.clone();
                    moved.set_god_data(player, 1);
                    moved.worker_xor(player, end.to_board());

                    // The placement square is vacated by the move, so it stays buildable
                    let blocked = all_workers | end.to_board() | domes;
                    let bucket = if end_height > 0 {
                        &mut power_climbing
                    } else {
                        &mut power_flat
                    };
                    add_builds(bucket, &moved, end, blocked);
                }
            }
        }

        let mut legal: HashSet<BoardSignature> =
            ordinary_climbing.union(&power_climbing).copied().collect();

        // Persephone can't force Jason to spend his power just to find a climb
        if ordinary_climbing.is_empty() {
            legal.extend(&ordinary_flat);

            if power_climbing.is_empty() {
                legal.extend(&power_flat);
            }
        }

        legal
    }

    fn real_next_boards(state: &FullGameState, player: Player) -> HashSet<BoardSignature> {
        let (active_god, oppo_god) = state.get_active_non_active_gods();
        active_god
            .get_all_moves(state, player)
            .iter()
            .map(|m| board_signature(&state.next_state(active_god, oppo_god, m.action).board))
            .collect()
    }

    fn assert_moves_match_bruteforce(state: &FullGameState) {
        let player = state.board.current_player;
        let bruteforce = bruteforce_next_boards(state, player);
        let real = real_next_boards(state, player);

        let missing: Vec<_> = bruteforce.difference(&real).collect();
        let extra: Vec<_> = real.difference(&bruteforce).collect();

        assert!(
            missing.is_empty() && extra.is_empty(),
            "Move mismatch on {:?}:\n  Missing ({}): {:?}\n  Extra ({}): {:?}",
            state,
            missing.len(),
            missing,
            extra.len(),
            extra,
        );
    }

    #[test]
    fn test_jason_vs_persephone_power_only_climb_binds_the_power_turn() {
        // Neither worker can climb, but a placed worker could step up onto D4
        let state = GameStateBuilder::new(GodName::Jason, GodName::Persephone)
            .with_p1_worker(A1)
            .with_p1_worker(A2)
            .with_p2_worker(E1)
            .with_p2_worker(E2)
            .with_height(D4, 1)
            .build();

        consistency_check(&state).unwrap();
        assert_moves_match_bruteforce(&state);

        let next_states = state.get_next_states_interactive();

        // Declining the power entirely
        MoveVerifier::new()
            .with_p1_worker_at(B1)
            .without_p1_worker_at(A1)
            .any(&next_states);

        // Taking the climb the power makes available
        MoveVerifier::new().with_p1_worker_at(D4).any(&next_states);

        // Placing next to D4 but landing flat
        MoveVerifier::new()
            .with_p1_worker_at(A1)
            .with_p1_worker_at(A2)
            .with_p1_worker_at(D5)
            .none(&next_states);

        // Placing on the far side of the board, out of reach of any climb
        MoveVerifier::new()
            .with_p1_worker_at(A1)
            .with_p1_worker_at(A2)
            .with_p1_worker_at(C5)
            .none(&next_states);

        for next_state in state.get_next_states() {
            let workers = next_state.board.workers[0];
            let placed_a_worker = workers.count_ones() > 2;
            assert!(
                !placed_a_worker || (workers & D4.to_board()).is_not_empty(),
                "Placed a worker without climbing: {:?}",
                next_state
            );
        }
    }

    #[test]
    fn test_jason_vs_persephone_no_climb_anywhere_frees_the_power_turn() {
        // The same position with D4 flattened, so no climb exists anywhere
        let state = GameStateBuilder::new(GodName::Jason, GodName::Persephone)
            .with_p1_worker(A1)
            .with_p1_worker(A2)
            .with_p2_worker(E1)
            .with_p2_worker(E2)
            .build();

        consistency_check(&state).unwrap();
        assert_moves_match_bruteforce(&state);

        let next_states = state.get_next_states_interactive();

        MoveVerifier::new()
            .with_p1_worker_at(A1)
            .with_p1_worker_at(A2)
            .with_p1_worker_at(C5)
            .any(&next_states);
    }

    #[test]
    fn test_jason_vs_persephone_ordinary_climb_binds_the_power() {
        // A1 can step up to A2 without the power, so every move has to climb
        let state = GameStateBuilder::new(GodName::Jason, GodName::Persephone)
            .with_p1_worker(A1)
            .with_p1_worker(C1)
            .with_p2_worker(E5)
            .with_p2_worker(D5)
            .with_height(A2, 1)
            .with_height(D3, 1)
            .build();

        consistency_check(&state).unwrap();
        assert_all_moves_climb(&state);
        assert_moves_match_bruteforce(&state);

        let next_states = state.get_next_states_interactive();

        MoveVerifier::new().with_p1_worker_at(A2).any(&next_states);
        MoveVerifier::new().with_p1_worker_at(D3).any(&next_states);

        // D1 is a flat step for C1, B5 a flat landing square for the power
        MoveVerifier::new().with_p1_worker_at(D1).none(&next_states);
        MoveVerifier::new().with_p1_worker_at(B5).none(&next_states);
    }

    #[test]
    fn test_jason_vs_persephone_spent_power_plays_as_mortal() {
        // Once the power is gone Jason is a plain mortal, and A1 -> A2 is the only climb
        let state =
            parse_fen("0000000000000001000000000/1/jason[x]:A1,C1/persephone:E5,D5").unwrap();

        consistency_check(&state).unwrap();
        assert_all_moves_climb(&state);
        assert_moves_match_bruteforce(&state);

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new().with_p1_worker_at(A2).all(&next_states);

        for next_state in state.get_next_states() {
            assert_eq!(next_state.board.workers[0].count_ones(), 2);
        }
    }

    #[test]
    fn test_jason_vs_persephone_climb_is_also_a_win() {
        // The forced climb is a winning one, so it's the whole move list
        let state = GameStateBuilder::new(GodName::Jason, GodName::Persephone)
            .with_p1_worker(A1)
            .with_p1_worker(C1)
            .with_p2_worker(E5)
            .with_p2_worker(D5)
            .with_height(A1, 2)
            .with_height(A2, 3)
            .with_height(D3, 1)
            .build();

        consistency_check(&state).unwrap();
        assert_moves_match_bruteforce(&state);

        let next_states = state.get_next_states_interactive();
        MoveVerifier::new().is_winner(Player::One).any(&next_states);
    }

    #[test]
    fn test_jason_vs_persephone_bruteforce_handcrafted_positions() {
        for fen in [
            // Flat board - nothing can climb, so nothing is restricted
            "0000000000000000000000000/1/jason:A1,C1/persephone:E5,D5",
            // Power spent, varied terrain
            "0102000210003021000102100/1/jason[x]:B2,D4/persephone:C3,A5",
            // Only the perimeter is raised, so the power has climbs everywhere
            "1111110000100001000011111/1/jason:B2,C3/persephone:D3,C2",
            // Crowded, high terrain with domes about
            "2413122132212314321213212/1/jason:A1,C3/persephone:A5,E1",
            // Jason walled in at height 2 while a ground-level climb waits for the power
            "0000000030300003330000010/1/jason:C3,B3/persephone:E5,E4",
            // A3 can walk across level 3 to B3, which neither wins nor counts as a climb
            "0000000000330000000000000/1/jason:A3,C1/persephone:E5,E4",
        ] {
            let state = parse_fen(fen).unwrap();
            consistency_check(&state).unwrap();
            assert_moves_match_bruteforce(&state);
        }
    }

    // Drives the searcher's filtered generators (mate-only, win blockers) over a whole game
    #[test]
    fn test_jason_vs_persephone_search_playout() {
        use crate::{
            search::{SearchContext, get_win_reached_search_terminator, negamax_search},
            search_terminators::DynamicMaxDepthSearchTerminator,
            transposition_table::TranspositionTable,
        };

        let mut state = GameStateBuilder::new(GodName::Jason, GodName::Persephone)
            .with_p1_worker(B2)
            .with_p1_worker(D4)
            .with_p2_worker(B4)
            .with_p2_worker(D2)
            .build();

        let mut tt = TranspositionTable::new();
        let mut used_power = false;

        for _ in 0..20 {
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
            used_power |= state.board.current_player == Player::One
                && JasonMove::from(best_move.action)
                    .maybe_place_position()
                    .is_some();

            let (active_god, oppo_god) = state.get_active_non_active_gods();
            state = state.next_state(active_god, oppo_god, best_move.action);
        }

        // The power can only be spent by a move we recognise as a power move
        assert_eq!(state.board.god_data[0] != 0, used_power);
    }

    #[test]
    fn test_jason_vs_persephone_bruteforce_random_games() {
        use crate::{
            matchup::Matchup,
            random_utils::{RandomSingleGameStateGenerator, get_random_starting_state},
        };
        use rand::rng;

        let matchup = Matchup::new(GodName::Jason, GodName::Persephone);
        let mut states_checked = 0;

        for _ in 0..100 {
            let starting_state = get_random_starting_state(&matchup, &mut rng());

            for state in RandomSingleGameStateGenerator::new(starting_state) {
                if state.board.get_winner().is_some() {
                    break;
                }
                if state.board.current_player == Player::One {
                    assert_moves_match_bruteforce(&state);
                    states_checked += 1;
                }
            }
        }

        assert!(
            states_checked > 100,
            "Expected to check many states, only checked {}",
            states_checked,
        );
    }
}
