use crate::common::{self, SMALL};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::time::Duration;
use survival::{
    flowfield,
    objects::{
        enemy::{Enemy, SpawnEnemy},
        player,
    },
    sight::{self, BlocksSight, Sight},
    spawning::{
        self,
        ambush::{AmbushTrigger, SpawnPoint, Triggered},
        out_of_sight::{self, SpawnOutOfSight},
    },
    world::MapArea,
};

/// Enemies the spawners sent, in order.
#[derive(Resource, Default)]
struct Spawned(Vec<SpawnEnemy>);

fn record(mut spawn_enemy: MessageReader<SpawnEnemy>, mut spawned: ResMut<Spawned>) {
    spawned.0.extend(spawn_enemy.read().copied());
}

/// App with the spawners, the flow field and the sight, a map of 600×600 px around the origin and
/// the player at the origin. Returns the map.
fn app() -> (App, Entity) {
    let mut app = common::app_with((flowfield::Plugin, sight::Plugin, spawning::Plugin));
    app.init_resource::<Spawned>().add_systems(Last, record);
    let map = app
        .world_mut()
        .spawn((
            MapArea(Rect::new(-300., -300., 300., 300.)),
            Transform::default(),
        ))
        .id();
    common::spawn(&mut app, (player::Player, player::physics()), Vec2::ZERO);
    (app, map)
}

/// Obstacle covering `rect` on `map` that blocks the sight.
fn spawn_wall(app: &mut App, map: Entity, rect: Rect) {
    let size = rect.size();
    common::spawn(
        app,
        (
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            BlocksSight,
            ChildOf(map),
        ),
        rect.center(),
    );
}

/// Wall right of the player, at x = 60..80 and y = -60..60. Behind it, everything is out of
/// sight, and the way around it is much longer than the direct line.
fn spawn_wall_right(app: &mut App, map: Entity) {
    spawn_wall(app, map, Rect::new(60., -60., 80., 60.));
}

fn spawned(app: &App) -> &[SpawnEnemy] {
    &app.world().resource::<Spawned>().0
}

fn requests(app: &mut App) -> usize {
    app.world_mut()
        .query::<&flowfield::Request>()
        .iter(app.world())
        .count()
}

#[test]
fn place_out_of_sight_is_behind_an_obstacle() {
    let (mut app, map) = app();
    spawn_wall_right(&mut app, map);
    app.world_mut()
        .write_message(SpawnOutOfSight(SMALL))
        .unwrap();

    common::run(&mut app, 0.1);

    let [spawned] = spawned(&app) else {
        panic!("spawned {:?}", spawned(&app));
    };
    assert_eq!(spawned.enemy, SMALL);
    assert!(!spawned.asleep);
    let position = spawned.position;
    // right behind the wall rather than in the open beyond the sight radius
    assert!(
        position.x > 80. && position.length() < 100.,
        "spawned at {position}"
    );
    let sight = app.world().resource::<Sight>();
    assert!(!sight.sees(position, SMALL.radius));
    // the grid isn't requested any longer
    assert_eq!(requests(&mut app), 0);
}

#[test]
fn place_out_of_sight_keeps_the_walking_distance() {
    let (mut app, _) = app();
    // the open map, where only cells beyond the sight radius are out of sight
    app.world_mut()
        .write_message(SpawnOutOfSight(SMALL))
        .unwrap();

    common::run(&mut app, 0.1);

    let [spawned] = spawned(&app) else {
        panic!("spawned {:?}", spawned(&app));
    };
    let distance = spawned.position.length();
    assert!(
        (out_of_sight::MIN_DISTANCE..=out_of_sight::MAX_DISTANCE).contains(&distance),
        "spawned {distance} px away"
    );
    assert!(distance > sight::SIGHT_RADIUS);
}

/// Trigger zone of the ambush `layer` of 40×40 px around `center`, like those from Tiled: a
/// static sensor without collision events.
fn spawn_zone_at(app: &mut App, layer: Entity, spawn_points: Option<u32>, center: Vec2) -> Entity {
    common::spawn(
        app,
        (
            AmbushTrigger {
                ambush: layer,
                spawn_points,
            },
            RigidBody::Static,
            Collider::rectangle(40., 40.),
            Sensor,
        ),
        center,
    )
}

/// Trigger zone of the ambush `layer` around the player.
fn spawn_zone(app: &mut App, layer: Entity, spawn_points: Option<u32>) -> Entity {
    spawn_zone_at(app, layer, spawn_points, Vec2::ZERO)
}

fn spawn_point(app: &mut App, layer: Entity, position: Vec2, enemy: Enemy, count: u32, delay: f32) {
    common::spawn(
        app,
        SpawnPoint {
            ambush: layer,
            enemy,
            count,
            delay: Duration::from_secs_f32(delay),
        },
        position,
    );
}

#[test]
fn ambush_uses_the_closest_spawn_points_out_of_sight() {
    let (mut app, map) = app();
    spawn_wall_right(&mut app, map);
    let layer = app.world_mut().spawn_empty().id();
    spawn_zone(&mut app, layer, Some(1));
    // in sight
    spawn_point(&mut app, layer, Vec2::new(-100., 0.), SMALL, 1, 0.);
    // behind the wall, the closer one first
    spawn_point(&mut app, layer, Vec2::new(150., 0.), SMALL, 1, 0.);
    spawn_point(&mut app, layer, Vec2::new(100., 0.), SMALL, 1, 0.);

    common::run(&mut app, 0.5);

    let positions: Vec<_> = spawned(&app).iter().map(|s| s.position).collect();
    assert_eq!(positions, [Vec2::new(100., 0.)]);
    assert!(spawned(&app).iter().all(|s| !s.asleep));
    assert_eq!(requests(&mut app), 0);
}

#[test]
fn ambush_spawns_the_count_of_each_point_after_its_delay() {
    let (mut app, map) = app();
    spawn_wall_right(&mut app, map);
    let layer = app.world_mut().spawn_empty().id();
    spawn_zone(&mut app, layer, None);
    let medium = Enemy { radius: 8. };
    spawn_point(&mut app, layer, Vec2::new(100., 0.), SMALL, 2, 0.);
    spawn_point(&mut app, layer, Vec2::new(150., 0.), medium, 1, 1.);

    common::run(&mut app, 0.5);
    let enemies: Vec<_> = spawned(&app).iter().map(|s| s.enemy).collect();
    assert_eq!(enemies, [SMALL, SMALL]);

    common::run(&mut app, 1.);
    let enemies: Vec<_> = spawned(&app).iter().map(|s| s.enemy).collect();
    assert_eq!(enemies, [SMALL, SMALL, medium]);
}

#[test]
fn ambush_is_triggered_once_by_all_of_its_zones() {
    let (mut app, map) = app();
    spawn_wall_right(&mut app, map);
    let layer = app.world_mut().spawn_empty().id();
    let zones = [
        spawn_zone(&mut app, layer, None),
        spawn_zone(&mut app, layer, None),
    ];
    let other_layer = app.world_mut().spawn_empty().id();
    let other_zone = spawn_zone_at(&mut app, other_layer, None, Vec2::new(-200., 0.));
    spawn_point(&mut app, layer, Vec2::new(100., 0.), SMALL, 1, 0.);

    common::run(&mut app, 0.5);

    assert_eq!(spawned(&app).len(), 1);
    for zone in zones {
        assert!(app.world().get::<Triggered>(zone).is_some());
    }
    assert!(app.world().get::<Triggered>(other_zone).is_none());
}

#[test]
fn zone_loaded_under_the_standing_player_triggers() {
    let (mut app, map) = app();
    spawn_wall_right(&mut app, map);
    // long enough for the player's body to fall asleep
    common::run(&mut app, 3.);
    let layer = app.world_mut().spawn_empty().id();
    let zone = spawn_zone(&mut app, layer, None);
    spawn_point(&mut app, layer, Vec2::new(100., 0.), SMALL, 1, 0.);

    common::run(&mut app, 0.5);

    assert!(app.world().get::<Triggered>(zone).is_some());
    assert_eq!(spawned(&app).len(), 1);
}

#[test]
fn ambush_skips_spawn_points_too_close_to_walk() {
    let (mut app, map) = app();
    // a pillar right of the player, close enough to walk around quickly
    spawn_wall(&mut app, map, Rect::new(20., -15., 30., 15.));
    let layer = app.world_mut().spawn_empty().id();
    let zone = spawn_zone(&mut app, layer, None);
    spawn_point(&mut app, layer, Vec2::new(40., 0.), SMALL, 1, 0.);

    common::run(&mut app, 0.5);

    assert!(app.world().get::<Triggered>(zone).is_some());
    let sight = app.world().resource::<Sight>();
    assert!(!sight.sees(Vec2::new(40., 0.), SMALL.radius));
    assert!(spawned(&app).is_empty());
    assert_eq!(requests(&mut app), 0);
}
