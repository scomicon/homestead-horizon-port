// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_controller_tick(game: &mut crate::game::Game<'_>) {
    crate::game::create_port_controller::tick(game);
}
