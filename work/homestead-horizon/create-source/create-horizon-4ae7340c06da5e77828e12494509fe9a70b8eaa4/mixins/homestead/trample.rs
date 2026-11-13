// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Secure Crops with the Homestead pack's rules (`secureFarmlandAndCrops` and
// `secureFarmlandIfEmpty` both true): farmland is never trampled.

fn homestead_trample(cancel: &mut bool) {
    *cancel = true;
}