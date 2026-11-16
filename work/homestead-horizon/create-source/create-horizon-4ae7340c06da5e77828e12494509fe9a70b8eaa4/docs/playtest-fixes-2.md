# Playtest fixes, part 2 (stage22)

Four problems from the second round of playtesting the carried data mods (`pack/mods/*/carry`, packed by
`Build-DataMods.ps1`). All four are repaired in the pack step: nothing in Rust or the engine changes and the client
is not rebuilt. The diagnosis is in `stage4/research/playtest-bugs-2.md`; the shared-file edits are in
`mixins/homestead/fragments/fixes2.md`.

## What was wrong

| # | Symptom | Cause | Fix | Tool |
| --- | --- | --- | --- | --- |
| A | A see-through hole under plush toys | `carry` has no oracle for these mods, so it guesses a vanilla template from the blockstate properties (`facing` -> `carved_pumpkin`, none -> `stone`). That template is an opaque full cube, so the faces of the neighbouring blocks are culled while the model is small | `properties.occludes: false` for every block whose model is not an opaque cube | `block_looks.py` |
| A2 | The player walks into some blocks and drops into the crawl pose (extra scope, see below) | The same template guess also sets the collision: a full cube under a flower or a carpet, and no collision at all on the button, plate, vine and crop templates | `collision_shapes` from the model bounds; plants get no collision | `block_looks.py` |
| B | Dozens of structure and biome advancements granted at once (1000 XP each from Repurposed Structures) | 1.20.1/1.21.1 `location` predicates say `structure` and `biome`; Horizon 26.3 reads only `structures` and `biomes`, ignores every other key and then treats the predicate as always true | rename the keys; fail on shapes it cannot map; never grant a visible advancement from an empty predicate | `advancement_fix.py` |
| C | Modded boats cannot be placed | Placing a boat needs a boat entity; the client only places `minecraft:<wood>_boat`, `_chest_boat` and `_raft`, and modded entities do not exist | hide the 32 Regions Unexplored boat items from the creative tabs and drop their recipes | `hide_unplaceable.py` |

## The tools

All are plain python (standard library only), run through `Build-DataMods.ps1` with the same interface as
`creative_tab.py`: `tool [--out <dir>] <mod dir>`. With `--out` only changed files are written there and the mod
dir is left alone; an overlay file replaces the carry file of the same path in the archive.

### `advancement_fix.py`

- Walks every JSON under `data/<ns>/{advancement,advancements,predicate,predicates,loot_table,loot_tables}`.
- In each location predicate (`location`, `stand_on`, `steps_on`, `movement_affected_by`, and the `predicate` of a
  `location_check`) it renames `biome` -> `biomes`, `structure` -> `structures` and the pre-1.19 `feature` ->
  `structures`. A string, `#tag` or list value is kept as it is: 26.3 matches all three.
- It fails loudly (exit code 2, file and path in the message) on a value of another shape, on a singular and a
  plural key together, and on an unknown key. The one tolerated unknown key is `player`: Incendium nests a stray
  `{"player": {"location": {...}}}` that 26.3 ignores. `--no-strict` turns the errors into warnings.
- A visible advancement (its `display` announces to chat or shows a toast) with a `minecraft:location` criterion
  whose whole condition is a vacuous predicate is neutralised to `minecraft:impossible`. Technical advancements
  (no `display`) are left alone: Incendium keeps an always-true `log_spam_fixer` criterion in them on purpose and
  neutralising it would stop their functions from ever running. `minecraft:tick` roots are never touched.
- `--check <archive-or-dir>` exits 1 when a file would still change.

Real data (45 mods, strict mode passes): 4 files in Regions Unexplored, 18 in Repurposed Structures and
21 advancements plus 18 predicates in Incendium change: 183 `biome`, 119 `structure` and 1 `feature` renames. The
diagnosis counted 42 advancement files; the 43rd is Incendium's `on_sword_get`, which used `feature`. Nothing
needed neutralising on the real data. The two loot tables the diagnosis flagged contain no location predicate.

### `block_looks.py`

For every block declaration `data/<ns>/horizon/block/*.json` it reads the blockstate and the models (parents inside
the mod; vanilla cube, plant and thin parents are known) and classifies them:

- **cube**: a cube parent, or non-rotated elements that cover all 4096 cells of the 16x16x16 grid;
- **transparent**: `render_type` cutout, cutout_mipped or translucent (glass, leaves, vines);
- **plant**: the `cross`/`crop` family, no collision in vanilla;
- **geo**: a model with no geometry of its own (drawn by a block entity);
- the union bounding box of all models, united with its three 90 degree turns when the blockstate rotates.

Templates are sorted into three groups:

| Group | Templates | What is set |
| --- | --- | --- |
| Full cube | `stone`, `carved_pumpkin`, `oak_log`, `redstone_lamp`, `furnace` | `occludes: false` unless the model is an opaque cube; `collision_shapes` = model bounds when the model is smaller than a cube; none for plants; geo: occlusion only |
| No collision | `oak_pressure_plate`, `oak_button`, `vine`, `wheat` | a collision only when every model is a solid block on its own and none is transparent (a button-like block with different models per face is judged model by model, not by their union) |
| State dependent | `oak_trapdoor`, `oak_stairs`, `oak_door`, `oak_fence_gate`, `oak_slab`, `glass_pane`, `oak_fence`, `cobblestone_wall` | nothing, only counted (`slab_mismatch` is a rough diagnostic: a slab model whose height is not about half a block) |

A model it cannot resolve (a parent in another mod, a missing file) is left alone and counted as `unknown`. The
result is merged into the existing declaration: other `properties` and an existing `collision_shapes` are kept.
`--shapes` also writes the targeting outline, `--no-collision` writes only the occlusion fix, `--dry-run --report`
only counts.

`--check <archive-or-dir>` is the checker the acceptance test calls: it exits 1 when a block with a non-cube
model still occludes, still has the template's oversized collision, a missing collision on a solid model on a
no-collision template, or an empty collision on a solid cube.

### `hide_unplaceable.py` and `unplaceable_items.json`

The policy names, per namespace, fnmatch patterns over item names (`regions_unexplored`: `*_boat`,
`*_chest_boat`). The tool prints `DROP <path>` for every recipe that names a hidden item anywhere and for every
recipe advancement that grants only dropped recipes, and removes the items from the creative tab file the
previous step wrote. `drop_items: false` keeps the item declarations registered so saved worlds and the item tags
`minecraft:boats` and `minecraft:chest_boats` that name them still load. `--check` exits 1 on a leftover.

Real data: 32 items (16 boats and 16 chest boats), 32 recipes and 32 recipe advancements. The diagnosis counted 33
because `oak_boat.json` has `boat` in its name; it crafts the vanilla `minecraft:oak_boat` from Regions
Unexplored planks, so it works and is kept.

## Collision on the real data (extra scope)

The extra question was how many carried blocks have an empty or wrong collision. Dry run over all 45 mods
(`block_looks.py --dry-run --report`), 9289 blocks:

| Result | Blocks | Meaning |
| --- | --- | --- |
| Full-cube template, smaller model ("oversized") | 971 | the template's full cube collision around a small model: a wall around a plush, a chair, a carpet, a path tile |
| Full-cube template, plant model | 142 | a flower or sapling that is a wall instead of walk-through |
| No-collision template, solid-looking model | 0 | would be walk-through blocks that look solid; none remain after the transparency and per-model rules (a first version flagged 24: 11 Chipped vines, 12 holiday ornaments, 1 duskmelon, all cutout or per-face models) |
| Geometry-less (geo/entity) models | 113 | occlusion fixed, collision stays the template's because the shape is unknown |
| State-dependent templates | 1170 | not evaluated; 73 of them are slabs whose model height is not half a block (mcwpaths 52, dyedend 16, mcwholidays 4, suppsquared 1) |
| Unresolvable models | 743 | left alone (Regions Unexplored 412, Chipped 245, luphieclutteredmod 83, candlelight 2, suppsquared 1) |

Occlusion: 1604 blocks get `occludes: false` (the 971 oversized, the 142 plants, 113 geo, the transparent cubes
and the remaining small models).

By mod (only mods with a change; the numbers are blocks in that mod):

| Mod | Blocks | occludes false | oversized | plant | geo | slab mismatch | unknown |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| chipped | 6936 | 436 | 328 | 108 | 0 | 0 | 245 |
| luphieclutteredmod | 509 | 358 | 164 | 0 | 0 | 0 | 83 |
| mcwholidays | 308 | 225 | 80 | 15 | 0 | 4 | 0 |
| mcwpaths | 258 | 154 | 154 | 0 | 0 | 52 | 0 |
| regions_unexplored | 805 | 157 | 41 | 18 | 88 | 0 | 412 |
| justoutdoorstuffs | 50 | 48 | 24 | 0 | 0 | 0 | 0 |
| dustydecorations | 79 | 77 | 62 | 0 | 0 | 0 | 0 |
| perfectplushies | 47 | 47 | 33 | 0 | 14 | 0 | 0 |
| froglightsreimagined | 48 | 32 | 32 | 0 | 0 | 0 | 0 |
| candlelight | 71 | 19 | 13 | 1 | 3 | 0 | 2 |
| alcocraftplus | 14 | 14 | 13 | 0 | 0 | 0 | 0 |
| bakery | 19 | 14 | 11 | 0 | 2 | 0 | 0 |
| betterendrod | 8 | 8 | 8 | 0 | 0 | 0 | 0 |
| fairylights | 21 | 7 | 1 | 0 | 6 | 0 | 0 |
| blahaj | 3 | 3 | 3 | 0 | 0 | 0 | 0 |
| suppsquared | 26 | 3 | 3 | 0 | 0 | 1 | 1 |
| beautify | 2 | 2 | 1 | 0 | 0 | 0 | 0 |
| dyedend | 64 | 0 | 0 | 0 | 0 | 16 | 0 |

The diagnosis estimated about 1970 affected blocks; the tool, which resolves models properly, finds 1604 for
occlusion. The remaining gap is the 743 models it cannot read.

**What this does and does not explain.** The pose in the screenshot (player lying inside four flat tiles, two brown
and two yellow, on a stone floor) is consistent with a wrong collision of the flat blocks of `mcwpaths` or
`luphieclutteredmod`/`chipped` carpets: a flat tile on a full-cube template is a full-height wall, and on a slab
template (52 mcwpaths blocks) a half-block step under a 1/16 tile. The first is fixed by `block_looks.py`; the second
(slab-mismatch) is only counted because a slab's collision depends on its `type`. Whether those exact blocks are
the ones in the screenshot is not established: the item names are not visible. A quick check in the game is to hover
the hotbar items and compare their ids with `block_looks.py --dry-run --report`.

## Acceptance

`Test-HomesteadFixes2.ps1` (prefix `stage28-fixes2-`, switch `-Fixes2`):

1. unpacks every packed data-mod archive (the bundled python has no `zlib` and cannot open deflated archives) and
   runs the three `--check` modes over the folders: no leftover occlusion or collision problem, no singular
   structure/biome key, no hidden boat in a creative tab or recipe;
2. in the game: `advancement revoke @s everything`, waits 30 seconds, counts the advancement messages and fails if
   more than eight arrive or any of the 20 known structure/biome titles appears. If the client has no
   `advancement` command the check is recorded as a note ("not verified"), not as a pass.

The tools have 66 unit tests (`test_advancement_fix.py`, `test_block_looks.py`, `test_hide_unplaceable.py`) with
samples cut from the real files (`ancient_cities`, `pioneer`, `on_sword_get`, a plush model with rotated elements),
checks over the real `pack/mods` data when it is on disk, and an end-to-end run (overlay on the real
`perfectplushies` carry: 80 violations before, 0 after). A full simulation of the pack chain (creative tab ->
`hide_unplaceable` -> `advancement_fix` -> `block_looks` -> overlay merged over the carry minus the drops -> the
three `--check` modes) over the 39 carried mods that have a `data` folder ended with 0 violations (about 7 minutes).

## Not verified

- Nothing was rendered or played: the hole under the plush, the collision of the repaired blocks and the absence
  of the advancement flood are checked in data, not on screen.
- The effect on an existing world: advancements already granted stay granted; use a new world or one revoke.
- Whether Horizon accepts `collision_shapes` on every one of these templates (the documented format is used, a
  load error would reject the mod). `--no-collision` is the fallback.
- The 743 unresolvable models and the 1170 state-dependent blocks keep their template's behaviour.
- Triggers outside minecraft (`yungsapi:structure_location`, 9 files) were not changed and were not checked in 26.3.
- Boats: hiding them loses the Regions Unexplored boat content; real boats need an entity layer in Horizon (a
  vanilla-boat alias would be an engine patch, see the diagnosis, option 2).
