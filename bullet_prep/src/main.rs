use std::collections::HashMap;
use std::fs::{self, File, OpenOptions, remove_file};
use std::io::{BufReader, BufWriter, prelude::*};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use clap::Parser;
use rand::seq::SliceRandom;
use rand::{Rng, rng};
use santorini_core::bitboard::BitBoard;
use santorini_core::board::{BoardState, FullGameState, GodData, GodPair};
use santorini_core::gods::{
    GOD_FEATURE_OFFSETS, GodName, TOTAL_GOD_DATA_FEATURE_COUNT, god_name_to_nnue_size,
};
use santorini_core::matchup::Matchup;
use santorini_core::nnue::emit_god_data_features;
use santorini_core::player::Player;
use santorini_core::utils::timestamp_string;

// !!! BulletSantoriniBoard needs to match exactly with the definition in santorini-trainer rep
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BulletSantoriniBoard {
    // First 100 bits into height map
    // top 28 bits are:
    // 8 bit result
    // 8 bit god1 id
    // 8 bit god2 id
    height_maps: u128,
    worker_maps: [u32; 2],
    god_datas: [u32; 2],
}
const _RIGHT_SIZE: () = assert!(std::mem::size_of::<BulletSantoriniBoard>() == 32);

const NUM_GODS: usize = 39;
const BOARD_FULL_MASK: u32 = (1 << 25) - 1;
const BASE_FEATURES: usize = 5 * 25;
const WORKER_FEATURES: usize = 25 * 4;

const PLAYER_GODS_OFFSET: usize = 0;
const PLAYER_WORKERS_OFFSET: usize = PLAYER_GODS_OFFSET + NUM_GODS;
const PLAYER_DATAS_OFFSET: usize = PLAYER_WORKERS_OFFSET + WORKER_FEATURES;
const PER_SIDE_FEATURES: usize = PLAYER_DATAS_OFFSET + TOTAL_GOD_DATA_FEATURE_COUNT;

const ACTIVE_PLAYER_OFFSET: usize = BASE_FEATURES;

const OPPO_OFFSET: usize = ACTIVE_PLAYER_OFFSET + PER_SIDE_FEATURES;

fn _get_worker_pos_height(height_maps: u128, pos: usize) -> usize {
    let pos = pos as u128;

    let res = ((height_maps >> pos) & 1)
        + 2 * ((height_maps >> (25 + pos)) & 1)
        + 3 * ((height_maps >> (50 + pos)) & 1)
        + 4 * ((height_maps >> (75 + pos)) & 1);
    res as usize
}

type FType = usize;
impl BulletSantoriniBoard {
    fn map_features<F: FnMut(FType)>(&self, mut f: F) {
        let mut remaining_spaces = BOARD_FULL_MASK;
        for h_idx in (0..4).rev() {
            let mut height_mask: u32 =
                ((self.height_maps >> (h_idx * 25)) as u32) & BOARD_FULL_MASK;
            remaining_spaces ^= height_mask;

            let height = h_idx + 1;
            while height_mask > 0 {
                let square = height_mask.trailing_zeros() as FType;
                height_mask &= height_mask - 1;
                let feature = (square * 5 + height as FType) as FType;
                f(feature);
            }
        }

        while remaining_spaces > 0 {
            let square = remaining_spaces.trailing_zeros();
            remaining_spaces &= remaining_spaces - 1;
            let feature = (square * 5) as FType;
            f(feature);
        }

        let god1 = ((self.height_maps >> 108) as u8) as usize;
        let god2 = ((self.height_maps >> 116) as u8) as usize;

        let player_offsets = [
            ACTIVE_PLAYER_OFFSET + PLAYER_WORKERS_OFFSET,
            OPPO_OFFSET + PLAYER_WORKERS_OFFSET,
        ];

        let data_offsets = [
            ACTIVE_PLAYER_OFFSET + PLAYER_DATAS_OFFSET + GOD_FEATURE_OFFSETS[god1],
            OPPO_OFFSET + PLAYER_DATAS_OFFSET + GOD_FEATURE_OFFSETS[god2],
        ];

        f(ACTIVE_PLAYER_OFFSET + god1 as usize);
        f(OPPO_OFFSET + god2 as usize);

        for i in 0..2 {
            let mut worker_map = self.worker_maps[i];
            while worker_map > 0 {
                let pos = worker_map.trailing_zeros() as FType;
                worker_map &= worker_map - 1;
                let worker_height = _get_worker_pos_height(self.height_maps, pos);
                let worker_pos_delta = 4 * pos + worker_height;

                let stm = player_offsets[i] + worker_pos_delta;
                f(stm);
            }

            let mut god_data = self.god_datas[i];
            while god_data > 0 {
                let pos = god_data.trailing_zeros() as FType;
                god_data &= god_data - 1;
                let stm = data_offsets[i] + pos;
                f(stm);
            }
        }
    }
}

const TMP_OUTPUT_FILE_COUNT: usize = 1024;

fn extract_god_data(god: GodName, data: GodData) -> u32 {
    let res = _extract_god_data_to_u32(god, data);
    let max_size = god_name_to_nnue_size(god);

    assert!(res.leading_zeros() as u32 >= (32 - max_size as u32));

    res
}

fn _extract_god_data_to_u32(god: GodName, data: GodData) -> u32 {
    let mut res = 0;

    emit_god_data_features(god, data, |feature_idx| {
        res |= 1 << feature_idx;
    });

    res
}

fn convert_row_to_board_and_meta(row: &str) -> Option<(FullGameState, Player)> {
    let parts: Vec<_> = row.split(' ').collect();
    if parts.len() < 6 {
        eprintln!("skipping malformed row: {}", row);
        return None;
    }
    let fen_str = parts[0];
    let winner_str = parts[1];
    let _score_str = parts[2];
    let _ply_str = parts[3];
    let _depth_str = parts[4];
    let _nodes_str = parts[5];

    let full_state = FullGameState::try_from(fen_str).expect("Could not parse fen");
    // let score: i16 = score_str.parse().expect("Could not parse score");
    let winner_idx: i32 = winner_str.parse().expect("Could not parse winner");

    let winner = match winner_idx {
        1 => Player::One,
        2 => Player::Two,
        _ => panic!("Winner string must be either 1 or 2"),
    };

    Some((full_state, winner))
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

// First 100 bits of this 128 are an exclusive height map. That is:
// 0..25: true if this square is exactly h=1
// 25..50: true if this square is exactly h=2
// ...
// Height 0 is completely skipped for space
fn height_map_to_exclusive_height_u128(heights: &[BitBoard; 4]) -> u128 {
    let mut result: u128 = 0;

    let mut remaining_spaces: u32 = (1 << 25) - 1;
    for height in (0..4).rev() {
        let height_mask = heights[height].0 & remaining_spaces;
        remaining_spaces ^= height_mask;

        result |= (height_mask as u128) << (height * 25);
    }

    result
}

fn fill_results_data(heights: &mut u128, result: u8, god1: u8, god2: u8) {
    let extra: u32 = ((result as u32) << 0) | ((god1 as u32) << 8) | ((god2 as u32) << 16);
    *heights |= (extra as u128) << 100;
}

fn convert_state_to_bullet(
    board: &BoardState,
    gods: GodPair,
    winner: Player,
) -> BulletSantoriniBoard {
    let [god1, god2] = gods;

    let mut height_maps = height_map_to_exclusive_height_u128(&board.height_map);

    let worker_maps = [board.workers[0].0, board.workers[1].0];
    let god_datas = [
        extract_god_data(god1.god_name, board.god_data[0]),
        extract_god_data(god2.god_name, board.god_data[1]),
    ];

    let winner_result: u8 = (board.current_player == winner) as u8;
    if board.current_player == Player::Two {
        fill_results_data(
            &mut height_maps,
            winner_result,
            god2.god_name as u8,
            god1.god_name as u8,
        );

        return BulletSantoriniBoard {
            height_maps,
            worker_maps: [worker_maps[1], worker_maps[0]],
            god_datas: [god_datas[1], god_datas[0]],
        };
    } else {
        fill_results_data(
            &mut height_maps,
            winner_result,
            god1.god_name as u8,
            god2.god_name as u8,
        );

        BulletSantoriniBoard {
            height_maps,
            worker_maps,
            god_datas,
        }
    }
}

fn process_raw_data_files_worker(
    input_files_queue: Arc<Mutex<Vec<PathBuf>>>,
    output_files: Arc<Mutex<Vec<File>>>,
    used_features: Arc<Mutex<Vec<u32>>>,
    total_records: Arc<Mutex<usize>>,
    delete_source: bool,
) {
    let try_fetch_next_input = move || {
        let mut queue = input_files_queue.lock().unwrap();
        let res = queue.pop();

        println!("{} Queue size: {}", timestamp_string(), queue.len());

        res
    };

    let mut total_examples = 0;
    let mut current_buffer = Vec::new();
    let mut temp_file_buffers: Vec<Vec<BulletSantoriniBoard>> =
        vec![Vec::new(); TMP_OUTPUT_FILE_COUNT];
    let all_god_datas = vec![0_u32; santorini_core::gods::ALL_GODS_BY_ID.len()];

    loop {
        let Some(next_input_file) = try_fetch_next_input() else {
            break;
        };

        current_buffer.clear();
        for buffer in &mut temp_file_buffers {
            buffer.clear();
        }

        // println!("Processing {:?}", next_input_file);

        let file_handle = File::open(&next_input_file).expect("Failed to open file");
        let reader = BufReader::new(file_handle);

        for line in reader.lines() {
            let Some((state, winner)) =
                convert_row_to_board_and_meta(&line.expect("Failed to read line"))
            else {
                continue;
            };

            for perm in state
                .board
                .get_all_permutations::<true>(state.gods, state.base_hash())
            {
                let bullet_board = convert_state_to_bullet(&perm, state.gods, winner);
                current_buffer.push(bullet_board);
            }
        }

        total_examples += current_buffer.len();

        println!(
            "{}: Shuffling and distributing {} examples",
            timestamp_string(),
            current_buffer.len()
        );

        let mut rng = rng();
        current_buffer.shuffle(&mut rng);
        for state in &current_buffer {
            let file_idx = rng.random_range(0..TMP_OUTPUT_FILE_COUNT);
            temp_file_buffers[file_idx].push(*state);
        }

        let mut output_files = output_files.lock().unwrap();
        for (file_idx, buffer) in temp_file_buffers.iter_mut().enumerate() {
            write_data_file_with_handle(buffer, &mut output_files[file_idx])
                .expect("Failed to write to output file");
        }

        if delete_source {
            if let Err(e) = std::fs::remove_file(&next_input_file) {
                eprintln!(
                    "{}: Warning: Failed to delete source file {:?}: {}",
                    timestamp_string(),
                    next_input_file,
                    e
                );
            } else {
                println!(
                    "{}: Deleted source file: {:?}",
                    timestamp_string(),
                    next_input_file
                );
            }
        }
    }

    println!("{}: Worker exiting", timestamp_string());

    {
        let mut god_datas = used_features.lock().unwrap();
        for (i, data) in all_god_datas.iter().enumerate() {
            god_datas[i] |= *data;
        }
    }

    {
        let mut total = total_records.lock().unwrap();
        *total += total_examples;
    }
}

// Step 1: Convert raw data files to temporary bullet format files, distributing across multiple outputs
fn process_raw_data_files(
    input_dir: PathBuf,
    temp_dir: PathBuf,
    delete_source: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let num_workers = num_cpus::get();
    let all_data_files = all_filenames_in_dir(&input_dir)?;
    println!(
        "Found {} raw data files, using {} worker threads",
        all_data_files.len(),
        num_workers
    );
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
    let output_files = Arc::new(Mutex::new(temp_files));

    let input_files_queue = Arc::new(Mutex::new(all_data_files));
    let used_features = Arc::new(Mutex::new(vec![
        0_u32;
        santorini_core::gods::ALL_GODS_BY_ID.len()
    ]));
    let total_records = Arc::new(Mutex::new(0_usize));

    let mut handles = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        let input_files_queue = Arc::clone(&input_files_queue);
        let output_files = Arc::clone(&output_files);
        let used_features = Arc::clone(&used_features);
        let total_records = Arc::clone(&total_records);
        let handle = std::thread::spawn(move || {
            process_raw_data_files_worker(
                input_files_queue,
                output_files,
                used_features,
                total_records,
                delete_source,
            );
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let total_examples = *total_records.lock().unwrap();
    println!(
        "{}: All workers complete, processed {} total examples into {} temporary files",
        timestamp_string(),
        total_examples,
        TMP_OUTPUT_FILE_COUNT
    );

    let all_god_datas = used_features.lock().unwrap();
    for god in santorini_core::gods::ALL_GODS_BY_ID {
        let god_id = god.god_name as usize;
        let feature_count = god_name_to_nnue_size(god.god_name);
        let all_features_used = (1 << feature_count) - 1;
        let unused_features = all_features_used & !all_god_datas[god_id];
        if unused_features > 0 {
            eprintln!(
                "Warning: God {:?} is missing features: {:032b}",
                god.god_name, unused_features
            );
        }
    }

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


#[derive(Parser, Debug)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Convert raw text data files into shuffled bullet-format binary
    Prep {
        #[arg(
            short = 'd',
            long,
            default_value_t = false,
            help = "Delete source raw data files after processing"
        )]
        is_delete: bool,
    },
    /// Split raw text data files into per-matchup directories
    SplitMatchups {
        #[arg(help = "Input directory containing raw .txt data files")]
        input_dir: PathBuf,
        #[arg(help = "Output directory where per-matchup files will be written")]
        output_dir: PathBuf,
    },
}

fn run_prep(is_delete: bool) -> Result<(), Box<dyn std::error::Error>> {

    let input_path = PathBuf::from("raw_data");
    let temp_path = PathBuf::from("temp_data");
    let output_path = PathBuf::from("final_data");

    println!("Step 1: Processing raw data files...");
    process_raw_data_files(input_path, temp_path.clone(), is_delete)?;

    println!("Step 2: Consolidating temporary files...");
    consolidate_temp_files(temp_path, output_path, true)?;

    println!("Data preparation complete!");
    Ok(())
}

/// Returns a canonical filename-safe matchup key, with god names sorted alphabetically.
fn matchup_filename(matchup: &Matchup) -> String {
    let mut gods = [matchup.gods[0].to_string(), matchup.gods[1].to_string()];
    gods.sort();
    format!("{}_vs_{}", gods[0], gods[1])
}

fn split_matchups(input_dir: PathBuf, output_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let all_data_files = all_filenames_in_dir(&input_dir)?;
    println!("Found {} raw data files to split", all_data_files.len());

    fs::create_dir_all(&output_dir)?;

    let mut writers: HashMap<String, BufWriter<File>> = HashMap::new();

    for (i, filename) in all_data_files.iter().enumerate() {
        println!(
            "{} {}/{} Processing {:?}",
            timestamp_string(),
            i + 1,
            all_data_files.len(),
            filename,
        );

        let file_handle = File::open(filename)?;
        let reader = BufReader::new(file_handle);

        for line in reader.lines() {
            let line = line?;
            let Some((state, _)) = convert_row_to_board_and_meta(&line) else {
                eprintln!("bad line: {:?}", &line);
                continue;
            };

            let key = matchup_filename(&state.get_matchup());
            let writer = writers.entry(key.clone()).or_insert_with(|| {
                let path = output_dir.join(format!("{}.txt", key));
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .expect("Failed to open matchup output file");
                BufWriter::new(file)
            });

            writeln!(writer, "{}", line).expect("Failed to write line");
        }
    }

    for (_, mut writer) in writers {
        writer.flush()?;
    }

    println!("{} Split complete", timestamp_string());
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Command::Prep { is_delete } => run_prep(is_delete)?,
        Command::SplitMatchups { input_dir, output_dir } => split_matchups(input_dir, output_dir)?,
    }
    Ok(())
}

// rm -rf temp_data
// rm final_data
// ulimit -n 2048
// cargo run -p bullet_prep --release -- prep
// cargo run -p bullet_prep --release -- prep -d
// cargo run -p bullet_prep --release -- split-matchups ./raw_data ./split_output
