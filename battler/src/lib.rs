use std::process::{Child, ChildStdin, Command, Stdio};

use std::sync::mpsc::{self, Receiver};

use santorini_core::board::FullGameState;
use serde::{Deserialize, Serialize};

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{Duration, Instant};
use std::thread;

use santorini_core::uci_types::EngineOutput;

const CORPUS_FILE_PATH: &str = "data/corpus.yaml";

pub const BINARY_DIRECTORY: &str = "all_versions";

#[derive(Serialize, Deserialize)]
pub struct StartingPosition {
    pub name: String,
    pub state: FullGameState,
    pub notes: String,
}

#[derive(Serialize, Deserialize)]
pub struct Corpus {
    pub positions: Vec<StartingPosition>,
}

pub fn write_corpus(corpus: &Corpus) {
    let toml_string = serde_yaml::to_string(corpus).expect("Failed to serialize corpus");
    std::fs::write(CORPUS_FILE_PATH, toml_string).expect("Failed to write corpus to file");
}

pub fn read_corpus() -> Corpus {
    let toml_string =
        std::fs::read_to_string(CORPUS_FILE_PATH).expect("Failed to read corpus file");
    serde_yaml::from_str(&toml_string).expect("Failed to deserialize corpus")
}

pub fn create_log_dir() {
    let path = std::env::current_dir()
        .expect("Failed to get current directory")
        .join("logs");
    std::fs::create_dir_all(&path).expect("Failed to create logs directory");
}

pub struct EngineSubprocess {
    pub engine_name: String,
    #[allow(dead_code)]
    pub child: Child,
    pub stdin: ChildStdin,
    pub receiver: Receiver<String>,
}

pub fn prepare_subprocess(log_path: &Path, engine_path: &Path) -> EngineSubprocess {
    let stderr_file = std::fs::File::create(log_path).expect("Failed to create error log file");

    let mut child = Command::new(engine_path)
        .stdin(Stdio::piped())
        .stderr(std::process::Stdio::from(stderr_file))
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");

    let (child_msg_tx, child_msg_rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    child_msg_tx.send(line).unwrap();
                }
                Err(e) => {
                    eprintln!("Error reading line: {}", e);
                    break;
                }
            }
        }
    });

    let end_at = Instant::now() + Duration::from_secs(10);
    loop {
        let now = Instant::now();
        if now >= end_at {
            panic!("Waited too long to spin up child");
        }

        let timeout = end_at - now;

        match child_msg_rx.recv_timeout(timeout) {
            Ok(msg) => {
                // println!("I got a message {}", msg);
                let parsed_msg: EngineOutput = serde_json::from_str(&msg).unwrap();
                match parsed_msg {
                    EngineOutput::Started(_) => {
                        // println!("Started!");
                        break;
                    }
                    _ => {
                        println!("received non-ready message. Still waiting...")
                    }
                }
                println!("parsed msg: {:?}", parsed_msg);
            }
            Err(e) => {
                panic!("Error while waiting for child: {:?}", e);
            }
        }
    }

    EngineSubprocess {
        engine_name: engine_path.to_str().unwrap().to_owned(),
        child,
        stdin,
        receiver: child_msg_rx,
    }
}
