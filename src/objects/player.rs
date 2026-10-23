use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default, Clone, Component)]
pub struct Player;

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut player_vel: Single<&mut LinearVelocity, With<Player>>,
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

    let new_vel = direction.normalize_or_zero() * MOVE_SPEED;
    if player_vel.0 != new_vel {
        player_vel.0 = new_vel;
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, move_player);
    }
}
