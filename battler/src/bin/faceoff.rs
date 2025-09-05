use std::fmt::Debug;
use std::path::PathBuf;
use std::time::Duration;

use battler::{BINARY_DIRECTORY, create_log_dir, do_battle, prepare_subprocess, read_corpus};
use chrono::Utc;
use clap::Parser;
use santorini_core::gods::GodName;
use santorini_core::player::Player;

const DEFAULT_DURATION_SECS: f32 = 1.0;

#[derive(Parser, Debug)]
struct FaceoffArgs {
    #[arg(short = 'e', long)]
    engine1: String,
    #[arg(short = 'E', long)]
    engine2: String,

    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,

    #[arg(short = 'g', long)]
    #[arg(short, long)]
    god: Option<GodName>,
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
    create_log_dir();

    let args = FaceoffArgs::parse();
    let now = Utc::now().format("%Y-%m-%d_%H-%M-%S").to_string();

    println!("Game ts: {}", now);
    let game_name = format!(
        "faceoff-{}-{}-{}s-{}",
        now, args.engine1, args.engine2, args.secs
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
        dbg!(&position);
        if !position.is_enabled {
            println!("Skipping position");
            continue;
        }

        let mut state = position.state.clone();
        if let Some(god_name) = args.god {
            state.gods[0] = god_name.to_power();
            state.gods[1] = god_name.to_power();
        }

        {
            let battle_result_1 = do_battle(
                &state,
                &mut c1,
                &mut c2,
                Duration::from_secs_f32(args.secs),
                true,
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
                &state,
                &mut c2,
                &mut c1,
                Duration::from_secs_f32(args.secs),
                true,
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
