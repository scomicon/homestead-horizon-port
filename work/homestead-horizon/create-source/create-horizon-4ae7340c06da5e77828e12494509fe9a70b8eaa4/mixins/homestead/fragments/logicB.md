# Logic mods group B (stage19): shared-file edits

New files (create-source root unless noted): `mixins/homestead/falling_trees.rs` (module
`homestead_falling_trees`), `mixins/homestead/falling_trees_destroy.rs` (hook helper),
`mixins/homestead/fragments/logicB.toml` (1 module, 1 injection), `docs/logic-b.md`,
`stage4/acceptance/Test-HomesteadLogicB.ps1`. Nothing was built, run or committed. Only
Falling Trees is in: Bridging Mod, Comforts and SimpleHats cannot be built on the engine's hooks
(`docs/logic-b.md` has the reasons and what would unblock each).

## 1. `mixins/homestead/horizon-mixin.toml`

Append the whole of `fragments/logicB.toml` at the end. Bump the version from
`0.1.0+homestead.8` to `0.1.0+homestead.9`. The `destroy_block` hook already has injections
(Create's, in the create bundle); this one is an additional helper, it only acts when `canceled`
is still false. The create bundle's `destroy_block` helpers that cancel (`inventory_blocked`) run
in whichever order the bundles apply; the helper has no order dependency.

## 2. `stage4/Run-Acceptance.ps1`

- `param(...)` line 5: add `,[switch] $LogicB`.
- `$reportName` chain (line 21): start it with `if ($LogicB) {'run-logicb-result.json'} elseif ($Motor) ...`
  (the existing leading `if ($Motor)` becomes `elseif`).
- In the `$runs` chain, next to the `-Feasts` branch (line 58) add:
  ```powershell
  } elseif ($LogicB) {
  $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicB.ps1'),'LogicB','stage25-logicb-'))
  ```

## 3. `stage4/regress.ps1`

In `$sets`, after the last entry add `,@('logicb',@{LogicB=$true})` (one more suite).

## 4. `stage4/Build-Package.ps1`

Add `'run-logicb-result.json'` to `$runs` (line 20), a provenance line such as
`'Homestead logic: Falling Trees (stage 25)'` after the stage 24 line (line ~133), and bump the
default package name to `homestead-horizon-stage19`.

## 5. Docs

`stage4/research/roadmap.md`: a row for the Falling Trees port and, in the list of small logic
mods still to do, remove `fallingtrees`, and note that `bridgingmod`, `comforts` and `simplehats`
are blocked on engine hooks (see `docs/logic-b.md`).

## 6. Risks (never compiled)

- `game.world.loaded(x, z)`, `game.native_machine_destroy_block(pos, tool, collect)`,
  `game.native_server_block_in_tag(state, tag)`, `game.damage_held_in_slot(u32, EquipmentSlot)`
  and `game.player.as_mut().cause_food_exhaustion(f32)` are used as `native_machines.rs`,
  `native_world_read.rs` and `harvest.rs` define and use them. If `native_machine_destroy_block`
  turns out to call the `destroy_block` hook again the `BUSY` flag stops the recursion.
- The `destroy_block` hook is assumed to fire for a player's break only (the earlier slices found
  that `setblock ... destroy` does not fire it) and before the block is removed.
- The test breaks with `break` on a block looked at from the north face; if the control socket
  needs another face or a different distance the `Chop` helper is the one place to change.
- The default tree limits (500 logs, leaf flood depth 7) and "no tool required" are recalled from
  the mod, not read from its jar.
