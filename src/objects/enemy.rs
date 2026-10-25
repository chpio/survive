use crate::{flowfield::FlowField, objects::player};
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default, Clone, Component)]
pub struct Enemy;

/// Shape of the collider and the placeholder mesh.
const SHAPE: Circle = Circle::new(5.);

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

fn spawn_enemy(
    mut spawn_enemy: MessageReader<SpawnEnemy>,
    mut cmds: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for _ in spawn_enemy.read() {
        cmds.spawn((
            Enemy,
            Name::new("Enemy"),
            Sensor,
            RigidBody::Kinematic,
            SHAPE.collider(),
            LockedAxes::ROTATION_LOCKED,
            Mesh2d(meshes.add(SHAPE)),
            MeshMaterial2d(materials.add(Color::srgb(0.8, 0.2, 0.2))),
            // above the map layer at z = 0
            Transform::from_xyz(50., -50., 10.),
        ));
    }
}

fn move_towards_player(
    player: Single<&Transform, With<player::Player>>,
    flow_field: Res<FlowField>,
    mut enemies: Query<(Entity, &Transform, &mut LinearVelocity), With<Enemy>>,
) {
    const MOVE_SPEED: f32 = 100.;
    /// Enemies closer than this push each other apart
    const SEPARATION_DISTANCE: f32 = SHAPE.radius * 2. + 4.;
    const SEPARATION_WEIGHT: f32 = 1.5;

    let positions: Vec<_> = enemies
        .iter()
        .map(|(entity, transform, _)| (entity, transform.translation.xy()))
        .collect();

    for (entity, transform, mut velocity) in &mut enemies {
        let position = transform.translation.xy();

        // follow the flow field around obstacles, walk straight once in the player's cell or
        // if the player can't be reached
        let direction = flow_field
            .direction(position)
            .unwrap_or_else(|| (player.translation.xy() - position).normalize_or_zero());

        let mut separation = Vec2::ZERO;
        for &(other, other_position) in &positions {
            let offset = position - other_position;
            let distance = offset.length();
            if other == entity || distance >= SEPARATION_DISTANCE {
                continue;
            }
            // enemies spawn on the same spot, so pick a direction per entity when they overlap
            let away = offset.try_normalize().unwrap_or_else(|| {
                Vec2::from_angle(entity.index_u32() as f32 * std::f32::consts::GOLDEN_RATIO)
            });
            separation += away * (1. - distance / SEPARATION_DISTANCE);
        }

        let new_vel =
            (direction + separation * SEPARATION_WEIGHT).clamp_length_max(1.) * MOVE_SPEED;
        if velocity.0 != new_vel {
            velocity.0 = new_vel;
        }
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
