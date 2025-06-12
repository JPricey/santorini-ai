use std::{sync::mpsc, thread};

struct UCI {}

impl UCI {
    pub fn new() -> Self {
        UCI {}
    }

    fn execute(&mut self) {
        let (tx, rx) = mpsc::channel();

        // Start the input thread
        let io_thread = thread::spawn(move || {
            loop {
                let mut input = String::new();
                std::io::stdin().read_line(&mut input).unwrap();

                println!("Received Command: {}", input);

                /*
                if input.trim() == "stop" {
                    STOP.store(true, Ordering::Relaxed);
                    continue;
                }
                if input.trim() == "quit" {
                    STOP.store(true, Ordering::Relaxed); //stop engine then pass quit to uci loop
                }
                */
                tx.send(input).unwrap();
            }
        });

        io_thread.join().unwrap();
    }
}

fn main() {
    let mut uci = UCI::new();
    println!("ready");

    uci.execute();

    println!("Bye");
}
