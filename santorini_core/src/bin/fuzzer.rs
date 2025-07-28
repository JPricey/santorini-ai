use rand::{Rng, thread_rng};

use santorini_core::{
    board::{FullGameState, NEIGHBOR_MAP},
    gods::GodName,
    random_utils::{get_board_with_random_placements, get_random_god, get_random_move},
};

fn check_state(root_state: &FullGameState) {
    let board = &root_state.board;
    let current_player = board.current_player;

    // let other_god = root_state.get_other_god();
    let active_god = root_state.get_active_god();

    // let other_wins = other_god.get_winning_moves(&board, !board.current_player);

    let winning_moves = active_god.get_winning_moves(&board, board.current_player);
    let all_moves = active_god.get_moves_for_search(&board, board.current_player);
    let checks = active_god.get_improver_moves(&board, board.current_player);

    // Test that checks actually result in wins
    for action in &checks {
        let stringed_action = active_god.stringify_move(action.action);
        let mut new_board = board.clone();
        active_god.make_move(&mut new_board, action.action);
        new_board.unset_worker_can_climb();

        if new_board.get_winner().is_some() {
            continue;
        }

        let winning_moves = active_god.get_winning_moves(&new_board, current_player);
        if winning_moves.len() == 0 {
            root_state.print_to_console();
            new_board.print_to_console();
            panic!("check move didn't result in a win: {}", stringed_action);
        }
    }

    // Test that no checks are missed. only relevant if we don't win on the spot
    if winning_moves.len() == 0 && board.get_worker_can_climb(current_player) {
        for action in &all_moves {
            let stringed_action = active_god.stringify_move(action.action);
            let mut new_board = board.clone();
            active_god.make_move(&mut new_board, action.action);
            new_board.unset_worker_can_climb();

            if active_god
                .get_winning_moves(&new_board, current_player)
                .len()
                > 0
            {
                if !checks.contains(action) {
                    root_state.print_to_console();
                    new_board.print_to_console();
                    panic!(
                        "Move was a check/win but wasn't in checks: {}",
                        stringed_action
                    );
                }
            }
        }
    }

    {
        // test that wins actually win
        for action in &winning_moves {
            let stringed_action = active_god.stringify_move(action.action);
            let mut new_board = board.clone();

            active_god.make_move(&mut new_board, action.action);
            if new_board.get_winner() != Some(board.current_player) {
                board.print_to_console();
                panic!("Winning move didn't actually win: {}", stringed_action);
            }

            let old_workers = board.workers[current_player as usize];
            let new_workers = new_board.workers[current_player as usize];
            let old_only = old_workers & !new_workers;
            let new_only = new_workers & !old_workers;

            assert_eq!(old_only.count_ones(), 1);
            assert_eq!(new_only.count_ones(), 1);

            let old_pos = old_only.lsb();
            let new_pos = new_only.lsb();

            let old_height = board.get_height_for_worker(old_only);
            let new_height = board.get_height_for_worker(new_only);

            let mut is_valid_win = false;
            if old_height == 2 && new_height == 3 {
                is_valid_win = true;
            } else if active_god.god_name == GodName::Pan
                && (old_height == 2 && new_height == 0 || old_height == 3 && new_height <= 1)
            {
                is_valid_win = true;
            } else if active_god.god_name == GodName::Artemis {
                let old_n = NEIGHBOR_MAP[old_pos as usize];
                let new_n = NEIGHBOR_MAP[new_pos as usize];
                let path = old_n & new_n;
                let path = path & board.exactly_level_2();
                let path = path & !(board.workers[0] | board.workers[1]);

                if (old_height == 1 || old_height == 3) && new_height == 3 && path.is_not_empty() {
                    is_valid_win = true;
                }
            }

            if !is_valid_win {
                root_state.print_to_console();
                new_board.print_to_console();
                eprintln!(
                    "action: {}. o:{old_pos}:{old_height} n:{new_pos}:{new_height}",
                    stringed_action
                );
                panic!("unexpected winning move");
            }
        }
    }

    {
        let mut all_states = Vec::new();
        for action in &all_moves {
            let stringed_action = active_god.stringify_move(action.action);

            let mut board_clone = board.clone();
            active_god.make_move(&mut board_clone, action.action);

            // no missing winning moves
            if board_clone.get_winner() == Some(board.current_player) {
                if !winning_moves.contains(action) {
                    board.print_to_console();
                    board_clone.print_to_console();
                    panic!(
                        "Move lead to win, but wasn't a winning move: {}",
                        stringed_action
                    );
                }
            }

            // Test uniqueness
            if all_states.contains(&board_clone) {
                eprintln!("Root state:");
                root_state.print_to_console();

                eprintln!("Cloned state:");
                board_clone.print_to_console();

                eprintln!(
                    "Duplicate state found after making move: {:?}",
                    active_god.stringify_move(action.action)
                );

                for other_action in &all_moves {
                    let mut bc2 = board.clone();
                    active_god.make_move(&mut bc2, other_action.action);
                    if bc2 == board_clone {
                        eprintln!(
                            "Duplicate state found with another action: {:?}",
                            active_god.stringify_move(other_action.action)
                        );
                        bc2.print_to_console();
                    }
                }

                panic!("");
            } else {
                all_states.push(board_clone.clone());
            }

            // Make/unmake
            active_god.unmake_move(&mut board_clone, action.action);
            if board_clone != *board {
                board_clone.print_to_console();
                panic!(
                    "Unmake move did not restore original state: {:?}",
                    active_god.stringify_move(action.action)
                );
            }
        }
    }
}

fn run_match(root_state: FullGameState, rng: &mut impl Rng) {
    let mut current_state = root_state;
    loop {
        if current_state.board.get_winner().is_some() {
            return;
        }

        check_state(&current_state);
        if let Some(next_state) = get_random_move(&current_state, rng) {
            current_state = next_state;
        } else {
            // current_state.print_to_console();
            return;
        }
    }
}

fn main() {
    let mut rng = thread_rng();

    loop {
        let mut root_state = get_board_with_random_placements(&mut rng);
        // root_state.gods[0] = GodName::Minotaur.to_power();
        // root_state.gods[1] = GodName::Mortal.to_power();

        root_state.gods[0] = get_random_god(&mut rng);
        root_state.gods[1] = get_random_god(&mut rng);

        if root_state.gods[0].god_name == GodName::Minotaur || root_state.gods[1].god_name == GodName::Minotaur {
            continue;
        }

        // root_state.print_to_console();

        run_match(root_state, &mut rng);
    }
}
