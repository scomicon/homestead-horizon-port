// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_schematool_use_on(
    game: &mut crate::game::Game<'_>,
    stack: &crate::items::ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    crate::game::create_port_schematic_tool::use_on(game, stack, pos, face, claimed);
}
