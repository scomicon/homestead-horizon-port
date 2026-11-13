# Schematicannon / Schematic Table / Material Checklist: shared-file edits for the lead

Everything new is in new files (paths relative to the create-source checkout):

- `mixins/create/schematic_data.rs`, `cannon_runtime.rs`, `schematic_table_runtime.rs` (modules)
- `mixins/create/cannon_component.rs`, `cannon_register.rs`, `cannon_command.rs`, `cannon_entity.rs`,
  `cannon_ticks.rs`, `cannon_tick.rs`, `cannon_update_tag.rs`, `cannon_destroy.rs`, `cannon_container.rs`,
  `cannon_screen.rs`, `cannon_screen_draw.rs`, `cannon_screen_click.rs`, `cannon_screen_tooltip.rs`,
  `cannon_menu_button.rs`, `cannon_quick_move.rs` (hook helpers)
- `mixins/create/fragments/schematic.toml` (3 modules + 15 injections)
- `tools/schematic_content.py`
- `stage4/acceptance/Test-HomesteadCannon.ps1`
- `docs/schematicannon.md` (new section "Native Horizon integration")

Nothing was built, run or committed. The edits below are all that is needed in shared files.

## 1. `mixins/create/horizon-mixin.toml`

Append the whole of `mixins/create/fragments/schematic.toml` at the end (or after the `schematic-command`
injection). The `create_engine` modules `material_checklist`, `schematic_printer`, `schematic_tool`,
`schematic_table`, `schematicannon` are already in `engine/src/lib.rs`; no change there.

## 2. `mixins/create/schematic_runtime.rs` (visibility only, 4 edits)

The cannon reuses the paste command's resolver, file reader and `destination` placement rules instead of copying
them.

1. Anchor `struct Registry<'a, 'b> {` / `    game: &'a Game<'b>,` (lines 43-44). Replace with

   ```rust
   pub(super) struct Registry<'a, 'b> {
       pub(super) game: &'a Game<'b>,
   ```

2. Anchor `fn read_file(filename: &str) -> Result<Vec<u8>, String> {` (line 502). Replace with
   `pub(super) fn read_file(filename: &str) -> Result<Vec<u8>, String> {`.

3. Anchor `struct Write {` (line 548) with its five fields. Replace the block with

   ```rust
   pub(super) struct Write {
       pub(super) pos: [i32; 3],
       pub(super) state: BlockStateId,
       pub(super) nbt: Option<String>,
       pub(super) old: BlockStateId,
       pub(super) old_entity: Option<BlockEntity>,
   }
   ```

4. Anchor `fn destination(game: &Game<'_>, plan: &Plan, origin: [i32; 3]) -> Result<Vec<Write>, Value> {` (line 609).
   Prefix with `pub(super) `.

(`Registry` implements `Resolver` already; `cannon_runtime.rs` calls
`create_schematics::preflight(&schematic, &paste::Registry { game }, &options)` and
`paste::destination(game, &single_block_plan, position)`.)

## 3. `tools/content.py`

a. Imports, after the `from whistle_content import ...` line (line 33):

```python
from schematic_content import (CANNON as SCHEMATICANNON, TABLE as SCHEMATIC_TABLE, schematic_blocks,
    schematic_items, schematic_recipes, LANG_EN as SCHEMATIC_EN, LANG_RU as SCHEMATIC_RU,
    ITEM_LANG_EN as SCHEMATIC_ITEM_EN, ITEM_LANG_RU as SCHEMATIC_ITEM_RU)
```

b. In `foundation()`, directly after `steam_whistle(write, model)` (line 976), same indentation:

```python
    schematic_blocks(write, model)
    schematic_items(write)
```

(`schematic_items` must run after the generic `for item in ITEMS:` loop so its item declarations win.)

c. Directly after `steam_whistle_recipes(write)` (line 1014):

```python
    schematic_recipes(write)
```

d. In the `minecraft/tags/block/mineable/pickaxe.json` values list (line 1019), append
   `f'create:{SCHEMATICANNON}', f'create:{SCHEMATIC_TABLE}'` after `f'create:{ITEM_VAULT}'`.

e. English names: after `extra_blocks[WHISTLE_EXT] = 'Steam Whistle Extension'` (line 1087) add
   `extra_blocks.update(SCHEMATIC_EN)`; after `extra_items.update({'whisk': 'Whisk', 'super_glue': 'Super Glue'})`
   (line 1118) add `extra_items.update(SCHEMATIC_ITEM_EN)`.

f. Russian names: after `lang['block.create.' + WHISTLE_EXT] = 'Удлинитель парового свистка'` (line 1237) add

```python
            lang.update({'block.create.' + k: v for k, v in SCHEMATIC_RU.items()})
            lang.update({'item.create.' + k: v for k, v in SCHEMATIC_ITEM_RU.items()})
```

(`ITEMS` and `BLOCKS` in catalog.py drive the creative tab: `all_blocks + ITEMS`, so no tab edit.)

## 4. `tools/catalog.py`

a. Import, after `from whistle_content import ...`:

```python
from schematic_content import (CANNON as SCHEMATICANNON, TABLE as SCHEMATIC_TABLE,
    ITEMS as SCHEMATIC_ITEMS, PARTS as SCHEMATIC_PARTS)
```

b. After `BLOCKS.append(WHISTLE)`:

```python
BLOCKS.append(SCHEMATICANNON)
BLOCKS.append(SCHEMATIC_TABLE)
```

c. After `ITEMS.extend([... 'experience_nugget'])` (the block ending with `'crushed_raw_zinc', 'copper_nugget', ...`):

```python
ITEMS.extend(SCHEMATIC_ITEMS)
```

d. After `PARTS += [f'{WHISTLE}/{part}' for part in WHISTLE_PARTS]`:

```python
PARTS += list(SCHEMATIC_PARTS)
```

## 5. `tools/fallback_textures.py`

In `FAMILIES`, next to `'steam_whistle': 'copper_casing',`:

```python
    'schematicannon': 'andesite_casing',
    'schematic_table': 'brass_casing',
```

## 6. Acceptance wiring

`stage4/Run-Acceptance.ps1`:

- line 5, `param(...)`: add `,[switch] $Cannon` after `[switch] $Pot`.
- line 20, `$reportName=` chain: start it with `if ($Cannon) {'run-cannon-result.json'} elseif ($Pot) {...` (i.e.
  replace the leading `if ($Pot) {` by `if ($Cannon) {'run-cannon-result.json'} elseif ($Pot) {`).
- line 49, replace `    if ($Pot) {` by

```powershell
    if ($Cannon) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadCannon.ps1'),'Cannon','stage30-cannon-'))
    } elseif ($Pot) {
```

`stage4/regress.ps1` line 3: append `,@('cannon',@{Cannon=$true})` after `@('pot',@{Pot=$true})` inside `$sets=@(...)`.

## 7. Verification order suggested for the lead

1. Apply 1-5, run the content generator (the new JSON must show up under `data/create/horizon/block/`,
   `.../item/`, `.../menu/schematicannon.json` and `schematic_table.json`).
2. Build the bundle; fix compile errors (the Rust was written against the engine sources by reading, never
   compiled). Likely spots: `native_container_slots` slot fields, `Text::literal(&str)`, `Game::held_mut` from a
   command context, the `menu_quick_move` and `screen_click` binding names.
3. `Run-Acceptance.ps1 -Cannon`.

## 8. Risks

- Never compiled. The bindings of the three hooks not used elsewhere in this bundle (`screen_click`,
  `menu_button`, `menu_quick_move`) follow `native-hooks.json` and `native_hooks.rs` of the staged engine.
- `Game::game_dir()` is assumed to be the instance directory passed as `-GameDir` (the paste command reads
  `schematics/` there); the script writes the fixture to `$GameDir/schematics/cannon_test.nbt`.
- The cannon ticks every tick and reads up to six neighbouring containers per tick (cheap, but not free).
- `setblock`/`fill` over a cannon does not fire `destroy_block`; a stale in-memory runtime is dropped when the
  new block entity has no `Init` tag, so this is handled, but a cannon swapped by `fill ... replace` keeping its
  entity is not re-initialised.
- In-flight shots are not saved (see docs/schematicannon.md).
- The `minecraft:barrel` template gives both blocks the barrel's `facing`/`open` state properties and its
  placement (facing from the look direction); the models ignore them.
- The engine's mod menu data slots are 16-bit: counters are split in two slots (low 15 bits, high bits).
