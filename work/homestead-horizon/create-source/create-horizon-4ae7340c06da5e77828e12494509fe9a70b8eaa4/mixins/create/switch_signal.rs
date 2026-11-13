// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_switch_signal(game: &crate::game::GameLevel<'_>, pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId, face: horizon_core::geometry::Direction,
    direct: bool, signal: &mut i32) {
    crate::game::create_port_switch::signal(game, pos, state, face, direct, signal);
}
