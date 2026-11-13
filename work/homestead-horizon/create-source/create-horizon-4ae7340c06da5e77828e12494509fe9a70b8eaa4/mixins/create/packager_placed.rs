// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_placed(game: &mut crate::game::Game<'_>, stack: &crate::items::ItemStack, pos: [i32; 3]) {
    if !game.native_on("create") {
        return;
    }
    if stack.item.as_ref() == crate::game::create_port_packager::BLOCK {
        crate::game::create_port_packager::placed(game, pos);
    } else if stack.item.as_ref() == crate::game::create_port_gauge::BLOCK {
        crate::game::create_port_gauge::placed(game, pos);
    }
}
