//! The player's input for items besides the HUD: the reach for picking them up (clicking one
//! goes through [`crate::interact`]) and the debug key that spawns them. The hotbar's keys are
//! handled by the HUD, since they depend on the inventory window.

use super::{ItemDefinitions, spawn_on_ground};
use crate::objects::player::Player;
use bevy::{prelude::*, window::PrimaryWindow};

/// Distance in px from the player's center up to which it picks up items.
pub const PICKUP_REACH: f32 = 24.;

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
        app.add_systems(Update, spawn_item_on_key);
    }
}
