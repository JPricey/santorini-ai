use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread,
    time::{Duration, Instant},
};

use crate::{
    board::SantoriniState,
    search::{NewBestMove, SearchState, search_with_state},
    transposition_table::TranspositionTable,
};

#[derive(Clone, Copy, Debug)]
pub enum EngineThreadState {
    Pending,
    Running,
}

#[derive(Clone)]
pub enum EngineThreadMessage {
    Compute(EngineThreadExecution),
}

#[derive(Clone)]
pub struct EngineThreadExecution {
    state: SantoriniState,
    stop_flag: Arc<AtomicBool>,
}

pub struct EngineThreadCtx {
    state: Arc<Mutex<EngineThreadState>>,
    receiver: Receiver<EngineThreadMessage>,
}

pub struct EngineThreadWrapper {
    thread: thread::JoinHandle<()>,
    active_execution: Option<EngineThreadExecution>,
    message_sender: Sender<EngineThreadMessage>,
}

impl EngineThreadWrapper {
    pub fn new() -> Self {
        let (sender, receiver) = channel();

        let engine_thread_ctx = EngineThreadCtx {
            state: Arc::new(Mutex::new(EngineThreadState::Pending)),
            receiver,
        };

        EngineThreadWrapper {
            message_sender: sender,
            active_execution: None,
            thread: thread::spawn(move || {
                Self::worker_thread_loop(engine_thread_ctx);
            }),
        }
    }

    fn worker_thread_loop(engine_thread_ctx: EngineThreadCtx) {
        loop {
            let mut state = engine_thread_ctx.state.lock().unwrap();
            *state = EngineThreadState::Pending;

            let Ok(msg) = engine_thread_ctx.receiver.recv() else {
                println!("EngineThread receiver received error");
                thread::sleep(Duration::from_millis(100));
                continue;
            };

            match msg {
                EngineThreadMessage::Compute(request) => {
                    *state = EngineThreadState::Running;

                    let mut search_state = SearchState {
                        tt: &mut TranspositionTable::new(),
                        stop_flag: request.stop_flag.clone(),
                        new_best_move_callback: Box::new(|new_best_move: NewBestMove| {
                            println!("New best move found: {:?}", new_best_move);
                        }),
                        last_fully_completed_depth: 0,
                    };

                    Self::execute_search(request.state, &mut search_state);
                }
            }
        }
    }

    fn execute_search(state: SantoriniState, search_state: &mut SearchState) {
        search_with_state(search_state, &state);
    }

    pub fn start_execution(search_state: &mut SearchState) {}

    pub fn stop(&mut self) -> Result<NewBestMove, String> {
        if let Some(active_execution) = &self.active_execution {
            active_execution.stop_flag.store(true, Ordering::Relaxed);

            todo!()
            // Ok(())
        } else {
            Err("Attempted to stop, but no active execution".to_owned())
        }
    }

    pub fn end(&mut self) {
        todo!()
    }
}

pub struct Engine {
    tt: Arc<Mutex<TranspositionTable>>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            tt: Arc::new(Mutex::new(TranspositionTable::new())),
        }
    }

    pub fn search_for_duration(
        &mut self,
        state: &SantoriniState,
        duration_secs: f32,
    ) -> NewBestMove {
        let tt = self.tt.clone();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let search_flag = stop_flag.clone();

        let outcome: Arc<Mutex<Option<NewBestMove>>> = Arc::new(Mutex::new(None));
        let outcome_shared = outcome.clone();

        let state = state.clone();
        let engine_thread = thread::spawn(move || {
            let mut tt = tt.lock().unwrap();
            tt.reset();
            let best_move: Rc<RefCell<Option<NewBestMove>>> = Rc::new(RefCell::new(None));
            let orig_best_move = best_move.clone();

            let start_time = std::time::Instant::now();
            let clone_time = start_time.clone();

            let mut search_state = SearchState {
                tt: &mut tt,
                stop_flag: search_flag,
                new_best_move_callback: Box::new(move |new_best_move: NewBestMove| {
                    let elapsed = clone_time.elapsed();
                    println!("{:.2}s: {:?}", elapsed.as_secs_f32(), new_best_move);
                    *best_move.borrow_mut() = Some(new_best_move);
                }),
                last_fully_completed_depth: 0,
            };

            search_with_state(&mut search_state, &state);

            let mut outcome_lock = outcome_shared.lock().unwrap();
            *outcome_lock = Some(orig_best_move.take().unwrap());
        });
        let start_time = std::time::Instant::now();
        let end_time = start_time + Duration::from_secs_f32(duration_secs);

        loop {
            if Instant::now() >= end_time {
                stop_flag.store(true, Ordering::Relaxed);
                engine_thread.join().unwrap();
                break;
            }

            if engine_thread.is_finished() {
                break;
            }

            let time_till_end = end_time - Instant::now();
            let max_wait = Duration::from_millis(100);
            let wait_for_duration = std::cmp::min(time_till_end, max_wait);

            thread::sleep(wait_for_duration);
        }

        outcome.lock().unwrap().clone().unwrap()
    }
}
