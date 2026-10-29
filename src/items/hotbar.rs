//! The hotbar: a few slots, each holding a usable item type, whose items are used with a key or a
//! click.
//! A slot keeps its type while the owner holds none of it, so it is ready again once they pick
//! one up.

use super::{HeldBy, Inventory, Item, ItemDefinitions, ItemId, UseItem};
use bevy::prelude::*;

/// Number of slots, used with the keys 1 to 5.
pub const SLOTS: usize = 5;

/// Item type in each slot of the owner's hotbar.
#[derive(Component, Clone, Default, PartialEq, Debug)]
pub struct Hotbar(pub [Option<ItemId>; SLOTS]);

/// Puts `id` into `slot` of the owner's hotbar, or empties it with `None`. Only types with an
/// effect can be put there. A type is only in one slot at a time: if it was in another one, that
/// slot gets what `slot` held before, so moving a type from slot to slot swaps them.
#[derive(Message, Clone, Debug)]
pub struct AssignSlot {
    pub owner: Entity,
    pub slot: usize,
    pub id: Option<ItemId>,
}

/// Uses one held item of the type in `slot` of the owner's hotbar.
#[derive(Message, Clone, Copy, Debug)]
pub struct UseSlot {
    pub owner: Entity,
    pub slot: usize,
}

pub(super) fn assign_slots(
    mut assigns: MessageReader<AssignSlot>,
    mut hotbars: Query<&mut Hotbar>,
    definitions: Res<ItemDefinitions>,
) {
    for AssignSlot { owner, slot, id } in assigns.read() {
        let Ok(mut hotbar) = hotbars.get_mut(*owner) else {
            continue;
        };
        let usable = |id| definitions.get(id).is_some_and(|d| d.effect.is_some());
        if *slot >= SLOTS || id.as_ref().is_some_and(|id| !usable(id)) {
            continue;
        }
        let previous = std::mem::replace(&mut hotbar.0[*slot], id.clone());
        if id.is_some()
            && let Some(other) = (0..SLOTS).find(|&other| other != *slot && hotbar.0[other] == *id)
        {
            hotbar.0[other] = previous;
        }
    }
}

/// Turns the used slots into [`UseItem`]s for the first held item of their type.
pub(super) fn use_slots(
    mut uses: MessageReader<UseSlot>,
    mut use_item: MessageWriter<UseItem>,
    owners: Query<(&Hotbar, Option<&Inventory>)>,
    items: Query<&Item, With<HeldBy>>,
) {
    for &UseSlot { owner, slot } in uses.read() {
        let Ok((hotbar, inventory)) = owners.get(owner) else {
            continue;
        };
        let Some(Some(id)) = hotbar.0.get(slot) else {
            continue;
        };
        let held = inventory.map(Inventory::items).unwrap_or_default();
        if let Some(&item) = held
            .iter()
            .find(|&&item| items.get(item).is_ok_and(|Item(held_id)| held_id == id))
        {
            use_item.write(UseItem { item });
        }
    }
}
