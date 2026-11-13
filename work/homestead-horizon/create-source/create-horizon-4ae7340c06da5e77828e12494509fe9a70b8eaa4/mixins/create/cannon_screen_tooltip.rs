// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_screen_tooltip(block: &str, at: [f32; 2], lines: &mut Vec<String>) {
    crate::game::create_port_cannon::screen_tooltip(block, at, lines);
    crate::game::create_port_schematic_table::screen_tooltip(block, at, lines);
}
