# Schematic tools (Schematic and Quill, Schematic deploy): shared-file edits for the lead

New files (paths relative to the create-source checkout):

- `mixins/create/schematic_tool_runtime.rs` (module `create_port_schematic_tool`)
- `mixins/create/schematool_register.rs`, `schematool_command.rs`, `schematool_use_on.rs`, `schematool_use_item.rs` (hook helpers)
- `mixins/create/fragments/schematic_tools.toml` (1 module + 4 injections)
- `tools/schematic_tool_content.py`
- `docs/schematic-tools.md`
- `stage4/acceptance/Test-HomesteadSchematicTools.ps1`

Nothing was built, run or committed. No visibility changes are needed in other Rust modules: the
runtime uses only what `schematic_data.rs` already exposes (`read`, `write`) and engine/`create_schematics` public items.

## 1. `mixins/create/horizon-mixin.toml`

Append the whole of `fragments/schematic_tools.toml` at the end. Bundle version: bump to the next
`0.11.0+homestead.1` like the earlier slices (lead's call).

## 2. `tools/content.py`

a. Imports, after the `from schematic_content import (...)` statement (lines 37-39):

```python
from schematic_tool_content import (schematic_tool_items, schematic_tool_recipes,
    LANG_EN as SCHEMATIC_TOOL_EN, LANG_RU as SCHEMATIC_TOOL_RU)
```

b. In `foundation()`, right after `schematic_items(write)` (line 986), same indentation:

```python
    schematic_tool_items(write)
```

c. Right after `schematic_recipes(write)` (line 1027):

```python
    schematic_tool_recipes(write)
```

d. English item names: right after `extra_items.update(SCHEMATIC_ITEM_EN)` (line 1135):

```python
    extra_items.update(SCHEMATIC_TOOL_EN)
```

e. Russian names: right after `lang.update({'item.create.' + k: v for k, v in SCHEMATIC_ITEM_RU.items()})` (line 1258), same indentation:

```python
            lang.update({'item.create.' + k: v for k, v in SCHEMATIC_TOOL_RU.items()})
```

## 3. `tools/catalog.py`

a. Import, after the `from schematic_content import (...)` statement (lines 21-22):

```python
from schematic_tool_content import ITEMS as SCHEMATIC_TOOL_ITEMS
```

b. After `ITEMS.extend(SCHEMATIC_ITEMS)` (line 65):

```python
ITEMS.extend(SCHEMATIC_TOOL_ITEMS)
```

(The item has no block and no `PARTS` entry; the creative tab is `all_blocks + ITEMS`.)

## 4. `tools/fallback_textures.py`

Nothing: the item uses the vanilla feather sprite.

## 5. Acceptance wiring

`stage4/Run-Acceptance.ps1`:

- line 5: append `,[switch] $SchematicTools` to the `param(...)` list (after `[switch] $Packager`).
- line 20: start the `$reportName=` chain with `if ($SchematicTools) {'run-schematic-tools-result.json'} elseif ($Switches) {` (i.e. replace the leading `if ($Switches) {`).
- line 55: replace `} elseif ($Packager) {` by

```powershell
} elseif ($SchematicTools) {
    $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadSchematicTools.ps1'),'SchematicTools','stage21-schematic-tools-'))
} elseif ($Packager) {
```

`stage4/regress.ps1` line 3: append `,@('schematictools',@{SchematicTools=$true})` after `@('packager',@{Packager=$true})`.
`stage4/Build-Package.ps1`: the `$runs` receipt list gets a 25th entry and the default package name becomes `homestead-horizon-stage15` (same pattern as stage14); add provenance lines.

## 6. Risks (never compiled)

- `registry.state(id)` descriptor fields `name` / `properties` are assumed to be `String`-like and a map of `String -> String` (as `basin.rs` and `schematic_runtime.rs` use them); the exporter converts with `to_string()`.
- `use_item_on` is assumed to pass the clicked face as a unit normal in `face`, and to fire for a block click with the held item before the block's own interaction; `use_item` is assumed not to double-fire the same click (the handler only acts if a selection exists, so a repeat is harmless).
- `game.held()` / `held_mut()` / `native_sneaking()` / `native_on()` / `Game::game_dir()` are used as `cannon_*` and `schematic_data.rs` use them.
- Exports leave out block entity data and entities (the loader only accepts schemas it knows), so a saved chest comes back empty. Blocks the loader does not support (`create:` blocks without an implemented schema, unlisted vanilla blocks) are saved but rejected when pasted/loaded; `create schematic inspect` shows which.
- DataVersion is written as `create_schematics::SOURCE_DATA_VERSION` (3955), which is what the loader requires.
- The Quill selection is one global handler (Create's is client side per player); in multiplayer two operators would share it.
