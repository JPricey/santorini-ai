use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP},
    build_god_power_movers,
    gods::{
        build_god_power_actions, god_power, mortal::{mortal_move_gen, MortalMove}, GodName, GodPower
    },
};

fn _limus_build_mask(own_workers: BitBoard) -> BitBoard {
    let mut own_neighbors = BitBoard::EMPTY;

    for worker in own_workers {
        own_neighbors |= NEIGHBOR_MAP[worker as usize];
    }

    !own_neighbors
}

pub const fn build_limus() -> GodPower {
    god_power(
        GodName::Limus,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        16891272677587276158,
        7282884513832450650,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_build_mask_fn(_limus_build_mask)
}
