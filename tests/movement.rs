use crate::common::{self, LARGE, SMALL};
use avian2d::prelude::*;
use bevy::prelude::*;
use survival::objects::{enemy::Enemy, player};

#[test]
fn reaches_desired_velocity() {
    let mut app = common::app();
    let player = common::spawn(&mut app, player::physics(), Vec2::ZERO);
    common::set_desired_velocity(&mut app, player, Vec2::new(200., 0.));

    common::run(&mut app, 0.25);

    // within 1 %, after a slight overshoot
    let velocity = common::velocity(&app, player);
    assert!(velocity.distance(Vec2::new(200., 0.)) < 2., "{velocity}");
}

#[test]
fn stops_when_desired_velocity_is_zero() {
    let mut app = common::app();
    let player = common::spawn(&mut app, player::physics(), Vec2::ZERO);
    common::set_desired_velocity(&mut app, player, Vec2::new(200., 0.));
    common::run(&mut app, 0.25);

    common::set_desired_velocity(&mut app, player, Vec2::ZERO);
    common::run(&mut app, 0.25);

    let velocity = common::velocity(&app, player);
    assert!(velocity.length() < 2., "{velocity}");
}

/// Spawns `count` copies of `enemy` in a row left of the player, all walking to the right.
/// Returns the player's x after 2 s and after 4 s.
fn push_player(count: usize, enemy: Enemy) -> (f32, f32) {
    let mut app = common::app();
    let player = common::spawn(&mut app, player::physics(), Vec2::ZERO);
    let gap = (player::SHAPE.radius + enemy.radius) + 0.5;
    for i in 0..count {
        let enemy = common::spawn(
            &mut app,
            enemy.physics(),
            Vec2::new(-gap - i as f32 * (enemy.radius * 2. + 0.5), 0.),
        );
        common::set_desired_velocity(&mut app, enemy, Vec2::new(100., 0.));
    }

    common::run(&mut app, 2.);
    let early = common::position(&app, player).x;
    common::run(&mut app, 2.);
    let late = common::position(&app, player).x;
    (early, late)
}

/// The player gives way up to `max_give` px, like a spring or knocked back on impact, but isn't
/// pushed further over time.
fn assert_holds_ground((early, late): (f32, f32), max_give: f32) {
    assert!(early.abs() < max_give, "player pushed to x = {early}");
    assert!(
        (late - early).abs() < 0.1,
        "player drifts from {early} to {late}"
    );
}

#[test]
fn single_enemy_cannot_push_player() {
    assert_holds_ground(push_player(1, SMALL), 5.);
}

#[test]
fn five_enemies_cannot_push_player() {
    assert_holds_ground(push_player(5, SMALL), 5.);
}

#[test]
fn horde_pushes_player() {
    let (early, late) = push_player(10, SMALL);
    assert!(
        late - early > 20.,
        "player only pushed from {early} to {late}"
    );
}

#[test]
fn single_large_enemy_cannot_push_player() {
    // nearly as strong and twice as heavy as the player, so the impact at full speed knocks it
    // back a bit
    assert_holds_ground(push_player(1, LARGE), 20.);
}

#[test]
fn two_large_enemies_push_player() {
    let (early, late) = push_player(2, LARGE);
    assert!(
        late - early > 20.,
        "player only pushed from {early} to {late}"
    );
}

#[test]
fn body_slides_past_corner_of_triangulated_collider() {
    // pond from (-48, -24) to (48, 24), split along the diagonal from top-left to bottom-right like
    // bevy_ecs_tiled triangulates the map's colliders, each triangle relative to its centroid
    let (top_left, top_right) = (Vec2::new(-48., 24.), Vec2::new(48., 24.));
    let (bottom_left, bottom_right) = (Vec2::new(-48., -24.), Vec2::new(48., -24.));
    let triangle = |a: Vec2, b: Vec2, c: Vec2| {
        let centroid = (a + b + c) / 3.;
        (
            centroid,
            Rotation::IDENTITY,
            Collider::triangle(a - centroid, b - centroid, c - centroid),
        )
    };
    let pond = Collider::compound(vec![
        triangle(top_left, top_right, bottom_right),
        triangle(top_left, bottom_left, bottom_right),
    ]);
    let mut app = common::app();
    common::spawn(&mut app, (RigidBody::Static, pond), Vec2::ZERO);
    // touching the bottom edge, walking right past the corner
    let start_y = -24. - player::SHAPE.radius;
    let body = common::spawn(&mut app, player::physics(), Vec2::new(20., start_y));
    common::set_desired_velocity(&mut app, body, Vec2::new(200., 0.));

    let mut lowest = f32::MAX;
    for _ in 0..30 {
        app.update();
        lowest = lowest.min(common::position(&app, body).y);
    }
    assert!(lowest > start_y - 0.1, "pushed down to {lowest}");
}
