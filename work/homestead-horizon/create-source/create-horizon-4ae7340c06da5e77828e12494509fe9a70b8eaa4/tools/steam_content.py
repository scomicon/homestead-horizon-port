"""Steam Engine and Powered Shaft (Create 6.0.10) with independent fallback artwork.

The block entities, boiler and kinetic source belong to the runtime
(`mixins/create/steam_runtime.rs`, `create_port_basin::tank` for the boiler, rules in
`engine/src/core/{boiler,steam_engine}.rs`). Original models are imported only from
the player's private JAR (same model names win); the boxes below are authored for
this port.

Block states follow SteamEngineBlock (`face` floor|wall|ceiling, `facing`,
`waterlogged`) and PoweredShaftBlock (`axis`, `waterlogged`; not an item, drawn
by its block entity like a Shaft).

UNVERIFIED (recalled, not diffed against the pinned source): the engine recipe,
its hardness/resistance, and the box shapes. The engine selection box is the full
cube (the original is smaller). `reference_recipes.py` exports the real recipe
once `steam_engine` is listed in catalog.BLOCKS.

`steam_whistle_draft` is the old unwired Steam Whistle / extender declaration kept
for the whistle port; `steam_blocks` does not call it.
"""

from fallback_textures import material

ENGINE = 'steam_engine'
POWERED = 'powered_shaft'
# Model names under assets/create/models/block/steam_engine/ (mirror in catalog.PARTS).
PARTS = ('block', 'item', 'piston')

FACINGS = ('north', 'south', 'west', 'east')
FACES = ('floor', 'wall', 'ceiling')
AXES = ('x', 'y', 'z')
# Model drawn for `up`, turned to point along each direction (same table as link_content).
_TURNS = {'north': {'x': 90}, 'south': {'x': 90, 'y': 180},
          'west': {'x': 90, 'y': 270}, 'east': {'x': 90, 'y': 90}}
_Y = {'north': 0, 'east': 90, 'south': 180, 'west': 270}

LANG_EN = {ENGINE: 'Steam Engine'}
LANG_RU = {ENGINE: 'Паровой двигатель'}
# Block-only names (no item): block.create.<key>
BLOCK_LANG_EN = {POWERED: 'Powered Shaft'}
BLOCK_LANG_RU = {POWERED: 'Силовой вал'}


def _enum(values):
    return {'type': 'enum', 'values': list(values)}


def _bool():
    return {'type': 'boolean'}


def _turn(**rotation):
    return {key: value for key, value in rotation.items() if value}


def steam_blocks(write, model):
    cube = [[0, 0, 0, 1, 1, 1]]
    write(f'data/create/horizon/block/{ENGINE}.json', {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': 3.5,
            'explosion_resistance': 6.0,
            'requires_tool': True,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:copper_block',
        },
        'state_properties': {'face': _enum(FACES), 'facing': _enum(FACINGS), 'waterlogged': _bool()},
        'default_state': {'face': 'floor', 'facing': 'north', 'waterlogged': 'false'},
        'shapes': {'default': cube},
        'collision_shapes': {'default': cube},
    })
    copper = material('copper_casing')
    # Authored pointing +Y (away from the tank): base plate, housing, cylinder cap.
    body = [([1, 0, 1], [15, 3, 15]), ([3, 3, 3], [13, 12, 13]), ([5, 12, 5], [11, 16, 11])]
    model(f'{ENGINE}/block', body, copper)
    model(f'{ENGINE}/item', body, copper)
    # The piston slides along +Y by up to 0.3 block (steam_runtime::draw).
    model(f'{ENGINE}/piston', [([6, 6, 6], [10, 16, 10]), ([4, 15, 4], [12, 17, 12])],
          material('steel_axle'))
    variants = {}
    for facing in FACINGS:
        for waterlogged in ('false', 'true'):
            suffix = f',waterlogged={waterlogged}'
            model_ref = f'create:block/{ENGINE}/block'
            variants[f'face=floor,facing={facing}{suffix}'] = {'model': model_ref, **_turn(y=_Y[facing])}
            variants[f'face=ceiling,facing={facing}{suffix}'] = {
                'model': model_ref, 'x': 180, **_turn(y=_Y[facing])}
            variants[f'face=wall,facing={facing}{suffix}'] = {'model': model_ref, **_TURNS[facing]}
    write(f'assets/create/blockstates/{ENGINE}.json', {'variants': variants})
    write(f'assets/create/models/item/{ENGINE}.json', {'parent': f'create:block/{ENGINE}/item'})
    write(f'assets/create/items/{ENGINE}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{ENGINE}'},
    })

    # Powered Shaft: declared like the Shaft (axis shapes, drawn by the block
    # entity), but with no item, no model of its own and no recipe.
    shapes = {'x': [[0, .375, .375, 1, .625, .625]],
              'y': [[.375, 0, .375, .625, 1, .625]],
              'z': [[.375, .375, 0, .625, .625, 1]]}
    write(f'data/create/horizon/block/{POWERED}.json', {
        'template': 'minecraft:glass',
        'item': False,
        'properties': {'hardness': 3.0, 'explosion_resistance': 6.0, 'requires_tool': True,
                       'render_shape': 'ENTITYBLOCK_ANIMATED', 'sounds': 'minecraft:copper_block'},
        'state_properties': {'axis': _enum(AXES), 'waterlogged': _bool()},
        'default_state': {'axis': 'y', 'waterlogged': 'false'},
        'shapes': {'property': 'axis', 'values': shapes},
    })
    write(f'assets/create/blockstates/{POWERED}.json', {'variants': {'': {'model': 'create:block/shaft'}}})


def steam_recipes(write):
    # UNVERIFIED recipe: golden sheet over piston over copper casing, with a shaft.
    write(f'data/create/recipe/crafting/kinetics/{ENGINE}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'misc',
        'key': {'G': 'create:golden_sheet', 'P': 'minecraft:piston',
                'C': 'create:copper_casing', 'S': 'create:shaft'},
        'pattern': ['G ', 'PS', 'C '],
        'result': {'id': f'create:{ENGINE}'},
        'show_notification': True,
    })
    write(f'data/create/loot_table/blocks/{ENGINE}.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'conditions': [{'condition': 'minecraft:survives_explosion'}],
            'entries': [{'type': 'minecraft:item', 'name': f'create:{ENGINE}'}],
            'rolls': 1.0,
        }],
        'random_sequence': f'create:blocks/{ENGINE}',
    })
    # A Powered Shaft drops a plain Shaft.
    write(f'data/create/loot_table/blocks/{POWERED}.json', {
        'type': 'minecraft:block',
        'pools': [{
            'bonus_rolls': 0.0,
            'conditions': [{'condition': 'minecraft:survives_explosion'}],
            'entries': [{'type': 'minecraft:item', 'name': 'create:shaft'}],
            'rolls': 1.0,
        }],
        'random_sequence': f'create:blocks/{POWERED}',
    })


# ---------------------------------------------------------------------------
# Old draft for the Steam Whistle port. Not called by steam_blocks.

SIZES = ('small', 'medium', 'large')
EXTENDER_SHAPES = ('single', 'double', 'double_connected')


def steam_whistle_draft(write):
    write('data/create/horizon/block/steam_whistle.json', {
        'template': 'minecraft:copper_block', 'item': True,
        'properties': {'hardness': 3.0, 'explosion_resistance': 6.0,
                       'requires_tool': True, 'sounds': 'minecraft:copper_block'},
        'state_properties': {'facing': _enum(FACINGS), 'wall': _bool(),
                             'powered': _bool(), 'size': _enum(SIZES)},
        'default_state': {'facing': 'north', 'wall': 'false',
                          'powered': 'false', 'size': 'medium'},
    })
    write('data/create/horizon/block/steam_whistle_extension.json', {
        'template': 'minecraft:copper_block', 'item': False,
        'properties': {'hardness': 3.0, 'explosion_resistance': 6.0,
                       'requires_tool': True, 'sounds': 'minecraft:copper_block'},
        'state_properties': {'shape': _enum(EXTENDER_SHAPES), 'size': _enum(SIZES)},
        'default_state': {'shape': 'single', 'size': 'medium'},
    })
