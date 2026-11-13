// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_screen_click(block: &str, at: [f32; 2], button: &mut Option<i32>) {
    crate::game::create_port_cannon::screen_click(block, at, button);
    crate::game::create_port_schematic_table::screen_click(block, at, button);
}
