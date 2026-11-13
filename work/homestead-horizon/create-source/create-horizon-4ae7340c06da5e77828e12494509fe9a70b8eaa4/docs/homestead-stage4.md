# Homestead stage 4: Andesite Belt Funnel

Scope: the belt-attached form of the Andesite Funnel (`BeltFunnelBlock`,
Create 6.0.10), placed by the existing Funnel item on a horizontal Belt.

## What is ported

- `engine/src/core/funnel.rs`: pure shape/placement/traversal rules
  (3 unit tests, part of the 267 passing core tests).
- `mixins/create/belt_funnel.rs`: native adapter (support check, Wrench cycle,
  chest/barrel IO, belt interception, parent-form fallback).
- Generated content (`tools/funnel_content.py`, `tools/content.py`): block
  `create:andesite_belt_funnel` (no item, drops the Andesite Funnel), eight
  fallback models (`funnel/belt_<shape>_<on|off>`), blockstate, loot table,
  localization, pickaxe tag. Selection/collision shapes are keyed by
  `facing` and `shape` from `AllShapes.BELT_FUNNEL_*`.

## Placement design

Horizon accepts a placement answer only for the block the item places
(`state_for_placement` rejects another block's state), so the belt form cannot
be answered directly. The ordinary Funnel is placed with its real `facing` and
`extracting` flag; on a supported Belt the adapter's `world_tick` converts it to
the belt form with `Shape::for_belt`. A non-sneaking placement therefore yields
`pushing` (extracting) on a perpendicular Belt, as `getShapeForPosition` does;
the Wrench turns it into `pulling`.

## Verified in game (native client, disposable world)

`work/homestead-horizon/stage4/Run-Acceptance.ps1` (PowerShell 7) drives
`acceptance/Test-HomesteadBeltFunnels.ps1`: Flow, FrontBlocked, SidePass and a
cold restart (Reload) all passed, with normal saved exit. Covered: item placement
on Chest/Trapped Chest/Barrel, Wrench shape toggles, redstone pause, stopped and
reversed Belts, inline backpressure versus side bypass, restoring cargo after a
process restart.

## Known differences / not covered

- Item-entity-only `FUNNEL_COLLISION` slab of `getCollisionShape` is not ported.
- Brass belt funnel, sloped belts, filters, waterlogged parent restoration and
  the Item Vault remain out of scope.
- Geometry came from `AllShapes` via a summarised web fetch; recheck against the
  pinned source.
- No Java-client comparison was run for this increment.
- Packaging (`outputs/homestead-horizon-stage4`) is not built yet.

## Local build notes (Windows)

Dot-source `work/rust-env.ps1`, put a `dlltool.exe` copy of
`x86_64-w64-mingw32-dlltool.exe` on PATH, set `PYTHONUTF8=1` before running
`tools/content.py`. Use `stage4/build.ps1` (cargo output is redirected through
`cmd /c`, since Windows PowerShell 5.1 treats native stderr as an error) and
`stage4/Prepare-Candidate.ps1` before the acceptance runner. The acceptance
scripts force the invariant culture (a Russian locale prints `136,5`).
