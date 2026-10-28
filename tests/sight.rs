use avian2d::prelude::*;
use bevy::prelude::*;
use survival::sight::{CELL_SIZE, OpaqueCells, SIGHT_FADE, SIGHT_RADIUS, Sight};

/// Map of 600×600 px around the origin. A player at the origin is in the cell centered at (2, 2).
fn map() -> OpaqueCells {
    OpaqueCells::new(Rect::new(-300., -300., 300., 300.))
}

/// Makes the rectangle `rect` block the sight.
fn block(map: &mut OpaqueCells, rect: Rect) {
    let size = rect.size();
    let collider = Collider::rectangle(size.x, size.y);
    map.block_collider(&collider, rect.center(), Rotation::IDENTITY);
}

/// Sight from `center` on the loaded `maps`.
fn sight_from(center: Vec2, maps: &[&OpaqueCells]) -> Sight {
    let mut sight = Sight::default();
    sight.set_center(center, maps);
    sight
}

#[test]
fn open_field_is_visible_within_the_radius() {
    let sight = sight_from(Vec2::ZERO, &[&map()]);

    assert_eq!(sight.visibility(Vec2::ZERO), 1.);
    assert_eq!(sight.visibility(Vec2::new(-100., 80.)), 1.);
    assert_eq!(
        sight.visibility(Vec2::new(SIGHT_RADIUS + CELL_SIZE * 2., 0.)),
        0.
    );
    // fades out before the radius
    let fading = sight.visibility(Vec2::new(SIGHT_RADIUS - SIGHT_FADE / 2., 0.));
    assert!((0.3..0.7).contains(&fading), "{fading}");
}

#[test]
fn obstacle_casts_a_shadow() {
    let mut map = map();
    block(&mut map, Rect::new(40., -20., 60., 20.));
    let sight = sight_from(Vec2::ZERO, &[&map]);

    // the side facing the player is lit, everything behind it is in its shadow
    assert!(sight.visibility(Vec2::new(41., 0.)) > 0.);
    assert_eq!(sight.visibility(Vec2::new(47., 0.)), 0.);
    assert_eq!(sight.visibility(Vec2::new(100., 0.)), 0.);
    assert_eq!(sight.visibility(Vec2::new(100., 10.)), 0.);
    // past its edge
    assert_eq!(sight.visibility(Vec2::new(100., 80.)), 1.);
    assert_eq!(sight.visibility(Vec2::new(-100., 0.)), 1.);
}

#[test]
fn shadow_moves_with_the_player() {
    let mut map = map();
    block(&mut map, Rect::new(40., -20., 60., 20.));
    let mut sight = sight_from(Vec2::ZERO, &[&map]);
    assert_eq!(sight.visibility(Vec2::new(100., 0.)), 0.);

    // above the obstacle, the line of sight passes it
    sight.set_center(Vec2::new(0., 60.), &[&map]);
    assert_eq!(sight.visibility(Vec2::new(100., 0.)), 1.);
}

#[test]
fn mask_is_only_computed_when_the_player_changes_cells() {
    let map = map();
    let mut sight = Sight::default();
    assert_eq!(sight.visibility(Vec2::ZERO), 0.);

    assert!(sight.set_center(Vec2::ZERO, &[&map]));
    assert!(!sight.set_center(Vec2::new(1., 1.), &[&map]));
    assert!(sight.set_center(Vec2::new(CELL_SIZE * 2., 0.), &[&map]));
}

#[test]
fn obstacles_outside_the_map_are_ignored_and_beyond_it_is_open() {
    let mut map = OpaqueCells::new(Rect::new(0., 0., 100., 100.));
    block(&mut map, Rect::new(-50., -50., -10., 50.));
    let sight = sight_from(Vec2::new(50., 50.), &[&map]);

    assert_eq!(sight.visibility(Vec2::new(-20., 50.)), 1.);
    assert_eq!(sight.visibility(Vec2::new(150., 50.)), 1.);
}

#[test]
fn window_is_centered_on_the_player() {
    let sight = sight_from(Vec2::new(100., -60.), &[&map()]);

    let window = sight.window_rect();
    assert!(window.center().distance(Vec2::new(100., -60.)) < CELL_SIZE);
    assert!(window.width() > 2. * SIGHT_RADIUS);
}

#[test]
fn shadows_keep_their_angle() {
    let mut map = map();
    // seen from the player's cell at about 63°
    block(&mut map, Rect::new(24., 54., 36., 66.));
    let sight = sight_from(Vec2::ZERO, &[&map]);

    // behind the obstacle on the same line
    assert_eq!(sight.visibility(Vec2::new(75., 150.)), 0.);
    // at about 79°, passing left of the obstacle, so not snapped to the vertical behind it
    assert!(sight.visibility(Vec2::new(30., 150.)) > 0.9);
    // at about 45°, passing right of it
    assert!(sight.visibility(Vec2::new(100., 100.)) > 0.9);
}

#[test]
fn obstacle_on_a_neighbor_map_casts_a_shadow() {
    let west = OpaqueCells::new(Rect::new(-300., -300., 0., 300.));
    let mut east = OpaqueCells::new(Rect::new(0., -300., 300., 300.));
    block(&mut east, Rect::new(20., -20., 40., 20.));
    let mut sight = sight_from(Vec2::new(-60., 0.), &[&west, &east]);
    assert_eq!(sight.visibility(Vec2::new(60., 0.)), 0.);

    // outside the loaded maps, nothing blocks the sight
    sight.forget_center();
    sight.set_center(Vec2::new(-60., 0.), &[&west]);
    assert_eq!(sight.visibility(Vec2::new(60., 0.)), 1.);
}
