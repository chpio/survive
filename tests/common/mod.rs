// each test binary uses only some of these helpers
#![allow(dead_code)]

use avian2d::prelude::*;
use bevy::{prelude::*, time::TimeUpdateStrategy};
use std::time::Duration;
use survival::movement;

/// Headless app with top-down physics and force-based movement, advancing one fixed step of
/// 1/64 s per update.
pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        PhysicsPlugins::default().with_length_unit(100.),
        movement::Plugin,
    ))
    .insert_resource(Gravity::ZERO)
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(
        1. / 64.,
    )));
    // `App::run` does this, but tests call `App::update` directly
    app.finish();
    app.cleanup();
    app
}

/// Runs the app for `seconds` of game time.
pub fn run(app: &mut App, seconds: f32) {
    for _ in 0..(seconds * 64.) as u32 {
        app.update();
    }
}

pub fn spawn(app: &mut App, bundle: impl Bundle, position: Vec2) -> Entity {
    app.world_mut()
        .spawn((bundle, Transform::from_translation(position.extend(0.))))
        .id()
}

pub fn set_desired_velocity(app: &mut App, entity: Entity, velocity: Vec2) {
    app.world_mut()
        .get_mut::<movement::DesiredVelocity>(entity)
        .unwrap()
        .0 = velocity;
}

pub fn position(app: &App, entity: Entity) -> Vec2 {
    app.world()
        .get::<Transform>(entity)
        .unwrap()
        .translation
        .xy()
}

pub fn velocity(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<LinearVelocity>(entity).unwrap().0
}
