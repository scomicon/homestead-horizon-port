"""Steam Whistle and Steam Whistle Extension blocks, item, recipe, loot and
independent fallback artwork.

The block entity, powering and sound belong to the runtime
(`mixins/create/whistle_runtime.rs`, rules in `engine/src/core/steam_whistle.rs`).
The original models are imported only from the player's private JAR; the boxes
below are authored for this port. Model names, the blockstate layout and the
state schema follow the JAR's `steam_whistle` / `steam_whistle_extension`
blockstates (Create 6.0.8.1 fabric build used as the local probe):

  create:steam_whistle            facing (horizontal), wall, powered, size
  create:steam_whistle_extension  shape (single|double|double_connected), size

With `wall=false` the whistle stands on a tank; with `wall=true` the base plate
is on the `facing` side (model drawn for `south`, plate at z=13..16).

Shape numbers are estimates of AllShapes (not diffed against the Java source).
The recipe (golden sheet over copper ingot) is the original's `c:plates/gold`
over `c:copper_ingots` mapped to this port's items. All fallback textures are
`create:block/fallback/*` tiles, so no `minecraft:` texture needs a FAMILIES
entry; `fallback_textures.FAMILIES['steam_whistle']` is still recommended for
tools that iterate PARTS.
"""

from fallback_textures import material

BLOCK = 'steam_whistle'
EXTENSION = 'steam_whistle_extension'

SIZES = ('small', 'medium', 'large')
FACINGS = ('north', 'south', 'west', 'east')
EXTENDER_SHAPES = ('single', 'double', 'double_connected')

# Model names under assets/create/models/block/steam_whistle/ (same names as the
# JAR; an imported original of the same name wins over the fallback).
PARTS = (
    ['item']
    + [f'block_{size}_{mount}{powered}' for size in SIZES for mount in ('floor', 'wall')
       for powered in ('', '_powered')]
    + [f'extension/{size}_{kind}' for size in SIZES for kind in ('single', 'double', 'top_rim')]
)

# Pixel half-extents per size: pipe, body, cap (min, max on both horizontal axes).
_DIMS = {
    'small': {'pipe': (6, 10), 'body': (5, 11), 'cap': (4, 12)},
    'medium': {'pipe': (5, 11), 'body': (4, 12), 'cap': (3, 13)},
    'large': {'pipe': (4, 12), 'body': (3, 13), 'cap': (2, 14)},
}
# facing -> blockstate y rotation (model drawn for south; as in the JAR).
_TURNS = {'south': {}, 'west': {'y': 90}, 'north': {'y': 180}, 'east': {'y': 270}}


def _enum(values):
    return {'type': 'enum', 'values': list(values)}


def _bool():
    return {'type': 'boolean'}


def _body(size):
    low, high = _DIMS[size]['body']
    return [([low, 4, low], [high, 16, high])]


def _cap(size, base=16, height=2):
    low, high = _DIMS[size]['cap']
    return [([low, base, low], [high, base + height, high])]


def _mouth(size):
    # The mouth faces away from the base plate (north for a south-facing model).
    low, _ = _DIMS[size]['body']
    return [([6, 6, low - 1.5], [10, 12, low])]


def _floor(size):
    low, high = _DIMS[size]['pipe']
    return ([([1, 0, 1], [15, 3, 15], 'plate'), ([low, 3, low], [high, 10, high])]
            + _body(size) + _cap(size) + _mouth(size))


def _wall(size):
    low, high = _DIMS[size]['pipe']
    return ([([1, 1, 13], [15, 15, 16], 'plate'), ([low, 3, low], [high, 9, 13])]
            + _body(size) + _cap(size) + _mouth(size))


def _selection(facing):
    """Estimated outline: base plate + column + the plate on the `facing` side."""
    plates = {
        'south': (1, 1, 13, 15, 15, 16), 'north': (1, 1, 0, 15, 15, 3),
        'west': (0, 1, 1, 3, 15, 15), 'east': (13, 1, 1, 16, 15, 15),
    }
    return [[1 / 16, 0, 1 / 16, 15 / 16, 3 / 16, 15 / 16],
            [4 / 16, 3 / 16, 4 / 16, 12 / 16, 1, 12 / 16],
            [v / 16 for v in plates[facing]]]


def steam_whistle(write, model):
    shapes = {facing: _selection(facing) for facing in FACINGS}
    write(f'data/create/horizon/block/{BLOCK}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': 3.0,
            'explosion_resistance': 6.0,
            'requires_tool': True,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': {
            'facing': _enum(FACINGS),
            'wall': _bool(),
            'powered': _bool(),
            'size': _enum(SIZES),
        },
        'default_state': {'facing': 'north', 'wall': 'false', 'powered': 'false', 'size': 'medium'},
        'shapes': {'property': 'facing', 'values': shapes},
        'collision_shapes': {'property': 'facing', 'values': shapes},
    })
    write(f'data/create/horizon/block/{EXTENSION}.json', {
        'template': 'minecraft:glass',
        'item': False,
        'properties': {
            'hardness': 3.0,
            'explosion_resistance': 6.0,
            'requires_tool': True,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': {'shape': _enum(EXTENDER_SHAPES), 'size': _enum(SIZES)},
        'default_state': {'shape': 'single', 'size': 'medium'},
        'shapes': {'property': 'size', 'values': {
            size: [[_DIMS[size]['body'][0] / 16, 0, _DIMS[size]['body'][0] / 16,
                    _DIMS[size]['body'][1] / 16, 1, _DIMS[size]['body'][1] / 16]]
            for size in SIZES}},
        'collision_shapes': {'property': 'size', 'values': {
            size: [[_DIMS[size]['body'][0] / 16, 0, _DIMS[size]['body'][0] / 16,
                    _DIMS[size]['body'][1] / 16, 1, _DIMS[size]['body'][1] / 16]]
            for size in SIZES}},
    })

    copper = material('copper_casing')
    for size in SIZES:
        for mount, boxes in (('floor', _floor(size)), ('wall', _wall(size))):
            model(f'{BLOCK}/block_{size}_{mount}', boxes, copper,
                  parts={'plate': material('brass_casing')})
            model(f'{BLOCK}/block_{size}_{mount}_powered', boxes, copper,
                  parts={'plate': material('sticker_powered')})
        low, high = _DIMS[size]['body']
        cap_low, cap_high = _DIMS[size]['cap']
        model(f'{BLOCK}/extension/{size}_single',
              [([low, 0, low], [high, 8, high]), ([cap_low, 8, cap_low], [cap_high, 10, cap_high])],
              copper)
        model(f'{BLOCK}/extension/{size}_double', [([low, 0, low], [high, 16, high])], copper)
        model(f'{BLOCK}/extension/{size}_top_rim', _cap(size), copper)
    model(f'{BLOCK}/item', _floor('medium'), copper, parts={'plate': material('brass_casing')})

    variants = {}
    for facing, turn in _TURNS.items():
        for size in SIZES:
            for wall in ('false', 'true'):
                for powered in ('false', 'true'):
                    part = f'block_{size}_{"wall" if wall == "true" else "floor"}'
                    if powered == 'true':
                        part += '_powered'
                    variants[f'facing={facing},powered={powered},size={size},wall={wall}'] = {
                        'model': f'create:block/{BLOCK}/{part}', **turn,
                    }
    write(f'assets/create/blockstates/{BLOCK}.json', {'variants': variants})

    multipart = []
    for size in SIZES:
        multipart += [
            {'when': {'shape': 'double', 'size': size},
             'apply': {'model': f'create:block/{BLOCK}/extension/{size}_top_rim'}},
            {'when': {'shape': 'single', 'size': size},
             'apply': {'model': f'create:block/{BLOCK}/extension/{size}_single'}},
            {'when': {'shape': 'double|double_connected', 'size': size},
             'apply': {'model': f'create:block/{BLOCK}/extension/{size}_double'}},
        ]
    write(f'assets/create/blockstates/{EXTENSION}.json', {'multipart': multipart})

    write(f'assets/create/models/item/{BLOCK}.json', {'parent': f'create:block/{BLOCK}/item'})
    write(f'assets/create/items/{BLOCK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BLOCK}'},
    })


def steam_whistle_recipes(write):
    write(f'data/create/recipe/crafting/kinetics/{BLOCK}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'P': 'create:golden_sheet', 'C': 'minecraft:copper_ingot'},
        'pattern': ['P', 'C'],
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
    # The original loot table drops nothing (an `minecraft:air` entry).
    write(f'data/create/loot_table/blocks/{EXTENSION}.json', {
        'type': 'minecraft:block',
        'pools': [],
        'random_sequence': f'create:blocks/{EXTENSION}',
    })


# Sound aliases for the built-in (no JAR) pack: the runtime plays `create:whistle`,
# `create:whistle_high`, `create:whistle_low` and `create:steam`. The player's JAR
# `sounds.json` replaces them with the original files.
SOUND_ALIASES = {
    'whistle': {'sounds': [{'type': 'event', 'name': 'minecraft:block.note_block.flute'}]},
    'whistle_high': {'sounds': [{'type': 'event', 'name': 'minecraft:block.note_block.flute',
                                 'pitch': 1.5}]},
    'whistle_low': {'sounds': [{'type': 'event', 'name': 'minecraft:block.note_block.flute',
                                'pitch': 0.75}]},
    'steam': {'sounds': [{'type': 'event', 'name': 'minecraft:block.fire.extinguish',
                          'volume': 0.5, 'pitch': 1.5}]},
}
