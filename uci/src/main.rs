use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use santorini_core::{
    board::{get_next_states_interactive, BoardState, FullGameState},
    engine::EngineThreadWrapper,
    gods::{PartialAction, ALL_GODS_BY_ID},
    search::NewBestMove,
    uci_types::{
        BestMoveMeta, BestMoveOutput, EngineOutput, NextMovesOutput, NextStateOutput, StartedOutput,
    },
};

fn find_action_path(
    start_state: &FullGameState,
    destination_state: &FullGameState,
) -> Option<Vec<PartialAction>> {
    let all_child_states = get_next_states_interactive(&start_state);
    for full_child in all_child_states {
        if &full_child.result_state == destination_state {
            return Some(full_child.actions);
        }
    }
    None
}

fn try_emit_message(message: &EngineOutput) {
    match serde_json::to_string(message) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing message: {}", e),
    }
}

fn handle_command(
    engine: &mut EngineThreadWrapper,
    raw_cmd: &str,
) -> Result<Option<String>, String> {
    let mut parts: Vec<String> = raw_cmd
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(&str::to_owned)
        .collect();
    if parts.is_empty() {
        thread::sleep(Duration::from_millis(100));
        return Err("Command was empty".to_owned());
    }
    let command = parts.remove(0);

    match &command as &str {
        "quit" => {
            std::process::exit(0);
        }
        "ping" => Ok(Some("pong".to_owned())),
        "stop" => match engine.stop() {
            Ok(best_move) => Err(format!("Stopping with best move: {:?}", best_move.state)),
            Err(e) => Err(e),
        },
        "set_position" => {
            if parts.len() != 1 {
                return Err("set_position should be followed by a single FEN string".to_owned());
            }

            let fen = parts.remove(0);
            let state =
                FullGameState::try_from(&fen).map_err(|e| format!("Error parsing FEN: {}", e))?;

            let _ = engine.stop();
            let start_time = Instant::now();
            let state_2 = state.clone();

            let callback = Arc::new(move |new_best_move: NewBestMove| {
                let Some(action_path) = find_action_path(&state_2, &new_best_move.state) else {
                    eprintln!(
                        "Found new best move but couldn't resolve path: {:?} -> {:?}",
                        state_2, new_best_move.state
                    );
                    return;
                };

                let output = EngineOutput::BestMove(BestMoveOutput {
                    start_state: state_2.clone(),
                    next_state: new_best_move.state.clone(),
                    trigger: new_best_move.trigger,
                    meta: BestMoveMeta {
                        score: new_best_move.score,
                        calculated_depth: new_best_move.depth,
                        elapsed_seconds: start_time.elapsed().as_secs_f32(),
                        actions: action_path,
                    },
                });

                try_emit_message(&output);
            });
            engine.start_search(&state, Some(callback))?;
            Ok(None)
        }
        "next_moves" => {
            if parts.len() != 1 {
                return Err("next_moves should be followed by a single FEN string".to_owned());
            }

            let fen = parts.remove(0);

            let state: FullGameState =
                FullGameState::try_from(&fen).map_err(|e| format!("Error parsing FEN: {}", e))?;

            let child_states = get_next_states_interactive(&state);

            let output = EngineOutput::NextMoves(NextMovesOutput {
                start_state: state.clone(),
                next_states: child_states
                    .into_iter()
                    .map(|full_choice| NextStateOutput {
                        next_state: full_choice.result_state,
                        actions: full_choice.actions,
                    })
                    .collect(),
            });

            serde_json::to_string(&output)
                .map(|v| Some(v))
                .map_err(|e| format!("{:?}", e))
        }
        _ => Err(format!("Skipping unknown command: {}", raw_cmd)),
    }
}

fn main() {
    let (cli_command_sender, cli_command_receiver) = mpsc::channel();

    let _io_thread = thread::spawn(move || {
        loop {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            cli_command_sender.send(input).unwrap();
        }
    });

    let mut engine = EngineThreadWrapper::new();

    try_emit_message(&EngineOutput::Started(StartedOutput {}));

    loop {
        let raw_cmd = cli_command_receiver.recv().unwrap();
        if raw_cmd.trim().is_empty() {
            println!("empty command");
            thread::sleep(Duration::from_secs(1));
            continue;
        }
        match handle_command(&mut engine, &raw_cmd) {
            Ok(Some(response)) => {
                println!("{}", response);
            }
            Ok(None) => {
                // No response to print
            }
            Err(err) => {
                eprintln!("Error: {}", err.trim());
            }
        }
    }

    // io_thread.join().unwrap();
    // eprintln!("bye");
}
