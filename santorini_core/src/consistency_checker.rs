use crate::{
    bitboard::BitBoard,
    board::{FullGameState, NEIGHBOR_MAP},
    gods::{
        GodName, StaticGod,
        athena::AthenaMove,
        generic::{CHECK_SENTINEL_SCORE, GenericMove, MOVE_DATA_MAIN_SECTION, ScoredMove},
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
        let current_player = self.state.board.current_player;
        let (active_god, other_god) = self.state.get_active_non_active_gods();

        let other_wins = other_god.get_winning_moves(&self.state, !current_player);

        let search_moves = active_god.get_moves_for_search(&self.state, current_player);
        self._check_wins_on_end_only("SearchMoves", &search_moves);

        let own_winning_moves = active_god.get_winning_moves(&self.state, current_player);

        self._opponent_check_blockers(&other_wins, &search_moves);
        self._self_check_validations(&search_moves);
        self._validate_wins(&own_winning_moves);

        if self.errors.len() == 0 {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn _check_wins_on_end_only(&mut self, label: &str, actions: &Vec<ScoredMove>) -> bool {
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

    fn _opponent_check_blockers(
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
        let blocker_len = scored_blocker_actions.len();
        self._check_wins_on_end_only("ScoredBlockerActions", &scored_blocker_actions);
        self._test_blocker_moves_are_consistent(&scored_blocker_actions, &unscored_blocker_actions);

        // Test that blockers actually block
        for (i, block_action) in scored_blocker_actions.iter().enumerate() {
            let block_action = block_action.action;
            let stringed_action = active_god.stringify_move(block_action);
            let blocked_state = self.state.next_state(active_god, block_action);

            if blocked_state.board.get_winner() == Some(current_player) {
                if i != blocker_len - 1 {
                    self.errors
                        .push(format!("Win blocker won, but wasn't last move: {i}/ {blocker_len}: {stringed_action}"));
                    return;
                }
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

            if other_god.god_name == GodName::Artemis {
                // TODO: scope this down
                // Artemis includes all neighboring 2s as part of their mask, but they aren't
                // nessesarily part of the path, so ignore
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

    fn _self_check_validations(&mut self, search_moves: &Vec<ScoredMove>) {
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
                self.errors.push(format!(
                    "Check flag/real checker mismatch on action {i}/{}: {}. Flag: {} RealChecker: {}",
                    search_moves.len(),
                    stringed_action,
                    is_check_flag,
                    is_real_checker
                ));
                continue;
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

    fn _validate_wins(&mut self, wins: &Vec<ScoredMove>) {
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
