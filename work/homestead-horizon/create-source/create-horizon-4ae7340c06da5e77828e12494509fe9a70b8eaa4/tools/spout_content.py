"""Spout declarations and independent fallback artwork.

The collision union follows MIT AllShapes.SPOUT at the pinned Create revision.
The models below are authored for this port; original ARR artwork is imported
only from the player's private JAR. Machine behavior belongs to the runtime.
"""

from fallback_textures import material

BLOCK = 'spout'
PARTS = ('top', 'middle', 'bottom')
SHAPE = [[coordinate / 16 for coordinate in box]
         for box in ((1, 2, 1, 15, 14, 15), (2, 0, 2, 14, 16, 14))]


def spout(write, model):
    write(f'data/create/horizon/block/{BLOCK}.json', {
        'template': 'minecraft:copper_block',
        'item': True,
        'properties': {
            'hardness': 3.0,
            'explosion_resistance': 6.0,
            'requires_tool': True,
            'occludes': False,
            'render_shape': 'MODEL',
        },
        'shapes': {'default': SHAPE},
        'collision_shapes': {'default': SHAPE},
    })
    body = [([1, 2, 1], [15, 14, 15]), ([2, 0, 2], [14, 16, 14])]
    nozzle = [([5, -1, 5], [11, 0, 11]),
              ([6, -3, 6], [10, -1, 10]),
              ([7, -5, 7], [9, -3, 9])]
    model(f'{BLOCK}/block', body, material('pipe_copper'))
    model(f'{BLOCK}/item', body + nozzle, material('pipe_copper'))
    for part, box in zip(PARTS, nozzle, strict=True):
        model(f'{BLOCK}/{part}', [box], material('pump_rim'))
    write(f'assets/create/blockstates/{BLOCK}.json', {
        'variants': {'': {'model': f'create:block/{BLOCK}/block'}},
    })
    write(f'assets/create/models/item/{BLOCK}.json', {
        'parent': f'create:block/{BLOCK}/item',
    })
    write(f'assets/create/items/{BLOCK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BLOCK}'},
    })


def spout_recipes(write):
    # MIT generated crafting/loot data. The water recipe is the exact Homestead
    # Create Fabric JSON operation, with 40500 Fabric units divided by 81.
    write('data/create/recipe/crafting/kinetics/spout.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'P': 'minecraft:dried_kelp', 'T': 'create:copper_casing'},
        'pattern': ['T', 'P'],
        'result': {'id': 'create:spout'},
        'show_notification': True,
    })
    write('data/create/recipe/filling/grass_block.json', {
        'type': 'create:filling',
        'ingredients': [
            'minecraft:dirt',
            {'type': 'neoforge:single', 'fluid': 'minecraft:water', 'amount': 500},
        ],
        'results': [{'id': 'minecraft:grass_block'}],
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
