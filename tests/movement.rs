mod common;

use bevy::prelude::*;
use survival::objects::{enemy, player};

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

/// Spawns `count` enemies in a row left of the player, all walking to the right. Returns the
/// player's x after 2 s and after 4 s.
fn push_player(count: usize) -> (f32, f32) {
    let mut app = common::app();
    let player = common::spawn(&mut app, player::physics(), Vec2::ZERO);
    let gap = (player::SHAPE.radius + enemy::SHAPE.radius) + 0.5;
    for i in 0..count {
        let enemy = common::spawn(
            &mut app,
            enemy::physics(),
            Vec2::new(-gap - i as f32 * (enemy::SHAPE.radius * 2. + 0.5), 0.),
        );
        common::set_desired_velocity(&mut app, enemy, Vec2::new(100., 0.));
    }

    common::run(&mut app, 2.);
    let early = common::position(&app, player).x;
    common::run(&mut app, 2.);
    let late = common::position(&app, player).x;
    (early, late)
}

/// The player gives way a little, like a spring, but isn't pushed further over time.
fn assert_holds_ground((early, late): (f32, f32)) {
    assert!(early.abs() < 5., "player pushed to x = {early}");
    assert!(
        (late - early).abs() < 0.1,
        "player drifts from {early} to {late}"
    );
}

#[test]
fn single_enemy_cannot_push_player() {
    assert_holds_ground(push_player(1));
}

#[test]
fn five_enemies_cannot_push_player() {
    assert_holds_ground(push_player(5));
}

#[test]
fn horde_pushes_player() {
    let (early, late) = push_player(10);
    assert!(
        late - early > 20.,
        "player only pushed from {early} to {late}"
    );
}
