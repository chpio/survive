mod common;

use avian2d::prelude::*;
use bevy::prelude::*;
use survival::{
    flowfield::FlowField,
    objects::{
        enemy::{self, Enemy},
        player,
    },
};

const SMALL: Enemy = Enemy { radius: 5. };

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
        (SMALL, SMALL.physics(), enemy::Heading(heading)),
        Vec2::ZERO,
    );
    (app, enemy)
}

fn facing(app: &App, enemy: Entity) -> Vec2 {
    let transform = app.world().get::<Transform>(enemy).unwrap();
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

    // the transform shows the rotation a frame or two later, so allow for a few steps less
    let turned = facing(&app, enemy).to_angle().abs();
    let max = enemy::TURN_SPEED * 0.25;
    assert!(
        turned <= max + 0.01 && turned > max * 0.75,
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
