// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_controller_register(
    game: &mut crate::game::Game<'_>,
    commands: &mut Vec<crate::mod_api::NativeCommand>,
) {
    crate::game::create_port_controller::register(game, commands);
}
