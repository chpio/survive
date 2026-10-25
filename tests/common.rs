use avian2d::prelude::*;
use bevy::{app::Plugins, prelude::*, time::TimeUpdateStrategy};
use std::time::Duration;
use survival::{combat::Health, movement, objects::enemy::Enemy};

pub const SMALL: Enemy = Enemy { radius: 5. };
pub const LARGE: Enemy = Enemy { radius: 12. };

/// Headless app with top-down physics and force-based movement, advancing one fixed step of
/// 1/64 s per update.
pub fn app() -> App {
    app_with(())
}

/// [`app`] with additional `plugins`, which can't be added after it is finished.
pub fn app_with<M>(plugins: impl Plugins<M>) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        PhysicsPlugins::default().with_length_unit(movement::LENGTH_UNIT),
        movement::Plugin,
        plugins,
    ))
    .insert_resource(Gravity::ZERO)
    .insert_resource(NarrowPhaseConfig {
        contact_tolerance: movement::CONTACT_TOLERANCE,
        ..default()
    })
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

pub fn health(app: &App, entity: Entity) -> Health {
    *app.world().get::<Health>(entity).unwrap()
}
