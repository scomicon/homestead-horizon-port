// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Detail Armor Bar, rebuilt from the mod's public description (no code or assets copied; see
// docs/client-hud.md): vanilla's armor row stops at 20 points (ten icons). This draws on top of
// it, after the `minecraft:player_health` layer,
//   - a gold, then cyan, then violet row of icons for each further 20 points, so 34 armor points
//     read as a full vanilla row plus a gold row of seven icons, and
//   - a readout left of the row: armor points, toughness and the share of a reference hit
//     (8 damage) that this armor and toughness take off (vanilla's `CombatRules`).
// Client window only. Survival and adventure only, as vanilla's rows are (no rows in creative).

use super::Game;
use horizon_client::{
    hud::HudBatch,
    hud_layout::{Blend, GuiDraw},
};
use std::borrow::Cow;

const LAYER: &str = "minecraft:player_health";
/// Tints of the 2nd, 3rd and 4th row of 20 armor points (ARGB).
const TIERS: [u32; 3] = [0xFFFF_C832, 0xFF50_E6FF, 0xFFD2_64FF];
/// The hit the percentage is worked out for.
const REFERENCE_DAMAGE: f64 = 8.0;

/// The share of a hit `damage` that armor `armor` with toughness `toughness` takes off:
/// `CombatRules.getDamageAfterAbsorb` (`f = 2 + toughness / 4`, `f1 = clamp(armor - damage / f,
/// armor / 5, 20)`, the hit is multiplied by `1 - f1 / 25`), as a fraction 0..0.8.
pub(super) fn protection(armor: f64, toughness: f64, damage: f64) -> f64 {
    let armor = armor.max(0.0);
    let f = 2.0 + toughness.max(0.0) / 4.0;
    // max then min, not `clamp`: with armor over 100 the lower bound passes the upper one.
    let f1 = (armor - damage.max(0.0) / f).max(armor * 0.2).min(20.0);
    f1 / 25.0
}

/// The y of vanilla's armor row (`Hud.extractPlayerHealth`, mirrored by `HealthHud::layout`):
/// the health rows stack up from `height - 39`, each row `10 - (rows - 2)` high (at least 3), and
/// the armor sits one icon above the topmost of them.
pub(super) fn armor_row_y(gui_height: i32, max_health: f64, absorption: f64) -> i32 {
    let rows = ((max_health + absorption.ceil()) / 2.0 / 10.0).ceil() as i32;
    let row_height = (10 - (rows - 2)).max(3);
    gui_height - 39 - (rows - 1) * row_height - 10
}

/// The icons of the rows above the first (points 21 and up), one `GuiDraw` per icon.
pub(super) fn overflow_icons(points: i32, x_left: i32, y: i32) -> Vec<GuiDraw> {
    let mut out = Vec::new();
    for (tier, tint) in TIERS.iter().enumerate() {
        let part = (points - 20 * (tier as i32 + 1)).min(20);
        if part <= 0 {
            break;
        }
        for n in 0..10 {
            let name = if n * 2 + 1 < part {
                "hud/armor_full"
            } else if n * 2 + 1 == part {
                "hud/armor_half"
            } else {
                continue;
            };
            out.push(GuiDraw::Sprite {
                sprite: Cow::Borrowed(name),
                x: x_left + n * 8,
                y,
                w: 9,
                h: 9,
                region: None,
                color: *tint,
                blend: Blend::Textured,
            });
        }
    }
    out
}

/// The readout, `24 T4 (62%)`: armor points, toughness when there is some, then the share.
pub(super) fn label(points: i32, toughness: f64, share: f64) -> String {
    let percent = (share * 100.0).round() as i32;
    if toughness >= 0.5 {
        format!("{points} T{:.0} ({percent}%)", toughness)
    } else {
        format!("{points} ({percent}%)")
    }
}

pub(super) fn draw(game: &mut Game<'_>, layer: &str, batch: &mut HudBatch) {
    if layer != LAYER || !game.native_window() {
        return;
    }
    let armor = game
        .native_local_attribute("minecraft:armor")
        .or_else(|| game.native_local_attribute("armor"))
        .unwrap_or(0.0);
    // `Hud` truncates the attribute the same way when it picks the icons.
    let points = armor as i32;
    if points <= 0 {
        return;
    }
    let toughness = game
        .native_local_attribute("minecraft:armor_toughness")
        .unwrap_or(0.0);
    let max_health = game
        .native_local_attribute("minecraft:max_health")
        .unwrap_or(20.0);
    let absorption = game
        .player
        .as_mut()
        .map_or(0.0, |p| f64::from(p.health.absorption()));
    let (w, h) = (batch.width as i32, batch.height as i32);
    let y = armor_row_y(h, max_health, absorption);
    let x_left = w / 2 - 91;
    let icons = overflow_icons(points, x_left, y);
    if !icons.is_empty() {
        game.native_gui_paint(batch, &icons);
    }
    let text = label(points, toughness, protection(armor, toughness, REFERENCE_DAMAGE));
    let width = game.native_text_width(&text);
    let x = ((x_left - 3) as f32 - width).max(2.0);
    game.native_text(batch, &text, [x, (y + 1) as f32], 0x00D0_D0D0, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protection_matches_vanilla_for_typical_sets() {
        // Diamond set: 20 armor, 8 toughness against an 8 damage hit: 20 - 8 / 4 = 18 -> 72%.
        assert!((protection(20.0, 8.0, 8.0) - 0.72).abs() < 1e-9);
        // No armor: nothing taken off.
        assert_eq!(protection(0.0, 0.0, 8.0), 0.0);
        // Armor is capped at 20 effective points (80%).
        assert!((protection(40.0, 12.0, 1.0) - 0.8).abs() < 1e-9);
        // The lower bound (armor / 5) wins against a huge hit.
        assert!((protection(10.0, 0.0, 1000.0) - 2.0 / 25.0).abs() < 1e-9);
    }

    #[test]
    fn armor_row_follows_the_health_rows() {
        // Ten hearts: one row, the armor sits at height - 49.
        assert_eq!(armor_row_y(240, 20.0, 0.0), 240 - 49);
        // Twenty hearts: two rows of 10 - 0 = 10 high: the armor goes one row higher.
        assert_eq!(armor_row_y(240, 40.0, 0.0), 240 - 49 - 10);
        // Absorption counts as extra containers: ten hearts and one absorption heart need two rows.
        assert_eq!(armor_row_y(240, 20.0, 2.0), 240 - 49 - 10);
    }

    #[test]
    fn overflow_rows_start_after_twenty_points() {
        assert!(overflow_icons(20, 0, 0).is_empty());
        // 21 points: one half icon of the second row.
        assert_eq!(overflow_icons(21, 0, 0).len(), 1);
        // 30 points: ten points over: five full icons.
        assert_eq!(overflow_icons(30, 0, 0).len(), 5);
        // 60 points: two full tinted rows (twenty icons) and a third one with nothing in it.
        assert_eq!(overflow_icons(60, 0, 0).len(), 20);
    }

    #[test]
    fn label_shows_toughness_only_when_present() {
        assert_eq!(label(24, 0.0, 0.5), "24 (50%)");
        assert_eq!(label(24, 4.0, 0.62), "24 T4 (62%)");
    }
}
