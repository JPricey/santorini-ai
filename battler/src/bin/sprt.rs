use std::{
    collections::HashMap,
    fs::OpenOptions,
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

use battler::{BattleResult, WorkerMessage, battling_worker_thread};
use clap::Parser;
use santorini_core::{
    gods::GodName,
    matchup::{Matchup, MatchupArgs},
    player::Player,
    utils::timestamp_string,
};
use serde::{Deserialize, Serialize};

const DEFAULT_DB_PATH: &str = "data/sprt_history.csv";
const DEFAULT_DURATION_SECS: f32 = 0.5;

/// SPRT engine comparison.
///
/// Plays color-swapped pairs per matchup like compare_engines, but:
/// - Scores pairs pentanomial-style (candidate sweep = 1, split = 0.5,
///   baseline sweep = 0) and runs a GSPRT, stopping early once the log
///   likelihood ratio crosses a bound.
/// - Records every pair outcome to a persistent CSV database.
/// - Uses that database to order matchups by historical *sweep rate*
///   (engine-decided rate). In a color-swapped pair a split always means the
///   same god won both games - position-decided, zero engine signal - so
///   matchups that historically always split ("known forced wins") are
///   skipped, and the most engine-sensitive (even) matchups run first.
#[derive(Parser, Debug)]
struct Args {
    /// Baseline engine (name under all_versions/)
    #[arg(short = 'e', long)]
    engine_base: String,
    /// Candidate engine (name under all_versions/)
    #[arg(short = 'E', long)]
    engine_cand: String,

    #[arg(short = 's', long, default_value_t = DEFAULT_DURATION_SECS)]
    secs: f32,

    /// H0: candidate is at most this many pair-elo stronger
    #[arg(long, default_value_t = 0.0)]
    elo0: f64,
    /// H1: candidate is at least this many pair-elo stronger
    #[arg(long, default_value_t = 10.0)]
    elo1: f64,
    #[arg(long, default_value_t = 0.05)]
    alpha: f64,
    #[arg(long, default_value_t = 0.05)]
    beta: f64,

    /// Pair-outcome database (appended to; also drives matchup ordering)
    #[arg(long, default_value = DEFAULT_DB_PATH)]
    db: PathBuf,

    /// Also run matchups the database says are forced (never swept)
    #[arg(long, default_value_t = false)]
    include_forced: bool,

    /// Only run mirror matchups - a quick, high-signal screen
    #[arg(long, default_value_t = false)]
    mirrors_only: bool,

    /// Stop after this many pairs even without a verdict (0 = play all)
    #[arg(long, default_value_t = 0)]
    max_pairs: usize,

    #[command(flatten)]
    matchups: MatchupArgs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PairRecord {
    timestamp: String,
    engine_base: String,
    engine_cand: String,
    secs: f32,
    god1: GodName,
    god2: GodName,
    outcome: PairOutcome,
    moves_g1: usize,
    moves_g2: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum PairOutcome {
    CandSweep,
    BaseSweep,
    /// Split where god1 (as player 1) won both games
    SplitGod1,
    /// Split where god2 (as player 2) won both games
    SplitGod2,
}

impl PairOutcome {
    fn is_sweep(self) -> bool {
        matches!(self, PairOutcome::CandSweep | PairOutcome::BaseSweep)
    }

    fn cand_score(self) -> f64 {
        match self {
            PairOutcome::CandSweep => 1.0,
            PairOutcome::BaseSweep => 0.0,
            PairOutcome::SplitGod1 | PairOutcome::SplitGod2 => 0.5,
        }
    }
}

/// game1: baseline as player 1. game2: candidate as player 1.
fn classify_pair(game1: &BattleResult, game2: &BattleResult) -> PairOutcome {
    match (game1.winning_player, game2.winning_player) {
        (Player::Two, Player::One) => PairOutcome::CandSweep,
        (Player::One, Player::Two) => PairOutcome::BaseSweep,
        (Player::One, Player::One) => PairOutcome::SplitGod1,
        (Player::Two, Player::Two) => PairOutcome::SplitGod2,
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct MatchupStats {
    pairs: usize,
    sweeps: usize,
}

fn load_db(path: &PathBuf) -> HashMap<Matchup, MatchupStats> {
    let mut stats: HashMap<Matchup, MatchupStats> = HashMap::new();

    if !path.exists() {
        return stats;
    }

    let mut rdr = csv::Reader::from_path(path).expect("Failed to open results db");
    for record in rdr.deserialize() {
        let record: PairRecord = record.expect("Malformed row in results db");
        let entry = stats
            .entry(Matchup::new(record.god1, record.god2))
            .or_default();
        entry.pairs += 1;
        entry.sweeps += record.outcome.is_sweep() as usize;
    }

    stats
}

fn append_to_db(path: &PathBuf, record: &PairRecord) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create db directory");
    }
    let is_new = !path.exists();
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .expect("Failed to open results db for append");
    let mut wtr = csv::WriterBuilder::new()
        .has_headers(is_new)
        .from_writer(file);
    wtr.serialize(record).expect("Failed to write pair record");
    wtr.flush().expect("Failed to flush results db");
}

/// Smoothed engine-decided rate. Mirrors get a higher prior: with identical
/// gods the position is even by construction, so any decisiveness must come
/// from the engines.
fn matchup_utility(matchup: &Matchup, stats: Option<&MatchupStats>) -> f64 {
    let prior_sweeps = if matchup.is_mirror() { 0.5 } else { 0.25 };
    let (pairs, sweeps) = stats.map_or((0, 0), |s| (s.pairs, s.sweeps));
    (sweeps as f64 + prior_sweeps) / (pairs as f64 + 1.0)
}

fn is_known_forced(stats: Option<&MatchupStats>) -> bool {
    stats.is_some_and(|s| s.pairs >= 3 && s.sweeps == 0)
}

/// Arrange matchups so that popping from the END of the vec yields the
/// execution order: mortal v mortal first, then all remaining mirrors, then
/// all non-mirrors - within each tier by historical evenness (sweep rate)
/// descending, tie-broken by god id order.
fn order_matchups_into_pop_stack(
    all_matchups: &mut Vec<Matchup>,
    db_stats: &HashMap<Matchup, MatchupStats>,
) {
    let mortal_mirror = Matchup::new(GodName::Mortal, GodName::Mortal);
    let tier = |m: &Matchup| -> u8 {
        if *m == mortal_mirror {
            0
        } else if m.is_mirror() {
            1
        } else {
            2
        }
    };
    all_matchups.sort(); // deterministic tie-break (god id order)
    all_matchups.sort_by(|a, b| {
        tier(a).cmp(&tier(b)).then_with(|| {
            let ua = matchup_utility(a, db_stats.get(a));
            let ub = matchup_utility(b, db_stats.get(b));
            ub.partial_cmp(&ua).unwrap()
        })
    });
    // Workers pop() from the end of the queue, so reverse into a stack.
    all_matchups.reverse();
}

fn logistic_score(elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-elo / 400.0))
}

/// Generalized SPRT log likelihood ratio over pair scores {0, 0.5, 1}.
/// Regularized with half a virtual win, draw, and loss so the variance can
/// never collapse to zero (an all-draw start would otherwise explode the
/// ratio and instantly cross a bound).
fn gsprt_llr(wins: usize, draws: usize, losses: usize, elo0: f64, elo1: f64) -> f64 {
    let (w, d, l) = (
        wins as f64 + 0.5,
        draws as f64 + 0.5,
        losses as f64 + 0.5,
    );
    let n = w + d + l;

    let mean = (w + 0.5 * d) / n;
    let variance = ((w + 0.25 * d) / n - mean * mean).max(1e-9);

    let s0 = logistic_score(elo0);
    let s1 = logistic_score(elo1);

    n * (s1 - s0) * (2.0 * mean - s0 - s1) / (2.0 * variance)
}

fn elo_estimate(wins: usize, draws: usize, losses: usize) -> f64 {
    let n = (wins + draws + losses) as f64;
    let mean = ((wins as f64 + 0.5 * draws as f64) / n).clamp(1e-6, 1.0 - 1e-6);
    -400.0 * (1.0 / mean - 1.0).log10()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let db_stats = load_db(&args.db);
    eprintln!(
        "Loaded {} matchups with history from {}",
        db_stats.len(),
        args.db.display()
    );

    // Mortal is only interesting as a mirror: drop mortal vs other gods
    // (still reachable explicitly via -m mortal:god), but always include
    // mortal v mortal as the most even baseline matchup.
    let mortal_mirror = Matchup::new(GodName::Mortal, GodName::Mortal);
    let selector = args
        .matchups
        .to_selector()
        .minus_god_for_both(GodName::Mortal)
        .with_extra_matchup(mortal_mirror);

    let mut all_matchups = selector.get_all();

    if args.mirrors_only {
        all_matchups.retain(|m| m.is_mirror());
    }

    if !args.include_forced {
        let before = all_matchups.len();
        all_matchups.retain(|m| *m == mortal_mirror || !is_known_forced(db_stats.get(m)));
        let skipped = before - all_matchups.len();
        if skipped > 0 {
            eprintln!("Skipping {skipped} known-forced matchups (never swept in >=3 recorded pairs; --include-forced to keep)");
        }
    }

    order_matchups_into_pop_stack(&mut all_matchups, &db_stats);
    let matchups_count = all_matchups.len();

    let lower_bound = (args.beta / (1.0 - args.alpha)).ln();
    let upper_bound = ((1.0 - args.beta) / args.alpha).ln();
    eprintln!(
        "SPRT: elo0={} elo1={} alpha={} beta={} -> bounds ({:.3}, {:.3}), {} candidate matchups",
        args.elo0, args.elo1, args.alpha, args.beta, lower_bound, upper_bound, matchups_count
    );

    let (tx, rx) = mpsc::channel::<WorkerMessage>();
    let num_workers = (num_cpus::get() / 2).max(1);
    eprintln!("Starting {} workers", num_workers);

    let matchups_queue = Arc::new(Mutex::new(all_matchups));

    for worker_idx in 0..num_workers {
        let tx = tx.clone();
        let queue = Arc::clone(&matchups_queue);
        let engine_base = PathBuf::from(&args.engine_base);
        let engine_cand = PathBuf::from(&args.engine_cand);
        let duration = Duration::from_secs_f32(args.secs);
        std::thread::spawn(move || {
            battling_worker_thread::<true>(
                worker_idx.to_string(),
                queue,
                &engine_base,
                &engine_cand,
                duration,
                tx.clone(),
            );
            std::thread::sleep(Duration::from_secs(1));
            tx.send(WorkerMessage::Done).unwrap();
        });
    }

    let (mut wins, mut draws, mut losses) = (0usize, 0usize, 0usize);
    let mut verdict: Option<&str> = None;
    let mut done_workers = 0;

    loop {
        match rx.recv()? {
            WorkerMessage::BattleResultPair((game1, game2)) => {
                let outcome = classify_pair(&game1, &game2);
                match outcome.cand_score() {
                    1.0 => wins += 1,
                    0.0 => losses += 1,
                    _ => draws += 1,
                }

                let record = PairRecord {
                    timestamp: timestamp_string(),
                    engine_base: args.engine_base.clone(),
                    engine_cand: args.engine_cand.clone(),
                    secs: args.secs,
                    god1: game1.god1,
                    god2: game1.god2,
                    outcome,
                    moves_g1: game1.moves_made,
                    moves_g2: game2.moves_made,
                };
                append_to_db(&args.db, &record);

                let pairs = wins + draws + losses;
                let llr = gsprt_llr(wins, draws, losses, args.elo0, args.elo1);
                eprintln!(
                    "{} [{}/{}] llr {:+.3} in ({:.2}, {:.2}) | W-D-L {}-{}-{} | elo ~{:+.1} | {:?} v {:?}: {:?}",
                    timestamp_string(),
                    pairs,
                    matchups_count,
                    llr,
                    lower_bound,
                    upper_bound,
                    wins,
                    draws,
                    losses,
                    elo_estimate(wins, draws, losses),
                    game1.god1,
                    game1.god2,
                    outcome,
                );

                let hit_max = args.max_pairs > 0 && pairs >= args.max_pairs;
                if verdict.is_none() {
                    if llr >= upper_bound {
                        verdict = Some("H1 accepted: candidate is stronger");
                    } else if llr <= lower_bound {
                        verdict = Some("H0 accepted: candidate is not stronger");
                    } else if hit_max {
                        verdict = Some("Inconclusive: hit --max-pairs");
                    }

                    if verdict.is_some() {
                        // Drain the queue; in-flight pairs still get recorded.
                        matchups_queue.lock().unwrap().clear();
                    }
                }
            }
            WorkerMessage::BattleResult(result) => {
                eprintln!("Unexpected unpaired result: {:?}", result);
            }
            WorkerMessage::Done => {
                done_workers += 1;
                if done_workers >= num_workers {
                    break;
                }
            }
        }
    }

    let pairs = wins + draws + losses;
    eprintln!("==========================================");
    eprintln!(
        "{} vs {} ({} pairs @ {}s/move)",
        args.engine_cand, args.engine_base, pairs, args.secs
    );
    eprintln!(
        "W-D-L {}-{}-{} | llr {:+.3} in ({:.2}, {:.2}) | elo ~{:+.1}",
        wins,
        draws,
        losses,
        gsprt_llr(wins, draws, losses, args.elo0, args.elo1),
        lower_bound,
        upper_bound,
        elo_estimate(wins, draws, losses),
    );
    eprintln!(
        "{}",
        verdict.unwrap_or("Inconclusive: ran out of matchups before a bound was crossed")
    );
    eprintln!("Pair outcomes appended to {}", args.db.display());

    Ok(())
}

// cargo run -p battler --bin sprt -r -- -e v122 -E v123
// cargo run -p battler --bin sprt -r -- -e v122 -E v123 --elo1 5 --max-pairs 400
// cargo run -p battler --bin sprt -r -- -e v122 -E v123 --include-forced

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llr_no_explosion_on_all_draws() {
        // A handful of draws is weak evidence - it must not cross a bound
        // (pre-fix, zero variance exploded the ratio to ~-200k at n=1).
        for n in [1, 3, 10, 20] {
            let llr = gsprt_llr(0, n, 0, 0.0, 10.0);
            assert!(llr.abs() < 2.944, "all-draw llr exploded: {llr} at n={n}");
        }
        // ...but a long pure-draw record SHOULD eventually accept H0:
        // drawing forever is proof the candidate is not +10 elo stronger.
        assert!(gsprt_llr(0, 200, 0, 0.0, 10.0) < -2.944);
        // Decisive records move the ratio in the right direction.
        assert!(gsprt_llr(10, 0, 0, 0.0, 10.0) > 0.5);
        assert!(gsprt_llr(0, 0, 10, 0.0, 10.0) < -0.5);
    }

    #[test]
    fn test_execution_order_mirrors_first() {
        let mut matchups = vec![
            Matchup::new(GodName::Pan, GodName::Artemis),
            Matchup::new(GodName::Artemis, GodName::Artemis),
            Matchup::new(GodName::Mortal, GodName::Mortal),
            Matchup::new(GodName::Pan, GodName::Hephaestus),
            Matchup::new(GodName::Pan, GodName::Pan),
            Matchup::new(GodName::Hephaestus, GodName::Hephaestus),
        ];

        // Give a non-mirror a perfect historical sweep rate: it must still
        // run after every mirror.
        let mut stats = HashMap::new();
        stats.insert(
            Matchup::new(GodName::Pan, GodName::Artemis),
            MatchupStats { pairs: 5, sweeps: 5 },
        );
        // A mirror with poor history still stays in the mirror tier.
        stats.insert(
            Matchup::new(GodName::Hephaestus, GodName::Hephaestus),
            MatchupStats { pairs: 5, sweeps: 1 },
        );

        order_matchups_into_pop_stack(&mut matchups, &stats);

        // Simulate worker pop() order.
        let mut execution = Vec::new();
        while let Some(m) = matchups.pop() {
            execution.push(m);
        }

        assert_eq!(
            execution,
            vec![
                Matchup::new(GodName::Mortal, GodName::Mortal),
                Matchup::new(GodName::Pan, GodName::Pan),
                Matchup::new(GodName::Artemis, GodName::Artemis),
                Matchup::new(GodName::Hephaestus, GodName::Hephaestus),
                Matchup::new(GodName::Pan, GodName::Artemis),
                Matchup::new(GodName::Pan, GodName::Hephaestus),
            ]
        );
    }
}
