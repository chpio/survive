use crate::common::{self, SMALL};
use avian2d::prelude::*;
use bevy::prelude::*;
use survival::{
    flowfield::{CELL_SIZE, FlowField, Obstacles},
    world::{self, MapArea},
};

fn app() -> App {
    let mut app = common::app();
    app.add_systems(PreUpdate, world::rasterize::<Obstacles, Without<Sensor>>);
    app
}

fn spawn_map(app: &mut App, area: Rect) -> Entity {
    app.world_mut()
        .spawn((MapArea(area), Transform::default()))
        .id()
}

/// Static collider covering `rect`, on `map` if there is one.
fn spawn_obstacle(app: &mut App, map: Option<Entity>, rect: Rect) -> Entity {
    let size = rect.size();
    let mut obstacle = app.world_mut().spawn((
        RigidBody::Static,
        Collider::rectangle(size.x, size.y),
        Transform::from_translation(rect.center().extend(0.)),
    ));
    if let Some(map) = map {
        obstacle.insert(ChildOf(map));
    }
    obstacle.id()
}

/// Whether the obstacles of `map` block the cell at `position` for small enemies.
fn blocked(app: &App, map: Entity, position: Vec2) -> bool {
    let obstacles = app.world().get::<Obstacles>(map).unwrap();
    let mut flow_field = FlowField::new([SMALL.radius]);
    flow_field.set_target(position, [SMALL.radius], &[obstacles]);
    let cell = (position / CELL_SIZE).floor().as_ivec2();
    flow_field.grids()[0]
        .cells()
        .find(|c| c.position == cell)
        .unwrap()
        .blocked
}

#[test]
fn maps_get_the_obstacles_of_their_colliders() {
    let mut app = app();
    let map = spawn_map(&mut app, Rect::new(0., 0., 160., 160.));
    spawn_obstacle(&mut app, Some(map), Rect::new(60., 60., 100., 100.));
    // belongs to no map
    spawn_obstacle(&mut app, None, Rect::new(120., 20., 140., 40.));

    common::run(&mut app, 0.25);

    assert!(blocked(&app, map, Vec2::new(80., 80.)));
    assert!(!blocked(&app, map, Vec2::new(130., 80.)));
    assert!(!blocked(&app, map, Vec2::new(130., 30.)));
}

#[test]
fn changed_collider_only_rasterizes_its_map_again() {
    let mut app = app();
    let west = spawn_map(&mut app, Rect::new(0., 0., 160., 160.));
    let east = spawn_map(&mut app, Rect::new(160., 0., 320., 160.));
    let wall = spawn_obstacle(&mut app, Some(west), Rect::new(60., 60., 100., 100.));
    spawn_obstacle(&mut app, Some(east), Rect::new(220., 60., 260., 100.));
    common::run(&mut app, 0.25);
    let last_changed = |app: &App, map: Entity| {
        app.world()
            .entity(map)
            .get_ref::<Obstacles>()
            .unwrap()
            .last_changed()
    };
    let (west_changed, east_changed) = (last_changed(&app, west), last_changed(&app, east));

    app.world_mut()
        .get_mut::<Transform>(wall)
        .unwrap()
        .translation
        .y += 40.;
    common::run(&mut app, 0.25);

    assert_ne!(last_changed(&app, west), west_changed);
    assert_eq!(last_changed(&app, east), east_changed);
    assert!(!blocked(&app, west, Vec2::new(80., 70.)));
    assert!(blocked(&app, west, Vec2::new(80., 130.)));
}
