use crate::{
    bitboard::{BitBoard, PUSH_MAPPING},
    board::BoardState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions, god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
    square::Square,
};

pub fn slide_position(board: &BoardState, from: Square, to: Square) -> Square {
    let Some(next_spot) = PUSH_MAPPING[from as usize][to as usize] else {
        return to;
    };

    let to_height = board.get_height(to);

    let next_height = board.get_height(next_spot);

    if next_height > to_height {
        return to;
    }

    let next_mask = BitBoard::as_mask(next_spot);

    if ((board.workers[0] | board.workers[1]) & next_mask).is_not_empty() {
        return to;
    }

    slide_position(board, to, next_spot)
}

pub const fn build_harpies() -> GodPower {
    god_power(
        GodName::Harpies,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        10276148328807193798,
        10430305106761659855,
    )
    .with_nnue_god_name(GodName::Mortal)
}

#[cfg(test)]
mod tests {
    use crate::{
        bitboard::NEIGHBOR_MAP,
        fen::parse_fen,
        gods::ALL_GODS_BY_ID,
        matchup::{self, BANNED_MATCHUPS, Matchup, is_matchup_banned},
    };

    use super::*;

    #[test]
    fn test_all_gods_respect_harpies() {
        // Doesn't test for perfect correctness, but at least that they aren't doing their basic
        // moves

        for god in ALL_GODS_BY_ID {
            let god_name = god.god_name;
            let matchup = Matchup::new(god_name, GodName::Harpies);
            if is_matchup_banned(&matchup) {
                eprintln!("skipping banned matchup: {}", matchup);
                continue;
            }

            let state = parse_fen(&format!(
                "00000 00000 00000 00000 00000/1/{}:A5/harpies:E2,D1",
                god_name
            ))
            .unwrap();

            let next_states = god.get_all_next_states(&state);
            let old_neighbors = NEIGHBOR_MAP[Square::A5 as usize];

            for next_state in next_states {
                let new_workers = next_state.workers[0];
                if (old_neighbors & new_workers).is_not_empty() {
                    eprintln!("{:?}", state);
                    next_state.print_to_console();
                    assert!(
                        false,
                        "Other god didn't respect harpies movement: {:?}",
                        god.god_name
                    );
                }
            }
        }
    }
}
