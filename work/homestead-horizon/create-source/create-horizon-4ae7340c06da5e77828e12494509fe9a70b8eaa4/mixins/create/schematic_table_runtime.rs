// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Schematic Table (SchematicTableBlock / SchematicTableBlockEntity / SchematicTableMenu /
// SchematicTableScreen) over `create_engine::schematic_table`.
//
// The original uploads a file the player picks on their own computer, in packets, with a
// progress bar. This port has no file chooser and no upload packets: the screen lists the
// `.nbt` files in the instance's `schematics` folder (the one `create schematic` also
// reads), the buttons pick one, and "Upload" turns the Empty Schematic in the input slot
// into a Schematic for that file in the output slot at once. See docs/schematicannon.md.
use super::{
    Game, create_port_cannon as ui, create_port_schematic_data as data,
};
use crate::{
    containers::{BlockEntity, ModBlockEntity, ModContainer},
    items::ItemStack,
};
use create_engine::{
    schematic_table::{TABLE_INPUT_SLOT, TABLE_OUTPUT_SLOT, table_may_place},
    schematic_tool::EMPTY_SCHEMATIC_ITEM,
};
use horizon_world::nbt::{Compound, JavaString, Tag};

pub(super) const BLOCK: &str = "create:schematic_table";
/// The menu type declared by `data/create/horizon/menu/schematic_table.json`.
const MENU: &str = "create:schematic_table";
const SLOTS: usize = 2;

const K_SELECTED: &str = "Selected";
const K_FILE: &str = "File";
const K_ERROR: &str = "Error";
const K_COUNT: &str = "FileCount";
const K_COOLDOWN: &str = "Rescan";
/// Ticks between looks at the schematics folder.
const RESCAN: i32 = 40;

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

pub(super) fn entity(block: &str, entity: &mut Option<(String, Compound)>) {
    if block == BLOCK && entity.is_none() {
        *entity = Some((block.into(), Compound::new()));
    }
}

pub(super) fn ticks(block: &str, ticks: &mut bool) {
    if block == BLOCK {
        *ticks = true;
    }
}

pub(super) fn container(block: &str, container: &mut Option<ModContainer>) {
    if block == BLOCK && container.is_none() {
        *container = Some(ModContainer {
            slots: SLOTS,
            faces: std::array::from_fn(|_| Vec::new()),
            inputs: Vec::new(),
            outputs: Vec::new(),
            menu: Some(MENU.into()),
        });
    }
}

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    }
}

fn string(nbt: &Compound, key: &str) -> String {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => s.to_string_lossy(),
        _ => String::new(),
    }
}

fn put(nbt: &mut Compound, key: &str, tag: Tag) -> bool {
    if nbt.get(&key.into()) == Some(&tag) {
        return false;
    }
    nbt.insert(key.into(), tag);
    true
}

pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    let text = |key: &str| -> (JavaString, Tag) { (key.into(), Tag::String(string(&entity.nbt, key).as_str().into())) };
    Compound::from([
        text(K_FILE),
        text(K_ERROR),
        (K_COUNT.into(), Tag::Int(int(&entity.nbt, K_COUNT))),
        (K_SELECTED.into(), Tag::Int(int(&entity.nbt, K_SELECTED))),
    ])
}

/// Re-reads the folder and the selection into the entity. True when a client-visible value changed.
fn refresh(entity: &mut ModBlockEntity) -> bool {
    let files = data::files();
    let count = i32::try_from(files.len()).unwrap_or(i32::MAX);
    let selected = int(&entity.nbt, K_SELECTED).clamp(0, (count - 1).max(0));
    let name = files.get(selected as usize).cloned().unwrap_or_default();
    let mut changed = put(&mut entity.nbt, K_COUNT, Tag::Int(count));
    changed |= put(&mut entity.nbt, K_SELECTED, Tag::Int(selected));
    changed |= put(&mut entity.nbt, K_FILE, Tag::String(name.as_str().into()));
    changed
}

fn publish(entity: &mut ModBlockEntity) {
    let ready = entity.items.len() >= SLOTS
        && entity.items[TABLE_INPUT_SLOT].as_ref().is_some_and(|s| &*s.item == EMPTY_SCHEMATIC_ITEM && s.count > 0)
        && entity.items[TABLE_OUTPUT_SLOT].is_none()
        && int(&entity.nbt, K_COUNT) > 0;
    let fresh = vec![int(&entity.nbt, K_COUNT), int(&entity.nbt, K_SELECTED), i32::from(ready)];
    if entity.data != fresh {
        entity.data = fresh;
    }
}

pub(super) fn tick(game: &mut Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    if entity.id != BLOCK || !simulates(game) {
        return;
    }
    let wait = int(&entity.nbt, K_COOLDOWN) - 1;
    let mut changed = false;
    if wait <= 0 {
        changed = refresh(entity);
        put(&mut entity.nbt, K_COOLDOWN, Tag::Int(RESCAN));
    } else {
        put(&mut entity.nbt, K_COOLDOWN, Tag::Int(wait));
    }
    publish(entity);
    if changed {
        game.native_block_entity_sync(pos);
    }
}

/// The Stack a table makes for its selected file (the Empty Schematic becomes this).
fn upload_stack(game: &Game<'_>, file: &str) -> Result<ItemStack, String> {
    if file.is_empty() {
        return Err("no file selected".into());
    }
    let size = data::file_size(file)?;
    data::schematic_stack(game, "Player", file, size).ok_or_else(|| "component write failed".to_owned())
}

/// The buttons: 1 previous file, 2 next file, 3 upload. `upload` is the Schematic the upload
/// would produce (built by the caller, which has the game).
fn press(entity: &mut ModBlockEntity, id: i32, upload: Result<ItemStack, String>) {
    if entity.items.len() < SLOTS {
        return;
    }
    refresh(entity);
    let count = int(&entity.nbt, K_COUNT);
    let selected = int(&entity.nbt, K_SELECTED);
    match id {
        1 | 2 if count > 0 => {
            let next = (selected + if id == 1 { count - 1 } else { 1 }) % count;
            put(&mut entity.nbt, K_SELECTED, Tag::Int(next));
            refresh(entity);
        }
        3 => {
            let can_write = entity.items[TABLE_INPUT_SLOT]
                .as_ref()
                .is_some_and(|s| &*s.item == EMPTY_SCHEMATIC_ITEM && s.count > 0)
                && entity.items[TABLE_OUTPUT_SLOT].is_none();
            if !can_write {
                return;
            }
            match upload {
                Ok(stack) => {
                    // `SchematicTable.start_upload` removes the whole input stack.
                    entity.items[TABLE_INPUT_SLOT] = None;
                    entity.items[TABLE_OUTPUT_SLOT] = Some(stack);
                    put(&mut entity.nbt, K_ERROR, Tag::String("".into()));
                }
                Err(e) => {
                    put(&mut entity.nbt, K_ERROR, Tag::String(e.chars().take(60).collect::<String>().as_str().into()));
                }
            }
        }
        _ => {}
    }
    publish(entity);
}

pub(super) fn menu_button(game: &mut Game<'_>, _pos: [i32; 3], entity: &mut ModBlockEntity, id: i32) {
    if entity.id != BLOCK || !simulates(game) {
        return;
    }
    refresh(entity);
    let upload = if id == 3 { upload_stack(game, &string(&entity.nbt, K_FILE)) } else { Err(String::new()) };
    press(entity, id, upload);
}

/// The operator command `create cannon control X Y Z prev|next|upload` for a table.
pub(super) fn control(game: &mut Game<'_>, pos: [i32; 3], action: &str) -> Result<String, String> {
    let id = match action {
        "prev" => 1,
        "next" => 2,
        "upload" => 3,
        _ => return Err("action: prev|next|upload".into()),
    };
    let file = game
        .native_with_mod_block_entity(pos, |entity| {
            (entity.id == BLOCK).then(|| {
                refresh(entity);
                string(&entity.nbt, K_FILE)
            })
        })
        .flatten()
        .ok_or_else(|| "no Schematic Table there".to_owned())?;
    let upload = if id == 3 { upload_stack(game, &file) } else { Err(String::new()) };
    game.native_with_mod_block_entity(pos, |entity| {
        press(entity, id, upload);
        format!(
            "file {} of {}: {}{}",
            int(&entity.nbt, K_SELECTED) + 1,
            int(&entity.nbt, K_COUNT),
            string(&entity.nbt, K_FILE),
            match string(&entity.nbt, K_ERROR) {
                e if e.is_empty() => String::new(),
                e => format!(" (error: {e})"),
            }
        )
    })
    .ok_or_else(|| "no Schematic Table there".to_owned())
}
/// Shift-click from the inventory into the input slot (`SchematicTableMenu.quickMoveStack`).
pub(super) fn quick_move(
    game: &Game<'_>,
    entity: &mut ModBlockEntity,
    container_slots: usize,
    slot: usize,
    stack: &mut ItemStack,
    handled: &mut bool,
) {
    if entity.id != BLOCK || slot < container_slots || entity.items.len() < SLOTS {
        return;
    }
    *handled = true;
    if !table_may_place(TABLE_INPUT_SLOT, &stack.item) {
        return;
    }
    let limit = game.native_max_stack_size(&stack.item).unwrap_or(64);
    match &mut entity.items[TABLE_INPUT_SLOT] {
        Some(held) if held.same_item_same_components(stack) => {
            let moved = stack.count.min(limit.saturating_sub(held.count));
            held.count += moved;
            stack.count -= moved;
        }
        Some(_) => {}
        empty => {
            let moved = stack.count.min(limit);
            let mut put_in = stack.clone();
            put_in.count = moved;
            stack.count -= moved;
            *empty = Some(put_in);
        }
    }
}

// ------------------------------------------------------------------ screen

const WIDTH: f32 = 176.0;
const HEIGHT: f32 = 166.0;
const SLOT_POS: [[f32; 2]; SLOTS] = [[12.0, 40.0], [148.0, 40.0]];
const INV_X: f32 = 8.0;
const INV_Y: f32 = 84.0;
/// id, x, y, width, height, label, tooltip.
const BUTTONS: [(i32, f32, f32, f32, f32, &str, &str); 3] = [
    (1, 36.0, 38.0, 14.0, 14.0, "<", "Previous file"),
    (2, 130.0, 38.0, 14.0, 14.0, ">", "Next file"),
    (3, 48.0, 60.0, 80.0, 14.0, "Upload", "Turn the Empty Schematic into a Schematic for this file"),
];

pub(super) fn screen(block: &str, screen: &mut Option<crate::mod_api::NativeScreen>) {
    if block != BLOCK || screen.is_some() {
        return;
    }
    *screen = Some(crate::mod_api::NativeScreen {
        width: WIDTH,
        height: HEIGHT,
        slots: SLOT_POS.iter().map(|p| Some(*p)).collect(),
        inventory_y: INV_Y,
        inventory_x: INV_X,
        own_labels: true,
        own_background: true,
    });
}

fn frame(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, x: f32, y: f32) {
    game.native_fill(batch, [x - 1.0, y - 1.0, 18.0, 18.0], 0xFF37_3737);
    game.native_fill(batch, [x, y, 16.0, 16.0], 0xFF8B_8B8B);
}

pub(super) fn screen_draw(
    game: &Game<'_>,
    block: &str,
    data: &[i32],
    batch: &mut horizon_client::hud::HudBatch,
    origin: [f32; 2],
) {
    if block != BLOCK {
        return;
    }
    let [x, y] = origin;
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    ui::panel(game, batch, origin, WIDTH, HEIGHT);
    for [sx, sy] in SLOT_POS {
        frame(game, batch, x + sx, y + sy);
    }
    for row in 0..3 {
        for col in 0..9 {
            frame(game, batch, x + INV_X + 18.0 * col as f32, y + INV_Y + 18.0 * row as f32);
        }
    }
    for col in 0..9 {
        frame(game, batch, x + INV_X + 18.0 * col as f32, y + INV_Y + 58.0);
    }
    for (id, bx, by, bw, bh, label, _) in BUTTONS {
        button_state(game, batch, [x + bx, y + by, bw, bh], label, id != 3 || get(2) > 0);
    }
    let nbt = game
        .native_screen_pos()
        .and_then(|at| match game.world.block_entities.get(&(at[0], at[1], at[2])) {
            Some(BlockEntity::Mod(e)) => Some(e.nbt.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let title = game.native_screen_title().unwrap_or_else(|| "Schematic Table".into());
    game.native_text(batch, &title, [x + 8.0, y + 6.0], 0x40_40_40, false);
    let file = string(&nbt, K_FILE);
    let shown: String = if file.chars().count() > 12 {
        file.chars().take(11).collect::<String>() + "~"
    } else {
        file
    };
    if get(0) > 0 {
        game.native_text(batch, &format!("{}/{}", get(1) + 1, get(0)), [x + 62.0, y + 26.0], 0x40_40_40, false);
        game.native_text(batch, &shown, [x + 54.0, y + 41.0], 0x20_20_20, false);
    } else {
        game.native_text(batch, "No .nbt files", [x + 54.0, y + 41.0], 0x9F_20_20, false);
        game.native_text(batch, "in the schematics folder", [x + 30.0, y + 26.0], 0x40_40_40, false);
    }
    let error = string(&nbt, K_ERROR);
    if !error.is_empty() {
        game.native_text(batch, &error, [x + 8.0, y + 76.0], 0x9F_20_20, false);
    }
    game.native_text(batch, "Inventory", [x + INV_X, y + INV_Y - 11.0], 0x40_40_40, false);
}

/// An inactive Upload button is drawn dimmed (the original disables it).
fn button_state(game: &Game<'_>, batch: &mut horizon_client::hud::HudBatch, rect: [f32; 4], label: &str, enabled: bool) {
    ui::button(game, batch, rect, label, false);
    if !enabled {
        game.native_fill(batch, [rect[0] + 1.0, rect[1] + 1.0, rect[2] - 2.0, rect[3] - 2.0], 0x80_C6_C6_C6);
    }
}

fn hit(at: [f32; 2]) -> Option<usize> {
    BUTTONS
        .iter()
        .position(|(_, x, y, w, h, _, _)| at[0] >= *x && at[0] < x + w && at[1] >= *y && at[1] < y + h)
}

pub(super) fn screen_click(block: &str, at: [f32; 2], button: &mut Option<i32>) {
    if block == BLOCK && button.is_none() && let Some(i) = hit(at) {
        *button = Some(BUTTONS[i].0);
    }
}

pub(super) fn screen_tooltip(block: &str, at: [f32; 2], lines: &mut Vec<String>) {
    if block == BLOCK && let Some(i) = hit(at) {
        lines.push(BUTTONS[i].6.into());
    }
}
