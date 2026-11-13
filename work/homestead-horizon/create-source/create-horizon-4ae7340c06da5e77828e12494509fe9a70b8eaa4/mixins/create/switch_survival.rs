// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_switch_survival(game: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId, survives: &mut Option<bool>) {
    crate::game::create_port_switch::can_survive(game, pos, state, survives);
}
