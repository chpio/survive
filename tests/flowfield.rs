use bevy::prelude::*;
use survival::flowfield::Grid;

/// Empty 160×160 grid with its bottom-left corner at the origin.
fn grid() -> Grid {
    Grid::new(Vec2::ZERO, Vec2::splat(160.))
}

/// Wall at x = 76..84 over the whole height, except for a gap at `gap_min..gap_max`.
fn wall_with_gap(grid: &mut Grid, gap_min: f32, gap_max: f32) {
    grid.block(Rect::new(76., 0., 84., gap_min));
    grid.block(Rect::new(76., gap_max, 84., 160.));
}

#[test]
fn open_field_points_straight_at_target() {
    let mut grid = grid();
    grid.set_target(Vec2::new(150., 80.));

    let direction = grid.direction(Vec2::new(10., 80.)).unwrap();
    assert!(direction.dot(Vec2::X) > 0.99, "{direction}");
}

#[test]
fn walks_around_wall_through_gap() {
    let mut grid = grid();
    wall_with_gap(&mut grid, 120., 160.);
    grid.set_target(Vec2::new(150., 20.));

    // the target is straight to the right, but the only way leads up through the gap
    let direction = grid.direction(Vec2::new(10., 20.)).unwrap();
    assert!(direction.y > 0.5, "{direction}");
}

#[test]
fn gap_narrower_than_enemy_is_closed() {
    let mut grid = grid();
    wall_with_gap(&mut grid, 76., 84.);
    grid.set_target(Vec2::new(150., 80.));

    assert_eq!(grid.direction(Vec2::new(10., 80.)), None);
}

#[test]
fn corridor_one_tile_wide_is_open() {
    let mut grid = grid();
    wall_with_gap(&mut grid, 72., 88.);
    grid.set_target(Vec2::new(150., 80.));

    let direction = grid.direction(Vec2::new(10., 80.)).unwrap();
    assert!(direction.x > 0.9, "{direction}");
}

#[test]
fn enclosed_target_is_unreachable() {
    let mut grid = grid();
    grid.block(Rect::new(100., 60., 140., 64.));
    grid.block(Rect::new(100., 96., 140., 100.));
    grid.block(Rect::new(100., 60., 104., 100.));
    grid.block(Rect::new(136., 60., 140., 100.));
    grid.set_target(Vec2::new(120., 80.));

    assert_eq!(grid.direction(Vec2::new(10., 80.)), None);
}

#[test]
fn leads_out_of_clearance_around_obstacle() {
    let mut grid = grid();
    grid.block(Rect::new(60., 60., 100., 100.));
    grid.set_target(Vec2::new(150., 80.));

    // touching the obstacle, closer than the clearance
    let direction = grid.direction(Vec2::new(57., 80.)).unwrap();
    assert!(direction.x < 0., "{direction}");
}

#[test]
fn diagonal_line_of_sight_is_straight() {
    let mut grid = grid();
    grid.set_target(Vec2::new(150., 110.));

    let direction = grid.direction(Vec2::new(10., 20.)).unwrap();
    let straight = Vec2::new(140., 90.).normalize();
    assert!(direction.dot(straight) > 0.999, "{direction}");
}

#[test]
fn no_direction_at_target_or_outside_grid() {
    let mut grid = grid();
    grid.set_target(Vec2::new(150., 80.));

    assert_eq!(grid.direction(Vec2::new(150., 80.)), None);
    assert_eq!(grid.direction(Vec2::new(-10., 80.)), None);
    assert_eq!(grid.direction(Vec2::new(170., 80.)), None);
}

#[test]
fn follows_target_when_it_moves() {
    let mut grid = grid();
    grid.set_target(Vec2::new(150., 80.));
    grid.set_target(Vec2::new(10., 80.));

    let direction = grid.direction(Vec2::new(80., 80.)).unwrap();
    assert!(direction.dot(Vec2::NEG_X) > 0.99, "{direction}");
}
