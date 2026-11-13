# Display Link + Display Board: edits to shared files (lead applies)

Slice files (all new, nothing shared was edited):

- `mixins/create/display_source.rs` (module `create_port_display_source`)
- `mixins/create/display_board_runtime.rs` (module `create_port_display_board`)
- `mixins/create/display_link_runtime.rs` (module `create_port_display_link`)
- hook helpers `display_entity.rs`, `display_ticks.rs`, `display_tick.rs`, `display_update_tag.rs`,
  `display_draw.rs`, `display_bounds.rs`, `display_right_click.rs`, `display_placement.rs`,
  `display_survival.rs`, `display_shape.rs`
- `mixins/create/fragments/displays.toml` (3 modules, 10 injections)
- `tools/display_content.py`
- `stage4/acceptance/Test-HomesteadDisplays.ps1`

No engine change: `engine/src/lib.rs` already declares `display_board`, `display_link`, `nixie`,
`source_trig` and `redstone_link`. No edit of `runtime.rs`, `right_click.rs`, `placed.rs`,
`destroy.rs` or `horizon-mixin.toml` is needed.

## 1. mixins/create/horizon-mixin.toml

Append the content of `fragments/displays.toml` at the end of the file (or let the mixin
build merge `fragments/*.toml`). All ids start with `display-`, all helper functions with
`create_display_`. Several injections on one hook already coexist (`can_survive` has three).

Ordering notes for the injections that share a hook with existing ones:

- `display-right-click` returns early if `*canceled` is already set, so it never double handles.
- `display-placement` only writes `answer` if it is still `None`.
- `display-update-tag` only writes `tag` if it is still `None`.
- `display-entity` only writes `entity` if it is still `None`.

## 2. tools/content.py

Import (next to `from link_content import ...`, line ~30):

```python
from display_content import BLOCKS as DISPLAY_BLOCKS, LINK as DISPLAY_LINK, BOARD as DISPLAY_BOARD, display_blocks, display_recipes, LANG_EN as DISPLAY_EN, LANG_RU as DISPLAY_RU
```

Block generation, directly after `redstone_link(write, model)` (line ~974):

```python
    display_blocks(write, model)
```

Recipes and loot, directly after `redstone_link_recipes(write)` (line ~1012):

```python
    display_recipes(write)
```

English names, directly after `extra_blocks[LINK] = 'Redstone Link'` (line ~1084):

```python
    extra_blocks.update(DISPLAY_EN)
```

Russian names, directly after `lang['block.create.' + LINK] = 'Звено красного камня'` (line ~1233):

```python
            lang.update({'block.create.' + k: v for k, v in DISPLAY_RU.items()})
```

## 3. tools/catalog.py

Import, next to `from link_content import ...` (line 16):

```python
from display_content import BLOCKS as DISPLAY_BLOCKS, PARTS as DISPLAY_PARTS
```

After `BLOCKS.append(LINK)` (line ~48):

```python
BLOCKS += list(DISPLAY_BLOCKS)
```

After `PARTS += [f'{LINK}/{part}' for part in LINK_PARTS]` (line ~83):

```python
PARTS += list(DISPLAY_PARTS)
```

Then run `tools/content.py` so `assets/src/catalog.rs` is regenerated (`check_distribution.py`
asserts it is current).

## 4. tools/fallback_textures.py

In the dict next to `'redstone_link': 'andesite_casing',` (line 25):

```python
    'display_link': 'andesite_casing',
    'display_board': 'andesite_casing',
```

## 5. stage4/Run-Acceptance.ps1

Add `[switch] $Displays` to the `param(...)` on line 5 (after `[switch] $Pot`).

Prepend to the `$reportName` chain on line 20:

```powershell
$reportName=if ($Displays) {'run-displays-result.json'} elseif ($Pot) { ...
```

(that is: put `if ($Displays) {'run-displays-result.json'} elseif` in front of the existing
`if ($Pot) {...}`; the existing chain stays as the tail).

In the run selection (the `if/elseif` chain around line 60-66), add one branch, e.g. right before
`} elseif ($Link) {`:

```powershell
    } elseif ($Displays) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadDisplays.ps1'),'Displays','stage11-displays-'))
```

If the chain starts with `if ($Pot)`, the first branch must be `if ($Displays) {... } elseif ($Pot) {`
instead; the intent is only that `-Displays` selects the script above.

## 6. stage4/regress.ps1

Append to the `$sets` array on line 3, after `@('pot',@{Pot=$true})`:

```powershell
,@('displays',@{Displays=$true})
```

## 7. Build / package notes

- The Python tools were not run in this authoring pass (no Python on the authoring machine);
  `display_content.py` follows `link_content.py` call for call.
- The recipes in `display_content.py` are placeholders, not Create's recipes.
- `display_board_runtime.rs` calls `Game::native_draw_text` (engine API `@API:draw_text@`) and does not call
  `BlockEntityDraw::keep` (API 94). Both exist in `staged-engine`; confirm they exist at the pinned
  `horizon_revision` before building.
- The acceptance script needs `data merge block` in the control socket's command set; if it is
  unavailable, the checks that configure a link by NBT need to be re-done through interaction.
