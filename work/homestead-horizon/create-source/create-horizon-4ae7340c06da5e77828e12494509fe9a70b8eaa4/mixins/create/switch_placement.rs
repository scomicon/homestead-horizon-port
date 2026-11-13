// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_switch_placement(
    game: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    crate::game::create_port_switch::placement(game, state, context, answer);
}
