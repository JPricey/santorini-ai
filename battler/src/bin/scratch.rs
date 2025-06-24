
use battler::{Corpus, StartingPosition, read_corpus};
use rand::{
    Rng,
    seq::{IteratorRandom, SliceRandom},
    thread_rng,
};
use santorini_core::{board::FullGameState, gods::GodName};

fn _get_board_with_random_placements(rng: &mut impl Rng) -> FullGameState {
    let mut result = FullGameState::new_empty_state(GodName::Mortal, GodName::Mortal);
    let worker_spots: Vec<usize> = (0..25).choose_multiple(rng, 4).iter().cloned().collect();

    result.board.workers[0] |= 1 << worker_spots[0];
    result.board.workers[0] |= 1 << worker_spots[1];

    result.board.workers[1] |= 1 << worker_spots[2];
    result.board.workers[1] |= 1 << worker_spots[3];

    result
}

fn _get_board_with_random_moves(rng: &mut impl Rng, num_moves: usize) -> FullGameState {
    let mut position = _get_board_with_random_placements(rng);

    for _ in 0..num_moves {
        let child_states = position.get_next_states();
        position = child_states.choose(rng).unwrap().clone();
    }

    position
}

#[allow(dead_code)]
fn _seed_corpus(corpus: &mut Corpus) {
    let mut rng = thread_rng();
    // Add some random positions to the starting position corpus
    for i in 0..10 {
        let position = _get_board_with_random_placements(&mut rng);
        corpus.positions.push(StartingPosition {
            name: format!("random_start_{}", i + 1),
            state: position,
            notes: "Position after completely random worker placements".to_owned(),
        });
    }

    for i in 0..20 {
        let position = _get_board_with_random_moves(&mut rng, 2);
        corpus.positions.push(StartingPosition {
            name: format!("random_2_moves_{}", i + 1),
            state: position,
            notes: "Position after random worker placements and random 2 ply".to_owned(),
        });
    }

    for i in 0..20 {
        let position = _get_board_with_random_moves(&mut rng, 3);
        corpus.positions.push(StartingPosition {
            name: format!("random_3_moves_{}", i + 1),
            state: position,
            notes: "Position after random worker placements followed by random 3 ply".to_owned(),
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
