// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn homestead_status_command(
    game: &mut crate::game::Game<'_>,
    player: &crate::mod_api::NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    crate::game::homestead_status::command(game, player, command, ran);
}
