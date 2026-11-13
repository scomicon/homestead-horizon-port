// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_pot_draw(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    entity: &crate::containers::ModBlockEntity,
    draw: &mut crate::game::native_block_entity_draw::BlockEntityDraw,
) {
    if block == crate::game::homestead_cooking_pot::BLOCK {
        crate::game::homestead_cooking_pot::draw(game, pos, entity, draw);
    }
}
