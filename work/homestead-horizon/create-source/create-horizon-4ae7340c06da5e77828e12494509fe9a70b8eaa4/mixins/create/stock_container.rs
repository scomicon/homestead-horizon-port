// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_stock_container(block: &str, container: &mut Option<crate::containers::ModContainer>) {
    crate::game::create_port_frogport::container(block, container);
}
