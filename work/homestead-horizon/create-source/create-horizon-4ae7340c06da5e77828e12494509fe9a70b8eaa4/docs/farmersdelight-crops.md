# Farmer's Delight: crops, rich soil, compost

Farmer's Delight 2.4.1 (Refabricated, MIT). Content comes from `fd/crops.py` (declarations on top of the carried
FD data), behaviour from `mixins/homestead/crops.rs` (module `homestead_crops`). The Java was not read for this
slice: shapes of the blocks come from the carried blockstates and loot tables, the rates are estimates.

## What works

| Thing | Port |
| --- | --- |
| Cabbages, Onions | Wheat-template crops (age 0-7) planted by Cabbage Seeds / the Onion item (`place_block`), grown by the template's random tick and bone meal, harvested by a right click (Right Click Harvest rules), loot is the carried table (cabbage + seeds, onion + fortune bonus). |
| Budding Tomatoes, Tomatoes | Beetroot-template crops (age 0-3) planted by Tomato Seeds. A ripe bud turns into a tomato vine (age 0, `ropelogged=false`); a ripe vine gives 1-2 tomatoes (+ fortune, 5% rotten tomato) and seeds, a right click harvests and resets it. |
| Rice | Seagrass-template water plant (age 0-3, `supporting`), planted from the Rice item on the floor under a water cell, so it only grows in water. Random ticks add a stage (1 in 5, light 6); at stage 3 Rice Panicles (age 0-3, beetroot template) grow on top when that cell is air. A ripe panicle gives a Rice Panicle, or Rice with a knife (carried loot), and the straw modifier is carried. |
| Rich Soil | Dirt-template block. A random tick gives the crop above one more growth step (1 in 3, light 9) for the vanilla crops and the FD ones. A hoe on its top makes Rich Soil Farmland. |
| Rich Soil Farmland | 15/16 high dirt-template block, `moisture` 0-7 (always 7: it never dries and never turns to dirt). Added to `minecraft:supports_crops`, so every crop stands on it, and it boosts growth like rich soil. Drops Rich Soil. |
| Organic Compost | Dirt-template block with `composting` 0-7. A random tick raises the stage (1 in 3 with a compost activator in the 3x3x3 neighbourhood, else 1 in 8); from stage 7 the next step turns it into Rich Soil. Activators are the `compost_activators` tag spelled out (mushrooms, podzol, mycelium, rich soil, compost, mushroom colonies). |
| Wild Cabbages, Onions, Tomatoes, Rice | Fern / seagrass templates (instant break), carried loot; wild rice is one cell (`half=lower`) with its own loot (shears keep the plant, else one Rice). |
| Tags | `minecraft:supports_crops` + Rich Soil Farmland and Rice (the ground of the panicles); `minecraft:mineable/shovel` + the three soil blocks. |

## Not ported or different

- Tomato vines grow in place (no growing upward, no rope; `ropelogged` is always false).
- Rice panicles are a separate block above the rice, as in the original, but the rice supports them through the
  `supports_crops` tag instead of a dedicated rule; breaking the rice drops the panicles.
- Rich soil does not spread mushrooms; no `unaffected_by_rich_soil` check, so the boost also applies to the plants
  FD lists as unaffected when they have an `age` and a known maximum (only crops are known, so in practice it does not).
- Hoe only on rich soil (no shovel back, no tilled-soil sound, no advancement).
- No compost bin interaction: FD adds none, and the vanilla composter's FD compost values are not added.
- Wild crops are single plants; no two-block wild rice, no worldgen (the carried features exist as data and are not
  verified to generate).
- Village compost piles (carried structures) are not verified to appear.
- Rates and light levels are estimates.

## Unverified (never built or run)

See `mixins/homestead/fragments/crops.md`, section 8. Acceptance: `Run-Acceptance.ps1 -Crops`
(`acceptance/Test-HomesteadCrops.ps1`, 15 scenarios).
