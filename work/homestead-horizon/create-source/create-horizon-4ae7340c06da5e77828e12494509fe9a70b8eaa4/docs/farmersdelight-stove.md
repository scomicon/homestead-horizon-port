# Farmer's Delight Stove (homestead bundle)

Module `homestead_stove` (`mixins/homestead/stove.rs`). Written from the Farmer's Delight 2.x
`StoveBlock` / `StoveBlockEntity` as remembered; the Java source was not available to diff.

## Behaviour

- Six food slots. Right click with food that has a `minecraft:campfire_cooking` recipe puts one item in the
  first free slot (FD's `getNextEmptySlot`; the hit position is not used). The recipe time is the cooking
  total of that slot.
- Cooks only while `lit=true`: +1 per tick per slot; when it reaches the total the cooked item (recipe
  result) is spawned above the stove, offset one step clockwise of `facing`, with FD's velocity.
- Unlit stove: progress falls by 2 per tick (clamped to 0..total).
- Blocked above (the block above has a full bottom collision face): the food drops and progress resets.
- Lit by Flint and Steel (1 durability) or Fire Charge (consumed); extinguished by a shovel (1 durability)
  or a water bucket (becomes a bucket). Creative mode keeps tools and items. Sneaking skips all of it.
- Contents drop when the block is broken (`destroy_block` hook, so not for explosions).
- Items are drawn on the top face, two rows of three.

## NBT (block entity `farmersdelight:stove`)

`Item0`..`Item5` (item compounds), `CookingTimes` (int array 6), `CookingTotalTimes` (int array 6).
Clients get only `Item0`..`Item5`. Not the same layout as FD's own save (`Inventory` handler), so FD
worlds do not carry their stove contents over.

## Not done / unverified

- FD's `stepOn` damage to entities on a lit stove, smoke/flame particles, and ignition by burning projectiles.
- Exact FD slot offsets/rotation in the renderer, flint-and-steel pitch randomness (game time stands in).
- Rain does not extinguish (believed to match FD).
- Explosions or pistons do not drop the contents.
