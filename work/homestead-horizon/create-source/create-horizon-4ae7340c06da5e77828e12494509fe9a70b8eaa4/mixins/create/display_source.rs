// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Display sources (DisplaySource subclasses) that can be read in this port:
// they gather data from the block a Display Link is attached to and hand it to
// the text rules of `create_engine::display_link`. Each source returns a value
// and the flap section kind the board should lay out for it.
//
// Not ported (no data in this port, see docs/redstone-displays.md): cuckoo
// clock gating of the time source, scoreboards, trains and stations, factory
// gauges, boilers, item/fluid lists and throughput, lectern/book text, the
// Nixie source and computer sources.

use super::Game;
use crate::{containers::BlockEntity, items::ItemStack};
use create_engine::{display_board::JavaRandom, display_link as dl};
use horizon_world::nbt::{Compound, Tag};

type Pos = [i32; 3];

/// Selectable source ids, in cycling order. `auto` picks one from the source block.
pub(super) static SOURCES: [&str; 9] = [
    "auto",
    "time_of_day",
    "game_time",
    "redstone_power",
    "item_count",
    "item_name",
    "fluid_amount",
    "kinetic_speed",
    "kinetic_stress",
];

/// Flap section kind a value is laid out in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
    Alphabet,
    Numeric,
    Instant,
    Progress,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Out {
    Value(String, Section),
    /// A progress bar, built by the caller once the board width is known.
    Bar(f32),
}

/// How many display modes a source has (`Mode` cycles in this range).
pub(super) fn modes(id: &str) -> i32 {
    match id {
        "time_of_day" | "kinetic_speed" => 2,
        "redstone_power" => 3,
        "kinetic_stress" => 5,
        _ => 1,
    }
}

pub(super) fn mode_name(id: &str, mode: i32) -> &'static str {
    match (id, mode) {
        ("time_of_day", 0) => "24 hour",
        ("time_of_day", _) => "12 hour",
        ("kinetic_speed", 0) => "absolute",
        ("kinetic_speed", _) => "directional",
        ("redstone_power", 0) => "number",
        ("redstone_power", 1) => "percent",
        ("redstone_power", _) => "progress bar",
        ("kinetic_stress", 0) => "progress bar",
        ("kinetic_stress", 1) => "percent",
        ("kinetic_stress", 2) => "stress",
        ("kinetic_stress", 3) => "capacity",
        ("kinetic_stress", _) => "remaining",
        _ => "default",
    }
}

fn entity_nbt<'a>(game: &'a Game<'_>, pos: Pos) -> Option<&'a Compound> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(entity)) => Some(&entity.nbt),
        _ => None,
    }
}

/// Item stacks of the container at `pos` (vanilla chest-like or a mod container
/// such as an Item Vault). None when the block has no readable item slots.
fn stacks(game: &Game<'_>, pos: Pos) -> Option<Vec<ItemStack>> {
    match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Chest(slots)) => Some(slots.iter().flatten().cloned().collect()),
        Some(BlockEntity::Mod(entity)) if !entity.items.is_empty() => {
            Some(entity.items.iter().flatten().cloned().collect())
        }
        _ => None,
    }
}

/// The source an `auto` link uses for the block it is attached to.
pub(super) fn resolve(game: &Game<'_>, source: Pos, configured: &str) -> &'static str {
    if let Some(id) = SOURCES.iter().find(|id| **id == configured && **id != "auto") {
        return *id;
    }
    match game.native_block(source).as_str() {
        "create:speedometer" => "kinetic_speed",
        "create:stressometer" => "kinetic_stress",
        "create:fluid_tank" => "fluid_amount",
        _ if stacks(game, source).is_some() => "item_count",
        _ => "redstone_power",
    }
}

fn number(nbt: &Compound, key: &str) -> f32 {
    super::create_port::number(nbt, key)
}

fn tank_amount(game: &Game<'_>, member: Pos) -> Option<i64> {
    let nbt = entity_nbt(game, member)?;
    let controller = match nbt.get(&"Controller".into()) {
        None => member,
        Some(Tag::IntArray(offset)) if offset.len() == 3 => {
            [member[0] + offset[0], member[1] + offset[1], member[2] + offset[2]]
        }
        Some(_) => return None,
    };
    let Some(Tag::Compound(content)) = entity_nbt(game, controller)?.get(&"TankContent".into()) else {
        return Some(0);
    };
    let Some(Tag::Compound(fluid)) = content.get(&"Fluid".into()) else {
        return Some(0);
    };
    Some(match fluid.get(&"amount".into()) {
        Some(Tag::Int(v)) => i64::from(*v),
        Some(Tag::Long(v)) => *v,
        _ => 0,
    })
}

/// Reads source `id` at the block `source`. None when the block has nothing
/// this source can show (a placeholder is then not written to the board).
pub(super) fn read(game: &mut Game<'_>, id: &str, source: Pos, mode: i32) -> Option<Out> {
    let mode = mode.rem_euclid(modes(id));
    Some(match id {
        "time_of_day" => {
            let natural = game.dim.id() == "minecraft:overworld";
            let mut rng = JavaRandom::new(game.world.game_time);
            let text = dl::time_of_day(
                game.native_day_time(),
                mode == 1,
                true,
                natural,
                &dl::TimeLabels::default(),
                &mut rng,
            );
            Out::Value(text, Section::Instant)
        }
        "game_time" => Out::Value(dl::format_number(game.world.game_time as f64), Section::Numeric),
        "redstone_power" => {
            let dimension = game.dim.id().to_owned();
            let direct = game.native_neighbor_signal(&dimension, source);
            let property = game
                .native_block_property(source, "power")
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(0);
            let progress = dl::redstone_power_progress(direct, property);
            match mode {
                0 => Out::Value(dl::redstone_power_number(progress).to_string(), Section::Numeric),
                1 => Out::Value(dl::percent_text(progress), Section::Numeric),
                _ => Out::Bar(progress),
            }
        }
        "item_count" => {
            let stacks = stacks(game, source)?;
            let slots: Vec<(i32, bool)> = stacks.iter().map(|s| (s.count as i32, true)).collect();
            Out::Value(dl::item_count(&slots), Section::Numeric)
        }
        "item_name" => {
            let stacks = stacks(game, source)?;
            let first = stacks.first()?;
            let name = first
                .custom_name()
                .unwrap_or_else(|| game.pack.names.display_item(&first.item).to_string());
            Out::Value(name, Section::Alphabet)
        }
        "fluid_amount" => {
            let amount = tank_amount(game, source)?;
            Out::Value(format!("{}mB", dl::format_number(dl::fluid_amount(&[(amount, true)]) as f64)), Section::Numeric)
        }
        "kinetic_speed" => {
            let nbt = entity_nbt(game, source)?;
            if !nbt.contains_key(&"Speed".into()) {
                return None;
            }
            Out::Value(dl::kinetic_speed(number(nbt, "Speed"), mode == 1, "rpm"), Section::Numeric)
        }
        "kinetic_stress" => {
            let nbt = entity_nbt(game, source)?;
            if !nbt.contains_key(&"Capacity".into()) {
                return None;
            }
            let (stress, capacity) = (number(nbt, "Stress"), number(nbt, "Capacity"));
            let value = dl::stress_progress(mode, stress, capacity);
            if dl::stress_progress_bar_active(mode) {
                Out::Bar(value)
            } else {
                Out::Value(dl::stress_format(mode, value, true, "su"), Section::Numeric)
            }
        }
        _ => return None,
    })
}
