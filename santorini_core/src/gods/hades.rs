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
        15915408769625054955,
        4326272341964757690,
    )
    .with_is_preventing_down()
}
