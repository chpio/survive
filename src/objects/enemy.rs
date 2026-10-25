use crate::objects::player;
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default, Clone, Component)]
pub struct Enemy;

#[derive(Message)]
pub struct SpawnEnemy;

fn spawn_enemy_on_key(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut spawn_enemy: MessageWriter<SpawnEnemy>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyE) {
        spawn_enemy.write(SpawnEnemy);
    }
}

fn spawn_enemy(mut spawn_enemy: MessageReader<SpawnEnemy>, mut cmds: Commands) {
    for _ in spawn_enemy.read() {
        cmds.spawn((
            Enemy,
            Name::new("Enemy"),
            Sensor,
            RigidBody::Kinematic,
            Collider::circle(5.),
            LockedAxes::ROTATION_LOCKED,
            Transform::from_xyz(50., -50., 0.),
        ));
    }
}

fn move_towards_player(
    player: Single<&Transform, With<player::Player>>,
    enemies: Query<(&Transform, &mut LinearVelocity), With<Enemy>>,
) {
    const MOVE_SPEED: f32 = 100.;

    for (enemy_pos, mut enemy_vel) in enemies {
        let direction = (player.translation.xy() - enemy_pos.translation.xy()).normalize_or_zero();
        enemy_vel.0 = direction * MOVE_SPEED;
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnEnemy>().add_systems(
            Update,
            (spawn_enemy_on_key, spawn_enemy, move_towards_player),
        );
    }
}
