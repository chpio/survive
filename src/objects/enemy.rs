use crate::{
    flowfield::FlowField,
    movement::{DesiredVelocity, Strength},
    objects::player,
};
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default, Clone, Component)]
pub struct Enemy;

/// Shape of the collider and the placeholder mesh.
pub const SHAPE: Circle = Circle::new(5.);

pub const MASS: f32 = 1.;
/// Reaches full speed in 0.1 s.
pub const STRENGTH: f32 = 1000.;

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

/// Physics components of an enemy. Enemies are dynamic bodies, so they collide with obstacles,
/// the player and each other.
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
            physics(),
            Mesh2d(meshes.add(SHAPE)),
            MeshMaterial2d(materials.add(Color::srgb(0.8, 0.2, 0.2))),
            // above the map layer at z = 0
            Transform::from_xyz(50., -50., 10.),
        ));
    }
}

pub fn move_towards_player(
    player: Single<&Transform, With<player::Player>>,
    flow_field: Res<FlowField>,
    spatial_query: SpatialQuery,
    positions: Query<&Transform, With<Enemy>>,
    mut enemies: Query<(Entity, &Transform, &mut DesiredVelocity), With<Enemy>>,
) {
    const MOVE_SPEED: f32 = 100.;
    /// Enemies closer than this push each other apart
    const SEPARATION_DISTANCE: f32 = SHAPE.radius * 2. + 4.;
    const SEPARATION_WEIGHT: f32 = 1.5;

    // touches the collider of every enemy whose center is closer than `SEPARATION_DISTANCE`
    let neighborhood = Collider::circle(SEPARATION_DISTANCE - SHAPE.radius);

    for (entity, transform, mut desired) in &mut enemies {
        let position = transform.translation.xy();

        // follow the flow field around obstacles, walk straight if the player can't be reached
        let direction = flow_field
            .direction(position)
            .unwrap_or_else(|| (player.translation.xy() - position).normalize_or_zero());

        let mut separation = Vec2::ZERO;
        spatial_query.shape_intersections_callback(
            &neighborhood,
            position,
            0.,
            &SpatialQueryFilter::default().with_excluded_entities([entity]),
            |other| {
                // the query also finds the player and obstacles
                let Ok(other_transform) = positions.get(other) else {
                    return true;
                };
                let offset = position - other_transform.translation.xy();
                let distance = offset.length();
                if distance < SEPARATION_DISTANCE {
                    // enemies spawn on the same spot, so pick a direction per entity when they
                    // overlap
                    let away = offset.try_normalize().unwrap_or_else(|| {
                        Vec2::from_angle(entity.index_u32() as f32 * std::f32::consts::GOLDEN_RATIO)
                    });
                    separation += away * (1. - distance / SEPARATION_DISTANCE);
                }
                true
            },
        );

        let new_velocity =
            (direction + separation * SEPARATION_WEIGHT).clamp_length_max(1.) * MOVE_SPEED;
        if desired.0 != new_velocity {
            desired.0 = new_velocity;
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
