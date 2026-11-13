// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_switch_flush(game: &mut crate::game::Game<'_>, dimension: &str) {
    crate::game::create_port_switch::flush(game, dimension);
}
