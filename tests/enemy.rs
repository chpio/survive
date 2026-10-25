mod common;

use avian2d::prelude::*;
use bevy::prelude::*;
use survival::{
    flowfield::FlowField,
    objects::{
        enemy::{self, Size},
        player,
    },
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
    let enemy = common::spawn(&mut app, enemy::physics(Size::Small), Vec2::ZERO);
    common::set_desired_velocity(&mut app, enemy, Vec2::new(100., 0.));

    // enough to cross the wall several times over
    common::run(&mut app, 2.);

    let x = common::position(&app, enemy).x;
    assert!(x <= 25. - Size::Small.radius() + 0.5, "enemy at x = {x}");
}

#[test]
fn overlapping_enemies_are_pushed_apart() {
    let mut app = common::app();
    let a = common::spawn(&mut app, enemy::physics(Size::Small), Vec2::ZERO);
    let b = common::spawn(&mut app, enemy::physics(Size::Small), Vec2::new(1., 0.));

    common::run(&mut app, 1.);

    let distance = common::position(&app, a).distance(common::position(&app, b));
    assert!(
        distance >= Size::Small.radius() * 2. - 0.5,
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
        .map(|i| {
            common::spawn(
                &mut app,
                (enemy::Enemy, enemy::physics(Size::Small)),
                Vec2::new(0., i as f32),
            )
        })
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
        min_distance > Size::Small.radius() * 2. + 2.,
        "closest enemies {min_distance} apart"
    );
}

#[test]
fn larger_separation_keeps_enemies_further_away() {
    let mut app = common::app();
    app.init_resource::<FlowField>()
        .add_systems(Update, enemy::move_towards_player);
    common::spawn(&mut app, player::Player, Vec2::new(500., 0.));
    let small = common::spawn(
        &mut app,
        (enemy::Enemy, enemy::physics(Size::Small)),
        Vec2::ZERO,
    );
    let large = enemy::Separation {
        radius: 20.,
        ..default()
    };
    let large = common::spawn(
        &mut app,
        (enemy::Enemy, enemy::physics(Size::Small), large),
        Vec2::new(0., 1.),
    );

    common::run(&mut app, 1.);

    // personal spaces of 7 and 20 px, normal enemies keep 14 px apart
    let distance = common::position(&app, small).distance(common::position(&app, large));
    assert!(distance > 22., "enemies {distance} apart");
}
