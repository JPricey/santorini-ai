#![allow(unused)]

use santorini_ai::{
    board::{FullChoice, MAIN_SECTION_MASK, PartialAction, Player, SantoriniState},
    search::{AlphaBetaSearch, NUM_SEARCHES, WINNING_SCORE},
};
use std::io;

const WIN_IN_1_STRING: &str = "0000011111222223333322222/10,11/0,1";
const PREVENT_WIN_IN_1_STRING: &str = "3200000000000000000022222/24,12/1,4";
const FORCE_WIN_IN_2_STRING: &str = "2200020000000000000011111/1,12/11,20";
const WIN_FASTER: &str = "2223222222000000444444400/0/20,24";

const WTF: &str = "1110021320210000000000000/2,10/5,8";

fn narrow_options(
    options: &Vec<FullChoice>,
    action_to_string: impl Fn(&FullChoice) -> String,
) -> Option<Vec<FullChoice>> {
    let mut string_options: Vec<String> = Vec::new();
    for action in options {
        let stringed_action = action_to_string(action);
        if !string_options.contains(&stringed_action) {
            string_options.push(stringed_action);
        }
    }

    let selected_idx = select_cli_option(&string_options)?;
    let selected_option = string_options[selected_idx].clone();

    let mut res = Vec::<FullChoice>::new();
    for action in options {
        let stringed_action = action_to_string(action);
        if stringed_action == selected_option {
            res.push(action.clone());
        }
    }

    Some(res)
}

fn select_cli_option(options: &Vec<String>) -> Option<usize> {
    for (i, option) in options.iter().enumerate() {
        println!("{}: {}", i, option);
    }

    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).unwrap();
    buffer = buffer.to_lowercase().trim().to_string();

    if let Ok(number) = buffer.trim().parse::<usize>() {
        if number >= options.len() {
            return None;
        }
        return Some(number);
    };

    for (i, option) in options.iter().enumerate() {
        if buffer == option.to_lowercase() {
            return Some(i);
        }
    }

    return None;
}

fn string_partial_action(partial_action: PartialAction) -> String {
    match partial_action {
        PartialAction::PlaceWorker(coord) => format!("{:?}", coord),
        PartialAction::SelectWorker(coord) => format!("{:?}", coord),
        PartialAction::MoveWorker(coord) => format!("{:?}", coord),
        PartialAction::Build(coord) => format!("{:?}", coord),
        PartialAction::NoMoves => "skip".to_owned(),
    }
}

fn _inner_get_human_action(mut all_outcomes: Vec<FullChoice>) -> Option<SantoriniState> {
    let mut idx = 0;
    while all_outcomes.len() > 1 {
        if let Some(new_options) = narrow_options(&all_outcomes, move |full_choice: &FullChoice| {
            string_partial_action(full_choice.action[idx])
        }) {
            all_outcomes = new_options;
        } else {
            return None;
        };
        idx += 1;
    }

    assert!(all_outcomes.len() == 1);
    Some(all_outcomes[0].result_state.clone())
}

fn get_human_action(state: &SantoriniState) -> SantoriniState {
    let all_outcomes = state.get_next_states_interactive();

    loop {
        state.print_to_console();
        if let Some(result) = _inner_get_human_action(all_outcomes.clone()) {
            return result;
        }
    }
}

trait Agent {
    fn make_move(&mut self, state: &SantoriniState) -> SantoriniState;
}

struct ComputerAgent {
    depth: usize,
}

impl Default for ComputerAgent {
    fn default() -> Self {
        ComputerAgent { depth: 6 }
    }
}

impl Agent for ComputerAgent {
    fn make_move(&mut self, state: &SantoriniState) -> SantoriniState {
        let start_time = std::time::Instant::now();
        let (child, score) = AlphaBetaSearch::search(state, self.depth);
        let outcome = state.get_path_to_outcome(&child);

        if score.abs() < WINNING_SCORE
            && start_time.elapsed().as_secs_f32() < 2.0
            && self.depth < 25
        {
            self.depth += 1;
            println!(
                "Computer thought for {:?}. Expanding depth to {}. (score: {score})",
                start_time.elapsed(),
                self.depth
            );
            return self.make_move(state);
        } else {
            let elapsed = start_time.elapsed();
            println!(
                "Computer player {:?} thought for {:?} at depth {}. Choosing move: {:?} with score: {}",
                state.current_player, elapsed, self.depth, outcome, score
            );
            return child;
        }
    }
}

struct HumanAgent {}

impl Default for HumanAgent {
    fn default() -> Self {
        HumanAgent {}
    }
}

impl Agent for HumanAgent {
    fn make_move(&mut self, state: &SantoriniState) -> SantoriniState {
        get_human_action(state)
    }
}

fn get_computer_action(state: &SantoriniState, depth: usize) -> SantoriniState {
    let (child, score) = AlphaBetaSearch::search(&state, depth);
    let outcome = state.get_path_to_outcome(&child);
    println!(
        "Computer player {:?} choosing move: {:?} with score: {}",
        state.current_player, outcome, score
    );
    return child;
}

fn alpha_beta_test() {
    let mut state = SantoriniState::new_basic_state();

    state.workers[0] ^= 1 << 17;
    state.workers[0] ^= 1 << 22;
    state.height_map[0] ^= 1 << 21;

    state.current_player = Player::Two;

    state.print_to_console();
    get_computer_action(&state, 5);
}

fn play(starting_string: Option<&str>) {
    let mut p1 = ComputerAgent::default();
    // let mut p1 = HumanAgent::default();
    let mut p2 = ComputerAgent::default();
    // let mut p2 = HumanAgent::default();

    let mut state = if let Some(string) = starting_string {
        SantoriniState::try_from(string).unwrap()
    } else {
        SantoriniState::new_basic_state()
    };

    loop {
        state.print_to_console();
        if state.current_player == Player::One {
            state = p1.make_move(&state);
        } else {
            state = p2.make_move(&state);
        };
        if let Some(winner) = state.get_winner() {
            println!("Winner: {:?}", winner);
            state.print_to_console();
            break;
        }
    }
}

fn test(case: &str, depth: usize) {
    let state = SantoriniState::try_from(case).unwrap();
    state.print_to_console();
    get_computer_action(&state, depth).print_to_console();
}

fn main() {
    // test(FORCE_WIN_IN_2_STRING, 6);
    // test(WTF, 8);
    // play(Some(FORCE_WIN_IN_2_STRING));
    // play(Some(WIN_FASTER));
    // play(Some(WTF));
    play(None);
    // test(PREVENT_WIN_IN_1_STRING);
    // test(FORCE_WIN_IN_2_STRING);
    // test(WIN_FASTER, 12);
    // alpha_beta_test();
    // play();

    return;

    println!("{}, {}", MAIN_SECTION_MASK, MAIN_SECTION_MASK.count_ones());

    let board_string = "0020322300000003333300000/5,10/12,23";
    let s = SantoriniState::try_from(board_string).unwrap();

    s.print_to_console();

    let (child, score) = AlphaBetaSearch::search(&s, 5);
    let outcome = s.get_path_to_outcome(&child);
    println!("{:?}", outcome);

    child.print_to_console();

    println!("Score: {score}");
}
