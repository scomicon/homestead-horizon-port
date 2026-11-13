# Logic mods group C (Stack Refill, CraftingPad): shared-file edits for the lead

New files (paths relative to the create-source checkout unless marked ST = `work/homestead-horizon/stage4`):

* `mixins/homestead/stack_refill.rs` (module `homestead_stack_refill`), `stack_refill_tick.rs`, `stack_refill_drop.rs`
* `mixins/homestead/craftingpad.rs` (module `homestead_craftingpad`), `craftingpad_use.rs`, `craftingpad_table.rs`
* `mixins/homestead/fragments/logicC.toml` (2 modules, 4 injections), this file
* `tools/craftingpad_content.py`, `docs/logic-c.md`
* ST `Build-CraftingPad.ps1`, ST `acceptance/Test-HomesteadLogicC.ps1`

Nothing was built, run or committed. Trade Cycling is not ported (docs/logic-c.md says why).

## 1. `mixins/homestead/horizon-mixin.toml`

Append `fragments/logicC.toml` verbatim at the end. Bump the homestead bundle version (it was
`0.1.0+homestead.8`; `0.1.0+homestead.9`). Hooks used: `level_tick_pre`, `item_dropped`,
`use_item` (all already used by other injections) and a new `target` head on
`horizon_client::game::storage::Game<'_>::tick_craft_table` (`cancellable = true`, the helper
returns `ControlFlow<()>`, like `anvil_limit.rs`). Ids are `stack-refill`, `craftingpad`,
`stack-refill-tick`, `stack-refill-drop`, `craftingpad-use`, `craftingpad-table`: check them
against the existing ids.

## 2. CraftingPad content mod

`ST/Build-CraftingPad.ps1` writes `ST/extra-mods/craftingpad-1.0.12-horizon.1.hmod`.

* `ST/Build-DataMods.ps1` line `Get-ChildItem $extra -Filter '*-horizon.*.hmod' | Where-Object { $_.Name -notlike 'farmersdelight-*' } | Remove-Item`:
  change the filter to `-notlike 'farmersdelight-*' -and $_.Name -notlike 'craftingpad-*'`
  so the data-mod build keeps the pad.
* Run `Build-CraftingPad.ps1` once before `Prepare-Candidate.ps1` and the acceptance (after
  `Build-DataMods.ps1`, if both run).
* `Build-Package.ps1`: nothing for the archive if the package copies every `extra-mods/*.hmod`
  (the data mods are handled that way); otherwise add the file next to the Farmer's Delight
  archive, and add a provenance line "Homestead: Stack Refill and CraftingPad (stage 25)".
* `ST/Test-Start`/`Start-Homestead.ps1`: all `.hmod` of `extra-mods` are loaded, no change.

## 3. Acceptance wiring

`ST/Run-Acceptance.ps1`:

* `param(...)`: add `,[switch] $LogicC`.
* `$reportName=` chain: start with `if ($LogicC) {'run-logicc-result.json'} elseif ...`.
* `$runs` chain: add the branch
  ```powershell
  } elseif ($LogicC) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicC.ps1'),'LogicC','stage25-logicc-'))
  ```
  in the layout of the neighbouring branches.

`ST/regress.ps1`: append `,@('logicc',@{LogicC=$true})` to `$sets` (one suite more than now).
`ST/Build-Package.ps1`: add `'run-logicc-result.json'` to `$runs`, the provenance line, the new
default package name.

The test runs in survival mode and sets creative again at its end; it needs the CraftingPad
archive loaded (`give @s craftingpad:craftingpad`).

## 4. Docs

Optional: `ST/research/roadmap.md`: a row "Stack Refill, CraftingPad (stage19); Trade Cycling
not ported" and remove both from the list of small logic mods still to do.

## 5. Risks (never compiled)

* `Game` fields used from the helpers and the new module: `selected`, `screen`, `remote`,
  `player`, `craft_size`, `craft_table`, `capture(bool)`; `ItemStack::count` assumed an integer
  that casts to `u64`; `items().get(..).max_damage` assumed castable to `i32`.
* `level_tick_pre` may run once per loaded dimension each tick; the refill is idempotent
  (the second call sees a refilled hand), the snapshot is overwritten by each call.
* Thread-local state: the refill and the `item_dropped` hook are assumed to run on the same
  thread, as the controller's thread-locals do.
* `tick_craft_table` is `pub(super)` in `game/storage.rs`; the mixin target path follows the
  `advance_clock` example (`horizon_client::game::chat::Game<'_>::advance_clock`).
* Placeholder recipe JSON uses the `item` key like the carried recipes; if the loader wants
  plain strings, change it in `tools/craftingpad_content.py`.
