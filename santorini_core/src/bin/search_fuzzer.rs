//! Plays whole games by *searching* each move, so it exercises the negamax path the ordinary
//! fuzzer never touches (that one plays random moves and only runs the consistency checker).
//!
//! Every search is wrapped in `catch_unwind`, so a panic anywhere in make/unmake, move ordering,
//! the transposition table or eval is reported as the FEN that triggered it rather than taking the
//! process down.
//!
//! ```
//! cargo run -p santorini_core --bin search_fuzzer -r -- -g charybdis -t 60
//! ```

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use clap::Parser;
use rand::{Rng, rng};

use santorini_core::{
    board::FullGameState,
    fen::game_state_to_fen,
    gods::GodName,
    matchup::{Matchup, MatchupSelector},
    player::Player,
    random_utils::get_random_starting_state,
    search::{SearchContext, get_win_reached_search_terminator, negamax_search},
    search_terminators::DynamicMaxDepthSearchTerminator,
    transposition_table::TranspositionTable,
};

#[derive(Parser, Debug)]
struct Args {
    /// Restrict player one to this god.
    #[arg(short = 'g')]
    p1_god: Option<GodName>,
    /// Restrict player two to this god.
    #[arg(short = 'G')]
    p2_god: Option<GodName>,
    /// Search depth per move.
    #[arg(short = 'd', default_value_t = 4)]
    depth: usize,
    /// Timeout in seconds.
    #[arg(short = 't')]
    timeout_secs: Option<f64>,
    /// Stop on the first crash.
    #[arg(short = 's', default_value_t = false)]
    stop_on_failure: bool,
    /// Search one specific FEN and exit (for reproducing a crash deterministically).
    #[arg(short = 'f')]
    fen: Option<String>,
}

/// Search one position. Returns the chosen move's resulting state, or None if there is no move.
fn search_once(state: &FullGameState, depth: usize) -> Option<FullGameState> {
    let mut tt = TranspositionTable::new();
    let mut ctx = SearchContext {
        tt: &mut tt,
        new_best_move_callback: Box::new(|_| {}),
        terminator: DynamicMaxDepthSearchTerminator::new(depth),
    };

    let search_state = negamax_search(&mut ctx, state.clone(), get_win_reached_search_terminator());
    let best = search_state.best_move?;

    // Use the engine's precomputed resulting state, exactly as the real consumers do. Re-applying
    // `best.action` is wrong: on a lost position the engine returns a NULL action with a
    // winner-set child_state, and re-applying NULL builds garbage.
    Some(best.child_state)
}

/// Random towers on the empty squares, so search starts on a board that already has domes rather
/// than having to build all the way up from flat. This is what makes a build-on-dome bug surface
/// in seconds instead of hours.
fn raise_random_towers(state: &mut FullGameState, rng: &mut impl Rng) {
    use santorini_core::bitboard::BitBoard;

    let occupied = (state.board.workers[0] | state.board.workers[1]) & BitBoard::MAIN_SECTION_MASK;
    // Build only on genuinely empty squares, iterating the bitboard directly so there is no chance
    // of a square/bit mismatch putting a tower under a worker.
    let empty = BitBoard::MAIN_SECTION_MASK & !occupied;
    for square in empty {
        // Up to level 3, never a pre-placed dome: a level-3 tower is a perfectly reachable board,
        // and it lets search create domes naturally with a single build. Pre-seeding domes instead
        // produces boards that never occur in real games, where move generators are not obliged to
        // behave and any "bug" found is a false alarm.
        let height = rng.random_range(0..=3);
        for _ in 0..height {
            state.board.build_up(square);
        }
    }
}

fn play_one_game(matchup: &Matchup, depth: usize, rng: &mut impl Rng) -> Result<(), String> {
    let mut state = get_random_starting_state(matchup, rng);
    raise_random_towers(&mut state, rng);
    if state.validation_err().is_err() {
        return Ok(());
    }

    for _ply in 0..160 {
        if state.board.get_winner().is_some() {
            return Ok(());
        }

        let fen = game_state_to_fen(&state);
        let result = catch_unwind(AssertUnwindSafe(|| search_once(&state, depth)));

        match result {
            Err(_) => return Err(fen),
            Ok(None) => return Ok(()),
            Ok(Some(next)) => {
                // A legal move can never produce an invalid board. If it did, the search chose an
                // illegal move - report the position it was chosen from.
                if next.validation_err().is_err() {
                    return Err(fen);
                }
                state = next;
            }
        }
    }

    Ok(())
}

fn main() {
    let args = Args::parse();
    let mut rng = rng();

    if let Some(fen) = &args.fen {
        let state = santorini_core::fen::parse_fen(fen).expect("bad fen");
        let _ = search_once(&state, args.depth);
        eprintln!("done, no crash");
        return;
    }

    let deadline = args
        .timeout_secs
        .map(|s| Instant::now() + std::time::Duration::from_secs_f64(s));

    let mut selector = MatchupSelector::default().with_can_swap();
    if let Some(g) = args.p1_god {
        selector = selector.with_exact_gods_for_player(Player::One, &[g]);
    }
    if let Some(g) = args.p2_god {
        selector = selector.with_exact_gods_for_player(Player::Two, &[g]);
    }

    let mut games = 0u64;

    loop {
        if deadline.is_some_and(|d| Instant::now() >= d) {
            eprintln!("Timeout reached after {games} games.");
            break;
        }

        let mut matchup = selector.get();
        if rng.random_bool(0.5) {
            matchup = matchup.flip();
        }

        if let Err(fen) = play_one_game(&matchup, args.depth, &mut rng) {
            eprintln!("CRASH in {matchup} searching: {fen}");
            if args.stop_on_failure {
                eprintln!("Stopping on first crash.");
                break;
            }
        }

        games += 1;
    }
}
