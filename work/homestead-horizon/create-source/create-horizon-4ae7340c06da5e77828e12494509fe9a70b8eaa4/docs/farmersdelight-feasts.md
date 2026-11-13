# Farmer's Delight feasts, pies, drinks and Comfort

Source: vectorwing/FarmersDelight 1.20 (MIT), read through a summarising fetch, not a local
checkout. Files: `fd/feast.py` (content), `mixins/homestead/feast.rs`, `comfort.rs` (native).

## Ported

- **Feasts** (`FeastBlock`): roast chicken, stuffed pumpkin, honey glazed ham, shepherd's pie
  (`servings` 0..4) and the rice roll medley (`servings` 0..8), each with `facing` and an item that
  places it (stack size 1). The carried blockstates, models and loot tables are used as they are.
  - A click with servings left hands out the serving item. The four plated feasts need a Bowl in
    hand (it is used up outside creative mode); the medley needs nothing. Without the bowl an
    action-bar message appears and nothing is served.
  - The medley serves `servings - 1` of cod roll, cod roll, salmon roll x3, kelp roll slice x3.
  - A feast at 0 servings is broken on the next click and drops its loot-table leftovers: bowl
    (all), bone meal (chicken), bone (ham). The stuffed pumpkin has no leftovers: the block goes
    with the last serving.
  - The full block drops itself (carried loot tables).
- **Pies** (`PieBlock`): apple pie, chocolate pie, sweet berry cheesecake with `bites` 0..3 and a
  plain loot table. A knife cuts a slice item out (the fourth cut removes the pie); any other
  click eats a bite for a player who can eat (slice food value, speed effect), and the fourth
  bite removes the pie.
- **Vanilla cake** with a knife: `farmersdelight:cake_slice` pops out and the cake loses a bite
  (seven slices).
- **Crafting recipes** of the feasts and pies (generated 1.20 data).
- **Comfort** (`ComfortEffect`): every 80 ticks of remaining duration the player heals 1 HP when
  hurt, unless Regeneration is active or saturation is above 0. No FD food grants it in 1.20.
- **Drinks**: Milk Bottle, Hot Cocoa and Melon Juice are food-less drinkable items (1.6 s, drink
  animation, bottle back) with the effects below.

## Differences and unverified

- Recipes use items instead of Forge tags (milk bucket for `forge:milk`, `farmersdelight:onion`,
  `minecraft:egg`, `minecraft:carrot`, `minecraft:cooked_mutton`, `minecraft:wheat`); the stuffed
  pumpkin recipe is an estimate.
- Sounds are stand-ins (armor equip for the portion, wool for the slice); FD's own events were not
  read. No serving particles. No comparator output (`getAnalogOutputSignal`).
- Shapes are one box per block instead of the per-state shapes; the models are the carried ones.
- Pie eating takes the food value of the slice (3, 0.3) and the speed effect for 600 ticks; the
  `onItemUseFinish` event is not fired. The slice is not pushed away from the player's facing.
- The knife check is a list of the five FD knives (the original uses the knife tag); modded knives
  do not cut pies.
- Milk Bottle clears **all** effects (original: one random milk-curable effect), Hot Cocoa clears all
  of a fixed harmful list (original: one random harmful curable effect), Melon Juice gives
  Regeneration for 100 ticks (original: instant 2 HP heal). The consume-effect shapes are the
  vanilla data-driven ones.
- Comfort heals through `set_health`, without going through the damage/heal event of the engine.
- Hot Cocoa's and Melon Juice's tooltips and the "Comforting!" advancement trigger are untested.
- Not covered: Gleaming Salad block (no declaration in the carried data), pumpkin pie block (debug
  item in the original), dog food, the `farmersdelight.block.feast.use_container` text with the
  container name (a fixed Bowl message is used).
