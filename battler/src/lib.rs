use std::process::{Child, ChildStdin, Command, Stdio};

use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};

use csv::Writer;
use santorini_core::board::FullGameState;
use santorini_core::fen::game_state_to_fen;
use santorini_core::gods::GodName;
use santorini_core::matchup::Matchup;
use santorini_core::player::Player;
use santorini_core::search::BestMoveTrigger;
use santorini_core::utils::timestamp_string;
use serde::{Deserialize, Serialize};

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use santorini_core::uci_types::{BestMoveOutput, EngineOutput};

const CORPUS_FILE_PATH: &str = "data/corpus.yaml";
pub const BINARY_DIRECTORY: &str = "all_versions";

fn _true_value() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StartingPosition {
    pub name: String,
    pub state: FullGameState,
    pub notes: String,
    #[serde(default = "_true_value")]
    pub is_enabled: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Corpus {
    pub positions: Vec<StartingPosition>,
}

pub fn write_corpus(corpus: &Corpus) {
    let toml_string = serde_yaml::to_string(corpus).expect("Failed to serialize corpus");
    std::fs::write(CORPUS_FILE_PATH, toml_string).expect("Failed to write corpus to file");
}

pub fn read_corpus() -> Corpus {
    let toml_string =
        std::fs::read_to_string(CORPUS_FILE_PATH).expect("Failed to read corpus file");
    serde_yaml::from_str(&toml_string).expect("Failed to deserialize corpus")
}

pub fn create_log_dir() {
    let path = std::env::current_dir()
        .expect("Failed to get current directory")
        .join("logs");
    std::fs::create_dir_all(&path).expect("Failed to create logs directory");
}

pub fn create_tmp_dir() {
    let path = std::env::current_dir()
        .expect("Failed to get current directory")
        .join("tmp");
    std::fs::create_dir_all(&path).expect("Failed to create logs directory");
}

pub struct EngineSubprocess {
    pub engine_name: String,
    #[allow(dead_code)]
    pub child: Child,
    pub stdin: ChildStdin,
    pub receiver: Receiver<String>,
}

pub fn prepare_subprocess(log_path: &PathBuf, engine_path: &PathBuf) -> EngineSubprocess {
    let log_dir = PathBuf::from("logs");
    std::fs::create_dir_all(&log_dir).unwrap();
    let log_path = log_dir.join(log_path);

    let stderr_file = std::fs::File::create(log_path).expect("Failed to create error log file");

    eprintln!("Spawning: {}", engine_path.display());

    let mut child = Command::new(engine_path)
        // .env("RUST_BACKTRACE", "full")
        .stdin(Stdio::piped())
        .stderr(std::process::Stdio::from(stderr_file))
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
                    if let Err(err) = child_msg_tx.send(line.clone()) {
                        eprintln!("Error sending line: {} {}", line, err);
                        break;
                    }
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
        engine_name: engine_path.to_str().unwrap().to_owned(),
        child,
        stdin,
        receiver: child_msg_rx,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BattleResult {
    pub god1: GodName,
    pub engine1: String,
    pub god2: GodName,
    pub engine2: String,

    pub winning_player: Player,
    pub moves_made: usize,
}

impl BattleResult {
    pub fn get_pretty_description(&self) -> String {
        let winner_str = match self.winning_player {
            Player::One => format!(
                "Won by player 1 ({} {}) after {} moves",
                self.god1, self.engine1, self.moves_made
            ),
            Player::Two => format!(
                "Won by player 2 ({} {}) after {} moves",
                self.god2, self.engine2, self.moves_made
            ),
        };

        format!(
            "{:?} ({}) v {:?} ({}) - {winner_str}",
            self.god1, self.engine1, self.god2, self.engine2
        )
    }
}

pub fn write_results_to_csv(results: &[BattleResult], path: &PathBuf) -> std::io::Result<()> {
    let mut wtr = Writer::from_path(path)?;
    for result in results {
        wtr.serialize(result)?;
    }
    wtr.flush()?;
    Ok(())
}

pub fn do_battle<'a>(
    start_state: &FullGameState,
    c1: &'a mut EngineSubprocess,
    c2: &'a mut EngineSubprocess,
    per_turn_duration: Duration,
    is_printing: bool,
) -> BattleResult {
    let mut moves_made = 0;
    let mut current_state = start_state.clone();

    if is_printing {
        start_state.print_to_console();
        println!();
    }

    loop {
        let (engine, other) = match current_state.board.current_player {
            Player::One => (&mut *c1, &mut *c2),
            Player::Two => (&mut *c2, &mut *c1),
        };

        let state_string = game_state_to_fen(&current_state);
        if is_printing {
            eprintln!(
                "{}: setting position {}",
                timestamp_string(),
                engine.engine_name
            );
        }
        writeln!(engine.stdin, "set_position {}", state_string).expect("Failed to write to stdin");
        writeln!(other.stdin, "set_position {}", state_string).expect("Failed to write to stdin");

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
                                    if is_printing {
                                        println!("Mate found, ending early");
                                    }
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
                    panic!("Error receiving message: {:?}", e);
                }
            }
        }

        // eprintln!("{}: stopping {}", timestamp_string(), engine.engine_name);
        // writeln!(engine.stdin, "stop").expect("Failed to write to stdin");

        moves_made += 1;

        let saved_best_move = saved_best_move.expect("Expected engine to output at least 1 move");

        current_state = saved_best_move.next_state.clone();

        let current_god = saved_best_move.start_state.get_active_god();

        if is_printing {
            println!(
                "({}) Made move for Player {:?} [{:?}]: {:?} | depth: {} score: {}, visited: {:?} secs: {:.04}",
                engine.engine_name,
                saved_best_move.start_state.board.current_player,
                current_god.god_name,
                saved_best_move.meta.actions,
                saved_best_move.meta.calculated_depth,
                saved_best_move.meta.score,
                saved_best_move.meta.nodes_visited,
                started_at.elapsed().as_secs_f32()
            );
            current_state.print_to_console();
            println!();
        }

        let winner = current_state.board.get_winner();
        if let Some(winner) = winner {
            writeln!(c1.stdin, "stop").expect("Failed to write to stdin");
            writeln!(c2.stdin, "stop").expect("Failed to write to stdin");

            return BattleResult {
                god1: current_state.gods[0].god_name,
                engine1: c1.engine_name.clone(),
                god2: current_state.gods[1].god_name,
                engine2: c2.engine_name.clone(),
                winning_player: winner,
                moves_made,
            };
        }
    }
}

pub fn battling_worker_thread<const RUN_BOTH_SIDES: bool>(
    worker_idx: String,
    matchups_queue: Arc<Mutex<Vec<Matchup>>>,
    engine1: &PathBuf,
    engine2: &PathBuf,
    duration: Duration,
    result_channel: mpsc::Sender<WorkerMessage>,
) {
    let now_str = timestamp_string();
    let mut c1 = prepare_subprocess(
        &PathBuf::from(format!(
            "compare-{worker_idx}-{}-{}.log",
            now_str,
            engine1.display()
        )),
        &PathBuf::new().join(BINARY_DIRECTORY).join(engine1),
    );
    let mut c2 = prepare_subprocess(
        &PathBuf::from(format!(
            "compare-{worker_idx}-{}-{}.log",
            now_str,
            engine2.display()
        )),
        &PathBuf::new().join(BINARY_DIRECTORY).join(engine2),
    );

    loop {
        let matchup = {
            let mut queue = matchups_queue.lock().unwrap();
            queue.pop()
        };

        match matchup {
            Some(matchup) => {
                let start_state = FullGameState::new_for_matchup(&matchup);

                if RUN_BOTH_SIDES {
                    let result1 = do_battle(&start_state, &mut c1, &mut c2, duration, false);
                    let result2 = do_battle(&start_state, &mut c2, &mut c1, duration, false);

                    result_channel
                        .send(WorkerMessage::BattleResultPair((result1, result2)))
                        .unwrap();
                } else {
                    let result = do_battle(&start_state, &mut c1, &mut c2, duration, false);
                    result_channel
                        .send(WorkerMessage::BattleResult(result))
                        .unwrap();
                }
            }
            None => {
                break;
            }
        }
    }

    writeln!(c1.stdin, "quit").expect("Failed to write to stdin");
    writeln!(c2.stdin, "quit").expect("Failed to write to stdin");
}

pub enum WorkerMessage {
    BattleResult(BattleResult),
    BattleResultPair((BattleResult, BattleResult)),
    Done,
}
