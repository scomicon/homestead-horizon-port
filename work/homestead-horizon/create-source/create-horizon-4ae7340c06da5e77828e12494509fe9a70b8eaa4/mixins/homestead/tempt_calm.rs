// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// TemptGoal.stop sets a 100 tick calm-down; clearing it before every canUse lets an animal that
// sees its food always be tempted again at once (No Animal Tempt Delay).
fn homestead_no_tempt_delay(tempt: &mut Tempt) {
    tempt.calm_down = 0;
}