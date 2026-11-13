// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_whistle_survival(game: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId, survives: &mut Option<bool>) {
    crate::game::create_port_whistle::can_survive(game, pos, state, survives);
}
