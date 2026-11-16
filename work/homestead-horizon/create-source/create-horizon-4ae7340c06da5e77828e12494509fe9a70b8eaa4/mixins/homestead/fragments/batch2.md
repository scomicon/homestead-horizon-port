# Batch 2 (stage 23): shared-file edits for the integrator

Data-only batch: **no Rust, no `horizon-mixin.toml` change, no bundle version bump, no client rebuild is
needed.** The slice packs 6 remaining data-tier / content mods as flat `.hmod` archives using the
player's own jars via `horizon-port-pack carry`.

New files (paths relative to the create-source checkout `CS`, or to `work/homestead-horizon/stage4` `ST`):

- `CS/tools/batch2_content.py` (`overlay` and `check-policy` sub-commands)
- `ST/batch2-policy.json` (per-mod policy: licence label, `publicRelease`, pruning, estimates switches)
- `ST/Build-DataMods-batch2.ps1` (carry from the player's own jars, overlay, creative tab, playtest fixes, archives)
- `ST/acceptance/Test-Homestead-Batch2.ps1` (receipt/transcript prefix `stage23-batch2-`)
- `CS/docs/batch2.md` (done / differs / not verified per mod, plus excluded candidates)
- `CS/mixins/homestead/fragments/batch2.toml` (empty marker; no Rust hooks)

Nothing was built or run in the game while writing this slice; checks run were Python syntax/compile,
policy validation (`batch2_content.py check-policy`), and PowerShell AST parse checks without execution.

## Order of operations (build machine)

1. Ensure batch1 (`ST/Build-Batch1.ps1`) is built first if Crate Delight should keep its Expanded Delight
   crates (`peanut_crate`, `salt_bag`, `ground_cinnamon_bag`).
2. `pwsh ST/Build-DataMods-batch2.ps1` (reads `pack/inventory.json` and prepared 26.3 `resources` with
   `facts.json`; writes `ST/extra-mods/<id>-<ver>-batch2.1.hmod` and `work/homestead-horizon/pack-batch2/`).
3. `pwsh ST/Run-Acceptance.ps1 -Batch2` (`Start-Test.ps1` installs every `extra-mods/*.hmod`).
4. Full `regress.ps1`, then `Build-Package.ps1`.

## 1. `ST/Run-Acceptance.ps1`

- `param(...)` (line 5): append `,[switch] $Batch2` before the closing parenthesis.
- `$reportName=` (line 22): start the chain with `if ($Batch2) {'run-batch2-result.json'} elseif ($Fixes2) ...`
  (turning the existing leading `if ($Fixes2)` into `elseif ($Fixes2)`).
- Directly after the `} elseif ($Batch1) {` branch (around line 88), insert:

```powershell
    } elseif ($Batch2) {
        $runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-Homestead-Batch2.ps1'),'Batch2','stage23-batch2-'))
```

No window is needed (`HOMESTEAD_WINDOWED` stays unset).

## 2. `ST/regress.ps1`

In `$sets=@(...)` (line 3), append `,@('batch2',@{Batch2=$true})` after `@('engine20',@{Engine20=$true})`.

## 3. `ST/Build-Package.ps1`

- `$runs` (line 24): append `'run-batch2-result.json'`.
- The `.hmod` archives need no copy-step edit: `Copy-Item -Path (Join-Path $PSScriptRoot 'extra-mods/*.hmod')`
  (line 115) already ships all `*-batch2.1.hmod` files, and `Start-Homestead.ps1` enables every `.hmod`
  by filename prefix before `-` (checked by `Build-DataMods-batch2.ps1`).
- Provenance and `README.ru.md` rows: "Batch 2: Ocean's Delight, Crate Delight, Obsidian Equipment Reworked,
  Minecraft Accelerated, Create Deco (shells), Sooty Chimneys (blocks only)". State clearly that Ocean's
  Delight food stats and Obsidian armor/tool stats are estimates, and that Ocean's Delight mob loot
  injections and Sooty Chimneys smoke/scraping are not ported.
- **Release archives:** exclude any `.hmod` whose entry in `work/homestead-horizon/pack-batch2/batch2-summary.json`
  has `"publicRelease": false` (`createdeco`) from any public release archive.

## 4. `ST/research/roadmap.md`

Add a row for `Batch 2 (data mods)` in the completed/in-progress table once `-Batch2` passes, and list
the deferred Java halves (Ocean's Delight entity loot injection & guardian soup serving, Sooty Chimneys
smoke particles & scraping, Obsidian Equipment survival template loot).

## 5. Not needed

`ST/Build-DataMods.ps1` (untouched; deletes only `*-horizon.*.hmod`), `ST/Start-Test.ps1` (already
installs `extra-mods/*.hmod`), `CS/mixins/homestead/horizon-mixin.toml`, `CS/tools/content.py`,
`CS/tools/catalog.py`.

## 6. Risks for the first run

- `createdeco` ships 397 blockstates and `create:pressing` recipes: `carry` may skip custom block classes
  without vanilla property matches, and `advancement_fix.py` is strict on unknown location predicates.
- `obsidianequipmentrework` ships 26.3-style `obsidian_nautilus_armor` and `obsidian_spear`; verify in
  `-Batch2` that `carry` + `overlay` load cleanly without unknown-template errors.
- `cratedelight` keeps Expanded Delight crates only when `pack-batch1/mods/expandeddelight/carry` exists
  during `Build-DataMods-batch2.ps1`.
