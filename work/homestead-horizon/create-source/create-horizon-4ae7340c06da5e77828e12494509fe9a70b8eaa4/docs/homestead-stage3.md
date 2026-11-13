# Homestead fluid automation increment

This local package is 0.9.0+homestead.3, on the same pinned Horizon and
Mixins revisions as compatibility.json. It extends the Spout increment;
it is not a new upstream Create release or a complete Homestead port.

## Item Drain and straight pipelines

The original ItemDrainBlockEntity exposes one 1500 mB tank through an
extraction-only capability on every face except UP, including an unsided
query. The native adapter follows those restrictions. Queries, exact/any
drain requests and simulation preserve unrelated NBT and transported cargo.
Only literal water/lava and empty fluid components are supported.

The existing bounded straight pipe/pump line now accepts either a singleton
Fluid Tank or an Item Drain as its source, and a singleton Fluid Tank or a
Spout as receiver. Both canonical records and capacity are preflighted before
either is written. No callbacks or notifications run between the writes; a
failed second write restores the exact source NBT. Drain insertion and Spout
extraction through this pipeline remain unsupported. Branching networks,
arbitrary third-party handlers, multiblock tanks and moving fluid storage
remain separate work.

Unknown tank fields and nonempty fluid components are rejected without being
replaced by empty storage. An empty Drain is saved as canonical empty content,
not a zero-amount Fluid. Its item/cycle fields remain unchanged by pipe
extraction. Fluid notifications share an eight-tick lazy cooldown and flush
after the pump stops; comparators still observe actual changes.

## Ancient debris crushing

The supplied Homestead script omits processingTime for ancient debris.
The pinned original ProcessingRecipeParams initializes processingDuration to
zero. The adapter retains that default only for create:crushing; other codecs
keep their existing duration checks. The crusher accepts zero-duration
recipes through its existing powered Apply/Eject lifecycle instead of
substituting a guessed duration.

The original outputs are two guaranteed netherite scraps, one extra at 0.35
and another at 0.15. Each source item is consumed once. This adds one recipe,
bringing the bounded Homestead script profile to 28 additions and four
removals. Missing items, unsupported scripts and fractional Fabric fluid
amounts remain explicitly rejected.

## Attribution and evidence

Behavior references are MIT source at Create commit
ac0c444d9828da3453ae8cc65338e8de063286fb:
content/fluids/drain/ItemDrainBlockEntity, foundation/fluid/SmartFluidTankBehaviour,
and content/processing/recipe/ProcessingRecipeParams. Copyright remains with
The Create Team / The Creators of Create.

The exact local Homestead Fabric Create 6.0.8.1 JAR remains a private resource
source; no original JAR or reserved artwork is part of this source package.
Component tests, extracted adapter/codec checks, live native machinery and
process restarts are reported separately in the delivered validation folder.
No Java runtime comparison or full presentation parity is claimed.