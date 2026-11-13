// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Display Board (FlapDisplayBlock / FlapDisplayBlockEntity): the multi-block
// text board a Display Link writes to. The rules live in
// `create_engine::display_board` (sections, layouts, size scan, colours); this
// file adapts them to the game.
//
// Blocks: `create:display_board` with `facing` (horizontal, the face that shows
// text), `up` and `down` (set when a board of the same facing is stacked above or
// below). The controller is the top-left block of a board as seen from the front
// (`should_be_controller`); only it carries data and draws.
//
// Differences from Create that are deliberate and listed in
// docs/redstone-displays.md: the board is not a kinetic block (it needs no
// rotation, `speed_requirement_fulfilled` is always true), there is no flap spin
// animation and no clacking sound (text changes instantly), and the manual
// interactions (dye, glow ink, name tag per line) are not ported.
//
// Block entity NBT (controller only has the data fields):
//   Controller (Byte)  1 for the controller, 0 otherwise
//   XSize, YSize (Int) board size in blocks
//   Lines (List)       per line: Key (String), Sections (List of Size Float, Cycle,
//                      Single, Gap, Right, Wide Byte, Text String)
//   Rows (List)        plain text of every line, as the flaps show it (for tools)
//   Colour, Glowing, Manual (IntArray) per line
//
// Reservation of lines by Display Links lives in a session cache, like the
// Redstone Link network: it is not saved.

use super::{Game, GameLevel, create_port_contraption as contraption, native_block_entity_draw::BlockEntityDraw};
use crate::containers::{BlockEntity, ModBlockEntity};
use create_engine::{
    display_board::{self as board, FlapDisplay, FlapLayout, FlapSection},
    display_link::{self as dlink, Reservations},
    transport::SessionCache,
};
use horizon_client::text::Text;
use horizon_core::geometry::Direction as Face;
use horizon_world::{
    level::PlacementContext,
    nbt::{Compound, Tag},
    registry::BlockStateId,
};
use std::{cell::RefCell, collections::BTreeMap};

pub(super) const BOARD: &str = "create:display_board";
type Pos = [i32; 3];

const CONTROLLER: &str = "Controller";
const X_SIZE: &str = "XSize";
const Y_SIZE: &str = "YSize";
const LINES: &str = "Lines";
const ROWS: &str = "Rows";
const COLOUR: &str = "Colour";
const GLOWING: &str = "Glowing";
const MANUAL: &str = "Manual";

/// Depth of the board box in blocks (estimate; the shape is not diffed).
const THICKNESS: f32 = 0.5;
/// Font pixels are scaled so one flap (3.5 board pixels) is six font pixels wide.
const FONT_SCALE: f32 = 3.5 / 16.0 / 6.0;
/// Height of one line in blocks (two lines per block).
const LINE_HEIGHT: f32 = 0.5;
/// Pixel margin around the lines, in board pixels (`getMaxCharCount`'s "- 2").
const MARGIN_PX: f32 = 1.0;
/// Gap after a section with `has_gap`, in board pixels (`getMaxCharCount`'s "4 * gaps").
const GAP_PX: f32 = 4.0;

thread_local! {
    static RESERVED: RefCell<SessionCache<BTreeMap<Pos, Reservations>>> = RefCell::new(Default::default());
}

fn simulates(game: &Game<'_>) -> bool {
    game.serving.is_some() || game.options.dedicated.is_some()
}

// ------------------------------------------------------------------- NBT

fn int(nbt: &Compound, key: &str) -> i32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => *v,
        Some(Tag::Byte(v)) => i32::from(*v),
        _ => 0,
    }
}

fn flag(nbt: &Compound, key: &str) -> bool {
    matches!(nbt.get(&key.into()), Some(Tag::Byte(v)) if *v != 0)
}

fn string(nbt: &Compound, key: &str) -> String {
    match nbt.get(&key.into()) {
        Some(Tag::String(v)) => v.to_string().ok().unwrap_or_default(),
        _ => String::new(),
    }
}

fn float(nbt: &Compound, key: &str) -> f32 {
    match nbt.get(&key.into()) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        Some(Tag::Int(v)) => *v as f32,
        _ => 0.0,
    }
}

fn int_array(nbt: &Compound, key: &str) -> Vec<i32> {
    match nbt.get(&key.into()) {
        Some(Tag::IntArray(v)) => v.clone(),
        _ => Vec::new(),
    }
}

pub(super) fn default_nbt() -> Compound {
    Compound::from([
        (CONTROLLER.into(), Tag::Byte(0)),
        (X_SIZE.into(), Tag::Int(1)),
        (Y_SIZE.into(), Tag::Int(1)),
    ])
}

/// What clients receive: the whole board data (the controller draws it).
pub(super) fn update_tag(entity: &ModBlockEntity) -> Compound {
    [CONTROLLER, X_SIZE, Y_SIZE, LINES, ROWS, COLOUR, GLOWING]
        .into_iter()
        .filter_map(|key| entity.nbt.get(&key.into()).map(|tag| (key.into(), tag.clone())))
        .collect()
}

// ---------------------------------------------------- FlapDisplay <-> NBT

fn rebuild_section(size: f32, cycle: &str, single: bool, gap: bool, right: bool, wide: bool, text: &str) -> FlapSection {
    let mut section = FlapSection::new(size, cycle, single, gap);
    section.right_aligned = right;
    section.wide_flaps = wide;
    if !text.is_empty() {
        section.set_text(text);
    }
    section
}

/// Builds the pure board model from block entity data.
pub(super) fn load(nbt: &Compound) -> FlapDisplay {
    let mut display = FlapDisplay::new();
    display.set_size(int(nbt, X_SIZE).max(1), int(nbt, Y_SIZE).max(1));
    let count = display.line_count();
    if let Some(Tag::List(lines)) = nbt.get(&LINES.into()) {
        for (index, line) in lines.iter().enumerate().take(count) {
            let Tag::Compound(line) = line else { continue };
            let Some(Tag::List(sections)) = line.get(&"Sections".into()) else { continue };
            let sections: Vec<FlapSection> = sections
                .iter()
                .filter_map(|section| match section {
                    Tag::Compound(c) => Some(rebuild_section(
                        float(c, "Size"),
                        &string(c, "Cycle"),
                        flag(c, "Single"),
                        flag(c, "Gap"),
                        flag(c, "Right"),
                        flag(c, "Wide"),
                        &string(c, "Text"),
                    )),
                    _ => None,
                })
                .collect();
            if !sections.is_empty() {
                display.lines()[index].configure(&string(line, "Key"), sections);
            }
        }
    }
    for (index, dye) in int_array(nbt, COLOUR).into_iter().enumerate().take(count) {
        display.colour[index] = u8::try_from(dye).ok();
    }
    for (index, on) in int_array(nbt, GLOWING).into_iter().enumerate().take(count) {
        display.glowing[index] = on != 0;
    }
    for (index, on) in int_array(nbt, MANUAL).into_iter().enumerate().take(count) {
        display.manual[index] = on != 0;
    }
    display
}

/// The characters one section shows (`FlapDisplaySection.refresh(false)`).
pub(super) fn flaps_of(section: &FlapSection) -> Vec<String> {
    let mut copy = rebuild_section(
        section.size,
        &section.cycle,
        section.single_flap,
        section.has_gap,
        section.right_aligned,
        section.wide_flaps,
        &section.component,
    );
    copy.refresh(false);
    copy.flaps().to_vec()
}

fn row_text(layout: &FlapLayout) -> String {
    layout.sections.iter().flat_map(flaps_of).collect::<String>().trim_end().to_string()
}

fn text_tag(value: &str) -> Tag {
    Tag::String(value.into())
}

/// Writes the pure model back into block entity data.
pub(super) fn save(display: &mut FlapDisplay, nbt: &mut Compound) {
    nbt.insert(X_SIZE.into(), Tag::Int(display.x_size));
    nbt.insert(Y_SIZE.into(), Tag::Int(display.y_size));
    let lines: Vec<Tag> = display
        .lines()
        .iter()
        .map(|layout| {
            let sections = layout
                .sections
                .iter()
                .map(|s| {
                    Tag::Compound(Compound::from([
                        ("Size".into(), Tag::Float(s.size.to_bits())),
                        ("Cycle".into(), text_tag(&s.cycle)),
                        ("Single".into(), Tag::Byte(i8::from(s.single_flap))),
                        ("Gap".into(), Tag::Byte(i8::from(s.has_gap))),
                        ("Right".into(), Tag::Byte(i8::from(s.right_aligned))),
                        ("Wide".into(), Tag::Byte(i8::from(s.wide_flaps))),
                        ("Text".into(), text_tag(&s.component)),
                    ]))
                })
                .collect();
            Tag::Compound(Compound::from([
                ("Key".into(), text_tag(&layout.key)),
                ("Sections".into(), Tag::List(sections)),
            ]))
        })
        .collect();
    let rows: Vec<Tag> = display.lines().iter().map(|layout| text_tag(&row_text(layout))).collect();
    nbt.insert(LINES.into(), Tag::List(lines));
    nbt.insert(ROWS.into(), Tag::List(rows));
    nbt.insert(COLOUR.into(), Tag::IntArray(display.colour.iter().map(|c| c.map_or(-1, i32::from)).collect()));
    nbt.insert(GLOWING.into(), Tag::IntArray(display.glowing.iter().map(|g| i32::from(*g)).collect()));
    nbt.insert(MANUAL.into(), Tag::IntArray(display.manual.iter().map(|m| i32::from(*m)).collect()));
}

// ------------------------------------------------------------- geometry

fn horizontal(name: &str) -> [i32; 3] {
    match name {
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => [0, 0, -1],
    }
}

/// The direction along a row, away from the controller: the viewer's right
/// when looking at the front, `facing.getCounterClockWise()`.
fn row_direction(facing: &str) -> [i32; 3] {
    horizontal(match facing {
        "north" => "west",
        "west" => "south",
        "south" => "east",
        _ => "north",
    })
}

fn shifted(pos: Pos, by: [i32; 3], times: i32) -> Pos {
    [pos[0] + by[0] * times, pos[1] + by[1] * times, pos[2] + by[2] * times]
}

fn is_board(game: &Game<'_>, pos: Pos) -> bool {
    game.native_block(pos) == BOARD
}

fn prop_true(game: &Game<'_>, pos: Pos, key: &str) -> bool {
    game.native_block_property(pos, key).as_deref() == Some("true")
}

fn same_state(game: &Game<'_>, a: Pos, b: Pos) -> bool {
    game.world.block(a[0], a[1], a[2]) == game.world.block(b[0], b[1], b[2])
}

/// The controller of the board containing `start`: climb while `up`, then go
/// left while the left neighbour has the same state.
pub(super) fn controller_of(game: &Game<'_>, start: Pos) -> Option<Pos> {
    if !is_board(game, start) {
        return None;
    }
    let mut at = start;
    for _ in 0..(2 * board::MAX_BOARD_WIDTH + 64) {
        let facing = game.native_block_property(at, "facing")?;
        if prop_true(game, at, "up") {
            let above = [at[0], at[1] + 1, at[2]];
            if !is_board(game, above) {
                return Some(at);
            }
            at = above;
            continue;
        }
        let left = shifted(at, row_direction(&facing), -1);
        if is_board(game, left) && same_state(game, at, left) {
            at = left;
            continue;
        }
        return Some(at);
    }
    Some(at)
}

// ------------------------------------------------------------------ tick

/// `FlapDisplayBlockEntity.tick` lazy part: every `LAZY_TICK_RATE` ticks the
/// block works out whether it is the controller and how big the board is.
pub(super) fn tick(game: &mut Game<'_>, pos: Pos, entity: &mut ModBlockEntity) {
    if !simulates(game) || !game.native_on("create") || !contraption::resident(game, pos) {
        return;
    }
    let known = entity.nbt.contains_key(&CONTROLLER.into());
    let phase = i64::from(pos[0] + pos[2]).rem_euclid(i64::from(board::LAZY_TICK_RATE));
    if known && game.world.game_time.rem_euclid(i64::from(board::LAZY_TICK_RATE)) != phase {
        return;
    }
    let Some(facing) = game.native_block_property(pos, "facing") else { return };
    let row = row_direction(&facing);
    let up = prop_true(game, pos, "up");
    let left = shifted(pos, row, -1);
    let left_same = is_board(game, left) && same_state(game, pos, left);
    let controller = board::should_be_controller(up, left_same);
    let was = flag(&entity.nbt, CONTROLLER);

    if !controller {
        if was || !known {
            for key in [LINES, ROWS, COLOUR, GLOWING, MANUAL] {
                entity.nbt.remove(&key.into());
            }
            entity.nbt.insert(CONTROLLER.into(), Tag::Byte(0));
            game.native_block_entity_sync(pos);
        }
        return;
    }

    let (x, y) = board::controller_size(
        |i| {
            let at = shifted(pos, row, i);
            is_board(game, at) && same_state(game, pos, at)
        },
        |i| prop_true(game, [pos[0], pos[1] - i, pos[2]], "down"),
    );
    let mut display = load(&entity.nbt);
    let resized = display.set_size(x, y);
    if resized || !was || !known {
        save(&mut display, &mut entity.nbt);
        entity.nbt.insert(CONTROLLER.into(), Tag::Byte(1));
        game.native_block_entity_sync(pos);
    }
}

// ---------------------------------------------------------------- targets

/// `DisplayBoardTarget` stats at `target` (any block of a board):
/// `(rows, columns)`, `(1, 1)` without a controller.
pub(super) fn stats(game: &Game<'_>, target: Pos) -> Option<(Pos, board::FlapDisplay)> {
    let controller = controller_of(game, target)?;
    match game.world.block_entities.get(&(controller[0], controller[1], controller[2])) {
        Some(BlockEntity::Mod(entity)) if entity.id == BOARD && flag(&entity.nbt, CONTROLLER) => {
            Some((controller, load(&entity.nbt)))
        }
        _ => None,
    }
}

/// Writes one text line set into the board containing `target` through the
/// core's `board_accept_flap_text`. `load_layout` is the source's layout rule.
/// Returns false when there is no ready controller.
#[allow(clippy::too_many_arguments)]
pub(super) fn accept(
    game: &mut Game<'_>,
    target: Pos,
    line: i32,
    cells: &[Vec<String>],
    single_line_source: bool,
    link: Pos,
    load_layout: &mut dyn FnMut(&mut FlapLayout, usize, i32),
) -> bool {
    let Some((controller, mut display)) = stats(game, target) else { return false };
    let session = std::ptr::from_ref(&game.world) as usize;
    let dimension = game.dim.id().to_owned();
    let mut reservations = RESERVED
        .with_borrow_mut(|cache| cache.dimension(session, &dimension).remove(&controller))
        .unwrap_or_default();
    {
        let level: &Game<'_> = &*game;
        let exists = |p: Pos| level.native_block(p) == super::create_port_display_link::BLOCK;
        dlink::board_accept_flap_text(
            &mut display,
            line,
            cells,
            single_line_source,
            true,
            link,
            &mut reservations,
            exists,
            load_layout,
        );
    }
    RESERVED.with_borrow_mut(|cache| {
        cache.dimension(session, &dimension).insert(controller, reservations);
    });
    let written = game.native_with_mod_block_entity(controller, |entity| {
        let before = entity.nbt.clone();
        save(&mut display, &mut entity.nbt);
        before != entity.nbt
    });
    if written == Some(true) {
        game.native_block_entity_sync(controller);
    }
    written.is_some()
}

// ------------------------------------------------------------- placement

fn facing_of(game: &GameLevel<'_>, id: BlockStateId) -> Option<String> {
    game.registry
        .state(id)
        .filter(|block| block.name == BOARD)
        .and_then(|block| block.properties.get("facing").cloned())
}

fn connected(game: &GameLevel<'_>, pos: Pos, facing: &str) -> bool {
    facing_of(game, game.world.block(pos[0], pos[1], pos[2])).is_some_and(|other| other == facing)
}

fn truth(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// `state_for_placement`: faces the player and joins boards above and below.
pub(super) fn placement(game: &GameLevel<'_>, state: BlockStateId, ctx: &PlacementContext) -> Option<BlockStateId> {
    let facing = super::create_port_placement::horizontal(ctx.yaw).opposite().name();
    let at = [ctx.pos.x, ctx.pos.y, ctx.pos.z];
    let state = game.native_state_with_property(state, "facing", facing)?;
    let state = game.native_state_with_property(state, "up", truth(connected(game, [at[0], at[1] + 1, at[2]], facing)))?;
    game.native_state_with_property(state, "down", truth(connected(game, [at[0], at[1] - 1, at[2]], facing)))
}

/// `update_shape`: `up` / `down` follow a board of the same facing above / below.
pub(super) fn shape(
    game: &GameLevel<'_>,
    state: BlockStateId,
    changed: Face,
    neighbour: BlockStateId,
    new: &mut Option<BlockStateId>,
) {
    let Some(block) = game.registry.state(state).filter(|block| block.name == BOARD) else {
        return;
    };
    let key = match changed {
        Face::Up => "up",
        Face::Down => "down",
        _ => return,
    };
    let Some(facing) = block.properties.get("facing") else { return };
    let want = facing_of(game, neighbour).is_some_and(|other| &other == facing);
    let have = block.properties.get(key).is_some_and(|v| v == "true");
    if want != have {
        *new = game.native_state_with_property(state, key, truth(want));
    }
}

// ------------------------------------------------------------------ draw

/// `DyeColor.getTextColor` in vanilla order (white .. black).
fn dye_rgb(dye: u8) -> u32 {
    const TABLE: [u32; 16] = [
        0xF9FFFE, 0xF9801D, 0xC74EBD, 0x3AB3DA, 0xFED83D, 0x80C71F, 0xF38BAA, 0x474F52, 0x9D9D97, 0x169C9C, 0x8932B8,
        0x3C44AA, 0x835432, 0x5E7C16, 0xB02E26, 0x1D1D21,
    ];
    TABLE[usize::from(dye).min(15)]
}

/// Draws the whole board from its controller. Text is drawn per flap on a fixed
/// pitch (a monospace grid); the font is proportional, so glyphs are left aligned
/// in their cell. No spin animation.
pub(super) fn draw(game: &Game<'_>, pos: Pos, entity: &ModBlockEntity, draw: &mut BlockEntityDraw) {
    if !flag(&entity.nbt, CONTROLLER) {
        return;
    }
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    let turn = match facing.as_str() {
        "south" => 0.0,
        "east" => 90.0,
        "west" => 270.0,
        _ => 180.0,
    };
    let mut display = load(&entity.nbt);
    let layouts = display.lines().clone();
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], turn, [0.5, 0.0, 0.5]);
    for (index, layout) in layouts.iter().enumerate() {
        let colour = display.line_color(index, dye_rgb);
        let glowing = display.glowing.get(index).copied().unwrap_or(false);
        draw.push();
        draw.translate([0.0, 1.0 - LINE_HEIGHT * index as f32 - 0.1, THICKNESS + 0.002]);
        draw.scale([FONT_SCALE, -FONT_SCALE, FONT_SCALE]);
        let mut x_px = MARGIN_PX;
        for section in &layout.sections {
            let pitch = if section.wide_flaps { board::WIDE_MONOSPACE } else { board::MONOSPACE } / 2.0;
            for (offset, flap) in flaps_of(section).iter().enumerate() {
                if flap.trim().is_empty() {
                    continue;
                }
                let at = (x_px + pitch * offset as f32) / 16.0 / FONT_SCALE;
                game.native_draw_text(draw, &Text::literal(flap.clone()), [at, 0.0], colour, glowing, 0);
            }
            x_px += pitch * section.flap_count() as f32;
            if section.has_gap {
                x_px += GAP_PX;
            }
        }
        draw.pop();
    }
    draw.pop();
}

/// Render bounds of the controller: the whole board, in block coordinates.
pub(super) fn bounds(entity: &ModBlockEntity, game: &Game<'_>, pos: Pos, out: &mut [f32; 6]) {
    if !flag(&entity.nbt, CONTROLLER) {
        return;
    }
    let facing = game.native_block_property(pos, "facing").unwrap_or_else(|| "north".into());
    let row = row_direction(&facing);
    let (x, y) = (int(&entity.nbt, X_SIZE).max(1) as f32 - 1.0, int(&entity.nbt, Y_SIZE).max(1) as f32 - 1.0);
    let mut low = [0.0f32, -y, 0.0];
    let mut high = [1.0f32, 1.0, 1.0];
    for axis in [0usize, 2] {
        match row[axis].signum() {
            1 => high[axis] += x,
            -1 => low[axis] -= x,
            _ => {}
        }
    }
    *out = [low[0], low[1], low[2], high[0], high[1], high[2]];
}
