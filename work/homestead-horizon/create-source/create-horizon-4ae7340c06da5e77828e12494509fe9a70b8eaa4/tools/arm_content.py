"""Mechanical Arm block, item, recipe and independent fallback artwork.

The block entity, kinetic behaviour and transfers belong to the runtime
(`mixins/create/arm_runtime.rs`, rules in `engine/src/core/arm.rs`). The original
Blockbench models are imported only from the player's private JAR; the boxes below
are authored for this port and approximate the original silhouette.

Shape numbers (base slab 0..6 plus a 12x12 column) are estimates of AllShapes
MECHANICAL_ARM and were not diffed against the Java source.
"""

from fallback_textures import material

BLOCK = 'mechanical_arm'
PARTS = ('base', 'lower_body', 'upper_body', 'claw_base', 'claw_base_goggles',
         'lower_claw_grip', 'upper_claw_grip', 'cog')

_FLOOR = [[0, 0, 0, 1, 6 / 16, 1], [4 / 16, 6 / 16, 4 / 16, 12 / 16, 10 / 16, 12 / 16]]
_CEILING = [[0, 10 / 16, 0, 1, 1, 1], [4 / 16, 6 / 16, 4 / 16, 12 / 16, 10 / 16, 12 / 16]]


def arm(write, model):
    write(f'data/create/horizon/block/{BLOCK}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': 2.0,
            'explosion_resistance': 6.0,
            'requires_tool': True,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': {'ceiling': {'type': 'boolean'}},
        'default_state': {'ceiling': 'false'},
        'shapes': {'property': 'ceiling', 'values': {'false': _FLOOR, 'true': _CEILING}},
        'collision_shapes': {'property': 'ceiling', 'values': {'false': _FLOOR, 'true': _CEILING}},
    })
    brass = material('brass_casing')
    model(f'{BLOCK}/block', [([0, 0, 0], [16, 6, 16])], brass)
    model(f'{BLOCK}/item', [([0, 0, 0], [16, 6, 16]), ([5, 6, 5], [11, 14, 11]),
                            ([6, 14, 3], [10, 17, 13])], brass)
    model(f'{BLOCK}/base', [([3, 6, 3], [13, 9, 13])], brass)
    model(f'{BLOCK}/lower_body', [([6, 0, 6], [10, 12, 10])], brass)
    model(f'{BLOCK}/upper_body', [([6.5, 0, 6.5], [9.5, 12, 9.5])], brass)
    model(f'{BLOCK}/claw_base', [([5, 0, 5], [11, 4, 11])], brass)
    model(f'{BLOCK}/claw_base_goggles', [([5, 0, 5], [11, 4, 11]), ([4, 4, 6], [12, 7, 10])], brass)
    model(f'{BLOCK}/lower_claw_grip', [([6, 0, 4], [10, 4, 5])], brass)
    model(f'{BLOCK}/upper_claw_grip', [([6, 0, 11], [10, 4, 12])], brass)
    model(f'{BLOCK}/cog', [([4, 2, 4], [12, 4, 12])], material('pump_rim'))
    write(f'assets/create/blockstates/{BLOCK}.json', {'variants': {
        'ceiling=false': {'model': f'create:block/{BLOCK}/block'},
        'ceiling=true': {'model': f'create:block/{BLOCK}/block', 'x': 180},
    }})
    write(f'assets/create/models/item/{BLOCK}.json', {'parent': f'create:block/{BLOCK}/item'})
    write(f'assets/create/items/{BLOCK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BLOCK}'},
    })


def arm_recipes(write):
    # Same pattern as the original: brass plates (sheets here), andesite alloy,
    # a brass casing and a precision mechanism.
    write(f'data/create/recipe/crafting/kinetics/{BLOCK}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'A': 'create:andesite_alloy', 'C': 'create:brass_casing',
                'I': 'create:precision_mechanism', 'L': 'create:brass_sheet'},
        'pattern': ['LLA', 'L  ', 'IC '],
        'result': {'id': f'create:{BLOCK}'},
        'show_notification': True,
    })
    write(f'data/create/loot_table/blocks/{BLOCK}.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'conditions': [{'condition': 'minecraft:survives_explosion'}],
            'entries': [{'type': 'minecraft:item', 'name': f'create:{BLOCK}'}],
            'rolls': 1.0,
        }],
        'random_sequence': f'create:blocks/{BLOCK}',
    })
