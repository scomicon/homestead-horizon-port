// Copyright (c) The Create Team / The Creators of Create
// Copyright (c) 2026 create-horizon contributors
// SPDX-License-Identifier: MIT

// Package item (`PackageItem`): the bridge between game item stacks and the pure
// logistics core (`create_engine::package`).
//
// A package is an ordinary item stack of one of the `cardboard_package_*` items.
// Its payload lives in the stack's `minecraft:custom_data` component:
//   Items        list of {Slot, id, count, components}  (the nine package slots)
//   Address      string                                  (`PACKAGE_ADDRESS`)
//   OrderId, LinkIndex, FragmentIndex  ints              (`PACKAGE_ORDER_DATA`)
//   IsFinalLink, IsFinal               bytes
// The Create data components (`create:package_contents` ...) are not registered
// here, so this layout is the port's own; stacks therefore compare equal exactly
// when their payloads are equal. `PackageOrderWithCrafts` (order context) is not
// persisted (see docs/logistics.md).
//
// Other items are mapped to core stacks with `components` = the canonical text of
// their saved component patch; `Registry` remembers the game stack behind every
// such key so a core result can be turned back into game stacks.

use crate::items::ItemStack;
use create_engine::package::{self as cp, Package, PackageOrderData};
use horizon_items::{
    item_data::{canon_tag, encode_components},
    item_nbt::{decode_items, encode_items},
};
use horizon_world::nbt::{Compound, Tag};
use std::collections::HashMap;

pub(super) const PREFIX: &str = "create:cardboard_package_";
const CUSTOM_DATA: &str = "minecraft:custom_data";
/// The four standard styles (`PackageStyles`); rare creator styles are not ported.
pub(super) const STYLES: [&str; 4] = [
    "create:cardboard_package_12x12",
    "create:cardboard_package_10x12",
    "create:cardboard_package_10x8",
    "create:cardboard_package_12x10",
];

pub(super) fn is_package(item: &str) -> bool {
    item.starts_with(PREFIX)
}

/// `!Item.canFitInsideContainerItems()` for the items this port knows about.
fn bulky(item: &str) -> bool {
    is_package(item) || item.ends_with("shulker_box")
}

/// Game stacks behind core item keys (id, component text).
#[derive(Default)]
pub(super) struct Registry(HashMap<(String, String), ItemStack>);

fn get<'a>(nbt: &'a Compound, key: &str) -> Option<&'a Tag> {
    nbt.get(&key.into())
}

fn int(nbt: &Compound, key: &str) -> Option<i32> {
    match get(nbt, key) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

fn flag(nbt: &Compound, key: &str) -> bool {
    int(nbt, key).is_some_and(|v| v != 0)
}

/// Game stack -> core stack. `max` gives the stack limit of an item id.
pub(super) fn to_core(reg: &mut Registry, stack: &ItemStack, max: &dyn Fn(&str) -> u32) -> cp::ItemStack {
    let id: &str = &stack.item;
    if is_package(id) {
        let mut out = cp::ItemStack::new(id, stack.count).with_max(1).bulky();
        out.package = Some(Box::new(read_package(reg, stack, max)));
        return out;
    }
    let mut single = stack.clone();
    single.count = 1;
    let components = encode_components(&single).map(|tag| canon_tag(&tag)).unwrap_or_default();
    reg.0.insert((id.to_owned(), components.clone()), single);
    let mut out = cp::ItemStack::new(id, stack.count).with_components(&components).with_max(max(id));
    if bulky(id) {
        out = out.bulky();
    }
    out
}

/// Core stack -> game stack (packages are rebuilt from their payload).
pub(super) fn to_game(reg: &Registry, stack: &cp::ItemStack) -> ItemStack {
    if let Some(package) = &stack.package {
        let mut out = ItemStack::new(&stack.key.id, stack.count);
        write_package(reg, &mut out, package);
        return out;
    }
    match reg.0.get(&(stack.key.id.clone(), stack.key.components.clone())) {
        Some(template) => {
            let mut out = template.clone();
            out.count = stack.count;
            out
        }
        None => ItemStack::new(&stack.key.id, stack.count),
    }
}

fn read_package(reg: &mut Registry, stack: &ItemStack, max: &dyn Fn(&str) -> u32) -> Package {
    let data = stack.custom_data().cloned().unwrap_or_else(Compound::new);
    let mut contents: Vec<Option<cp::ItemStack>> = decode_items(get(&data, "Items"), cp::SLOTS)
        .iter()
        .map(|slot| slot.as_ref().map(|s| to_core(reg, s, max)))
        .collect();
    contents.resize(cp::SLOTS, None);
    let address = match get(&data, "Address") {
        Some(Tag::String(text)) => text.to_string().ok(),
        _ => None,
    };
    let order_data = int(&data, "OrderId").map(|order_id| PackageOrderData {
        order_id,
        link_index: int(&data, "LinkIndex").unwrap_or(-1),
        is_final_link: flag(&data, "IsFinalLink"),
        fragment_index: int(&data, "FragmentIndex").unwrap_or(-1),
        is_final: flag(&data, "IsFinal"),
        order_context: None,
    });
    Package { contents, address, order_data, order_context: None }
}

fn write_package(reg: &Registry, stack: &mut ItemStack, package: &Package) {
    let slots: Vec<Option<ItemStack>> =
        package.contents.iter().map(|slot| slot.as_ref().map(|s| to_game(reg, s))).collect();
    let mut data = Compound::new();
    data.insert("Items".into(), encode_items(&slots));
    if let Some(address) = &package.address {
        data.insert("Address".into(), Tag::String(address.as_str().into()));
    }
    if let Some(order) = &package.order_data {
        data.insert("OrderId".into(), Tag::Int(order.order_id));
        data.insert("LinkIndex".into(), Tag::Int(order.link_index));
        data.insert("FragmentIndex".into(), Tag::Int(order.fragment_index));
        data.insert("IsFinalLink".into(), Tag::Byte(i8::from(order.is_final_link)));
        data.insert("IsFinal".into(), Tag::Byte(i8::from(order.is_final)));
    }
    stack.data_mut().set_other(CUSTOM_DATA, Some(Tag::Compound(data)));
}

/// The address written on a package stack ("" when none).
#[allow(dead_code, reason = "read helper for tooling and tooltips")]
pub(super) fn address(stack: &ItemStack) -> String {
    stack
        .custom_data()
        .and_then(|data| match get(data, "Address") {
            Some(Tag::String(text)) => text.to_string().ok(),
            _ => None,
        })
        .unwrap_or_default()
}
