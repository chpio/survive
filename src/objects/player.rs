use crate::movement::{DesiredVelocity, Strength};
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default, Clone, Component)]
pub struct Player;

/// Shape of the collider and the placeholder mesh.
pub const SHAPE: Circle = Circle::new(5.);

pub const MASS: f32 = 3.;
/// Reaches full speed in 0.1 s and holds its ground against six enemies pushing together.
pub const STRENGTH: f32 = 6000.;

/// Physics components of the player.
pub fn physics() -> impl Bundle {
    (
        RigidBody::Dynamic,
        SHAPE.collider(),
        LockedAxes::ROTATION_LOCKED,
        Mass(MASS),
        NoAutoMass,
        Strength(STRENGTH),
    )
}

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut desired: Single<&mut DesiredVelocity, With<Player>>,
) {
    const MOVE_SPEED: f32 = 200.;

    let mut direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyD) {
        direction.x += 1.;
    }

    if keyboard_input.pressed(KeyCode::KeyA) {
        direction.x += -1.;
    }

    if keyboard_input.pressed(KeyCode::KeyW) {
        direction.y += 1.;
    }

    if keyboard_input.pressed(KeyCode::KeyS) {
        direction.y += -1.;
    }

    let new_velocity = direction.normalize_or_zero() * MOVE_SPEED;
    if desired.0 != new_velocity {
        desired.0 = new_velocity;
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, move_player);
    }
}
