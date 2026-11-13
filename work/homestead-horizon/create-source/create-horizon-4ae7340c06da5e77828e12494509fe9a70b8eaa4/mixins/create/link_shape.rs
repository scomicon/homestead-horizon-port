// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_link_shape(game: &crate::game::GameLevel<'_>, state: horizon_world::registry::BlockStateId,
    direction: horizon_core::geometry::Direction, neighbour: horizon_world::registry::BlockStateId,
    new: &mut Option<horizon_world::registry::BlockStateId>) {
    crate::game::create_port_link::shape(game, state, direction, neighbour, new);
}
