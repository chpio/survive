use crate::{
    combat::{Attack, Health},
    movement::{self, DesiredVelocity},
};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::time::Duration;

#[derive(Default, Clone, Component)]
pub struct Player;

/// Where the player starts, on the first map.
pub const START: Vec2 = Vec2::new(218., -386.);

/// Shape of the collider and the placeholder mesh.
pub const SHAPE: Circle = Circle::new(5.);

pub const MASS: f32 = 3.;
/// Reaches full speed in 0.1 s and holds its ground against six enemies pushing together.
pub const STRENGTH: f32 = 6000.;

/// Physics components of the player.
pub fn physics() -> impl Bundle {
    movement::body(SHAPE.collider(), MASS, STRENGTH)
}

pub const MAX_HEALTH: f32 = 100.;

/// Health and attack of the player, which hits all enemies around it twice a second.
pub fn combat() -> impl Bundle {
    (
        Health::new(MAX_HEALTH),
        Attack::new(10., SHAPE.radius + 6., 300., Duration::from_millis(500)),
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

    desired.set_if_neq(DesiredVelocity(direction.normalize_or_zero() * MOVE_SPEED));
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, move_player);
    }
}
