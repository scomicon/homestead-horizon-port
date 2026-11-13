# Copyright (c) 2026 create-horizon contributors
# SPDX-License-Identifier: MIT
"""Schematic and Quill item (the Schematic and Empty Schematic items live in schematic_content.py).

Data only; the behaviour is mixins/create/schematic_tool_runtime.rs (see docs/schematic-tools.md).
Wire-up (content.py / catalog.py) is described in mixins/create/fragments/schematic_tools.md.
"""

QUILL = 'schematic_and_quill'
ITEMS = (QUILL,)
LANG_EN = {QUILL: 'Schematic and Quill'}
LANG_RU = {QUILL: 'Схема и перо'}
_SPRITE = 'feather'


def schematic_tool_items(write):
    write(f'data/create/horizon/item/{QUILL}.json', {
        'template': 'minecraft:iron_ingot',
        'max_stack_size': 1,
    })
    write(f'assets/create/models/item/{QUILL}.json', {
        'parent': 'minecraft:item/generated',
        'textures': {'layer0': f'minecraft:item/{_SPRITE}'},
    })
    write(f'assets/create/items/{QUILL}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{QUILL}'},
    })


def schematic_tool_recipes(write):
    # UNVERIFIED placeholder (the original recipe was not read).
    write(f'data/create/recipe/crafting/schematics/{QUILL}.json', {
        'type': 'minecraft:crafting_shapeless',
        'category': 'misc',
        'ingredients': ['create:empty_schematic', 'minecraft:feather'],
        'result': {'id': f'create:{QUILL}'},
    })
