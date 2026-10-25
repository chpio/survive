use crate::{
    combat::{Attack, Health},
    flowfield::FlowField,
    movement::{DesiredVelocity, Strength},
    objects::player,
};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::{
    f32::consts::{PI, TAU},
    time::Duration,
};

/// How fast enemies turn towards their [`Heading`], in rad/s.
pub const TURN_SPEED: f32 = TAU;

#[derive(Clone, Copy, Component, PartialEq, Debug)]
#[require(Separation, Heading)]
pub struct Enemy {
    /// Radius in px, which sets the enemy's shape, mass, strength and personal space
    pub radius: f32,
}

impl Default for Enemy {
    fn default() -> Self {
        Self { radius: 5. }
    }
}

impl Enemy {
    /// Shape of the collider and the placeholder mesh.
    pub fn shape(self) -> Circle {
        Circle::new(self.radius)
    }

    /// Grows with the area, an enemy with a radius of 5 px weighs 1.
    pub fn mass(self) -> f32 {
        (self.radius / 5.).powi(2)
    }

    /// Grows with the mass, so all sizes reach full speed in 0.1 s.
    pub fn strength(self) -> f32 {
        self.mass() * 1000.
    }

    pub fn separation(self) -> Separation {
        Separation {
            radius: self.radius + 2.,
            strength: 2.5,
        }
    }

    /// Physics components of the enemy. Enemies are dynamic bodies, so they collide with
    /// obstacles, the player and each other.
    pub fn physics(self) -> impl Bundle {
        (
            RigidBody::Dynamic,
            self.shape().collider(),
            LockedAxes::ROTATION_LOCKED,
            Mass(self.mass()),
            NoAutoMass,
            Strength(self.strength()),
        )
    }

    /// Health and attack. Larger enemies last longer and push the player further (both grow with
    /// the mass) and hit harder (with the radius, so a few large ones don't kill the player in
    /// one hit). An enemy hits the player once a second while touching it.
    pub fn combat(self) -> impl Bundle {
        let mass = self.mass();
        (
            Health::new(20. * mass),
            Attack::new(
                self.radius,
                self.radius + 2.,
                100. * mass,
                Duration::from_secs(1),
            ),
        )
    }
}

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
        Enemy::default().separation()
    }
}

/// Direction an enemy wants to walk in, which it turns to face. Set by its navigation, so it
/// doesn't change when the enemy is pushed or knocked back.
#[derive(Component, Default)]
pub struct Heading(pub Vec2);

#[derive(Message)]
pub struct SpawnEnemy(pub Enemy);

fn spawn_enemy_on_key(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut spawn_enemy: MessageWriter<SpawnEnemy>,
) {
    for (key, radius) in [
        (KeyCode::KeyE, 5.),
        (KeyCode::KeyR, 8.),
        (KeyCode::KeyT, 12.),
    ] {
        if keyboard_input.just_pressed(key) {
            spawn_enemy.write(SpawnEnemy(Enemy { radius }));
        }
    }
}

fn spawn_enemy(
    mut spawn_enemy: MessageReader<SpawnEnemy>,
    mut cmds: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for &SpawnEnemy(enemy) in spawn_enemy.read() {
        cmds.spawn((
            enemy,
            Name::new("Enemy"),
            enemy.separation(),
            enemy.physics(),
            enemy.combat(),
            Mesh2d(meshes.add(enemy.shape())),
            MeshMaterial2d(materials.add(Color::srgb(0.8, 0.2, 0.2))),
            // above the map layer at z = 0
            Transform::from_xyz(50., -50., 10.),
            // dot at the front, showing where the enemy faces (+x when not rotated)
            children![(
                Mesh2d(meshes.add(Circle::new(enemy.radius * 0.25))),
                MeshMaterial2d(materials.add(Color::srgb(1., 0.9, 0.9))),
                Transform::from_xyz(enemy.radius * 0.55, 0., 0.1),
            )],
        ));
    }
}

pub fn move_towards_player(
    player: Single<&Transform, With<player::Player>>,
    flow_field: Res<FlowField>,
    spatial_query: SpatialQuery,
    others: Query<(&Transform, &Separation), With<Enemy>>,
    mut enemies: Query<(
        Entity,
        &Enemy,
        &Transform,
        &Separation,
        &mut Heading,
        &mut DesiredVelocity,
    )>,
) {
    const MOVE_SPEED: f32 = 100.;

    let max_radius = others
        .iter()
        .map(|(_, separation)| separation.radius)
        .fold(0., f32::max);

    for (entity, enemy, transform, separation, mut heading, mut desired) in &mut enemies {
        let position = transform.translation.xy();

        // follow the flow field around obstacles, walk straight if the player can't be reached
        let direction = flow_field
            .direction(position, enemy.radius)
            .unwrap_or_else(|| (player.translation.xy() - position).normalize_or_zero());
        // keep the last heading when standing on the player
        if direction != Vec2::ZERO && heading.0 != direction {
            heading.0 = direction;
        }

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

/// Turns enemies towards their [`Heading`], at most at [`TURN_SPEED`]. Rotation is locked for the
/// physics, so contacts don't spin enemies, and set here directly instead.
pub fn face_heading(time: Res<Time>, mut enemies: Query<(&Heading, &mut Rotation), With<Enemy>>) {
    let max_step = TURN_SPEED * time.delta_secs();
    for (heading, mut rotation) in &mut enemies {
        if heading.0 == Vec2::ZERO {
            continue;
        }
        let current = rotation.as_radians();
        // shortest way round, in -π..π
        let difference = (heading.0.to_angle() - current + PI).rem_euclid(TAU) - PI;
        if difference != 0. {
            *rotation = Rotation::radians(current + difference.clamp(-max_step, max_step));
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnEnemy>().add_systems(
            Update,
            (
                spawn_enemy_on_key,
                spawn_enemy,
                move_towards_player,
                // after the heading is updated
                face_heading.after(move_towards_player),
            ),
        );
    }
}
