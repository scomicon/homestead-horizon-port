fn local_growth(game: &mut Game<'_>, view: &View, slot: usize) {
    let Some((pos, _)) = view.locate(slot) else { return; };
    let Some(BlockEntity::Mod(entity)) = game.world.block_entities.get_mut(&pos) else { return; };
    if entity.id == "create:item_vault" && entity.items.len() < 20 {
        entity.items.resize(20, None);
    }
}
