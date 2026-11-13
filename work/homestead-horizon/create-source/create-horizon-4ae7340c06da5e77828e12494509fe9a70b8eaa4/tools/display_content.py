"""Display Link and Display Board blocks, items, recipes and fallback artwork.

The block entities, the link update loop and the board text belong to the
runtime (`mixins/create/display_link_runtime.rs`, `display_board_runtime.rs`,
`display_source.rs`; rules in `engine/src/core/display_link.rs` and
`display_board.rs`). The original models are imported only from the player's
private JAR; the boxes below are authored for this port.

State schema follows DisplayLinkBlock (`facing` 6 directions, the plate is
attached to the opposite neighbour, `powered`) and FlapDisplayBlock (`facing`
horizontal = the face that shows text, `up` / `down` = a board of the same
facing above / below). Shape numbers are estimates and were not diffed against
the Java source: link plate 10x10x2 (like the Redstone Link), board 16x16x8
hugging the back. The recipes are placeholders (electron tube / brass sheet),
not Create's recipes; `reference_recipes.py` can export the real ones once the
blocks are listed in catalog.BLOCKS.
"""

LINK = 'display_link'
BOARD = 'display_board'
BLOCKS = (LINK, BOARD)
# Model names under assets/create/models/block/<block>/.
LINK_PARTS = ('link', 'link_powered', 'item')
BOARD_PARTS = ('board', 'item')
PARTS = tuple(f'{LINK}/{p}' for p in LINK_PARTS) + tuple(f'{BOARD}/{p}' for p in BOARD_PARTS)

_FACES = ('down', 'up', 'north', 'south', 'west', 'east')
_PLATES = {
    'up': (3, 0, 3, 13, 2, 13),
    'down': (3, 14, 3, 13, 16, 13),
    'north': (3, 3, 14, 13, 13, 16),
    'south': (3, 3, 0, 13, 13, 2),
    'west': (14, 3, 3, 16, 13, 13),
    'east': (0, 3, 3, 2, 13, 13),
}
_TURNS = {'down': {'x': 180}, 'up': {}, 'north': {'x': 90}, 'south': {'x': 90, 'y': 180},
          'west': {'x': 90, 'y': 270}, 'east': {'x': 90, 'y': 90}}
_LAMPS = {'link': 'minecraft:block/cyan_concrete', 'link_powered': 'minecraft:block/lime_concrete'}

# Board: model drawn facing south (front +z, body at the back, z 0..8 px).
_BOARD_BOXES = {
    'south': (0, 0, 0, 16, 16, 8),
    'north': (0, 0, 8, 16, 16, 16),
    'east': (0, 0, 0, 8, 16, 16),
    'west': (8, 0, 0, 16, 16, 16),
}
_BOARD_TURNS = {'south': {}, 'west': {'y': 90}, 'north': {'y': 180}, 'east': {'y': 270}}


def _boxes(box):
    return [[v / 16 for v in box]]


def _block_json(write, name, state_properties, default_state, shapes, hardness):
    write(f'data/create/horizon/block/{name}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': hardness,
            'explosion_resistance': 3.0,
            'requires_tool': False,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': state_properties,
        'default_state': default_state,
        'shapes': shapes,
        'collision_shapes': shapes,
    })


def display_blocks(write, model):
    # --- Display Link
    link_shapes = {'property': 'facing', 'values': {f: _boxes(p) for f, p in _PLATES.items()}}
    _block_json(write, LINK, {
        'facing': {'type': 'enum', 'values': list(_FACES)},
        'powered': {'type': 'boolean'},
    }, {'facing': 'up', 'powered': 'false'}, link_shapes, 2.0)
    base = [([3, 0, 3], [13, 2, 13])]
    for part, lamp in _LAMPS.items():
        model(f'{LINK}/{part}', base + [([5, 2, 5], [11, 3, 11], 'lamp')],
              'minecraft:block/gray_concrete', parts={'lamp': lamp})
    model(f'{LINK}/item', base + [([5, 2, 5], [11, 3, 11], 'lamp')],
          'minecraft:block/gray_concrete', parts={'lamp': _LAMPS['link']})
    variants = {}
    for face, turn in _TURNS.items():
        for powered in ('false', 'true'):
            part = 'link_powered' if powered == 'true' else 'link'
            variants[f'facing={face},powered={powered}'] = {'model': f'create:block/{LINK}/{part}', **turn}
    write(f'assets/create/blockstates/{LINK}.json', {'variants': variants})
    write(f'assets/create/models/item/{LINK}.json', {'parent': f'create:block/{LINK}/item'})
    write(f'assets/create/items/{LINK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{LINK}'},
    })

    # --- Display Board
    board_shapes = {'property': 'facing', 'values': {f: _boxes(b) for f, b in _BOARD_BOXES.items()}}
    _block_json(write, BOARD, {
        'facing': {'type': 'enum', 'values': ['north', 'south', 'west', 'east']},
        'up': {'type': 'boolean'},
        'down': {'type': 'boolean'},
    }, {'facing': 'north', 'up': 'false', 'down': 'false'}, board_shapes, 2.0)
    board_model = [([0, 0, 0], [16, 16, 6]), ([0, 0, 6], [16, 16, 8], 'screen')]
    model(f'{BOARD}/board', board_model, 'minecraft:block/gray_concrete',
          parts={'screen': 'minecraft:block/black_concrete'})
    model(f'{BOARD}/item', board_model, 'minecraft:block/gray_concrete',
          parts={'screen': 'minecraft:block/black_concrete'})
    variants = {}
    for face, turn in _BOARD_TURNS.items():
        for up in ('false', 'true'):
            for down in ('false', 'true'):
                variants[f'facing={face},up={up},down={down}'] = {'model': f'create:block/{BOARD}/board', **turn}
    write(f'assets/create/blockstates/{BOARD}.json', {'variants': variants})
    write(f'assets/create/models/item/{BOARD}.json', {'parent': f'create:block/{BOARD}/item'})
    write(f'assets/create/items/{BOARD}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BOARD}'},
    })


def _loot(write, name):
    write(f'data/create/loot_table/blocks/{name}.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'conditions': [{'condition': 'minecraft:survives_explosion'}],
            'entries': [{'type': 'minecraft:item', 'name': f'create:{name}'}],
            'rolls': 1.0,
        }],
        'random_sequence': f'create:blocks/{name}',
    })


def display_recipes(write):
    # Placeholder recipes (not Create's): see the module docstring.
    write(f'data/create/recipe/crafting/logistics/{LINK}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'redstone',
        'key': {'E': 'create:electron_tube', 'B': 'create:brass_sheet', 'G': 'minecraft:glass_pane'},
        'pattern': ['E', 'B', 'G'],
        'result': {'id': f'create:{LINK}', 'count': 1},
        'show_notification': True,
    })
    write(f'data/create/recipe/crafting/logistics/{BOARD}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'redstone',
        'key': {'E': 'create:electron_tube', 'B': 'create:brass_sheet'},
        'pattern': ['BBB', 'BEB', 'BBB'],
        'result': {'id': f'create:{BOARD}', 'count': 1},
        'show_notification': True,
    })
    _loot(write, LINK)
    _loot(write, BOARD)


LANG_EN = {LINK: 'Display Link', BOARD: 'Display Board'}
LANG_RU = {LINK: 'Звено дисплея', BOARD: 'Табло'}
