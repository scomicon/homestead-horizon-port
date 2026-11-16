# Farmer's Delight stage 23: baskets, cabinets, rugs, ropes, mushroom colonies. Shared-file edits for the lead

New files (paths relative to the create-source checkout `CS`, and to `work/homestead-horizon/stage4` = `ST`):

- `fd/stage23.py` (content: 17 block declarations, cabinet blockstate widening; runs inside the overlay)
- `tools/fd_stage23_content.py` (check on a copy of the overlay; `--selftest` for the geometry alone)
- `mixins/homestead/storage.rs` (module `homestead_storage`), helpers `storage_entity.rs`, `storage_ticks.rs`,
  `storage_tick.rs`, `storage_container.rs`, `storage_placement.rs`
- `mixins/homestead/colony.rs` (module `homestead_colony`), helper `colony_tick.rs`
- `mixins/homestead/fragments/fd-stage23.toml` (2 modules + 6 injections)
- `docs/fd-stage23.md`
- `ST/acceptance/Test-Homestead-FdStage23.ps1` (17 scenarios, prefix `stage23-fd-`)

Nothing was built, run or committed except these new files. No change to any existing Rust module is needed.

## 1. `mixins/homestead/horizon-mixin.toml`

Append the whole of `fragments/fd-stage23.toml` at the end and raise the version by one (it is
`0.1.0+homestead.12` at the time of writing). All six injections are additional HEAD helpers on
hooks that other modules already use (`block_entity`, `block_entity_ticks`, `block_entity_tick`,
`block_entity_container`, `state_for_placement`, `random_tick`); each helper acts only on its own
blocks (baskets and cabinets; rich soil with a colony above it).

## 2. `fd/overlay.py`

After `feast.patch(out)` (same indentation inside `main()`):

```python
    spec = importlib.util.spec_from_file_location('fd_stage23', Path(__file__).with_name('stage23.py'))
    stage23 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(stage23)
    stage23.patch(out)
```

It must run after the carry (it rewrites carried block declarations and widens carried blockstates)
and needs nothing else; it does not touch items or the language file.

## 3. `ST/Build-FarmersDelight.ps1`

Raise `$version` by one (currently `'horizon.9'`, so `'horizon.10'`). Before the rebuild the content check
can run on the existing overlay:

```powershell
$py='work/llvm-mingw/llvm-mingw-20261104-msvcrt-x86_64/python/bin/python.exe'
& $py -I 'CS/tools/fd_stage23_content.py' 'work/homestead-horizon/fd-overlay'
```

(`CS` spelled as the checkout path). It prints `fd stage23 content check: ok (0 problems)`; it runs `stage23.patch` on a copy.

## 4. Acceptance wiring

`ST/Run-Acceptance.ps1`:

- `param(...)`: append `,[switch] $FdStage23`.
- `$reportName=` chain: start with `if ($FdStage23) {'run-fdstage23-result.json'} elseif (...)`.
- `$runs` chain: add next to the other FD branches (same shape as `-Crops`):
  ```powershell
  } elseif ($FdStage23) {
      $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-Homestead-FdStage23.ps1'),'FdStage23','stage23-fd-'))
  ```

`ST/regress.ps1`: append `,@('fdstage23',@{FdStage23=$true})` to `$sets`.
`ST/Build-Package.ps1`: add `'run-fdstage23-result.json'` to `$runs` and the provenance line
"Farmer's Delight: baskets, cabinets, canvas rug, rope, mushroom colonies (stage 23)".

## 5. Docs

`ST/research/roadmap.md`: a row for the slice ("Farmer's Delight stage 23: baskets, cabinets, rope, rugs, colonies;
acceptance `-FdStage23`") and the open items (tatami and full tatami mat, rope climbing of tomato vines, cooking fuel).
`CS/docs/farmersdelight-stage8.md`: a pointer to `docs/fd-stage23.md` from the "Not done yet" list.

## 6. Risks (never compiled)

- `ModContainer` with filled `faces`/`inputs`/`outputs` is used for the first time in the homestead bundle
  (the cooking pot leaves them empty); `graves.rs` does the same with empty lists too. If the engine rejects
  overlapping lists, give `faces` the slots of one face only.
- `game.native_item_entities(&dim, min, max)` is taken with `dim: String`, as `belt_funnel.rs` does.
- `native_set_item_entity` with a zero count is relied on to remove the item entity (as `belt_funnel.rs`).
- A barrel template is relied on to open the 9x3 menu of a `ModContainer` that names `minecraft:generic_9x3`
  (`graves.rs` names `minecraft:generic_9x6` the same way; its block is a barrel template too).
- `random_ticks: false` on the colony templates must switch off the vanilla mushroom spread; otherwise change the
  template (see docs/fd-stage23.md, "Not verified" 5).
- The `rope` block uses `connects_to` with a tag that has two optional entries from other mods
  (`quark:rope`, `supplementaries:rope`, `required: false`); the engine must accept missing optional ids.
- Block ids after the overlay: `tatami` and `full_tatami_mat` stay undeclared, their carried blockstates and loot
  tables are unused data and must not make the loader fail (check with `-ReloadOnly`).
