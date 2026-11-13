"""Redstone switches and the Nixie tube: blocks, items, recipes, loot and
independent fallback artwork.

Runtime behaviour belongs to `mixins/create/switch_runtime.rs` (module
`create_port_switch`); the rules live in `engine/src/core/redstone_switches.rs`
and `engine/src/core/nixie.rs`. Models are authored here from fallback tiles
(`create:block/fallback/*`, see `fallback_textures.material`); an imported
original of the same model name wins.

Block states (alphabetical order is the blockstate key order):

  pulse_repeater / pulse_extender / pulse_timer
      facing (north south west east; the INPUT side, as a vanilla repeater),
      inverted, powered (delayed input), powering (output before inversion)
  powered_toggle_latch   facing, powered, powering
  powered_latch          facing, powered, powered_side, powering
  analog_lever           facing (6, the side the lever points out of), power (0..15)
  redstone_contact       facing (6), powered
  nixie_tube             mount (floor_<dir>, ceiling_<dir>, wall_<dir>)

The recipes are estimates (the pinned Create source was not diffed); shapes,
hardness and models are ours.
"""

import itertools
import math

from fallback_textures import material

PULSE_REPEATER = 'pulse_repeater'
PULSE_EXTENDER = 'pulse_extender'
PULSE_TIMER = 'pulse_timer'
POWERED_LATCH = 'powered_latch'
TOGGLE_LATCH = 'powered_toggle_latch'
ANALOG_LEVER = 'analog_lever'
REDSTONE_CONTACT = 'redstone_contact'
NIXIE = 'nixie_tube'

DIODES = (PULSE_REPEATER, PULSE_EXTENDER, PULSE_TIMER)
BLOCKS = (*DIODES, POWERED_LATCH, TOGGLE_LATCH, ANALOG_LEVER, REDSTONE_CONTACT, NIXIE)

_LEVER_BUCKETS = 5
# Model names under assets/create/models/block/<block>/ (also listed in catalog.PARTS).
PARTS = {
    **{name: ('off', 'on', 'item') for name in (*DIODES, POWERED_LATCH, TOGGLE_LATCH, REDSTONE_CONTACT)},
    ANALOG_LEVER: (*[f'handle_{i}' for i in range(_LEVER_BUCKETS)], 'item'),
    # The JAR's nixie_tube/block.json uses the porting_lib composite loader, which the asset
    # importer rejects (and the rejection aborts the whole import), so the Nixie keeps its
    # generated model and is not listed for import.
    NIXIE: (),
}

LANG_EN = {
    PULSE_REPEATER: 'Pulse Repeater',
    PULSE_EXTENDER: 'Pulse Extender',
    PULSE_TIMER: 'Pulse Timer',
    POWERED_LATCH: 'Powered Latch',
    TOGGLE_LATCH: 'Powered Toggle Latch',
    ANALOG_LEVER: 'Analog Lever',
    REDSTONE_CONTACT: 'Redstone Contact',
    NIXIE: 'Nixie Tube',
}
LANG_RU = {
    PULSE_REPEATER: 'Импульсный повторитель',
    PULSE_EXTENDER: 'Импульсный удлинитель',
    PULSE_TIMER: 'Импульсный таймер',
    POWERED_LATCH: 'Питаемая защёлка',
    TOGGLE_LATCH: 'Питаемая защёлка-переключатель',
    ANALOG_LEVER: 'Аналоговый рычаг',
    REDSTONE_CONTACT: 'Контакт красного камня',
    NIXIE: 'Газоразрядная лампа',
}

_FACES = ('down', 'up', 'north', 'south', 'west', 'east')
_HORIZONTAL = ('north', 'south', 'west', 'east')
# Model drawn for `south` (input side +z): blockstate y per facing.
_DIODE_Y = {'south': 0, 'west': 90, 'north': 180, 'east': 270}
# Model drawn for `up`; rotation that points it out of each face.
_TURNS = {'down': {'x': 180}, 'up': {}, 'north': {'x': 90}, 'south': {'x': 90, 'y': 180},
          'west': {'x': 90, 'y': 270}, 'east': {'x': 90, 'y': 90}}
# Nixie mount -> blockstate (x, y). Keep in sync with `mount_turn` in switch_runtime.rs.
_WALL_Y = {'north': 0, 'east': 90, 'south': 180, 'west': 270}
_MOUNT_TURNS = {
    **{f'floor_{d}': (0, y) for d, y in _DIODE_Y.items()},
    **{f'ceiling_{d}': (180, y) for d, y in _DIODE_Y.items()},
    **{f'wall_{d}': (90, y) for d, y in _WALL_Y.items()},
}


def _bool():
    return {'type': 'boolean'}


def _enum(values):
    return {'type': 'enum', 'values': list(values)}


def _turn_point(point, x_degrees, y_degrees):
    """Blockstate rotation: about X by -x, then about Y by -y, around the block centre."""
    px, py, pz = (v - 0.5 for v in point)
    a = math.radians(-x_degrees)
    py, pz = py * math.cos(a) - pz * math.sin(a), py * math.sin(a) + pz * math.cos(a)
    b = math.radians(-y_degrees)
    px, pz = px * math.cos(b) + pz * math.sin(b), -px * math.sin(b) + pz * math.cos(b)
    return [round(v + 0.5, 6) for v in (px, py, pz)]


def _turned_box(box16, x_degrees, y_degrees):
    x0, y0, z0, x1, y1, z1 = (v / 16 for v in box16)
    corners = [_turn_point((x, y, z), x_degrees, y_degrees)
               for x in (x0, x1) for y in (y0, y1) for z in (z0, z1)]
    return [min(c[i] for c in corners) for i in range(3)] + [max(c[i] for c in corners) for i in range(3)]


def _declare(write, name, *, props, defaults, shapes, hardness=1.0, template_extra=None):
    declaration = {
        'template': 'minecraft:glass',
        'item': True,
        'properties': {
            'hardness': hardness,
            'explosion_resistance': 3.0,
            'requires_tool': False,
            'occludes': False,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:andesite',
        },
        'state_properties': props,
        'default_state': defaults,
        'shapes': shapes,
        'collision_shapes': shapes,
    }
    if template_extra:
        declaration.update(template_extra)
    write(f'data/create/horizon/block/{name}.json', declaration)


def _blockstate(write, name, props, choose):
    """One variant per property combination; `choose(state)` returns the variant dict."""
    keys = sorted(props)
    variants = {}
    for combo in itertools.product(*[props[k]['values'] if props[k]['type'] == 'enum'
                                     else ['false', 'true'] for k in keys]):
        state = dict(zip(keys, combo))
        variants[','.join(f'{k}={v}' for k, v in state.items())] = choose(state)
    write(f'assets/create/blockstates/{name}.json', {'variants': variants})


def _item(write, name, model_name):
    write(f'assets/create/models/item/{name}.json', {'parent': f'create:block/{name}/{model_name}'})
    write(f'assets/create/items/{name}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{name}'},
    })


def _plate(model, name, part, base, lamp, extra=()):
    """A 2 px plate drawn for `south`: input bar on +z, output bar on -z, lamp in the middle."""
    boxes = [([0, 0, 0], [16, 2, 16]),
             ([6, 2, 11], [10, 3, 16], 'bar'), ([6, 2, 0], [10, 3, 5], 'bar'),
             ([5, 2, 5], [11, 4, 11], 'lamp'), *extra]
    model(f'{name}/{part}', boxes, material(base),
          parts={'bar': material('steel_axle'), 'lamp': material(lamp), 'side': material('brass_casing')})


def _diode_like(write, model, name, base, accent):
    shapes = {'default': [[0, 0, 0, 1, 2 / 16, 1]]}
    props = {'facing': _enum(_HORIZONTAL), 'powered': _bool(), 'powering': _bool()}
    defaults = {'facing': 'north', 'powered': 'false', 'powering': 'false'}
    side_bars = ()
    if name == POWERED_LATCH:
        props['powered_side'] = _bool()
        defaults['powered_side'] = 'false'
        side_bars = (([0, 2, 6], [4, 3, 10], 'side'), ([12, 2, 6], [16, 3, 10], 'side'))
    if name in DIODES:
        props['inverted'] = _bool()
        defaults['inverted'] = 'false'
    _declare(write, name, props=props, defaults=defaults, shapes=shapes)
    _plate(model, name, 'off', base, 'chain_indicator', side_bars)
    _plate(model, name, 'on', base, 'chain_indicator_powered', side_bars)
    _plate(model, name, 'item', base, accent, side_bars)

    def choose(state):
        lit = (state['powering'] == 'true') != (state.get('inverted', 'false') == 'true')
        variant = {'model': f'create:block/{name}/{"on" if lit else "off"}'}
        if _DIODE_Y[state['facing']]:
            variant['y'] = _DIODE_Y[state['facing']]
        return variant

    _blockstate(write, name, props, choose)
    _item(write, name, 'item')


def _lever(write, model):
    base = [([4, 0, 4], [12, 3, 12], 'all')]
    box = [4, 0, 4, 12, 12, 12]
    shapes = {'property': 'facing', 'values': {
        face: [_turned_box(box, turn.get('x', 0), turn.get('y', 0))] for face, turn in _TURNS.items()}}
    props = {'facing': _enum(_FACES), 'power': _enum([str(i) for i in range(16)])}
    _declare(write, ANALOG_LEVER, props=props, defaults={'facing': 'up', 'power': '0'}, shapes=shapes)
    for bucket in range(_LEVER_BUCKETS):
        angle = -45.0 + 22.5 * bucket
        name = f'{ANALOG_LEVER}/handle_{bucket}'
        # Handle and tip tilt about the base pivot (x axis), the base stays put.
        write(f'assets/create/models/block/{name}.json', {
            'textures': {'all': material('andesite_casing'), 'handle': material('brass_casing'),
                         'particle': material('andesite_casing')},
            'elements': [
                _element([4, 0, 4], [12, 3, 12], 'all'),
                {**_element([7, 3, 7], [9, 11, 9], 'handle'),
                 'rotation': {'origin': [8, 3, 8], 'axis': 'x', 'angle': angle}},
                {**_element([6, 11, 6], [10, 13, 10], 'handle'),
                 'rotation': {'origin': [8, 3, 8], 'axis': 'x', 'angle': angle}},
            ],
        })
    model(f'{ANALOG_LEVER}/item', base + [([7, 3, 7], [9, 11, 9], 'handle'), ([6, 11, 6], [10, 13, 10], 'handle')],
          material('andesite_casing'), parts={'handle': material('brass_casing')})

    def choose(state):
        power = int(state['power'])
        bucket = 0 if power <= 2 else 1 if power <= 5 else 2 if power <= 9 else 3 if power <= 12 else 4
        return {'model': f'create:block/{ANALOG_LEVER}/handle_{bucket}', **_TURNS[state['facing']]}

    _blockstate(write, ANALOG_LEVER, props, choose)
    _item(write, ANALOG_LEVER, 'item')


def _element(start, end, slot):
    face = {'texture': '#' + slot, 'uv': [0, 0, 16, 16]}
    return {'from': start, 'to': end,
            'faces': {side: dict(face) for side in ('down', 'up', 'north', 'south', 'west', 'east')}}


def _contact(write, model):
    shapes = {'default': [[0, 0, 0, 1, 1, 1]]}
    props = {'facing': _enum(_FACES), 'powered': _bool()}
    _declare(write, REDSTONE_CONTACT, props=props, defaults={'facing': 'north', 'powered': 'false'},
             shapes=shapes, hardness=1.5)
    for part, pad in (('off', 'brass_casing'), ('on', 'chain_indicator_powered'), ('item', 'brass_casing')):
        # Drawn for `up`: the contact pad is the top face.
        model(f'{REDSTONE_CONTACT}/{part}', [([0, 0, 0], [16, 15, 16]), ([2, 15, 2], [14, 16, 14], 'pad')],
              material('andesite_casing'), parts={'pad': material(pad)})

    def choose(state):
        part = 'on' if state['powered'] == 'true' else 'off'
        return {'model': f'create:block/{REDSTONE_CONTACT}/{part}', **_TURNS[state['facing']]}

    _blockstate(write, REDSTONE_CONTACT, props, choose)
    _item(write, REDSTONE_CONTACT, 'item')


def _nixie(write, model):
    box = [3, 0, 3, 13, 11, 13]
    shapes = {'property': 'mount', 'values': {
        mount: [_turned_box(box, x, y)] for mount, (x, y) in _MOUNT_TURNS.items()}}
    props = {'mount': _enum(_MOUNT_TURNS)}
    _declare(write, NIXIE, props=props, defaults={'mount': 'floor_south'}, shapes=shapes)
    boxes = [([3, 0, 3], [13, 10, 13]), ([4, 10, 4], [12, 11, 12], 'plate')]
    for part in ('block', 'item'):
        model(f'{NIXIE}/{part}', boxes, material('copper_casing'), parts={'plate': material('andesite_casing')})

    def choose(state):
        x, y = _MOUNT_TURNS[state['mount']]
        turn = {}
        if x:
            turn['x'] = x
        if y:
            turn['y'] = y
        return {'model': f'create:block/{NIXIE}/block', **turn}

    _blockstate(write, NIXIE, props, choose)
    _item(write, NIXIE, 'item')


def switches(write, model):
    _diode_like(write, model, PULSE_REPEATER, 'brass_casing', 'chain_indicator')
    _diode_like(write, model, PULSE_EXTENDER, 'andesite_casing', 'chain_indicator')
    _diode_like(write, model, PULSE_TIMER, 'copper_casing', 'chain_indicator')
    _diode_like(write, model, POWERED_LATCH, 'andesite_casing', 'chain_indicator')
    _diode_like(write, model, TOGGLE_LATCH, 'brass_casing', 'chain_indicator')
    _lever(write, model)
    _contact(write, model)
    _nixie(write, model)


_RECIPES = {
    PULSE_REPEATER: (['R', 'E', 'A'], {'R': 'minecraft:redstone', 'E': 'create:electron_tube',
                                       'A': 'create:andesite_alloy'}, 1),
    PULSE_EXTENDER: (['R', 'E', 'B'], {'R': 'minecraft:redstone', 'E': 'create:electron_tube',
                                       'B': 'create:brass_sheet'}, 1),
    PULSE_TIMER: (['R', 'E', 'C'], {'R': 'minecraft:redstone', 'E': 'create:electron_tube',
                                    'C': 'create:copper_sheet'}, 1),
    POWERED_LATCH: (['T', 'E'], {'T': 'minecraft:redstone_torch', 'E': 'create:electron_tube'}, 1),
    TOGGLE_LATCH: (['L', 'E'], {'L': 'minecraft:lever', 'E': 'create:electron_tube'}, 1),
    ANALOG_LEVER: (['L', 'C'], {'L': 'minecraft:lever', 'C': 'create:andesite_casing'}, 1),
    REDSTONE_CONTACT: (['I', 'R', 'I'], {'I': 'create:iron_sheet', 'R': 'minecraft:redstone'}, 2),
    NIXIE: (['E', 'G'], {'E': 'create:electron_tube', 'G': 'minecraft:glass'}, 1),
}


def switch_recipes(write):
    for name, (pattern, key, count) in _RECIPES.items():
        write(f'data/create/recipe/crafting/logistics/{name}.json', {
            'type': 'minecraft:crafting_shaped',
            'category': 'redstone',
            'key': key,
            'pattern': pattern,
            'result': {'id': f'create:{name}', 'count': count},
            'show_notification': True,
        })
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
