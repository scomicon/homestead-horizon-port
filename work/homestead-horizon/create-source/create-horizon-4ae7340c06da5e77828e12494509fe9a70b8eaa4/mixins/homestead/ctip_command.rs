// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_ctip_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::homestead_client_tooltips::command(game, player, command, ran);
}
