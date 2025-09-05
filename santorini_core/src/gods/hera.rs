use crate::{
    bitboard::MIDDLE_SPACES_MASK,
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions, god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
};

pub const fn build_hera() -> GodPower {
    god_power(
        GodName::Hera,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        16962623483081936195,
        6551432319336663185,
    )
    .with_win_mask(MIDDLE_SPACES_MASK)
}
