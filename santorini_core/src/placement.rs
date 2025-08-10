use crate::{
    bitboard::BitBoard,
    board::BoardState,
    gods::generic::{GenericMove, WorkerPlacement},
    square::Square,
};

pub fn get_starting_placements_count(board: &BoardState) -> Result<usize, String> {
    let p1_workers = board.workers[0].count_ones();
    let p2_workers = board.workers[1].count_ones();

    match (p1_workers, p2_workers) {
        (0, 0) => Ok(2),
        (_, 0) => Ok(1),
        (0, _) => Err(
            "Invalid starting position. Player 2 has placed workers but not player 1".to_owned(),
        ),
        _ => Ok(0),
    }
}

pub fn get_all_placements(board: &BoardState) -> Vec<WorkerPlacement> {
    debug_assert!(board.workers[board.current_player as usize] == BitBoard::EMPTY);
    let mut res = Vec::new();

    for a in 0_usize..25 {
        let a_sq = Square::from(a);
        if (board.workers[!board.current_player as usize] & BitBoard::as_mask(a_sq)).is_not_empty()
        {
            continue;
        }

        for b in a + 1..25 {
            let b_sq = Square::from(b);
            if (board.workers[!board.current_player as usize] & BitBoard::as_mask(b_sq))
                .is_not_empty()
            {
                continue;
            }

            let action = WorkerPlacement::new(a_sq, b_sq);
            res.push(action);
        }
    }

    res
}

pub fn get_unique_placements(board: &BoardState) -> Vec<WorkerPlacement> {
    let mut b_clone = board.clone();
    let mut res = Vec::new();
    let mut unique_boards = Vec::new();

    let placements = get_all_placements(board);
    for p in placements {
        p.make_move(&mut b_clone);
        let mut is_new = true;
        for permutation in b_clone.get_all_permutations::<true>() {
            if unique_boards.contains(&permutation) {
                is_new = false;
                break;
            }
        }
        if is_new {
            unique_boards.push(b_clone.clone());
            res.push(p);
        }
        p.unmake_move(&mut b_clone);
    }

    res
}
