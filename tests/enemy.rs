mod common;

use avian2d::prelude::*;
use bevy::prelude::*;
use survival::objects::enemy;

#[test]
fn enemy_does_not_walk_through_wall() {
    let mut app = common::app();
    // wall from x = 25 to 35
    common::spawn(
        &mut app,
        (RigidBody::Static, Collider::rectangle(10., 100.)),
        Vec2::new(30., 0.),
    );
    let enemy = common::spawn(&mut app, enemy::physics(), Vec2::ZERO);
    common::set_desired_velocity(&mut app, enemy, Vec2::new(100., 0.));

    // enough to cross the wall several times over
    common::run(&mut app, 2.);

    let x = common::position(&app, enemy).x;
    assert!(x <= 25. - enemy::SHAPE.radius + 0.5, "enemy at x = {x}");
}

#[test]
fn overlapping_enemies_are_pushed_apart() {
    let mut app = common::app();
    let a = common::spawn(&mut app, enemy::physics(), Vec2::ZERO);
    let b = common::spawn(&mut app, enemy::physics(), Vec2::new(1., 0.));

    common::run(&mut app, 1.);

    let distance = common::position(&app, a).distance(common::position(&app, b));
    assert!(
        distance >= enemy::SHAPE.radius * 2. - 0.5,
        "enemies {distance} apart"
    );
}
