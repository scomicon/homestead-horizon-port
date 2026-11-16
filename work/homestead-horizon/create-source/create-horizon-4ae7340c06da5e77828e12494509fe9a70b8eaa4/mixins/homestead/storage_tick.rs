// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_storage_tick(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    entity: &mut crate::containers::ModBlockEntity,
) {
    crate::game::homestead_storage::tick(game, pos, entity);
}
