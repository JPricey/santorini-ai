use rand::{Rng, thread_rng};

use santorini_core::{
    board::FullGameState,
    gods::pan,
    random_utils::{get_board_with_random_placements, get_random_god, get_random_move},
};

fn check_state(root_state: &FullGameState) {
    let board = &root_state.board;

    let other_god = root_state.get_other_god();
    let active_god = root_state.get_active_god();

    let other_wins = other_god.get_winning_moves(&board, !board.current_player);

    let winning_moves = active_god.get_winning_moves(&board, board.current_player);
    let all_moves = active_god.get_moves_for_search(&board, board.current_player);

    // Test uniqueness & make/unmake
    {
        let mut all_states = Vec::new();
        for action in &all_moves {
            let mut board_clone = board.clone();
            active_god.make_move(&mut board_clone, action.action);
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
        root_state.gods[0] = get_random_god(&mut rng);
        root_state.gods[1] = get_random_god(&mut rng);

        root_state.print_to_console();

        run_match(root_state, &mut rng);
    }
}
