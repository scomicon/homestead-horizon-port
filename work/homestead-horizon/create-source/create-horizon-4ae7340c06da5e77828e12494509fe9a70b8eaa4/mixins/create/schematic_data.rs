// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Schematic items and the data both the Schematicannon and the Schematic Table need:
// the `create:schematic_data` item component (SchematicItem's SCHEMATIC_FILE / OWNER /
// ANCHOR / DEPLOYED / ROTATION / MIRROR / BOUNDS in one compound), the checklist
// component of the Material Checklist item, the instance's schematics folder, and the
// `create cannon ...` operator commands that stand in for the Schematic Tool's
// in-world placement (see docs/schematicannon.md).
use super::Game;
use crate::{
    items::ItemStack,
    mod_api::{NativeCommand, NativeServerPlayer},
};
use create_engine::{
    material_checklist::{MAX_ENTRIES_PER_CLIPBOARD_PAGE, MaterialChecklist},
    schematic_tool::{EMPTY_SCHEMATIC_ITEM, Mirror, Rotation, SCHEMATIC_ITEM, SchematicStackData},
    schematicannon::Stack,
};
use horizon_world::nbt::Tag;

pub(super) const COMPONENT: &str = "create:schematic_data";
pub(super) const CHECKLIST_COMPONENT: &str = "create:checklist";
pub(super) const CLIPBOARD: &str = "create:clipboard";
pub(super) const CANNON: &str = "create:schematicannon";

pub(super) fn component_types(types: &mut Vec<String>) {
    types.push(COMPONENT.into());
    types.push(CHECKLIST_COMPONENT.into());
}

fn mirror_ordinal(mirror: Mirror) -> i32 {
    match mirror {
        Mirror::None => 0,
        Mirror::LeftRight => 1,
        Mirror::FrontBack => 2,
    }
}

fn ints(tag: &Tag, key: &str) -> Option<[i32; 3]> {
    match tag.get(key) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    }
}

fn text(tag: &Tag, key: &str) -> Option<String> {
    match tag.get(key) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

/// The schematic components of a stack, None for a stack without them.
pub(super) fn read(game: &Game<'_>, stack: &ItemStack) -> Option<SchematicStackData> {
    let tag = game.native_component(stack, COMPONENT).ok()??;
    let int = |key: &str| match tag.get(key) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    Some(SchematicStackData {
        file: text(tag, "File"),
        owner: text(tag, "Owner"),
        anchor: ints(tag, "Anchor"),
        deployed: matches!(tag.get("Deployed"), Some(Tag::Byte(v)) if *v != 0),
        rotation: Rotation::from_ordinal(int("Rotation")),
        mirror: match int("Mirror") {
            1 => Mirror::LeftRight,
            2 => Mirror::FrontBack,
            _ => Mirror::None,
        },
        bounds: ints(tag, "Bounds"),
    })
}

pub(super) fn write(game: &Game<'_>, stack: &mut ItemStack, data: &SchematicStackData) -> bool {
    let mut entries: Vec<(&str, Tag)> = vec![
        ("Deployed", Tag::Byte(i8::from(data.deployed))),
        ("Rotation", Tag::Int(data.rotation.ordinal())),
        ("Mirror", Tag::Int(mirror_ordinal(data.mirror))),
    ];
    if let Some(file) = &data.file {
        entries.push(("File", Tag::String(file.as_str().into())));
    }
    if let Some(owner) = &data.owner {
        entries.push(("Owner", Tag::String(owner.as_str().into())));
    }
    if let Some(a) = data.anchor {
        entries.push(("Anchor", Tag::IntArray(a.to_vec())));
    }
    if let Some(b) = data.bounds {
        entries.push(("Bounds", Tag::IntArray(b.to_vec())));
    }
    game.native_set_component(stack, COMPONENT, Tag::compound(entries)).is_ok()
}

/// A new Schematic item for `file` (SchematicItem.create: not deployed, anchor zero).
pub(super) fn schematic_stack(game: &Game<'_>, owner: &str, file: &str, bounds: [i32; 3]) -> Option<ItemStack> {
    let mut stack = ItemStack::new(SCHEMATIC_ITEM, 1);
    write(game, &mut stack, &SchematicStackData::create(owner, file, bounds)).then_some(stack)
}

/// A game stack as the cannon core sees it.
pub(super) fn cannon_stack(game: &Game<'_>, stack: &ItemStack) -> Stack {
    let item = stack.item.to_string();
    let schematic = if item == SCHEMATIC_ITEM { read(game, stack) } else { None };
    Stack { item, count: stack.count as i32, schematic }
}

// ------------------------------------------------------------------- files

fn valid_name(name: &str) -> bool {
    name.ends_with(".nbt")
        && name.len() <= 128
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

/// The `.nbt` files of the instance's `schematics` folder, sorted (plain names only).
pub(super) fn files() -> Vec<String> {
    let Ok(read) = std::fs::read_dir(Game::game_dir().join("schematics")) else {
        return Vec::new();
    };
    let mut names: Vec<String> = read
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| valid_name(name))
        .collect();
    names.sort();
    names
}

/// Size of a structure file (no block validation: the cannon preflights when it loads it).
pub(super) fn file_size(name: &str) -> Result<[i32; 3], String> {
    let bytes = super::create_port_schematic_runtime::read_file(name)?;
    create_schematics::read_gzip(&bytes, &create_schematics::Limits::default())
        .map(|s| s.size)
        .map_err(|e| e.to_string())
}

// --------------------------------------------------------------- checklist

fn pretty(item: &str) -> String {
    let name = item.split_once(':').map_or(item, |(_, n)| n);
    name.split('_')
        .map(|w| {
            let mut c = w.chars();
            c.next().map_or(String::new(), |f| f.to_uppercase().collect::<String>() + c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Writes the checklist into a Material Checklist stack (`Entries`: unfinished first, as
/// MaterialChecklist.createWrittenClipboard orders them).
pub(super) fn write_checklist(game: &Game<'_>, stack: &mut ItemStack, list: &MaterialChecklist) -> bool {
    let layout = list.layout(MAX_ENTRIES_PER_CLIPBOARD_PAGE, &|item| pretty(item), &|_| 1);
    let entries: Vec<Tag> = layout
        .pages
        .iter()
        .flat_map(|page| page.entries.iter())
        .map(|e| {
            Tag::compound([
                ("Item", Tag::String(e.item.as_str().into())),
                ("Amount", Tag::Int(e.amount)),
                ("Done", Tag::Byte(i8::from(e.completed))),
            ])
        })
        .collect();
    let notice = i8::from(layout.blocks_not_loaded_notice);
    game.native_set_component(
        stack,
        CHECKLIST_COMPONENT,
        Tag::compound([("Entries", Tag::List(entries)), ("NotLoaded", Tag::Byte(notice))]),
    )
    .is_ok()
}

pub(super) fn checklist_lines(game: &Game<'_>, stack: &ItemStack) -> Vec<String> {
    let Some(tag) = game.native_component(stack, CHECKLIST_COMPONENT).ok().flatten() else {
        return Vec::new();
    };
    let mut lines = Vec::new();
    if matches!(tag.get("NotLoaded"), Some(Tag::Byte(v)) if *v != 0) {
        lines.push("Some blocks are not loaded, the list may be incomplete.".to_owned());
    }
    if let Some(Tag::List(entries)) = tag.get("Entries") {
        for e in entries {
            let item = text(e, "Item").unwrap_or_default();
            let amount = match e.get("Amount") {
                Some(Tag::Int(v)) => *v,
                _ => 0,
            };
            let done = matches!(e.get("Done"), Some(Tag::Byte(v)) if *v != 0);
            lines.push(format!(
                "[{}] {} x{} | {}\u{25A4} +{}",
                if done { "x" } else { " " },
                pretty(&item),
                amount,
                amount / 64,
                amount % 64
            ));
        }
    }
    lines
}

// ---------------------------------------------------------------- commands

const SYNTAXES: &[&str] = &[
    "create cannon list",
    "create cannon checklist",
    "create cannon give <filename:word>",
    "create cannon status <x:integer> <y:integer> <z:integer>",
    "create cannon control <x:integer> <y:integer> <z:integer> <action:word>",
    "create cannon deploy <x:integer> <y:integer> <z:integer>",
    "create cannon deploy <x:integer> <y:integer> <z:integer> <quarter_turns:integer>",
    "create cannon deploy <x:integer> <y:integer> <z:integer> <quarter_turns:integer> <mirror:word>",
    "create cannon load <x:integer> <y:integer> <z:integer> <filename:word> <ax:integer> <ay:integer> <az:integer>",
    "create cannon load <x:integer> <y:integer> <z:integer> <filename:word> <ax:integer> <ay:integer> <az:integer> <quarter_turns:integer>",
    "create cannon load <x:integer> <y:integer> <z:integer> <filename:word> <ax:integer> <ay:integer> <az:integer> <quarter_turns:integer> <mirror:word>",
];

pub(super) fn register(game: &Game<'_>, commands: &mut Vec<NativeCommand>) {
    if game.native_on("create") {
        for syntax in SYNTAXES {
            commands.push(NativeCommand::op(*syntax, 2));
        }
    }
}

fn say(game: &mut Game<'_>, message: &str) {
    game.native_feedback(horizon_client::text::Text::literal(message));
}

fn turns_and_mirror(words: &[&str]) -> Result<(Rotation, Mirror), String> {
    let rotation = match words.first() {
        None => Rotation::None,
        Some(t) => Rotation::from_ordinal(
            t.parse::<u8>()
                .ok()
                .filter(|t| *t <= 3)
                .ok_or("quarter_turns must be 0 to 3")?
                .into(),
        ),
    };
    let mirror = match words.get(1).copied() {
        None | Some("none") => Mirror::None,
        Some("left_right") => Mirror::LeftRight,
        Some("front_back") => Mirror::FrontBack,
        Some(_) => return Err("mirror must be none, left_right or front_back".into()),
    };
    Ok((rotation, mirror))
}

fn coords(words: &[&str]) -> Result<[i32; 3], String> {
    let mut out = [0; 3];
    for (slot, word) in out.iter_mut().zip(words) {
        *slot = word.parse().map_err(|_| "coordinates must be integers".to_owned())?;
    }
    if words.len() < 3 {
        return Err("three coordinates are required".into());
    }
    Ok(out)
}

fn run(game: &mut Game<'_>, player: &NativeServerPlayer, words: &[&str]) -> Result<String, String> {
    match words {
        ["list"] => {
            let files = files();
            Ok(if files.is_empty() { "no .nbt files in the schematics folder".into() } else { files.join(", ") })
        }
        ["give", file] => {
            let size = file_size(file)?;
            let stack = schematic_stack(game, &player.name, file, size).ok_or("component write failed")?;
            if let Some(rest) = game.native_give_stack(player.uuid, stack) {
                game.spawn_item(rest, player.position, [0.0, 0.2, 0.0], 10);
            }
            Ok(format!("gave a Schematic for {file}, size {size:?}"))
        }
        ["deploy", rest @ ..] if rest.len() >= 3 => {
            let anchor = coords(rest)?;
            let (rotation, mirror) = turns_and_mirror(&rest[3..])?;
            let mut stack = game.held().cloned().ok_or("hold a Schematic in the selected hotbar slot")?;
            if &*stack.item != SCHEMATIC_ITEM {
                return Err("hold a Schematic in the selected hotbar slot".into());
            }
            let mut data = read(game, &stack).ok_or("this Schematic has no file")?;
            data.anchor = Some(anchor);
            data.deployed = true;
            data.rotation = rotation;
            data.mirror = mirror;
            if !write(game, &mut stack, &data) {
                return Err("component write failed".into());
            }
            *game.held_mut() = Some(stack);
            Ok(format!("deployed at {anchor:?}"))
        }
        ["load", rest @ ..] if rest.len() >= 7 => {
            let cannon = coords(rest)?;
            let file = rest[3];
            let anchor = coords(&rest[4..])?;
            let (rotation, mirror) = turns_and_mirror(&rest[7..])?;
            let size = file_size(file)?;
            let mut stack = schematic_stack(game, &player.name, file, size).ok_or("component write failed")?;
            let mut data = read(game, &stack).ok_or("component read failed")?;
            data.anchor = Some(anchor);
            data.deployed = true;
            data.rotation = rotation;
            data.mirror = mirror;
            if !write(game, &mut stack, &data) {
                return Err("component write failed".into());
            }
            let placed = game
                .native_with_mod_block_entity(cannon, |e| {
                    if e.id != CANNON || e.items.is_empty() {
                        return false;
                    }
                    e.items[0] = Some(stack);
                    true
                })
                .unwrap_or(false);
            if !placed {
                return Err("no Schematicannon there".into());
            }
            game.native_block_entity_sync(cannon);
            Ok(format!("loaded {file} into the cannon, anchor {anchor:?}"))
        }
        ["status", rest @ ..] if rest.len() == 3 => {
            let at = coords(rest)?;
            super::create_port_cannon::status_line(game, at).ok_or_else(|| "no Schematicannon there".to_owned())
        }
        ["control", rest @ ..] if rest.len() == 4 => {
            let at = coords(rest)?;
            super::create_port_cannon::control(game, at, rest[3])
                .or_else(|_| super::create_port_schematic_table::control(game, at, rest[3]))
        }
        ["checklist"] => {
            let stack = game.held().cloned().ok_or("hold a Material Checklist")?;
            if &*stack.item != CLIPBOARD {
                return Err("hold a Material Checklist".into());
            }
            let lines = checklist_lines(game, &stack);
            Ok(if lines.is_empty() { "the checklist is blank".into() } else { lines.join("\n") })
        }
        _ => Err("usage: create cannon list|status X Y Z|control X Y Z ACTION|give FILE|deploy X Y Z [turns [mirror]]|load CX CY CZ FILE AX AY AZ [turns [mirror]]|checklist".into()),
    }
}

pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, line: &str, ran: &mut bool) {
    let words = line.split_whitespace().collect::<Vec<_>>();
    if words.get(..2) != Some(&["create", "cannon"][..]) || !game.native_on("create") {
        return;
    }
    *ran = true;
    if player.permission < 2 {
        say(game, "create cannon commands need operator permission 2");
        return;
    }
    let message = match run(game, player, &words[2..]) {
        Ok(ok) => ok,
        Err(e) => format!("create cannon: {e}"),
    };
    say(game, &message);
}

/// Empty Schematic stack for the Schematic Table's upload.
pub(super) fn empty_schematic(count: u32) -> ItemStack {
    ItemStack::new(EMPTY_SCHEMATIC_ITEM, count)
}
