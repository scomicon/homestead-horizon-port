# Batch 2: data mods (stage 23)

Six Homestead mods whose effect in this slice is content or data, packed without any client Rust
change: Ocean's Delight, Crate Delight, Obsidian Equipment Reworked, Minecraft Accelerated,
Create Deco and Sooty Chimneys.

Nothing was built, compiled or run in the game while this was written. Everything marked **not verified**
needs one run of `stage4/Build-DataMods-batch2.ps1` and `stage4/Run-Acceptance.ps1 -Batch2`.

## How it works

- `stage4/Build-DataMods-batch2.ps1` runs `horizon-port-pack carry` on the player's own jars
  (paths from `pack/inventory.json`; no third-party jars or assets are copied into the repo). It uses
  its own work directory `work/homestead-horizon/pack-batch2`, leaving `pack/mods` (the 45 stage17 mods)
  and `pack-batch1` untouched.
- `tools/batch2_content.py overlay` prunes recipes that cannot load (blocked recipe types, unsatisfied
  `fabric:load_conditions` / `neoforge:conditions`, or items from namespaces absent from the port), drops
  uncraftable items when configured, and adds component/block metadata that the Java mods set in code.
- `tools/creative_tab.py`, `tools/hide_unplaceable.py`, `tools/advancement_fix.py` and
  `tools/block_looks.py` add creative tabs and stage22 playtest fixes before writing one flat archive
  per mod to `stage4/extra-mods/<id>-<ver>-batch2.1.hmod`.
- `stage4/batch2-policy.json` records per-mod licence metadata, `publicRelease` flags, pruning rules
  and overlay switches.

## Estimates, stated once

Food values (nutrition, saturation) for Ocean's Delight and durability/damage/speed/armor/toughness/
enchantability values for Obsidian Equipment Reworked added by `tools/batch2_content.py` are **estimates**
shaped like Farmer's Delight and vanilla diamond/netherite tiers. None of the mods' Java code was read or
decompiled. No food status effects (such as raw pufferfish poison or Soup comfort/regeneration) or custom
tool hit effects are modelled.

## Per mod

### Ocean's Delight `fdrf-fabric-1.0.2-1.20` (MIT / CC0-1.0; Farmer's Delight add-on)

- **Done.** Carried: 22 item models/items, 51 recipes (`farmersdelight:cooking`, `farmersdelight:cutting`,
  `smelting`, `smoking`, `campfire_cooking`, `crafting_shapeless`), 5 tag files. Overlay: estimated food
  and consumable components for 20 food items (`squid_rings`, `seagrass_salad`, `braised_sea_pickle`,
  `bowl_of_guardian_soup` return `minecraft:bowl` with max stack size 16), plus the `guardian_soup`
  feast block shell (`facing` + `servings=0..4`, `occludes: false`) and placeable item so the cooking
  recipe has a valid registered result.
- **Differs.** Entity loot table injections (`data/farmersdelight/loot_tables/inject/entities/squid.json`,
  `guardian.json`, `elder_guardian.json`) do not inject into vanilla mob drops without a loot-injection
  hook. `guardian_soup` places as a decorative feast shell; right-click bowl serving is not wired into
  `feast.rs` in this data-only slice. Food status effects are omitted.
- **Not verified.** Carry output counts, whether `c:dough`, `c:eggs`, `c:crops/onion` and `c:crops/tomato`
  tags match in the native pot, and cooking/cutting execution in the running client.

### Crate Delight `26.07.01-1.20-fabric` (MIT)

- **Done.** Carried: up to 30 storage crate and bag blocks (`apple_crate`, `carrot_crate`, `potato_crate`,
  `beetroot_crate`, `egg_crate`, `cod_crate`, `salmon_crate`, `berry_crate`, `glowberry_crate`,
  `cocoabeans_bag`, `sugar_bag`, `gunpowder_bag`, `peanut_crate`, `salt_bag`, `ground_cinnamon_bag`,
  etc.), 124 recipes across `cratedelight` and `expandeddelight` recipe folders, 29 block loot tables,
  `mineable/axe` tag. Overlay scans all `data/*/recipe*` directories inside the carry output, drops
  recipes whose `fabric:load_conditions` or item namespaces are absent from the port, and prunes
  uncraftable crates together with their unpacking recipes.
- **Differs.** Crates whose source mod is not in the port (such as `blueberry_crate`, `duck_egg_crate`,
  `catfish_crate`, `bass_crate`, `end_fish_crate`, `emu_egg_crate`) are pruned when their crafting recipe
  is unavailable; Expanded Delight crates stay only if `pack-batch1` is present when building batch2.
- **Not verified.** Exact surviving crate count after `carry` + `overlay`, tag membership in game, and
  block placement/mining.

### Obsidian Equipment Reworked `1.1` (CC0-1.0 / MIT)

- **Done.** Carried: 14 items (`obsidian_ingot`, `obsidian_upgrade_smithing_template`, `obsidian_sword`,
  `obsidian_pickaxe`, `obsidian_axe`, `obsidian_shovel`, `obsidian_hoe`, `obsidian_helmet`,
  `obsidian_chestplate`, `obsidian_leggings`, `obsidian_boots`, `obsidian_horse_armor`,
  `obsidian_nautilus_armor`, `obsidian_spear`), smelting/blasting/crafting/smithing recipes, equipment
  layer textures and item tags. Overlay: estimated armor (`minecraft:equippable`, `minecraft:armor`,
  `minecraft:armor_toughness`, `minecraft:knockback_resistance`, `minecraft:repairable`) and tool/weapon
  components, plus dropping the spurious `obsidian_spear_in_hand` model entry if generated as an item.
- **Differs.** Vanilla `data/minecraft/recipe/netherite_*_smithing.json` overrides shipped by the mod are
  dropped by policy (`drop_vanilla_recipe_overrides: true`) so vanilla diamond-to-netherite smithing
  remains available while survival chest loot for `obsidian_upgrade_smithing_template` is unverified.
  Any special spear/nautilus-armor code behaviour beyond data components is not ported.
- **Not verified.** Rendering of `obsidianequipmentrework:obsidian` armor/horse/nautilus layers, smithing
  recipes in the smithing table, tool mining speeds and survival acquisition of the upgrade template.

### Minecraft Accelerated `1.0.0` (CC0-1.0)

- **Done.** Carried: 28 carved pumpkin, jack o'lantern and soul jack o'lantern blocks (`*_check`,
  `*_creeper`, `*_derp`, `*_evil`, `*_heart`, `*_herobrine`, `*_shout`, `*_smile`, `*_x`, plus
  `soul_jack_o_lantern_normal`), 39 shaped crafting recipes, 28 self-drop loot tables, tags. Overlay sets
  `properties.light: 15` on `jack_o_lantern_*` and `properties.light: 10` on `soul_jack_o_lantern_*`.
- **Differs.** No code behaviour needed; blocks act as decorative directional blocks with light emission.
- **Not verified.** In-game light levels, textures and placement orientation.

### Create Deco `2.1.1-1.20.1-fabric` (MIT on Modrinth; `fabric.mod.json` placeholder; `publicRelease: false`)

- **Done.** Carried: industrial Create-themed decorative blocks, items, recipes (`crafting_shaped`,
  `crafting_shapeless`, `stonecutting`, `smelting`, `blasting`, `create:pressing`), loot tables and tags.
  Overlay prunes recipes referencing unavailable namespaces while preserving `create:*` dependencies.
- **Differs.** Marked `publicRelease: false` conservatively because the upstream `fabric.mod.json` leaves
  `"Insert License Here"` even though Modrinth lists MIT. Custom block classes without matching Horizon
  templates are skipped by `carry`; interactive block behaviours (shipping container inventories, clickable
  decals, custom lamp redstone toggling beyond templates) are decorative shells only.
- **Not verified.** How many of the 397 blockstates `carry` maps to templates, whether Create's
  mechanical press picks up `create:pressing` recipes from `createdeco`, and visual models.

### Sooty Chimneys `1.3.4` (MIT)

- **Done.** Carried: 14 chimney blocks (7 clean and 7 `dirty_*` variants: brick, cobblestone,
  stone_brick, mud_brick, iron, copper, terracotta), crafting recipes, self-drop loot tables and tags.
  Overlay drops `sootychimneys:soot_scraping` recipes (`blocked_recipe_types`).
- **Differs.** Smoke particle emission, redstone smoke blocking, gradual soot accumulation and right-click
  soot scraping require client particle and block-entity hooks and are **not ported**. The 14 blocks ship
  as decorative building blocks only.
- **Not verified.** Block placement, `block_looks.py` collision/occlusion boxes and crafting recipes.

## Excluded candidates (checked and left out)

- **All Rights Reserved / custom-restricted licences:** `hearth_and_home` (`LicenseRef-Starfish-Studios-Mod-License` on Modrinth), `squish` (`LicenseRef-All-Rights-Reserved`), `twigs`, `serene_shrubbery`, `decorativelamps`, `fetzisasiandeco`, `amendments`, `supplementaries`, `handcrafted`, `vinery`, `meadow`, `beachparty`, `herbalbrews`, `wildernature`, `vanillabackport`.
- **Require code/hooks/engine patches:** `yungsbridges` (uses `yungsbridges:bridge` custom worldgen feature), `darkpaintings` (Horizon `PaintingVariants::load` only reads `data/minecraft/painting_variant`), `chefsdelight` (villager professions), `villagersplus` (villager trades/workstations), `cozystudioscore` (custom entities, kiln block entity, music discs), `tectonic` (Lithostitched/custom terrain), `clutter` / `mythicupgrades` / `hexalia` / `spelunkery` (heavy Java entities, biomes, effects and mechanics), `betterthanmending` (no data, pure mixin), `crafttweaker` (jar holds only example `.zs` scripts).

## Licence & public-repo audit (`publish-audit.md` & `pack/inventory.json`)

Checked all 7 files added in batch2 (`docs/batch2.md`, `mixins/homestead/fragments/batch2.{md,toml}`, `tools/batch2_content.py`, `stage4/Build-DataMods-batch2.ps1`, `stage4/acceptance/Test-Homestead-Batch2.ps1`, `stage4/batch2-policy.json`) against `stage4/research/publish-audit.md` and `pack/inventory.json`:

1. **Nothing extracted from third-party JARs enters public files.**
   - None of the 7 tracked files contain `.jar`, `.hmod`, textures, models, sounds, recipes, loot tables, lang files or decompiled Java code from any mod JAR.
   - `tools/batch2_content.py` holds only item/block identifier keys (`FOODS`, `BOWL_FOODS`, `OBSIDIAN_ARMOR`, `OBSIDIAN_MOUNT_ARMOR`, `OBSIDIAN_TOOLS`) with documented estimated stats and the `guardian_soup` block-shell shape mirroring `fd/feast.py`.
   - `stage4/Build-DataMods-batch2.ps1` reads the user's own JARs locally via `pack/inventory.json`, writes `carry`/overlay outputs to `work/homestead-horizon/pack-batch2/` and packs `.hmod` files into `stage4/extra-mods/` (both excluded from git and from the public repo whitelist).
2. **Metadata cross-check against `pack/inventory.json` (`license` / `info.license` from `fabric.mod.json`):**
   - `oceansdelight`: `inventory.json` records `MIT` (`batch2-policy.json` notes `MIT / CC0-1.0`; `publicRelease: true`).
   - `cratedelight`: `inventory.json` records `MIT` (`publicRelease: true`).
   - `obsidianequipmentrework`: `inventory.json` records `CC0-1.0` (`batch2-policy.json` notes `CC0-1.0 / MIT`; `publicRelease: true`).
   - `mcaccel`: `inventory.json` records `CC0-1.0` (`publicRelease: true`).
   - `createdeco`: `inventory.json` records `"Insert License Here"` (placeholder in `fabric.mod.json`), matching `publicRelease: false` in `batch2-policy.json` so any locally built `.hmod` stays excluded from release archives.
   - `sootychimneys`: `inventory.json` records `MIT` (`publicRelease: true`).
3. **Carried `.hmod` caveat (`publish-audit.md` §2.2, §6.1):**
   - Packed `.hmod` archives built locally in `stage4/extra-mods/` contain carried assets from the user's JARs and must never be committed to git or the public repository. For any future public release archive, `createdeco` must be excluded via `publicRelease: false` (or `-NoNonPublic`), and per `publish-audit.md`, asset licences inside MIT-labelled mod JARs (`oceansdelight`, `cratedelight`, `sootychimneys`) were not audited file-by-file against embedded `LICENSE` texts.
