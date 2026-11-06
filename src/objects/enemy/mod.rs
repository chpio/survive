use crate::{
    combat::{Attack, Died, Health, health_bar::UnitHeight},
    figure::{Figure, FigureControl, FigureRoot},
    flowfield::FlowField,
    movement::{self, DesiredVelocity},
    objects::player,
    sight::Sight,
    world::MapArea,
};
use avian2d::prelude::*;
use bevy::{
    ecs::system::SystemParam, prelude::*, time::common_conditions::on_timer, window::PrimaryWindow,
};
use std::{
    f32::consts::{GOLDEN_RATIO, TAU},
    time::Duration,
};

pub mod swarm;

/// How fast enemies walk, in px/s. Swarm enemies walk faster or slower depending on their state.
pub const MOVE_SPEED: f32 = 100.;

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
        movement::body(self.shape().collider(), self.mass(), self.strength())
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

/// Direction an enemy wants to walk in, which its [`Body`] turns to face. Set by its navigation,
/// so it doesn't change when the enemy is pushed or knocked back.
#[derive(Component, Default, PartialEq)]
pub struct Heading(pub Vec2);

/// Visible body of an enemy, a child showing its figure, which turns towards the [`Heading`]
/// (through [`FigureControl::facing`]).
#[derive(Component)]
pub struct Body;

/// Spawns an enemy. The sources decide what spawns where, `spawn_enemy` only builds the entity.
#[derive(Message, Clone, Copy, PartialEq, Debug)]
pub struct SpawnEnemy {
    pub enemy: Enemy,
    pub position: Vec2,
    /// Whether it starts [`Asleep`], so it waits until the player sees it
    pub asleep: bool,
}

/// E / R / T spawn a sleeping small / medium / large enemy at the mouse cursor (next to the player
/// while the cursor is outside the window). It wakes up once the player sees it.
fn spawn_enemy_on_key(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform)>,
    player: Single<&Transform, With<player::Player>>,
    mut spawn_enemy: MessageWriter<SpawnEnemy>,
) {
    let (camera, camera_transform) = *camera;
    for (key, radius) in [
        (KeyCode::KeyE, 5.),
        (KeyCode::KeyR, 8.),
        (KeyCode::KeyT, 12.),
    ] {
        if keyboard_input.just_pressed(key) {
            let position = window
                .cursor_position()
                .and_then(|cursor| camera.viewport_to_world_2d(camera_transform, cursor).ok())
                .unwrap_or(player.translation.xy() + Vec2::X * 30.);
            spawn_enemy.write(SpawnEnemy {
                enemy: Enemy { radius },
                position,
                asleep: true,
            });
        }
    }
}

/// The figure all enemies are drawn as, scaled to their radius.
#[derive(Resource)]
struct EnemyAssets {
    beetle: Handle<Figure>,
}

/// Radius in px the beetle figure is made for; enemies of other sizes scale it.
const BEETLE_RADIUS: f32 = 5.;

/// Speed in px/s at which the walk animation of an enemy with [`BEETLE_RADIUS`] plays at its own
/// pace; faster enemies play it faster, larger ones slower, since their steps are longer.
const WALK_ANIMATION_SPEED: f32 = 40.;

fn create_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(EnemyAssets {
        beetle: asset_server.load("characters/beetle.glb"),
    });
}

fn spawn_enemy(
    mut spawn_enemy: MessageReader<SpawnEnemy>,
    mut cmds: Commands,
    assets: Res<EnemyAssets>,
) {
    for &SpawnEnemy {
        enemy,
        position,
        asleep,
    } in spawn_enemy.read()
    {
        let mut entity = cmds.spawn((
            enemy,
            Name::new("Enemy"),
            enemy.separation(),
            enemy.physics(),
            enemy.combat(),
            // the beetle's shell
            UnitHeight(5. * enemy.radius / BEETLE_RADIUS),
            // above the map layers
            Transform::from_translation(position.extend(10.)),
            // shown through its children
            Visibility::default(),
        ));
        let scale = enemy.radius / BEETLE_RADIUS;
        entity.with_child((
            Body,
            FigureRoot(assets.beetle.clone()),
            FigureControl {
                animation: "walk",
                ..default()
            },
            Transform::from_scale(Vec3::new(scale, scale, 1.)),
            Visibility::default(),
        ));
        if enemy.radius <= swarm::MAX_RADIUS {
            entity.insert(swarm::Swarm);
        }
        if asleep {
            entity.insert(Asleep);
        }
    }
}

/// Distance in px from the player beyond which awake enemies fall asleep. Larger than the sight
/// radius, so enemies at its edge don't keep waking up and falling asleep, and than the flow
/// field's radius, so enemies following the player out of it keep walking straight at them.
pub const DEACTIVATION_RADIUS: f32 = 400.;

/// A sleeping enemy stands still and doesn't attack until the player sees it. It doesn't count for
/// the flow field, so its grid isn't computed for it.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Asleep;

/// Wakes up the enemies the player sees any part of, and lets those further away than
/// [`DEACTIVATION_RADIUS`] fall asleep where they stand.
pub fn wake_and_sleep(
    sight: Res<Sight>,
    player: Single<&Transform, With<player::Player>>,
    mut enemies: Query<(
        Entity,
        &Enemy,
        &Transform,
        Has<Asleep>,
        &mut DesiredVelocity,
    )>,
    mut commands: Commands,
) {
    let player = player.translation.xy();
    for (entity, enemy, transform, asleep, mut desired) in &mut enemies {
        let position = transform.translation.xy();
        if asleep {
            if sight.sees(position, enemy.radius) {
                commands.entity(entity).remove::<Asleep>();
            }
        } else if position.distance_squared(player) > DEACTIVATION_RADIUS.powi(2) {
            commands.entity(entity).insert(Asleep);
            desired.set_if_neq(DesiredVelocity(Vec2::ZERO));
        }
    }
}

/// Debug switch: frozen enemies neither walk, turn nor attack. They are still bodies, so they can
/// be hit and pushed, and hold their ground with their strength.
#[derive(Resource, Default, Clone, Copy, PartialEq, Debug)]
pub struct Frozen(pub bool);

/// Run condition for what enemies do on their own: false while they are [`Frozen`].
pub fn active(frozen: Option<Res<Frozen>>) -> bool {
    !frozen.is_some_and(|frozen| frozen.0)
}

/// G freezes all enemies or lets them go again.
fn toggle_frozen_on_key(keyboard_input: Res<ButtonInput<KeyCode>>, mut frozen: ResMut<Frozen>) {
    if keyboard_input.just_pressed(KeyCode::KeyG) {
        frozen.0 = !frozen.0;
    }
}

/// Makes frozen enemies stand still instead of keeping the velocity they had.
fn stop(mut enemies: Query<&mut DesiredVelocity, With<Enemy>>) {
    for mut desired in &mut enemies {
        desired.set_if_neq(DesiredVelocity(Vec2::ZERO));
    }
}

/// How enemies steer away from each other while their personal spaces overlap.
#[derive(SystemParam)]
pub struct Crowd<'w, 's> {
    spatial_query: SpatialQuery<'w, 's>,
    others: Query<'w, 's, (&'static Transform, &'static Separation), With<Enemy>>,
}

impl Crowd<'_, '_> {
    /// Radius of the largest personal space.
    fn max_radius(&self) -> f32 {
        self.others
            .iter()
            .map(|(_, separation)| separation.radius)
            .fold(0., f32::max)
    }

    /// Direction away from the enemies whose personal space overlaps with the one of `entity`,
    /// longer the more they overlap. `max_radius` is [`Self::max_radius`].
    fn push(
        &self,
        entity: Entity,
        position: Vec2,
        separation: &Separation,
        max_radius: f32,
    ) -> Vec2 {
        // finds every enemy whose personal space could overlap with this one's
        let neighborhood = Collider::circle(separation.radius + max_radius);
        let mut push = Vec2::ZERO;
        self.spatial_query.shape_intersections_callback(
            &neighborhood,
            position,
            0.,
            &SpatialQueryFilter::default().with_excluded_entities([entity]),
            |other| {
                // the query also finds the player and obstacles
                let Ok((other_transform, other_separation)) = self.others.get(other) else {
                    return true;
                };
                let offset = position - other_transform.translation.xy();
                let distance = offset.length();
                let range = separation.radius + other_separation.radius;
                if distance < range {
                    // enemies spawn on the same spot, so pick a direction per entity when they
                    // overlap
                    let away = offset.try_normalize().unwrap_or_else(|| {
                        Vec2::from_angle(entity.index_u32() as f32 * GOLDEN_RATIO)
                    });
                    push += away * (1. - distance / range);
                }
                true
            },
        );
        push
    }
}

/// Direction for `enemy` at `position` to walk towards the player: along the flow field around
/// obstacles, straight if the player can't be reached.
fn towards_player(flow_field: &FlowField, enemy: &Enemy, position: Vec2, player: Vec2) -> Vec2 {
    flow_field
        .direction(position, enemy.radius)
        .unwrap_or_else(|| (player - position).normalize_or_zero())
}

/// Lets the awake enemies walk towards the player, around obstacles and away from each other.
/// Swarm enemies walk by their own state ([`swarm`]).
pub fn move_towards_player(
    player: Single<&Transform, With<player::Player>>,
    flow_field: Res<FlowField>,
    crowd: Crowd,
    mut enemies: Query<
        (
            Entity,
            &Enemy,
            &Transform,
            &Separation,
            &mut Heading,
            &mut DesiredVelocity,
        ),
        (Without<Asleep>, Without<swarm::Swarm>),
    >,
) {
    let max_radius = crowd.max_radius();
    for (entity, enemy, transform, separation, mut heading, mut desired) in &mut enemies {
        let position = transform.translation.xy();
        let direction = towards_player(&flow_field, enemy, position, player.translation.xy());
        // keep the last heading when standing on the player
        if direction != Vec2::ZERO {
            heading.set_if_neq(Heading(direction));
        }

        let push = crowd.push(entity, position, separation, max_radius);
        desired.set_if_neq(DesiredVelocity(
            (direction + push * separation.strength).clamp_length_max(1.) * MOVE_SPEED,
        ));
    }
}

/// Turns the [`Body`] of enemies towards their [`Heading`], at most at [`TURN_SPEED`].
pub fn face_heading(
    time: Res<Time>,
    headings: Query<&Heading, With<Enemy>>,
    mut bodies: Query<(&ChildOf, &mut FigureControl), With<Body>>,
) {
    let max_step = TURN_SPEED * time.delta_secs();
    for (child_of, mut control) in &mut bodies {
        let Ok(heading) = headings.get(child_of.parent()) else {
            continue;
        };
        if heading.0 == Vec2::ZERO {
            continue;
        }
        let angle = control.facing.angle_to(heading.0);
        let step = angle.clamp(-max_step, max_step);
        if step != 0. {
            control.facing = Vec2::from_angle(step).rotate(control.facing);
        }
    }
}

/// How far in px an enemy may stand outside the loaded maps before it is despawned, so enemies
/// walking along the edge of a map don't vanish.
pub const UNLOAD_MARGIN: f32 = 16.;

/// Despawns enemies that stand on no loaded map, since the obstacles around them are gone with the
/// maps. Enemies aren't saved, so they are gone for good.
pub fn despawn_outside_maps(
    enemies: Query<(Entity, &Transform), With<Enemy>>,
    maps: Query<&MapArea>,
    mut commands: Commands,
) {
    for (entity, transform) in &enemies {
        let position = transform.translation.xy();
        if !maps
            .iter()
            .any(|map| map.0.inflate(UNLOAD_MARGIN).contains(position))
        {
            commands.entity(entity).despawn();
        }
    }
}

/// Plays the walk animation of the enemies' figures as fast as they move, so it stops with them,
/// and slower for larger enemies.
fn animate_walk(
    enemies: Query<(&Enemy, &LinearVelocity)>,
    mut bodies: Query<(&ChildOf, &mut FigureControl), With<Body>>,
) {
    for (child_of, mut control) in &mut bodies {
        let Ok((enemy, velocity)) = enemies.get(child_of.parent()) else {
            continue;
        };
        let scale = enemy.radius / BEETLE_RADIUS;
        let speed = (velocity.length() / (WALK_ANIMATION_SPEED * scale)).min(4.);
        // compared first, so unchanged speeds don't count as changes
        if (control.speed - speed).abs() > 0.01 {
            control.speed = speed;
        }
    }
}

/// Dead enemies are despawned.
pub fn despawn_on_death(died: On<Died>, enemies: Query<(), With<Enemy>>, mut commands: Commands) {
    if enemies.contains(died.entity) {
        // the player's death may despawn all enemies in the same frame
        commands.entity(died.entity).try_despawn();
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnEnemy>()
            .add_observer(despawn_on_death)
            .init_resource::<Frozen>()
            .add_systems(Startup, create_assets)
            .add_systems(
                Update,
                (
                    spawn_enemy_on_key,
                    toggle_frozen_on_key,
                    spawn_enemy,
                    (
                        wake_and_sleep,
                        (
                            move_towards_player,
                            // after the heading is updated
                            face_heading
                                .after(move_towards_player)
                                .after(swarm::move_swarm),
                        )
                            .run_if(active),
                        stop.run_if(not(active)),
                    )
                        .chain(),
                    despawn_outside_maps.run_if(on_timer(Duration::from_secs(1))),
                    animate_walk,
                ),
            );
    }
}
