# Farmer's Delight Cooking Pot (with menu)

Module `homestead_cooking_pot` (`mixins/homestead/cooking_pot.rs`), written from the 1.20 source of
vectorwing/FarmersDelight (`CookingPotBlock`, `CookingPotBlockEntity`, `CookingPotMenu`,
`CookingPotScreen`, read from GitHub raw files through a summarizer, not diffed). The menu type is declared in
`data/farmersdelight/horizon/menu/cooking_pot.json` (written by `fd/cooking_pot.py`).

## Storage
- The nine slots live in the engine's `ModBlockEntity.items` (0-5 ingredients, 6 meal display, 7 container, 8 output),
  declared by `native_block_entity_container` (`menu: farmersdelight:cooking_pot`, no faces/inputs/outputs, so
  hoppers cannot reach it). The engine saves them as `Items`, drops them on break and syncs them to the open menu.
- `CookTime`, `CookTimeTotal`, `Container` (container stored when the meal was made) stay in NBT.
- Menu data slots = `entity.data`: [0] cook time, [1] cook time total, [2] heated (0/1; an addition for the flame).
- Migration: an old `Inventory` NBT list is moved into `items` on the first tick (also on serve and on break) and removed.
- Update tag: `Meal` (the meal-slot item, an empty compound for none) for the in-world draw on clients.

## Flow
- Tick: cooking as before; then `moveMealToOutput` (meal without a container goes to slot 8) or
  `useStoredContainersOnMeal` (container in slot 7 matches the meal's container: as many servings as the container
  count, the meal count and the room in slot 8 allow, one container used per serving; FD has no per-tick cap).
- Right click: only serving with the matching container in hand is native (one portion, container used up).
  Every other click returns false so the engine opens the menu. The old direct ingredient insertion is gone.
- Menu: ingredient slots take anything, slot 7 takes anything, slot 8 is take-only (`results`). The meal display slot
  is left out of the menu by the screen (FD's slot cannot be filled or emptied; a mod menu type cannot say
  mayPickup=false) and the screen draws the meal itself.

## Screen
176x166 over `gui/cooking_pot.png` (256x256): slots from `CookingPotMenu` (ingredients (30,17) 3x2 at 18, container
(92,55), output (124,55), inventory y 84, hotbar +58). Heat icon src (176,0,17,15) at (47,55); arrow src (176,15) at
(89,25), width `time*24/total + 1`; title at x 28; "Inventory" label at y 72; tooltip over the heat icon.
Texture regions were confirmed against the PNG's opaque areas.

## Unverified / differences
- Heat tooltip texts ("Heated"/"Not heated"), count text position on the meal, hard-coded English "Inventory" label.
- FD's drops leave out the meal slot and the pot item carries the meal (`copy_meal`); here the meal is dropped as an item.
- No meal hover tooltip ("served on"), no recipe book, no hopper/automation, no comparator signal.
- The meal container: recipe `container`, else the result's remainder (FD: stored container, else the meal's remainder).
- Remainder override list, ejection point/velocity from memory; take-portion sound is a stand-in; no boil sound or steam.
- Experience (`RecipesUsed`) is not awarded. Waterlogging, handle toggle by sneaking not done.
