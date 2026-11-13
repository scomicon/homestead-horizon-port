// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_schematool_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::create_port_schematic_tool::command(game, player, command, ran);
}
