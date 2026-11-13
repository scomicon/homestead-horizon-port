// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_stock_placed(game: &mut crate::game::Game<'_>, stack: &crate::items::ItemStack, pos: [i32; 3]) {
    if !game.native_on("create") {
        return;
    }
    if stack.item.as_ref() == crate::game::create_port_stock_link::BLOCK {
        crate::game::create_port_stock_link::placed(game, pos);
    }
}
