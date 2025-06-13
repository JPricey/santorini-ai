use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use santorini_engine::board::SantoriniState;

fn main() {
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
}
