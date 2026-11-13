# Logic mods group A (stage 19 slice): shared-file edits for the lead

New files (paths relative to the create-source checkout `CS`, and to `work/homestead-horizon/stage4` = `ST`):

- `mixins/homestead/doubledoors.rs`, `shulkerdrops.rs`, `invisibleframes.rs`, `accdecay.rs` (modules
  `homestead_doors`, `homestead_shulker`, `homestead_frames`, `homestead_decay`)
- helpers `doors_click.rs`, `shulker_drops.rs`, `frames_interact.rs`, `decay_changed.rs`,
  `decay_tick.rs`, `tempt_calm.rs` (same folder)
- `mixins/homestead/fragments/logicA.toml` (4 `[[module]]`, 6 `[[injection]]`, ids prefixed `logica-`)
- `docs/logic-a.md`
- `ST/acceptance/Test-HomesteadLogicA.ps1` (12 scenarios, prefix `stage25-logica-`)

Nothing was built, run or committed. The fragment parses, its ids do not clash with the bundle and
every helper/source file it names exists.

## 1. `mixins/homestead/horizon-mixin.toml`

Append `fragments/logicA.toml` at the end. Bump `version` (currently `0.1.0+homestead.8`) to the next
one (`0.1.0+homestead.9`, or the lead's choice if stage 17/18 already changed it).

The tempt injection targets `horizon_client_lib::mobs::goals::<Tempt as Goal>::can_use` (the
`mobs` module lives in the library crate, as the anvil injection's `horizon_client_lib::stations`).
If the build says the target cannot be resolved, check the trait impl spelling (`impl Goal for Tempt`
in `crates/horizon-client/src/mobs/goals.rs`); if the compile complains that `calm_down` is private,
the helper is not placed in the `goals` module, and the fallback is to drop this one injection (the
other five are independent) and document No Animal Tempt Delay as not ported.

## 2. `ST/Run-Acceptance.ps1`

- `param(...)`: append `,[switch] $LogicA` before the closing parenthesis.
- `$reportName=`: put `if ($LogicA) {'run-logica-result.json'} elseif ...` at the front of the chain.
- In the `$runs` branch chain add
  ```powershell
  } elseif ($LogicA) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadLogicA.ps1'),'LogicA','stage25-logica-'))
  ```
  in the same shape as the neighbouring `-Crops` / `-Feasts` branches.

## 3. `ST/regress.ps1`

Append `,@('logica',@{LogicA=$true})` inside `$sets=@(...)` (after the last entry).

## 4. `ST/Build-Package.ps1`

Add `'run-logica-result.json'` to the `$runs` receipt list and a provenance line such as
"Logic mods A: Double Doors, Shulker Drops Two, Invisible Frames, Accelerated Decay, No Animal Tempt Delay".

## 5. Docs

Optional: roadmap rows (Stack Refill etc. stay open; the five here are done) in
`ST/research/roadmap.md`, and the package README row for this slice.

## Risks for the first build (all unverified)

- `interact_entity` may not fire for item frames; then the Invisible Frames scenario fails with a
  clear message and the mod needs another hook (`use_item_on` does not reach entities).
- `horizon-ctl entities` lists mobs, items and orbs; if it omits item frames the test cannot get
  their id and reports that instead of passing.
- `Game::native_lifecycle.epoch()`, `EntityRef::new`, `Tag::Byte`, `Compound::default()` and
  `Compound::insert` are used as read in the engine source; a name or signature may differ.
- The `loot_drops` binding type `&horizon_client::mod_api::NativeLootDrops<'_>` must match the
  target's parameter exactly.
- The decay sweep calls `native_block` for up to 3375 positions per queued check; a felled tree
  queues one check per log, up to 128 entries. If that costs ticks, merge entries within a few
  blocks of each other.
- The door scenarios assume `open` is still the pre-click value when the `right_click_block` HEAD
  runs and that the helper runs before the door's own use.
