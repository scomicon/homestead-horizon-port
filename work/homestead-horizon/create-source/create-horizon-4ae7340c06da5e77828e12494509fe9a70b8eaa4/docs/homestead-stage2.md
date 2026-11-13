# Homestead Spout increment

This local extension is identified as `0.9.0+homestead.2`; it is not an upstream
Create Horizon release. It retains the pinned Horizon/Mixins revisions in
`compatibility.json` and uses the original player-supplied Homestead JAR only
for private resource import. The import schema is 51.

## Native scope

`create:spout` has its own block entity, a canonical single 1000 mB
`SmartFluidTankBehaviour` tank, source `ProcessingTicks`, comparator output and
the original collision union. The three nozzle parts and fluid box use the
Spout renderer's source transforms. Independent built-in models remain
available without copying original artwork into the source package.

The implementation accepts water and lava with empty fluid-component patches.
Fluid handling excludes the lower face. The existing straight, bounded native
pump network can transfer from an isolated Fluid Tank into a Spout from another
permitted face. The source and receiver are preflighted and committed together;
a failed receiver write restores the exact original source fields. This is not
a general fluid-network, cross-chunk or external-handler transaction API.

The existing transport processor contract serves both Depots and bare horizontal
Belts. The Spout uses the original 20-tick phase, exchanges one item at phase
five, retains the remaining stack, clears its fan processing state and completes
its cooldown. Recipe and fluid availability are checked again at exchange.
An unknown tank or phase remains opaque and holds cargo rather than being
treated as an empty tank. Ordinary empty vanilla buckets use Horizon's fluid
item API and require 1000 mB. Glass-bottle and arbitrary capability filling,
sequenced assembly, block spouting and other fluids remain unsupported.

Supported filling JSON contains one ordinary item ingredient, one explicit
single water/lava ingredient and one deterministic registered item result.
Unknown predicates, probabilistic results and fluid components are not silently
simplified. Recipe caches are cleared on data reload and world reset.

## Homestead data profile

The optional, separately generated `create_oxidized` content mod retains twelve
original recipe identifiers from the exact local Create Oxidized 0.1.1 JAR.
Each oxidizes one vanilla copper block, cut block, slab or stair by one stage,
using 20250 Fabric units, exactly 250 mB. Eighteen recipes referencing missing
Create copper tiles and shingles remain rejected. No Java entrypoint is run.
The source metadata's MIT declaration and bundled CC0 notice are both retained
by the data adapter instead of resolving their discrepancy by assumption.

The native content also includes the player JAR's Dirt + 40500 Fabric units of
water -> Grass Block operation as 500 mB. This is a Homestead resource-data
adaptation, not a claim that the reference NeoForge data is identical.
Amounts not divisible by 81, including 27000-unit Fabric bottle recipes,
are not rounded to the native integer-millibucket representation.

## Reference and validation boundaries

MIT source mapping at original Create commit
`ac0c444d9828da3453ae8cc65338e8de063286fb`:

- `content/fluids/spout/SpoutBlockEntity`: callbacks, capacity, sided capability,
  timer and saved phase.
- `content/fluids/spout/FillingBySpout`, `transfer/GenericItemFilling`: recipe
  precedence, one-item conversion and the bounded bucket fallback.
- `content/fluids/spout/SpoutBlock`, `AllShapes.SPOUT`: collision, comparator,
  and downward-expanded render bounds.
- `content/fluids/spout/SpoutRenderer`: nozzle translations, stored-fluid volume
  and narrowing fluid stream.

Copyright remains with The Create Team / The Creators of Create. Their code
license is MIT in the pinned `LICENSE.md`; original assets are reserved and
remain inside the private JAR/cache workflow.

Six core phase tests, two additional importer tests and the data adapter's
tests cover their bounded contracts. Actual machine and process-restart
acceptance is recorded separately in the delivered validation folder. Numeric
phase tests and resource-import success do not establish Java/Fabric parity.
Particles, advancement ownership, all presentation details, remote/dedicated
multiplayer and mid-cycle Java/native cold-load comparisons still need their
own evidence. The behavioral reference remains NeoForge Create 6.0.10 while
Homestead's resource/data source is Fabric Create 6.0.8.1.
