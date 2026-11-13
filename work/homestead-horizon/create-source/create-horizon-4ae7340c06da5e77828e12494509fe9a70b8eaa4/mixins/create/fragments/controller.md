# Linked Controller: shared-file edits for the lead

New files (paths relative to the create-source checkout): `mixins/create/controller_runtime.rs` (module),
helpers `controller_component.rs`, `controller_register.rs`, `controller_command.rs`, `controller_tick.rs`,
`controller_packet.rs`, `controller_flush.rs`, `controller_right_click.rs`, `mixins/create/fragments/controller.toml`,
`tools/controller_content.py`, `docs/linked-controller.md`, `stage4/acceptance/Test-HomesteadController.ps1`.
Nothing was built, run or committed.

## 1. `mixins/create/link_runtime.rs` (append one function at the end)

The controller needs the level's `LinkNetwork`, which this file owns privately. `ControllerServer::receive_pressed`
discards the receiver writes of `add_to_network`, so the function rescans the receivers after a call that asked
for it and stores any changed value through the normal `apply` / `flush` path.

```rust
/// Linked Controller access to the level's network (controller_runtime.rs). `f` gets the network and
/// answers whether receiver values may have changed; if so every receiver is re-read.
pub(super) fn controller_net<R>(
    game: &mut Game<'_>,
    dimension: &str,
    f: impl FnOnce(&mut LinkNetwork) -> (R, bool),
) -> R {
    let (result, writes) = with_dimension(game, dimension, |d| {
        let (result, scan) = f(&mut d.net);
        let writes: Vec<(Pos, i32)> = if scan {
            d.ids
                .iter()
                .filter_map(|(pos, id)| {
                    d.net
                        .get(*id)
                        .filter(|entry| matches!(entry.mode, LinkMode::Receive { .. }))
                        .map(|entry| (*pos, entry.received()))
                })
                .collect()
        } else {
            Vec::new()
        };
        (result, writes)
    });
    apply(game, None, writes);
    result
}
```

(`with_dimension` takes `&Game`; the `&mut Game` reborrows. `apply` queues only receivers whose stored value changed.)

## 2. `mixins/create/horizon-mixin.toml`

Append `mixins/create/fragments/controller.toml` (one `[[module]]`, 7 `[[injection]]`s), with ONE exception: cut the
last injection (`controller-bind`, hook `right_click_block`) and insert it directly BEFORE the injection with
`id = "right-click"` (toml line ~342). Reason: the link's own right click (in `right_click.rs`) would take the
controller as a frequency item; `controller_right_click.rs` returns early when `*canceled` is already set.
If ordering is not guaranteed by file position, the fallback is one call at the top of
`mixins/create/right_click.rs`, before the first `if`:

```rust
    if crate::game::create_port_controller::right_click(game, pos) {
        game.swing_arm();
        *canceled = true;
        return;
    }
```
and then drop the `controller-bind` injection and `controller_right_click.rs`.

Hooks used (all already have other injections): `component_types`, `register_commands`, `perform_command`, `tick`,
`net_from_client`, `level_tick_post`, `right_click_block`.

## 3. `tools/content.py`

1. Imports, after the `from schematic_content import (...)` block (line ~39):
   ```python
   from controller_content import (controller_items, controller_recipes,
       ITEM_LANG_EN as CONTROLLER_ITEM_EN, ITEM_LANG_RU as CONTROLLER_ITEM_RU)
   ```
2. After `schematic_items(write)` (line ~986): `controller_items(write)`
3. After `schematic_recipes(write)` (line ~1027): `controller_recipes(write)`
4. After `extra_items.update(SCHEMATIC_ITEM_EN)` (line ~1135): `extra_items.update(CONTROLLER_ITEM_EN)`
5. After `lang.update({'item.create.' + k: v for k, v in SCHEMATIC_ITEM_RU.items()})` (line ~1258), same indentation:
   `lang.update({'item.create.' + k: v for k, v in CONTROLLER_ITEM_RU.items()})`

`catalog.py` and `fallback_textures.py` need nothing (an item, no blocks or parts).

## 4. Acceptance wiring

`stage4/Run-Acceptance.ps1`:
- line 5, `param(...)`: add `,[switch] $Controller` before the closing parenthesis.
- line 20, `$reportName=`: start the chain with `if ($Controller) {'run-controller-result.json'} elseif ($Switches) ...`
  (the existing leading `if ($Switches)` becomes `elseif`).
- before `} elseif ($Packager) {` (line ~55) insert:
  ```powershell
      } elseif ($Controller) {
          $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadController.ps1'),'Controller','stage23-controller-'))
  ```
  (match the exact layout of the neighbouring `$Switches` / `$Packager` branches).

`stage4/regress.ps1`, in `$sets=@(...)` after the `packager` entry add `,@('controller',@{Controller=$true})` (25 suites).

`stage4/Build-Package.ps1`: add `'run-controller-result.json'` to the `$runs` list (line 20, after
`run-packager-result.json`) and a provenance line "Create: Linked Controller (stage 23)" next to the stage 22 line
(line ~128); bump the default package name when packaging.

## 5. Docs

Optional: in `docs/redstone-link.md` mention `docs/linked-controller.md`; `stage4/research/roadmap.md` row for the
Linked Controller.