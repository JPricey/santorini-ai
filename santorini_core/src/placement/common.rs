use crate::{
    board::{BoardState, GodPair},
    gods::generic::{GenericMove, GodMove},
    player::Player,
    utils::base_hash_for_god_pair,
};

pub trait WorkerPlacementMove: GodMove {
    fn make_move_no_swap_sides(&self, board: &mut BoardState, player: Player);

    fn get_all_placements(gods: GodPair, board: &BoardState, player: Player) -> Vec<GenericMove>;

    fn get_unique_placements(gods: GodPair, board: &BoardState, player: Player)
    -> Vec<GenericMove>;
}

pub(super) fn compute_unique_placements<W: WorkerPlacementMove>(
    gods: GodPair,
    board: &BoardState,
    player: Player,
) -> Vec<GenericMove> {
    let all_placements = W::get_all_placements(gods, board, player);
    let mut res = Vec::<GenericMove>::new();
    let mut all_seen_boards = Vec::new();
    let base_hash = base_hash_for_god_pair(gods);

    for placement in all_placements {
        let mut new_board = board.clone();
        W::from(placement).make_move_no_swap_sides(&mut new_board, player);

        if all_seen_boards.contains(&new_board) {
            continue;
        }

        res.push(placement);

        for permutation in new_board.get_all_permutations::<true>(gods, base_hash) {
            all_seen_boards.push(permutation);
        }
    }

    res
}
