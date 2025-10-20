use battler::{Corpus, StartingPosition, read_corpus};
use rand::{Rng, rng};
use santorini_core::{
    board::FullGameState,
    gods::GodName,
    matchup::Matchup,
    random_utils::{get_random_move, get_random_starting_state},
};

fn _get_board_with_random_moves(rng: &mut impl Rng, num_moves: usize) -> FullGameState {
    let mut position =
        get_random_starting_state(&Matchup::new(GodName::Mortal, GodName::Mortal), rng);

    for _ in 0..num_moves {
        position = get_random_move(&position, rng).unwrap();
    }

    position
}

#[allow(dead_code)]
fn _seed_corpus(corpus: &mut Corpus) {
    let mut rng = rng();
    // Add some random positions to the starting position corpus
    for i in 0..10 {
        let position =
            get_random_starting_state(&Matchup::new(GodName::Mortal, GodName::Mortal), &mut rng);
        corpus.positions.push(StartingPosition {
            name: format!("random_start_{}", i + 1),
            state: position,
            notes: "Position after completely random worker placements".to_owned(),
            is_enabled: true,
        });
    }

    for i in 0..20 {
        let position = _get_board_with_random_moves(&mut rng, 2);
        corpus.positions.push(StartingPosition {
            name: format!("random_2_moves_{}", i + 1),
            state: position,
            notes: "Position after random worker placements and random 2 ply".to_owned(),
            is_enabled: true,
        });
    }

    for i in 0..20 {
        let position = _get_board_with_random_moves(&mut rng, 3);
        corpus.positions.push(StartingPosition {
            name: format!("random_3_moves_{}", i + 1),
            state: position,
            notes: "Position after random worker placements followed by random 3 ply".to_owned(),
            is_enabled: true,
        });
    }
}

fn print_corpus(corpus: &Corpus) {
    for position in &corpus.positions {
        println!("{}: {}", position.name, position.notes);
        position.state.print_to_console();
    }
}

fn main() {
    let corpus = read_corpus();
    print_corpus(&corpus);
}

// cargo run -p battler --bin scratch
// RUST_BACKTRACE=full cargo run -p battler --bin scratch
