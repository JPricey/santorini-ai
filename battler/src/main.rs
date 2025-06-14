use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use santorini_core::board::{FullGameState, Player};
use santorini_core::fen::game_state_to_fen;
use santorini_core::gods::GodName;
use santorini_core::search::BestMoveTrigger;
use santorini_core::uci_types::{BestMoveOutput, EngineOutput};

struct EngineSubprocess {
    engine_name: String,
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    receiver: Receiver<String>,
}

fn prepare_subprocess(engine_path: &str) -> EngineSubprocess {
    let mut child = Command::new(engine_path)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");

    let (child_msg_tx, child_msg_rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    child_msg_tx.send(line).unwrap();
                }
                Err(e) => {
                    eprintln!("Error reading line: {}", e);
                    break;
                }
            }
        }
    });

    let end_at = Instant::now() + Duration::from_secs(10);
    loop {
        let now = Instant::now();
        if now >= end_at {
            panic!("Waited too long to spin up child");
        }

        let timeout = end_at - now;

        match child_msg_rx.recv_timeout(timeout) {
            Ok(msg) => {
                // println!("I got a message {}", msg);
                let parsed_msg: EngineOutput = serde_json::from_str(&msg).unwrap();
                match parsed_msg {
                    EngineOutput::Started(_) => {
                        // println!("Started!");
                        break;
                    }
                    _ => {
                        println!("received non-ready message. Still waiting...")
                    }
                }
                println!("parsed msg: {:?}", parsed_msg);
            }
            Err(e) => {
                panic!("Error while waiting for child: {:?}", e);
            }
        }
    }

    EngineSubprocess {
        engine_name: engine_path.to_owned(),
        child,
        stdin,
        receiver: child_msg_rx,
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct BattleResult {
    winning_player: Player,
    depth: usize,
    // history
}

fn do_battle<'a>(
    root: &FullGameState,
    c1: &'a mut EngineSubprocess,
    c2: &'a mut EngineSubprocess,
    thinking_time_secs: f32,
) -> BattleResult {
    let mut depth = 0;
    let mut current_state = root.clone();

    root.print_to_console();
    println!();

    loop {
        let (engine, other) = match current_state.board.current_player {
            Player::One => (&mut *c1, &mut *c2),
            Player::Two => (&mut *c2, &mut *c1),
        };

        // Just incase
        writeln!(other.stdin, "stop").expect("Failed to write to stdin");

        let state_string = game_state_to_fen(&current_state);
        writeln!(engine.stdin, "set_position {}", state_string).expect("Failed to write to stdin");

        let started_at = Instant::now();
        let end_at = started_at + Duration::from_secs_f32(thinking_time_secs);
        let mut saved_best_move: Option<BestMoveOutput> = None;

        loop {
            let now = Instant::now();
            if now >= end_at {
                break;
            }

            let timeout = end_at - now;
            match engine.receiver.recv_timeout(timeout) {
                Ok(msg) => {
                    let parsed_msg: EngineOutput = serde_json::from_str(&msg).unwrap();
                    match parsed_msg {
                        EngineOutput::BestMove(best_move) => {
                            if best_move.start_state != current_state {
                                // println!("Message for wrong state");
                                continue;
                            }
                            let is_eol = best_move.trigger == BestMoveTrigger::EndOfLine;
                            saved_best_move = Some(best_move);
                            if is_eol {
                                println!("Mate found, ending early");
                                break;
                            }
                        }
                        _ => {
                            eprintln!("Unexpected message: {:?}", parsed_msg);
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // eprintln!("timeout reached");
                    break;
                }
                Err(e) => {
                    eprintln!("Error receiving message: {:?}", e);
                }
            }
        }

        writeln!(engine.stdin, "stop").expect("Failed to write to stdin");

        depth += 1;

        let saved_best_move = saved_best_move.expect("Expected engine to output at least 1 move");

        current_state = saved_best_move.next_state.clone();

        let current_god = match saved_best_move.start_state.board.current_player {
            Player::One => saved_best_move.start_state.p1_god,
            Player::Two => saved_best_move.start_state.p2_god,
        };

        current_state.print_to_console();
        println!(
            "({}) Made move for Player {:?} [{:?}]: {:?} | d: {} s: {}",
            engine.engine_name,
            saved_best_move.start_state.board.current_player,
            current_god.god_name,
            saved_best_move.meta.actions,
            saved_best_move.meta.calculated_depth,
            saved_best_move.meta.score
        );

        println!();

        let winner = current_state.board.get_winner();
        if let Some(winner) = winner {
            return BattleResult {
                winning_player: winner,
                depth,
            };
        }
    }
}

struct BattlerArgs {}

fn main() {
    let mut c1 = prepare_subprocess("./all_versions/v3");
    let mut c2 = prepare_subprocess("./all_versions/v3");

    let mut root = FullGameState::new_basic_state_mortals();
    root.p1_god = GodName::Mortal.to_power();
    root.p2_god = GodName::Pan.to_power();
    let outcome = do_battle(&root, &mut c1, &mut c2, 10.0);
    println!("Game has ended {:?}", outcome);
}
