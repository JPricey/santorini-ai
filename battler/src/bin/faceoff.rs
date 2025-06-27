use std::fmt::Debug;
use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use battler::{BINARY_DIRECTORY, EngineSubprocess, prepare_subprocess, read_corpus};
use chrono::Utc;
use clap::Parser;
use santorini_core::board::FullGameState;
use santorini_core::fen::game_state_to_fen;
use santorini_core::player::Player;
use santorini_core::search::BestMoveTrigger;
use santorini_core::uci_types::{BestMoveOutput, EngineOutput};

const DEFAULT_DURATION_SECS: f32 = 2.0;

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct BattleResult {
    winning_player: Player,
    depth: usize,
    // history?
}

fn do_battle<'a>(
    root: &FullGameState,
    c1: &'a mut EngineSubprocess,
    c2: &'a mut EngineSubprocess,
    per_turn_duration: Duration,
) -> BattleResult {
    let mut depth = 0;
    let mut current_state = root.clone();

    root.print_to_console();
    println!();

    loop {
        let engine = match current_state.board.current_player {
            Player::One => &mut *c1,
            Player::Two => &mut *c2,
        };

        let state_string = game_state_to_fen(&current_state);
        writeln!(engine.stdin, "set_position {}", state_string).expect("Failed to write to stdin");

        let started_at = Instant::now();
        let end_at = started_at + per_turn_duration;
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
                            saved_best_move = Some(best_move.clone());
                            match best_move.trigger {
                                BestMoveTrigger::StopFlag => {
                                    break;
                                }
                                BestMoveTrigger::EndOfLine => {
                                    println!("Mate found, ending early");
                                    break;
                                }
                                BestMoveTrigger::Improvement | BestMoveTrigger::Saved => (),
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

        let current_god = saved_best_move.start_state.get_active_god();

        println!(
            "({}) Made move for Player {:?} [{:?}]: {:?} | depth: {} score: {} secs: {:.04}",
            engine.engine_name,
            saved_best_move.start_state.board.current_player,
            current_god.god_name,
            saved_best_move.meta.actions,
            saved_best_move.meta.calculated_depth,
            saved_best_move.meta.score,
            started_at.elapsed().as_secs_f32()
        );
        current_state.print_to_console();

        println!();

        let winner = current_state.board.get_winner();
        if let Some(winner) = winner {
            writeln!(c1.stdin, "stop").expect("Failed to write to stdin");
            writeln!(c2.stdin, "stop").expect("Failed to write to stdin");

            return BattleResult {
                winning_player: winner,
                depth,
            };
        }
    }
}

#[derive(Parser, Debug)]
struct FaceoffArgs {
    #[arg(short = 'e', long)]
    engine1: String,
    #[arg(short = 'E', long)]
    engine2: String,

    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,
}

struct SidedPosition {
    name: String,
    player: Player,
}

impl Debug for SidedPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{:?}", self.name, self.player)
    }
}

fn main() {
    let args = FaceoffArgs::parse();
    let now = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    println!("Game ts: {}", now);
    let game_name = format!(
        "faceoff-{}-{}-{}s-{}",
        args.engine1, args.engine2, args.secs, now
    );

    let c1_logs_name = format!("{}-{}", game_name, args.engine1);
    let c2_logs_name = format!("{}-{}", game_name, args.engine2);

    let mut c1 = prepare_subprocess(
        &PathBuf::new().join("logs").join(c1_logs_name),
        &PathBuf::new().join(BINARY_DIRECTORY).join(args.engine1),
    );
    let mut c2 = prepare_subprocess(
        &PathBuf::new().join("logs").join(c2_logs_name),
        &PathBuf::new().join(BINARY_DIRECTORY).join(args.engine2),
    );

    let corpus = read_corpus();

    let mut e1_wins: Vec<SidedPosition> = Vec::new();
    let mut e2_wins: Vec<SidedPosition> = Vec::new();

    for position in corpus.positions {
        {
            let battle_result_1 = do_battle(
                &position.state,
                &mut c1,
                &mut c2,
                Duration::from_secs_f32(args.secs),
            );
            if battle_result_1.winning_player == Player::One {
                e1_wins.push(SidedPosition {
                    name: position.name.clone(),
                    player: Player::One,
                });
            } else {
                e2_wins.push(SidedPosition {
                    name: position.name.clone(),
                    player: Player::Two,
                });
            }
        }
        println!(
            "Current score. E1: {}. E2: {}",
            e1_wins.len(),
            e2_wins.len(),
        );

        {
            let battle_result_2 = do_battle(
                &position.state,
                &mut c2,
                &mut c1,
                Duration::from_secs_f32(args.secs),
            );
            if battle_result_2.winning_player == Player::One {
                e2_wins.push(SidedPosition {
                    name: position.name.clone(),
                    player: Player::One,
                });
            } else {
                e1_wins.push(SidedPosition {
                    name: position.name.clone(),
                    player: Player::Two,
                });
            }
        }
        println!(
            "Current score. E1: {}. E2: {}",
            e1_wins.len(),
            e2_wins.len(),
        );
    }

    println!("E1 wins: {:?}", e1_wins);
    println!("E2 wins: {:?}", e2_wins);
    println!(
        "Overall, e1_wins: {} e2_wins: {}",
        e1_wins.len(),
        e2_wins.len()
    );

    let _ = c1.child.kill();
    let _ = c2.child.kill();
}

// cargo run -p battler --bin faceoff -- -e v1 -E v18
