use crate::{
    flowfield::FlowField,
    movement::{DesiredVelocity, Strength},
    objects::player,
};
use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Default, Clone, Component)]
#[require(Separation)]
pub struct Enemy;

/// Personal space of an enemy. Enemies steer away from each other while their personal spaces
/// overlap, so a larger radius keeps all other enemies further away.
#[derive(Component, Clone, Copy)]
pub struct Separation {
    /// Radius of the personal space around the enemy's center, in px
    pub radius: f32,
    /// How strongly the enemy steers away, relative to walking towards the player
    pub strength: f32,
}

impl Default for Separation {
    fn default() -> Self {
        Self {
            radius: SHAPE.radius + 2.,
            strength: 2.5,
        }
    }
}

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
    others: Query<(&Transform, &Separation), With<Enemy>>,
    mut enemies: Query<(Entity, &Transform, &Separation, &mut DesiredVelocity), With<Enemy>>,
) {
    const MOVE_SPEED: f32 = 100.;

    let max_radius = others
        .iter()
        .map(|(_, separation)| separation.radius)
        .fold(0., f32::max);

    for (entity, transform, separation, mut desired) in &mut enemies {
        let position = transform.translation.xy();

        // follow the flow field around obstacles, walk straight if the player can't be reached
        let direction = flow_field
            .direction(position)
            .unwrap_or_else(|| (player.translation.xy() - position).normalize_or_zero());

        // finds every enemy whose personal space could overlap with this one's
        let neighborhood = Collider::circle(separation.radius + max_radius);
        let mut push = Vec2::ZERO;
        spatial_query.shape_intersections_callback(
            &neighborhood,
            position,
            0.,
            &SpatialQueryFilter::default().with_excluded_entities([entity]),
            |other| {
                // the query also finds the player and obstacles
                let Ok((other_transform, other_separation)) = others.get(other) else {
                    return true;
                };
                let offset = position - other_transform.translation.xy();
                let distance = offset.length();
                let range = separation.radius + other_separation.radius;
                if distance < range {
                    // enemies spawn on the same spot, so pick a direction per entity when they
                    // overlap
                    let away = offset.try_normalize().unwrap_or_else(|| {
                        Vec2::from_angle(entity.index_u32() as f32 * std::f32::consts::GOLDEN_RATIO)
                    });
                    push += away * (1. - distance / range);
                }
                true
            },
        );

        let new_velocity =
            (direction + push * separation.strength).clamp_length_max(1.) * MOVE_SPEED;
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
