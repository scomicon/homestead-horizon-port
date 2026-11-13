// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_board_draw(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: horizon_world::registry::BlockStateId,
    entity: &crate::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    let _ = (pos, state);
    if block == crate::game::homestead_cutting_board::BLOCK {
        crate::game::homestead_cutting_board::draw(game, entity, draw);
    }
}
