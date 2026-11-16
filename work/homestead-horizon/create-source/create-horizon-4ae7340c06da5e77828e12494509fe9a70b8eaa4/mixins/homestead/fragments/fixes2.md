# Playtest fixes, part 2 (stage22): shared-file edits for the integrator

New files (paths relative to the create-source checkout `CS`, and `ST` = `work/homestead-horizon/stage4`):

- `CS/tools/advancement_fix.py` (+ `test_advancement_fix.py`)
- `CS/tools/block_looks.py` (+ `test_block_looks.py`)
- `CS/tools/hide_unplaceable.py`, `CS/tools/unplaceable_items.json` (+ `test_hide_unplaceable.py`)
- `ST/acceptance/Test-HomesteadFixes2.ps1`
- `CS/docs/playtest-fixes-2.md`, this fragment

No Rust, no `horizon-mixin.toml` change, no client rebuild: the three fixes only change what
`Build-DataMods.ps1` packs into the `.hmod` archives. After the edits below, run
`ST/Build-DataMods.ps1`, then `Prepare-Candidate.ps1` as usual, then `-Fixes2` and `-DataMods`.

Run the unit tests with the bundled python (66 tests, about 8 s), from `CS/tools`:

```powershell
$env:PYTHONUTF8 = '1'
& "<workspace>\work\llvm-mingw\llvm-mingw-20261104-msvcrt-x86_64\python\bin\python.exe" -m unittest test_advancement_fix test_block_looks test_hide_unplaceable
```

(Without `-I`: `-I` removes the current folder from the import path.)

## 1. `ST/Build-DataMods.ps1`

Add the three steps right after the creative tab step (the `& $python -I ... creative_tab.py --out $tabs $carry`
line) and before `$over = @{}` is built. All three write into the same `$tabs` overlay folder, so the existing
loop that fills `$over` picks their files up, and an overlay file replaces the carry file of the same path.
`hide_unplaceable.py` must run after `creative_tab.py` (it filters the tab file that step wrote) and prints the
files to drop as `DROP <relative path>` lines.

```powershell
    # Playtest fixes, part 2 (stage22): modded boats, advancement predicates, block occlusion and collision.
    $dropFix = @(& $python -I (Join-Path $create 'tools/hide_unplaceable.py') --out $tabs $carry |
        Where-Object { $_ -like 'DROP *' } | ForEach-Object { $_.Substring(5) })
    & $python -I (Join-Path $create 'tools/advancement_fix.py') --out $tabs $carry | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "advancement_fix failed for $id (an unsupported predicate shape; see its message)" }
    & $python -I (Join-Path $create 'tools/block_looks.py') --out $tabs $carry | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "block_looks failed for $id" }
    $drop = @($drop) + $dropFix
```

`$drop` is the per-mod list built from `data-mods-skip.json` a few lines above (`$drop = @($skip[$id])`); the
archive loop already skips every path in it. Optionally bump `$version = 'horizon.2'` to `'horizon.3'` so the
archives differ from the ones players already have.

Notes:
- `advancement_fix.py` is strict by default: an unknown key in a location predicate stops the build for that mod
  with the file and path in the message (the real data passes; see the doc). Use `--no-strict` only as a
  temporary workaround.
- `block_looks.py` writes `collision_shapes` as well as `properties.occludes`. Add `--no-collision` to the
  call if only the occlusion fix should ship, or `--shapes` to also replace the targeting outline.

## 2. `ST/Run-Acceptance.ps1`

Add a `-Fixes2` switch next to `-DataMods` and the same three wiring points that `-DataMods` has:

- `param(...)`: append `,[switch] $Fixes2`.
- the `$reportName=` chain: start with `if ($Fixes2) {'run-fixes2-result.json'} elseif ...` (the former leading `if` becomes `elseif`).
- the `$runs` chain: a branch for `Test-HomesteadFixes2.ps1`, modelled on the `-DataMods` branch:
  `$runs=@(,@((Join-Path $PSScriptRoot 'acceptance/Test-HomesteadFixes2.ps1'),'Fixes2','stage28-fixes2-'))`

The script needs the running client only for its last check (the advancement revoke); the static checks read the
built archives in `stage4/extra-mods`, so build them first.

## 3. `ST/regress.ps1`

In `$sets=@(...)`, directly after the `datamods` entry, add `,@('fixes2',@{Fixes2=$true})` (it must run after
`-DataMods` has proven the rebuilt archives load).

## 4. `ST/Build-Package.ps1`

- Add `'run-fixes2-result.json'` to the `$runs` receipt list.
- Add a provenance line "Playtest fixes 2: advancement predicates, block occlusion/collision, hidden modded boats (stage 22)".
- README.ru.md row (honest wording): the 45 data mods no longer grant the structure advancements at once,
  small-model blocks no longer leave holes and have a matching collision, Regions Unexplored boats are hidden
  because boats need an entity layer; not verified visually.

## 5. Order of work for the integrator

1. `Build-DataMods.ps1` edit, then `pwsh ST/Build-DataMods.ps1` (about two minutes; it unpacks nothing, it only reads `carry`).
2. `pwsh ST/Run-Acceptance.ps1 -Fixes2` and `-DataMods` (one game at a time).
3. Existing worlds keep the advancements they were already granted. Use a new world, or
   `advancement revoke @s everything` once, to see the effect.

## 6. Risks

- **Collision of blocks on a template guess.** `collision_shapes` replaces the template collision with the model
  bounds (all four turns united when the blockstate rotates). A block that is bigger than its bounds (a chair with
  thin legs) gets a box around the legs, which is deliberate. Blocks with state-dependent templates (slab,
  stairs, trapdoor, door, fence, wall, pane) are only counted, not changed.
- **Models the tool cannot read** (743 blocks, mostly Regions Unexplored and Chipped, parents outside the mod)
  keep their template and still show the original bugs. They are listed by `block_looks.py --dry-run --report`.
- **Geo/entity blocks** (113): occlusion is fixed, collision stays the template's because the shape is unknown.
- **Python `zlib`**: the bundled llvm-mingw python cannot open deflated archives. The acceptance script unpacks
  the archives with .NET and checks the folders; the checkers print a clear message if given a deflated archive.
