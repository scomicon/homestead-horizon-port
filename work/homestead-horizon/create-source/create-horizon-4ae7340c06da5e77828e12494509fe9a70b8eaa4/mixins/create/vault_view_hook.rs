// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

fn create_vault_view(
    containers: &crate::game::machines::Containers<'_>,
    pos: (i32, i32, i32),
) -> core::ops::ControlFlow<Option<View>> {
    match crate::game::machines::create_vault_view::view(containers, pos) {
        Some(view) => core::ops::ControlFlow::Break(Some(view)),
        None => core::ops::ControlFlow::Continue(()),
    }
}
