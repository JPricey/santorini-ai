use crate::{
    bitboard::{BitBoard, NEIGHBOR_MAP, apply_mapping_to_mask},
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions, god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
};

fn _limus_build_mask(own_workers: BitBoard) -> BitBoard {
    !apply_mapping_to_mask(own_workers, &NEIGHBOR_MAP)
}

pub const fn build_limus() -> GodPower {
    god_power(
        GodName::Limus,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        16891272677587276158,
        7282884513832450650,
    )
    .with_build_mask_fn(_limus_build_mask)
}
