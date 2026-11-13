"""Schematicannon, Schematic Table, Empty Schematic, Schematic and Material Checklist
(Create 6.0.10) with independent fallback artwork.

The block entities, menus, screens and the cannon's simulation belong to the runtime
(`mixins/create/cannon_runtime.rs`, `schematic_table_runtime.rs`, `schematic_data.rs`;
rules in `engine/src/core/schematicannon.rs`, `schematic_printer.rs`,
`schematic_table.rs`, `material_checklist.rs`). Original models are imported only from the
player's private JAR (same model names win); the boxes below are authored for this port.

Both blocks use the `minecraft:barrel` template because a mod menu opens only when the
template's use effect opens one (a glass template opens nothing). The barrel's own state
properties (`facing`, `open`) come from the template and are not declared here; the model is
the same for every state.

Items: `create:empty_schematic`, `create:schematic` (its data lives in the runtime's
`create:schematic_data` component) and `create:clipboard`, the Material Checklist (its entries
live in the `create:checklist` component the cannon writes). Schematic and Quill is not ported.

UNVERIFIED (recalled, not diffed against the pinned source): the recipes, hardness and
resistance, and the box shapes. The recipes are placeholders built from items of this port.
"""

from fallback_textures import material

CANNON = 'schematicannon'
TABLE = 'schematic_table'
BLOCKS = (CANNON, TABLE)
EMPTY = 'empty_schematic'
SCHEMATIC = 'schematic'
CLIPBOARD = 'clipboard'
ITEMS = (EMPTY, SCHEMATIC, CLIPBOARD)
# Model names under assets/create/models/block/ (mirror in catalog.PARTS).
PARTS = (f'{CANNON}/block', f'{CANNON}/item', f'{TABLE}/block', f'{TABLE}/item')

LANG_EN = {CANNON: 'Schematicannon', TABLE: 'Schematic Table'}
LANG_RU = {CANNON: 'Схематическая пушка', TABLE: 'Стол схем'}
ITEM_LANG_EN = {EMPTY: 'Empty Schematic', SCHEMATIC: 'Schematic', CLIPBOARD: 'Material Checklist'}
ITEM_LANG_RU = {EMPTY: 'Пустая схема', SCHEMATIC: 'Схема', CLIPBOARD: 'Список материалов'}

# Vanilla item textures standing in for the item sprites (the JAR replaces them).
_SPRITES = {EMPTY: 'paper', SCHEMATIC: 'map', CLIPBOARD: 'writable_book'}
_STACK_SIZE = {EMPTY: 64, SCHEMATIC: 1, CLIPBOARD: 64}


def _cannon_boxes():
    # Base plate, housing, and a short barrel pointing up.
    return [([1, 0, 1], [15, 5, 15]), ([3, 5, 3], [13, 10, 13]), ([5, 10, 5], [11, 16, 11], 'barrel')]


def _table_boxes():
    # Desk top with four legs.
    boxes = [([0, 10, 0], [16, 16, 16], 'top')]
    for x in (1, 12):
        for z in (1, 12):
            boxes.append(([x, 0, z], [x + 3, 10, z + 3]))
    return boxes


def schematic_blocks(write, model):
    for name, shape in ((CANNON, [[0, 0, 0, 1, 1, 1]]), (TABLE, [[0, 0, 0, 1, 1, 1]])):
        write(f'data/create/horizon/block/{name}.json', {
            'template': 'minecraft:barrel',
            'item': True,
            'properties': {
                'hardness': 2.0,
                'explosion_resistance': 6.0,
                'requires_tool': True,
                'occludes': False,
                'render_shape': 'MODEL',
                'sounds': 'minecraft:copper_block',
            },
            'shapes': {'default': shape},
            'collision_shapes': {'default': shape},
        })
        write(f'assets/create/blockstates/{name}.json', {
            'variants': {'': {'model': f'create:block/{name}/block'}},
        })
        write(f'assets/create/models/item/{name}.json', {'parent': f'create:block/{name}/item'})
        write(f'assets/create/items/{name}.json', {
            'model': {'type': 'minecraft:model', 'model': f'create:item/{name}'},
        })
    andesite = material('andesite_casing')
    brass = material('brass_casing')
    for part in ('block', 'item'):
        model(f'{CANNON}/{part}', _cannon_boxes(), andesite, parts={'barrel': material('steel_axle')})
        model(f'{TABLE}/{part}', _table_boxes(), andesite, parts={'top': brass})
    # Mod menu types: slot count, data slots the screen reads, slots players only take from.
    write(f'data/create/horizon/menu/{CANNON}.json', {'slots': 5, 'data_slots': 9, 'results': [1, 3]})
    write(f'data/create/horizon/menu/{TABLE}.json', {'slots': 2, 'data_slots': 3, 'results': [1]})


def schematic_items(write):
    for item in ITEMS:
        write(f'data/create/horizon/item/{item}.json', {
            'template': 'minecraft:iron_ingot',
            'max_stack_size': _STACK_SIZE[item],
        })
        write(f'assets/create/models/item/{item}.json', {
            'parent': 'minecraft:item/generated',
            'textures': {'layer0': f'minecraft:item/{_SPRITES[item]}'},
        })
        write(f'assets/create/items/{item}.json', {
            'model': {'type': 'minecraft:model', 'model': f'create:item/{item}'},
        })


def _drop(name):
    return {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'conditions': [{'condition': 'minecraft:survives_explosion'}],
            'entries': [{'type': 'minecraft:item', 'name': f'create:{name}'}],
            'rolls': 1.0,
        }],
        'random_sequence': f'create:blocks/{name}',
    }


def schematic_recipes(write):
    # UNVERIFIED placeholders.
    write(f'data/create/recipe/crafting/schematics/{CANNON}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'I': 'minecraft:iron_block', 'D': 'minecraft:dispenser', 'C': 'create:andesite_casing'},
        'pattern': ['I', 'D', 'C'],
        'result': {'id': f'create:{CANNON}'},
        'show_notification': True,
    })
    write(f'data/create/recipe/crafting/schematics/{TABLE}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'S': 'minecraft:oak_slab', 'C': 'create:andesite_casing'},
        'pattern': ['SSS', ' C '],
        'result': {'id': f'create:{TABLE}'},
        'show_notification': True,
    })
    write(f'data/create/recipe/crafting/schematics/{EMPTY}.json', {
        'type': 'minecraft:crafting_shapeless',
        'category': 'misc',
        'ingredients': ['minecraft:paper', 'minecraft:lapis_lazuli'],
        'result': {'id': f'create:{EMPTY}'},
    })
    for name in BLOCKS:
        write(f'data/create/loot_table/blocks/{name}.json', _drop(name))
