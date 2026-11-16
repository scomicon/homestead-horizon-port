// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Better Mount HUD, rebuilt from the mod's public description (no code or assets copied; see
// docs/client-hud.md). Vanilla replaces the food row with the mount's hearts while riding a
// living mount (`HealthHud::layout` skips food when `vehicle_hearts > 0`, and the layer
// `minecraft:vehicle_health` draws the hearts on the food's row). This keeps the food row and
// puts the mount's hearts one row above it, with a readout to the right of them: the mount's
// health, its top speed (the movement-speed attribute in blocks per second) and, while a jump is
// charging, the charge in percent.
//
// How: `gui_layer_pre` hides vanilla's `vehicle_health` layer (survival and adventure only: in
// creative the health rows are not drawn at all, so vanilla's hearts stay) and `health_hud`
// answers the whole health pass with Horizon's own layout (`HealthHud::layout`, called with the
// vehicle count at 0 so the food is drawn) plus the mount's hearts and the readout.
// Client window only.

use super::Game;
use horizon_client::{
    hud_layout::{GuiDraw, HealthInputs, vehicle_health},
    text::Text,
};

/// Blocks per second of a movement-speed attribute of 1.0 (vanilla: a horse of speed 0.225 runs
/// at about 9.7 blocks per second).
const BLOCKS_PER_SECOND: f64 = 43.17;

pub(super) fn hide(game: &Game<'_>, layer: &str, canceled: &mut bool) {
    if layer == "minecraft:vehicle_health" && game.native_window() && !game.creative {
        *canceled = true;
    }
}

/// The readout beside the mount's hearts: `12/30  9.7 b/s  jump 60%`.
pub(super) fn readout(health: f32, max_health: f32, speed_attribute: f64, jump: f32) -> String {
    let mut text = format!(
        "{:.0}/{:.0}  {:.1} b/s",
        health.ceil().max(0.0),
        max_health,
        speed_attribute * BLOCKS_PER_SECOND
    );
    if jump > 0.0 {
        text.push_str(&format!("  jump {:.0}%", (jump.min(1.0)) * 100.0));
    }
    text
}

pub(super) fn health(
    game: &mut Game<'_>,
    inputs: &HealthInputs,
    wi: i32,
    hi: i32,
    draws: &mut Option<Vec<GuiDraw>>,
) {
    if !game.native_window() || inputs.vehicle_hearts <= 0 || draws.is_some() {
        return;
    }
    let Some(index) = game.mount_index() else {
        return;
    };
    let (mount_health, max_health, speed) = {
        let mount = &game.mobs[index];
        (mount.health, mount.attrs.max_health, mount.attrs.speed)
    };
    let jump = if game.mount_jumping() {
        game.mount.jump_scale
    } else {
        0.0
    };
    // Horizon's own rows with the vehicle count at 0: armor, hearts, food and air.
    let own = HealthInputs {
        tick: inputs.tick,
        millis: inputs.millis,
        health: inputs.health,
        max_health: inputs.max_health,
        absorption: inputs.absorption,
        armor: inputs.armor,
        food: inputs.food,
        saturation: inputs.saturation,
        kind: inputs.kind,
        regeneration: inputs.regeneration,
        hunger: inputs.hunger,
        hardcore: inputs.hardcore,
        damage_cooldown: inputs.damage_cooldown,
        air: inputs.air,
        max_air: inputs.max_air,
        under_water: inputs.under_water,
        vehicle_hearts: 0,
    };
    let mut out = game.native_health_hud_state().layout(&own, wi, hi);
    // The mount's hearts, one row above where vanilla puts them (and where the food is now).
    let mut hearts = vehicle_health(wi, hi, inputs.vehicle_hearts, mount_health);
    for draw in &mut hearts {
        if let GuiDraw::Sprite { y, .. } = draw {
            *y -= 10;
        }
    }
    out.extend(hearts);
    let text = readout(mount_health, max_health, speed, jump);
    out.push(GuiDraw::Text {
        text: Text::literal(&text),
        x: wi / 2 + 91 + 4,
        y: hi - 39 - 10 + 1,
        color: 0xFFFF_FFFF,
        shadow: true,
    });
    *draws = Some(out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readout_shows_health_speed_and_jump_only_while_charging() {
        assert_eq!(readout(12.0, 30.0, 0.225, 0.0), "12/30  9.7 b/s");
        assert_eq!(readout(11.5, 30.0, 0.225, 0.6), "12/30  9.7 b/s  jump 60%");
        assert_eq!(readout(0.0, 20.0, 0.0, 2.0), "0/20  0.0 b/s  jump 100%");
    }
}
