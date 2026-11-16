// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
//
// AmbientSounds (rebuilt from its public description, no code or assets of the original): sounds
// that suit where the player is and what the world is doing, played near the player now and then.
// The original's sound files are not redistributable, so this mod ships none: the rules of
// `homestead-ambient.txt` name sound events from the player's own resource pack, and the content
// mod `homestead_sound` adds three placeholder events (homestead_sound:tone.low|mid|high) that
// point at vanilla sounds so the engine can be tried and tested.
//
// * Client tick only. A rule is due every `interval` ticks; it then rolls `chance`, checks its
//   conditions against the world around the player and, on a hit, queues the sound at a random
//   spot within 6 blocks. `homestead_sound_config::flush` plays the queue with
//   `native_play_local_sound`.
// * Conditions (joined with '+', all must hold): day night rain thunder clear cave surface
//   underwater overworld nether end biome:<pattern>. `cave` is approximated as "below y 50":
//   the client has no sky light to read.
// * The context (biome, clock, weather, water at the eyes) is read only when some rule is due.

use super::{
    Game,
    homestead_sound_config::{MAX_PENDING, Pending, glob, state},
};

struct Context {
    y: f64,
    biome: String,
    day_time: i64,
    rain: f32,
    thunder: f32,
    dimension: String,
    underwater: bool,
}

/// Whether `condition` is one of the words of the rule file.
pub(crate) fn known_condition(condition: &str) -> bool {
    matches!(
        condition,
        "-" | "day" | "night" | "rain" | "thunder" | "clear" | "cave" | "surface" | "underwater" | "overworld" | "nether" | "end"
    ) || condition.strip_prefix("biome:").is_some_and(|pattern| !pattern.is_empty())
}

fn holds(condition: &str, context: &Context) -> bool {
    let time = context.day_time.rem_euclid(24000);
    match condition {
        "-" => true,
        "day" => time < 12000,
        "night" => (13000..23000).contains(&time),
        "rain" => context.rain > 0.2,
        "thunder" => context.thunder > 0.2,
        "clear" => context.rain < 0.01,
        "cave" => context.y < 50.0,
        "surface" => context.y >= 50.0,
        "underwater" => context.underwater,
        "overworld" => context.dimension == "minecraft:overworld",
        "nether" => context.dimension == "minecraft:the_nether",
        "end" => context.dimension == "minecraft:the_end",
        other => other.strip_prefix("biome:").is_some_and(|pattern| glob(pattern, &context.biome)),
    }
}

/// A float in [0, 1) from a small linear congruential generator.
fn unit(rng: &mut u64) -> f64 {
    *rng = rng.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    (*rng >> 11) as f64 / (1u64 << 53) as f64
}

/// Client tick: rolls the rules that are due.
pub(crate) fn tick(game: &mut Game<'_>) {
    let due = {
        let st = state();
        st.ambient_enabled && st.ambient_rules.iter().any(|rule| st.ticks >= rule.next)
    };
    if !due {
        return;
    }
    let Some(player) = game.native_player() else { return };
    let block = [player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32];
    let (rain, thunder) = game.native_weather();
    let context = Context {
        y: player.y,
        biome: game.native_biome(block).unwrap_or_default(),
        day_time: game.native_day_time(),
        rain,
        thunder,
        dimension: player.dimension.to_string(),
        underwater: game.native_block([block[0], (player.y + 1.6).floor() as i32, block[2]]) == "minecraft:water",
    };
    let at_player = [player.x, player.y, player.z];
    let mut st = state();
    let now = st.ticks;
    let mut rng = st.rng;
    let mut queued = Vec::new();
    for index in 0..st.ambient_rules.len() {
        if now < st.ambient_rules[index].next {
            continue;
        }
        let interval = u64::from(st.ambient_rules[index].interval);
        st.ambient_rules[index].next = now + interval;
        let rule = &st.ambient_rules[index];
        let roll = unit(&mut rng);
        if roll >= f64::from(rule.chance) || !rule.conditions.iter().all(|c| holds(c, &context)) {
            continue;
        }
        let offset = [(unit(&mut rng) - 0.5) * 12.0, unit(&mut rng) * 2.0, (unit(&mut rng) - 0.5) * 12.0];
        queued.push(Pending {
            sound: rule.sound.clone(),
            at: [at_player[0] + offset[0], at_player[1] + offset[1], at_player[2] + offset[2]],
            volume: rule.volume,
            pitch: rule.pitch,
            ambient: true,
        });
    }
    st.rng = rng;
    for pending in queued {
        if st.pending.len() < MAX_PENDING {
            st.pending.push(pending);
            st.counters.ambient_queued += 1;
        }
    }
}
