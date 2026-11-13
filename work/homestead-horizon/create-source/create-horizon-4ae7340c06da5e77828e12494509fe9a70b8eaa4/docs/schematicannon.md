# Schematicannon and schematic tooling (pure logic)

Pure, deterministic Rust ports of the behaviour in Create 6.0.10
(`Creators-of-Create/Create`, revision `ac0c444d9828da3453ae8cc65338e8de063286fb`,
`content/schematics/...`, MIT). Behaviour is ported, not pasted. World, inventory,
files, networking, rendering and sound go through small traits or are left to
adapters. The existing bounded reader/paste crate (`mixins/create/schematics`) was
only read and is not modified or depended on.

Source files (all in `mixins/create/engine/src/core/`):

| File | Contents |
| --- | --- |
| `schematicannon.rs` | cannon state machine, inventory, options, placement rules, flight time |
| `schematic_printer.rs` | print-order cursor (`SchematicPrinter` scan, deferred queue, entities) |
| `material_checklist.rs` | `MaterialChecklist` and the `ItemRequirement` vocabulary, page layout |
| `schematic_tool.rs` | schematic item components, transformation/anchor math, tool list and sync debounce, Schematic and Quill selection, export file naming |
| `schematic_table.rs` | table state, slot filter, upload rules, upload sessions, client size/chunk rules |

## Ported

- Cannon: `State` (STOPPED/PAUSED/RUNNING), all status keys including `running`,
  `paused`, `stopped` set by the button packet; 5-slot inventory and its filters;
  `tick` order (inventory rescan every 100 ticks while not stopped, flying items,
  paper printer, fuel refill, up to 1000 skip iterations of `tickPrinter`, progress);
  `initializePrinter` (invalid / not placed / errored / expired / out of range 256);
  fuel (`shots_per_gunpowder` default 400, internal slot first, then neighbouring
  inventory, cap to the config, 0 with a creative crate, `noGunpowder` -> `ready`);
  shot cooldown (`schematicannonDelay`, default 10); missing item pause vs
  `skipMissing`; searching/skipping; target-not-loaded pause; creative crate; damage
  requirements via the item source; `finishedPrinting` (empty schematic added to
  slot 1, water updates in replace-empty mode); replace modes 0..3 and
  `replaceBlockEntities`; `shouldIgnoreBlockState`; 2 block minimum distance;
  `ConfigureSchematicannonPacket` options; menu quick-move targets (slots 0, 2, 4);
  flight time `max(10, (int)(distSqr^(1/4) * 4))` and the +1 tick landing delay.
- Paper printer: one page per `progress += 0.05f` (f32: 20 additions give 1.0000001, and the page is
  written on the following tick, so 21 ticks per page; see test), checklist refreshed once, clipboard
  vs written book output, stack count in slot 3, output-full and no-paper reset.
- Print cursor: X-fastest, then Z, then Y scan with the original off-by-one edge
  columns and the absolute-`y` end check, FIFO deferred queue, entity stage.
- Checklist: required/damage/gathered maps, `collect` filter, damage -> whole items
  (`ceil`, Java float/int cast semantics), sort by lowercased display name, 5 (book)
  / 7 (clipboard) entries per page, unfinished first then completed with one shared
  page counter and the two continuation kinds, `x<amount>` and ` | n▤ +r` text.
- Schematic item: `create` defaults, `.nbt` load rule, sneak+main-hand edit-screen
  rule, `SchematicSyncPacket` application (no clamping, item-type check helper).
- `SchematicTransformation`: init from anchor/rotation/mirror/size, `rotate90`
  (clockwise adds -90), `flip` axis mapping, `getAnchor`/`fromAnchor`, half-block
  rotation offset, mirror modifier axis quirk, `toSettings` normalisation (both
  mirrors = +180), 0.45 position chase, move/startAt.
- Handler: `ToolType` list order (MOVE, MOVE_Y, DEPLOY, ROTATE, FLIP, +PRINT in
  creative), undeployed -> DEPLOY only, re-init when the file changes, spectator and
  item-lost deactivation, `markDirty`/sync debounce (fires on the 9th tick), right
  click guards, `printInstantly`.
- Schematic and Quill: right-click sequence (first/second corner, noTarget, prompt,
  sneak discard), target-from-hit rule, range 1..100 (default 10), face selection,
  Ctrl-wheel resize math, selection box, export bounds (inclusive, no emptiness or
  size check), file naming (`slug`, `_N` numbering from the lowest free index,
  `.nbt` suffix, overwrite path for instant saves).
- Table/upload: config defaults and clamps, slot filter, `startUpload`/`finishUpload`
  state, `canWrite`, path-escape check, `.nbt` check, size limit (`kb * 1000`,
  strictly greater fails), same-name delete + oldest-file replacement at the cap
  (directories counted, never deleted), write accounting (bytes counted before
  validation), idle timeout with post-increment compare, client gzip magic check,
  packet chunking, natural list ordering.

## Not ported / gaps

This list is about the pure cores. The first native slice (next section, `Native Horizon integration`)
wires part of it; what that slice still lacks is listed there.

- Reading schematic files, `SchematicLevel`, `StructureTemplate.placeInWorld`,
  block entity transform, `SchematicProcessor`, `ItemRequirement.of(state, be)`
  tables, entity requirements, brittle-block predicate (`BlockMovementChecks.isBrittle`),
  gantry carriage / mechanical arm deferral: adapter side (`PrinterBackend`). The
  existing `create-schematics` crate is the natural source for loading and
  preflight; it was not wired in.
- `LaunchedItem.place` (`BlockHelper.placeSchematicBlock`, belt creation, shaft
  stripping in `stripBeltIfNotLast`, entity spawn) and block entity data preparation.
- NBT/data-component serialisation of the cannon (`Progress`, `PaperProgress`,
  `RemainingFuel`, `Status`, `State`, `AmountPlaced`, `AmountToPlace`, `MissingItem`,
  `Options`, `Printer`, `FlyingBlocks`, `DefaultYaw`), menu/screen, renderer, visual,
  sounds, particles, written book/clipboard component construction (only the page
  layout is produced), language text, the tool classes' selection/drag handling,
  outlines, `LerpedFloat` rotation and mirror animations, `ToolSelectionScreen`.
- `SchematicPlacePacket` creative placement, `SchematicInstances` hash cache,
  `SchematicTableScreen` upload button flow (only the table state), `InstantSchematicPacket`
  beyond the delete-oldest rule.
- Water tick scheduling details of `sendBlockUpdates`.

## Known uncertainties (verify against a Java run or the full source)

The source was read through summarising fetches, not as local files; a few points
are inferred and marked here:

- `LerpedFloat.chase(..).startWithValue(v)`: assumed to leave the chase target at
  `v` (needed for the settings round trip).
- `VecHelper.rotate` / `Mth.sin` tables: recalled from memory (float table indexed
  by `(int)(x * 10430.378f)`); affects only floor edge cases of the anchor.
- `Lang.asId` assumed to lower-case; `slug` therefore lower-cases.
- Ctrl-wheel resize signs (positive wheel shrinks from outside, grows from inside)
  follow the fetched description of the source; not run against the game.
- Quill range scrolling uses `delta as i32` truncation; the Java rounding was not shown.
- Belts launched as `LaunchAction::Belt` count as one placed block (the belt
  launch increment was not in the fetched part of the source).
- Slot limit of slot 3 is the default 64; written books stack to 16 in vanilla,
  which the original handles by `setStackInSlot` overriding limits (not modelled).
- Accepted items of slot 2 (clipboard, book, written book) come from a summary.
- The leading "blocks not loaded" notice only exposes a flag; whether it counts
  towards the clipboard's 7 entries is not verified.
- Anchor drift: `getAnchor` and `fromAnchor` floor at different points, as in the
  original, so a rotated anchor can differ by one block after a round trip.

## Java -> Rust mapping

| Java (`content/schematics/...`) | Rust |
| --- | --- |
| `cannon/SchematicannonBlockEntity` (`tick`, `tickPrinter`, `initializePrinter`, `finishedPrinting`, `refillFuelIfPossible`, `grabItemsFromAttachedInventories`, `updateChecklist`, `tickPaperPrinter`, `shouldPlace`, `shouldIgnoreBlockState`, `launchBlock*`) | `Schematicannon`, `PlaceRules::should_place`, `should_ignore_block_state` |
| `cannon/SchematicannonInventory`, `SchematicannonMenu.quickMoveStack` | `CannonInventory` |
| `cannon/ConfigureSchematicannonPacket` | `ConfigOption`, `Schematicannon::apply_option` |
| `cannon/LaunchedItem` | `FlyingItem`, `ticks_for_distance`, `Schematicannon::tick_flying`, `LaunchAction` |
| `cannon/MaterialChecklist`, `requirement/ItemRequirement` | `MaterialChecklist`, `ItemRequirement`, `StackRequirement`, `UseKind` |
| `SchematicPrinter` | `PrintCursor` (order), `PrinterBackend` (loading/queries) |
| `SchematicItem`, `packet/SchematicSyncPacket` | `SchematicStackData`, `SyncPacket`, `sync_accepts` |
| `client/SchematicTransformation` | `Transformation` |
| `client/SchematicHandler`, `tools/ToolType` | `SchematicHandler`, `ToolType` |
| `client/SchematicAndQuillHandler` | `QuillSelection` |
| `SchematicExport`, `FilesHelper.findFirstValidFilename/slug` | `export_bounds`, `export_file_name`, `find_first_valid_filename`, `slug` |
| `table/SchematicTableBlockEntity`, `SchematicTableMenu` | `SchematicTable`, `table_may_place` |
| `ServerSchematicLoader` | `plan_new_upload`, `UploadManager`, `instant_upload_delete`, `path_is_safe` |
| `client/ClientSchematicLoader` | `client_validate_size`, `is_gzip`, `packet_sizes`, `natural_cmp` |
| `infrastructure/config/CSchematics` | `UploadConfig`, `CannonConfig` |

## Integration

1. `lib.rs` (do not edit from this subsystem; add when merging):

   ```rust
   #[path = "core/material_checklist.rs"]
   pub mod material_checklist;
   #[path = "core/schematic_printer.rs"]
   pub mod schematic_printer;
   #[path = "core/schematic_tool.rs"]
   pub mod schematic_tool;
   #[path = "core/schematic_table.rs"]
   pub mod schematic_table;
   #[path = "core/schematicannon.rs"]
   pub mod schematicannon;
   ```

   Modules reference each other through `crate::material_checklist`,
   `crate::schematic_tool`, `crate::schematic_printer`. Add the same names to
   `horizon-mixin.toml` wherever the vault module is listed.
2. Adapter hooks:
   - `PrinterBackend`: wraps the loaded schematic (use the `create-schematics`
     crate for reading and preflight, place into a scratch level with
     rotation/mirror from `SchematicStackData`, build `PrintCursor` from the
     encapsulated bounds and the entity block positions, provide `ItemRequirement`
     per state/block entity, `PlaceQuery` per cell including the bed/door
     second block and the printer vetoes: unloaded chunk, world border, equal state,
     unbreakable).
   - `ItemSource`: the six neighbour item handlers (creative crate detection,
     extract, damage, gunpowder, scan for the checklist).
   - `TargetWorld::is_loaded` for chunk checks.
   - Per server tick call `Schematicannon::tick`, then place each returned
     `FlyingItem` (`BlockHelper.placeSchematicBlock`, belt creation, entity spawn);
     drain `events` for sounds and printed checklists (build the book/clipboard from
     `MaterialChecklist::book_layout/clipboard_layout`); the client calls `tick`
     with `server: false` for the flight bookkeeping.
   - Client: drive `SchematicHandler::tick` with the held stack, send the returned
     `SyncPacket`; `QuillSelection` from mouse/scroll/tick input.
3. Block entities: Schematicannon (inventory with the 5 filters, options,
   persisted fields listed above, neighbour scan, `sendUpdate` -> block update),
   Schematic Table (2 slot inventory, upload progress sync).
4. Items / data components: `create:schematic`, `create:empty_schematic`,
   `create:schematic_and_quill`, `create:clipboard`, components `SCHEMATIC_FILE`,
   `SCHEMATIC_OWNER`, `SCHEMATIC_ANCHOR`, `SCHEMATIC_DEPLOYED`, `SCHEMATIC_ROTATION`,
   `SCHEMATIC_MIRROR`, `SCHEMATIC_BOUNDS` (map to `SchematicStackData`).
5. Packets: `ConfigureSchematicannonPacket` (option + set flag, only valid while the
   player's open menu is a schematicannon menu), `SchematicSyncPacket`,
   `SchematicUploadPacket` (begin/write/finish), `InstantSchematicPacket`,
   `SchematicPlacePacket` (creative only).
6. Content: schematicannon / schematic table blocks and menus (slot positions in the
   Java menus), gunpowder as fuel, tool icons, language keys
   `schematicannon.status.*`, `schematic.tool.*`, `schematicAndQuill.*`, config
   (`schematicannonDelay`, `schematicannonShotsPerGunpowder`, `maxSchematics`,
   `maxTotalSchematicSize`, `maxSchematicPacketSize`, `schematicIdleTimeout`,
   `creativePrintIncludesAir`), a player schematic folder and an uploaded schematic
   folder per player.

## Tests

`cargo test --offline` in a scratch copy of the engine crate with the five module
lines added (the main folder was never built). 112 tests in these modules, whole crate
383 passed, 0 failed, no warnings. Numeric tests are algorithm oracles derived from
the Java code, not in-game comparisons.

## Native Horizon integration (first vertical slice)

Files in `mixins/create/`: `schematic_data.rs` (items, components, folder, `create cannon` commands),
`cannon_runtime.rs` (Schematicannon block entity, printer/inventory adapters, screen),
`schematic_table_runtime.rs` (Schematic Table), `cannon_*.rs` (hook helpers), injections in
`fragments/schematic.toml`, content in `tools/schematic_content.py`, acceptance in
`stage4/acceptance/Test-HomesteadCannon.ps1`. The `create_schematics` crate (the same preflight and
placement rules as `create schematic paste`) is the schematic loader.

What works (authoring-time statement; nothing here was built or run when it was written, see the acceptance
script for what is verified in a game):

- Blocks `create:schematicannon` and `create:schematic_table` (barrel template, so their menus open), the
  items `create:empty_schematic`, `create:schematic` and `create:clipboard` (the Material Checklist).
- Cannon: 5 slots, menu with status, progress, fuel, buttons (Play, Pause, Stop, the four replace modes, skip
  missing, replace block entities), shift-click routing from the inventory (`CannonInventory::quick_move_target`).
  The core state machine runs unchanged: gunpowder fuel (400 shots, internal slot first, then a neighbouring
  container), 10 tick shot delay, flight time, `PlaceRules`, missing item pause / skip, finish returns an Empty
  Schematic, the paper printer writes the checklist.
- Items come from the six neighbouring containers (vanilla chests, barrels, hoppers, other mod containers that
  expose slots on that face), read as a snapshot each tick and taken with `native_extract_item`.
- Schematic files: `.nbt` structure files in the instance's `schematics` folder, loaded with
  `create_schematics::read_gzip` + `preflight` (rotation and mirror from the Schematic item). Any block the paste
  adapter rejects makes the whole schematic "errored" and the cannon discards it, as for `create schematic paste`.
  Rotation and mirror pivot about the zero point (as `SchematicItem.getSettings`), so a turned schematic extends to
  negative offsets from the anchor (one clockwise quarter turn maps (x, z) to (-z, x)); the print cursor covers the
  transformed bounding box (`transformed_bounds`), not `[0, size)`.
- Stop resets the printer (counters and cursor to zero, as `resetPrinter`); the Schematic stays in its slot, so the
  cannon initialises again on the next tick and shows paused / "Ready", as in Create. Finishing a print also zeroes
  the placed / to-place counters.
- Landing: after the flight the block is placed through the paste adapter's `destination` rules (same block
  entity policy as `create schematic paste`); doors, tall plants and beds are placed together with their second half.
- The cannon's state (state, status, fuel, counters, options, print cursor) is saved in its block entity; a cannon
  loaded from disk is paused ("ready") and replays its cursor, as Create's does after a reload.
- Table: lists the folder's `.nbt` files, previous / next / upload buttons; Upload replaces the Empty Schematic
  (whole stack, `SchematicTable::start_upload`) by a Schematic for the selected file at once.
- Operator commands (permission 2) standing in for the Schematic Tool: `create cannon give FILE`,
  `create cannon deploy X Y Z [turns [mirror]]` (held Schematic), `create cannon load CX CY CZ FILE AX AY AZ
  [turns [mirror]]`, `create cannon control X Y Z play|pause|stop|replace_*|skip_missing|replace_entities` (a table:
  `prev|next|upload`), `create cannon status X Y Z`, `create cannon list`, `create cannon checklist` (held clipboard).

Deliberate differences and gaps of this slice:

- No Schematic Tool: no in-world ghost, no Move / Rotate / Flip tools, no tool selection screen, no sync packet. A
  Schematic is placed by `create cannon deploy` / `load`. Schematic and Quill (selection and export) is not ported
  although its core exists; there is no item for it.
- No file chooser or upload packets: the table reads the instance folder (no per-player folders, no size limits, no
  progress bar, instant). The owner of a table-made Schematic is a fixed string.
- Screens are drawn with flat colours (no `schematicannon.png` / table texture, no original layout); the status
  texts are English lang strings recalled from Create (unverified). Buttons have tooltips, no icons.
- No projectile entity or renderer, no cannon barrel animation, particles or Create sounds: vanilla sounds stand in
  (`block.dispenser.launch`, `block.note_block.chime`).
- No creative crate detection (`has_creative_crate` is always false), no Gantry/Arm deferral, no entities in
  schematics (`entities` are ignored), no belts (a `create:belt` cell has an invalid requirement and is skipped),
  no damage-use requirements, no `sendBlockUpdates` water shell in replace-empty mode.
- Requirements come from the block state's item (`StateFacts.item`) with slab, snow, candle, pickle and egg counts;
  fluids and fire are invalid (skipped). Create's `ItemRequirement` special cases (crops to seeds, farmland to dirt,
  cake, ...) are not tabled.
- Projectiles in flight are not saved: unloading the chunk while a shot is in the air loses that block (it stays
  counted as placed).
- The Material Checklist is a `create:clipboard` carrying a `create:checklist` component (read it with
  `create cannon checklist`); a Book or Written Book in the paper slot also produces a clipboard, since a
  written-book component is not built here. The clipboard has no screen. It stacks to 64 (Create: 1) because the
  core's output slot assumes it.
- No hopper or other automation access to the cannon's slots (no container faces).
- The recipes, hardness, resistance and shapes are placeholders (marked UNVERIFIED in `schematic_content.py`).