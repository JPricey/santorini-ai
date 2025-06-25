use std::str::FromStr;

use crate::{
    bitboard::BitBoard,
    board::{BoardState, FullGameState, NUM_SQUARES},
    gods::{ALL_GODS_BY_ID, GodName},
    player::Player,
    square::Square,
};

fn player_section_string(state: &FullGameState, player: Player) -> String {
    let mut result = String::new();
    if state.board.get_winner() == Some(player) {
        result += "#";
    }

    result += state.get_god_for_player(player).god_name.into();

    result += ":";
    let mut position_strings = state
        .board
        .get_positions_for_player(player)
        .iter()
        .map(Square::to_string)
        .collect::<Vec<String>>();
    position_strings.sort();
    result += &position_strings.join(",");

    result
}

pub fn game_state_to_fen(state: &FullGameState) -> String {
    let board = &state.board;

    let mut result = String::new();
    for p in 0..NUM_SQUARES {
        result += &board.get_true_height(BitBoard(1 << p)).to_string();
    }

    result += "/";

    result += &(board.current_player as usize + 1).to_string();

    result += "/";
    result += &player_section_string(state, Player::One);

    result += "/";
    result += &player_section_string(state, Player::Two);

    result
}

struct CharacterFen {
    #[allow(dead_code)]
    god: GodName,
    worker_locations: Vec<Square>,
    is_won: bool,
}

const CHARACTER_FEN_WARNING: &str =
    "Player details must be in the format: /god_name:<worker_id_1>,...[#(if won)]/";

fn parse_character_section(s: &str) -> Result<CharacterFen, String> {
    if s.len() == 0 {
        return Err(CHARACTER_FEN_WARNING.to_owned());
    }

    let is_won = s.contains("#");
    let s = s.replace("#", "");

    let colon_splits: Vec<_> = s.split(":").collect();
    if colon_splits.len() > 2 {
        return Err(CHARACTER_FEN_WARNING.to_owned());
    }

    let (god, worker_split) = if colon_splits.len() == 1 {
        eprintln!("[DEPRECATION WARNING] No god title found. Defaulting to mortal for now");
        (GodName::Mortal, colon_splits[0])
    } else {
        let god_name = GodName::from_str(colon_splits[0])
            .map_err(|e| format!("Failed to parse god name {}: {}", colon_splits[0], e))?;
        (god_name, colon_splits[1])
    };

    let mut worker_locations: Vec<Square> = Vec::new();
    for worker_pos_string in worker_split.split(',') {
        if worker_pos_string.is_empty() {
            continue;
        }
        let pos: Square = worker_pos_string.parse()?;
        worker_locations.push(pos);
    }

    Ok(CharacterFen {
        god,
        worker_locations,
        is_won,
    })
}

pub fn parse_fen(s: &str) -> Result<FullGameState, String> {
    let sections: Vec<&str> = s.split('/').collect();
    if sections.len() != 4 {
        return Err("Input string must have exactly 4 sections separated by '/'".to_string());
    }

    let mut result = BoardState::default();

    let heights = sections[0]
        .chars()
        .filter(|c| *c >= '0' && *c <= '4')
        .collect::<Vec<char>>();

    if heights.len() != NUM_SQUARES {
        return Err("Height map must be exactly 25 characters".to_string());
    }

    for (p, char) in heights.iter().enumerate() {
        let height = (*char as u8 - b'0') as usize;
        for h in 0..height {
            result.height_map[h].0 |= 1 << p;
        }
    }

    let current_player_marker = sections[1].trim();
    let current_player = match current_player_marker {
        "1" => Player::One,
        "2" => Player::Two,
        _ => {
            return Err(format!(
                "Current player marker must be either a 1 or 2. Found: {}",
                current_player_marker
            ));
        }
    };
    result.current_player = current_player;

    let p1_section = parse_character_section(sections[2])?;
    let p2_section = parse_character_section(sections[3])?;

    if p1_section.is_won && p2_section.is_won {
        return Err("Cannot have both players won".to_owned());
    }

    for square in p1_section.worker_locations {
        result.workers[0] |= BitBoard::as_mask(square);
    }
    for square in p2_section.worker_locations {
        result.workers[1] |= BitBoard::as_mask(square);
    }

    if p1_section.is_won {
        result.set_winner(Player::One);
    } else if p2_section.is_won {
        result.set_winner(Player::Two);
    }

    Ok(FullGameState {
        board: result,
        p1_god: &ALL_GODS_BY_ID[p1_section.god as usize],
        p2_god: &ALL_GODS_BY_ID[p2_section.god as usize],
    })
}

#[cfg(test)]
mod tests {
    use rand::{seq::SliceRandom, thread_rng};

    use super::*;

    #[test]
    fn fuzz_test_string_and_collect() {
        let mut rng = thread_rng();

        for _ in 0..10 {
            let mut state = FullGameState::new_basic_state_mortals();
            loop {
                let state_string = format!("{state:?}");
                let rebuilt_state = FullGameState::try_from(state_string.as_str()).unwrap();

                assert_eq!(
                    state, rebuilt_state,
                    "State mismatch after string conversion"
                );

                if state.board.get_winner().is_some() {
                    break;
                }

                let child_states = state.get_next_states();
                state = child_states.choose(&mut rng).unwrap().clone();
            }
        }
    }
}
