use crate::{
    bitboard::BitBoard,
    board::BoardState,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions, god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
};

// Allows a single lvl 0 worker to move, which isn't legal but whatever
pub fn hypnus_moveable_worker_filter(board: &BoardState, workers: BitBoard) -> BitBoard {
    let lvl_3_workers = board.height_map[2] & workers;
    let lvl_3_worker_count = lvl_3_workers.count_ones();
    if lvl_3_worker_count == 1 {
        return workers ^ lvl_3_workers;
    } else if lvl_3_worker_count > 1 {
        return workers;
    }

    let lvl_2_workers = board.height_map[1] & workers;
    let lvl_2_worker_count = lvl_2_workers.count_ones();
    if lvl_2_worker_count == 1 {
        return workers ^ lvl_2_workers;
    } else if lvl_2_worker_count > 1 {
        return workers;
    }

    let lvl_1_workers = board.height_map[0] & workers;
    let lvl_1_worker_count = lvl_1_workers.count_ones();
    if lvl_1_worker_count == 1 {
        return workers ^ lvl_1_workers;
    }

    workers
}

pub const fn build_hypnus() -> GodPower {
    god_power(
        GodName::Hypnus,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        15915408769625054955,
        4326272341964757690,
    )
    .with_moveable_worker_filter(hypnus_moveable_worker_filter)
}
