# Batch 1: data mods (stage 27)

Eight Homestead mods whose effect is content or a data tweak, packed without any client change:
Expanded Delight, Farmer's Knives, Mo Glass, Perfect Homestead Plushies, Spud's Shops, Go Fish,
Nether's Delight (data half) and SparseStructures. Source of the plan: `stage4/research/logic-machines-triage.md`.

Nothing was built or run while this was written. Everything marked **not verified** needs one run of
`stage4/Build-Batch1.ps1` and `Run-Acceptance.ps1 -Batch1`.

## How it works

- `stage4/Build-Batch1.ps1` runs `horizon-port-pack carry` on the player's own jars (their paths are in
  `pack/inventory.json`; the jars are not copied anywhere else). It uses its own work dir
  `work/homestead-horizon/pack-batch1`, so `pack/mods` (the 45 data mods) and their acceptance suite are untouched.
- `tools/batch1_content.py overlay` then prunes what the port cannot honour and adds the item components the Java
  mods set in code. `tools/creative_tab.py` adds the creative tab, and one flat archive per mod is written to
  `stage4/extra-mods/<id>-<ver>-batch1.1.hmod`.
- `batch1-policy.json` holds the per-mod policy (licence label, `publicRelease`, items to drop, estimate switches).
- Farmer's Delight cooking and cutting recipes of these mods need no code: the native pot and board read
  every `farmersdelight:cooking` / `farmersdelight:cutting` recipe of any namespace
  (`native_recipes_of_type`), so Expanded Delight and Nether's Delight meals cook in the pot we already ship.
- **Licence handling.** `publicRelease: false` marks content mods that contain assets from the player's own jar
  under All Rights Reserved / GPL / LGPL labels (Mo Glass, Perfect Homestead Plushies, Nether's Delight). They are
  for this machine and its private package only; release archives must filter on that flag. The licence labels are
  the Modrinth/jar metadata, not legal advice, and none of the carried assets was checked file by file.

## Estimates, stated once

Food values (nutrition, saturation), knife and machete stats (attack, durability, speed, enchantability) added by
the overlay are **estimates** shaped like Farmer's Delight's own numbers. The mods' Java code was not read
(no sources here; decompiling was out of scope). Nothing the mods set in code beyond these numbers is modelled:
no status effects on foods, no special tooltips, no repair rules except the vanilla material noted in the table
of `tools/batch1_content.py`.

## Per mod

### Expanded Delight 0.3.2 (MIT, Farmer's Delight add-on)

- **Done.** Carried: 17 blockstates, 55 item models, 72 recipes (10 `farmersdelight:cooking`, 6
  `farmersdelight:cutting`, shaped/shapeless, smelting, smoking, campfire), 17 loot tables, tags, worldgen files,
  lang. Overlay: food components for the 33 foods/drinks listed in `FOODS['expandeddelight']` (bowls and bottles
  keep their container as `use_remainder`, stack size 16).
- **Differs.** Recipes of type `botanypots:crop` and `expandeddelight:juicing` are dropped (no consumer: the
  juicer block and Botany Pots are not in the port); the items `juicer_item` and `mortar_and_pestle_item` are dropped
  with them. Recipes that need items of namespaces that are not in the port are pruned (listed in
  `pack-batch1/overlay-expandeddelight.json`). `create:*` recipes stay and are judged by the loader.
  Wild crops and salt ore generation (14 worldgen files) load only if the loader accepts those formats.
- **Not verified.** That the carry result loads (item/block counts), that tag-based pot ingredients
  (`c:cheese`, `c:foods/...`) match in the native pot, effects of foods, the asparagus/chili/peanut/sweet-potato
  crops growing (block behaviour is not ported: they are shells like the other carried crops).

### Farmer's Knives 3.2 (MIT)

- **Done.** 60 knife item models, 60 recipes. A recipe stays only if every `item` it names exists in the port
  (vanilla and carried namespaces); the knife of a dropped recipe is dropped with it (60 knives are made of ingots
  of other mods, most of which are not in Homestead). Kept knives get the same components as Farmer's Delight's
  knives (`#farmersdelight:mineable/knife` tool rule, attack 0.5 + tier bonus, speed -2.0).
- **Differs.** Tier numbers are estimates matched to the closest vanilla material by name
  (`KNIFE_TIERS`; unknown material = 250 uses, bonus 2). `fabric:load_conditions` are ignored: pruning is by
  item availability instead.
- **Not verified.** Which knives survive (depends on carried namespaces), cutting-board use with them (the board
  reads the `c:tools/knives` / FD knife tag: the mod adds 2 tags, not checked that they reach the board),
  repair rules.

### Mo Glass 1.7 (GPL-3.0-or-later; `publicRelease: false`)

- **Done.** 36 glass blocks with blockstates, 36 self-drop loot tables, 72 recipes, 113 tag files
  (`c:glass_blocks`, `c:glass_panes`, ...).
- **Differs.** No behaviour is added or needed. The mod's code was not used.
- **Not verified.** Connected-texture/pane models (the loader may reject some), the tag membership check in the
  suite covers one block, translucency/render layer of the glass blocks (the carried models decide).

### Perfect Homestead Plushies 1.1.0 (ARR; `publicRelease: false`)

- **Done.** 53 plush blocks with blockstates, models, loot tables, lang.
- **Differs.** No recipes in the jar (the plushies come from the mod's trader/loot config in Java, not ported).
  Some models use GeckoLib geo/animation files (5 geo, 2 animations); the port has no GeckoLib, so those
  plushies fall back to whatever block model the carry resolves, or are rejected.
- **Not verified.** How the plushies look, how many of the 53 place. There is no way to obtain them in
  survival (no recipe, no trader); they are creative-tab items.

### Spud's Shops 1.9.0 (MIT)

- **Done.** 43 blocks, 211 items, 209 recipes, 42 loot tables, 213 advancements (carried as data).
- **Differs.** Assets live under the namespace `spudaciousshops` while data and the mod id are `spuds_shops`;
  whether the carry joins them correctly is unknown. Any shop behaviour (trading, selling) is Java and not ported.
  The 213 recipe-unlock advancements need no code but only matter if the game shows the recipe book.
- **Not verified.** Everything about loading (two namespaces), block placement, whether the items show textures.

### Go Fish 1.6.3 (CC0-1.0)

- **Done.** Items (78), crates (blocks with loot tables), recipes (37), tags (`minecraft:fishes`, nourish tags,
  biome tags).
- **Not ported (the point of the mod).** Catching the new fish is Java: `GoFishLoot` injects entries into the
  fishing loot, custom loot conditions (`BiomeLoot`, `Weather`, `FullMoon`), bobber mixins (lava fishing in the
  Nether, fishing in the End, auto-smelt, fireproof bobbers), enchantments and an astral crate renderer. None of
  that is data, so the fish and crates exist in the creative tab but no cast produces them. Not guessed: no
  invented loot table was written. A future slice needs a fishing-loot injection hook.
- **Not verified.** Whether the carried crate loot tables load at all (their custom conditions may be rejected).

### Nether's Delight Refabricated 4.1.1 (LGPL v2.1; `publicRelease: false`)

- **Done (data half).** Foods, meats, plates, skewers, machetes, hoglin trophy, 3 cooking and 5 cutting recipes,
  crafting/smelting recipes, tags. Overlay: food components for 14 items, estimated sword-like components for
  the 4 machetes.
- **Dropped on purpose** (behaviour needed): `blackstone_furnace`, `blackstone_blast_furnace`,
  `blackstone_stove`, `nether_brick_smoker` (furnace/stove behaviour), `crimson_fungus_colony`,
  `warped_fungus_colony`, `propelplant_cane/stem/torch` (growth), `rich_soul_soil`, `soul_compost`, `mimicarnation`.
  Recipes that produce them are pruned with them. The second half of this mod (stove-like blocks, colonies,
  soils) is a later slice.
- **Not verified.** Machete behaviour (the original adds a tool action that harvests crops and cuts; only the sword
  numbers are estimated here), food effects, whether `nethersdelight:raw_strider` and other tags used by recipes
  resolve.

### SparseStructures 3.0 (MIT)

- **Done.** The jar holds code and config only (no data), so it ships as the data pack
  `datapacks/homestead-sparse`: for every `random_spread` structure set found in the player's own prepared
  vanilla resources and in the carried mods (`pack/mods/*/carry`), spacing and separation are multiplied by the
  factor from the Homestead config (`spreadFactor` 1.2; custom `minecraft:mansion` x2, which replaces the global
  factor for sets that contain that structure). Spacing is rounded, at least 2, separation is kept below spacing.
  Dry run here: 82 sets seen, 78 changed (strongholds, nether fossils and two mod sets use other placements and
  are left alone).
- **Differs.** The original applies its factor at runtime to every structure set of every loaded mod; the data
  pack can only cover the sets present when it is built. Whether the custom factor replaces or multiplies the
  global one in the original is not documented in the config comments; the pack replaces it (stated assumption).
  Mod structure sets from mods added later are not covered until the pack is rebuilt.
- **Not verified.** Any effect on world generation: data packs from `new-world-datapacks` apply to **newly created
  worlds only**; the acceptance world already exists, so the suite checks the generated files, not generation.
  `Start-Homestead.ps1` must also install every data pack folder (see `fragments/batch1.md`, section 4).

## Not part of this batch

Obsidian Equipment Reworked, Create Deco, CraftTweaker and the rest of the data-only list in the triage report
were left for a later pass (they need a material check, a licence check, or are covered by other work).
