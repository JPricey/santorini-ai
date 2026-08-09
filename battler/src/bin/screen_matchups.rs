use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Instant,
};

use clap::Parser;
use santorini_core::{
    board::FullGameState,
    gods::GodName,
    matchup::{Matchup, MatchupArgs},
    player::Player,
    search::{
        Heuristic, SearchContext, WINNING_SCORE_BUFFER, get_win_reached_search_terminator,
        negamax_search,
    },
    search_terminators::DynamicNodesVisitedSearchTerminator,
    transposition_table::TranspositionTable,
    utils::timestamp_string,
};
use serde::{Deserialize, Serialize};

const DEFAULT_OUT_PATH: &str = "data/matchup_evals.csv";

/// Screen every matchup for balance by self-playing the opening.
#[derive(Parser, Debug)]
struct Args {
    #[arg(short = 'p', long, default_value_t = 8)]
    plies: usize,

    #[arg(short = 'n', long, default_value_t = 200_000)]
    nodes: usize,

    #[arg(short = 'o', long, default_value = DEFAULT_OUT_PATH)]
    out: PathBuf,

    /// Worker threads. Each holds its own transposition table (~160MB).
    #[arg(short = 't', long)]
    threads: Option<usize>,

    #[command(flatten)]
    matchups: MatchupArgs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MatchupEval {
    god1: GodName,
    god2: GodName,
    nodes: usize,
    plies_played: usize,
    // Equal nodes buys different depths per god; shallow rows are low confidence.
    min_depth_reached: usize,
    mean_depth_reached: f32,
    // Evals are all from player one's perspective.
    final_eval: Heuristic,
    mean_eval: f32,
    max_abs_eval: Heuristic,
    // A mate score appeared, so the opening already decided the matchup.
    is_decisive: bool,
    ended_early: bool,
    evals: String,
    seconds: f32,
}

fn screen_matchup(
    tt: &mut TranspositionTable,
    matchup: &Matchup,
    plies: usize,
    nodes: usize,
) -> MatchupEval {
    // Start cold so one matchup's entries and history don't leak into the next.
    tt.reset();

    let started_at = Instant::now();
    let mut state = FullGameState::new_for_matchup(matchup);
    let mut evals: Vec<Heuristic> = Vec::with_capacity(plies);
    let mut depths: Vec<usize> = Vec::with_capacity(plies);
    let mut ended_early = false;

    for _ in 0..plies {
        if state.board.get_winner().is_some() {
            ended_early = true;
            break;
        }

        let mut context = SearchContext::new(tt, DynamicNodesVisitedSearchTerminator::new(nodes));
        let result = negamax_search(
            &mut context,
            state.clone(),
            get_win_reached_search_terminator(),
        );

        let Some(best_move) = result.best_move else {
            ended_early = true;
            break;
        };

        // negamax scores the side to move, so flip to keep plies comparable.
        let player_one_eval = match state.board.current_player {
            Player::One => best_move.score,
            Player::Two => -best_move.score,
        };
        evals.push(player_one_eval);
        depths.push(result.last_fully_completed_depth);
        state = best_move.child_state.clone();
    }

    let plies_played = evals.len();
    let mean_eval = if plies_played == 0 {
        0.0
    } else {
        evals.iter().map(|e| *e as f32).sum::<f32>() / plies_played as f32
    };
    let mean_depth_reached = if plies_played == 0 {
        0.0
    } else {
        depths.iter().map(|d| *d as f32).sum::<f32>() / plies_played as f32
    };

    MatchupEval {
        god1: matchup.gods[0],
        god2: matchup.gods[1],
        nodes,
        plies_played,
        min_depth_reached: depths.iter().copied().min().unwrap_or(0),
        mean_depth_reached,
        final_eval: evals.last().copied().unwrap_or(0),
        mean_eval,
        max_abs_eval: evals.iter().map(|e| e.abs()).max().unwrap_or(0),
        is_decisive: evals.iter().any(|e| e.abs() >= WINNING_SCORE_BUFFER),
        ended_early,
        evals: evals
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(";"),
        seconds: started_at.elapsed().as_secs_f32(),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mut all_matchups = args.matchups.to_selector().get_all();
    // Workers pop from the end, so reverse to run in god id order.
    all_matchups.sort();
    all_matchups.reverse();
    let total = all_matchups.len();

    let num_workers = args.threads.unwrap_or_else(|| (num_cpus::get() / 2).max(1));
    eprintln!(
        "{} Screening {} ordered matchups, {} plies @ {} nodes, {} workers -> {}",
        timestamp_string(),
        total,
        args.plies,
        args.nodes,
        num_workers,
        args.out.display()
    );

    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let queue = Arc::new(Mutex::new(all_matchups));
    let (tx, rx) = mpsc::channel::<Option<MatchupEval>>();

    for _ in 0..num_workers {
        let tx = tx.clone();
        let queue = Arc::clone(&queue);
        let (plies, nodes) = (args.plies, args.nodes);
        std::thread::spawn(move || {
            let mut tt = TranspositionTable::new();
            loop {
                let Some(matchup) = queue.lock().unwrap().pop() else {
                    break;
                };
                let record = screen_matchup(&mut tt, &matchup, plies, nodes);
                if tx.send(Some(record)).is_err() {
                    break;
                }
            }
            let _ = tx.send(None);
        });
    }
    drop(tx);

    // Flushed per row so a cancelled run keeps what it finished.
    let mut writer = csv::Writer::from_path(&args.out)?;
    let mut done_workers = 0;
    let mut completed = 0;

    while done_workers < num_workers {
        match rx.recv()? {
            None => done_workers += 1,
            Some(record) => {
                completed += 1;
                writer.serialize(&record)?;
                writer.flush()?;

                eprintln!(
                    "{} [{}/{}] {:?} v {:?}: final {:+} mean {:+.0} max|{}| d{:.0} {}{}({:.1}s)",
                    timestamp_string(),
                    completed,
                    total,
                    record.god1,
                    record.god2,
                    record.final_eval,
                    record.mean_eval,
                    record.max_abs_eval,
                    record.mean_depth_reached,
                    if record.is_decisive { "DECISIVE " } else { "" },
                    if record.ended_early { "ended-early " } else { "" },
                    record.seconds,
                );
            }
        }
    }

    eprintln!("{} Wrote {} rows to {}", timestamp_string(), completed, args.out.display());
    Ok(())
}

// cargo run -p battler --bin screen_matchups -r
// cargo run -p battler --bin screen_matchups -r -- -p 8 -n 200000
// cargo run -p battler --bin screen_matchups -r -- --gods demeter pan athena apollo
