use std::collections::HashMap;

use crate::{
    bitboard::{BitBoard, INCLUSIVE_NEIGHBOR_MAP, NEIGHBOR_MAP, apply_mapping_to_mask},
    board::{BoardState, FullGameState},
    gods::{
        GodName, StaticGod,
        athena::AthenaMove,
        generic::{CHECK_SENTINEL_SCORE, GenericMove, MOVE_DATA_MAIN_SECTION, ScoredMove},
        harpies::slide_position,
        mortal::MortalMove,
    },
    player::Player,
};

pub fn consistency_check(state: &FullGameState) -> Result<(), Vec<String>> {
    let mut checker = ConsistencyChecker::new(state);
    checker.perform_all_validations()
}

/// Performs validations on god move generators,
/// such as ensuring that win/check tags are correct
/// And that opponent turn powers are correctly respected
struct ConsistencyChecker {
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
            self.errors.push(err);
        } else {
            let current_player = self.state.board.current_player;
            let (active_god, other_god) = self.state.get_active_non_active_gods();

            let other_wins = other_god.get_winning_moves(&self.state, !current_player);

            let search_moves = active_god.get_moves_for_search(&self.state, current_player);
            self.check_wins_on_end_only("SearchMoves", &search_moves);

            let own_winning_moves = active_god.get_winning_moves(&self.state, current_player);

            self.opponent_check_blockers(&other_wins, &search_moves);
            self.self_check_validations(&search_moves);
            self.validate_non_duplicates(&search_moves);
            self.validate_wins(&own_winning_moves);
            self.validate_build_blockers(&search_moves);
            self.validate_hypnus_moves(&search_moves);
            self.validate_aphrodite_moves(&search_moves);
        }

        if self.errors.len() == 0 {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn validate_non_duplicates(&mut self, actions: &Vec<ScoredMove>) {
        let mut seen = HashMap::<BoardState, GenericMove>::new();
        let active_god = self.state.get_active_god();

        for action in actions {
            let action = action.action;
            let new_state = self.state.next_state(active_god, action);

            if let Some(other_action) = seen.get(&new_state.board) {
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

    fn validate_aphrodite_moves(&mut self, actions: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, other_god) = self.state.get_active_non_active_gods();

        if other_god.god_name != GodName::Aphrodite {
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

            let new_state = self.state.next_state(active_god, action);
            let new_workers = new_state.board.workers[current_player as usize];

            let old_only = old_workers & !new_workers;
            if (old_only & old_affinity_area).is_empty() {
                continue;
            }

            let new_only = new_workers & !old_workers;
            let new_aphro_workers = new_state.board.workers[!current_player as usize];
            let new_affinity_area =
                apply_mapping_to_mask(new_aphro_workers, &INCLUSIVE_NEIGHBOR_MAP);

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
        let (active_god, other_god) = self.state.get_active_non_active_gods();

        if other_god.god_name != GodName::Hypnus {
            return;
        }

        for action in actions {
            let action = action.action;

            let new_state = self.state.next_state(active_god, action);
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
        let (active_god, other_god) = self.state.get_active_non_active_gods();

        if other_god.god_name != GodName::Limus {
            return;
        }

        let mut dome_build_actions = Vec::new();

        // Returns a mask of all builds done, except for lvl 3 -> domes (which limus allows)
        fn get_new_builds_mask(new_state: &BoardState, old_state: &BoardState) -> BitBoard {
            let mut new_builds = BitBoard::EMPTY;
            new_builds |= new_state.height_map[2] & !old_state.height_map[2];
            new_builds |= new_state.height_map[1] & !old_state.height_map[1];
            new_builds |= new_state.height_map[0] & !old_state.height_map[0];
            let new_dome_builds_from_non_lvl_3 =
                new_state.height_map[3] & !old_state.height_map[2] & !old_state.height_map[3];

            new_builds | new_dome_builds_from_non_lvl_3
        }

        for action in actions {
            let action = action.action;
            if action.get_is_winning() {
                continue;
            }

            let new_state = self.state.next_state(active_god, action);
            let new_builds = get_new_builds_mask(&new_state.board, &self.state.board);
            let new_dome_builds_from_lvl_3 = new_state.board.height_map[3]
                & self.state.board.height_map[2]
                & !self.state.board.height_map[3];

            let build_mask =
                other_god.get_build_mask(new_state.board.workers[!current_player as usize]);

            if new_dome_builds_from_lvl_3.is_not_empty() {
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

        let mut against_mortal_state = self.state.clone();
        against_mortal_state.gods[!current_player as usize] = GodName::Mortal.to_power();
        let mortal_search_moves =
            active_god.get_moves_for_search(&against_mortal_state, current_player);

        for mortal_move in mortal_search_moves {
            let mortal_action = mortal_move.action;
            let new_state = self.state.next_state(active_god, mortal_action);
            // We could have built a dome and ALSO somewhere else. these moves are invalid too, so
            // skip.
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
                        "Was able to build vaid dome against mortal, but not limus: {} -> {:?}",
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
        let (active_god, other_god) = self.state.get_active_non_active_gods();

        if other_wins.is_empty() {
            return;
        }

        let mut key_moves = BitBoard::EMPTY;
        for other_win_action in other_wins {
            key_moves |= other_god.get_blocker_board(&self.state.board, other_win_action.action);
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
            let blocked_state = self.state.next_state(active_god, block_action);

            if blocked_state.board.get_winner() == Some(current_player) {
                continue;
            }

            let post_block_oppo_wins = other_god.get_winning_moves(&blocked_state, !current_player);
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

            if other_god.god_name == GodName::Pan {
                let any_pan_move: MortalMove = other_wins[0].action.into();

                if active_god.god_name == GodName::Athena {
                    let athena_move: AthenaMove = block_action.into();
                    let did_pan_fall = self
                        .state
                        .board
                        .get_height(any_pan_move.move_from_position())
                        >= self.state.board.get_height(any_pan_move.move_to_position()) + 2;

                    if athena_move.get_did_climb() && did_pan_fall {
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
            }

            if other_god.god_name == GodName::Artemis {
                // Artemis can have multiple paths to level 3, but only the start and end are
                // reflected in the winning move.
                // Check that we at least made the key moves map smaller
                // let mut blocked_key_moves = BitBoard::EMPTY;
                // for other_win_action in post_block_oppo_wins {
                //     blocked_key_moves |=
                //         other_god.get_blocker_board(&blocked_state.board, other_win_action.action);
                // }
                // if key_moves & blocked_key_moves == key_moves {
                //     let mut err_str =
                //         format!("Block move didn't remove any wins: {}: ", stringed_action);
                //     for winning_action in other_wins {
                //         err_str.push_str(&format!(
                //             "{} ",
                //             other_god.stringify_move(winning_action.action)
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

            if other_god.god_name == GodName::Minotaur {
                // TODO: scope this down
                // Minotaur puts spots that it pushes TO during a mate into the blocker board
                // but this only works on dome builds / moves - not lower builds.
                continue;
            }

            let mut err_str = format!("Block move didn't remove any wins: {}: ", stringed_action);
            for winning_action in other_wins {
                err_str.push_str(&format!(
                    "{} ",
                    other_god.stringify_move(winning_action.action)
                ));
            }
            err_str.push_str(&format!("\n{}", key_moves));

            self.errors.push(err_str);
            return;
        }

        // Test that we didn't miss any blockers
        for action in search_moves {
            let action = action.action;
            if scored_blocker_actions.iter().any(|a| a.action == action) {
                continue;
            }
            let stringed_action = active_god.stringify_move(action);

            let new_state = self.state.next_state(active_god, action);
            let new_oppo_wins = other_god.get_winning_moves(&new_state, !current_player);
            if new_oppo_wins.len() < other_wins.len() {
                let mut error_str = format!(
                    "Missed blocking action: {}. {}\n",
                    stringed_action, key_moves
                );
                error_str += "Old Wins: ";
                for old in other_wins {
                    error_str += &format!("{}, ", other_god.stringify_move(old.action));
                }

                error_str += "New Wins: ";
                for new in &new_oppo_wins {
                    error_str += &format!("{}, ", other_god.stringify_move(new.action));
                }
                self.errors.push(error_str);
            }
        }
    }

    fn self_check_validations(&mut self, search_moves: &Vec<ScoredMove>) {
        let current_player = self.state.board.current_player;
        let (active_god, other_god) = self.state.get_active_non_active_gods();

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

            let mut check_state = self.state.next_state(active_god, action.action);
            check_state.flip_current_player();
            check_state.board.unset_worker_can_climb();
            let wins_from_check_state = active_god.get_winning_moves(&check_state, current_player);
            let is_real_checker = wins_from_check_state.len() > 0;

            if is_check_flag != is_real_checker {
                if is_real_checker
                    && active_god.god_name == GodName::Artemis
                    && other_god.god_name == GodName::Harpies
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
                } else if is_check_flag && other_god.god_name == GodName::Aphrodite {
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
                    other_god,
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
        let (active_god, other_god) = self.state.get_active_non_active_gods();
        self._validate_win(
            &"FromRootState",
            &self.state.clone(),
            current_player,
            active_god,
            other_god,
            action,
        );
    }

    fn _validate_win(
        &mut self,
        label: &str,
        state: &FullGameState,
        current_player: Player,
        active_god: StaticGod,
        other_god: StaticGod,
        action: GenericMove,
    ) {
        let stringed_action = active_god.stringify_move(action);
        let won_state = state.next_state(active_god, action);
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

        if !state.board.get_worker_can_climb(current_player) && !is_pan_falling_win {
            self.errors.push(format!(
                "Win when blocked by athena: {}. {:?} -> {:?}",
                stringed_action, state, won_state,
            ));
            return;
        }

        let win_mask = other_god.win_mask;
        if (win_mask & new_only).is_empty() {
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

            if other_god.god_name == GodName::Harpies {
                let mut matched = false;
                for n in old_n {
                    let slide_n = slide_position(&state.board, old_pos, n);
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

        self.errors.push(format!(
            "Move won with unknown winning condition: {}. {:?} -> {:?}",
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
