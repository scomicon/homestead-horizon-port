"""Fluid pipe network variants (Create 6.0.10 content/fluids) with vanilla-textured fallbacks.

Wired by the generator (see docs/fluid-pipe-network.md, integration checklist). It adds only the
NEW block ids; `fluid_pipe`, `mechanical_pump` and `fluid_valve` stay in fluid_pipe_content.py with
their current states (the valve keeps `facing` + `axis_along_first` + `enabled` + `waterlogged`, the
pump `facing` + `waterlogged`, the pipe six side booleans + `waterlogged`), so placed blocks keep loading.

    glass_fluid_pipe   axis (x|y|z), alt, waterlogged          -> PipeBlock::Glass   (no item, made with the wrench)
    encased_fluid_pipe six side booleans                       -> PipeBlock::Encased (no item, made with copper casing)
    smart_fluid_pipe   face (floor|wall|ceiling), facing, waterlogged -> PipeBlock::Smart (item; filter in the block entity)

`sounds` must be a BLOCK id (harness lesson), not a sound event.
"""

from itertools import product

from fluid_pipe_content import FACES, PIPE

GLASS, ENCASED, SMART = 'glass_fluid_pipe', 'encased_fluid_pipe', 'smart_fluid_pipe'
NEW_BLOCKS = (GLASS, ENCASED, SMART)
#: Blocks that have an item (go into catalog.BLOCKS so they get the generic loot table and lang entry).
ITEM_BLOCKS = (SMART,)
AXES = ('x', 'y', 'z')
SMART_FACES = ('floor', 'wall', 'ceiling')
HORIZONTAL = ('north', 'east', 'south', 'west')
#: Blockstate rotation of a model built along Y so that it runs along the given axis (as the valve does).
ALONG = {'x': {'x': 90, 'y': 90}, 'y': {}, 'z': {'x': 90}}
LANG_EN = {GLASS: 'Glass Fluid Pipe', ENCASED: 'Encased Fluid Pipe', SMART: 'Smart Fluid Pipe'}
LANG_RU = {GLASS: 'Стеклянная жидкостная труба', ENCASED: 'Закрытая жидкостная труба',
           SMART: 'Умная жидкостная труба'}

_PHYSICAL = {'explosion_resistance': 6.0, 'requires_tool': True, 'occludes': False}


def _scaled(boxes):
    return [[c / 16 for corner in box for c in corner] for box in boxes]


def _side_arm(face):
    start, end = [4, 4, 4], [12, 12, 12]
    axis = {'down': 1, 'up': 1, 'north': 2, 'south': 2, 'west': 0, 'east': 0}[face]
    start[axis], end[axis] = (12, 16) if face in ('up', 'south', 'east') else (0, 4)
    return [start, end]


def _loot(write, name, drops):
    write(f'data/create/loot_table/blocks/{name}.json', {
        'type': 'minecraft:block',
        'pools': [{'rolls': 1, 'entries': [{'type': 'minecraft:item', 'name': drop}]} for drop in drops]})


def _item(write, name):
    write(f'assets/create/models/item/{name}.json', {'parent': f'create:block/{name}/fallback_item'})
    write(f'assets/create/items/{name}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{name}'}})


def pipes(write, model, axial_boxes=None):
    """Write blocks, blockstates, fallback models and loot of the new pipe variants."""
    # -- glass pipe: a straight tube with a window; `alt` only flips the original model variant.
    tube = {'x': [[0, 4, 4], [16, 12, 12]], 'y': [[4, 0, 4], [12, 16, 12]], 'z': [[4, 4, 0], [12, 12, 16]]}
    glass_shape = {'property': 'axis', 'values': {axis: _scaled([tube[axis]]) for axis in AXES}}
    write(f'data/create/horizon/block/{GLASS}.json', {
        'template': 'minecraft:glass',
        'properties': {**_PHYSICAL, 'hardness': 0.5, 'sounds': 'minecraft:glass', 'map_color': 15},
        'state_properties': {'axis': {'type': 'enum', 'values': list(AXES)},
                             'alt': {'type': 'boolean'}, 'waterlogged': {'type': 'boolean'}},
        'default_state': {'axis': 'y', 'alt': 'false', 'waterlogged': 'false'},
        'shapes': glass_shape, 'collision_shapes': glass_shape,
    })
    model(f'{GLASS}/fallback', [tube['y']], 'minecraft:block/glass')
    model(f'{GLASS}/fallback_item', [tube['x']], 'minecraft:block/glass')
    write(f'assets/create/blockstates/{GLASS}.json', {'variants': {
        f'axis={axis},alt={alt}': {'model': f'create:block/{GLASS}/fallback', **ALONG[axis]}
        for axis in AXES for alt in ('false', 'true')}})
    _loot(write, GLASS, [f'create:{PIPE}'])

    # -- encased pipe: casing cube plus the open sides.
    core = [[2, 2, 2], [14, 14, 14]]
    values = {}
    for selected in product((False, True), repeat=len(FACES)):
        key = ','.join(str(flag).lower() for flag in selected)
        values[key] = _scaled([core] + [_side_arm(f) for f, flag in zip(FACES, selected, strict=True) if flag])
    encased_shape = {'properties': list(FACES), 'values': values}
    write(f'data/create/horizon/block/{ENCASED}.json', {
        'template': 'minecraft:copper_block',
        'properties': {**_PHYSICAL, 'hardness': 1.0, 'sounds': 'minecraft:copper_block', 'map_color': 15},
        'state_properties': {name: {'type': 'boolean'} for name in FACES},
        'default_state': dict.fromkeys(FACES, 'false'),
        'shapes': encased_shape, 'collision_shapes': encased_shape,
    })
    model(f'{ENCASED}/fallback_core', [core], 'minecraft:block/copper_block')
    for face in FACES:
        model(f'{ENCASED}/fallback_{face}', [_side_arm(face)], 'minecraft:block/copper_block')
    write(f'assets/create/blockstates/{ENCASED}.json', {'multipart': [
        {'apply': {'model': f'create:block/{ENCASED}/fallback_core'}},
        *({'when': {face: 'true'}, 'apply': {'model': f'create:block/{ENCASED}/fallback_{face}'}} for face in FACES),
    ]})
    _loot(write, ENCASED, [f'create:{PIPE}', 'create:copper_casing'])

    # -- smart pipe: tube with a box; the tube runs along Y on a wall and perpendicular
    # to `facing` on floor / ceiling (SmartFluidPipeBlock axis rule, see pipes.rs docs).
    def smart_axis(face, facing):
        if face == 'wall':
            return 'y'
        return 'x' if facing in ('north', 'south') else 'z'

    smart_values = {f'{face},{facing}': _scaled([tube[smart_axis(face, facing)], [[3, 3, 3], [13, 13, 13]]])
                    for face, facing in product(SMART_FACES, HORIZONTAL)}
    smart_shape = {'properties': ['face', 'facing'], 'values': smart_values}
    write(f'data/create/horizon/block/{SMART}.json', {
        'template': 'minecraft:copper_block', 'item': True,
        'properties': {**_PHYSICAL, 'hardness': 3.0, 'sounds': 'minecraft:copper_block', 'map_color': 15},
        'state_properties': {'face': {'type': 'enum', 'values': list(SMART_FACES)},
                             'facing': {'type': 'enum', 'values': list(HORIZONTAL)},
                             'waterlogged': {'type': 'boolean'}},
        'default_state': {'face': 'wall', 'facing': 'north', 'waterlogged': 'false'},
        'shapes': smart_shape, 'collision_shapes': smart_shape,
    })
    model(f'{SMART}/fallback', [tube['y'], [[3, 3, 3], [13, 13, 13]]], 'minecraft:block/copper_block')
    model(f'{SMART}/fallback_item', [tube['x'], [[3, 3, 3], [13, 13, 13]]], 'minecraft:block/copper_block')
    write(f'assets/create/blockstates/{SMART}.json', {'variants': {
        f'face={face},facing={facing},waterlogged={wl}': {
            'model': f'create:block/{SMART}/fallback', **ALONG[smart_axis(face, facing)]}
        for face, facing, wl in product(SMART_FACES, HORIZONTAL, ('false', 'true'))}})
    _item(write, SMART)
    # The smart pipe has an item: its loot table comes from the generic catalog.BLOCKS loop.


def lang(extra_blocks):
    """Fill content.py's `extra_blocks` English names (blocks without an item use block.create.*)."""
    extra_blocks.update(LANG_EN)
    return extra_blocks
