// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Schematic item and Schematic and Quill, on top of the pure rules of
// `create_engine::schematic_tool`:
//
// * Schematic and Quill: two corners by right click on blocks (SchematicAndQuillHandler:
//   a vertical face of a solid block selects the neighbouring block), sneak discards,
//   export of the box to `schematics/<name>.nbt` with SchematicExport's naming
//   (`FilesHelper.slug`, `_1`, `_2` ... unless replacing).
// * Schematic: right click on a block deploys it with its anchor next to the clicked
//   face, sneak + right click picks it up again; rotation, mirror and anchor are changed
//   through `Transformation` (the maths of SchematicTransformation) and written back as a
//   SchematicSyncPacket would.
// * `create schematool ...` operator commands that stand in for the name prompt, the tool
//   wheel and the edit screen (none of which exist in this port); no ghost preview.
//
// Not ported: the in-world ghost, outlines, the name prompt screen, the tool wheel and its
// key bindings, the scroll wheel (range / resizing), the schematic edit screen, entities and
// block entity data in exports (the loader only accepts schemas it knows).
use super::{Game, create_port_schematic_data as data};
use crate::{
    items::ItemStack,
    mod_api::{NativeCommand, NativeServerPlayer},
};
use create_engine::schematic_tool::{
    Direction, Mirror, QuillClick, QuillSelection, Rotation, SCHEMATIC_AND_QUILL_ITEM, SCHEMATIC_ITEM,
    SchematicStackData, SyncPacket, Transformation, export_bounds, export_file_name, sync_accepts,
};
use create_schematics::{Block, BlockState, Limits, Schematic};
use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard, OnceLock},
};

type Pos = [i32; 3];

/// `schematicAndQuill.fallbackName`.
const FALLBACK_NAME: &str = "My Schematic";

static QUILL: OnceLock<Mutex<QuillSelection>> = OnceLock::new();

/// The selection of the (single, client side in Create) Schematic and Quill handler.
fn quill() -> MutexGuard<'static, QuillSelection> {
    QUILL
        .get_or_init(|| Mutex::new(QuillSelection::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

// ---------------------------------------------------------------- commands

const SYNTAXES: &[&str] = &[
    "create schematool quill",
    "create schematool corner <x:integer> <y:integer> <z:integer>",
    "create schematool discard",
    "create schematool selection",
    "create schematool save",
    "create schematool save <name:word>",
    "create schematool replace <name:word>",
    "create schematool info",
    "create schematool rotate <direction:word>",
    "create schematool mirror <axis:word>",
    "create schematool move <dx:integer> <dy:integer> <dz:integer>",
    "create schematool anchor <x:integer> <y:integer> <z:integer>",
    "create schematool deploy",
    "create schematool deploy <x:integer> <y:integer> <z:integer>",
    "create schematool undeploy",
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

fn coords(words: &[&str]) -> Result<Pos, String> {
    if words.len() < 3 {
        return Err("three coordinates are required".into());
    }
    let mut out = [0; 3];
    for (slot, word) in out.iter_mut().zip(words) {
        *slot = word.parse().map_err(|_| "coordinates must be integers".to_owned())?;
    }
    Ok(out)
}

fn run(game: &mut Game<'_>, player: &NativeServerPlayer, words: &[&str]) -> Result<String, String> {
    match words {
        ["quill"] => {
            let stack = ItemStack::new(SCHEMATIC_AND_QUILL_ITEM, 1);
            if let Some(rest) = game.native_give_stack(player.uuid, stack) {
                game.spawn_item(rest, player.position, [0.0, 0.2, 0.0], 10);
            }
            Ok("gave a Schematic and Quill".into())
        }
        ["corner", rest @ ..] if rest.len() == 3 => {
            let at = coords(rest)?;
            Ok(click(at, false))
        }
        ["discard"] => {
            quill().discard();
            Ok("selection discarded".into())
        }
        ["selection"] => Ok(selection_text()),
        ["save"] => save(game, FALLBACK_NAME, false),
        ["save", name] => save(game, name, false),
        ["replace", name] => save(game, name, true),
        ["info"] => {
            let (_, d) = held_schematic(game)?;
            Ok(info_text(&d))
        }
        ["rotate", direction] => {
            let clockwise = match *direction {
                "cw" | "clockwise" => true,
                "ccw" | "counterclockwise" => false,
                _ => return Err("direction must be cw or ccw".into()),
            };
            edit_held(game, |t| t.rotate90(clockwise))
        }
        ["mirror", axis] => {
            let axis = match *axis {
                "x" => 'x',
                "z" => 'z',
                _ => return Err("axis must be x or z".into()),
            };
            edit_held(game, |t| t.flip(axis))
        }
        ["move", rest @ ..] if rest.len() == 3 => {
            let delta = coords(rest)?;
            edit_held(game, |t| t.move_by(delta))
        }
        ["anchor", rest @ ..] if rest.len() == 3 => {
            let anchor = coords(rest)?;
            edit_held(game, |t| t.move_to(t.from_anchor(anchor)))
        }
        ["deploy"] => set_deployed(game, None, true),
        ["deploy", rest @ ..] if rest.len() == 3 => set_deployed(game, Some(coords(rest)?), true),
        ["undeploy"] => set_deployed(game, None, false),
        _ => Err("usage: create schematool quill|corner X Y Z|discard|selection|save [NAME]|replace NAME|info|rotate cw|ccw|mirror x|z|move DX DY DZ|anchor X Y Z|deploy [X Y Z]|undeploy".into()),
    }
}

pub(super) fn command(game: &mut Game<'_>, player: &NativeServerPlayer, line: &str, ran: &mut bool) {
    let words = line.split_whitespace().collect::<Vec<_>>();
    if words.get(..2) != Some(&["create", "schematool"][..]) || !game.native_on("create") {
        return;
    }
    *ran = true;
    if player.permission < 2 {
        say(game, "create schematool commands need operator permission 2");
        return;
    }
    let message = match run(game, player, &words[2..]) {
        Ok(ok) => ok,
        Err(e) => format!("create schematool: {e}"),
    };
    say(game, &message);
}

// ------------------------------------------------------------------- quill

fn size_text(a: Pos, b: Pos) -> String {
    let (_, size) = export_bounds(a, b);
    format!("{} x {} x {}", size[0], size[1], size[2])
}

fn selection_text() -> String {
    let sel = quill();
    match (sel.first, sel.second) {
        (None, _) => "no corner selected".into(),
        (Some(a), None) => format!("first corner {a:?}, second corner not set"),
        (Some(a), Some(b)) => format!("first corner {a:?}, second corner {b:?}, size {}", size_text(a, b)),
    }
}

/// One right click of the Schematic and Quill on `target` (already the selected block).
fn click(target: Pos, sneaking: bool) -> String {
    let mut sel = quill();
    sel.selected = Some(target);
    match sel.right_click(true, sneaking) {
        QuillClick::NotHandled => String::new(),
        QuillClick::Discarded => "selection discarded".into(),
        QuillClick::NoTarget => "no block targeted".into(),
        QuillClick::FirstPos => format!("first corner {target:?}"),
        QuillClick::SecondPos | QuillClick::OpenPrompt => {
            let (a, b) = (sel.first.unwrap_or(target), sel.second.unwrap_or(target));
            format!(
                "second corner {b:?}, size {}: save it with /create schematool save <name>",
                size_text(a, b)
            )
        }
    }
}

fn direction(face: Pos) -> Option<Direction> {
    Some(match face {
        [0, -1, 0] => Direction::Down,
        [0, 1, 0] => Direction::Up,
        [0, 0, -1] => Direction::North,
        [0, 0, 1] => Direction::South,
        [-1, 0, 0] => Direction::West,
        [1, 0, 0] => Direction::East,
        _ => return None,
    })
}

fn valid_file(name: &str) -> bool {
    name.ends_with(".nbt")
        && name.len() <= 128
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

/// Writes the selected box to the schematics folder. Returns the report line.
fn save(game: &mut Game<'_>, name: &str, overwrite: bool) -> Result<String, String> {
    let (first, second) = {
        let sel = quill();
        (sel.first, sel.second)
    };
    let (Some(a), Some(b)) = (first, second) else {
        return Err("select two corners with the Schematic and Quill first".into());
    };
    let limits = Limits::default();
    let (origin, size) = export_bounds(a, b);
    if size.iter().any(|s| *s > limits.max_side) {
        return Err(format!("a side is longer than {} blocks", limits.max_side));
    }
    if size.iter().map(|s| *s as u64).product::<u64>() > limits.max_volume {
        return Err(format!("the box is larger than {} blocks", limits.max_volume));
    }
    let registry = game.pack.assets.registry;
    let mut palette: Vec<BlockState> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut blocks = Vec::new();
    let mut dropped_entities = 0usize;
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                let world = [origin[0] + x, origin[1] + y, origin[2] + z];
                if !game.world.loaded(world[0], world[2]) {
                    return Err(format!("the chunk at {} {} is not loaded", world[0], world[2]));
                }
                let id = game.world.block(world[0], world[1], world[2]);
                if game.res.behaviours().is_air(id) {
                    continue;
                }
                let described = registry.state(id).ok_or("unknown block state in the selection")?;
                let state = BlockState {
                    name: described.name.to_string(),
                    properties: described
                        .properties
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                        .collect(),
                };
                if game.world.block_entities.contains_key(&(world[0], world[1], world[2])) {
                    dropped_entities += 1;
                }
                let key = state.command_string();
                let state_index = *index.entry(key).or_insert_with(|| {
                    palette.push(state);
                    palette.len() - 1
                });
                blocks.push(Block { relative_pos: [x, y, z], state_index, block_entity: None });
            }
        }
    }
    if blocks.is_empty() {
        return Err("the selection holds only air".into());
    }
    if blocks.len() > limits.max_blocks {
        return Err(format!("more than {} blocks", limits.max_blocks));
    }
    let count = blocks.len();
    let schematic = Schematic {
        data_version: create_schematics::SOURCE_DATA_VERSION,
        size,
        palettes: vec![palette],
        blocks,
        entities: Vec::new(),
    };
    let bytes = create_schematics::nbt::encode_gzip(&schematic.to_compound(), &limits)
        .map_err(|e| e.to_string())?;
    let directory = Game::game_dir().join("schematics");
    let exists = |candidate: &str| directory.join(candidate).exists();
    let file = export_file_name(name, overwrite, FALLBACK_NAME, &exists);
    if !valid_file(&file) {
        return Err(format!("{file} is not a plain file name (letters, digits, _ . -)"));
    }
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    std::fs::write(directory.join(&file), bytes).map_err(|e| e.to_string())?;
    quill().finish_save(true);
    let mut line = format!(
        "saved {file}: {} x {} x {}, {count} blocks",
        size[0], size[1], size[2]
    );
    if dropped_entities > 0 {
        line.push_str(&format!(", {dropped_entities} block entities saved without their data"));
    }
    line.push_str("; hand it out with /create cannon give ");
    line.push_str(&file);
    Ok(line)
}

// --------------------------------------------------------------- schematic

const HOLD: &str = "hold a Schematic in the selected hotbar slot";

fn held_schematic(game: &Game<'_>) -> Result<(ItemStack, SchematicStackData), String> {
    let stack = game.held().cloned().ok_or(HOLD)?;
    if !sync_accepts(&stack.item) {
        return Err(HOLD.into());
    }
    let d = data::read(game, &stack).ok_or("this Schematic has no file")?;
    Ok((stack, d))
}

fn store(game: &mut Game<'_>, mut stack: ItemStack, d: &SchematicStackData) -> Result<(), String> {
    if !data::write(game, &mut stack, d) {
        return Err("component write failed".into());
    }
    *game.held_mut() = Some(stack);
    Ok(())
}

fn mirror_name(mirror: Mirror) -> &'static str {
    match mirror {
        Mirror::None => "none",
        Mirror::LeftRight => "left_right",
        Mirror::FrontBack => "front_back",
    }
}

fn info_text(d: &SchematicStackData) -> String {
    format!(
        "file {}, bounds {:?}, anchor {:?}, quarter turns {}, mirror {}, {}",
        d.file.as_deref().unwrap_or("-"),
        d.bounds.unwrap_or([0; 3]),
        d.anchor.unwrap_or([0; 3]),
        d.rotation.ordinal(),
        mirror_name(d.mirror),
        if d.deployed { "deployed" } else { "not deployed" }
    )
}

/// SchematicHandler's client transformation applied to the stack's settings, as the
/// SchematicSyncPacket it would send: the anchor follows the transformation.
fn edit(d: &SchematicStackData, op: impl FnOnce(&mut Transformation)) -> Result<SyncPacket, String> {
    let size = d.bounds.ok_or("this Schematic has no bounds")?;
    let mut t = Transformation::init(d.anchor.unwrap_or([0; 3]), d.rotation, d.mirror, size);
    op(&mut t);
    let (rotation, mirror) = t.to_settings();
    Ok(SyncPacket { slot: -1, rotation, mirror, anchor: t.anchor(), deployed: d.deployed })
}

fn edit_held(game: &mut Game<'_>, op: impl FnOnce(&mut Transformation)) -> Result<String, String> {
    let (stack, mut d) = held_schematic(game)?;
    let sync = edit(&d, op)?;
    d.apply_sync(&sync);
    store(game, stack, &d)?;
    Ok(info_text(&d))
}

fn set_deployed(game: &mut Game<'_>, anchor: Option<Pos>, deployed: bool) -> Result<String, String> {
    let (stack, mut d) = held_schematic(game)?;
    if let Some(anchor) = anchor {
        let sync = edit(&d, |t| t.move_to(t.from_anchor(anchor)))?;
        d.apply_sync(&sync);
    }
    d.deployed = deployed;
    store(game, stack, &d)?;
    Ok(info_text(&d))
}

// ------------------------------------------------------------------- hooks

fn has_block_entity(game: &Game<'_>, pos: Pos) -> bool {
    game.world.block_entities.contains_key(&(pos[0], pos[1], pos[2]))
}

/// `use_item_on`: right click on a block with a Schematic and Quill or a Schematic.
pub(super) fn use_on(game: &mut Game<'_>, stack: &ItemStack, pos: Pos, face: Pos, claimed: &mut bool) {
    if !game.native_on("create") {
        return;
    }
    let item: &str = &stack.item;
    if item == SCHEMATIC_AND_QUILL_ITEM {
        let Some(face) = direction(face) else { return };
        let target = QuillSelection::target_from_hit(pos, face, false);
        let message = click(target, game.native_sneaking());
        if !message.is_empty() {
            say(game, &message);
        }
        *claimed = true;
    } else if item == SCHEMATIC_ITEM {
        // Blocks with an inventory or a screen (Schematicannon, Schematic Table, chests) keep
        // their own right click.
        if has_block_entity(game, pos) {
            return;
        }
        let sneaking = game.native_sneaking();
        let Some(mut d) = data::read(game, stack) else { return };
        if sneaking && d.deployed {
            d.deployed = false;
        } else if !sneaking && !d.deployed {
            let anchor = [pos[0] + face[0], pos[1] + face[1], pos[2] + face[2]];
            d.anchor = Some(anchor);
            d.deployed = true;
        } else {
            return;
        }
        let mut next = stack.clone();
        if data::write(game, &mut next, &d) {
            *game.held_mut() = Some(next);
            let message = info_text(&d);
            say(game, &message);
        }
        *claimed = true;
    }
}

/// `use_item`: sneak + right click in the air discards the Quill's selection.
pub(super) fn use_air(game: &mut Game<'_>, stack: &ItemStack, claimed: &mut bool) {
    if !game.native_on("create") || &*stack.item != SCHEMATIC_AND_QUILL_ITEM || !game.native_sneaking() {
        return;
    }
    let had = {
        let mut sel = quill();
        let had = sel.first.is_some() || sel.second.is_some();
        sel.discard();
        had
    };
    if had {
        say(game, "selection discarded");
        *claimed = true;
    }
}
