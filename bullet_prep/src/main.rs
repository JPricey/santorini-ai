use std::fs::File;
use std::io::{BufReader, prelude::*};
use std::path::PathBuf;

use rand::seq::SliceRandom;
use rand::thread_rng;
use santorini_core::board::{BoardState, FullGameState, Player};

// !!! BulletSantoriniBoard needs to match exactly with the definition in santorini-trainer rep
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BulletSantoriniBoard {
    height_maps: [u32; 4],
    worker_maps: [u32; 2],
    score: i16,
    result: u8,
    extra1: u8, // TODO: add depth / parity to maybe add a horizon offset. Gen1 data records last
    // completed depth, not the actually chosen depth, though
    extra2: u32,
}
const _RIGHT_SIZE: () = assert!(std::mem::size_of::<BulletSantoriniBoard>() == 32);

fn convert_row_to_board_and_meta(row: &str) -> (BoardState, i16, u8) {
    let parts: Vec<_> = row.split(' ').collect();
    let fen_str = parts[0];
    let winner_str = parts[1];
    let score_str = parts[2];
    let _ply_str = parts[3];
    let _depth_str = parts[4];
    let _nodes_str = parts[5];

    let mut full_state = FullGameState::try_from(fen_str).expect("Could not parse fen");
    let score: i16 = score_str.parse().expect("Could not parse score");
    let winner_idx: i32 = winner_str.parse().expect("Could not parse winner");
    assert!(
        winner_idx == 1 || winner_idx == 2,
        "Winner string must be either 1 or 2"
    );
    let result: u8 = if full_state.board.current_player == Player::Two {
        full_state.board.workers.swap(0, 1);
        full_state.board.current_player = Player::One;

        if winner_idx == 2 { 1 } else { 0 }
    } else {
        if winner_idx == 1 { 1 } else { 0 }
    };

    (full_state.board, score, result)
}

fn write_data_file<T: Copy>(items: &[T], path: &str) -> std::io::Result<()> {
    let bytes_len = items.len() * size_of::<T>();
    let bytes = unsafe { std::slice::from_raw_parts(items.as_ptr() as *const u8, bytes_len) };

    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    Ok(())
}

fn all_filenames_in_dir(path: PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut filenames = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    filenames.push(entry.path());
                }
            }
        }
    }
    Ok(filenames)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let data_path = PathBuf::new().join("tmp").join("gen_1_raw");

    let all_data_files = all_filenames_in_dir(data_path)?;
    let mut all_records = Vec::new();

    for (i, filename) in all_data_files.iter().enumerate() {
        println!(
            "{}/{} Processing {:?}. (Result count: {})",
            i,
            all_data_files.len(),
            filename,
            all_records.len()
        );
        let file_handle = File::open(filename).expect("Failed to open file");
        let reader = BufReader::new(file_handle);

        for line in reader.lines() {
            let (board, score, result) = convert_row_to_board_and_meta(&line?);
            for perm in board.get_all_permutations() {
                let bullet_board = BulletSantoriniBoard {
                    height_maps: perm.height_map,
                    worker_maps: perm.workers,
                    score,
                    result,
                    extra1: 0,
                    extra2: 0,
                };

                all_records.push(bullet_board);
            }
        }
    }

    println!("shuffling");
    let mut rng = thread_rng();
    all_records.shuffle(&mut rng);

    println!("writing");
    write_data_file(&all_records, "gen_1_bullet_data").unwrap();

    Ok(())
}
