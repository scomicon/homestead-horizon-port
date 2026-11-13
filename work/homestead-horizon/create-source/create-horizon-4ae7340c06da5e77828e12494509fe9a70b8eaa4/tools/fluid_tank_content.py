"""Multiblock Fluid Tank with a simple vanilla-textured fallback.

Blockstate: top/bottom (end caps of the column) and shape (plain, window or one
of the four corner windows of a 2-wide tank). The adapter keeps them in sync
with the multiblock, so the models only draw what each block shows.
"""

from itertools import product

BLOCK = 'fluid_tank'
SHAPES = ('plain', 'window', 'window_nw', 'window_sw', 'window_ne', 'window_se')
SIDES = ('down', 'up', 'north', 'south', 'west', 'east')
# Sides of the block that carry glass for each window shape; a corner shape
# has glass on its two outer sides only, the other two stay frame.
GLASS_SIDES = {
    'window': ('north', 'south', 'west', 'east'),
    'window_nw': ('north', 'west'),
    'window_sw': ('south', 'west'),
    'window_ne': ('north', 'east'),
    'window_se': ('south', 'east'),
}
# Suffix of the model for (bottom, top).
ENDS = {(True, True): 'single', (True, False): 'bottom', (False, True): 'top', (False, False): 'middle'}


def _name(shape, bottom, top):
    """Model name. The single tank keeps its original file names."""
    if shape == 'plain':
        return 'block_single'
    end = ENDS[(bottom, top)]
    if shape == 'window':
        return 'block_single_window' if end == 'single' else f'block_window_{end}'
    return f'block_{shape}_{end}'


def _window_elements(glass_sides, bottom, top):
    low = 4 if bottom else 0
    high = 12 if top else 16
    boxes = []
    if bottom:
        boxes.append(('frame', [0, 0, 0], [16, 4, 16]))
    if top:
        boxes.append(('frame', [0, 12, 0], [16, 16, 16]))
    for x, z in product((0, 14), repeat=2):
        boxes.append(('frame', [x, low, z], [x + 2, high, z + 2]))
    for side, start, end in (
        ('north', [2, low, 0], [14, high, 1]),
        ('south', [2, low, 15], [14, high, 16]),
        ('west', [0, low, 2], [1, high, 14]),
        ('east', [15, low, 2], [16, high, 14]),
    ):
        boxes.append(('glass' if side in glass_sides else 'frame', start, end))
    return [{'from': start, 'to': end, 'faces': dict.fromkeys(SIDES, {'texture': '#' + material})}
            for material, start, end in boxes]


def fluid_tank(write, model):
    write(f'data/create/horizon/block/{BLOCK}.json', {
        'template': 'minecraft:copper_block',
        'item': True,
        'properties': {'occludes': False},
        'state_properties': {'top': {'type': 'boolean'}, 'bottom': {'type': 'boolean'},
                             'shape': {'type': 'enum', 'values': SHAPES}},
        'default_state': {'top': 'true', 'bottom': 'true', 'shape': 'window'},
    })
    model(f'{BLOCK}/block_single', [([0, 0, 0], [16, 16, 16])], 'minecraft:block/copper_block')
    textures = {'frame': 'minecraft:block/copper_block', 'glass': 'minecraft:block/glass',
                'particle': '#frame'}
    written = set()
    for shape, bottom, top in product(SHAPES[1:], (True, False), (True, False)):
        name = _name(shape, bottom, top)
        if name in written:
            continue
        written.add(name)
        write(f'assets/create/models/block/{BLOCK}/{name}.json', {
            'textures': textures,
            'elements': _window_elements(GLASS_SIDES[shape], bottom, top),
        })
    write(f'assets/create/blockstates/{BLOCK}.json', {'variants': {
        f'bottom={bottom},shape={shape},top={top}': {
            'model': f'create:block/{BLOCK}/' + _name(shape, bottom == 'true', top == 'true')}
        for bottom, shape, top in product(('false', 'true'), SHAPES, ('false', 'true'))
    }})
    write(f'assets/create/models/item/{BLOCK}.json', {'parent': f'create:block/{BLOCK}/block_single_window'})
    write(f'assets/create/items/{BLOCK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BLOCK}'},
    })
