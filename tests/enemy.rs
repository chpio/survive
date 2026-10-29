use crate::common::{self, SMALL};
use avian2d::prelude::*;
use bevy::prelude::*;
use survival::{
    flowfield::{self, FlowField},
    objects::{
        enemy::{self, Enemy},
        player,
    },
    sight::{self, BlocksSight},
    world::MapArea,
};

#[test]
fn enemy_does_not_walk_through_wall() {
    let mut app = common::app();
    // wall from x = 25 to 35
    common::spawn(
        &mut app,
        (RigidBody::Static, Collider::rectangle(10., 100.)),
        Vec2::new(30., 0.),
    );
    let enemy = common::spawn(&mut app, SMALL.physics(), Vec2::ZERO);
    common::set_desired_velocity(&mut app, enemy, Vec2::new(100., 0.));

    // enough to cross the wall several times over
    common::run(&mut app, 2.);

    let x = common::position(&app, enemy).x;
    assert!(x <= 25. - SMALL.radius + 0.5, "enemy at x = {x}");
}

#[test]
fn overlapping_enemies_are_pushed_apart() {
    let mut app = common::app();
    let a = common::spawn(&mut app, SMALL.physics(), Vec2::ZERO);
    let b = common::spawn(&mut app, SMALL.physics(), Vec2::new(1., 0.));

    common::run(&mut app, 1.);

    let distance = common::position(&app, a).distance(common::position(&app, b));
    assert!(
        distance >= SMALL.radius * 2. - 0.5,
        "enemies {distance} apart"
    );
}

#[test]
fn enemies_keep_distance_from_each_other() {
    let mut app = common::app();
    app.init_resource::<FlowField>()
        .add_systems(Update, enemy::move_towards_player);
    // far enough away that the enemies don't reach the player
    common::spawn(&mut app, player::Player, Vec2::new(500., 0.));
    let enemies: Vec<_> = (0..6)
        .map(|i| common::spawn(&mut app, (SMALL, SMALL.physics()), Vec2::new(0., i as f32)))
        .collect();

    common::run(&mut app, 1.);

    let positions: Vec<_> = enemies.iter().map(|&e| common::position(&app, e)).collect();
    let min_distance = positions
        .iter()
        .enumerate()
        .flat_map(|(i, a)| positions[i + 1..].iter().map(move |b| a.distance(*b)))
        .fold(f32::INFINITY, f32::min);
    // colliders alone only keep them `2 * radius` apart
    assert!(
        min_distance > SMALL.radius * 2. + 2.,
        "closest enemies {min_distance} apart"
    );
}

#[test]
fn larger_separation_keeps_enemies_further_away() {
    let mut app = common::app();
    app.init_resource::<FlowField>()
        .add_systems(Update, enemy::move_towards_player);
    common::spawn(&mut app, player::Player, Vec2::new(500., 0.));
    let small = common::spawn(&mut app, (SMALL, SMALL.physics()), Vec2::ZERO);
    let large = enemy::Separation {
        radius: 20.,
        ..default()
    };
    let large = common::spawn(&mut app, (SMALL, SMALL.physics(), large), Vec2::new(0., 1.));

    common::run(&mut app, 1.);

    // personal spaces of 7 and 20 px, normal enemies keep 14 px apart
    let distance = common::position(&app, small).distance(common::position(&app, large));
    assert!(distance > 22., "enemies {distance} apart");
}

/// Enemy at the origin facing +x, heading towards `heading`.
fn facing_app(heading: Vec2) -> (App, Entity) {
    let mut app = common::app();
    app.add_systems(Update, enemy::face_heading);
    let enemy = common::spawn(
        &mut app,
        (
            SMALL,
            SMALL.physics(),
            enemy::Heading(heading),
            children![(enemy::Body, Transform::default())],
        ),
        Vec2::ZERO,
    );
    (app, enemy)
}

/// Direction the enemy's body faces.
fn facing(app: &App, enemy: Entity) -> Vec2 {
    let body = app.world().get::<Children>(enemy).unwrap()[0];
    let transform = app.world().get::<Transform>(body).unwrap();
    (transform.rotation * Vec3::X).xy()
}

#[test]
fn enemy_turns_towards_heading() {
    let (mut app, enemy) = facing_app(Vec2::Y);

    common::run(&mut app, 1.);

    let facing = facing(&app, enemy);
    assert!(facing.dot(Vec2::Y) > 0.999, "facing {facing}");
}

#[test]
fn enemy_turns_at_limited_speed() {
    // half a turn away
    let (mut app, enemy) = facing_app(Vec2::NEG_X);

    common::run(&mut app, 0.25);

    // the first update has no time delta, so it turns one step less
    let turned = facing(&app, enemy).to_angle().abs();
    let max = enemy::TURN_SPEED * 0.25;
    assert!(
        turned <= max + 0.01 && turned > max * 0.9,
        "turned {turned} rad"
    );
}

#[test]
fn knockback_does_not_turn_enemy() {
    let (mut app, enemy) = facing_app(Vec2::X);
    common::run(&mut app, 0.25);

    // knocked back against its heading
    app.world_mut().get_mut::<LinearVelocity>(enemy).unwrap().0 = Vec2::new(-300., 50.);
    common::run(&mut app, 0.5);

    assert!(common::position(&app, enemy).x < -10.);
    let facing = facing(&app, enemy);
    assert!(facing.dot(Vec2::X) > 0.999, "facing {facing}");
}

fn spawn_map(app: &mut App, area: Rect) -> Entity {
    app.world_mut()
        .spawn((MapArea(area), Transform::default()))
        .id()
}

/// Distance from the enemy's collider to the player's after it walked around a pond-like
/// obstacle for 6 s, starting at `start` on the other side.
fn gap_after_walking_around_pond(enemy: Enemy, start: Vec2, player_at: Vec2) -> f32 {
    let mut app = common::app_with(flowfield::Plugin);
    app.add_systems(Update, enemy::move_towards_player);
    let map = spawn_map(&mut app, Rect::new(-200., -200., 200., 200.));
    common::spawn(
        &mut app,
        (
            RigidBody::Static,
            Collider::rectangle(96., 48.),
            ChildOf(map),
        ),
        Vec2::ZERO,
    );
    let player = common::spawn(&mut app, (player::Player, player::physics()), player_at);
    let entity = common::spawn(&mut app, (enemy, enemy.physics()), start);

    common::run(&mut app, 6.);

    common::position(&app, entity).distance(common::position(&app, player))
        - enemy.radius
        - player::SHAPE.radius
}

#[test]
fn enemies_walk_around_pond_corners() {
    let mut stuck = vec![];
    for radius in [5., 8., 12.] {
        let enemy = Enemy { radius };
        // right below the pond, inside the clearance of the larger enemies, and further away
        for player_at in [Vec2::new(0., -32.), Vec2::new(30., -70.)] {
            for start in [
                Vec2::new(-90., 60.),
                Vec2::new(-30., 60.),
                Vec2::new(0., 60.),
                Vec2::new(40., 60.),
                Vec2::new(100., 30.),
                Vec2::new(-100., 0.),
            ] {
                let gap = gap_after_walking_around_pond(enemy, start, player_at);
                if gap > 3. {
                    stuck.push(format!(
                        "radius {radius}, player {player_at}, start {start}: {gap:.1} px away"
                    ));
                }
            }
        }
    }
    assert!(stuck.is_empty(), "{stuck:#?}");
}

#[test]
fn enemy_slides_along_obstacle_instead_of_sticking() {
    let mut app = common::app();
    // bottom-right corner at (48, -24)
    common::spawn(
        &mut app,
        (RigidBody::Static, Collider::rectangle(96., 48.)),
        Vec2::ZERO,
    );
    // touching the bottom edge left of the corner
    let enemy = common::spawn(&mut app, SMALL.physics(), Vec2::new(46., -29.5));
    // mostly into the edge, a little along it, like an enemy walking straight at a player above
    // to the right
    common::set_desired_velocity(&mut app, enemy, Vec2::new(0.34, 0.94).normalize() * 100.);

    common::run(&mut app, 1.);

    let position = common::position(&app, enemy);
    assert!(position.x > 48. && position.y > -24., "enemy at {position}");
}

#[test]
fn enemy_outside_the_loaded_maps_is_despawned() {
    let mut app = common::app();
    app.add_systems(Update, enemy::despawn_outside_maps);
    spawn_map(&mut app, Rect::new(0., 0., 160., 160.));
    let inside = common::spawn(&mut app, (SMALL, SMALL.physics()), Vec2::new(80., 80.));
    let near_edge = common::spawn(
        &mut app,
        (SMALL, SMALL.physics()),
        Vec2::new(160. + enemy::UNLOAD_MARGIN / 2., 80.),
    );
    let outside = common::spawn(&mut app, (SMALL, SMALL.physics()), Vec2::new(200., 80.));

    app.update();

    assert!(app.world().get_entity(inside).is_ok());
    assert!(app.world().get_entity(near_edge).is_ok());
    assert!(app.world().get_entity(outside).is_err());
}

#[test]
fn enemy_walking_onto_a_neighbor_map_stays() {
    let mut app = common::app();
    app.add_systems(Update, enemy::despawn_outside_maps);
    spawn_map(&mut app, Rect::new(0., 0., 160., 160.));
    spawn_map(&mut app, Rect::new(160., 0., 320., 160.));
    let enemy = common::spawn(&mut app, (SMALL, SMALL.physics()), Vec2::new(100., 80.));
    common::set_desired_velocity(&mut app, enemy, Vec2::new(100., 0.));

    common::run(&mut app, 1.);

    assert!(app.world().get_entity(enemy).is_ok());
    let x = common::position(&app, enemy).x;
    assert!(x > 180., "enemy at x = {x}");
}

/// App in which enemies wake up, fall asleep and walk towards the player, with a map of 600×600
/// px around the origin.
fn sleep_app() -> (App, Entity) {
    let mut app = common::app_with(sight::Plugin);
    app.init_resource::<FlowField>().add_systems(
        Update,
        (enemy::wake_and_sleep, enemy::move_towards_player).chain(),
    );
    let map = spawn_map(&mut app, Rect::new(-300., -300., 300., 300.));
    (app, map)
}

#[test]
fn sleeping_enemy_stands_still_until_the_player_sees_it() {
    let (mut app, map) = sleep_app();
    // between the player and the enemy
    common::spawn(
        &mut app,
        (
            RigidBody::Static,
            Collider::rectangle(20., 80.),
            BlocksSight,
            ChildOf(map),
        ),
        Vec2::new(50., 0.),
    );
    let player = common::spawn(&mut app, player::Player, Vec2::ZERO);
    let enemy = common::spawn(
        &mut app,
        (SMALL, SMALL.physics(), enemy::Asleep),
        Vec2::new(100., 0.),
    );

    common::run(&mut app, 1.);
    assert!(app.world().get::<enemy::Asleep>(enemy).is_some());
    let position = common::position(&app, enemy);
    assert!(
        position.distance(Vec2::new(100., 0.)) < 0.5,
        "enemy at {position}"
    );

    // above the enemy, with nothing in between
    app.world_mut()
        .get_mut::<Transform>(player)
        .unwrap()
        .translation = Vec3::new(100., 100., 0.);
    common::run(&mut app, 0.5);

    assert!(app.world().get::<enemy::Asleep>(enemy).is_none());
    let y = common::position(&app, enemy).y;
    assert!(y > 20., "enemy at y = {y}");
}

#[test]
fn enemy_far_from_the_player_falls_asleep() {
    let (mut app, _) = sleep_app();
    common::spawn(&mut app, player::Player, Vec2::ZERO);
    let near = common::spawn(&mut app, (SMALL, SMALL.physics()), Vec2::new(-150., 0.));
    let far = common::spawn(
        &mut app,
        (SMALL, SMALL.physics()),
        Vec2::new(enemy::DEACTIVATION_RADIUS + 20., 0.),
    );

    common::run(&mut app, 0.5);

    assert!(app.world().get::<enemy::Asleep>(near).is_none());
    assert!(app.world().get::<enemy::Asleep>(far).is_some());
    // stopped where it fell asleep instead of walking on
    let x = common::position(&app, far).x;
    assert!(x > enemy::DEACTIVATION_RADIUS + 15., "enemy at x = {x}");
}
