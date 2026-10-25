use avian2d::prelude::*;
use bevy::prelude::*;
use survival::sight::{CELL_SIZE, SIGHT_FADE, SIGHT_RADIUS, Sight};

/// 600×600 px around the origin, with the player's cell centered at (2, 2).
fn sight() -> Sight {
    Sight::new(Vec2::splat(-300.), Vec2::splat(600.))
}

/// Makes the rectangle `rect` block the sight.
fn block(sight: &mut Sight, rect: Rect) {
    let size = rect.size();
    let collider = Collider::rectangle(size.x, size.y);
    sight.block_collider(&collider, rect.center(), Rotation::IDENTITY);
}

#[test]
fn open_field_is_visible_within_the_radius() {
    let mut sight = sight();
    sight.set_center(Vec2::ZERO);

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
    let mut sight = sight();
    block(&mut sight, Rect::new(40., -20., 60., 20.));
    sight.set_center(Vec2::ZERO);

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
    let mut sight = sight();
    block(&mut sight, Rect::new(40., -20., 60., 20.));
    sight.set_center(Vec2::ZERO);
    assert_eq!(sight.visibility(Vec2::new(100., 0.)), 0.);

    // above the obstacle, the line of sight passes it
    sight.set_center(Vec2::new(0., 60.));
    assert_eq!(sight.visibility(Vec2::new(100., 0.)), 1.);
}

#[test]
fn mask_is_only_computed_when_the_player_changes_cells() {
    let mut sight = sight();
    assert_eq!(sight.visibility(Vec2::ZERO), 0.);

    assert!(sight.set_center(Vec2::ZERO));
    assert!(!sight.set_center(Vec2::new(1., 1.)));
    assert!(sight.set_center(Vec2::new(CELL_SIZE * 2., 0.)));
}

#[test]
fn obstacles_outside_the_map_are_ignored_and_beyond_it_is_open() {
    let mut sight = Sight::new(Vec2::ZERO, Vec2::splat(100.));
    block(&mut sight, Rect::new(-50., -50., -10., 50.));
    sight.set_center(Vec2::new(50., 50.));

    assert_eq!(sight.visibility(Vec2::new(-20., 50.)), 1.);
    assert_eq!(sight.visibility(Vec2::new(150., 50.)), 1.);
}

#[test]
fn window_is_centered_on_the_player() {
    let mut sight = sight();
    sight.set_center(Vec2::new(100., -60.));

    let window = sight.window_rect();
    assert!(window.center().distance(Vec2::new(100., -60.)) < CELL_SIZE);
    assert!(window.width() > 2. * SIGHT_RADIUS);
}

#[test]
fn shadows_keep_their_angle() {
    let mut sight = sight();
    // seen from the player's cell at (2, 2) at about 63°
    block(&mut sight, Rect::new(24., 54., 36., 66.));
    sight.set_center(Vec2::ZERO);

    // behind the obstacle on the same line
    assert_eq!(sight.visibility(Vec2::new(75., 150.)), 0.);
    // at about 79°, passing left of the obstacle, so not snapped to the vertical behind it
    assert!(sight.visibility(Vec2::new(30., 150.)) > 0.9);
    // at about 45°, passing right of it
    assert!(sight.visibility(Vec2::new(100., 100.)) > 0.9);
}
