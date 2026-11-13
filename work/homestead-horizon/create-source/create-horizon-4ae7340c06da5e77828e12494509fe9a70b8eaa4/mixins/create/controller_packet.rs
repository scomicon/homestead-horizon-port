// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_controller_packet(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    crate::game::create_port_controller::receive(game, player, channel, data);
}
