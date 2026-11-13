# Copyright (c) 2026 create-horizon contributors
# SPDX-License-Identifier: MIT
"""Writes the CraftingPad content mod (one item, one recipe, a creative tab) as a `.hmod` tree.

CraftingPad 1.0.12 (MIT, StrikerRocker and Guntram Blohm) is one item: a crafting table directly
above a stick makes one pad, and a right click opens a crafting table's menu. The item is declared
here the way the Farmer's Delight overlay declares its items (a stick template with a stack limit
of one); the behaviour is native (`mixins/homestead/craftingpad.rs`). The texture is a generated
16x16 placeholder, not the original artwork.

    python craftingpad_content.py <out_dir>
"""
import binascii
import json
import struct
import sys
from pathlib import Path

ID = 'craftingpad'
VERSION = '1.0.12'

LANG_EN = {
    'item.craftingpad.craftingpad': 'Crafting Pad',
    'itemGroup.craftingpad': 'Crafting Pad',
}
LANG_RU = {
    'item.craftingpad.craftingpad': 'Верстак-планшет',
    'itemGroup.craftingpad': 'Crafting Pad',
}


def zlib_stored(raw):
    """A zlib stream of stored (uncompressed) deflate blocks: the bundled Python has no zlib."""
    out = bytearray(b'\x78\x01')
    for start in range(0, max(len(raw), 1), 65535):
        block = raw[start:start + 65535]
        last = 1 if start + 65535 >= len(raw) else 0
        out += struct.pack('<BHH', last, len(block), len(block) ^ 0xFFFF) + block
    a, b = 1, 0
    for byte in raw:
        a = (a + byte) % 65521
        b = (b + a) % 65521
    return bytes(out) + struct.pack('>I', (b << 16) | a)


def png(width, height, pixels):
    """A PNG from rows of RGBA tuples."""
    raw = b''.join(b'\x00' + b''.join(bytes(p) for p in row) for row in pixels)

    def chunk(kind, data):
        body = kind + data
        return struct.pack('>I', len(data)) + body + struct.pack('>I', binascii.crc32(body) & 0xFFFFFFFF)

    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib_stored(raw)) + chunk(b'IEND', b''))


def texture():
    """A crafting table seen from above: wooden frame, a 3x3 grid, a handle (placeholder art)."""
    clear = (0, 0, 0, 0)
    frame, plank, line, handle = (96, 62, 28, 255), (176, 134, 76, 255), (70, 44, 20, 255), (110, 110, 118, 255)
    rows = [[clear] * 16 for _ in range(16)]
    for y in range(2, 14):
        for x in range(2, 14):
            rows[y][x] = frame if x in (2, 13) or y in (2, 13) else plank
    for y in range(4, 12):
        for x in range(4, 12):
            rows[y][x] = line if (x - 4) % 3 == 0 or (y - 4) % 3 == 0 else plank
    for y in range(0, 3):
        rows[y][7], rows[y][8] = handle, handle
    return png(16, 16, rows)


def files():
    toml = (f'# Written by tools/craftingpad_content.py after CraftingPad-fabric-1.20.1-{VERSION}.jar (MIT licence),\n'
            '# for local use only: the item and its recipe, the behaviour is native (homestead bundle).\n'
            f'id = "{ID}"\nname = "CraftingPad"\nversion = "{VERSION}"\napi = "0.2"\nside = "both"\n'
            'capabilities = ["registries"]\n')
    return {
        'horizon-mod.toml': toml,
        f'data/{ID}/horizon/item/{ID}.json': {'template': 'minecraft:stick', 'max_stack_size': 1},
        f'data/{ID}/horizon/creative_tab/{ID}.json': {
            'title': f'itemGroup.{ID}', 'icon': f'{ID}:{ID}', 'items': [f'{ID}:{ID}']},
        f'data/{ID}/recipe/{ID}.json': {
            'type': 'minecraft:crafting_shaped', 'category': 'misc',
            'key': {'T': {'item': 'minecraft:crafting_table'}, 'S': {'item': 'minecraft:stick'}},
            'pattern': ['T', 'S'],
            'result': {'item': f'{ID}:{ID}'}, 'show_notification': True},
        f'assets/{ID}/items/{ID}.json': {'model': {'model': f'{ID}:item/{ID}', 'type': 'minecraft:model'}},
        f'assets/{ID}/models/item/{ID}.json': {'parent': 'minecraft:item/generated', 'textures': {'layer0': f'{ID}:item/{ID}'}},
        f'assets/{ID}/lang/en_us.json': LANG_EN,
        f'assets/{ID}/lang/ru_ru.json': LANG_RU,
        f'assets/{ID}/textures/item/{ID}.png': texture(),
    }


def main(out):
    out = Path(out)
    for rel, content in files().items():
        path = out / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        if isinstance(content, bytes):
            path.write_bytes(content)
        elif isinstance(content, str):
            path.write_text(content, encoding='utf-8', newline='\n')
        else:
            path.write_text(json.dumps(content, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')


if __name__ == '__main__':
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    main(sys.argv[1])
