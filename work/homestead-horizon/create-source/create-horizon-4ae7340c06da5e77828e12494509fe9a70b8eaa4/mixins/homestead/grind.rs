// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Grind Enchantments, rebuilt from the mod's public description only (the mod is copyleft, its source was not
// read). In the grindstone an enchanted item (a tool, armour or an enchanted book) beside a plain book gives
// an enchanted book that carries the item's enchantments (curses stay on the item). Two modes:
//
//     mode=all      every non-curse enchantment moves to the book (default)
//     mode=first    only the first one moves, the rest stay on the item ("move the first enchantment")
//
// Taking the result leaves the item, without what moved, in the grindstone's first slot, uses the book up,
// charges `cost` levels (default 1; not in creative; a player with fewer levels pays what they have: the
// result itself cannot know the player) and awards no experience. Config: `config/homestead-grind.txt`
// (`enabled=false`, `mode=first|all`, `cost=<levels>`), read once.
//
// The result slot is answered by an injection into `stations::grindstone_result` (pure item logic shared by
// the preview, the host's menu and a LAN guest's menu); the take is the engine patch 22-grind-take.ps1 in
// menu.rs. Not ported: the original's configurable costs per enchantment level and its GUI buttons.

use crate::{enchant::Enchantments, item_data::Levels, items::ItemStack};
use std::sync::OnceLock;

struct Config {
    enabled: bool,
    first_only: bool,
    cost: i32,
}

fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let mut config = Config {
            enabled: true,
            first_only: false,
            cost: 1,
        };
        if let Ok(text) = std::fs::read_to_string("config/homestead-grind.txt") {
            for line in text.lines() {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let value = value.trim();
                match key.trim() {
                    "enabled" => config.enabled = !value.eq_ignore_ascii_case("false"),
                    "mode" => config.first_only = value.eq_ignore_ascii_case("first"),
                    "cost" => {
                        if let Ok(levels) = value.parse::<i32>() {
                            config.cost = levels.clamp(0, 100);
                        }
                    }
                    _ => {}
                }
            }
        }
        config
    })
}

/// What a grindstone with an enchanted item and a plain book does.
pub struct Plan {
    /// The enchanted book in the result slot.
    pub result: ItemStack,
    /// The item that stays in the first input slot when the result is taken.
    pub remaining: Option<ItemStack>,
    /// The levels the take costs.
    pub cost: i32,
}

fn is_plain_book(stack: &ItemStack) -> bool {
    &*stack.item == "minecraft:book" && !stack.has_any_enchantments()
}

/// The plan for the two inputs, or None when this is vanilla's grindstone business.
pub fn plan(
    enchantments: &Enchantments,
    first: Option<&ItemStack>,
    second: Option<&ItemStack>,
) -> Option<Plan> {
    let config = config();
    if !config.enabled {
        return None;
    }
    let (first, second) = (first?, second?);
    if first.count != 1 || second.count != 1 {
        return None;
    }
    let (source, book) = if is_plain_book(second) && first.has_any_enchantments() {
        (first, second)
    } else if is_plain_book(first) && second.has_any_enchantments() {
        (second, first)
    } else {
        return None;
    };
    let all: Levels = source.crafting_enchantments().to_vec();
    let movable: Vec<_> = all
        .iter()
        .filter(|(id, _)| !enchantments.is_curse(id))
        .cloned()
        .collect();
    if movable.is_empty() {
        return None;
    }
    let moved: Vec<_> = if config.first_only {
        movable.iter().take(1).cloned().collect()
    } else {
        movable
    };
    let kept: Levels = all.iter().filter(|e| !moved.contains(e)).cloned().collect();
    let mut result = book.transmute("minecraft:enchanted_book", 1);
    result.set_crafting_enchantments(moved.into_iter().collect());
    result.tidy();
    let mut remaining = source.clone();
    remaining.set_crafting_enchantments(kept);
    if &*remaining.item == "minecraft:enchanted_book" && remaining.stored_enchantments().is_empty()
    {
        remaining = remaining.transmute("minecraft:book", 1);
    }
    remaining.tidy();
    Some(Plan {
        result,
        remaining: Some(remaining),
        cost: config.cost,
    })
}
