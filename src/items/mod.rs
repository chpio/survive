//! Items lying on the ground or held by an owner (see `docs/items.md`).
//!
//! Each item is an entity with [`Item`], stackable ones also have a [`Stack`]. On the ground, an
//! item shows a sprite at its position. Picked up, it is hidden and attached to its owner through
//! [`HeldBy`], which Bevy mirrors in the owner's [`Inventory`]. Picking up a stackable item merges
//! it into a stack of the same type the owner already holds, dropping part of a stack splits it.
//! Using a held item uses it up and sends [`ItemUsed`] with its type's [`Effect`], which other
//! modules apply (e.g. combat heals). Owners with a [`hotbar::Hotbar`] use items through its
//! slots.

pub mod definitions;
pub mod hotbar;
pub mod input;

use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Key of an item type in [`ItemDefinitions`].
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

impl From<&str> for ItemId {
    fn from(id: &str) -> Self {
        Self(id.to_string())
    }
}

/// Base values of an item type, the same for all items of that type.
#[derive(Clone, PartialEq, Debug, Deserialize)]
pub struct ItemDefinition {
    pub name: String,
    /// Weight of one item in kg
    pub weight: f32,
    /// Whether items of this type are held in a [`Stack`]
    pub stackable: bool,
    /// sRGB color of the placeholder sprite
    pub color: (f32, f32, f32),
    /// What using one item does, `None` if items of this type can't be used
    #[serde(default)]
    pub effect: Option<Effect>,
}

/// What using an item does to its owner.
#[derive(Clone, Copy, PartialEq, Debug, Deserialize)]
pub enum Effect {
    /// Restores this much health, up to the maximum
    Heal(f32),
}

impl ItemDefinition {
    pub fn color(&self) -> Color {
        Color::srgb(self.color.0, self.color.1, self.color.2)
    }
}

/// All item types, read from `assets/items.ron` by [`definitions::Plugin`].
#[derive(Resource, Default, Clone, Debug)]
pub struct ItemDefinitions(pub BTreeMap<ItemId, ItemDefinition>);

impl ItemDefinitions {
    pub fn get(&self, id: &ItemId) -> Option<&ItemDefinition> {
        self.0.get(id)
    }

    /// Color of the item's sprite, magenta for unknown types.
    fn color(&self, id: &ItemId) -> Color {
        self.get(id)
            .map_or(Color::srgb(1., 0., 1.), ItemDefinition::color)
    }
}

#[derive(Component, Clone, PartialEq, Debug)]
pub struct Item(pub ItemId);

/// Number of items in a stack. Only stackable items have it, all others count as one.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Stack(pub u32);

/// The owner holding the item.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
#[relationship(relationship_target = Inventory)]
pub struct HeldBy(pub Entity);

/// The items held by an owner, in the order they were picked up.
#[derive(Component, Debug)]
#[relationship_target(relationship = HeldBy)]
pub struct Inventory(Vec<Entity>);

impl Inventory {
    pub fn items(&self) -> &[Entity] {
        &self.0
    }
}

/// Edge length of an item's placeholder sprite in px.
pub const SIZE: f32 = 8.;

/// Items on the ground lie between the map layer (z = 0) and the units (z = 10).
const Z: f32 = 5.;

/// Distance in px from the owner's center at which dropped items are put down (below it).
pub const DROP_DISTANCE: f32 = 12.;

/// Visible sprite and position of an item on the ground. Sprite picking only considers sprites
/// with `Pickable`, so the player can click it.
fn on_ground(definitions: &ItemDefinitions, id: &ItemId, position: Vec2) -> impl Bundle {
    (
        Sprite::from_color(definitions.color(id), Vec2::splat(SIZE)),
        Transform::from_translation(position.extend(Z)),
        Visibility::Inherited,
        Pickable::default(),
    )
}

/// Spawns `count` items of type `id` on the ground: one stack if the type is stackable, otherwise
/// one entity per item.
pub fn spawn_on_ground(
    commands: &mut Commands,
    definitions: &ItemDefinitions,
    id: &ItemId,
    count: u32,
    position: Vec2,
) {
    let stackable = definitions
        .get(id)
        .is_some_and(|definition| definition.stackable);
    let entities = if stackable { 1 } else { count };
    for _ in 0..entities {
        let mut item = commands.spawn((
            Name::new("Item"),
            Item(id.clone()),
            on_ground(definitions, id, position),
        ));
        if stackable {
            item.insert(Stack(count));
        }
    }
}

/// Lets `by` pick up `item`, if it lies on the ground.
#[derive(Message, Clone, Copy, Debug)]
pub struct PickUp {
    pub item: Entity,
    pub by: Entity,
}

/// Puts down `count` of the held `item` next to its owner (all of it if `count` is larger).
#[derive(Message, Clone, Copy, Debug)]
pub struct DropItem {
    pub item: Entity,
    pub count: u32,
}

/// Uses one of the held `item`: it is used up, and its owner gets the effect of its type. Items
/// without an effect stay.
#[derive(Message, Clone, Copy, Debug)]
pub struct UseItem {
    pub item: Entity,
}

/// `by` used an item with `effect`, which the modules dealing with it apply.
#[derive(Message, Clone, Copy, PartialEq, Debug)]
pub struct ItemUsed {
    pub by: Entity,
    pub effect: Effect,
}

fn pick_up(
    mut commands: Commands,
    mut pick_ups: MessageReader<PickUp>,
    mut items: Query<(&Item, Option<&mut Stack>, Has<HeldBy>)>,
    inventories: Query<&Inventory>,
    // items picked up this frame, which are only in the inventory once the commands are applied
    mut picked_up: Local<Vec<(Entity, Entity)>>,
) {
    picked_up.clear();
    for &PickUp { item, by } in pick_ups.read() {
        let Ok((Item(id), stack, held)) = items.get(item) else {
            continue;
        };
        if held || picked_up.iter().any(|&(picked, _)| picked == item) {
            continue;
        }
        let id = id.clone();
        let count = stack.map(|stack| stack.0);

        let held_items = inventories
            .get(by)
            .map(Inventory::items)
            .unwrap_or_default();
        let picked_by_owner = picked_up
            .iter()
            .filter(|&&(_, owner)| owner == by)
            .map(|&(picked, _)| picked);
        let same_stack = count.and(held_items.iter().copied().chain(picked_by_owner).find(
            |&held| {
                items
                    .get(held)
                    .is_ok_and(|(Item(held_id), stack, _)| *held_id == id && stack.is_some())
            },
        ));

        match (same_stack, count) {
            (Some(stack), Some(count)) => {
                items.get_mut(stack).unwrap().1.unwrap().0 += count;
                commands.entity(item).despawn();
            }
            _ => {
                // hidden instead of removing the sprite: Bevy draws sprites as meshes it adds
                // along with them, which stay after removing the sprite
                commands
                    .entity(item)
                    .remove::<Pickable>()
                    .insert((Visibility::Hidden, HeldBy(by)));
            }
        }
        picked_up.push((item, by));
    }
}

fn drop_items(
    mut commands: Commands,
    mut drops: MessageReader<DropItem>,
    definitions: Res<ItemDefinitions>,
    mut items: Query<(&Item, Option<&mut Stack>, &HeldBy)>,
    owners: Query<&Transform>,
) {
    for &DropItem { item, count } in drops.read() {
        let Ok((Item(id), stack, &HeldBy(owner))) = items.get_mut(item) else {
            continue;
        };
        let position = owners
            .get(owner)
            .map_or(Vec2::ZERO, |transform| transform.translation.xy())
            - Vec2::Y * DROP_DISTANCE;

        match stack {
            Some(mut stack) if count < stack.0 => {
                if count == 0 {
                    continue;
                }
                stack.0 -= count;
                let id = id.clone();
                commands.spawn((
                    Name::new("Item"),
                    Item(id.clone()),
                    Stack(count),
                    on_ground(&definitions, &id, position),
                ));
            }
            _ => {
                commands.entity(item).remove::<HeldBy>().insert(on_ground(
                    &definitions,
                    id,
                    position,
                ));
            }
        }
    }
}

fn use_items(
    mut commands: Commands,
    mut uses: MessageReader<UseItem>,
    mut used: MessageWriter<ItemUsed>,
    definitions: Res<ItemDefinitions>,
    mut items: Query<(&Item, Option<&mut Stack>, &HeldBy)>,
    // items used up this frame, which are only despawned once the commands are applied
    mut used_up: Local<Vec<Entity>>,
) {
    used_up.clear();
    for &UseItem { item } in uses.read() {
        if used_up.contains(&item) {
            continue;
        }
        let Ok((Item(id), stack, &HeldBy(by))) = items.get_mut(item) else {
            continue;
        };
        let Some(effect) = definitions.get(id).and_then(|definition| definition.effect) else {
            continue;
        };
        match stack {
            Some(mut stack) if stack.0 > 1 => stack.0 -= 1,
            _ => {
                commands.entity(item).despawn();
                used_up.push(item);
            }
        }
        used.write(ItemUsed { by, effect });
    }
}

/// Picking up, dropping and using items. The item types come from [`definitions::Plugin`], the player's
/// input from [`input::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PickUp>()
            .add_message::<DropItem>()
            .add_message::<UseItem>()
            .add_message::<ItemUsed>()
            .add_message::<hotbar::AssignSlot>()
            .add_message::<hotbar::UseSlot>()
            .init_resource::<ItemDefinitions>()
            .add_systems(
                Update,
                (
                    pick_up,
                    drop_items,
                    hotbar::assign_slots,
                    hotbar::use_slots,
                    use_items,
                )
                    .chain(),
            );
    }
}
