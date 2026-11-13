// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_stove_draw(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: horizon_world::registry::BlockStateId,
    entity: &crate::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    let _ = state;
    if block == crate::game::homestead_stove::BLOCK {
        crate::game::homestead_stove::draw(game, pos, entity, draw);
    }
}
