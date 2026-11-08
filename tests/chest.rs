use crate::common;
use bevy::prelude::*;
use survival::interact::{Interact, Interactable};
use survival::items::{self, Item, ItemId, Stack};
use survival::objects::chest::{self, Chest, parse_content};

#[test]
fn content_lists_items_with_counts() {
    assert_eq!(
        parse_content(" apple*3, healing_potion ,sword * 2"),
        vec![
            (ItemId::from("apple"), 3),
            (ItemId::from("healing_potion"), 1),
            (ItemId::from("sword"), 2),
        ]
    );
}

#[test]
fn content_skips_what_does_not_parse() {
    assert_eq!(
        parse_content("apple*x, , potion*2"),
        vec![(ItemId::from("potion"), 2)]
    );
    assert!(parse_content("").is_empty());
}

#[test]
fn interacting_opens_the_chest_once_and_drops_its_content() {
    let mut app = common::app_with(items::Plugin);
    // `chest::Plugin` loads the figure, which needs the asset server
    app.add_observer(chest::open_on_interact);
    let player = app.world_mut().spawn_empty().id();
    let entity = app
        .world_mut()
        .spawn(Transform::from_xyz(100., 50., 0.))
        .id();
    let mut properties = bevy_ecs_tiled::prelude::tiled::Properties::new();
    properties.insert(
        "content".to_owned(),
        bevy_ecs_tiled::prelude::tiled::PropertyValue::StringValue("apple*3, sword".to_owned()),
    );
    let mut commands = app.world_mut().commands();
    chest::insert(&mut commands, entity, &properties);
    app.world_mut().flush();
    app.update();

    for _ in 0..2 {
        app.world_mut().trigger(Interact { entity, by: player });
        app.update();
    }

    assert!(app.world().get::<Chest>(entity).unwrap().open);
    assert!(app.world().get::<Interactable>(entity).is_none());
    let mut items = app.world_mut().query::<(&Item, &Stack, &Transform)>();
    let mut dropped: Vec<_> = items
        .iter(app.world())
        .map(|(Item(id), stack, transform)| (id.0.clone(), stack.0, transform.translation.y))
        .collect();
    dropped.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(dropped.len(), 2, "{dropped:?}");
    assert_eq!((dropped[0].0.as_str(), dropped[0].1), ("apple", 3));
    assert_eq!((dropped[1].0.as_str(), dropped[1].1), ("sword", 1));
    // in front of the chest, below it on screen
    assert!(dropped.iter().all(|d| d.2 < 50.));
}
