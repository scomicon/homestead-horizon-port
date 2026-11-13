"""Linked Controller item (Create 6.0.10): item definition, model, recipe, names.

The behaviour (button frequencies, key input, the held-input network members) lives in the
runtime (`mixins/create/controller_runtime.rs`; rules in `engine/src/core/redstone_link.rs`).
The item keeps its frequencies in the runtime's `create:linked_controller_items` component.

UNVERIFIED (recalled, not diffed against the pinned source): the recipe, and the sprite (a
vanilla compass stands in until the player's JAR supplies the original model).
"""

ITEM = 'linked_controller'
ITEM_LANG_EN = {ITEM: 'Linked Controller'}
ITEM_LANG_RU = {ITEM: 'Пульт дистанционного управления'}


def controller_items(write):
    write(f'data/create/horizon/item/{ITEM}.json', {
        'template': 'minecraft:iron_ingot',
        'max_stack_size': 1,
    })
    write(f'assets/create/models/item/{ITEM}.json', {
        'parent': 'minecraft:item/generated',
        'textures': {'layer0': 'minecraft:item/compass_00'},
    })
    write(f'assets/create/items/{ITEM}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{ITEM}'},
    })


def controller_recipes(write):
    # UNVERIFIED placeholder: lever over a casing over a Redstone Link.
    write(f'data/create/recipe/crafting/controller/{ITEM}.json', {
        'type': 'minecraft:crafting_shaped',
        'category': 'redstone',
        'key': {'L': 'minecraft:lever', 'C': 'create:andesite_casing', 'R': 'create:redstone_link'},
        'pattern': ['L', 'C', 'R'],
        'result': {'id': f'create:{ITEM}'},
        'show_notification': True,
    })
