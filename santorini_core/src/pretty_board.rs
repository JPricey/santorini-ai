use serde::Serialize;

use crate::{
    bitboard::BitBoard,
    board::{FullGameState, GodData},
    gods::{GodName, MoveWorkerMeta, PartialAction},
    placement::{PlacementType, get_starting_placement_state},
    player::Player,
    square::Square,
};

#[derive(Clone, PartialEq, Eq, Serialize, Debug)]
pub struct PrettyPlayer {
    god: GodName,
    workers: Vec<Square>,
    tokens: Vec<Square>,
    special_text: Option<String>,
}

impl Default for PrettyPlayer {
    fn default() -> Self {
        Self {
            god: GodName::Mortal,
            workers: Default::default(),
            tokens: Default::default(),
            special_text: Default::default(),
        }
    }
}

#[derive(Default, Clone, PartialEq, Eq, Serialize, Debug)]
pub struct PrettyBoard {
    acting_player: Player,
    winner: Option<Player>,
    heights: [[u8; 5]; 5],
    players: [PrettyPlayer; 2],
}

fn _set_pretty_player(state: &FullGameState, player: Player, pretty_player: &mut PrettyPlayer) {
    let player_god = state.gods[player as usize];

    pretty_player.god = state.gods[player as usize].god_name;
    pretty_player.workers = state.board.workers[player as usize].all_squares();
    pretty_player.tokens = (player_god.get_frozen_mask(&state.board, player)
        | player_god.get_female_worker_mask(&state.board, player))
    .all_squares();
    pretty_player.special_text = player_god.pretty_stringify_god_data(&state.board, player);
}

pub fn get_acting_player(state: &FullGameState) -> Result<Player, String> {
    let placement_state = get_starting_placement_state(&state.board, state.gods)?;
    match placement_state {
        Some(placement_state) => Ok(placement_state.next_placement),
        None => Ok(state.board.current_player),
    }
}

pub fn state_to_pretty_board(state: &FullGameState) -> PrettyBoard {
    let mut result = PrettyBoard::default();

    for r in 0..5 {
        for c in 0..5 {
            result.heights[r][c] = state.board.height_lookup[r * 5 + c];
        }
    }
    result.acting_player = get_acting_player(state).unwrap_or(state.board.current_player);
    result.winner = state.get_winner();

    _set_pretty_player(state, Player::One, &mut result.players[0]);
    _set_pretty_player(state, Player::Two, &mut result.players[1]);

    result
}

pub fn game_state_with_partial_actions(
    state: &FullGameState,
    actions: &Vec<PartialAction>,
) -> FullGameState {
    if actions.is_empty() {
        return state.clone();
    }

    let Ok(current_player) = get_acting_player(&state) else {
        return state.clone();
    };

    let mut result = state.clone();
    let board = &mut result.board;

    let mut selected_square: Option<Square> = None;

    for action in actions.iter().cloned() {
        match action {
            PartialAction::PlaceWorker(square) => {
                board.worker_xor(current_player, BitBoard::as_mask(square));
            }
            PartialAction::SetFemaleWorker(square) => {
                board.set_god_data(current_player, square.to_board().0);
            }
            PartialAction::SelectWorker(square) => {
                assert!(selected_square.is_none());
                selected_square = Some(square);
            }
            PartialAction::ForceOpponentWorker(from, to) => {
                let xor_mask = from.to_board() ^ to.to_board();
                board.worker_xor(!current_player, xor_mask);

                if state.gods[!current_player as usize].placement_type
                    == PlacementType::FemaleWorker
                {
                    let is_f_worker_forced =
                        (board.god_data[!current_player as usize] & xor_mask.0) != 0;
                    if is_f_worker_forced {
                        board.xor_god_data(!current_player, xor_mask.0);
                    }
                }
            }
            PartialAction::MoveWorker(data) => {
                let selected_square = selected_square.take().unwrap();
                let self_mask = BitBoard::as_mask(selected_square) ^ BitBoard::as_mask(data.dest);
                board.worker_xor(current_player, self_mask);

                if let Some(meta) = data.meta {
                    match meta {
                        MoveWorkerMeta::MoveEnemyWorker(move_enemy_worker) => {
                            let xor_mask = BitBoard::as_mask(move_enemy_worker.from)
                                ^ BitBoard::as_mask(move_enemy_worker.to);
                            board.worker_xor(!current_player, xor_mask);

                            if state.gods[!current_player as usize].placement_type
                                == PlacementType::FemaleWorker
                            {
                                let is_f_worker_forced =
                                    (board.god_data[!current_player as usize] & xor_mask.0) != 0;
                                if is_f_worker_forced {
                                    board.xor_god_data(!current_player, xor_mask.0);
                                }
                            }
                        }
                        MoveWorkerMeta::KillEnemyWorker(kill_enemy_worker) => {
                            let enemy_worker_mask = BitBoard::as_mask(kill_enemy_worker.square);
                            board.worker_xor(!current_player, enemy_worker_mask);
                        }
                        MoveWorkerMeta::IsFWorker => {
                            board.xor_god_data(current_player, self_mask.0);
                        }
                    }
                }
            }
            PartialAction::Build(square) => {
                board.build_up(square);
            }
            PartialAction::Dome(square) => {
                board.dome_up(square);
            }
            PartialAction::Destroy(square) => {
                board.unbuild(square);
            }
            PartialAction::SetTalusPosition(square) => {
                board.set_god_data(current_player, BitBoard::as_mask(square).0 as GodData);
            }
            PartialAction::NoMoves
            | PartialAction::EndTurn
            | PartialAction::HeroPower(_)
            | PartialAction::SetWindDirection(_) => (),
        }
    }

    result
}
