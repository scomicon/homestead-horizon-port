// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_schematool_use_item(
    game: &mut crate::game::Game<'_>,
    stack: &crate::items::ItemStack,
    claimed: &mut bool,
) {
    crate::game::create_port_schematic_tool::use_air(game, stack, claimed);
}
