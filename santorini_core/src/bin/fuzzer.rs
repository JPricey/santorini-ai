use rand::{Rng, rng};

use santorini_core::{
    board::FullGameState,
    consistency_checker::consistency_check,
    random_utils::{get_board_with_random_placements, get_random_god, get_random_move},
};

fn run_match(root_state: FullGameState, rng: &mut impl Rng) {
    let mut current_state = root_state;
    loop {
        if current_state.board.get_winner().is_some() {
            return;
        }

        if let Err(err) = consistency_check(&current_state) {
            eprintln!("Consistency check failed: {:?}", current_state);
            current_state.print_to_console();

            for error_line in err {
                eprintln!("{error_line}");
            }
            return;
            // panic!("Consistency check failed");
        }

        if let Some(next_state) = get_random_move(&current_state, rng) {
            current_state = next_state;
        } else {
            // current_state.print_to_console();
            return;
        }
    }
}

fn main() {
    let mut rng = rng();

    let banned_gods = vec![];

    loop {
        let mut root_state = get_board_with_random_placements(&mut rng);
        // root_state.gods[0] = GodName::Minotaur.to_power();
        // root_state.gods[1] = GodName::Mortal.to_power();

        root_state.gods[0] = get_random_god(&mut rng);
        root_state.gods[1] = get_random_god(&mut rng);

        if banned_gods.contains(&root_state.gods[0].god_name)
            || banned_gods.contains(&root_state.gods[1].god_name)
        {
            continue;
        }

        run_match(root_state, &mut rng);
    }
}
