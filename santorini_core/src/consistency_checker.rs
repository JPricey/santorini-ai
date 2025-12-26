use std::collections::{HashMap, HashSet};

use crate::{
    bitboard::{
        BETWEEN_MAPPING, BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP, apply_mapping_to_mask,
    },
    board::{BoardState, FullGameState},
    fen::{game_state_to_fen, parse_fen},
    gods::{
        GodName, StaticGod,
        ares::AresMove,
        athena::AthenaMove,
        castor::CastorMove,
        generic::{CHECK_SENTINEL_SCORE, GenericMove, MOVE_DATA_MAIN_SECTION, ScoredMove},
        harpies::slide_position_with_custom_blockers,
        hydra::HydraMove,
        mortal::MortalMove,
    },
    hashing::compute_hash_from_scratch,
    player::Player,
};

pub fn consistency_check(state: &FullGameState) -> Result<(), Vec<String>> {
    let mut checker = ConsistencyChecker::new(state);
    checker.perform_all_validations()
}

/// Performs validations on god move generators,
/// such as ensuring that win/check tags are correct
/// And that opponent turn powers are correctly respected
pub(crate) struct ConsistencyChecker {
    state: FullGameState,
    errors: Vec<String>,
}

impl ConsistencyChecker {
    pub fn new(state: &FullGameState) -> Self {
        Self {
            state: state.clone(),
            errors: Default::default(),
        }
    }

    pub fn perform_all_validations(&mut self) -> Result<(), Vec<String>> {
        if let Err(err) = self.state.validation_err() {
            self.errors
                .push(format!("Root state has validation errors: {}", err));
        } else {
            let current_player = self.state.board.current_player;
            let (active_god, oppo_god) = self.state.get_active_non_active_gods();

            let other_wins = oppo_god.get_winning_moves(&self.state, !current_player);

            let search_moves = active_god.get_moves_for_search(&self.state, current_player);
            self.check_wins_on_end_only("SearchMoves", &search_moves);

            let own_winning_moves = active_god.get_winning_moves(&self.state, current_player);
            let all_moves = active_god.get_all_moves(&self.state, current_player);

            self.validate_fen();
            self.validate_hash();
            self.self_check_validations(&search_moves);
            self.opponent_check_blockers(&other_wins, &search_moves);
            self.validate_non_duplicates(&search_moves);
            self.validate_wins(&own_winning_moves);
            self.validate_build_blockers(&search_moves);

            self.validate_search_moves_subset_all_moves(
                &all_moves,
                &search_moves,
                &own_winning_moves,
            );

            self.validate_hypnus_moves(&search_moves);
            self.validate_aphrodite_moves(&search_moves);
            self.validate_persephone_moves(&search_moves);
            self.validate_hades_moves(&search_moves);
            self.validate_frozen_moves(&search_moves);
        }

        if self.errors.len() == 0 {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn validate_fen(&mut self) {
        let fen = game_state_to_fen(&self.state);
        match parse_fen(&fen) {
            Ok(parsed_state) => {
                if parsed_state != self.state {
                    self.errors.push(format!(
                        "FEN round trip mismatch. fen: {} Parsed: {:?} Original: {:?}",
                        fen, parsed_state, self.state
                    ));
                }
            }
            Err(err) => {
                self.errors
                    .push(format!("FEN parse error: {}. fen: {}", err, fen));
            }
        }
    }

    fn validate_hash(&mut self) {
        let computed_hash = compute_hash_from_scratch(&self.state);
        if self.state.board.hash != computed_hash {
            self.errors.push(format!(
                "Hash mismatch. Expected: {} Computed: {}",
                self.state.board.hash, computed_hash
            ));
        }
    }

    fn validate_search_moves_subset_all_moves(
        &mut self,
        all_moves: &Vec<ScoredMove>,
        search_moves: &Vec<ScoredMove>,
        own_winning_moves: &Vec<ScoredMove>,
    ) {
        let mut all_move_map = HashMap::<u32, bool>::new();

        for action in all_moves {
            let key = action.action.0 & MOVE_DATA_MAIN_SECTION;
            all_move_map.insert(key, action.get_is_winning());
        }

        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        for action in search_moves {
            let key = action.action.0 & MOVE_DATA_MAIN_SECTION;
            if !all_move_map.contains_key(&key) {
                self.errors.push(format!(
                    "Search move not in all moves: {} -> {:?}",
                    active_god.stringify_move(action.action),
                    self.state.next_state(active_god, oppo_god, action.action)
                ));
            } else {
                let was_winning = all_move_map[&key];
                if active_god.god_name == GodName::Proteus {
                    continue;
                }
                if was_winning != action.get_is_winning() {
                    self.errors.push(format!(
                        "Search move win flag mismatch from all moves: {}. AllMovesWin: {} SearchMovesWin: {}",
                        active_god.stringify_move(action.action),
                        was_winning,
                        action.get_is_winning()
                    ));
                }
            }
        }

        for action in own_winning_moves {
            let key = action.action.0 & MOVE_DATA_MAIN_SECTION;
            if !all_move_map.contains_key(&key) {
                self.errors.push(format!(
                    "Winning move not in all moves: {} -> {:?}",
                    active_god.stringify_move(action.action),
                    self.state.next_state(active_god, oppo_god, action.action)
                ));
            } else {
                if active_god.god_name == GodName::Proteus {
                    continue;
                }
                let was_winning = all_move_map[&key];
                if !was_winning {
                    self.errors.push(format!(
                        "Winning move not marked as winning in all moves: {}",
                        active_god.stringify_move(action.action),
                    ));
                }
            }
        }

        if own_winning_moves.is_empty() {
            let mut search_states_map = HashSet::<BoardState>::new();
            let mut all_states_map = HashSet::<BoardState>::new();
            if own_winning_moves.is_empty() {
                for action in search_moves {
                    let new_state = self.state.next_state(active_god, oppo_god, action.action);
                    search_states_map.insert(new_state.board);
                }

                for action in all_moves {
                    let new_state = self.state.next_state(active_god, oppo_god, action.action);
                    all_states_map.insert(new_state.board);
                }

                let all_move_only_states = all_states_map
                    .difference(&search_states_map)
                    .collect::<Vec<&BoardState>>();

                for all_move_only_state in all_move_only_states {
                    let full_game_state =
                        FullGameState::new(all_move_only_state.clone(), self.state.gods.clone());

                    let mut relevant_actions = Vec::new();
                    for action in all_moves {
                        let new_state = self.state.next_state(active_god, oppo_god, action.action);
                        if &new_state.board == all_move_only_state {
                            relevant_actions.push(active_god.stringify_move(action.action));
                        }
                    }

                    self.errors.push(format!(
                        "All moves has state not in search moves: {:?} ({:?})",
                        full_game_state, relevant_actions
                    ));
                }

                let search_only_states = search_states_map
                    .difference(&all_states_map)
                    .collect::<Vec<&BoardState>>();
                for search_only_state in search_only_states {
                    let full_game_state =
                        FullGameState::new(search_only_state.clone(), self.state.gods.clone());

                    self.errors.push(format!(
                        "Search moves has state not in all moves: {:?}",
                        full_game_state
                    ));
                }
            }
        }
    }

    fn validate_non_duplicates(&mut self, actions: &Vec<ScoredMove>) {
        let mut seen = HashMap::<BoardState, GenericMove>::new();
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if active_god.god_name == GodName::Proteus && oppo_god.god_name == GodName::Harpies {
            // Workers can end up in their own spot. This results in duplicate moves
            return;
        }
        if active_god.god_name == GodName::Hydra {
            return;
        }

        for action in actions {
            let action = action.action;
            let new_state = self.state.next_state(active_god, oppo_god, action);

            if let Some(other_action) = seen.get(&new_state.board) {
                if active_god.god_name == GodName::Castor {
                    let action: CastorMove = action.into();
                    let other_action: CastorMove = action.into();

                    let action_is_double_move = action.maybe_move_from_position_2().is_some()
                        && action.maybe_move_from_position_2().is_some();
                    let other_is_double_move = other_action.maybe_move_from_position_2().is_some()
                        && other_action.maybe_move_from_position_2().is_some();

                    if action_is_double_move && other_is_double_move {
                        continue;
                    }
                }

                self.errors.push(format!(
                    "Duplicate move found: {} / {} -> {:?}",
                    active_god.stringify_move(action),
                    active_god.stringify_move(*other_action),
                    new_state,
                ));
                return;
            }

            seen.insert(new_state.board, action);
        }
    }

    fn validate_frozen_moves(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        let oppo_frozens = oppo_god.get_frozen_mask(&self.state.board, !current_player);
        let oppo_workers = self.state.board.workers[!current_player as usize];

        if oppo_frozens.is_empty() {
            return;
        }
        let other_frozen_workers = oppo_workers & oppo_frozens;

        for action in actions {
            let action = action.action;

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_workers = new_state.board.workers[current_player as usize];

            let new_oppo_workers = new_state.board.workers[!current_player as usize];
            let new_oppo_frozen_workers = new_oppo_workers & oppo_frozens;

            if new_oppo_frozen_workers != other_frozen_workers {
                self.errors.push(format!(
                    "oppo frozen workers changed: {} -> {:?}\n Frozen: {}",
                    active_god.stringify_move(action),
                    new_state,
                    oppo_frozens
                ));
                return;
            }

            if (new_workers & oppo_frozens).is_not_empty() {
                self.errors.push(format!(
                    "Moved a worker into a frozen space: {} -> {:?}\n Frozen: {}",
                    active_god.stringify_move(action),
                    new_state,
                    oppo_frozens
                ));
                return;
            }

            for frozen_sq in oppo_frozens {
                let old_height = self.state.board.get_height(frozen_sq);
                let new_height = new_state.board.get_height(frozen_sq);
                if new_height != old_height {
                    self.errors.push(format!(
                        "Changed height of a frozen space: {} -> {:?}\n Frozen: {}",
                        active_god.stringify_move(action),
                        new_state,
                        oppo_frozens
                    ));
                    return;
                }
            }
        }
    }

    fn validate_hades_moves(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if oppo_god.god_name != GodName::Hades {
            return;
        }

        if active_god.god_name == GodName::Proteus
            || active_god.god_name == GodName::Hydra
            || active_god.god_name == GodName::Nemesis
        {
            return;
        }

        let old_workers = self.state.board.workers[current_player as usize];

        for action in actions {
            let action = action.action;

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_workers = new_state.board.workers[current_player as usize];

            let old_only = old_workers & !new_workers;
            let new_only = new_workers & !old_workers;

            let mut old_heights = Vec::new();
            let mut new_heights = Vec::new();
            for old_pos in old_only {
                old_heights.push(self.state.board.get_height(old_pos));
            }
            for new_pos in new_only {
                new_heights.push(new_state.board.get_height(new_pos));
            }

            if old_heights.len() != new_heights.len() {
                self.errors.push(format!(
                    "different number of workers in hades change that we don't know how to handle {} -> {:?}",
                    active_god.stringify_move(action),
                    new_state,
                ));
                continue;
            }
            old_heights.sort();
            new_heights.sort();

            for (old_h, new_h) in old_heights.iter().zip(new_heights) {
                if new_h < *old_h {
                    self.errors.push(format!(
                        "Decreased height against hades: {} -> {:?}",
                        active_god.stringify_move(action),
                        new_state,
                    ));
                    return;
                }
            }
        }
    }

    fn validate_persephone_moves(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if oppo_god.god_name != GodName::Persephone {
            return;
        }

        let active_god_name = active_god.god_name;
        // Ignore zeus, who can appear to move up by building under himself
        // TODO: scope this down
        if active_god_name == GodName::Zeus {
            return;
        }

        if active_god_name == GodName::Proteus
            || active_god_name == GodName::Hydra
            || active_god_name == GodName::Nemesis
        {
            return;
        }

        let old_workers = self.state.board.workers[current_player as usize];

        let mut increase_move = None;
        let mut non_increase_move = None;

        for action in actions {
            let action = action.action;

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_workers = new_state.board.workers[current_player as usize];

            let old_only = old_workers & !new_workers;
            let new_only = new_workers & !old_workers;

            let mut did_any_increase = false;

            // Check if artemis could have made an improvement at any point on their turn
            if active_god.god_name == GodName::Artemis {
                assert_eq!(old_only.count_ones(), 1);
                assert_eq!(new_only.count_ones(), 1);

                let old_sq = old_only.lsb();
                let new_sq = new_only.lsb();

                let old_height = self.state.board.get_height(old_sq);
                let new_height = self.state.board.get_height(new_sq);

                if new_height > old_height {
                    did_any_increase = true;
                }

                let mut shared_neighbors =
                    NEIGHBOR_MAP[old_sq as usize] & NEIGHBOR_MAP[new_sq as usize];
                shared_neighbors &= !(self.state.board.workers[0]
                    | self.state.board.workers[1]
                    | self.state.board.height_map[3]);

                for sq in shared_neighbors {
                    let sq_height = self.state.board.get_height(sq);
                    if sq_height == old_height + 1
                        || sq_height <= old_height && new_height == sq_height + 1
                    {
                        did_any_increase = true;
                    }
                }
            } else if active_god.god_name == GodName::Castor {
                // TODO: detect this properly
                return;
            } else {
                let mut old_heights = Vec::new();
                let mut new_heights = Vec::new();
                for old_pos in old_only {
                    old_heights.push(self.state.board.get_height(old_pos));
                }
                for new_pos in new_only {
                    new_heights.push(new_state.board.get_height(new_pos));
                }

                if old_heights.len() != new_heights.len() {
                    self.errors.push(format!(
                    "different number of workers in persephone change that we don't know how to handle {} -> {:?}",
                    active_god.stringify_move(action),
                    new_state,
                ));
                    continue;
                }
                old_heights.sort();
                new_heights.sort();

                for (old_h, new_h) in old_heights.iter().zip(new_heights) {
                    if new_h > *old_h {
                        did_any_increase = true;
                        break;
                    }
                }
            }

            if did_any_increase {
                increase_move = Some(action);
                if non_increase_move.is_some() {
                    break;
                }
            } else {
                non_increase_move = Some(action);
                if increase_move.is_some() {
                    break;
                }
            }
        }

        if let Some(inc) = increase_move
            && let Some(non_inc) = non_increase_move
        {
            let inc_str = active_god.stringify_move(inc);
            let non_inc_str = active_god.stringify_move(non_inc);

            self.errors.push(format!(
                "Vs Persephone, has some moves to increase height({}) and some non({}): {:?}",
                inc_str, non_inc_str, self.state
            ));
        }
    }

    fn validate_aphrodite_moves(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if oppo_god.god_name != GodName::Aphrodite {
            return;
        }

        let active_god_name = active_god.god_name;
        if active_god_name == GodName::Proteus
            || active_god_name == GodName::Hydra
            || active_god_name == GodName::Medusa
        {
            return;
        }

        let old_workers = self.state.board.workers[current_player as usize];
        let old_aphro_workers = self.state.board.workers[!current_player as usize];
        let old_affinity_area = apply_mapping_to_mask(old_aphro_workers, &INCLUSIVE_NEIGHBOR_MAP);

        if (old_workers & old_affinity_area).is_empty() {
            return;
        }

        for action in actions {
            let action = action.action;

            if active_god.god_name == GodName::Scylla && action.get_is_winning() {
                // Scylla can always pull aphrodite into range, and we didn't bother to implement the drag on winning moves
                // So just ignore this check
                continue;
            }

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_workers = new_state.board.workers[current_player as usize];

            let old_only = old_workers & !new_workers;
            if (old_only & old_affinity_area).is_empty() {
                continue;
            }

            let new_only = new_workers & !old_workers;
            let new_aphro_workers = new_state.board.workers[!current_player as usize];

            // If Bia enters the affinity area and kills a worker, she's allowed to end her turn outside
            // the new affinity area, so keep checking against the old one.
            let new_affinity_area = if active_god.god_name == GodName::Bia {
                old_affinity_area
            } else {
                apply_mapping_to_mask(new_aphro_workers, &INCLUSIVE_NEIGHBOR_MAP)
            };

            if old_only.count_ones() != new_only.count_ones() {
                self.errors.push(format!(
                    "Unexpected worker change? {} -> {:?}",
                    active_god.stringify_move(action),
                    new_state,
                ));
            }

            if (old_only & old_affinity_area).count_ones()
                > (new_only & new_affinity_area).count_ones()
            {
                self.errors.push(format!(
                    "Moved a worker out of aphrodite affinity area: {} -> {:?}",
                    active_god.stringify_move(action),
                    new_state,
                ));
            }
        }
    }

    fn validate_hypnus_moves(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if oppo_god.god_name != GodName::Hypnus {
            return;
        }

        if active_god.god_name == GodName::Proteus
            || active_god.god_name == GodName::Hydra
            || active_god.god_name == GodName::Nemesis
        {
            return;
        }

        for action in actions {
            let action = action.action;

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_workers = new_state.board.workers[current_player as usize];
            let old_workers = self.state.board.workers[current_player as usize];

            let moved_workers = old_workers & !new_workers;
            for moved_worker in moved_workers {
                let old_worker_height = self.state.board.get_height(moved_worker);
                if old_worker_height == 0 {
                    continue;
                }

                let height_at_worker = self.state.board.height_map[old_worker_height - 1];
                if (old_workers & height_at_worker).count_ones() == 1 {
                    self.errors.push(format!(
                        "Moved a highest worker against hypnus: {} -> {:?}",
                        active_god.stringify_move(action),
                        new_state,
                    ));
                    return;
                }
            }
        }
    }

    fn validate_build_blockers(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if oppo_god.god_name != GodName::Limus {
            return;
        }
        if active_god.god_name == GodName::Medusa {
            // Medusa can "build" around limus, by converting limus workers in to stone
            return;
        }
        if active_god.god_name == GodName::Polyphemus {
            return;
        }

        let mut dome_build_actions = Vec::new();

        // Returns a mask of all builds done, except for domes
        fn get_new_builds_mask(new_state: &BoardState, old_state: &BoardState) -> BitBoard {
            let mut new_builds = BitBoard::EMPTY;
            new_builds |= new_state.height_map[2] & !old_state.height_map[2];
            new_builds |= new_state.height_map[1] & !old_state.height_map[1];
            new_builds |= new_state.height_map[0] & !old_state.height_map[0];

            new_builds & !new_state.height_map[3]
        }

        for action in actions {
            let action = action.action;
            if action.get_is_winning() {
                continue;
            }

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_builds = get_new_builds_mask(&new_state.board, &self.state.board);
            let new_dome_builds = new_state.board.height_map[3] & !self.state.board.height_map[3];

            let state_for_build_mask = if active_god.god_name == GodName::Nemesis {
                &self.state
            } else {
                &new_state
            };

            let build_mask = oppo_god
                .get_build_mask(state_for_build_mask.board.workers[!current_player as usize]);

            if new_dome_builds.is_not_empty() {
                dome_build_actions.push(action);
            }

            if (new_builds & !build_mask).is_not_empty() {
                let error_string = format!(
                    "Built in a build masked area: {} -> {:?}. Build mask:\n{}. Builds:\n:{}",
                    active_god.stringify_move(action),
                    new_state,
                    build_mask,
                    new_builds
                );
                self.errors.push(error_string);
                return;
            }
        }

        if active_god.god_name == GodName::Morpheus || active_god.god_name == GodName::Poseidon {
            // The check below only works for domers.
            // Morpheus "domes" over multiple builds, which limus does not allow
            return;
        }
        let mut against_mortal_state = self.state.clone();
        against_mortal_state.gods[!current_player as usize] = GodName::Mortal.to_power();
        let mortal_search_moves =
            active_god.get_moves_for_search(&against_mortal_state, current_player);

        for mortal_move in mortal_search_moves {
            let mortal_action = mortal_move.action;
            let new_state = self.state.next_state(active_god, oppo_god, mortal_action);
            // We could have built a dome and ALSO somewhere else.
            // these moves are invalid vs limus too, so skip this check.
            let new_builds = get_new_builds_mask(&new_state.board, &self.state.board);
            if new_builds.is_not_empty() {
                continue;
            }

            let new_dome_builds_from_lvl_3 = new_state.board.height_map[3]
                & self.state.board.height_map[2]
                & !self.state.board.height_map[3];

            if new_dome_builds_from_lvl_3.is_not_empty() {
                let seen_dome_build = dome_build_actions.contains(&mortal_action);

                if !seen_dome_build {
                    let error_string = format!(
                        "Was able to build valid dome against mortal, but not limus: {} -> {:?}",
                        active_god.stringify_move(mortal_action),
                        new_state,
                    );
                    self.errors.push(error_string);
                    return;
                }
            }
        }
    }

    fn check_wins_on_end_only(&mut self, label: &str, actions: &Vec<ScoredMove>) -> bool {
        for (i, action) in actions.iter().enumerate() {
            if action.get_is_winning() {
                if i < actions.len() - 1 {
                    self.errors.push(format!(
                        "{label}: Winning action was not at end of actions list: {} / {}",
                        i,
                        actions.len()
                    ));
                    return false;
                }
            }
        }
        return true;
    }

    fn opponent_check_blockers(
        &mut self,
        other_wins: &Vec<ScoredMove>,
        search_moves: &Vec<ScoredMove>,
    ) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        if other_wins.is_empty() {
            return;
        }

        let mut key_moves = BitBoard::EMPTY;
        for other_win_action in other_wins {
            key_moves |= oppo_god.get_blocker_board(&self.state.board, other_win_action.action);
        }

        if key_moves.is_empty() {
            self.errors
                .push("Opponent had wins, with no blocker board".to_owned());
            return;
        }

        let scored_blocker_actions =
            active_god.get_scored_blocker_moves(&self.state, current_player, key_moves);
        let unscored_blocker_actions =
            active_god.get_unscored_blocker_moves(&self.state, current_player, key_moves);
        self.check_wins_on_end_only("ScoredBlockerActions", &scored_blocker_actions);
        self._test_blocker_moves_are_consistent(&scored_blocker_actions, &unscored_blocker_actions);

        // Test that blockers actually block
        for block_action in scored_blocker_actions.iter() {
            let block_action = block_action.action;
            let stringed_action = active_god.stringify_move(block_action);
            let blocked_state = self.state.next_state(active_god, oppo_god, block_action);

            if blocked_state.board.get_winner() == Some(current_player) {
                continue;
            }

            let post_block_oppo_wins = oppo_god.get_winning_moves(&blocked_state, !current_player);
            let mut did_block_any = false;
            for win_action in other_wins {
                if !post_block_oppo_wins.contains(win_action) {
                    did_block_any = true;
                    break;
                }
            }

            if did_block_any {
                continue;
            }

            if active_god.god_name == GodName::Hydra {
                // Hydra can get onto lvl 3, then move into the lvl 3 win spot, then kill itself
                // TODO: fix that in move gen
                let block_action: HydraMove = block_action.into();
                if block_action.move_to_position() == block_action.special_worker_position() {
                    continue;
                }
            }

            if oppo_god.god_name == GodName::Charon {
                if active_god.god_name == GodName::Hermes {
                    // Hermes thinks that while standing still in Charon's winning square he's blocking a win
                    continue;
                }

                // If a character builds in the flip zone, it doesn't do anything.
                // TODO: add check to confirm this is what's happening
                continue;
            }

            if oppo_god.god_name == GodName::Pan {
                let any_pan_move: MortalMove = other_wins[0].action.into();

                if active_god.god_name == GodName::Persephone {
                    continue;
                }

                if active_god.god_name == GodName::Morpheus
                    || active_god.god_name == GodName::Poseidon
                {
                    // morpheus can triple build into 0's, allowing pan to make the same move to
                    // win
                    continue;
                }

                if [GodName::Athena, GodName::Nike].contains(&active_god.god_name) {
                    let athena_move: AthenaMove = block_action.into();
                    let did_pan_fall = self
                        .state
                        .board
                        .get_height(any_pan_move.move_from_position())
                        >= self.state.board.get_height(any_pan_move.move_to_position()) + 2;

                    if athena_move.get_is_stopping_climbing() && did_pan_fall {
                        continue;
                    }
                }

                let mut is_pan_big_fall = false;
                for pan_move in other_wins {
                    let pan_move: MortalMove = pan_move.action.into();
                    if self.state.board.get_height(pan_move.move_from_position()) == 3 {
                        // Pan threatens to fall from 3->0. Even building on that destination doesn't
                        // stop it
                        is_pan_big_fall = true;
                        break;
                    }
                }
                if is_pan_big_fall {
                    continue;
                }
            }

            if active_god.god_name == GodName::Aphrodite {
                // Aphrodite can try to block by moving next to an opponent worker, as long as it's
                // not also adjacent to some other non-worker key move.
                // But this can false positive if another worker can pull out a win anyway
                if (key_moves & self.state.board.workers[!current_player as usize]).count_ones() > 1
                {
                    continue;
                }

                // Aphrodite can't block scylla with affinity restrictions, so ignore this check
                // TODO: consider if we should add this check to Aphrodite's check response instead
                if oppo_god.god_name == GodName::Scylla {
                    continue;
                }
            }

            if active_god.god_name == GodName::Ares {
                let ares_move: AresMove = block_action.into();
                if let Some(remove_build) = ares_move.remove_build_position() {
                    if remove_build == ares_move.build_position()
                        && (key_moves & BitBoard::as_mask(remove_build)).is_not_empty()
                    {
                        // Ares can undo his own build, not worth checking for
                        continue;
                    }
                }
            }

            if oppo_god.god_name == GodName::Iris {
                // Jumps
                if key_moves == BitBoard::MAIN_SECTION_MASK {
                    continue;
                }
            }

            if oppo_god.god_name == GodName::Artemis {
                // Artemis can have multiple paths to level 3, but only the start and end are
                // reflected in the winning move.
                // Check that we at least made the key moves map smaller
                // TODO: try this again sometime
                // let mut blocked_key_moves = BitBoard::EMPTY;
                // for other_win_action in post_block_oppo_wins {
                //     blocked_key_moves |=
                //         oppo_god.get_blocker_board(&blocked_state.board, other_win_action.action);
                // }
                // if key_moves & blocked_key_moves == key_moves {
                //     let mut err_str =
                //         format!("Block move didn't remove any wins: {}: ", stringed_action);
                //     for winning_action in other_wins {
                //         err_str.push_str(&format!(
                //             "{} ",
                //             oppo_god.stringify_move(winning_action.action)
                //         ));
                //     }
                //     err_str.push_str(&format!("\nkey moves: {}", key_moves));
                //     err_str.push_str(&format!("\nblocked key moves: {}", key_moves));

                //     self.errors.push(err_str);
                //     blocked_state.print_to_console();
                //     return;
                // }
                continue;
            }

            if oppo_god.god_name == GodName::Minotaur {
                // TODO: scope this down
                // Minotaur puts spots that it pushes TO during a mate into the blocker board
                // but this only works on dome builds / moves - not lower builds.
                continue;
            }

            if oppo_god.god_name == GodName::Maenads {
                // TODO: scope this down.
                // Maenads dancing wins have a huge blocker board, since so far we haven't included
                // FROM positions as part of key square maps
                continue;
            }

            if oppo_god.god_name == GodName::Eros {
                // TODO: scope this down.
                continue;
            }

            if _test_castor_bad_key_move_blockers(&self.state) {
                continue;
            }

            let mut err_str = format!(
                "Block move didn't remove any wins (Move: {}) - Other wins:  ",
                stringed_action
            );
            for winning_action in other_wins {
                err_str.push_str(&format!(
                    "{}, ",
                    oppo_god.stringify_move(winning_action.action)
                ));
            }
            err_str.push_str(&format!("\n Key moves: {}", key_moves));

            self.errors.push(err_str);
            return;
        }

        let mut did_output_key_moves = false;

        // Test that we didn't miss any blockers
        for action in search_moves {
            let action = action.action;
            if scored_blocker_actions.iter().any(|a| a.action == action) {
                continue;
            }
            let stringed_action = active_god.stringify_move(action);

            let new_state = self.state.next_state(active_god, oppo_god, action);
            let new_oppo_wins = oppo_god.get_winning_moves(&new_state, !current_player);
            if new_oppo_wins.len() < other_wins.len() {
                if active_god.god_name == GodName::Persephone && oppo_god.god_name == GodName::Pan {
                    // TODO: comment this? Forget why it's here.
                    if new_oppo_wins.len() > 0 {
                        continue;
                    }
                }

                let mut error_str = format!("Missed blocking action: {}\n", stringed_action);

                if !did_output_key_moves {
                    error_str += &format!("Key moves board:{}\n", key_moves);
                    did_output_key_moves = true;
                }

                error_str += "| Old Wins: ";
                for old in other_wins {
                    error_str += &format!("{}, ", oppo_god.stringify_move(old.action));
                }

                error_str += "| New Wins: ";
                for new in &new_oppo_wins {
                    error_str += &format!("{}, ", oppo_god.stringify_move(new.action));
                }
                self.errors.push(error_str);
            }
        }
    }

    fn self_check_validations(&mut self, search_moves: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();

        for (i, action) in search_moves.iter().enumerate() {
            if action.get_is_winning() {
                continue;
            }

            let stringed_action = active_god.stringify_move(action.action);
            let is_check_flag = action.action.get_is_check();
            let is_check_score = action.score == CHECK_SENTINEL_SCORE;

            if is_check_score != is_check_flag {
                self.errors.push(format!(
                    "Check flag/score mismatch on action {i}/{}: {}. Flag: {} Score: {}",
                    search_moves.len(),
                    stringed_action,
                    is_check_flag,
                    action.score
                ));
                continue;
            }

            let mut check_state = self.state.next_state(active_god, oppo_god, action.action);
            oppo_god.make_passing_move(&mut check_state.board);
            let wins_from_check_state = active_god.get_winning_moves(&check_state, current_player);
            let is_real_checker = wins_from_check_state.len() > 0;

            if is_check_flag != is_real_checker {
                if is_real_checker && active_god.god_name == GodName::Maenads {
                    // maenads dancing kills...
                    // TODO: include these
                    continue;
                }

                if is_real_checker
                    && active_god.god_name == GodName::Artemis
                    && oppo_god.god_name == GodName::Harpies
                {
                    let wins_from_mortal_check_state = GodName::Mortal
                        .to_power()
                        .get_winning_moves(&check_state, current_player);
                    if wins_from_mortal_check_state.len() > 0 {
                        self.errors.push(format!(
                            "Check detection failure. Artemis v Harpies missed a win that a mortal could make. Check move: {}. Mortal win: {}",
                            stringed_action,
                            GodName::Mortal.to_power().stringify_move(wins_from_mortal_check_state[0].action),
                        ));
                    }
                } else if is_check_flag && oppo_god.god_name == GodName::Aphrodite {
                    // We don't consider aphrodite areas when doing check detection
                    // Replace aphrodite with a mortal and detect for checks again. If it is,
                    // ignore it
                    let mut checks_vs_mortal_state = check_state.clone();
                    checks_vs_mortal_state.gods[!current_player as usize] =
                        GodName::Mortal.to_power();

                    let wins_against_mortal =
                        active_god.get_winning_moves(&checks_vs_mortal_state, current_player);
                    if wins_against_mortal.len() == 0 {
                        let type_msg = match is_real_checker {
                            true => "Missed real check.",
                            false => "False positive.",
                        };

                        self.errors.push(format!(
                            "Check detection failure. {type_msg} {i}/{}: {}. Flag: {} RealChecker: {}",
                            search_moves.len(),
                            stringed_action,
                            is_check_flag,
                            is_real_checker
                        ));
                    }
                } else if is_check_flag
                    && (active_god.god_name == GodName::Pan || active_god.god_name == GodName::Eros)
                    && oppo_god.god_name == GodName::Persephone
                {
                    // Persephone can force pan to go up, preventing his downfall win con
                    // Doesn't seem worth trying to account for
                    continue;
                } else {
                    let type_msg = match is_real_checker {
                        true => "Missed real check.",
                        false => "False positive.",
                    };

                    self.errors.push(format!(
                        "Check detection failure. {type_msg} {i}/{}: {}. Flag: {} RealChecker: {}",
                        search_moves.len(),
                        stringed_action,
                        is_check_flag,
                        is_real_checker
                    ));
                    continue;
                }
            }

            for winning_action in wins_from_check_state {
                self._validate_win(
                    &format!("FromCheckState > {} ({:?})", stringed_action, check_state),
                    &check_state,
                    current_player,
                    active_god,
                    oppo_god,
                    winning_action.action,
                );
            }
        }
    }

    fn validate_wins(&mut self, wins: &Vec<ScoredMove>) {
        for winning_action in wins {
            self._validate_win_from_current_state(winning_action.action);
        }
    }

    fn _validate_win_from_current_state(&mut self, action: GenericMove) {
        let current_player = self.state.board.current_player;
        let (active_god, oppo_god) = self.state.get_active_non_active_gods();
        self._validate_win(
            &"FromRootState",
            &self.state.clone(),
            current_player,
            active_god,
            oppo_god,
            action,
        );
    }

    fn _validate_win(
        &mut self,
        label: &str,
        state: &FullGameState,
        current_player: Player,
        active_god: StaticGod,
        oppo_god: StaticGod,
        action: GenericMove,
    ) {
        let stringed_action = active_god.stringify_move(action);
        let won_state = state.next_state(active_god, oppo_god, action);
        if won_state.get_winner() != Some(current_player) {
            self.errors.push(format!(
                "{label}:Winning move did not result in win: {}. {:?} -> {:?} winner: {:?} current_player: {:?}",
                stringed_action, state, won_state, won_state.get_winner(), current_player
            ));
            return;
        }

        let old_workers = state.board.workers[current_player as usize];
        let new_workers = won_state.board.workers[current_player as usize];
        let old_only = old_workers & !new_workers;
        let new_only = new_workers & !old_workers;
        assert_eq!(old_only.count_ones(), 1);
        assert_eq!(new_only.count_ones(), 1);
        let old_pos = old_only.lsb();
        let new_pos = new_only.lsb();
        let old_height = state.board.get_height(old_pos) as i32;
        let new_height = won_state.board.get_height(new_pos) as i32;

        let is_pan_falling_win =
            active_god.god_name == GodName::Pan && new_height <= old_height - 2;

        if active_god.god_name == GodName::Maenads {
            // Maenads wins by dancing
            return;
        }

        if active_god.god_name == GodName::Eros {
            return;
        }

        let can_climb = oppo_god.can_opponent_climb(&state.board, !current_player);
        if !can_climb && !is_pan_falling_win {
            self.errors.push(format!(
                "Win when blocked by athena: {}. {:?} -> {:?}",
                stringed_action, state, won_state,
            ));
            return;
        }

        let win_mask = oppo_god.win_mask;
        if (win_mask & new_only).is_empty() {
            if active_god.god_name == GodName::Castor {
                if _validate_castor_double_move_win(
                    state,
                    &won_state,
                    current_player,
                    action,
                    win_mask,
                ) {
                    return;
                }
            }

            self.errors.push(format!(
                "Winning move did not move to win mask: {}. {:?} -> {:?}",
                stringed_action, state, won_state,
            ));
            return;
        }

        if old_height == 2 && new_height == 3 {
            return;
        }

        if is_pan_falling_win {
            return;
        }

        if active_god.god_name == GodName::Artemis {
            let old_n = NEIGHBOR_MAP[old_pos as usize];
            let new_n = NEIGHBOR_MAP[new_pos as usize];
            let path = old_n & new_n;
            let path = path & state.board.exactly_level_2();
            let path = path & !(state.board.workers[0] | state.board.workers[1]);

            if (old_height == 1 || old_height == 3) && new_height == 3 && path.is_not_empty() {
                return;
            }

            if oppo_god.god_name == GodName::Harpies {
                let mut matched = false;
                for n in old_n {
                    let slide_n = slide_position_with_custom_blockers(
                        &state.board,
                        old_pos,
                        n,
                        state.board.workers[0] | state.board.workers[1],
                    );
                    if state.board.get_height(slide_n) == 2 {
                        let final_n = NEIGHBOR_MAP[slide_n as usize];

                        if (new_only & final_n).is_not_empty() {
                            matched = true;
                            break;
                        }
                    }
                }

                if matched {
                    return;
                }
            }
        }

        if active_god.god_name == GodName::Castor {
            if _validate_castor_double_move_win(state, &won_state, current_player, action, win_mask)
            {
                return;
            }
        }

        // Iris wins by jumping over a worker to level 2
        if active_god.god_name == GodName::Iris {
            if let Some(jumped_pos) = BETWEEN_MAPPING[old_pos as usize][new_pos as usize] {
                let all_workers = won_state.board.workers[0] | won_state.board.workers[1];

                if all_workers.contains_square(jumped_pos) && old_height < 3 && new_height == 3 {
                    return;
                }
            }
        }

        self.errors.push(format!(
            "Move won with unknown winning condition: {}. {:?} -> {:?}. Old pos: {old_pos} New pos: {new_pos}. Old height: {old_height} New height: {new_height}",
            stringed_action, state, won_state,
        ));
    }

    fn _test_blocker_moves_are_consistent(
        &mut self,
        scored: &Vec<ScoredMove>,
        unscored: &Vec<ScoredMove>,
    ) {
        if scored.len() != unscored.len() {
            self.errors
                .push("Scored and unscored blocker boards had different lengths".to_owned());
            return;
        }

        let active_god = self.state.get_active_god();
        for (i, (s, u)) in scored.iter().zip(unscored).enumerate() {
            if (s.action.0 & MOVE_DATA_MAIN_SECTION) != (u.action.0 & MOVE_DATA_MAIN_SECTION) {
                self.errors.push(format!(
                    "Blocker move {i} are different. Scored: {} {:b} Unscored: {} {:b}",
                    active_god.stringify_move(s.action),
                    s.action.0,
                    active_god.stringify_move(u.action),
                    u.action.0,
                ));
                return;
            }
        }
    }
}

fn _test_castor_bad_key_move_blockers(state: &FullGameState) -> bool {
    // Castor has you in check
    // One of his workers is on lvl 3, the other on lvl 2
    // And the check is to move the lvl 3 worker out of the way, so the lvl 2 worker can step up
    // The spot that the lvl 2 worker moves into is a key square, so you try to build there to
    // block it.
    // If that doesn't dome the square, that's not enough. But whatever.

    let current_player = state.board.current_player;
    let (_active_god, oppo_god) = state.get_active_non_active_gods();

    if oppo_god.god_name != GodName::Castor {
        return false;
    }

    let castor_workers = state.board.workers[!current_player as usize];
    let exactly_2 = castor_workers & state.board.height_map[1] & !state.board.height_map[2];
    let exactly_3 = castor_workers & state.board.height_map[2] & !state.board.height_map[3];

    if exactly_2.count_ones() != 1 {
        return false;
    }
    if exactly_3.count_ones() != 1 {
        return false;
    }

    let pos2 = exactly_2.lsb();
    if (NEIGHBOR_MAP[pos2 as usize] & exactly_3).is_empty() {
        return false;
    }

    return true;
}

fn _validate_castor_double_move_win(
    prev_state: &FullGameState,
    won_state: &FullGameState,
    _player: Player,
    action: GenericMove,
    win_mask: BitBoard,
) -> bool {
    let action: CastorMove = action.into();

    let Some(f1) = action.maybe_move_from_position_1() else {
        return false;
    };
    let Some(f2) = action.maybe_move_from_position_2() else {
        return false;
    };

    let t1 = action.move_to_position_1();
    let t2 = action.move_to_position_2();

    let o1 = prev_state.board.get_height(f1) as i32;
    let n1 = won_state.board.get_height(t1) as i32;
    if o1 == 2 && n1 == 3 && (t1.to_board() & win_mask).is_not_empty() {
        return true;
    }

    let o2 = prev_state.board.get_height(f2) as i32;
    let n2 = won_state.board.get_height(t2) as i32;
    if o2 == 2 && n2 == 3 && (t2.to_board() & win_mask).is_not_empty() {
        return true;
    }

    false
}
