#![allow(unused)]
use santorini_ai::{
    board::{
        BOARD_WIDTH, BitmapType, Coord, NUM_SQUARES, SantoriniState, coord_to_position,
        position_to_coord,
    },
    transposition_table::TTEntry,
};
use std::{collections::HashMap, hint::black_box, time::Duration};

fn output_neighbor_mask() {
    for p in 0..NUM_SQUARES {
        let coord = position_to_coord(p);
        let (x, y) = (coord.x as i64, coord.y as i64);

        let mut neighbor_mask = 0 as BitmapType;
        for dx in [-1, 0, 1] {
            for dy in [-1, 0, 1] {
                if dx == dy && dx == 0 {
                    continue;
                }

                let nx = x + dx;
                let ny = y + dy;

                if nx < 0 || nx >= BOARD_WIDTH as i64 || ny < 0 || ny >= BOARD_WIDTH as i64 {
                    continue;
                }

                let nc: usize = coord_to_position(Coord::new(nx as usize, ny as usize));
                neighbor_mask |= 1 << nc;
            }
        }
        println!("{},", neighbor_mask);

        // println!("{:?}", coord);
        // print_full_bitmap(neighbor_mask);
    }
}

fn benchmark_fn(msg: &str, f: impl Fn()) {
    let start_time = std::time::Instant::now();
    f();
    let elapsed = start_time.elapsed();
    println!("{}: {} ms", msg, elapsed.as_millis());
}

const LOOP: usize = 10_000_000;
fn testing_loop_exists() {
    /*
    benchmark_fn("asdf", || {
        let mut i = 0;
        for _ in 0..LOOP {
            i = black_box(i + 1);
        }
        println!("{i}");
    });
    */

    let dur: Duration = Duration::from_secs(5);

    {
        let start = std::time::Instant::now();
        let mut i = 0;
        loop {
            if start.elapsed() > dur {
                break;
            }
            i = black_box(i + 1);
        }
        println!("time check: {i}");
    }

    {
        let start = std::time::Instant::now();
        let mut i = 0;
        loop {
            if i % 1 << 8 == 0 && start.elapsed() > dur {
                break;
            }
            i = black_box(i + 1);
        }
        println!("check every {}: {i}", 1 << 8);
    }
}

fn benchmark_vec_allocations_empty() {
    benchmark_fn("vec_allocations_empty", || {
        for _ in 0..1000000 {
            black_box(Vec::<u32>::with_capacity(100));
        }
    });
}

fn benchmark_vec_allocations_keep() {
    benchmark_fn("vec_allocations_empty", || {
        fn get_thingy() -> Vec<Vec<u32>> {
            let mut res = Vec::new();
            for _ in 0..1000000 {
                res.push(Vec::<u32>::with_capacity(100));
            }
            res
        }
        black_box(get_thingy());
    });
}

fn benchmark_finding_children_with_hueristic() {
    let state = SantoriniState::new_basic_state();
    benchmark_fn("with scores", || {
        for _ in 0..1000000 {
            black_box(state.get_next_states_with_scores());
        }
    });
}

fn benchmark_finding_children_fast() {
    let state = SantoriniState::new_basic_state();
    benchmark_fn("fast", || {
        for _ in 0..1000000 {
            black_box(state.get_valid_next_states());
        }
    });
}

fn benchmark_finding_children_interactive() {
    let state = SantoriniState::new_basic_state();
    benchmark_fn("interactive", || {
        for _ in 0..1000000 {
            black_box(state.get_next_states_interactive());
        }
    });
}

type TestNode = i32;
type TestHueristic = i32;

pub fn search() -> (TestNode, TestHueristic) {
    let mut node_tree = HashMap::new();

    let a = 100;
    let b = 101;
    let c = 102;
    let d = 103;
    let e = 104;
    let f = 105;
    let g = 106;

    node_tree.insert(a, vec![b, c]);
    node_tree.insert(b, vec![d, e]);
    node_tree.insert(d, vec![3, 5]);
    node_tree.insert(e, vec![6, 9]);
    node_tree.insert(c, vec![f, g]);
    node_tree.insert(f, vec![1, 2]);
    node_tree.insert(g, vec![0, -1]);

    let root = a;
    let depth = 4;

    let color = 1;
    let res = _inner_search(
        &node_tree,
        &root,
        depth,
        color,
        TestHueristic::MIN + 1,
        TestHueristic::MAX,
    );

    res
}

fn _inner_search(
    node_tree: &HashMap<TestNode, Vec<TestNode>>,
    state: &TestNode,
    remaining_depth: usize,
    color: TestHueristic,
    mut alpha: TestHueristic,
    beta: TestHueristic,
) -> (TestNode, TestHueristic) {
    println!("Visiting {} a: {} b: {}", state, alpha, beta);

    let children = node_tree.get(&state).map_or_else(|| Vec::new(), Vec::clone);
    let is_terminal = children.len() == 0;

    if remaining_depth == 0 || is_terminal {
        return (state.clone(), color * state);
    }

    // if color == 1 {
    //     children.sort_by(|a, b| (b.1).partial_cmp(&a.1).unwrap());
    // } else {
    //     children.sort_by(|a, b| (a.1).partial_cmp(&b.1).unwrap());
    // }

    let mut best_board = children[0];
    let mut best_score = TestHueristic::MIN;

    for child in &children {
        let (_, score) =
            _inner_search(node_tree, child, remaining_depth - 1, -color, -beta, -alpha);
        let score = -score;

        if score > best_score {
            println!(
                "{}: new best score: {} > {} : {}",
                state, score, best_score, *child
            );

            best_score = score;
            best_board = *child;

            if score > alpha {
                alpha = score;

                if alpha >= beta {
                    println!(
                        "{}: pruning after {}. a: {} b: {}",
                        state, child, alpha, beta
                    );
                    break;
                }
            }
        }
    }

    println!("Returning {}, {}", best_board, best_score);
    (best_board.clone(), best_score)
}

fn main() {
    /*
    // 48 bytes
    // 128 * 1000000 / 48
    // =2_666_666
    // 22_633_363*48. yeah it's about a gig
    println!("State size: {:?}", size_of::<SantoriniState>());
    println!("TTEntry size: {:?}", size_of::<TTEntry>());
    println!("Option<TTEntry> size: {:?}", size_of::<Option<TTEntry>>());
    */

    // println!("Search outcome: {:?}", search());
    testing_loop_exists();

    /*
    benchmark_vec_allocations_empty();
    benchmark_vec_allocations_keep();
    benchmark_finding_children_fast();
    benchmark_finding_children_with_hueristic();
    benchmark_finding_children_interactive();
    */
}
