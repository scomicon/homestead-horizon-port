# Schematic tools (native Horizon integration)

Adapter in `mixins/create/schematic_tool_runtime.rs` over the pure rules of
`engine/src/core/schematic_tool.rs` (Create 6.0.10 `SchematicItem`, `SchematicAndQuillItem`,
`SchematicAndQuillHandler`, `SchematicTransformation`, `SchematicSyncPacket`, `SchematicExport`,
`FilesHelper`). Bundle edits are in `mixins/create/fragments/schematic_tools.{toml,md}`.

## What works

### Schematic and Quill (`create:schematic_and_quill`)

- Right click on a block selects a corner: the first click sets corner one, the second corner two
  (`SchematicAndQuillHandler`; a vertical face of a solid block selects the neighbouring block, so
  clicking the top of the floor selects the block above it).
- With both corners set, another click only repeats the size and the hint. Sneak + right click (on a
  block or in the air) discards the selection.
- `/create schematool save [name]` writes the box to `<instance>/schematics/<name>.nbt` and clears the
  selection; `/create schematool replace <name>` overwrites. Naming follows `SchematicExport`: an empty
  name becomes `My Schematic`; without replace the name is slugged (`FilesHelper.slug`: lower case,
  runs of non-word characters become `_`) and the first free `slug.nbt`, `slug_1.nbt`, `slug_2.nbt` ...
  is used.
- The box is the inclusive span of the two corners (`export_bounds`). Air is not stored. The file is a
  gzip structure NBT with DataVersion 3955, so `create schematic inspect|paste` and the Schematicannon
  load it. Limits are the loader's: 128 per side, 262144 volume, 32768 blocks.

### Schematic (`create:schematic`)

- Right click on a block with a Schematic that is not deployed deploys it with the anchor on the clicked
  face's neighbour. Sneak + right click on a block picks a deployed one up. Blocks with a block entity
  (Schematicannon, Schematic Table, chests ...) keep their own right click.
- `rotate cw|ccw`, `mirror x|z`, `move dx dy dz`, `anchor x y z`, `deploy [x y z]`, `undeploy` change
  the held Schematic through `Transformation` (anchor, rotation and mirror conversions of
  `SchematicTransformation`) and write the result the way `SchematicSyncPacket` does
  (`apply_sync`; the anchor is stored as given). `info` prints the stored state.
- Hand a file out with `create cannon give <file>` (existing command), print it with a Schematicannon.

### Operator commands (permission 2)

| Command | Meaning |
| --- | --- |
| `create schematool quill` | give a Schematic and Quill |
| `create schematool corner X Y Z` | the click code path without a held item (selects exactly that block) |
| `create schematool selection` / `discard` | show / clear the corners |
| `create schematool save [name]` / `replace name` | export |
| `create schematool info` | held Schematic's file, bounds, anchor, quarter turns, mirror, deployed |
| `create schematool rotate cw|ccw` / `mirror x|z` / `move dx dy dz` / `anchor x y z` | transform the held Schematic |
| `create schematool deploy [x y z]` / `undeploy` | deploy at the stored or given anchor / pick up |

The root is `create schematool` because `create schematic ...` (paste/inspect) and `create cannon ...`
already claim every line below their roots.

## Not done

- No ghost preview, outlines, tool wheel, key bindings, scroll wheel (range, resizing), name prompt
  screen or Schematic edit screen. The commands above stand in for them.
- Exports carry no block entity data and no entities (the loader only accepts known schemas); a saved
  chest is saved empty. Blocks the loader does not support (`create:` blocks without an implemented
  schema, unlisted vanilla blocks) are written but are rejected when loaded; `create schematic inspect`
  names them.
- The selection is one global handler (client side per player in Create).
- The Schematic's `Owner` is not checked on deploy; the Quill recipe is a placeholder.
- Rotation/scale animation (LerpedFloat chasers) is presentation only.

## Unverified

Never compiled or run when written: assumptions about the `use_item_on` face argument (unit normal),
registry descriptor fields, and the held-item helpers are listed in `fragments/schematic_tools.md`.
