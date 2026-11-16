// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_storage_placement(
    game: &crate::game::GameLevel<'_>,
    state: horizon_world::registry::BlockStateId,
    context: &horizon_world::level::PlacementContext,
    answer: &mut Option<Option<horizon_world::registry::BlockStateId>>,
) {
    crate::game::homestead_storage::placement(game, state, context, answer);
}
