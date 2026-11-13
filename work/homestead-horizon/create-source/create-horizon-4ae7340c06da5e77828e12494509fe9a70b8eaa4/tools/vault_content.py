"""Item Vault (ItemVaultBlock, Create 6.0.10) with a vanilla-textured fallback."""

BLOCK = 'item_vault'
AXES = ('x', 'z')


def item_vault(write, model):
    write(f'data/create/horizon/block/{BLOCK}.json', {
        'template': 'minecraft:iron_block', 'item': True,
        'properties': {'hardness': 3.0, 'explosion_resistance': 1200.0,
                       'requires_tool': True, 'sounds': 'minecraft:netherite_block'},
        'state_properties': {'axis': {'type': 'enum', 'values': list(AXES)},
                             'large': {'type': 'boolean'}},
        'default_state': {'axis': 'z', 'large': 'false'},
    })
    model(BLOCK, [([0, 0, 0], [16, 16, 16])], 'minecraft:block/iron_block')
    # The original blockstate rotates only the X axis; the large flag keeps one model.
    write(f'assets/create/blockstates/{BLOCK}.json', {'variants': {
        f'axis={axis},large={large}': {'model': f'create:block/{BLOCK}',
                                       **({'y': 90} if axis == 'x' else {})}
        for axis in AXES for large in ('false', 'true')}})
    write(f'assets/create/models/item/{BLOCK}.json', {'parent': f'create:block/{BLOCK}'})
    write(f'assets/create/items/{BLOCK}.json', {
        'model': {'type': 'minecraft:model', 'model': f'create:item/{BLOCK}'}})
    write(f'data/create/loot_table/blocks/{BLOCK}.json', {
        'type': 'minecraft:block', 'pools': [{'rolls': 1, 'entries': [
            {'type': 'minecraft:item', 'name': f'create:{BLOCK}'}]}]})


def recipe(write):
    write(f'data/create/recipe/crafting/kinetics/{BLOCK}.json', {
        'type': 'minecraft:crafting_shaped', 'category': 'misc',
        'key': {'B': '#c:plates/iron', 'C': '#c:barrels/wooden'},
        'pattern': ['B', 'C', 'B'],
        'result': {'count': 1, 'id': f'create:{BLOCK}'}})
