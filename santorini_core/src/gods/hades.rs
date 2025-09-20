use crate::{
    build_god_power_movers,
    gods::{
        GodName, GodPower, build_god_power_actions, god_power,
        mortal::{MortalMove, mortal_move_gen},
    },
};

pub const fn build_hades() -> GodPower {
    god_power(
        GodName::Hades,
        build_god_power_movers!(mortal_move_gen),
        build_god_power_actions::<MortalMove>(),
        3909168555047842639,
        3956786047127225345,
    )
    .with_is_preventing_down()
}
