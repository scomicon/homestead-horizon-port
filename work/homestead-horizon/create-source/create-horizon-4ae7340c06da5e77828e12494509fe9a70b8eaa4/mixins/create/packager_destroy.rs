// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_logistics_destroy(game: &mut crate::game::Game<'_>, pos: [i32; 3], canceled: &mut bool) {
    if !*canceled {
        crate::game::create_port_packager::destroyed(game, pos);
    }
}
