# Farmer's Delight Skillet (Homestead)

The Skillet block follows `SkilletBlock`, `SkilletBlockEntity` and `HeatableBlockEntity` of
vectorwing/FarmersDelight 1.20, read from the GitHub source (Refabricated 2.4.1 itself was not
diffed). The content mod carries the blockstate, models and sounds; `fd/skillet.py` declares the block,
the item, a plain loot table and the recipe; `mixins/homestead/skillet.rs` (module
`homestead_skillet`) is the behaviour.

## Behaviour

* The item is an iron-tier weapon (250 uses, attack 5.0 + 2.0, speed -3.1, knockback +1) and a block
  item (`place_block`). It places on any block; the block has no support requirement.
* The pan holds one stack. A food with a `minecraft:campfire_cooking` recipe goes in with a right click
  (as much as fits; the hand keeps the rest). The cook time is `getSkilletCookingTime(recipe time)`:
  a fifth of the recipe's whole seconds, at least 60 ticks (600-tick recipes cook in 120 ticks).
* Heated (`#farmersdelight:heat_sources` below, lit if it has `lit`, or a `heat_conductors` block above a
  source), the cook time rises by 1 a tick; cold, it falls by 2. At the total, one item becomes the
  recipe result and leaves the pan with velocity (clockwise facing x 0.08, 0.25).
* Empty-hand right click takes the whole stack out (nothing for a creative player, as in FD).
  A non-cookable item says "This can't be cooked..." and the click passes on.
* `support` is true when a tray heat source (lava, campfires, fire, braziers) lies below; `facing` is
  the way the placer looks.
* Breaking the block drops the stack; the item is drawn lying in the pan.

NBT: `Item` (the stack, with count), `CookTime`, `CookTimeTotal` (ints). Clients get `Item`.

## Gaps and unverified

* Hand-held cooking (`SkilletItem.use`: off-hand food, flipping, the `cooking` item model, the use
  animation) is not implemented.
* Fire Aspect (faster cooking, enchanted hit particles) and the `Skillet` stack stored in the block
  entity (`copy_skillet` loot function, pick-block) are missing; the dropped item is a fresh skillet.
* Waterlogging, the tray collision slab below the block (y -1..0), steam particles and the
  `SKILLET_EXCLUDED`-style item rules are missing. Cooking in water is therefore not refused.
* Unverified numbers: hardness 0.5 / resistance 6.0 / lantern sounds, the sizzle rate, the draw
  transform, the 64-item pan capacity, `attack_knockback` as an item attribute.
* A block removed without being destroyed (`setblock air`, pistons) does not drop the contents.
* `TRAY_HEAT_SOURCES` is a copy of the tag inside the placement hooks, which see no tags.
