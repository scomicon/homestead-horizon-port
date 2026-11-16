// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// `homestead status`: a read-only diagnostic line for play-test reports (stage 22). Prints the game
// mode, difficulty, the player's food, saturation and exhaustion, the invulnerable flag of the
// movement state (it suppresses food exhaustion) and of the server view, and the active effects.
// Nothing is changed. It runs on the control socket's / chat's command path, on the window's Game.
use super::Game;
use crate::mod_api::{NativeCommand, NativeServerPlayer};
use horizon_client::text::Text;

pub(super) fn register(commands: &mut Vec<NativeCommand>) {
    commands.push(NativeCommand::op("homestead status", 0));
}

fn mode_name(mode: i32) -> &'static str {
    match mode {
        0 => "survival",
        1 => "creative",
        2 => "adventure",
        3 => "spectator",
        _ => "unknown",
    }
}

fn difficulty_name(value: horizon_world::food::Difficulty) -> &'static str {
    use horizon_world::food::Difficulty;
    match value {
        Difficulty::Peaceful => "peaceful",
        Difficulty::Easy => "easy",
        Difficulty::Normal => "normal",
        Difficulty::Hard => "hard",
    }
}

pub(super) fn command(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    let words = line.split_whitespace().collect::<Vec<_>>();
    if words != ["homestead", "status"] {
        return;
    }
    *ran = true;
    let food = match game.player.as_ref() {
        Some(motion) => format!(
            "food {} saturation {:.1} exhaustion {:.2} invulnerable(motion) {} spectator(motion) {}",
            motion.health.food.food,
            motion.health.food.saturation,
            motion.health.food.exhaustion,
            motion.invulnerable,
            motion.spectator
        ),
        None => "food unknown (no local player)".to_string(),
    };
    let effects = game
        .potions
        .effects
        .list
        .iter()
        .map(|effect| {
            format!(
                "{}x{} {}t",
                effect.id,
                effect.amplifier + 1,
                effect.duration
            )
        })
        .collect::<Vec<_>>();
    let effects = if effects.is_empty() {
        "none".to_string()
    } else {
        effects.join(", ")
    };
    let message = format!(
        "status: nick {} mode {} difficulty {} health {:.1} {} invulnerable(server view) {} may_fly {} flying {} effects: {}",
        player.name,
        mode_name(player.game_mode),
        difficulty_name(game.world.health_rules.difficulty),
        player.health,
        food,
        player.invulnerable,
        player.may_fly,
        player.flying,
        effects
    );
    game.native_feedback(Text::literal(&message));
}
