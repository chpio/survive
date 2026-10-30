//! The player's interaction with items in the world: clicking an item on the ground picks it up
//! if it is within reach and not completely out of sight. Clicks on the HUD never get here, since the HUD
//! blocks picking below it (see `docs/ui-events.md`). The hotbar's keys are handled by the HUD,
//! since they depend on the inventory window.

use super::{HeldBy, Item, ItemDefinitions, PickUp, SIZE, spawn_on_ground};
use crate::{objects::player::Player, sight::Sight};
use bevy::{
    picking::{events::PointerClick, pointer::PointerButton},
    prelude::*,
    window::PrimaryWindow,
};

/// Distance in px from the player's center up to which it picks up items.
pub const PICKUP_REACH: f32 = 24.;

fn pick_up_on_click(
    click: On<PointerClick>,
    items: Query<&Transform, (With<Item>, Without<HeldBy>)>,
    player: Single<(Entity, &Transform), With<Player>>,
    sight: Option<Res<Sight>>,
    mut pick_up: MessageWriter<PickUp>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    let Ok(item) = items.get(click.entity) else {
        return;
    };
    let (player, player_transform) = *player;
    let position = item.translation.xy();
    // the material hides an item out of sight, but picking would still hit it
    let seen = sight.is_none_or(|sight| sight.sees(position, SIZE / 2.));
    if seen && position.distance(player_transform.translation.xy()) <= PICKUP_REACH {
        pick_up.write(PickUp {
            item: click.entity,
            by: player,
        });
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
    spawn_on_ground(&mut commands, id, 1, position);
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_observer(pick_up_on_click)
            .add_systems(Update, spawn_item_on_key);
    }
}
