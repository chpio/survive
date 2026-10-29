use crate::common;
use bevy::prelude::*;
use std::collections::BTreeMap;
use survival::items::{
    self, DROP_DISTANCE, DropItem, Effect, HeldBy, Inventory, Item, ItemDefinition,
    ItemDefinitions, ItemId, ItemUsed, PickUp, Stack, UseItem,
};

fn app() -> App {
    let mut app = common::app_with(items::Plugin);
    let definition = |stackable, effect| ItemDefinition {
        name: String::new(),
        weight: 1.,
        stackable,
        color: (1., 1., 1.),
        effect,
    };
    app.insert_resource(ItemDefinitions(BTreeMap::from([
        ("apple".into(), definition(true, None)),
        ("potion".into(), definition(true, Some(Effect::Heal(30.)))),
        ("sword".into(), definition(false, None)),
    ])));
    app
}

/// An item on the ground, a stack of `count` if `count` is given.
fn spawn_item(app: &mut App, id: &str, count: Option<u32>) -> Entity {
    let mut item =
        app.world_mut()
            .spawn((Item(id.into()), Sprite::default(), Transform::default()));
    if let Some(count) = count {
        item.insert(Stack(count));
    }
    item.id()
}

fn spawn_owner(app: &mut App, position: Vec2) -> Entity {
    common::spawn(app, (), position)
}

fn pick_up(app: &mut App, item: Entity, by: Entity) {
    app.world_mut().write_message(PickUp { item, by });
}

fn inventory(app: &App, owner: Entity) -> Vec<Entity> {
    app.world()
        .get::<Inventory>(owner)
        .map_or_else(Vec::new, |inventory| inventory.items().to_vec())
}

fn stack(app: &App, item: Entity) -> Option<u32> {
    app.world().get::<Stack>(item).map(|stack| stack.0)
}

#[test]
fn picked_up_item_is_held_and_leaves_the_ground() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let item = spawn_item(&mut app, "sword", None);

    pick_up(&mut app, item, owner);
    app.update();

    assert_eq!(app.world().get::<HeldBy>(item), Some(&HeldBy(owner)));
    assert_eq!(
        app.world().get::<Visibility>(item),
        Some(&Visibility::Hidden)
    );
    assert!(app.world().get::<Pickable>(item).is_none());
    assert_eq!(inventory(&app, owner), [item]);
}

#[test]
fn picked_up_stack_is_merged_into_held_stack() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let held = spawn_item(&mut app, "apple", Some(2));
    pick_up(&mut app, held, owner);
    app.update();

    let item = spawn_item(&mut app, "apple", Some(3));
    pick_up(&mut app, item, owner);
    app.update();

    assert_eq!(stack(&app, held), Some(5));
    assert!(app.world().get_entity(item).is_err());
    assert_eq!(inventory(&app, owner), [held]);
}

#[test]
fn stacks_picked_up_in_the_same_frame_are_merged() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let first = spawn_item(&mut app, "apple", Some(2));
    let second = spawn_item(&mut app, "apple", Some(3));

    pick_up(&mut app, first, owner);
    pick_up(&mut app, second, owner);
    // picking up the same item twice counts it once
    pick_up(&mut app, second, owner);
    app.update();

    assert_eq!(stack(&app, first), Some(5));
    assert_eq!(inventory(&app, owner), [first]);
}

#[test]
fn items_that_do_not_stack_are_held_separately() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let first = spawn_item(&mut app, "sword", None);
    let second = spawn_item(&mut app, "sword", None);

    pick_up(&mut app, first, owner);
    pick_up(&mut app, second, owner);
    app.update();

    assert_eq!(inventory(&app, owner), [first, second]);
}

#[test]
fn held_item_cannot_be_picked_up_by_someone_else() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let other = spawn_owner(&mut app, Vec2::ZERO);
    let item = spawn_item(&mut app, "sword", None);
    pick_up(&mut app, item, owner);
    app.update();

    pick_up(&mut app, item, other);
    app.update();

    assert_eq!(app.world().get::<HeldBy>(item), Some(&HeldBy(owner)));
    assert!(inventory(&app, other).is_empty());
}

#[test]
fn dropped_stack_lies_below_its_owner() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::new(40., 20.));
    let item = spawn_item(&mut app, "apple", Some(3));
    pick_up(&mut app, item, owner);
    app.update();

    app.world_mut().write_message(DropItem { item, count: 3 });
    app.update();

    assert!(app.world().get::<HeldBy>(item).is_none());
    assert_eq!(
        app.world().get::<Visibility>(item),
        Some(&Visibility::Inherited)
    );
    assert!(app.world().get::<Pickable>(item).is_some());
    let position = app.world().get::<Transform>(item).unwrap().translation.xy();
    assert_eq!(position, Vec2::new(40., 20. - DROP_DISTANCE));
    assert_eq!(stack(&app, item), Some(3));
    assert!(inventory(&app, owner).is_empty());
}

#[test]
fn dropping_part_of_a_stack_splits_it() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let item = spawn_item(&mut app, "apple", Some(5));
    pick_up(&mut app, item, owner);
    app.update();

    app.world_mut().write_message(DropItem { item, count: 2 });
    app.update();

    assert_eq!(stack(&app, item), Some(3));
    assert_eq!(inventory(&app, owner), [item]);
    let mut dropped = app
        .world_mut()
        .query_filtered::<&Stack, (With<Item>, Without<HeldBy>)>();
    let dropped: Vec<_> = dropped.iter(app.world()).map(|stack| stack.0).collect();
    assert_eq!(dropped, [2]);
}

/// An item held by `owner`, a stack of `count` if `count` is given.
fn spawn_held(app: &mut App, id: &str, count: Option<u32>, owner: Entity) -> Entity {
    let item = spawn_item(app, id, count);
    pick_up(app, item, owner);
    app.update();
    item
}

fn use_item(app: &mut App, item: Entity) {
    app.world_mut().write_message(UseItem { item });
}

/// The items used in the last update.
fn used(app: &App) -> Vec<ItemUsed> {
    app.world()
        .resource::<Messages<ItemUsed>>()
        .iter_current_update_messages()
        .copied()
        .collect()
}

#[test]
fn using_one_of_a_stack_reduces_it_and_reports_the_effect() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let potion = spawn_held(&mut app, "potion", Some(3), owner);

    use_item(&mut app, potion);
    app.update();

    assert_eq!(stack(&app, potion), Some(2));
    assert_eq!(
        used(&app),
        [ItemUsed {
            by: owner,
            effect: Effect::Heal(30.)
        }]
    );
}

#[test]
fn using_the_last_item_uses_it_up() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let potion = spawn_held(&mut app, "potion", Some(1), owner);

    // twice in the same frame, but there is only one
    use_item(&mut app, potion);
    use_item(&mut app, potion);
    app.update();

    assert!(app.world().get_entity(potion).is_err());
    assert_eq!(inventory(&app, owner), []);
    assert_eq!(used(&app).len(), 1);
}

#[test]
fn items_without_effect_are_not_used() {
    let mut app = app();
    let owner = spawn_owner(&mut app, Vec2::ZERO);
    let apple = spawn_held(&mut app, "apple", Some(2), owner);

    use_item(&mut app, apple);
    app.update();

    assert_eq!(stack(&app, apple), Some(2));
    assert_eq!(used(&app), []);
}

#[test]
fn items_on_the_ground_cannot_be_used() {
    let mut app = app();
    let potion = spawn_item(&mut app, "potion", Some(1));

    use_item(&mut app, potion);
    app.update();

    assert_eq!(stack(&app, potion), Some(1));
    assert_eq!(used(&app), []);
}

#[test]
fn definitions_file_parses() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/items.ron");
    let definitions: BTreeMap<ItemId, ItemDefinition> =
        ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        definitions[&ItemId::from("healing_potion")].effect,
        Some(Effect::Heal(30.))
    );
    assert_eq!(definitions[&ItemId::from("sword")].effect, None);
}
