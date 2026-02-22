use std::str::FromStr;

use regex::Regex;

use crate::{
    bitboard::{BitBoard, NUM_SQUARES},
    board::{BoardState, FullGameState, GodData},
    gods::{ALL_GODS_BY_ID, GodName},
    matchup::Matchup,
    player::Player,
    square::Square,
};

fn player_section_string(state: &FullGameState, player: Player) -> String {
    let mut result = String::new();
    if state.board.get_winner() == Some(player) {
        result += "#";
    }

    let god = state.get_god_for_player(player);
    result += god.god_name.into();

    let god_data = state.board.god_data[player as usize];
    if let Some(god_data_str) = god.stringify_god_data(god_data) {
        result += "[";
        result += &god_data_str;
        result += "]";
    }

    let position_strings = state
        .board
        .get_positions_for_player(player)
        .iter()
        // .map(|s| (*s as u8).to_string())
        .map(Square::to_string)
        .collect::<Vec<String>>();

    if position_strings.len() > 0 {
        result += ":";
    }

    result += &position_strings.join(",");

    result
}

pub fn game_state_to_fen(state: &FullGameState) -> String {
    let board = &state.board;

    let mut result = String::new();
    for p in 0..NUM_SQUARES {
        result += &board.get_height(p.into()).to_string();
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
    god_data: GodData,
    is_up_limited: bool,
}

const CHARACTER_FEN_WARNING: &str =
    "Player details must be in the format: /[#(if won)]god_name[optional_datas]:<worker_id_1>,.../";

fn parse_character_section(s: &str) -> Result<CharacterFen, String> {
    if s.len() == 0 {
        return Err(CHARACTER_FEN_WARNING.to_owned());
    }

    let colon_splits: Vec<_> = s.split(":").collect();
    if colon_splits.len() > 2 {
        return Err(CHARACTER_FEN_WARNING.to_owned());
    }

    let re = Regex::new(r"([^\[]*)(\[(.*)\])?").map_err(|e| format!("{}", e))?;
    let god_name_captures = re.captures(colon_splits[0]).ok_or_else(|| {
        format!(
            "Failed to parse god name from section: {}. {}",
            colon_splits[0], CHARACTER_FEN_WARNING
        )
    })?;
    let god_string = god_name_captures.get(1).unwrap().as_str().to_owned();
    let is_won = god_string.contains("#");
    let is_up_limited = god_string.contains("-");
    let god_string = god_string.replace("#", "");
    let god_string = god_string.replace("-", "");

    let god = GodName::from_str(god_string.trim())
        .map_err(|e| format!("Failed to parse god name {}: {}", god_string.as_str(), e))?;

    let god_data: GodData = if let Some(data_capture) = god_name_captures.get(3) {
        god.to_power().parse_god_data(data_capture.as_str())?
    } else {
        0
    };

    let worker_split = if colon_splits.len() == 1 {
        ""
    } else {
        colon_splits[1]
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
        god_data,
        is_up_limited,
    })
}

fn parse_heights(board: &mut BoardState, height_str: &str) -> Result<(), String> {
    if height_str.trim().is_empty() {
        return Ok(());
    }

    let heights = height_str
        .chars()
        .filter(|c| *c >= '0' && *c <= '4')
        .collect::<Vec<char>>();

    if heights.len() != NUM_SQUARES {
        return Err("Height map must be exactly 25 characters".to_string());
    }

    for (p, char) in heights.iter().enumerate() {
        let height = (*char as u8 - b'0') as usize;
        for h in 0..height {
            board.height_map[h].0 |= 1 << p;
        }
    }

    Ok(())
}

pub fn parse_fen(s: &str) -> Result<FullGameState, String> {
    let sections: Vec<&str> = s.split('/').collect();
    if sections.len() != 4 {
        return Err("Input string must have exactly 4 sections separated by '/'".to_string());
    }

    let mut result = BoardState::default();

    parse_heights(&mut result, sections[0])?;

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

    let mut p1_section = parse_character_section(sections[2])?;
    let mut p2_section = parse_character_section(sections[3])?;

    if p1_section.is_up_limited && p2_section.god == GodName::Athena {
        p2_section.god_data = 1;
    }
    if p2_section.is_up_limited && p1_section.god == GodName::Athena {
        p1_section.god_data = 1;
    }

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

    result.god_data[0] = p1_section.god_data;
    result.god_data[1] = p2_section.god_data;

    // TODO
    // result.flip_worker_can_climb(Player::One, p1_section.is_movement_blocked);
    // result.flip_worker_can_climb(Player::Two, p2_section.is_movement_blocked);

    let mut full_result = FullGameState {
        board: result,
        gods: [
            &ALL_GODS_BY_ID[p1_section.god as usize],
            &ALL_GODS_BY_ID[p2_section.god as usize],
        ],
    };

    full_result.recalculate_internals();
    full_result.validation_err()?;

    Ok(full_result)
}

/// Extracts just the matchup (god names) from a FEN string without doing a full parse.
/// Avoids all the expensive work (height bitboards, regex, zobrist hashing, validation).
pub fn extract_matchup_from_fen(fen: &str) -> Option<Matchup> {
    let mut slash_iter = fen.splitn(4, '/');
    slash_iter.next()?; // heights
    slash_iter.next()?; // player
    let god1_section = slash_iter.next()?;
    let god2_section = slash_iter.next()?;

    let god1 = extract_god_name_from_fen_section(god1_section)?;
    let god2 = extract_god_name_from_fen_section(god2_section)?;
    Some(Matchup::new(god1, god2))
}

/// Parses just the god name from a FEN player section like `#athena[^]:C2,D3`.
/// Skips leading `#`/`-` markers and stops at `:` or `[`.
fn extract_god_name_from_fen_section(section: &str) -> Option<GodName> {
    let s = section.trim_start_matches(&['#', '-']);
    let name_end = s.find(&[':', '[']).unwrap_or(s.len());
    GodName::from_str(&s[..name_end]).ok()
}

#[cfg(test)]
mod tests {
    use crate::random_utils::GameStateFuzzer;

    use super::*;

    #[test]
    fn test_fen_athena_backcompat() {
        let res = parse_fen("0000000000000000000000000/1/-mortal:B3,D3/athena:C2,C4");
        assert!(res.is_ok());
        assert_eq!(
            game_state_to_fen(&res.unwrap()),
            "0000000000000000000000000/1/mortal:B3,D3/athena[^]:C4,C2"
        )
    }

    #[test]
    fn test_fen_basic() {
        let res = parse_fen("0000000000000000000000000/1/mortal:B3,D3/mortal:C2,C4");
        assert!(res.is_ok());
    }

    #[test]
    fn test_fen_no_workers() {
        let res = parse_fen("0000000000000000000000000/1/mortal:/mortal:");
        assert!(res.is_ok());
    }

    #[test]
    fn test_fen_no_workers_no_semi() {
        let res = parse_fen("0000000000000000000000000/1/mortal/mortal");
        assert!(res.is_ok());
    }

    #[test]
    fn test_fen_placement_player_2() {
        let res = parse_fen("0000000000000000000000000/2/mortal:A1,B2/mortal");
        assert!(res.is_ok());
    }

    #[test]
    fn test_fen_placement_out_of_order() {
        let res = parse_fen("0000000000000000000000000/1/mortal/mortal:A1,B2");
        assert!(res.is_err());
    }

    #[test]
    fn test_fen_datas() {
        let res = parse_fen("0000000000000000000000000/1/athena[^]:B3,D3/mortal:C2,C4");
        assert!(res.is_ok());
        assert_eq!(res.unwrap().board.god_data[0], 1);
    }

    #[test]
    fn test_fen_winner() {
        let res = parse_fen("0000000000000000000000000/1/#athena:B3,D3/mortal:C2,C4");
        assert!(res.is_ok());
        assert_eq!(res.unwrap().get_winner(), Some(Player::One));
    }

    #[test]
    fn test_fuzz_string_and_collect() {
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let state_string = format!("{state:?}");
            let rebuilt_state = FullGameState::try_from(state_string.as_str()).unwrap();

            assert_eq!(
                state, rebuilt_state,
                "State mismatch after string conversion"
            );
        }
    }

    #[test]
    fn test_extract_matchup_basic() {
        let fen = "0000000000000000000000000/1/mortal:B3,D3/pan:C2,C4";
        let matchup = extract_matchup_from_fen(fen).unwrap();
        assert_eq!(matchup, Matchup::new(GodName::Mortal, GodName::Pan));
    }

    #[test]
    fn test_extract_matchup_with_winner_marker() {
        let fen = "0000000000000000000000000/1/#athena:B3,D3/mortal:C2,C4";
        let matchup = extract_matchup_from_fen(fen).unwrap();
        assert_eq!(matchup, Matchup::new(GodName::Athena, GodName::Mortal));
    }

    #[test]
    fn test_extract_matchup_with_god_data() {
        let fen = "0000000000000000000000000/1/athena[^]:B3,D3/mortal:C2,C4";
        let matchup = extract_matchup_from_fen(fen).unwrap();
        assert_eq!(matchup, Matchup::new(GodName::Athena, GodName::Mortal));
    }

    #[test]
    fn test_extract_matchup_with_up_limited() {
        let fen = "0000000000000000000000000/1/-mortal:B3,D3/athena[^]:C2,C4";
        let matchup = extract_matchup_from_fen(fen).unwrap();
        assert_eq!(matchup, Matchup::new(GodName::Mortal, GodName::Athena));
    }

    #[test]
    fn test_extract_matchup_no_workers() {
        let fen = "0000000000000000000000000/1/mortal/mortal";
        let matchup = extract_matchup_from_fen(fen).unwrap();
        assert_eq!(matchup, Matchup::new(GodName::Mortal, GodName::Mortal));
    }

    #[test]
    fn test_extract_matchup_agrees_with_full_parse() {
        let game_state_fuzzer = GameStateFuzzer::default();

        for state in game_state_fuzzer {
            let fen = game_state_to_fen(&state);
            let expected = state.get_matchup();
            let actual = extract_matchup_from_fen(&fen)
                .expect(&format!("extract_matchup_from_fen failed on: {}", fen));
            assert_eq!(actual, expected, "Matchup mismatch for FEN: {}", fen);
        }
    }
}
