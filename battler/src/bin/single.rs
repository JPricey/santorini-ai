use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};
use std::{fs, thread};

use chrono::Utc;
use clap::Parser;
use santorini_core::board::FullGameState;
use santorini_core::fen::game_state_to_fen;
use santorini_core::gods::GodName;
use santorini_core::player::Player;
use santorini_core::search::BestMoveTrigger;
use santorini_core::uci_types::{BestMoveOutput, EngineOutput};

const BINARY_DIRECTORY: &str = "all_versions";
const DEFAULT_DURATION_SECS: f32 = 5.0;

struct EngineSubprocess {
    engine_name: String,
    #[allow(dead_code)]
    child: Child,
    stdin: ChildStdin,
    receiver: Receiver<String>,
}

fn prepare_subprocess(log_path: &Path, engine_path: &str) -> EngineSubprocess {
    let stderr_file = std::fs::File::create(log_path).expect("Failed to create error log file");

    let mut child = Command::new(engine_path)
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
    conf1: BattlingPlayerConfig,
    conf2: BattlingPlayerConfig,
) -> BattleResult {
    let mut depth = 0;
    let mut current_state = root.clone();

    root.print_to_console();
    println!();

    loop {
        let (engine, other, conf) = match current_state.board.current_player {
            Player::One => (&mut *c1, &mut *c2, &conf1),
            Player::Two => (&mut *c2, &mut *c1, &conf2),
        };

        // Just incase
        writeln!(other.stdin, "stop").expect("Failed to write to stdin");

        let state_string = game_state_to_fen(&current_state);
        writeln!(engine.stdin, "set_position {}", state_string).expect("Failed to write to stdin");

        let started_at = Instant::now();
        let end_at = started_at + conf.duration_per_turn;
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

        let current_god = saved_best_move.start_state.get_active_god();

        println!(
            "({}) Made move for Player {:?} [{:?}]: {:?} | depth: {} score: {}",
            engine.engine_name,
            saved_best_move.start_state.board.current_player,
            current_god.god_name,
            saved_best_move.meta.actions,
            saved_best_move.meta.calculated_depth,
            saved_best_move.meta.score
        );
        current_state.print_to_console();

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

fn _get_latest_updated_version() -> String {
    let entries = fs::read_dir(BINARY_DIRECTORY).unwrap();

    entries
        .into_iter()
        .filter_map(|e| match e {
            Ok(entry) => {
                let path = entry.path();
                if path.is_file() {
                    let metadata = fs::metadata(&path).ok()?;
                    let modified_time = metadata.modified().ok()?;
                    Some((path, modified_time))
                } else {
                    None
                }
            }
            Err(_) => None,
        })
        .max_by_key(|e| e.1)
        .map(|e| e.0)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned()
}

fn _log_name(ts_str: &str, player: Player) -> PathBuf {
    let mut path = std::env::current_dir().expect("Failed to get current directory");
    path.push("logs");
    std::fs::create_dir_all(&path).expect("Failed to create logs directory");

    let filename = format!("game_{}_player_{:?}.log", ts_str, player).to_lowercase();
    path.push(filename);
    path
}

#[derive(Parser, Debug)]
struct BattlerCliArgs {
    #[arg(short = 'g', long)]
    #[arg(short, long)]
    god: Option<GodName>,
    #[arg(short = 'G', long)]
    god2: Option<GodName>,

    #[arg(short = 'e', long)]
    engine: Option<String>,
    #[arg(short = 'E', long)]
    engine2: Option<String>,

    #[arg(short = 's', long)]
    secs: Option<f32>,
    #[arg(short = 'S', long)]
    secs2: Option<f32>,

    #[arg(short = 'b', long)]
    board: Option<String>,
}

#[derive(Debug, Clone)]
struct BattlingPlayerConfig {
    god: GodName,
    engine_name: String,
    duration_per_turn: Duration,
}

fn _resolve_engine(name: Option<&str>) -> String {
    match name.as_deref() {
        Some("latest") | None => _get_latest_updated_version(),
        Some(path) => format!("all_versions/{}", path),
    }
}

fn massage_inputs(args: &BattlerCliArgs) -> (BattlingPlayerConfig, BattlingPlayerConfig) {
    let (god1, god2) = match (args.god, args.god2) {
        (Some(g1), Some(g2)) => (g1, g2),
        (Some(g1), None) => (g1, g1),
        (None, Some(g2)) => (GodName::Mortal, g2),
        (None, None) => (GodName::Mortal, GodName::Mortal),
    };

    let (engine1, engine2) = match (&args.engine, &args.engine2) {
        (Some(e1), None) => {
            let engine = _resolve_engine(Some(e1));
            (engine.clone(), engine.clone())
        }
        (a, b) => (_resolve_engine(a.as_deref()), _resolve_engine(b.as_deref())),
    };

    let (s1, s2) = match (args.secs, args.secs2) {
        (Some(g1), Some(g2)) => (g1, g2),
        (Some(g1), None) => (g1, g1),
        (None, Some(g2)) => (DEFAULT_DURATION_SECS, g2),
        (None, None) => (DEFAULT_DURATION_SECS, DEFAULT_DURATION_SECS),
    };

    (
        BattlingPlayerConfig {
            god: god1,
            engine_name: engine1,
            duration_per_turn: Duration::from_secs_f32(s1),
        },
        BattlingPlayerConfig {
            god: god2,
            engine_name: engine2,
            duration_per_turn: Duration::from_secs_f32(s2),
        },
    )
}

fn main() {
    let args = BattlerCliArgs::parse();
    let (mut conf1, mut conf2) = massage_inputs(&args);

    let state = match args.board {
        Some(fen) => match FullGameState::try_from(&fen) {
            Ok(state) => {
                conf1.god = state.gods[0].god_name;
                conf2.god = state.gods[1].god_name;
                state
            }
            Err(e) => {
                eprintln!("Error parsing FEN: {}", e);
                return;
            }
        },
        None => {
            let mut state = FullGameState::new_basic_state_mortals();
            state.gods = [conf1.god.to_power(), conf2.god.to_power()];

            state
        }
    };

    if state.board.get_winner().is_some() {
        println!("Game is already over. Quitting");
        return;
    }

    let now = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    println!("Game ts: {}", now);

    println!("Configs for Player One: {:?}", conf1);
    println!("Configs for Player Two: {:?}", conf2);

    let mut c1 = prepare_subprocess(&_log_name(&now, Player::One), &conf1.engine_name);
    let mut c2 = prepare_subprocess(&_log_name(&now, Player::Two), &conf2.engine_name);

    let outcome = do_battle(&state, &mut c1, &mut c2, conf1, conf2);
    println!("Game has ended {:?}", outcome);

    let _ = c1.child.kill();
    let _ = c2.child.kill();
}
