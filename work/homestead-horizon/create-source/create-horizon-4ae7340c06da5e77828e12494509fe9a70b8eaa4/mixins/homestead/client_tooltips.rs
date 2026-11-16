// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT
// Client tooltips, three small behaviours rebuilt from the public descriptions of
// EnchantmentDescriptions, More Enchantment Info and Tooltip Overhaul; none of those mods' code or
// assets is used (several are copyleft or all-rights-reserved). The wording of every description
// below is ours.
//
//   * Descriptions: a short wrapped line under each enchantment line of an item tooltip. The text
//     is looked up as `enchantment.<namespace>.<path>.desc` in the selected language (so a
//     resource pack or a content mod can supply or translate it), and only then falls back to the
//     built-in English/Russian table for vanilla enchantments.
//   * More info: `Max level`, `Applies to` and `Incompatible with` under the enchantment, from the
//     enchantment registry the client already holds (`Enchantment::max_level`, `primary` /
//     `supported`, `exclusive`).
//   * Borders: the tooltip frame takes the item's rarity colour (uncommon yellow, rare aqua, epic
//     magenta); common items keep vanilla's purple frame.
//
// Both text features act on enchanted books always and on other items only while Shift is held
// (a short tooltip by default). This default is our choice; the original mods' defaults were not
// read (docs/client-tooltips.md).
//
// Everything here is client side: the tooltip hooks (`tooltip`, `tooltip_color`) are called by the
// window's `Game` while it draws or measures an item tooltip, so the player's screen exists. No
// server-thread hook is used (the reason Stack Refill and CraftingPad were dropped in stage 19).
//
// Runtime switches (not saved; reset on every start) and a test aid are the operator command
// `homestead tooltip ...`.
use super::Game;
use crate::mod_api::{NativeCommand, NativeServerPlayer};
use horizon_client::{hud::rgba, items::ItemStack, text::Text};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static DESCRIPTIONS: AtomicBool = AtomicBool::new(true);
static MORE_INFO: AtomicBool = AtomicBool::new(true);
static BORDERS: AtomicBool = AtomicBool::new(true);
/// Test aid: behave as if Shift were held (the control socket has no modifier keys).
static FORCE_SHIFT: AtomicBool = AtomicBool::new(false);
/// How many times each hook ran in this process (visible through `homestead tooltip status`).
static TEXT_CALLS: AtomicU64 = AtomicU64::new(0);
static COLOR_CALLS: AtomicU64 = AtomicU64::new(0);

/// Widest wrapped description line, in characters.
const WRAP: usize = 38;

/// `(path, text)` for vanilla enchantments; our own wording. English.
static DESC_EN: [(&str, &str); 42] = [
    ("protection", "Reduces most kinds of damage."),
    ("fire_protection", "Reduces fire damage and burn time."),
    ("feather_falling", "Reduces fall damage."),
    (
        "blast_protection",
        "Reduces explosion damage and knockback.",
    ),
    ("projectile_protection", "Reduces damage from projectiles."),
    ("respiration", "Extends breathing time underwater."),
    ("aqua_affinity", "Mines faster underwater."),
    ("thorns", "Reflects some damage back to attackers."),
    ("depth_strider", "Moves faster underwater."),
    ("frost_walker", "Freezes water into ice underfoot."),
    ("binding_curse", "The item cannot be taken off."),
    ("soul_speed", "Moves faster on soul sand and soul soil."),
    ("swift_sneak", "Sneak faster."),
    ("sharpness", "Increases melee damage."),
    ("smite", "Extra damage to undead mobs."),
    (
        "bane_of_arthropods",
        "Extra damage to arthropods and slows them.",
    ),
    ("knockback", "Knocks enemies back further."),
    ("fire_aspect", "Sets targets on fire."),
    ("looting", "Mobs drop more loot."),
    ("sweeping_edge", "Increases sweeping attack damage."),
    ("efficiency", "Mines faster."),
    ("silk_touch", "Mined blocks drop as themselves."),
    ("unbreaking", "A chance not to lose durability."),
    ("fortune", "Increases block drops."),
    ("power", "Increases arrow damage."),
    ("punch", "Knocks back targets hit by arrows."),
    ("flame", "Arrows set targets on fire."),
    ("infinity", "Shoots without using up arrows."),
    ("luck_of_the_sea", "Better treasure while fishing."),
    ("lure", "Fish bite faster."),
    ("loyalty", "The trident returns after being thrown."),
    ("impaling", "Extra trident damage to aquatic mobs."),
    ("riptide", "Launches you forward in water or rain."),
    ("channeling", "The trident calls lightning in storms."),
    ("multishot", "Fires three arrows at once."),
    ("quick_charge", "Reloads crossbows faster."),
    ("piercing", "Arrows pass through several targets."),
    ("density", "More mace smash damage per block fallen."),
    ("breach", "The mace ignores part of the target's armor."),
    ("wind_burst", "A mace smash throws you back up."),
    ("mending", "Repairs the item with experience."),
    ("vanishing_curse", "The item disappears when you die."),
];

/// The same entries in Russian.
static DESC_RU: [(&str, &str); 42] = [
    ("protection", "Снижает почти любой урон."),
    ("fire_protection", "Снижает урон от огня и время горения."),
    ("feather_falling", "Снижает урон от падения."),
    (
        "blast_protection",
        "Снижает урон и отбрасывание от взрывов.",
    ),
    ("projectile_protection", "Снижает урон от снарядов."),
    ("respiration", "Дольше дышать под водой."),
    ("aqua_affinity", "Быстрее копать под водой."),
    ("thorns", "Возвращает часть урона нападающим."),
    ("depth_strider", "Быстрее двигаться под водой."),
    ("frost_walker", "Замораживает воду под ногами."),
    ("binding_curse", "Предмет нельзя снять."),
    ("soul_speed", "Быстрее ходить по песку и почве душ."),
    ("swift_sneak", "Быстрее красться."),
    ("sharpness", "Увеличивает урон в ближнем бою."),
    ("smite", "Больше урона нежити."),
    (
        "bane_of_arthropods",
        "Больше урона членистоногим и замедление.",
    ),
    ("knockback", "Сильнее отбрасывает врагов."),
    ("fire_aspect", "Поджигает цель."),
    ("looting", "С мобов выпадает больше добычи."),
    ("sweeping_edge", "Увеличивает урон размашистой атаки."),
    ("efficiency", "Быстрее копать."),
    ("silk_touch", "Блоки выпадают сами по себе."),
    ("unbreaking", "Шанс не терять прочность."),
    ("fortune", "Увеличивает добычу с блоков."),
    ("power", "Увеличивает урон стрел."),
    ("punch", "Стрелы отбрасывают цель."),
    ("flame", "Стрелы поджигают цель."),
    ("infinity", "Стрельба без расхода стрел."),
    ("luck_of_the_sea", "Лучше добыча при рыбалке."),
    ("lure", "Рыба клюёт быстрее."),
    ("loyalty", "Трезубец возвращается после броска."),
    ("impaling", "Больше урона трезубца водным мобам."),
    ("riptide", "Бросает вас вперёд в воде или под дождём."),
    ("channeling", "Трезубец призывает молнию в грозу."),
    ("multishot", "Стреляет сразу тремя стрелами."),
    ("quick_charge", "Быстрее перезаряжает арбалет."),
    ("piercing", "Стрелы пробивают несколько целей."),
    ("density", "Больше урона булавы за каждый блок падения."),
    ("breach", "Булава игнорирует часть брони цели."),
    ("wind_burst", "Удар булавой подбрасывает вас вверх."),
    ("mending", "Чинит предмет опытом."),
    ("vanishing_curse", "Предмет исчезает при вашей смерти."),
];

/// Item groups an enchantment applies to (the last segment of a `#minecraft:enchantable/...` tag).
static GROUPS_EN: [(&str, &str); 22] = [
    ("sword", "Swords"),
    ("sharp_weapon", "Swords and axes"),
    ("melee_weapon", "Melee weapons"),
    ("weapon", "Weapons"),
    ("mace", "Maces"),
    ("trident", "Tridents"),
    ("bow", "Bows"),
    ("crossbow", "Crossbows"),
    ("fishing", "Fishing rods"),
    ("armor", "Armor"),
    ("head_armor", "Helmets"),
    ("chest_armor", "Chestplates"),
    ("leg_armor", "Leggings"),
    ("foot_armor", "Boots"),
    ("durability", "Tools, weapons and armor"),
    ("mining", "Mining tools"),
    ("mining_loot", "Mining tools"),
    ("equippable", "Wearable items"),
    ("vanishing", "Most items"),
    ("fire_aspect", "Swords and maces"),
    ("axe", "Axes"),
    ("book", "Books"),
];

static GROUPS_RU: [(&str, &str); 22] = [
    ("sword", "Мечи"),
    ("sharp_weapon", "Мечи и топоры"),
    ("melee_weapon", "Оружие ближнего боя"),
    ("weapon", "Оружие"),
    ("mace", "Булавы"),
    ("trident", "Трезубцы"),
    ("bow", "Луки"),
    ("crossbow", "Арбалеты"),
    ("fishing", "Удочки"),
    ("armor", "Броня"),
    ("head_armor", "Шлемы"),
    ("chest_armor", "Нагрудники"),
    ("leg_armor", "Поножи"),
    ("foot_armor", "Ботинки"),
    ("durability", "Инструменты, оружие и броня"),
    ("mining", "Кирки, лопаты, топоры и мотыги"),
    ("mining_loot", "Инструменты для добычи"),
    ("equippable", "Надеваемые предметы"),
    ("vanishing", "Большинство предметов"),
    ("fire_aspect", "Мечи и булавы"),
    ("axe", "Топоры"),
    ("book", "Книги"),
];

fn is_ru() -> bool {
    // `language.code` is a vanilla language key; when the language lacks it the English text is used.
    horizon_client::lang::get_or("language.code", "").starts_with("ru")
}

fn pick(table: &'static [(&'static str, &'static str)], key: &str) -> &'static str {
    table
        .iter()
        .find(|(k, _)| *k == key)
        .map_or("", |(_, v)| *v)
}

/// `minecraft:sharpness` -> `sharpness`; other namespaces have no built-in text.
fn vanilla_path(id: &str) -> Option<&str> {
    id.strip_prefix("minecraft:")
}

/// The description of enchantment `id`: the language's `enchantment.<ns>.<path>.desc` first, else
/// our built-in text, else empty.
fn description(id: &str, ru: bool) -> String {
    let key = format!("enchantment.{}.desc", id.replacen(':', ".", 1));
    let builtin = vanilla_path(id).map_or("", |p| pick(if ru { &DESC_RU } else { &DESC_EN }, p));
    horizon_client::lang::get_or(&key, builtin)
}

/// Word wrap at `WRAP` characters.
fn wrap(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > WRAP {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn roman(level: u32) -> String {
    const NAMES: [&str; 10] = ["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
    NAMES
        .get(level.wrapping_sub(1) as usize)
        .map_or_else(|| level.to_string(), |s| (*s).to_owned())
}

fn title_case(raw: &str) -> String {
    let spaced = raw.replace('_', " ");
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn group_name(spec: &str, ru: bool) -> String {
    let spec = spec.trim_start_matches('#');
    let last = spec.rsplit(['/', ':']).next().unwrap_or(spec);
    let known = pick(if ru { &GROUPS_RU } else { &GROUPS_EN }, last);
    if known.is_empty() {
        title_case(last)
    } else {
        known.to_owned()
    }
}

fn enchantment_name(game: &Game<'_>, id: &str) -> String {
    match game.enchantments().get(id) {
        Some(def) => horizon_client::lang::get_or(&def.description_key, &def.description),
        None => id.to_owned(),
    }
}

/// `Max level`, `Applies to` and `Incompatible with` for enchantment `id`.
fn info_lines(game: &Game<'_>, id: &str, ru: bool) -> Vec<String> {
    let Some(def) = game.enchantments().get(id) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let max = if ru {
        "Макс. уровень: "
    } else {
        "Max level: "
    };
    out.push(format!("{max}{}", roman(def.max_level)));
    let specs = def.primary.as_ref().unwrap_or(&def.supported);
    let mut groups: Vec<String> = specs.iter().map(|s| group_name(s, ru)).collect();
    groups.dedup();
    if !groups.is_empty() {
        let applies = if ru {
            "Применяется к: "
        } else {
            "Applies to: "
        };
        out.push(format!("{applies}{}", groups.join(", ")));
    }
    if !def.exclusive.is_empty() {
        let mut names: Vec<String> = def
            .exclusive
            .iter()
            .map(|other| enchantment_name(game, other))
            .collect();
        names.sort();
        let clash = if ru {
            "Несовместимо с: "
        } else {
            "Incompatible with: "
        };
        out.push(format!("{clash}{}", names.join(", ")));
    }
    out
}

fn shift_held(game: &Game<'_>) -> bool {
    FORCE_SHIFT.load(Ordering::Relaxed) || game.has_shift_down()
}

/// Hook `tooltip`: adds description and info lines under each enchantment line.
pub(super) fn tooltip(game: &Game<'_>, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    TEXT_CALLS.fetch_add(1, Ordering::Relaxed);
    let descriptions = DESCRIPTIONS.load(Ordering::Relaxed);
    let more = MORE_INFO.load(Ordering::Relaxed);
    if !descriptions && !more {
        return;
    }
    let stored = stack.stored_enchantments();
    let normal = stack.enchantments();
    if stored.is_empty() && normal.is_empty() {
        return;
    }
    let shift = shift_held(game);
    let ru = is_ru();
    let enchantments = game.enchantments();
    let desc_color = rgba(128, 128, 128, 255);
    let info_color = rgba(110, 150, 200, 255);
    // (index of the enchantment line, lines to put after it)
    let mut inserts: Vec<(usize, Vec<(Text, u32)>)> = Vec::new();
    for (levels, book) in [(stored, true), (normal, false)] {
        if !book && !shift {
            continue;
        }
        for (id, level) in levels {
            let name = enchantments.full_name(id, *level).0;
            let Some(at) = lines.iter().position(|(text, _)| text.plain() == name) else {
                continue;
            };
            let mut extra: Vec<(Text, u32)> = Vec::new();
            if descriptions {
                for line in wrap(&description(id, ru)) {
                    extra.push((Text::literal(format!("  {line}")), desc_color));
                }
            }
            if more {
                for line in info_lines(game, id, ru) {
                    extra.push((Text::literal(format!("  {line}")), info_color));
                }
            }
            if !extra.is_empty() {
                inserts.push((at, extra));
            }
        }
    }
    // From the last line to the first, so the indices of the earlier ones stay valid.
    inserts.sort_by(|a, b| b.0.cmp(&a.0));
    for (at, extra) in inserts {
        for (offset, line) in extra.into_iter().enumerate() {
            lines.insert(at + 1 + offset, line);
        }
    }
}

fn argb(a: u32, r: u32, g: u32, b: u32) -> u32 {
    (a << 24) | (r << 16) | (g << 8) | b
}

/// Hook `tooltip_color`: a frame in the item's rarity colour.
pub(super) fn color(
    game: &Game<'_>,
    stack: Option<&ItemStack>,
    background: &mut u32,
    border_top: &mut u32,
    border_bottom: &mut u32,
) {
    COLOR_CALLS.fetch_add(1, Ordering::Relaxed);
    if !BORDERS.load(Ordering::Relaxed) {
        return;
    }
    let Some(stack) = stack else {
        return;
    };
    let props = game.enchantments().props(&stack.item);
    let rarity =
        horizon_client::item_tooltip::rarity(stack, props.map_or("common", |p| p.rarity.as_str()));
    let (r, g, b) = match rarity {
        "uncommon" => (255, 255, 85),
        "rare" => (85, 255, 255),
        "epic" => (255, 85, 255),
        // Common items keep vanilla's purple frame.
        _ => return,
    };
    *border_top = argb(0xF0, r, g, b);
    *border_bottom = argb(0xB0, r * 3 / 5, g * 3 / 5, b * 3 / 5);
    // A faint tint of the same colour in the otherwise near-black background.
    *background = argb(0xF0, 16 + r / 24, 8 + g / 24, 16 + b / 24);
}

// ------------------------------------------------------------------- commands

static SYNTAXES: [&str; 8] = [
    "homestead tooltip status",
    "homestead tooltip show",
    "homestead tooltip color",
    "homestead tooltip descriptions <state:word>",
    "homestead tooltip info <state:word>",
    "homestead tooltip borders <state:word>",
    "homestead tooltip shift <state:word>",
    "homestead tooltip",
];

pub(super) fn register(commands: &mut Vec<NativeCommand>) {
    for syntax in SYNTAXES {
        commands.push(NativeCommand::op(syntax, 2));
    }
}

fn say(game: &mut Game<'_>, message: &str) {
    game.native_feedback(Text::literal(message));
}

fn parse_state(word: &str) -> Result<bool, String> {
    match word {
        "on" | "true" | "yes" => Ok(true),
        "off" | "false" | "no" => Ok(false),
        _ => Err("state must be on or off".into()),
    }
}

fn set(flag: &AtomicBool, name: &str, word: &str) -> Result<String, String> {
    let on = parse_state(word)?;
    flag.store(on, Ordering::Relaxed);
    Ok(format!(
        "homestead tooltip: {name} {}",
        if on { "on" } else { "off" }
    ))
}

fn flag(flag: &AtomicBool) -> &'static str {
    if flag.load(Ordering::Relaxed) {
        "on"
    } else {
        "off"
    }
}

fn run(game: &mut Game<'_>, words: &[&str]) -> Result<String, String> {
    match words {
        [] | ["status"] => Ok(format!(
            "homestead tooltip: descriptions {}, info {}, borders {}, shift forced {}, text hook calls {}, color hook calls {}",
            flag(&DESCRIPTIONS),
            flag(&MORE_INFO),
            flag(&BORDERS),
            flag(&FORCE_SHIFT),
            TEXT_CALLS.load(Ordering::Relaxed),
            COLOR_CALLS.load(Ordering::Relaxed)
        )),
        ["show"] => {
            let stack = game.held().cloned().ok_or("hold an item in the main hand")?;
            // `stack_tooltip` is the lines the client draws, native tooltip hooks included.
            let lines = game.stack_tooltip(&stack);
            let plain: Vec<String> = lines.iter().map(|(text, _)| text.plain()).collect();
            Ok(format!("homestead tooltip lines: {}", plain.join(" | ")))
        }
        ["color"] => {
            let stack = game.held().cloned().ok_or("hold an item in the main hand")?;
            let (mut background, mut top, mut bottom) = (0xf010_0010_u32, 0x5050_00ff_u32, 0x5028_007f_u32);
            // The same call the frame drawing makes, so the injected hook is the one measured.
            game.native_tooltip_color(Some(&stack), &mut background, &mut top, &mut bottom);
            Ok(format!("homestead tooltip color: background {background:08X} top {top:08X} bottom {bottom:08X}"))
        }
        ["descriptions", state] => set(&DESCRIPTIONS, "descriptions", state),
        ["info", state] => set(&MORE_INFO, "info", state),
        ["borders", state] => set(&BORDERS, "borders", state),
        ["shift", state] => set(&FORCE_SHIFT, "shift forced", state),
        _ => Err("usage: homestead tooltip status|show|color|descriptions on|off|info on|off|borders on|off|shift on|off".into()),
    }
}

pub(super) fn command(
    game: &mut Game<'_>,
    player: &NativeServerPlayer,
    line: &str,
    ran: &mut bool,
) {
    let words = line.split_whitespace().collect::<Vec<_>>();
    if words.get(..2) != Some(&["homestead", "tooltip"][..]) {
        return;
    }
    *ran = true;
    if player.permission < 2 {
        say(
            game,
            "homestead tooltip commands need operator permission 2",
        );
        return;
    }
    let message = match run(game, &words[2..]) {
        Ok(ok) => ok,
        Err(e) => format!("homestead tooltip: {e}"),
    };
    say(game, &message);
}
