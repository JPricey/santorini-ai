mod dropdown;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use eframe::{
    egui::{
        self, Color32, Key, Label, Modifiers, Rangef, Response, RichText, Stroke, Ui, UiBuilder,
        mutex::Mutex,
    },
    epaint::EllipseShape,
};
use santorini_core::{
    bitboard::BitBoard,
    board::FullGameState,
    direction::maybe_wind_direction_to_ui_square,
    engine::EngineThreadWrapper,
    fen::{game_state_to_fen, parse_fen},
    gods::{ALL_GODS_BY_ID, GameStateWithAction, GodName, PartialAction, WIP_GODS},
    player::Player,
    pretty_board::{game_state_with_partial_actions, get_acting_player},
    search::{BestMoveTrigger, BestSearchResult, WINNING_SCORE, WINNING_SCORE_BUFFER},
    square::Square,
    utils::sigmoid,
};

fn main() -> Result<(), eframe::Error> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Santorini Analysis Engine")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([600.0, 220.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/dome.png")[..])
                    .unwrap(),
            ),
        ..Default::default()
    };

    eframe::run_native(
        "Santorini Analysis Engine",
        native_options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}

#[derive(Default, Debug, Eq, PartialEq, Clone, Copy)]
enum EditMode {
    #[default]
    Play,
    EditHeights,
    EditWorkers,
}

const WORKER_ROTATION: [Option<Player>; 3] = [None, Some(Player::One), Some(Player::Two)];

const SHORTCUT_REDO_TURN: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::NONE, Key::ArrowDown);
const SHORTCUT_ENGINE_MOVE: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::NONE, Key::ArrowUp);
const SHORTCUT_STATE_FORWARD: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::NONE, Key::ArrowRight);
const SHORTCUT_STATE_BACKWARD: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::NONE, Key::ArrowLeft);
const SHORTCUT_CHANGE_MODE: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::NONE, Key::M);
const SHORTCUT_AUTOPLAY_START: egui::KeyboardShortcut =
    egui::KeyboardShortcut::new(Modifiers::NONE, Key::A);

fn shortcut_text(shortcut: egui::KeyboardShortcut) -> String {
    shortcut.format(&egui::ModifierNames::SYMBOLS, false)
}

fn shortcut_text_long(shortcut: egui::KeyboardShortcut) -> String {
    shortcut.format(&egui::ModifierNames::NAMES, false)
}

fn shortcut_button(
    ui: &mut Ui,
    button_text: &str,
    tooltip: &str,
    shortcut: egui::KeyboardShortcut,
) -> Response {
    ui.add(egui::Button::new(button_text).shortcut_text(shortcut_text(shortcut)))
        .on_hover_text(format!(
            "{tooltip} Shortcut: {}",
            shortcut_text_long(shortcut)
        ))
}

fn next_worker_rotation(current: Option<Player>, forward: bool) -> Option<Player> {
    let current_idx = WORKER_ROTATION.iter().position(|x| *x == current).unwrap() as i32;
    let new_idx = if forward {
        (current_idx + 1).rem_euclid(WORKER_ROTATION.len() as i32)
    } else {
        (current_idx - 1).rem_euclid(WORKER_ROTATION.len() as i32)
    };
    WORKER_ROTATION[new_idx as usize]
}

fn square_for_interaction(action: &PartialAction) -> Option<Square> {
    match action {
        PartialAction::PlaceWorker(x)
        | PartialAction::SetFemaleWorker(x)
        | PartialAction::SelectWorker(x)
        | PartialAction::Build(x)
        | PartialAction::SetTalusPosition(x)
        | PartialAction::Dome(x)
        | PartialAction::HeroPower(x)
        | PartialAction::Destroy(x) => Some(*x),
        PartialAction::MoveWorker(data) => Some(data.dest),
        PartialAction::ForceOpponentWorker(from, _) => Some(*from),
        PartialAction::SetWindDirection(d) => Some(maybe_wind_direction_to_ui_square(*d)),
        PartialAction::NoMoves | PartialAction::EndTurn => None,
    }
}

fn partial_action_color(action: &PartialAction) -> egui::Color32 {
    match action {
        PartialAction::PlaceWorker(_) => egui::Color32::YELLOW,
        PartialAction::SetFemaleWorker(_) => egui::Color32::LIGHT_RED,
        PartialAction::SelectWorker(_) => egui::Color32::BLUE,
        PartialAction::MoveWorker(_) => egui::Color32::DARK_GREEN,
        PartialAction::ForceOpponentWorker(_, _) => egui::Color32::GREEN,
        PartialAction::Build(_) => egui::Color32::RED,
        PartialAction::Dome(_) => egui::Color32::PURPLE,
        PartialAction::Destroy(_) => egui::Color32::ORANGE,
        PartialAction::SetTalusPosition(_) => egui::Color32::PURPLE,
        PartialAction::EndTurn => egui::Color32::WHITE,
        PartialAction::HeroPower(_) => egui::Color32::GREEN,
        PartialAction::NoMoves => egui::Color32::BLACK,
        PartialAction::SetWindDirection(maybe_direction) => match maybe_direction {
            None => egui::Color32::GRAY,
            _ => egui::Color32::MAGENTA,
        },
    }
}

fn partial_action_label(action: &PartialAction) -> String {
    match action {
        PartialAction::PlaceWorker(_) => "Place Worker".to_string(),
        PartialAction::SetFemaleWorker(_) => "Pick Female Worker".to_string(),
        PartialAction::SelectWorker(_) => "Select Worker".to_string(),
        PartialAction::MoveWorker(_) => "Move Worker".to_string(),
        PartialAction::ForceOpponentWorker(_, _) => "Force Opponent Worker".to_string(),
        PartialAction::Build(_) => "Build".to_string(),
        PartialAction::Dome(_) => "Add Dome".to_string(),
        PartialAction::Destroy(_) => "Destroy Block".to_string(),
        PartialAction::SetTalusPosition(_) => "Place Talus".to_string(),
        PartialAction::EndTurn => "End Turn".to_string(),
        PartialAction::NoMoves => "Pass".to_string(),
        PartialAction::HeroPower(_) => "Use Hero Power".to_string(),
        PartialAction::SetWindDirection(maybe_direction) => match maybe_direction {
            None => "No Wind Direction".to_string(),
            Some(direction) => format!("Prevent Movements: {:?}", direction),
        },
    }
}

struct EngineThinkingState {
    state: FullGameState,
    engine_messages: Vec<(BestSearchResult, Duration)>,
    start_time: Instant,
}

impl EngineThinkingState {
    pub fn new(state: FullGameState) -> Self {
        Self {
            state,
            engine_messages: Vec::new(),
            start_time: Instant::now(),
        }
    }

    pub fn reset(&mut self, state: FullGameState) {
        self.state = state;
        self.engine_messages.clear();
        self.start_time = Instant::now();
    }

    pub fn add_message(&mut self, state: &FullGameState, message: BestSearchResult) {
        if state == &self.state && message.trigger != BestMoveTrigger::Seed {
            self.engine_messages
                .push((message, self.start_time.elapsed()));
        }
    }
}

struct MyApp {
    state: FullGameState,
    state_history: Vec<FullGameState>,
    state_idx: usize,
    editor_fen_string: String,
    editor_fen_error: Option<String>,
    next_states: Vec<GameStateWithAction>,
    current_actions: Vec<PartialAction>,
    available_next_actions: Vec<PartialAction>,
    engine: EngineThreadWrapper,
    engine_thinking: Arc<Mutex<EngineThinkingState>>,

    // Edit mode
    edit_mode: EditMode,
    may_arrow_shortcuts: bool,
    may_show_wip_gods: bool,

    // Autoplay
    is_autoplay_enabled: bool,
    is_autoplay_per_player: [bool; 2],
    autoplay_speed_secs: f32,
    autoplay_last_status_change_time: Instant,

    // God selector buf
    god_selector_bufs: [String; 2],
}

impl MyApp {
    pub fn update_state(&mut self, state: FullGameState) {
        assert_eq!(self.state, self.state_history[self.state_idx]);

        let mut is_playable = true;
        self.autoplay_last_status_change_time = Instant::now();

        if let Err(err) = state.validation_err() {
            self.editor_fen_error = Some(err);

            self.is_autoplay_enabled = false;
            is_playable = false;

            if state.representation_err().is_err() {
                return;
            }
        } else {
            self.editor_fen_error = None;
        }

        if state.get_winner().is_some() {
            self.is_autoplay_enabled = false;
            is_playable = false;
        }

        self.state = state.clone();
        if self.state_history.get(self.state_idx) == Some(&self.state) {
            // noop
        } else if self.state_history.get(self.state_idx + 1) == Some(&self.state) {
            self.state_idx = self.state_idx + 1;
        } else {
            self.state_history.truncate(self.state_idx + 1);
            self.state_history.push(self.state.clone());
            self.state_idx += 1;
        }

        self.copy_editor_fen();
        self.compute_next_states(is_playable);
        self.engine_thinking.lock().reset(state.clone());
        let engine_thinking_clone = self.engine_thinking.clone();
        let state_clone = state.clone();

        let callback = Arc::new(move |new_best_move: BestSearchResult| {
            engine_thinking_clone
                .lock()
                .add_message(&state_clone, new_best_move);
        });

        let _ = self.engine.stop();

        if is_playable {
            let res = self.engine.start_search(&state, Some(callback));
            if let Err(err) = res {
                panic!("Failed to start search in state {:?}: {}", state, err);
            }
        }
    }

    pub fn compute_next_states(&mut self, is_playable: bool) {
        self.current_actions.clear();
        self.available_next_actions.clear();

        if is_playable {
            self.next_states = self.state.get_next_states_interactive();
            self.compute_next_actions();
        } else {
            self.next_states.clear();
        }
    }

    pub fn compute_next_actions(&mut self) {
        let mut update_next_state: Option<FullGameState> = None;
        self.available_next_actions.clear();

        for possible_next_state in &self.next_states {
            if possible_next_state
                .actions
                .starts_with(&self.current_actions)
            {
                if let Some(next_action) =
                    possible_next_state.actions.get(self.current_actions.len())
                {
                    if !self.available_next_actions.contains(next_action) {
                        self.available_next_actions.push(*next_action);
                    }
                } else {
                    update_next_state = Some(possible_next_state.state.clone());
                }
            }
        }

        if let Some(state) = update_next_state {
            if self.available_next_actions.is_empty() {
                self.update_state(state);
            } else {
                self.available_next_actions.push(PartialAction::EndTurn);
            }
        }
    }

    pub fn get_action_for_square(&mut self, square: Square) -> Option<PartialAction> {
        if self.available_next_actions == vec![PartialAction::NoMoves] {
            return Some(PartialAction::NoMoves);
        }

        let mut has_end = false;

        for action in &self.available_next_actions {
            if Some(square) == square_for_interaction(action) {
                return Some(action.clone());
            } else if action == &PartialAction::EndTurn {
                has_end = true;
            }
        }

        if has_end {
            Some(PartialAction::EndTurn)
        } else {
            None
        }
    }

    pub fn accept_action(&mut self, action: PartialAction) {
        if self.available_next_actions.contains(&action) {
            if action == PartialAction::EndTurn {
                for next_state in &self.next_states {
                    if next_state.actions == self.current_actions {
                        self.update_state(next_state.state.clone());
                        return;
                    }
                }
            } else {
                self.current_actions.push(action);
                self.compute_next_actions();
            }
        }
    }

    pub fn try_set_editor_fen(&mut self) {
        self.is_autoplay_enabled = false;
        match parse_fen(&self.editor_fen_string) {
            Ok(new_state) => self.update_state(new_state),
            Err(err_str) => self.editor_fen_error = Some(err_str),
        }
    }

    pub fn copy_editor_fen(&mut self) {
        self.editor_fen_string = game_state_to_fen(&self.state);
    }

    pub fn clear_board(&mut self) {
        let state = FullGameState::new_empty_state(
            self.state.gods[0].god_name,
            self.state.gods[1].god_name,
        );
        self.is_autoplay_enabled = false;
        self.update_state(state);
    }

    pub fn try_engine_move(&mut self) {
        let engine_state = self.engine_thinking.lock();
        if engine_state.state == self.state {
            if let Some(last_engine_move) = engine_state.engine_messages.last().clone() {
                let next_state = last_engine_move.0.child_state.clone();
                drop(engine_state);
                self.update_state(next_state);
            }
        }
    }

    pub fn try_engine_move_if_end_of_line(&mut self) {
        let engine_state = self.engine_thinking.lock();
        if engine_state.state == self.state {
            if let Some(last_engine_move) = engine_state.engine_messages.last().clone() {
                if last_engine_move.0.trigger == BestMoveTrigger::EndOfLine {
                    let next_state = last_engine_move.0.child_state.clone();
                    drop(engine_state);
                    self.update_state(next_state);
                }
            }
        }
    }

    pub fn clear_actions_for_edit(&mut self) {
        self.current_actions.clear();
        self.available_next_actions.clear();
    }

    pub fn clear_actions(&mut self) {
        self.current_actions.clear();
        self.compute_next_actions();
    }

    pub fn try_forward_state(&mut self) {
        if let Some(state) = self.state_history.get(self.state_idx + 1) {
            self.is_autoplay_enabled = false;
            self.state = state.clone();
            self.state_idx += 1;
            self.update_state(state.clone());
        }
    }

    pub fn try_back_state(&mut self) {
        if self.state_idx > 0 {
            if let Some(state) = self.state_history.get(self.state_idx - 1) {
                self.is_autoplay_enabled = false;
                self.state = state.clone();
                self.state_idx -= 1;
                self.update_state(state.clone());
            }
        }
    }

    pub fn reset_after_mode_change(&mut self) {
        if self.edit_mode == EditMode::Play {
            self.clear_actions();
        } else {
            self.clear_actions_for_edit();
        }
    }

    pub fn rotate_through_mode(&mut self) {
        self.edit_mode = match self.edit_mode {
            EditMode::Play => EditMode::EditHeights,
            EditMode::EditHeights => EditMode::EditWorkers,
            EditMode::EditWorkers => EditMode::Play,
        };
        self.reset_after_mode_change();
    }
}

impl Default for MyApp {
    fn default() -> Self {
        let default_state = FullGameState::new_empty_state(GodName::Mortal, GodName::Mortal);
        let mut result = Self {
            state: default_state.clone(),
            state_history: vec![default_state.clone()],
            state_idx: 0,
            editor_fen_string: game_state_to_fen(&default_state),
            editor_fen_error: None,
            next_states: Default::default(),
            current_actions: Default::default(),
            available_next_actions: Default::default(),
            engine: EngineThreadWrapper::new(),
            engine_thinking: Arc::new(Mutex::new(EngineThinkingState::new(default_state.clone()))),
            edit_mode: Default::default(),
            may_arrow_shortcuts: Default::default(),
            may_show_wip_gods: Default::default(),
            // Autoplay
            is_autoplay_enabled: false,
            is_autoplay_per_player: [true; 2],
            autoplay_speed_secs: 1.0,
            autoplay_last_status_change_time: Instant::now(),

            god_selector_bufs: Default::default(),
        };

        result.update_state(result.state.clone());

        result
    }
}

struct GameGrid<'a> {
    app: &'a mut MyApp,
}

impl<'a> GameGrid<'a> {
    fn _edit_workers(&mut self, square: Square, forward: bool) {
        let mut new_state = self.app.state.clone();

        for _ in 0..2 {
            let current_worker = new_state.board.get_worker_at(square);
            let new_worker = next_worker_rotation(current_worker, forward);

            if let Some(current_worker) = current_worker {
                new_state
                    .board
                    .worker_xor(current_worker, BitBoard::as_mask(square));
            }

            if let Some(new_worker) = new_worker {
                new_state
                    .board
                    .worker_xor(new_worker, BitBoard::as_mask(square));
            }
            if new_state.representation_err().is_ok() {
                break;
            }
        }

        self.app.update_state(new_state);
    }
}

impl<'a> egui::Widget for GameGrid<'a> {
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        if self.app.is_autoplay_enabled
            && self.app.is_autoplay_per_player
                [self.app.state.get_current_player_consider_placement_mode() as usize]
        {
            let elapsed_secs = self
                .app
                .autoplay_last_status_change_time
                .elapsed()
                .as_secs_f32();
            if elapsed_secs > self.app.autoplay_speed_secs {
                self.app.try_engine_move();
            } else if elapsed_secs > 0.1 {
                self.app.try_engine_move_if_end_of_line();
            }
        }

        let desired_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        let full_width = rect.width();
        let full_height = rect.height();

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(132, 206, 235));

        let max_dim = full_height.min(full_width) * 0.99;
        let legend_dim = max_dim * 0.05;

        let full_grid_dim = max_dim - legend_dim;

        let full_float_x = (full_width - max_dim) / 2.0;
        let full_float_y = (full_height - max_dim) / 2.0;

        let full_float_pos = rect.min + egui::Vec2::new(full_float_x, full_float_y);
        let grid_float_pos = full_float_pos + egui::Vec2::new(legend_dim, 0.0);

        let bound_dim = full_grid_dim / 5.0;
        let size = egui::vec2(bound_dim, bound_dim);

        let render_state =
            game_state_with_partial_actions(&self.app.state, &self.app.current_actions);

        let (p1_tokens, p2_tokens) = self.app.state.get_token_squares();

        for r in 0..5 {
            for c in 0..5 {
                let square = Square::from_col_row(c, r);
                let ui_action = if self.app.edit_mode == EditMode::Play {
                    self.app.get_action_for_square(square)
                } else {
                    None
                };

                let token = if p1_tokens.contains_square(square) {
                    Some(Player::One)
                } else if p2_tokens.contains_square(square) {
                    Some(Player::Two)
                } else {
                    None
                };

                let square_space = SquareSpace {
                    worker: render_state.board.get_worker_at(square),
                    token,
                    height: render_state.board.get_height(square),
                    dim: bound_dim,
                    ui_action: ui_action.clone(),
                };

                let point =
                    grid_float_pos + egui::Vec2::new(c as f32 * bound_dim, r as f32 * bound_dim);

                let mut placed_square =
                    ui.put(egui::Rect::from_min_size(point, size), square_space);
                if let Some(ui_action) = ui_action {
                    placed_square = placed_square.on_hover_text(partial_action_label(&ui_action));
                }

                if placed_square.clicked() {
                    match self.app.edit_mode {
                        EditMode::Play => {
                            if let Some(action) = ui_action {
                                self.app.accept_action(action);
                            }
                        }
                        EditMode::EditHeights => {
                            let mut new_state = self.app.state.clone();

                            for _ in 0..2 {
                                let current_height = new_state.board.get_height(square);
                                if current_height == 4 {
                                    new_state.board.undome(square, 0);
                                } else {
                                    new_state.board.build_up(square);
                                }
                                if new_state.representation_err().is_ok() {
                                    break;
                                }
                            }
                            self.app.update_state(new_state);
                        }
                        EditMode::EditWorkers => {
                            self._edit_workers(square, true);
                        }
                    }
                } else if placed_square.secondary_clicked() {
                    match self.app.edit_mode {
                        EditMode::EditHeights => {
                            let mut new_state = self.app.state.clone();

                            for _ in 0..2 {
                                let current_height = new_state.board.get_height(square);
                                if current_height == 0 {
                                    new_state.board.dome_up(square);
                                } else if current_height == 4 {
                                    new_state.board.undome(square, 3);
                                } else {
                                    new_state.board.unbuild(square);
                                }
                                if new_state.representation_err().is_ok() {
                                    break;
                                }
                            }
                            self.app.update_state(new_state);
                        }
                        EditMode::EditWorkers => {
                            self._edit_workers(square, false);
                        }
                        _ => {}
                    }
                }
            }
        }

        let legend_font = egui::FontId::monospace(max_dim / 24.0);

        for r in 0..5 {
            let text = format!("{}", 5 - r);
            let text_pos =
                full_float_pos + egui::vec2(legend_dim / 2.0, (r as f32 + 0.5) * bound_dim);
            painter.text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                text,
                legend_font.clone(),
                egui::Color32::BLACK,
            );
        }

        for c in 0..5 {
            let text = format!("{}", (b'A' + c as u8) as char);
            let text_pos = full_float_pos
                + egui::vec2(
                    legend_dim + (c as f32 + 0.5) * bound_dim,
                    max_dim - legend_dim / 2.0,
                );
            painter.text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                text,
                legend_font.clone(),
                egui::Color32::BLACK,
            );
        }

        response
    }
}

struct SquareSpace {
    dim: f32,
    worker: Option<Player>,
    token: Option<Player>,
    height: usize,
    ui_action: Option<PartialAction>,
}

fn unit_color(player: Player) -> egui::Color32 {
    match player {
        Player::One => egui::Color32::LIGHT_GRAY,
        Player::Two => egui::Color32::from_rgb(23, 23, 23),
    }
}

impl egui::Widget for SquareSpace {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let width = self.dim;
        let height = width;
        let (rect, mut response) =
            ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
        let mut painter = ui.painter_at(rect);

        painter.rect_filled(rect, 1.0, egui::Color32::LIGHT_GREEN);
        painter.rect(
            rect,
            0.0,
            egui::Color32::LIGHT_GREEN,
            Stroke::new(2.0, Color32::DARK_GRAY),
            egui::StrokeKind::Middle,
        );

        let box_height = height / 5.0;
        let box_bot_offset = height * 0.05;

        for h in (0..self.height).rev() {
            let color = match h {
                0 => egui::Color32::LIGHT_GRAY,
                1 => egui::Color32::GRAY,
                2 => egui::Color32::DARK_GRAY,
                3 => egui::Color32::from_rgb(14, 17, 161),
                _ => unreachable!(),
            };

            let hf = h as f32;
            let box_full_width = width * 0.90 * f32::cos(hf / 4.0);
            let box_width_margin = (width - box_full_width) / 2.0;
            let box_bot = box_bot_offset + (1.0 + hf) * box_height;
            let stroke = Stroke::new(1.0, egui::Color32::BLACK);

            if h == 3 {
                let box_bot = box_bot - box_height;
                let box_full_width = box_full_width - 0.1;
                let center = rect.min + egui::vec2(width / 2.0, height - box_bot);
                let radius = egui::vec2(box_full_width / 2.0, height * 0.2);

                let dome = EllipseShape {
                    center,
                    radius,
                    fill: color,
                    stroke,
                };
                painter.add(dome);
            } else {
                let box_rect = egui::Rect::from_min_size(
                    rect.min + egui::vec2(box_width_margin, height - box_bot),
                    egui::vec2(box_full_width, box_height),
                );

                painter.rect(
                    box_rect,
                    width / 50.0,
                    color,
                    stroke,
                    egui::StrokeKind::Middle,
                );
            }
        }

        let player_rad = width / 7.0;
        let circle_h = player_rad
            + 0.2_f32.max(self.height as f32) * box_height
            + height * 0.02
            + box_bot_offset;
        let circle_center = egui::pos2(rect.center().x, rect.min.y + height - circle_h);

        if let Some(player) = self.worker {
            painter.circle(
                circle_center,
                player_rad,
                unit_color(player),
                Stroke::new(width / 128.0, egui::Color32::BLACK),
            );
        }

        if let Some(player) = self.token {
            let token_side_len = player_rad * 2.0;
            let triangle_base_x = match player {
                Player::One => rect.min.x + rect.width() * (1.0 / 3.0),
                Player::Two => rect.min.x + rect.width() * (2.0 / 3.0),
            };
            let triangle_base_y = circle_center.y + token_side_len * 0.5;
            let mut points = Vec::with_capacity(3);
            points.push(egui::pos2(
                triangle_base_x - token_side_len / 2.0,
                triangle_base_y,
            ));
            points.push(egui::pos2(
                triangle_base_x + token_side_len / 2.0,
                triangle_base_y,
            ));
            points.push(egui::pos2(
                triangle_base_x,
                triangle_base_y - token_side_len * 0.866,
            ));

            painter.add(egui::Shape::convex_polygon(
                points,
                unit_color(player),
                Stroke::new(width / 168.0, egui::Color32::BLACK),
            ));
        }

        painter.set_opacity(0.4);
        if let Some(ui_action) = self.ui_action {
            let color = partial_action_color(&ui_action);
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            painter.rect_filled(rect.scale_from_center(0.95), width / 25.0, color);
        }

        response
    }
}

struct GodChanger<'a> {
    app: &'a mut MyApp,
    player: Player,
}

fn ordered_god_names() -> Vec<GodName> {
    let mut god_names: Vec<GodName> = ALL_GODS_BY_ID.iter().map(|g| g.god_name).collect();
    god_names.sort_by_key(|g| {
        (
            if *g == GodName::Mortal { 0 } else { 1 },
            format!("{:?}", g),
        )
    });
    god_names
}

impl<'a> egui::Widget for GodChanger<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let player_id = self.player as usize;

        let mut selected = self.app.state.gods[player_id].god_name;
        let before = selected;
        let inner = ui.horizontal(|ui| {
            let text = match self.player {
                Player::One => "P1 God:",
                Player::Two => "P2 God:",
            };
            ui.label(text);

            let ordered_gods = ordered_god_names();
            let available_gods_iter = ordered_gods
                .iter()
                .cloned()
                .filter(|g| self.app.may_show_wip_gods || !WIP_GODS.contains(&g))
                .map(|g| g);

            ui.add(dropdown::DropdownComboBox::<GodName, _, _>::new(
                text.to_string(),
                &mut self.app.god_selector_bufs[self.player as usize],
                available_gods_iter,
                &mut selected,
                |god_name| {
                    if WIP_GODS.contains(god_name) {
                        format!("{:?} (WIP)", god_name)
                    } else {
                        format!("{:?}", god_name)
                    }
                },
            ))
        });

        if selected != before {
            let mut new_state = self.app.state.clone();
            new_state.gods[player_id] = selected.to_power();
            new_state.board.god_data[player_id] = 0;
            new_state.recalculate_internals();
            self.app.update_state(new_state);
        }

        inner.inner
    }
}

struct EvalBar<'a> {
    app: &'a MyApp,
}

impl<'a> egui::Widget for EvalBar<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut eval_for_p1 = 0;
        if let Some(winner) = self.app.state.get_winner() {
            eval_for_p1 = match winner {
                Player::One => 10_000,
                Player::Two => -10_000,
            }
        } else {
            let engine = self.app.engine_thinking.lock();
            let active_player = engine.state.get_current_player_consider_placement_mode();
            if let Some(message) = engine.engine_messages.last() {
                eval_for_p1 = match active_player {
                    Player::One => message.0.score,
                    Player::Two => -message.0.score,
                }
            }
        }

        let desired_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
        let full_width = rect.width();
        let full_height = rect.height();

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(23, 23, 23));

        let pct = sigmoid(eval_for_p1 as f32 / 400.0);

        let eval_height = pct * full_height;
        let eval_rect = egui::Rect::from_min_size(rect.min, egui::vec2(full_width, eval_height));

        painter.rect_filled(eval_rect, 0.0, egui::Color32::LIGHT_GRAY);

        painter.hline(
            Rangef::new(rect.min.x, rect.min.x + full_width),
            rect.min.y + full_height / 2.0,
            Stroke::new(1.0, egui::Color32::RED),
        );

        response
    }
}

struct PlayerInfo<'a> {
    player: Player,
    state: &'a FullGameState,
}

impl<'a> egui::Widget for PlayerInfo<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.set_min_height(60.0);

        let mut header_text = format!(
            "Player {:?} - {:?}",
            self.player, self.state.gods[self.player as usize].god_name
        );

        if let Some(winner) = self.state.get_winner() {
            if winner == self.player {
                header_text += " (Winner!)";
            }
        } else if get_acting_player(&self.state) == self.player {
            header_text += " (To Play)";
        }
        let resp = ui.heading(header_text);

        let god = self.state.gods[self.player as usize];
        if let Some(text) = god.pretty_stringify_god_data(&self.state.board, self.player) {
            ui.label(text);
        }

        if god.god_name == GodName::Morpheus {
            ui.add(Label::new(
                RichText::new(
                    "* Multiple Morpheus builds must start in the NW corner and progress clockwise",
                )
                .italics(),
            ));
        }

        resp
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::right("right_panel")
            .resizable(false)
            .exact_width(450.0)
            .show(ctx, |ui| {
                self.may_arrow_shortcuts = true;

                ui.style_mut().spacing.item_spacing = egui::vec2(8.0, 12.0);
                let available_size = ui.available_size();

                let scroll_area_height = available_size.y * 0.4;

                ui.scope_builder(UiBuilder::new(), |ui| {
                    ui.set_min_height(scroll_area_height);

                    egui::ScrollArea::vertical()
                        .min_scrolled_height(scroll_area_height)
                        .max_height(scroll_area_height)
                        .show(ui, |ui| {
                            egui::Grid::new("Moves").striped(true).show(ui, |ui| {
                                ui.label("Depth");
                                ui.label("Action");
                                ui.label("Score");
                                ui.label("Secs");
                                ui.label("Nodes");
                                ui.label("Type");
                                ui.end_row();

                                let rows = self.engine_thinking.lock().engine_messages.clone();
                                for row in rows.iter().rev() {
                                    let (msg, dur) = row;
                                    let score_str = match msg.score {
                                        x @ WINNING_SCORE_BUFFER.. => format!("Win in {}", WINNING_SCORE - x),
                                        x if x < -WINNING_SCORE_BUFFER => format!("Loss in {}", WINNING_SCORE + x),
                                        x => format!("{}", x),
                                    };

                                    ui.label(format!("{}", msg.depth));
                                    ui.label(msg.action_str.to_owned());
                                    ui.label(score_str);
                                    ui.label(format!("{:.2}", dur.as_secs_f32()));
                                    ui.label(format!("{}", msg.nodes_visited));
                                    ui.label(format!("{:?}", msg.trigger));
                                    ui.end_row();
                                }
                            });
                        });
                });

                ui.heading("Controls");
                ui.horizontal(|ui| {
                    if shortcut_button(
                        ui,
                        "Do Engine Move",
                        "Play the engine move.",
                        SHORTCUT_ENGINE_MOVE,
                    )
                    .clicked()
                    {
                        self.try_engine_move();
                    }

                    if shortcut_button(
                        ui,
                        "Restart Turn",
                        "Undo any actions taken this turn.",
                        SHORTCUT_REDO_TURN,
                    )
                    .clicked()
                    {
                        self.clear_actions();
                    }

                    if shortcut_button(ui, "Back", "Go back a turn", SHORTCUT_STATE_BACKWARD)
                        .clicked()
                    {
                        self.try_back_state();
                    }

                    if shortcut_button(ui, "Forward", "Go forward a turn", SHORTCUT_STATE_FORWARD)
                        .clicked()
                    {
                        self.try_forward_state();
                    }
                });

                ui.heading("State Settings");
                let fen = game_state_to_fen(&self.state);
                ui.label(fen);

                let fen_input = egui::TextEdit::singleline(&mut self.editor_fen_string)
                    .clip_text(false)
                    .desired_width(available_size.x);

                if ui.add(fen_input).has_focus() {
                    self.may_arrow_shortcuts = false;
                }

                ui.horizontal(|ui| {
                    if ui
                        .button("Set Position")
                        .on_hover_text("Set current position to the FEN in the editor above")
                        .clicked()
                    {
                        self.try_set_editor_fen();
                    }

                    if ui
                        .button("Reset Board")
                        .on_hover_text("Reset to starting positions")
                        .clicked()
                    {
                        self.clear_board();
                    }

                    if ui
                        .button("Swap Gods")
                        .on_hover_text("Swap the gods in this position")
                        .clicked()
                    {
                        let mut new_state = self.state.clone();
                        new_state.gods.swap(0, 1);
                        new_state.board.god_data.swap(0, 1);

                        new_state.recalculate_internals();
                        self.update_state(new_state);
                    }

                    if ui
                        .button("Swap Turns")
                        .on_hover_text("Swap whose turn it is")
                        .clicked()
                    {
                        let mut new_state = self.state.clone();
                        new_state.board.current_player = !new_state.board.current_player;
                        new_state.recalculate_internals();
                        self.update_state(new_state);
                    }

                    if ui
                        .button("Clear History")
                        .on_hover_text("Remove old move history")
                        .clicked()
                    {
                        self.state_history = self.state_history.split_off(self.state_idx);
                        self.state_idx = 0;
                    }
                });

                if let Some(fen_error) = &self.editor_fen_error {
                    ui.label(fen_error);
                }

                ui.horizontal(|ui| {
                    if ui.add(GodChanger {
                        app: self,
                        player: Player::One,
                    }).has_focus() {
                        self.may_arrow_shortcuts = false;
                    }

                    if ui.add(GodChanger {
                        app: self,
                        player: Player::Two,
                    }).has_focus() {
                        self.may_arrow_shortcuts = false;
                    }
                });

                if WIP_GODS.len() > 0 {
                    let wip_gods_string = WIP_GODS.iter()
                        .map(|g| format!("{:?}", g))
                        .collect::<Vec<String>>()
                        .join(", ");
                    ui.checkbox(&mut self.may_show_wip_gods, "Include WIP gods").on_hover_text(&format!("Some gods are WIP, meaning their move logic is supported, but the AI does not know how to evaluate their positions correctly. Check this box to include them in the gods picker. Includes: {}", wip_gods_string));
                }

                // MODES
                ui.heading("Modes") .on_hover_text(format!(
                    "Change UI mode between playing a game, edit board heights, or edit worker locations. Shortcut: {}",
                    shortcut_text_long(SHORTCUT_CHANGE_MODE)
                ));
                let before = self.edit_mode;
                ui.horizontal(|ui| {
                    ui.radio_value(&mut self.edit_mode, EditMode::Play, "Play");
                    ui.radio_value(&mut self.edit_mode, EditMode::EditHeights, "Edit Height")
                        .on_hover_text("Edit square heights on the game board");
                    ui.radio_value(&mut self.edit_mode, EditMode::EditWorkers, "Edit Worker")
                        .on_hover_text("Edit worker placements on the game board");
                });
                if before != self.edit_mode {
                    self.reset_after_mode_change();
                }

                // AUTOPLAY
                ui.horizontal(|ui| {
                    ui.heading("Autoplay");
                    ui.checkbox(&mut self.is_autoplay_enabled, "Enable Autoplay")
                        .on_hover_text(
                            &format!("When enabled, engine moves will automatically be made after the timeout. Shortcut: {}",
                                shortcut_text_long(SHORTCUT_AUTOPLAY_START)
                        ));
                });
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.is_autoplay_per_player[0], "Autoplay for Player 1").on_hover_text("Enable autoplay for Player 1");
                    ui.checkbox(&mut self.is_autoplay_per_player[1], "Autoplay for Player 2").on_hover_text("Enable autoplay for Player 2");
                });
                ui.horizontal(|ui| {
                    ui.label("Autoplay Speed (secs):");
                    ui.add(egui::DragValue::new(&mut self.autoplay_speed_secs).speed(0.1).range(0.1..=30.0 * 60.0));
                });
            });

        egui::TopBottomPanel::bottom("character_panel").show(ctx, |ui| {
            ui.columns(2, |columns| {
                let p1 = PlayerInfo {
                    player: Player::One,
                    state: &self.state,
                };
                columns[0].add(p1);

                let p2 = PlayerInfo {
                    player: Player::Two,
                    state: &self.state,
                };
                columns[1].add(p2);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ctx.options_mut(|options| {
                let central_panel_size = ui.available_size();
                let min_dim = central_panel_size.x.max(central_panel_size.y);

                options.input_options.max_click_dist = (min_dim / 4.0).max(10.0);
                options.input_options.max_click_duration = 1.0;
            });

            let eval_bar_size = 20.0;
            let total_size = ui.available_size();
            if total_size.x <= eval_bar_size {
                let game_grid = GameGrid { app: self };
                ui.add(game_grid);
            } else {
                let game_grid_size = egui::vec2(total_size.x - eval_bar_size, total_size.y);
                ui.horizontal(|ui| {
                    ui.set_height(total_size.y);
                    ui.scope_builder(UiBuilder::new(), |ui| {
                        ui.set_width(game_grid_size.x.max(1.0));
                        let game_grid = GameGrid { app: self };
                        ui.add(game_grid);
                    });
                    ui.add(EvalBar { app: self });
                });
            }
        });

        ctx.input_mut(|i| {
            if i.consume_shortcut(&egui::KeyboardShortcut::new(Modifiers::CTRL, Key::W)) {
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                });
            }

            if self.may_arrow_shortcuts {
                if i.consume_shortcut(&SHORTCUT_ENGINE_MOVE) {
                    self.try_engine_move();
                }

                if i.consume_shortcut(&SHORTCUT_REDO_TURN) {
                    self.clear_actions();
                }

                if i.consume_shortcut(&SHORTCUT_STATE_FORWARD) {
                    self.try_forward_state();
                }

                if i.consume_shortcut(&SHORTCUT_STATE_BACKWARD) {
                    self.try_back_state();
                }

                if i.consume_shortcut(&SHORTCUT_CHANGE_MODE) {
                    self.rotate_through_mode()
                }

                if i.consume_shortcut(&SHORTCUT_AUTOPLAY_START) {
                    self.is_autoplay_enabled = !self.is_autoplay_enabled;
                }
            }
        });

        ctx.request_repaint();
    }
}

// RUSTFLAGS="-C target-cpu=native" cargo run -p ui
// RUSTFLAGS="-C target-cpu=native" cargo run -p ui -r
