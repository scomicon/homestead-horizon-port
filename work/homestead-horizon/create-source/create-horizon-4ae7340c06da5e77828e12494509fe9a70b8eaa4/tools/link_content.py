"""Redstone Link block, item, recipe and independent fallback artwork.

The block entity, frequency network and signal output belong to the runtime
(`mixins/create/link_runtime.rs`, rules in `engine/src/core/redstone_link.rs`).
The original models are imported only from the player's private JAR; the boxes
below are authored for this port.

State schema follows RedstoneLinkBlock: `facing` (6 directions; the plate is
attached to the opposite neighbour), `powered`, `receiver` (false = transmitter).
Shape numbers (a 10x10x2 plate) are estimates of AllShapes.REDSTONE_BRIDGE and
were not diffed against the Java source. The recipe (electron tube over brass
sheet, 2 links) and the block properties are recalled, not read from the pinned
source; `reference_recipes.py` exports the real recipe once the block is listed
in catalog.BLOCKS.
"""

BLOCK = 'redstone_link'
# Model names under assets/create/models/block/redstone_link/. The same names
# are used for the fallback models; an imported original of the same name wins.
PARTS = ('transmitter', 'transmitter_powered', 'receiver', 'receiver_powered', 'item')

_FACES = ('down', 'up', 'north', 'south', 'west', 'east')
# Plate boxes (pixels) hugging the face opposite to `facing`.
_PLATES = {
    'up': (3, 0, 3, 13, 2, 13),
    'down': (3, 14, 3, 13, 16, 13),
    'north': (3, 3, 14, 13, 13, 16),
    'south': (3, 3, 0, 13, 13, 2),
    'west': (14, 3, 3, 16, 13, 13),
    'east': (0, 3, 3, 2, 13, 13),
}
# Same rotation table as the other directional blocks (model drawn for `up`).
_TURNS = {'down': {'x': 180}, 'up': {}, 'north': {'x': 90}, 'south': {'x': 90, 'y': 180},
          'west': {'x': 90, 'y': 270}, 'east': {'x': 90, 'y': 90}}
_LAMPS = {
    'transmitter': 'minecraft:block/gold_block',
    'transmitter_powered': 'minecraft:block/redstone_block',
    'receiver': 'minecraft:block/lapis_block',
    'receiver_powered': 'minecraft:block/redstone_lamp_on',
}


def _boxes(plate):
    return [[v / 16 for v in plate]]


def redstone_link(write, model):
    shapes = {face: _boxes(plate) for face, plate in _PLATES.items()}
    write(f'data/create/horizon/block/{BLOCK}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': 2.0,
            'explosion_resistance': 3.0,
            'requires_tool': False,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:oak_planks',
        },
        'state_properties': {
            'facing': {'type': 'enum', 'values': list(_FACES)},
            'powered': {'type': 'boolean'},
            'receiver': {'type': 'boolean'},
        },
        'default_state': {'facing': 'up', 'powered': 'false', 'receiver': 'false'},
        'shapes': {'property': 'facing', 'values': shapes},
        # The plate has no collision in Create's decorative sense but is selectable;
        # keep it solid like the rest of the shape so entities do not clip into it.
        'collision_shapes': {'property': 'facing', 'values': shapes},
    })
    base = [([3, 0, 3], [13, 2, 13])]
    for name, lamp in _LAMPS.items():
        model(f'{BLOCK}/{name}', base + [([6, 2, 6], [10, 3, 10], 'lamp')],
              'minecraft:block/dark_oak_planks', parts={'lamp': lamp})
    model(f'{BLOCK}/item', base + [([6, 2, 6], [10, 3, 10], 'lamp')],
          'minecraft:block/dark_oak_planks', parts={'lamp': _LAMPS['transmitter']})
    variants = {}
    for face, turn in _TURNS.items():
        for powered in ('false', 'true'):
            for receiver in ('false', 'true'):
                part = ('receiver' if receiver == 'true' else 'transmitter') + (
                    '_powered' if powered == 'true' else '')
                variants[f'facing={face},powered={powered},receiver={receiver}'] = {
                    'model': f'create:block/{BLOCK}/{part}', **turn,
                }
    write(f'assets/create/blockstates/{BLOCK}.json', {'variants': variants})
    write(f'assets/create/models/item/{BLOCK}.json', {'parent': f'create:block/{BLOCK}/item'})
    write(f'assets/create/items/{BLOCK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BLOCK}'},
    })


def redstone_link_recipes(write):
    write(f'data/create/recipe/crafting/logistics/{BLOCK}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'redstone',
        'key': {'E': 'create:electron_tube', 'B': 'create:brass_sheet'},
        'pattern': ['E', 'B'],
        'result': {'id': f'create:{BLOCK}', 'count': 2},
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
