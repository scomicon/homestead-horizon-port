# Decorative and material content (data only)

`tools/decor_content.py` declares 350 Create 6.0.10 decorative blocks from the table
`tools/decor_catalog.json`. Every block is a vanilla-template block (`data/create/horizon/block`),
so no Rust logic is involved. Nothing is wired yet: see "How to wire it".

## Ported (350 blocks, 350 block items, 584 recipes, 45 tag files, 350 lang names)

| Group | Blocks | Template (behaviour class) |
| --- | --- | --- |
| Stone palettes | 246 | `stone_slab`, `stone_stairs`, `cobblestone_wall`, `basalt` (pillars, `axis`), the stone's own block for cubes |
| Copper tiles and shingles | 48 | `waxed_cut_copper` (+ `_slab`, `_stairs`): 4 stages x waxed/unwaxed x (block, slab, stairs) x 2 |
| Windows and panes | 28 | `glass`, `glass_pane` (11 wooden, industrial, weathered and ornate iron) |
| Glass | 8 | tiled, framed, horizontal and vertical framed glass + panes |
| Metal decor | 9 | `iron_bars`, `ladder`, `scaffolding` in andesite, brass and copper |
| Casings | 3 | railway, shadow steel, refined radiance (light 12) |
| Rose quartz | 3 | rose quartz block (axis), rose quartz tiles, small rose quartz tiles |
| Misc | 3 | cardboard, bound cardboard (horizontal `axis`), experience block (light 15) |
| Trapdoors | 2 | framed glass and train trapdoor (`oak_trapdoor`, hand-openable) |

Stone types (all 14 of `AllPaletteStoneTypes`): andesite, diorite, granite, calcite, dripstone, deepslate,
tuff (vanilla bases, no own base block) and asurine, crimsite, limestone, ochrum, scoria, scorchia,
veridium (own base block). Per type: cut, polished cut, cut bricks, small bricks, each with stairs, slab and
wall; plus pillar (`axis`) and layered. The 13 blocks `catalog.py` already declares (cut, polished, bricks and
small bricks of andesite, granite and diorite, and `layered_andesite`) are excluded; the table adds their
stairs, slabs, walls and pillars. IDs equal the original (`create:cut_limestone_brick_stairs`, ...).

Per block the generator writes: block declaration, blockstate, models, item model, item definition, loot table
(self, slab double count, silk touch only, or bound cardboard), lang name. Shared output: recipes under
`data/create/recipe/palettes/`, and tags. Recipes (219 shaped, 73 shapeless, 284 stonecutting, 8 smelting of framed
glass back to glass) and tag membership are derived from the pinned Create recipe/tag data and converted to the
26.x format (string ingredients, `{id, count}` results). Stonecutting uses `#create:stone_types/<stone>` tags.
New tag files include `c:glass_blocks/colorless`, `c:glass_panes/colorless` (the original `colorless_glass`
tags), `minecraft:slabs|stairs|walls|trapdoors|climbable|impermeable`, `create:casing`, `create:fan_transparent`,
`create:wrench_pickup`, the stone-type tags and the mineable/needs-tool tags.

Visuals are vanilla-textured stand-ins (a glass pane for the pane models, vanilla stair/slab/wall/trapdoor/ladder
models, colored terracotta or concrete for the colored stones). The player's JAR replaces them: every id is in the
JAR, so adding the ids to `catalog.BLOCKS` makes the importer read the original blockstates and models.
Connected-texture behaviour (framed glass, windows, shingles) is a Java model loader and is **not** reproduced.

## Skipped, with reason

- Block entity or entity behaviour: all doors (`andesite_door`, `brass_door`, `copper_door`, `train_door`,
  `framed_glass_door`: sliding door BE and contraption movement), seats (16, seat entity), valve handles (17,
  kinetic BE), nixie tubes (17, BE), table cloths (19), toolboxes (16), postboxes (16), `placard`, `metal_bracket`,
  `wooden_bracket`, `metal_girder`, `metal_girder_encased_shaft`, `rose_quartz_lamp` (redstone toggling),
  copycat blocks, `display_board`, `desk_bell`, bells, cuckoo clocks.
- `honey` and `chocolate` (fluids), `chain_conveyor`, and every kinetic or logistics machine (other subsystems).
- Sails and the already declared casings, iron blocks and zinc blocks.

## Known gaps (not verified in the game)

- **Copper weathering and waxing do not work.** Unwaxed stages are plain blocks (they use the waxed templates so
  vanilla does not turn them into `minecraft:*cut_copper`). Oxidation, honeycomb waxing, axe scraping and the
  deployer/Create waxing recipes need a native hook.
- Cardboard and bound cardboard: the item burn time (4000) and "ignited by lava" are not expressible; the template is
  a bookshelf with the horizontal `axis` state declared. Bound cardboard drop is converted from the original loot.
- Properties not read from Java (the sources I could reach were truncated): bars, ladder, scaffolding, window,
  casing, rose quartz, cardboard, trapdoor hardness, resistance and sound are best estimates of the base blocks
  (iron bars, ladder, scaffolding, glass, andesite, bookshelf, oak trapdoor with overrides). Stone, copper and
  experience values are copied from `AllPaletteStoneTypes` and the original registration. Compare in game.
- `minecraft:scaffolding` template: the vanilla stability and placement logic may only recognise vanilla
  scaffolding, and the scaffolding item may lack the extend-by-placing behaviour. Test before shipping.
- Trapdoors use `oak_trapdoor` so they open by hand; the wood sound set and flammability come with it.
- Windows have no silk-touch-only explosion behaviour beyond the loot table; none of the original connected
  textures exist without the JAR.
- Recipes use `minecraft:black_dye` where the original used the black-dye tag, and `c:ingots/brass|copper`
  (present in the reference tags).
- External ingredients the recipes need from other subsystems (must exist as items): `andesite_alloy`, `cardboard`
  (not declared yet, so the cardboard recipes do nothing until it exists), `rose_quartz`, `polished_rose_quartz`,
  `experience_nugget`, `industrial_iron_block`, `weathered_iron_block`.
- The validation JAR is Create 6.0.8.1 Fabric 1.20.1, not the pinned 6.0.10 NeoForge 1.21.1 JAR. Names, blockstates
  and models were checked against it only; re-run the check with the pinned JAR.
- Three textures (`tuff_bricks`, `polished_tuff`, `chiseled_tuff`) do not exist in the 1.20.1 vanilla jar used
  for checking; they do in the target version (`tuff_bricks` is already used by `content.py`).
- Russian names are not provided (the game falls back to English). The `ru_ru` locale needs its own table.

## Validation result (scratch run)

`validate_decor.py` (kept outside the repo, in `%TEMP%\decor-test`) runs `declare()` with a recording `write`:
3219 files, all valid JSON; 350 block declarations, 350 blockstates, 840 block models, 350 item models, 350 item
definitions, 350 loot tables, 584 recipes, 45 tag files. Every `create:` model reference resolves inside the output;
the 12 references to `create:block/cut_<andesite|granite|diorite>...` (double slabs of the pre-declared cubes) resolve
to models `content.py` already writes. All 24 vanilla parent models and 87 vanilla textures exist in the
1.20.1 jar except the three newer tuff textures above. For all 350 ids the JAR has the blockstate, item model, parent
models and textures, with no custom model loader. No id overlaps `catalog.py`. No JAR file was copied into the repo.

## How to wire it

`tools/catalog.py` (after the `PALETTE_BLOCKS = ...` line, before `BLOCKS = ...`):

```python
from decor_content import DECOR_BLOCKS
PALETTE_BLOCKS += DECOR_BLOCKS
```

`tools/content.py`:

```python
# imports
from decor_content import declare as declare_decor, lang_en as decor_lang, pickaxe_values as decor_pickaxe

# foundation(), right after `vault_recipe(write)` (it must run after the recipe and tag cleanup loop):
declare_decor(write, model, reference['tags'])

# foundation(), the mineable/pickaxe write: replace `*pickaxe['values']` with
*decor_pickaxe(reference['tags'])

# before the locale loop (next to extra_blocks[ITEM_VAULT] = 'Item Vault'):
extra_blocks.update({key.removeprefix('block.create.'): title for key, title in decor_lang().items()})
```

`tools/fallback_textures.py` (so the generic fallback retargeting leaves these vanilla-textured models alone):

```python
from decor_content import OWNED_MODEL_PATHS
# first line of retarget(), after `prefix = 'assets/create/models/'`:
if path in OWNED_MODEL_PATHS:
    return
```

Then run `python tools/content.py` (regenerates `assets/src/catalog.rs` through `rust_catalog()` and the
`palettes` creative tab, which lists `PALETTE_BLOCKS`). No change to `lib.rs` or `horizon-mixin.toml` is needed.
`tests.rs` and `block_catalog_checks.py` were only grepped for hard-coded counts and found none; run them.
