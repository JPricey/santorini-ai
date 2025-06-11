use std::{sync::mpsc, thread};

/*
// Multi threaded UCI loop, takes input in a parallel thread
fn uci_loop(&mut self) {
    let (tx, rx) = mpsc::channel();

    // Start the input thread
    let io_thread = thread::spawn(move || {
        loop {
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();

            if input.trim() == "stop" {
                STOP.store(true, Ordering::Relaxed);
                continue;
            }
            if input.trim() == "quit" {
                STOP.store(true, Ordering::Relaxed); //stop engine then pass quit to uci loop
            }
            tx.send(input).unwrap();
        }
    });

    // Start the UCI loop in the main thread
    self.uci_handle(&rx);

    io_thread.join().unwrap();
}
*/

fn main() {
    println!("Hello world")
}
