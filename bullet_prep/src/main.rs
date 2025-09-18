use std::fs::{self, File, OpenOptions, remove_file};
use std::io::{BufReader, BufWriter, prelude::*};
use std::path::PathBuf;

use rand::seq::SliceRandom;
use rand::{Rng, rng};
use santorini_core::bitboard::BitBoard;
use santorini_core::board::FullGameState;
use santorini_core::gods::GodName;
use santorini_core::matchup::Matchup;
use santorini_core::player::Player;

// !!! BulletSantoriniBoard needs to match exactly with the definition in santorini-trainer rep
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BulletSantoriniBoard {
    height_maps: [BitBoard; 4],
    worker_maps: [BitBoard; 2],
    score: i16,
    result: u8,
    god1: u8,
    god2: u8,
    is_athena_block: bool,
    extra: u8,
}
const _RIGHT_SIZE: () = assert!(std::mem::size_of::<BulletSantoriniBoard>() == 32);

const TMP_OUTPUT_FILE_COUNT: usize = 1024;

fn convert_row_to_board_and_meta(row: &str) -> Option<(FullGameState, Player, i16)> {
    let parts: Vec<_> = row.split(' ').collect();
    if parts.len() < 6 {
        eprintln!("skipping malformed row: {}", row);
        return None;
    }
    let fen_str = parts[0];
    let winner_str = parts[1];
    let score_str = parts[2];
    let _ply_str = parts[3];
    let _depth_str = parts[4];
    let _nodes_str = parts[5];

    let full_state = FullGameState::try_from(fen_str).expect("Could not parse fen");
    let score: i16 = score_str.parse().expect("Could not parse score");
    let winner_idx: i32 = winner_str.parse().expect("Could not parse winner");

    let winner = match winner_idx {
        1 => Player::One,
        2 => Player::Two,
        _ => panic!("Winner string must be either 1 or 2"),
    };

    Some((full_state, winner, score))
}

fn write_data_file<T: Copy>(items: &[T], path: &PathBuf) -> std::io::Result<()> {
    let bytes_len = items.len() * std::mem::size_of::<T>();
    let bytes = unsafe { std::slice::from_raw_parts(items.as_ptr() as *const u8, bytes_len) };

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(bytes)?;
    Ok(())
}

#[allow(dead_code)]
fn write_single_record(item: &BulletSantoriniBoard, file: &mut File) -> std::io::Result<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(
            item as *const BulletSantoriniBoard as *const u8,
            std::mem::size_of::<BulletSantoriniBoard>(),
        )
    };
    file.write_all(bytes)
}

fn write_data_file_with_handle<T: Copy>(items: &[T], file: &mut File) -> std::io::Result<()> {
    let bytes_len = items.len() * std::mem::size_of::<T>();
    let bytes = unsafe { std::slice::from_raw_parts(items.as_ptr() as *const u8, bytes_len) };
    file.write_all(bytes)
}

fn read_data_file(path: &PathBuf) -> std::io::Result<Vec<BulletSantoriniBoard>> {
    let file = File::open(path)?;
    let file_size = file.metadata()?.len() as usize;
    let item_count = file_size / std::mem::size_of::<BulletSantoriniBoard>();

    let mut reader = BufReader::new(file);
    let mut buffer = vec![0u8; file_size];
    reader.read_exact(&mut buffer)?;

    let items = unsafe {
        std::slice::from_raw_parts(buffer.as_ptr() as *const BulletSantoriniBoard, item_count)
    };

    Ok(items.to_vec())
}

fn all_filenames_in_dir(root: &PathBuf) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    fn visit_dirs(dir: &PathBuf, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, files);
                } else if path.is_file() {
                    if entry.path().extension() == Some(std::ffi::OsStr::new("txt")) {
                        files.push(path);
                    } else {
                        println!("skipping: {:?}", path);
                    }
                }
            }
        }
    }
    let mut files = Vec::new();
    visit_dirs(&root, &mut files);
    Ok(files)
}

// Step 1: Convert raw data files to temporary bullet format files, distributing across multiple outputs
fn process_raw_data_files(
    input_dir: PathBuf,
    temp_dir: PathBuf,
    delete_source: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rng();
    let all_data_files = all_filenames_in_dir(&input_dir)?;

    std::fs::create_dir_all(&temp_dir)?;

    let mut temp_files: Vec<File> = Vec::with_capacity(TMP_OUTPUT_FILE_COUNT);
    for i in 0..TMP_OUTPUT_FILE_COUNT {
        let temp_file_path = temp_dir.join(format!("temp_{:04}.dat", i));
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(temp_file_path)?;
        temp_files.push(file);
    }

    let mut total_examples = 0;

    let mut current_buffer = Vec::new();
    let mut temp_file_buffers: Vec<Vec<BulletSantoriniBoard>> =
        vec![Vec::new(); TMP_OUTPUT_FILE_COUNT];

    for (i, filename) in all_data_files.iter().enumerate() {
        println!(
            "{}/{} Processing {:?} ({})",
            i + 1,
            all_data_files.len(),
            filename,
            total_examples
        );

        let file_handle = File::open(filename).expect("Failed to open file");
        let reader = BufReader::new(file_handle);

        for line in reader.lines() {
            let Some((state, winner, score)) = convert_row_to_board_and_meta(&line?) else {
                continue;
            };

            let (god1, god2) = match state.board.current_player {
                Player::One => (state.gods[0], state.gods[1]),
                Player::Two => (state.gods[1], state.gods[0]),
            };
            let god1 = god1.god_name as u8;
            let god2 = god2.god_name as u8;
            // let is_athena_block = state.board.get_worker_can_climb(state.board.current_player);
            let is_athena_block = false;
            let result = if winner == state.board.current_player {
                1
            } else {
                0
            };

            for perm in state
                .board
                .get_all_permutations::<true>(state.gods, state.base_hash())
            {
                // TODO: flip god datas too
                let mut worker_maps = perm.workers;
                if state.board.current_player == Player::Two {
                    worker_maps.swap(0, 1);
                }

                let bullet_board = BulletSantoriniBoard {
                    height_maps: perm.height_map,
                    worker_maps,
                    score,
                    result,
                    god1,
                    god2,
                    is_athena_block,
                    extra: 0,
                };

                current_buffer.push(bullet_board);
                total_examples += 1;
            }
        }

        println!(
            "Shuffling and distributing {} examples",
            current_buffer.len()
        );
        current_buffer.shuffle(&mut rng);

        for state in &current_buffer {
            let file_idx = rng.random_range(0..TMP_OUTPUT_FILE_COUNT);
            temp_file_buffers[file_idx].push(*state);
        }
        current_buffer.clear();

        for (file_idx, buffer) in temp_file_buffers.iter_mut().enumerate() {
            write_data_file_with_handle(buffer, &mut temp_files[file_idx])?;
            buffer.clear();
        }

        if delete_source {
            if let Err(e) = std::fs::remove_file(filename) {
                eprintln!(
                    "Warning: Failed to delete source file {:?}: {}",
                    filename, e
                );
            } else {
                println!("Deleted source file: {:?}", filename);
            }
        }
    }

    println!(
        "Processed {} total examples into {} temporary files",
        total_examples, TMP_OUTPUT_FILE_COUNT
    );
    Ok(())
}

// Step 2: Read each temporary file, shuffle it, and write to final output
fn consolidate_temp_files(
    temp_dir: PathBuf,
    output_path: PathBuf,
    delete_temp: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rng();
    let mut total_examples = 0;

    // Clear output file if it exists
    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }

    for i in 0..TMP_OUTPUT_FILE_COUNT {
        let temp_file_path = temp_dir.join(format!("temp_{:04}.dat", i));

        if !temp_file_path.exists() {
            continue;
        }

        println!(
            "Processing temporary file {}/{}: {:?}",
            i + 1,
            TMP_OUTPUT_FILE_COUNT,
            temp_file_path
        );

        // Read the entire temporary file
        let mut data = read_data_file(&temp_file_path)?;

        if data.is_empty() {
            println!("Skipping empty file: {:?}", temp_file_path);
            continue;
        }

        println!("Read {} examples, shuffling...", data.len());
        data.shuffle(&mut rng);

        println!("Writing {} examples to output", data.len());
        write_data_file(&data, &output_path)?;

        total_examples += data.len();

        if delete_temp {
            if let Err(e) = remove_file(&temp_file_path) {
                eprintln!(
                    "Warning: Failed to delete temp file {:?}: {}",
                    temp_file_path, e
                );
            } else {
                println!("Deleted temp file: {:?}", temp_file_path);
            }
        }
    }

    println!(
        "Consolidated {} total examples into final output: {:?}",
        total_examples, output_path
    );
    Ok(())
}

#[allow(dead_code)]
fn filter_atlas_vs_athena() -> Result<(), Box<dyn std::error::Error>> {
    let all_data_files = all_filenames_in_dir(&PathBuf::from("./tmp/gen_3/"))?;

    let matching_dir_path = PathBuf::from("matching");
    let non_matching_dir_path = PathBuf::from("not_matching");

    fs::create_dir_all(&matching_dir_path)?;
    fs::create_dir_all(&non_matching_dir_path)?;

    let m1 = Matchup::new(GodName::Athena, GodName::Atlas);

    for (i, filename) in all_data_files.iter().enumerate() {
        println!(
            "{}/{} Processing {:?}",
            i + 1,
            all_data_files.len(),
            filename,
        );

        let file_handle = File::open(filename).expect("Failed to open file");
        let reader = BufReader::new(file_handle);

        let matching_path = matching_dir_path.join(filename.file_name().unwrap());
        let non_matching_path = non_matching_dir_path.join(filename.file_name().unwrap());

        let mut matching_file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(matching_path)?,
        );
        let mut non_matching_file = BufWriter::new(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(non_matching_path)?,
        );

        for line in reader.lines() {
            let line = line?;
            let Some((state, _, _)) = convert_row_to_board_and_meta(&line) else {
                eprintln!("bad line: {:?}", &line);
                continue;
            };

            let m = state.get_matchup();
            if m.is_same_gods(&m1) {
                writeln!(matching_file, "{}", line)?;
            } else {
                writeln!(non_matching_file, "{}", line)?;
            }
        }
    }

    Ok(())
}

fn real_main() -> Result<(), Box<dyn std::error::Error>> {
    let is_delete = true;

    let input_path = PathBuf::from("raw_data");
    let temp_path = PathBuf::from("temp_data");
    let output_path = PathBuf::from("final_data");

    println!("Step 1: Processing raw data files...");
    process_raw_data_files(input_path, temp_path.clone(), is_delete)?;

    println!("Step 2: Consolidating temporary files...");
    consolidate_temp_files(temp_path, output_path, is_delete)?;

    println!("Data preparation complete!");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    real_main()?;
    // filter_atlas_vs_athena()?;
    Ok(())
}

// fn break_up_final_data(
//     final_data_path: PathBuf,
//     temp_dir: PathBuf,
// ) -> Result<(), Box<dyn std::error::Error>> {
//     let mut rng = rng();
//
//     // Read all data from final file
//     println!("Reading final data from: {:?}", final_data_path);
//     let mut all_data = read_data_file(&final_data_path)?;
//     println!("Read {} total examples", all_data.len());
//
//     // Create temp directory
//     std::fs::create_dir_all(&temp_dir)?;
//
//     // Create temp files
//     let mut temp_files: Vec<File> = Vec::with_capacity(TMP_OUTPUT_FILE_COUNT);
//     for i in 0..TMP_OUTPUT_FILE_COUNT {
//         let temp_file_path = temp_dir.join(format!("temp_{:04}.dat", i));
//         let file = OpenOptions::new()
//             .create(true)
//             .truncate(true)
//             .write(true)
//             .open(temp_file_path)?;
//         temp_files.push(file);
//     }
//
//     // Distribute data across temp files
//     let chunk_size = (all_data.len() + TMP_OUTPUT_FILE_COUNT - 1) / TMP_OUTPUT_FILE_COUNT;
//
//     for (file_idx, chunk) in all_data.chunks(chunk_size).enumerate() {
//         if file_idx >= TMP_OUTPUT_FILE_COUNT {
//             break;
//         }
//
//         println!("Writing {} examples to temp file {}", chunk.len(), file_idx);
//         write_data_file_with_handle(chunk, &mut temp_files[file_idx])?;
//     }
//
//     println!(
//         "Successfully broke up data into {} temporary files",
//         TMP_OUTPUT_FILE_COUNT
//     );
//     Ok(())
// }
//
// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let final_data_path = PathBuf::from("final_data");
//     let temp_dir = PathBuf::from("temp_data");
//
//     if !final_data_path.exists() {
//         eprintln!("Error: final_data file does not exist");
//         return Ok(());
//     }
//
//     break_up_final_data(final_data_path, temp_dir)?;
//
//     println!("Data break-up complete!");
//     Ok(())
// }

// rm -rf temp_data
// rm final_data
// ulimit -n 2048
// cargo run -p bullet_prep --release
