// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_cannon_container(block: &str, container: &mut Option<crate::containers::ModContainer>) {
    crate::game::create_port_cannon::container(block, container);
    crate::game::create_port_schematic_table::container(block, container);
}
