//! The player's interaction with items in the world: clicking an item on the ground picks it up
//! if it is within reach, hovering highlights it. Clicks on the HUD never get here, since the HUD
//! blocks picking below it (see `docs/ui-events.md`).

use super::{HeldBy, Item, ItemDefinitions, PickUp, spawn_on_ground};
use crate::objects::player::Player;
use bevy::{
    color::Luminance,
    picking::{
        events::{PointerClick, PointerOut, PointerOver},
        pointer::PointerButton,
    },
    prelude::*,
    window::PrimaryWindow,
};

/// Distance in px from the player's center up to which it picks up items.
pub const PICKUP_REACH: f32 = 24.;

/// How much lighter a hovered item is drawn.
const HIGHLIGHT: f32 = 0.2;

fn pick_up_on_click(
    click: On<PointerClick>,
    items: Query<&Transform, (With<Item>, Without<HeldBy>)>,
    player: Single<(Entity, &Transform), With<Player>>,
    mut pick_up: MessageWriter<PickUp>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(item) = items.get(click.entity) else {
        return;
    };
    let (player, player_transform) = *player;
    let distance = item
        .translation
        .xy()
        .distance(player_transform.translation.xy());
    if distance <= PICKUP_REACH {
        pick_up.write(PickUp {
            item: click.entity,
            by: player,
        });
    }
}

fn highlight(
    over: On<PointerOver>,
    mut items: Query<(&Item, &mut Sprite), Without<HeldBy>>,
    definitions: Res<ItemDefinitions>,
) {
    if let Ok((Item(id), mut sprite)) = items.get_mut(over.entity) {
        sprite.color = definitions.color(id).lighter(HIGHLIGHT);
    }
}

fn unhighlight(
    out: On<PointerOut>,
    mut items: Query<(&Item, &mut Sprite), Without<HeldBy>>,
    definitions: Res<ItemDefinitions>,
) {
    if let Ok((Item(id), mut sprite)) = items.get_mut(out.entity) {
        sprite.color = definitions.color(id);
    }
}

/// F spawns an item at the mouse cursor (next to the player while the cursor is outside the
/// window), each press the next type in the order of their ids.
fn spawn_item_on_key(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    definitions: Res<ItemDefinitions>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    player: Single<&Transform, With<Player>>,
    mut next: Local<usize>,
) {
    if !keyboard_input.just_pressed(KeyCode::KeyF) || definitions.0.is_empty() {
        return;
    }
    let id = definitions
        .0
        .keys()
        .nth(*next % definitions.0.len())
        .unwrap();
    *next += 1;

    let (camera, camera_transform) = *camera;
    let position = window
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
        .unwrap_or(player.translation.xy() + Vec2::X * PICKUP_REACH);
    spawn_on_ground(&mut commands, &definitions, id, 1, position);
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_observer(pick_up_on_click)
            .add_observer(highlight)
            .add_observer(unhighlight)
            .add_systems(Update, spawn_item_on_key);
    }
}
