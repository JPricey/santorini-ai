use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use santorini_engine::board::SantoriniState;
use santorini_engine::uci_types::EngineOutput;

fn prepare_subprocess(engine_path: &str) {
    let mut child = Command::new(engine_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");

    let (child_msg_tx, child_msg_rx) = mpsc::channel::<String>();

    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    println!("Received stdout: {}", line);
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

        // let timeout = end_at - now;
        let timeout = Duration::from_secs(1);

        println!("polling");
        // thread::sleep(Duration::from_secs(1));

        match child_msg_rx.recv_timeout(timeout) {
            Ok(msg) => {
                println!("I got a message {}", msg);
                let parsed_msg: EngineOutput = serde_json::from_str(&msg).unwrap();
                println!("parsed msg: {:?}", parsed_msg);
            }
            Err(e) => {
                // panic!("Error while waiting for child: {:?}", e);
            }
        }
    }
}

fn main() {
    prepare_subprocess("./all_versions/v1");

    /*
    let mut child = Command::new("./all_versions/v1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn process");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let stdout = child.stdout.take().expect("Failed to open stdout");

    let (child_msg_tx, child_msg_rx) = mpsc::channel();

    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    println!("Recived stdout: {}", line);
                    child_msg_tx.send(line).unwrap();
                }
                Err(e) => {
                    eprintln!("Error reading line: {}", e);
                    break;
                }
            }
        }
    });

    let msg = child_msg_rx.recv().unwrap();
    println!("first message! {}", msg);

    // Example of sending commands to the subprocess
    // You can integrate this into your main loop or create another input handling mechanism
    writeln!(stdin, "your command here").expect("Failed to write to stdin");

    // Main thread continues to check if the child process is still running
    loop {
        // Example: Read a line from user and send to subprocess
        let mut input = String::new();
        print!("> ");
        std::io::stdout().flush().expect("Failed to flush stdout");
        std::io::stdin()
            .read_line(&mut input)
            .expect("Failed to read line");

        println!("got input from user: {input}");

        // Send the command to the subprocess
        writeln!(stdin, "{}", input.trim()).expect("Failed to write to stdin");

        // Check if process is still running
        match child.try_wait() {
            Ok(Some(status)) => {
                println!("Process exited with status: {}", status);
                break;
            }
            Ok(None) => {
                // Process still running
                thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("Error waiting for process: {}", e);
                break;
            }
        }
    }
    */
}
