use crate::board::{NUM_SQUARES, Player, SantoriniState};

pub fn board_to_fen(board: &SantoriniState) -> String {
    let mut result = String::new();
    for p in 0..NUM_SQUARES {
        result += &board.get_true_height(1 << p).to_string();
    }

    result += "/";

    result += &(board.current_player as usize + 1).to_string();

    result += "/";

    result += &board
        .get_positions_for_player(Player::One)
        .iter()
        .map(usize::to_string)
        .collect::<Vec<String>>()
        .join(",");

    result += "/";

    result += &board
        .get_positions_for_player(Player::Two)
        .iter()
        .map(usize::to_string)
        .collect::<Vec<String>>()
        .join(",");

    result
}

pub fn parse_fen(s: &str) -> Result<SantoriniState, String> {
    let sections: Vec<&str> = s.split('/').collect();
    if sections.len() != 4 {
        return Err("Input string must have exactly 3 sections separated by '/'".to_string());
    }

    let mut result = SantoriniState::default();

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
            result.height_map[h] |= 1 << p;
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

    let p1_positions = sections[2];
    for p1_pos in p1_positions.split(',') {
        if p1_pos.is_empty() {
            continue;
        }
        let pos: usize = p1_pos
            .parse()
            .map_err(|_| format!("Invalid position '{}'", p1_pos))?;
        if pos >= NUM_SQUARES {
            return Err(format!("Position {} out of bounds", pos));
        }
        result.workers[0] |= 1 << pos;
    }

    let p2_positions = sections[3];
    for p2_pos in p2_positions.split(',') {
        if p2_pos.is_empty() {
            continue;
        }
        let pos: usize = p2_pos
            .parse()
            .map_err(|_| format!("Invalid position '{}'", p2_pos))?;
        if pos >= NUM_SQUARES {
            return Err(format!("Position {} out of bounds", pos));
        }
        result.workers[1] |= 1 << pos;
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use rand::{seq::SliceRandom, thread_rng};

    use super::*;

    #[test]
    fn fuzz_test_string_and_collect() {
        let mut rng = thread_rng();

        for _ in 0..10 {
            let mut state = SantoriniState::new_basic_state();
            loop {
                if state.get_winner().is_some() {
                    break;
                }

                let state_string = format!("{state:?}");
                let rebuilt_state = SantoriniState::try_from(state_string.as_str()).unwrap();

                assert_eq!(
                    state, rebuilt_state,
                    "State mismatch after string conversion"
                );

                let child_states = state.get_valid_next_states();
                state = child_states.choose(&mut rng).unwrap().clone();
            }
        }
    }
}
