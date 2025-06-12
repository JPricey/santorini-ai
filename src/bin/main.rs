#![allow(unused)]

use santorini_ai::{
    board::{FullChoice, MAIN_SECTION_MASK, PartialAction, Player, SantoriniState},
    engine::EngineThreadWrapper,
    search::{NUM_SEARCHES, WINNING_SCORE_BUFFER},
};
use std::io;

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

struct ComputerAgent {
    engine: EngineThreadWrapper,
}

impl Default for ComputerAgent {
    fn default() -> Self {
        ComputerAgent {
            engine: EngineThreadWrapper::new(),
        }
    }
}

impl Agent for ComputerAgent {
    fn make_move(&mut self, state: &SantoriniState) -> SantoriniState {
        let start_time = std::time::Instant::now();
        let best_move = self.engine.search_for_duration(state, 30.0).unwrap();
        let outcome = state.get_path_to_outcome(&best_move.state);

        println!(
            "Computer player {:?}: Choosing move: {:?} with score: {}. Elapsed: {:.2}",
            state.current_player,
            outcome,
            best_move.score,
            start_time.elapsed().as_secs_f32()
        );

        return best_move.state;
    }
}

enum AgentType {
    Human,
    CPU,
}

fn play(starting_string: Option<&str>, p1_type: AgentType, p2_type: AgentType) {
    let mut human: Box<dyn Agent> = Box::new(HumanAgent::default());
    let mut cpu: Box<dyn Agent> = Box::new(ComputerAgent::default());

    let mut state = if let Some(string) = starting_string {
        SantoriniState::try_from(string).unwrap()
    } else {
        SantoriniState::new_basic_state()
    };

    loop {
        state.print_to_console();
        let agent = match state.current_player {
            Player::One => match p1_type {
                AgentType::Human => &mut human,
                AgentType::CPU => &mut cpu,
            },
            Player::Two => match p2_type {
                AgentType::Human => &mut human,
                AgentType::CPU => &mut cpu,
            },
        };
        state = agent.make_move(&state);
        if let Some(winner) = state.get_winner() {
            println!("Winner: {:?}", winner);
            state.print_to_console();
            break;
        }

        println!("");
    }
}

fn main() {
    // test("0000011111222223333322222/10,11/0,1", 4); // win in 1
    // test("3200000000000000000022222/24,12/1,4", 4); // prevent win in 1
    // test("2200020000000000000011111/1,12/11,20", 4); // force win in 2
    // test("2223222222000000444444400/0/20,24", 4); // win fast
    // test("1110021320210000000000000/2,10/5,8", 4); // bug
    // test("1120011100400000000000000/1/0,5/1,7", 4); // other bug/?

    // test("1120011100400000000000000/2/0,5/1,7", 6);

    // test("1120011200400000000000000/1/0,5/1,2", 5);

    // test("1120011200400000100000000/2/0,11/1,2", 6);

    // play(Some(FORCE_WIN_IN_2_STRING));
    // play(Some(WIN_FASTER));
    // play(Some(WTF));

    // play(Some("0002000000010000010300001/1/2,24/11,13"));
    // play(None, AgentType::CPU, AgentType::CPU);
    play(
        Some("4112202311011420102000100/2/3,14/1,12"),
        AgentType::CPU,
        AgentType::CPU,
    );
    println!("bye");
}
