use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use crate::{
    board::FullGameState,
    search::{BestSearchResult, SearchContext, get_past_win_search_terminator, negamax_search},
    search_terminators::{
        AndSearchTerminator, OrSearchTerminator, StaticMaxDepthSearchTerminator,
        StaticNodesVisitedSearchTerminator, StopFlagSearchTerminator,
    },
    transposition_table::TranspositionTable,
};

type EachMoveCallback = Arc<dyn Fn(BestSearchResult) + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineThreadState {
    Starting,
    Pending,
    Running,
}

#[derive(Clone)]
pub enum EngineThreadMessage {
    Compute(EngineThreadExecution),
    End,
}

#[derive(Clone)]
pub struct EngineThreadExecution {
    state: FullGameState,
    stop_flag: Arc<AtomicBool>,
    best_move: Arc<Mutex<Option<BestSearchResult>>>,
    new_best_move_sender: Sender<BestSearchResult>,
    each_move_callback: Option<EachMoveCallback>,
}

pub struct EngineThreadCtx {
    worker_state: Arc<Mutex<EngineThreadState>>,
    receiver: Receiver<EngineThreadMessage>,
}

pub struct EngineThreadWrapper {
    is_ending: bool,
    thread: Option<JoinHandle<()>>,
    active_execution: Option<EngineThreadExecution>,
    request_sender: Sender<EngineThreadMessage>,
    worker_state: Arc<Mutex<EngineThreadState>>,
}

#[allow(dead_code)]
type DatagenStaticSearchTerminator = OrSearchTerminator<
    StaticNodesVisitedSearchTerminator<700_000>,
    AndSearchTerminator<
        StaticMaxDepthSearchTerminator<8>,
        StaticNodesVisitedSearchTerminator<350_000>,
    >,
>;

// type EngineStaticSearchTerminator = DatagenStaticSearchTerminator;

impl EngineThreadWrapper {
    pub fn new() -> Self {
        let (sender, receiver) = channel::<EngineThreadMessage>();
        let worker_state = Arc::new(Mutex::new(EngineThreadState::Starting));

        let engine_thread_ctx = EngineThreadCtx {
            worker_state: worker_state.clone(),
            receiver,
        };

        EngineThreadWrapper {
            is_ending: false,
            request_sender: sender,
            active_execution: None,
            worker_state: worker_state.clone(),
            thread: Some(thread::spawn(move || {
                Self::worker_thread_loop(engine_thread_ctx);
            })),
        }
    }

    fn worker_thread_loop(engine_thread_ctx: EngineThreadCtx) {
        let mut transposition_table = TranspositionTable::new();

        loop {
            {
                let mut worker_state = engine_thread_ctx.worker_state.lock().unwrap();
                *worker_state = EngineThreadState::Pending;
            }

            let Ok(msg) = engine_thread_ctx.receiver.recv() else {
                eprintln!("EngineThread receiver received error");
                thread::sleep(Duration::from_millis(100));
                continue;
            };

            match msg {
                EngineThreadMessage::Compute(request) => {
                    let best_move_mutex = request.best_move;
                    let best_move_sender = request.new_best_move_sender;
                    {
                        let mut worker_state = engine_thread_ctx.worker_state.lock().unwrap();
                        *worker_state = EngineThreadState::Running;
                    }

                    let mut search_state = SearchContext {
                        tt: &mut transposition_table,
                        new_best_move_callback: Box::new(move |new_best_move: BestSearchResult| {
                            let mut best_move_handle = best_move_mutex.lock().unwrap();
                            *best_move_handle = Some(new_best_move.clone());

                            if let Some(each_move_callback) = &request.each_move_callback {
                                each_move_callback(new_best_move.clone());
                            }

                            let _ = best_move_sender.send(new_best_move.clone());
                        }),
                        terminator: StopFlagSearchTerminator::new(request.stop_flag.clone()),
                    };

                    negamax_search(
                        &mut search_state,
                        request.state.clone(),
                        get_past_win_search_terminator(),
                    );

                    request.stop_flag.store(true, Ordering::Relaxed);
                }
                EngineThreadMessage::End => {
                    break;
                }
            }
        }
    }

    fn clear_active_state_if_already_stopped(&mut self) {
        if let Some(active_execution) = &self.active_execution {
            if active_execution.stop_flag.load(Ordering::Relaxed) {
                self.active_execution = None;
                self.spin_for_pending_state();
            }
        }
    }

    pub fn spin_for_pending_state(&self) {
        loop {
            {
                let worker_state = self.worker_state.lock().unwrap().clone();
                if worker_state == EngineThreadState::Pending {
                    break;
                }
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    pub fn start_search(
        &mut self,
        state: &FullGameState,
        each_move_callback: Option<EachMoveCallback>,
    ) -> Result<Receiver<BestSearchResult>, String> {
        if self.is_ending {
            panic!("Tried to start a search when engine thread is already ended");
        }

        self.clear_active_state_if_already_stopped();

        if self.active_execution.is_some() {
            return Err("A search is already in progress".to_owned());
        }

        let (sender, receiver) = channel();

        let compute_request = EngineThreadExecution {
            state: state.clone(),
            stop_flag: Arc::new(AtomicBool::new(false)),
            best_move: Arc::new(Mutex::new(None)),
            new_best_move_sender: sender,
            each_move_callback,
        };

        self.request_sender
            .send(EngineThreadMessage::Compute(compute_request.clone()))
            .map_err(|err| format!("{}", err))?;

        self.active_execution = Some(compute_request.clone());

        Ok(receiver)
    }

    pub fn stop(&mut self) -> Result<BestSearchResult, String> {
        if let Some(active_execution) = &self.active_execution.take() {
            active_execution.stop_flag.store(true, Ordering::Relaxed);

            let result_state = active_execution.best_move.lock().unwrap();
            result_state
                .clone()
                .ok_or_else(|| format!("Search returned no results. {:?}", active_execution.state))
        } else {
            Err("Attempted to stop, but no active execution".to_owned())
        }
    }

    pub fn end(&mut self) {
        self.is_ending = true;
        self.request_sender.send(EngineThreadMessage::End).unwrap();
        let _ = self.stop();
        self.thread.take().map(JoinHandle::join).unwrap().unwrap();
    }

    pub fn search_for_duration(
        &mut self,
        state: &FullGameState,
        duration_secs: f32,
    ) -> Result<BestSearchResult, String> {
        let _message_receiver = self.start_search(state, None)?;

        let start_time = Instant::now();
        let end_time = start_time + Duration::from_secs_f32(duration_secs);

        loop {
            let is_over_on_time = Instant::now() >= end_time;
            let is_already_over = if let Some(active_execution) = &self.active_execution {
                active_execution.stop_flag.load(Ordering::Relaxed)
            } else {
                true
            };

            if is_over_on_time || is_already_over {
                let result = self.stop();

                self.spin_for_pending_state();

                return result;
            }

            let time_till_end = end_time - Instant::now();
            let max_wait = Duration::from_millis(100);
            let wait_for_duration = std::cmp::min(time_till_end, max_wait);

            thread::sleep(wait_for_duration);
        }
    }
}
