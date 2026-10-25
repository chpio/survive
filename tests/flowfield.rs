use avian2d::prelude::*;
use bevy::prelude::*;
use survival::flowfield::{CELL_SIZE, FlowField, Grid, debug::FlowFieldDebug};

/// Radii of a small and a large enemy
const SMALL: f32 = 5.;
const LARGE: f32 = 12.;

/// Empty 160×160 grid for small enemies with its bottom-left corner at the origin.
fn grid() -> Grid {
    grid_for(SMALL)
}

fn grid_for(radius: f32) -> Grid {
    Grid::new(Vec2::ZERO, Vec2::splat(160.), radius)
}

/// Blocks the cells near the rectangle `obstacle`.
fn block(grid: &mut Grid, obstacle: Rect) {
    let (collider, position) = rectangle(obstacle);
    grid.block_collider(&collider, position, Rotation::IDENTITY);
}

/// Blocks the cells near the rectangle `obstacle` in all grids.
fn block_all(flow_field: &mut FlowField, obstacle: Rect) {
    let (collider, position) = rectangle(obstacle);
    flow_field.block_collider(&collider, position, Rotation::IDENTITY);
}

fn rectangle(rect: Rect) -> (Collider, Vec2) {
    let size = rect.size();
    (Collider::rectangle(size.x, size.y), rect.center())
}

/// Wall at x = 76..84 over the whole height, except for a gap at `gap_min..gap_max`.
fn wall_with_gap(grid: &mut Grid, gap_min: f32, gap_max: f32) {
    block(grid, Rect::new(76., 0., 84., gap_min));
    block(grid, Rect::new(76., gap_max, 84., 160.));
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
fn only_the_shape_of_an_obstacle_blocks() {
    // one outline for the border and a wall with a gap at the top, like the merged colliders of
    // a map layer, whose bounding box covers the whole grid
    let border_and_wall = Collider::polyline(
        vec![
            Vec2::new(2., 2.),
            Vec2::new(158., 2.),
            Vec2::new(158., 158.),
            Vec2::new(2., 158.),
            Vec2::new(80., 2.),
            Vec2::new(80., 120.),
        ],
        Some(vec![[0, 1], [1, 2], [2, 3], [3, 0], [4, 5]]),
    );
    let mut grid = grid();
    grid.block_collider(&border_and_wall, Vec2::ZERO, Rotation::IDENTITY);
    grid.set_target(Vec2::new(150., 20.));

    let direction = grid.direction(Vec2::new(10., 20.)).unwrap();
    assert!(direction.y > 0.5, "{direction}");
}

#[test]
fn enclosed_target_is_unreachable() {
    let mut grid = grid();
    block(&mut grid, Rect::new(100., 60., 140., 64.));
    block(&mut grid, Rect::new(100., 96., 140., 100.));
    block(&mut grid, Rect::new(100., 60., 104., 100.));
    block(&mut grid, Rect::new(136., 60., 140., 100.));
    grid.set_target(Vec2::new(120., 80.));

    assert_eq!(grid.direction(Vec2::new(10., 80.)), None);
}

#[test]
fn leads_out_of_clearance_around_obstacle() {
    let mut grid = grid();
    block(&mut grid, Rect::new(60., 60., 100., 100.));
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

#[test]
fn corridor_one_tile_wide_is_closed_for_large_enemies() {
    let mut grid = grid_for(LARGE);
    wall_with_gap(&mut grid, 72., 88.);
    grid.set_target(Vec2::new(150., 80.));

    assert_eq!(grid.direction(Vec2::new(10., 80.)), None);
}

#[test]
fn corridor_two_tiles_wide_is_open_for_large_enemies() {
    let mut grid = grid_for(LARGE);
    wall_with_gap(&mut grid, 64., 96.);
    grid.set_target(Vec2::new(150., 80.));

    let direction = grid.direction(Vec2::new(10., 80.)).unwrap();
    assert!(direction.x > 0.9, "{direction}");
}

#[test]
fn unlisted_radius_uses_next_larger_grid() {
    let mut flow_field = FlowField::new(Vec2::ZERO, Vec2::splat(160.), [12., 5., 8.]);
    // corridor 16 px wide: open for a clearance of 5, closed for 8 and 12
    wall_with_gap_in(&mut flow_field, 72., 88.);
    flow_field.set_target(Vec2::new(150., 80.));

    let start = Vec2::new(10., 80.);
    assert!(flow_field.direction(start, 5.).is_some());
    // would fit, but uses the grid with a clearance of 8
    assert_eq!(flow_field.direction(start, 6.5), None);
    // larger than all grids, uses the largest one
    assert_eq!(flow_field.direction(start, 20.), None);
}

fn wall_with_gap_in(flow_field: &mut FlowField, gap_min: f32, gap_max: f32) {
    block_all(flow_field, Rect::new(76., 0., 84., gap_min));
    block_all(flow_field, Rect::new(76., gap_max, 84., 160.));
}

#[test]
fn cells_report_obstacles_distances_and_directions() {
    let mut grid = grid();
    block(&mut grid, Rect::new(60., 60., 100., 100.));
    grid.set_target(Vec2::new(150., 80.));
    let cell_at = |position: Vec2| {
        let cell = (position / CELL_SIZE).floor().as_ivec2();
        grid.cells().find(|c| c.position == cell).unwrap()
    };

    let target = cell_at(Vec2::new(150., 80.));
    assert_eq!(
        (target.blocked, target.distance, target.direction),
        (false, Some(0), None)
    );

    let obstacle = cell_at(Vec2::new(80., 80.));
    assert!(obstacle.blocked);
    assert_eq!(obstacle.distance, None);

    let left = cell_at(Vec2::new(10., 140.));
    assert!(!left.blocked);
    assert!(left.distance.unwrap() > 0);
    assert!(left.direction.unwrap().x > 0.);

    assert_eq!(
        grid.cells().count(),
        (grid.size().x * grid.size().y) as usize
    );
}

#[test]
fn debug_overlay_cycles_through_grids_and_hides() {
    let mut debug = FlowFieldDebug::default();
    let shown: Vec<_> = (0..4)
        .map(|_| {
            debug.cycle(3);
            debug.grid
        })
        .collect();
    assert_eq!(shown, [Some(0), Some(1), Some(2), None]);

    // no grids before a map is loaded
    debug.cycle(0);
    assert_eq!(debug.grid, None);
}

#[test]
fn target_closer_to_obstacle_than_clearance_is_reachable() {
    let mut grid = grid_for(LARGE);
    block(&mut grid, Rect::new(60., 60., 100., 100.));
    // 4 px right of the obstacle, inside its clearance
    grid.set_target(Vec2::new(104., 80.));

    // behind the obstacle, the way leads around it
    let direction = grid.direction(Vec2::new(10., 80.)).unwrap();
    assert!(direction.y.abs() > 0.5, "{direction}");
    // in front of it, straight at the target
    let direction = grid.direction(Vec2::new(150., 80.)).unwrap();
    assert!(direction.dot(Vec2::NEG_X) > 0.99, "{direction}");
}

#[test]
fn target_next_to_obstacle_is_not_reached_through_it() {
    let mut grid = grid_for(LARGE);
    // thin wall over the whole height
    block(&mut grid, Rect::new(77., 0., 79., 160.));
    // 3 px right of the wall, so the cells right behind it are within the clearance of the target
    grid.set_target(Vec2::new(82., 80.));

    assert_eq!(grid.direction(Vec2::new(74., 80.)), None);
    assert_eq!(grid.direction(Vec2::new(10., 80.)), None);
}

/// Follows the field from `start` in 1 px steps. Returns the closest distance to `target` it
/// reached, and where it stopped making progress.
fn follow(flow_field: &FlowField, radius: f32, start: Vec2, target: Vec2) -> (f32, Vec2) {
    let mut position = start;
    let mut closest = position.distance(target);
    for _ in 0..2000 {
        let Some(direction) = flow_field.direction(position, radius) else {
            break;
        };
        position += direction;
        closest = closest.min(position.distance(target));
    }
    (closest, position)
}

#[test]
fn following_the_field_around_a_pond_reaches_the_target() {
    let mut stuck = vec![];
    for target in [
        Vec2::new(100., 30.),
        Vec2::new(100., 54.),
        Vec2::new(130., 80.),
    ] {
        let mut flow_field = FlowField::new(Vec2::ZERO, Vec2::splat(200.), [5., 8., 12., 16.]);
        // pond from 52..148 × 76..124
        block_all(&mut flow_field, Rect::new(52., 76., 148., 124.));
        flow_field.set_target(target);
        for &radius in &[5., 8., 12., 16.] {
            for x in (4..200).step_by(6) {
                for y in (4..200).step_by(6) {
                    let start = Vec2::new(x as f32, y as f32);
                    // skip starts in the clearance or the pond, where the field only leads out
                    if flow_field.direction(start, radius).is_none() {
                        continue;
                    }
                    let (closest, end) = follow(&flow_field, radius, start, target);
                    if closest > 3. {
                        stuck.push(format!(
                            "radius {radius}, target {target}, start {start}: stuck at {end}, \
                             {closest:.1} px away"
                        ));
                    }
                }
            }
        }
    }
    assert!(
        stuck.is_empty(),
        "{} stuck: {:#?}",
        stuck.len(),
        &stuck[..stuck.len().min(25)]
    );
}
