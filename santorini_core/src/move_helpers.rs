#[macro_export]
macro_rules! add_scored_move {
    (
        $new_action:ident,
        $is_include_score:ident,
        $is_check:ident,
        $is_improving:ident,
        $result:ident
    ) => {
        let scored_move = if !$is_include_score {
            ScoredMove::new_unscored_move($new_action.into())
        } else if $is_check {
            ScoredMove::new_checking_move($new_action.into())
        } else if $is_improving {
            ScoredMove::new_improving_move($new_action.into())
        } else {
            ScoredMove::new_non_improver($new_action.into())
        };

        $result.push(scored_move)
    };
}

#[macro_export]
macro_rules! build_parse_flags {
    (
        $is_mate_only:ident,
        $is_include_score:ident,
        $is_stop_on_mate:ident,
        $is_interact_with_key_squares:ident
    ) => {
        let $is_mate_only = F & crate::gods::generic::MATE_ONLY != 0;
        let $is_include_score = F & crate::gods::generic::INCLUDE_SCORE != 0;
        let $is_stop_on_mate = F & crate::gods::generic::STOP_ON_MATE != 0;
        let $is_interact_with_key_squares =
            F & crate::gods::generic::INTERACT_WITH_KEY_SQUARES != 0;
    };
}

#[macro_export]
macro_rules! variable_prelude {
    (
        state: $state:ident,
        player: $player:ident,
        board: $board:ident,
        other_player: $other_player:ident,
        current_player_idx: $current_player_idx:ident,
        other_player_idx: $other_player_idx:ident,
        other_god: $other_god:ident,
        exactly_level_0: $exactly_level_0:ident,
        exactly_level_1: $exactly_level_1:ident,
        exactly_level_2: $exactly_level_2:ident,
        exactly_level_3: $exactly_level_3:ident,
        domes: $domes:ident,
        win_mask: $win_mask:ident,
        build_mask: $build_mask:ident,
        is_against_hypnus: $is_against_hypnus:ident,
        own_workers: $own_workers:ident,
        oppo_workers: $oppo_workers:ident,
        result: $result:ident,
        all_workers_mask: $all_workers_mask:ident,
        is_mate_only: $is_mate_only: ident,
        acting_workers: $acting_workers: ident,
        checkable_worker_positions_mask: $checkable_worker_positions_mask: ident,
    ) => {
        $crate::non_checking_variable_prelude!(
            state: $state,
            player: $player,
            board: $board,
            other_player: $other_player,
            current_player_idx: $current_player_idx,
            other_player_idx: $other_player_idx,
            other_god: $other_god,
            exactly_level_0: $exactly_level_0,
            exactly_level_1: $exactly_level_1,
            exactly_level_2: $exactly_level_2,
            exactly_level_3: $exactly_level_3,
            domes: $domes,
            win_mask: $win_mask,
            build_mask: $build_mask,
            is_against_hypnus: $is_against_hypnus,
            own_workers: $own_workers,
            oppo_workers: $oppo_workers,
            result: $result,
            all_workers_mask: $all_workers_mask,
            is_mate_only: $is_mate_only,
            acting_workers: $acting_workers,
        );

        let $checkable_worker_positions_mask = $exactly_level_2;
        if $is_mate_only {
            $acting_workers &= $checkable_worker_positions_mask;
        }
    };
}

#[macro_export]
macro_rules! non_checking_variable_prelude {
    (
        state: $state:ident,
        player: $player:ident,
        board: $board:ident,
        other_player: $other_player:ident,
        current_player_idx: $current_player_idx:ident,
        other_player_idx: $other_player_idx:ident,
        other_god: $other_god: ident,
        exactly_level_0: $exactly_level_0:ident,
        exactly_level_1: $exactly_level_1:ident,
        exactly_level_2: $exactly_level_2:ident,
        exactly_level_3: $exactly_level_3:ident,
        domes: $domes:ident,
        win_mask: $win_mask: ident,
        build_mask: $build_mask: ident,
        is_against_hypnus: $is_against_hypnus: ident,
        own_workers: $own_workers:ident,
        oppo_workers: $oppo_workers:ident,
        result: $result:ident,
        all_workers_mask: $all_workers_mask:ident,
        is_mate_only: $is_mate_only: ident,
        acting_workers:  $acting_workers: ident,
    ) => {
        let $board = &$state.board;
        let $other_player = !$player;

        let $current_player_idx = $player as usize;
        let $other_player_idx = $other_player as usize;
        let $other_god = $state.gods[$other_player_idx];

        #[allow(unused_variables)]
        let $exactly_level_0 = $board.exactly_level_0();
        #[allow(unused_variables)]
        let $exactly_level_1 = $board.exactly_level_1();
        let $exactly_level_2 = $board.exactly_level_2();
        let $exactly_level_3 = $board.exactly_level_3();
        #[allow(unused_variables)]
        let $domes = $board.at_least_level_4();

        let $own_workers = $board.workers[$current_player_idx] & BitBoard::MAIN_SECTION_MASK;
        let $oppo_workers = $board.workers[$other_player_idx] & BitBoard::MAIN_SECTION_MASK;

        let capacity = if $is_mate_only { 1 } else { 128 };
        let mut $result: Vec<ScoredMove> = Vec::with_capacity(capacity);
        let $all_workers_mask = $own_workers | $oppo_workers;

        let $win_mask = $other_god.win_mask;
        let $build_mask = $other_god.get_build_mask($oppo_workers) | $exactly_level_3;

        let $is_against_hypnus = $other_god.is_hypnus();
        let mut $acting_workers = $own_workers;
        if $is_against_hypnus {
            $acting_workers =
                crate::gods::hypnus::hypnus_moveable_worker_filter(&$board, $acting_workers);
        }
    };
}

#[macro_export]
macro_rules! build_push_winning_moves {
    (
        $win_mask:ident,
        $worker_moves:ident,
        $build_winning_move:path,
        $worker_start_pos:path,
        $result:ident,
        $is_stop_on_mate:ident,
    ) => {
        $worker_moves ^= $win_mask;
        for end_position in $win_mask.into_iter() {
            let winning_move = ScoredMove::new_winning_move(
                $build_winning_move($worker_start_pos, end_position).into(),
            );
            $result.push(winning_move);
            if $is_stop_on_mate {
                return $result;
            }
        }
    };
}

#[macro_export]
macro_rules! build_building_masks {
    (
        worker_end_pos: $worker_end_pos: ident,
        open_squares: $open_squares: ident,
        build_mask: $build_mask: ident,
        is_interact_with_key_squares: $is_interact_with_key_squares:path,
        key_squares_expr: $key_squares_expr:expr,
        key_squares: $key_squares:ident,

        all_possible_builds: $all_possible_builds:ident,
        narrowed_builds: $narrowed_builds:ident,
        worker_plausible_next_moves: $worker_plausible_next_moves:ident,
    ) => {
        let mut $all_possible_builds =
            crate::bitboard::NEIGHBOR_MAP[$worker_end_pos as usize] & $open_squares;
        let $worker_plausible_next_moves = $all_possible_builds;
        $all_possible_builds &= $build_mask;
        let mut $narrowed_builds = $all_possible_builds;

        if $is_interact_with_key_squares {
            #[allow(unused_parens)]
            if $key_squares_expr {
                $narrowed_builds &= $key_squares;
            }
        }
    };
}

#[macro_export]
macro_rules! build_power_move_generator {
    (
        $fn_name:ident,
        build_winning_move: $build_winning_move:path,
        state: $state:ident,
        is_include_score: $is_include_score:ident,
        is_check: $is_check:ident,
        is_improving: $is_improving:ident,
        exactly_level_1: $exactly_level_1: ident,
        exactly_level_2: $exactly_level_2: ident,
        exactly_level_3: $exactly_level_3: ident,
        worker_start_pos: $worker_start_pos: ident,
        worker_end_pos: $worker_end_pos: ident,
        all_possible_builds: $all_possible_builds:ident,
        narrowed_builds: $narrowed_builds:ident,
        reach_board: $reach_board:ident,
        unblocked_squares: $unblocked_squares:ident,
        result: $result:ident,
        building_block: $building_block: block,
        extra_init: $extra_init:stmt,
    ) => {
        pub fn $fn_name<const F: MoveGenFlags>(
            $state: &FullGameState,
            player: Player,
            key_squares: BitBoard,
        ) -> Vec<ScoredMove> {
            $crate::build_parse_flags!(is_mate_only, $is_include_score, is_stop_on_mate, is_interact_with_key_squares);

            $crate::variable_prelude!(
                state: $state,
                player:  player,
                board:  board,
                other_player:  other_player,
                current_player_idx:  current_player_idx,
                other_player_idx:  other_player_idx,
                other_god:  other_god,
                exactly_level_0:  exactly_level_0,
                exactly_level_1: $exactly_level_1,
                exactly_level_2: $exactly_level_2,
                exactly_level_3: $exactly_level_3,
                domes:  domes,
                win_mask:  win_mask,
                build_mask: build_mask,
                is_against_hypnus: is_against_hypnus,
                own_workers:  own_workers,
                oppo_workers:  oppo_workers,
                result: $result,
                all_workers_mask:  all_workers_mask,
                is_mate_only:  is_mate_only,
                acting_workers:  acting_workers,
                checkable_worker_positions_mask:  checkable_worker_positions_mask,
            );

            $extra_init

            for $worker_start_pos in acting_workers.into_iter() {
                let moving_worker_start_mask = BitBoard::as_mask($worker_start_pos);
                #[allow(unused_variables)]
                let other_own_workers = own_workers ^ moving_worker_start_mask;
                let worker_starting_height = board.get_height($worker_start_pos);

                let other_threatening_workers =
                    (own_workers ^ moving_worker_start_mask) & checkable_worker_positions_mask;
                let other_threatening_neighbors = $crate::bitboard::apply_mapping_to_mask(other_threatening_workers, &$crate::bitboard::NEIGHBOR_MAP);

                let mut worker_moves = crate::bitboard::NEIGHBOR_MAP[$worker_start_pos as usize]
                    & !(board.height_map
                        [board.get_worker_climb_height(player, worker_starting_height)]
                        | all_workers_mask);

                if is_mate_only || worker_starting_height == 2 {
                    let moves_to_level_3 = worker_moves & $exactly_level_3 & win_mask;
                    $crate::build_push_winning_moves!(
                        moves_to_level_3,
                        worker_moves,
                        $build_winning_move,
                        $worker_start_pos,
                        $result,
                        is_stop_on_mate,
                    );
                }

                if is_mate_only {
                    continue;
                }

                let non_selected_workers = all_workers_mask ^ moving_worker_start_mask;
                let $unblocked_squares = !(non_selected_workers | domes);

                for $worker_end_pos in worker_moves.into_iter() {
                    let moving_worker_end_mask = BitBoard::as_mask($worker_end_pos);
                    let worker_end_height = board.get_height($worker_end_pos);
                    let $is_improving = worker_end_height > worker_starting_height;

                    $crate::build_building_masks!(
                        worker_end_pos: $worker_end_pos,
                        open_squares: $unblocked_squares,
                        build_mask: build_mask,
                        is_interact_with_key_squares: is_interact_with_key_squares,
                        key_squares_expr: (moving_worker_end_mask & key_squares).is_empty(),
                        key_squares: key_squares,

                        all_possible_builds: $all_possible_builds,
                        narrowed_builds: $narrowed_builds,
                        worker_plausible_next_moves: worker_plausible_next_moves,
                    );

                    let own_final_workers = other_own_workers | moving_worker_end_mask;
                    let is_now_lvl_2 =  (worker_end_height == 2) as usize;

                    let $reach_board =
                    if is_against_hypnus && (other_threatening_workers.count_ones() as usize + is_now_lvl_2) < 2 {
                        BitBoard::EMPTY
                    } else {
                        (other_threatening_neighbors | (worker_plausible_next_moves & BitBoard::CONDITIONAL_MASK[is_now_lvl_2]))
                            & win_mask
                            & !own_final_workers
                    };

                    $building_block
                }
            }

            $result
        }
    };
}
