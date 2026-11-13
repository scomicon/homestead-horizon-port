"""Universal Graves (LGPLv3, Fabric, server side) content for the native port.

The original is a server-only mod: its grave is a skull-like block shown through a hologram, with no
assets. Horizon needs a real block, so this writes a very small content mod `universal_graves` with
one block, `universal_graves:grave`, that the native module `homestead_graves`
(mixins/homestead/graves.rs) places when a player dies, gives a block entity of 54 item slots and
protects for 900 s:

* `horizon-mod.toml` (id `universal_graves`),
* the block `data/universal_graves/horizon/block/grave.json` on the barrel template (a barrel's use opens
  the block's mod menu; the container and its generic 9x6 menu come from the native module), no item:
  a grave is never crafted or found in the creative inventory,
* a blockstate and a cube model (placeholder look: polished blackstone bricks),
* an empty loot table (the engine drops the container's items when the block breaks, the grave itself
  drops nothing),
* the English and Russian block names.

    python graves_content.py <out dir>

The out dir is zipped flat into stage4/extra-mods by stage4/Build-Graves.ps1.
"""
import json
import sys
from pathlib import Path

NS = 'universal_graves'
BLOCK = f'{NS}:grave'


def _write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(value, str):
        path.write_text(value, encoding='utf-8')
    else:
        path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')


def build(out):
    out = Path(out)
    _write(out / 'horizon-mod.toml', (
        '# Written by tools/graves_content.py for the native port of Universal Graves (LGPLv3).\n'
        f'id = "{NS}"\n'
        'name = "Universal Graves"\n'
        'version = "3.0.3"\n'
        'api = "0.2"\n'
        'side = "both"\n'
        'capabilities = ["registries"]\n'
    ))
    data = out / 'data' / NS
    _write(data / 'horizon/block/grave.json', {
        # A barrel template: its use opens the block's mod menu (see farmersdelight:cooking_pot).
        'template': 'minecraft:barrel',
        'item': False,
        'properties': {
            'hardness': 2.0,
            'explosion_resistance': 6.0,
            'requires_tool': False,
            'occludes': True,
            'render_shape': 'MODEL',
            'sounds': 'minecraft:stone',
        },
    })
    _write(data / 'loot_table/blocks/grave.json', {
        'type': 'minecraft:block',
        'pools': [],
        'random_sequence': f'{NS}:blocks/grave',
    })
    assets = out / 'assets' / NS
    _write(assets / 'blockstates/grave.json', {'variants': {'': {'model': f'{NS}:block/grave'}}})
    _write(assets / 'models/block/grave.json', {
        'parent': 'minecraft:block/cube_all',
        'textures': {'all': 'minecraft:block/polished_blackstone_bricks'},
    })
    _write(assets / 'lang/en_us.json', {f'block.{NS}.grave': 'Grave'})
    _write(assets / 'lang/ru_ru.json', {f'block.{NS}.grave': 'Могила'})
    print('universal graves: block, blockstate, model, loot table, names')


if __name__ == '__main__':
    build(sys.argv[1])
