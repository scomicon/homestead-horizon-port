# Homestead stage 4: data and utility candidates

Read-only scope: the supplied instance's script text, fabric.mod.json, bundled
license text and data/**/recipes/**/*.json. No Java class body or original save
was read. Native registration/recipe comparison used the current Create
catalog and generated content, plus the owned target Minecraft 26.3 resources.

## Recommendation: two concrete automation choices

1. **Real Item Vault, then its Homestead recipe.**
   scripts/mods/Create/vault.zs removes recipes producing create:item_vault and
   adds a 1-wide vertical pattern: create:iron_sheet / minecraft:barrel /
   create:iron_sheet -> two create:item_vault. The displayed script is a
   three-column grid with air on either side; trimming those columns preserves
   the recipe. Input IDs already exist. The output block/item does not exist in
   the active catalog, so importing just this JSON cannot provide storage.
   Required work is genuine Vault storage and access from a member block,
   conservative persistence/drop handling and the original multiblock storage
   contract. A registered barrel-shaped replacement with a Vault label would
   not satisfy the recipe or the mod's behavior. The independent engine/Vault
   audit determines the exact shared-inventory extension boundary.

2. **Andesite Belt Funnel, using the already craftable real Funnel item.**
   The local Fabric Create JAR contains this exact vertical craft:
   create:andesite_alloy / minecraft:dried_kelp -> two create:andesite_funnel.
   The native port already registers the item, same ingredients and output.
   Brass's existing vertical craft is create:electron_tube /
   #c:brass_ingots / minecraft:dried_kelp -> two create:brass_funnel in source;
   the modern native recipe uses #c:ingots/brass and the registered brass ingot.
   No new recipe count is gained by copying these data files. The real increment
   is placing the existing Funnel item on transport into its belt-specific block,
   then normal inventory/Belt insertion or extraction, power gating, shape/wrench
   transitions and conservative backpressure. The current native catalog has
   ordinary andesite_funnel/brass_funnel but no belt-funnel blockstates.
   Implement Andesite first if a narrow, observable transport increment is wanted.

The root chose option 2. Content and asset work must be based on the pinned
MIT BeltFunnelBlock/AllShapes and the user's private original-resource import.

## Why Chipped is not a shortcut to functional logistics

The exact chipped-fabric-1.20.1-3.0.7.jar has SHA256
b1af1f1a653a65b200c5fd547c59a97e176f6d5b2e41519596f7a30394d4f330.
Its 14 recipe JSON files comprise seven minecraft:crafting_shaped recipes
producing Chipped workbenches and one recipe for each of seven custom serializers:
alchemy_bench, botanist_workbench, carpenters_table, glassblower, loom_table,
mason_table and tinkering_table. The custom entries name tag-based variant sets.
None supplies a native storage or transport implementation. The seven workbench
outputs are unregistered, and their custom menu/recipe handlers are absent.
Metadata says Terrarium Licence; the earlier static data carry is not a
functional port or a permission to redistribute original artwork.

## Other small utility candidate, for a later decision

CraftingPad-fabric-1.20.1-1.0.12.jar, SHA256
dc9a31b6838141fa9b60f9080e22283aaef94d0f406a61d3235863374a9e205f,
contains one craft: a crafting table directly above a stick -> one
craftingpad:craftingpad. The jar metadata's license field is absent, but bundled
LICENSE_CraftingPad-fabric-1.20.1 is MIT and attributes StrikerRocker and Guntram
Blohm; metadata links the StrikerRockers-Mods/CraftingPad source repository.

It is a potentially small portable-crafting utility, not data-only: its item is
unregistered, and its actual use semantics, stack limit and close/remainder
behavior need verified source. Pinned Horizon's native_open_menu currently opens
chest-row menus for connected players, not a portable vanilla crafting context.
Opening a fake world crafting-table block or drawing a nonfunctional menu is not
acceptable. A proper port needs item use plus the real 3x3 crafting result,
ordinary clicks/shift-click, ingredients/remainders and safe close behavior.

Tom's Storage's 20 JSON recipes all depend on unregistered storage/cable/terminal
items; Create Contraption Terminals has zero data recipe JSON and needs the code
integration. Simple Copper Pipes' ten recipes create its unregistered pipe/fitting
family and do not port its transport. Sophisticated storage/backpack upgrades
require their inventory-capacity behavior, not just item registration.

## Available data-only script changes are smaller than the chosen increment

The remaining 86 unsupported Create script operations are blocked by missing
items or unsupported fluid/recipe semantics; the stage-3 ancient-debris exception
has already been added. The flour compatibility script needs missing farm_and_charm
items and old tag aliases; existing native Create already crafts dough from its
registered wheat flour and a water bucket. It is not a new production mechanism.

Custom name-tag and saddle scripts are not a substitute for transport/storage:
the owned target 26.3 resources already contain vanilla crafts for both.
Importing the Homestead name-tag grid would change material cost rather than
unlock portable crafting or a missing storage system. Custom tuff is a small
building-material recipe, outside the requested logistics priority.

No production source or output was changed by this research.