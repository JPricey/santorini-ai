use crate::{
    bitboard::BitBoard,
    board::{BOARD_EDGES, BoardState},
    build_god_power_move_modifiers, build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions,
        generic::{INCLUDE_SCORE, INTERACT_WITH_KEY_SQUARES, MATE_ONLY, STOP_ON_MATE},
        god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
    player::Player,
    square::Square,
};

pub fn hera_move_modifier<const IS_WIN: bool, const IS_NOW: bool>(
    _board: &BoardState,
    _me: Player,
    _other: Player,
    _from: Square,
    tos: BitBoard,
) -> BitBoard {
    if IS_WIN { tos & BOARD_EDGES } else { tos }
}

pub const fn build_hera() -> GodPower {
    god_power(
        GodName::Hera,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        16962623483081936195,
        6551432319336663185,
    )
    .with_nnue_god_name(GodName::Mortal)
    .with_move_modifier_group(build_god_power_move_modifiers!(hera_move_modifier))
}
