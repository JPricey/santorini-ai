use std::{error, sync::mpsc, thread};

use santorini_ai::{engine::EngineThreadWrapper, fen::parse_fen};

fn handle_command(
    engine: &mut EngineThreadWrapper,
    raw_cmd: &str,
) -> Result<Option<String>, String> {
    let mut parts: Vec<String> = raw_cmd
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(&str::to_owned)
        .collect();
    if parts.is_empty() {
        return Err("Command was empty".to_owned());
    }
    let command = parts.remove(0);

    match &command as &str {
        "quit" => {
            std::process::exit(0);
        }
        "ping" => Ok(Some("pong".to_owned())),
        "set_position" => {
            if parts.len() != 1 {
                return Err("set_position should be followed by a single FEN string".to_owned());
            }

            let fen = parts.remove(0);
            match parse_fen(&fen) {
                Ok(state) => {
                    let _ = engine.stop();
                    engine.start_search(&state)?;
                    Ok(None)
                }
                Err(e) => Err(format!("Error parsing FEN: {}", e)),
            }
        }
        _ => Err(format!("Skipping unknown command: {}", raw_cmd)),
    }
}

fn main() {
    let (cli_command_sender, cli_command_receiver) = mpsc::channel();

    let _io_thread = thread::spawn(move || {
        loop {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            cli_command_sender.send(input).unwrap();
        }
    });

    let mut engine = EngineThreadWrapper::new();

    loop {
        let raw_cmd = cli_command_receiver.recv().unwrap();
        match handle_command(&mut engine, &raw_cmd) {
            Ok(Some(response)) => {
                println!("{}", response);
            }
            Ok(None) => {
                // No response to print
            }
            Err(err) => {
                eprintln!("Error: {}", err.trim());
            }
        }
    }

    // io_thread.join().unwrap();
    // println!("bye");
}
