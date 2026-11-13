// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_screen(block: &str, screen: &mut Option<crate::mod_api::NativeScreen>) {
    crate::game::create_port_cannon::screen(block, screen);
    crate::game::create_port_schematic_table::screen(block, screen);
}
