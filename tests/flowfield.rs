use avian2d::prelude::*;
use bevy::prelude::*;
use survival::flowfield::{
    CELL_SIZE, Cell, FLOW_FIELD_RADIUS, FlowField, Grid, Obstacles, debug::FlowFieldDebug,
};

/// Radii of a small and a large enemy
const SMALL: f32 = 5.;
const LARGE: f32 = 12.;

/// Empty 160×160 map with its bottom-left corner at the origin.
fn map() -> Obstacles {
    Obstacles::new(Rect::new(0., 0., 160., 160.))
}

/// Empty 160×160 map right of [`map`].
fn east_map() -> Obstacles {
    Obstacles::new(Rect::new(160., 0., 320., 160.))
}

/// Blocks the cells near the rectangle `obstacle`.
fn block(map: &mut Obstacles, obstacle: Rect) {
    let size = obstacle.size();
    map.block_collider(
        &Collider::rectangle(size.x, size.y),
        obstacle.center(),
        Rotation::IDENTITY,
    );
}

/// Wall at x = 76..84 over the whole height, except for a gap at `gap_min..gap_max`.
fn wall_with_gap(map: &mut Obstacles, gap_min: f32, gap_max: f32) {
    block(map, Rect::new(76., 0., 84., gap_min));
    block(map, Rect::new(76., gap_max, 84., 160.));
}

/// Flow field for units with `radius` on the loaded `maps`, towards `target`.
fn field(maps: &[&Obstacles], radius: f32, target: Vec2) -> FlowField {
    let mut flow_field = FlowField::new([radius]);
    flow_field.set_target(target, [radius], maps);
    flow_field
}

/// Direction for a small enemy at `start` towards `target` on `map`.
fn direction(map: &Obstacles, target: Vec2, start: Vec2) -> Option<Vec2> {
    direction_for(SMALL, map, target, start)
}

fn direction_for(radius: f32, map: &Obstacles, target: Vec2, start: Vec2) -> Option<Vec2> {
    field(&[map], radius, target).direction(start, radius)
}

/// The cell of `grid` at `position`, `None` outside its window.
fn cell_at(grid: &Grid, position: Vec2) -> Option<Cell> {
    let cell = (position / CELL_SIZE).floor().as_ivec2();
    grid.cells().find(|c| c.position == cell)
}

#[test]
fn open_field_points_straight_at_target() {
    let direction = direction(&map(), Vec2::new(150., 80.), Vec2::new(10., 80.)).unwrap();
    assert!(direction.dot(Vec2::X) > 0.99, "{direction}");
}

#[test]
fn walks_around_wall_through_gap() {
    let mut map = map();
    wall_with_gap(&mut map, 120., 160.);

    // the target is straight to the right, but the only way leads up through the gap
    let direction = direction(&map, Vec2::new(150., 20.), Vec2::new(10., 20.)).unwrap();
    assert!(direction.y > 0.5, "{direction}");
}

#[test]
fn gap_narrower_than_enemy_is_closed() {
    let mut map = map();
    wall_with_gap(&mut map, 76., 84.);

    assert_eq!(
        direction(&map, Vec2::new(150., 80.), Vec2::new(10., 80.)),
        None
    );
}

#[test]
fn corridor_one_tile_wide_is_open() {
    let mut map = map();
    wall_with_gap(&mut map, 72., 88.);

    let direction = direction(&map, Vec2::new(150., 80.), Vec2::new(10., 80.)).unwrap();
    assert!(direction.x > 0.9, "{direction}");
}

#[test]
fn only_the_shape_of_an_obstacle_blocks() {
    // one outline for the border and a wall with a gap at the top, like the merged colliders of
    // a map layer, whose bounding box covers the whole map
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
    let mut map = map();
    map.block_collider(&border_and_wall, Vec2::ZERO, Rotation::IDENTITY);

    let direction = direction(&map, Vec2::new(150., 20.), Vec2::new(10., 20.)).unwrap();
    assert!(direction.y > 0.5, "{direction}");
}

#[test]
fn enclosed_target_is_unreachable() {
    let mut map = map();
    block(&mut map, Rect::new(100., 60., 140., 64.));
    block(&mut map, Rect::new(100., 96., 140., 100.));
    block(&mut map, Rect::new(100., 60., 104., 100.));
    block(&mut map, Rect::new(136., 60., 140., 100.));

    assert_eq!(
        direction(&map, Vec2::new(120., 80.), Vec2::new(10., 80.)),
        None
    );
}

#[test]
fn leads_out_of_clearance_around_obstacle() {
    let mut map = map();
    block(&mut map, Rect::new(60., 60., 100., 100.));

    // touching the obstacle, closer than the clearance
    let direction = direction(&map, Vec2::new(150., 80.), Vec2::new(57., 80.)).unwrap();
    assert!(direction.x < 0., "{direction}");
}

#[test]
fn diagonal_line_of_sight_is_straight() {
    let direction = direction(&map(), Vec2::new(150., 110.), Vec2::new(10., 20.)).unwrap();
    let straight = Vec2::new(140., 90.).normalize();
    assert!(direction.dot(straight) > 0.999, "{direction}");
}

#[test]
fn no_direction_at_target_or_outside_the_maps() {
    let flow_field = field(&[&map()], SMALL, Vec2::new(150., 80.));

    assert_eq!(flow_field.direction(Vec2::new(150., 80.), SMALL), None);
    assert_eq!(flow_field.direction(Vec2::new(-10., 80.), SMALL), None);
    assert_eq!(flow_field.direction(Vec2::new(170., 80.), SMALL), None);
}

#[test]
fn follows_target_when_it_moves() {
    let map = map();
    let mut flow_field = field(&[&map], SMALL, Vec2::new(150., 80.));
    flow_field.set_target(Vec2::new(10., 80.), [SMALL], &[&map]);

    let direction = flow_field.direction(Vec2::new(80., 80.), SMALL).unwrap();
    assert!(direction.dot(Vec2::NEG_X) > 0.99, "{direction}");
}

#[test]
fn corridor_one_tile_wide_is_closed_for_large_enemies() {
    let mut map = map();
    wall_with_gap(&mut map, 72., 88.);

    assert_eq!(
        direction_for(LARGE, &map, Vec2::new(150., 80.), Vec2::new(10., 80.)),
        None
    );
}

#[test]
fn corridor_two_tiles_wide_is_open_for_large_enemies() {
    let mut map = map();
    wall_with_gap(&mut map, 64., 96.);

    let direction = direction_for(LARGE, &map, Vec2::new(150., 80.), Vec2::new(10., 80.)).unwrap();
    assert!(direction.x > 0.9, "{direction}");
}

#[test]
fn unlisted_radius_uses_next_larger_grid() {
    let mut flow_field = FlowField::new([12., 5., 8.]);
    // corridor 16 px wide: open for a clearance of 5, closed for 8 and 12
    let mut map = map();
    wall_with_gap(&mut map, 72., 88.);
    flow_field.set_target(Vec2::new(150., 80.), [5., 8., 12.], &[&map]);

    let start = Vec2::new(10., 80.);
    assert!(flow_field.direction(start, 5.).is_some());
    // would fit, but uses the grid with a clearance of 8
    assert_eq!(flow_field.direction(start, 6.5), None);
    // larger than all grids, uses the largest one
    assert_eq!(flow_field.direction(start, 20.), None);
}

#[test]
fn cells_report_obstacles_distances_and_directions() {
    let mut map = map();
    block(&mut map, Rect::new(60., 60., 100., 100.));
    let flow_field = field(&[&map], SMALL, Vec2::new(150., 80.));
    let grid = &flow_field.grids()[0];
    let cell_at = |position: Vec2| cell_at(grid, position).unwrap();

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

    assert_eq!(grid.cells().count(), grid.window().len());
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
    let mut map = map();
    block(&mut map, Rect::new(60., 60., 100., 100.));
    // 4 px right of the obstacle, inside its clearance
    let flow_field = field(&[&map], LARGE, Vec2::new(104., 80.));

    // behind the obstacle, the way leads around it
    let direction = flow_field.direction(Vec2::new(10., 80.), LARGE).unwrap();
    assert!(direction.y.abs() > 0.5, "{direction}");
    // in front of it, straight at the target
    let direction = flow_field.direction(Vec2::new(150., 80.), LARGE).unwrap();
    assert!(direction.dot(Vec2::NEG_X) > 0.99, "{direction}");
}

#[test]
fn target_next_to_obstacle_is_not_reached_through_it() {
    let mut map = map();
    // thin wall over the whole height
    block(&mut map, Rect::new(77., 0., 79., 160.));
    // 3 px right of the wall, so the cells right behind it are within the clearance of the target
    let flow_field = field(&[&map], LARGE, Vec2::new(82., 80.));

    assert_eq!(flow_field.direction(Vec2::new(74., 80.), LARGE), None);
    assert_eq!(flow_field.direction(Vec2::new(10., 80.), LARGE), None);
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
        let mut flow_field = FlowField::new([5., 8., 12., 16.]);
        let mut map = Obstacles::new(Rect::new(0., 0., 200., 200.));
        // pond from 52..148 × 76..124
        block(&mut map, Rect::new(52., 76., 148., 124.));
        flow_field.set_target(target, [5., 8., 12., 16.], &[&map]);
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

/// Walking distance of the cell at `position` in the only grid of `flow_field`.
fn distance_at(flow_field: &FlowField, position: Vec2) -> Option<u32> {
    cell_at(&flow_field.grids()[0], position).and_then(|cell| cell.distance)
}

#[test]
fn distances_are_only_computed_within_the_radius() {
    let map = Obstacles::new(Rect::new(0., 0., FLOW_FIELD_RADIUS + 100., 40.));
    let target = Vec2::new(10., 20.);
    let flow_field = field(&[&map], SMALL, target);

    let inside = target + Vec2::X * (FLOW_FIELD_RADIUS - CELL_SIZE);
    let outside = target + Vec2::X * (FLOW_FIELD_RADIUS + CELL_SIZE);
    assert!(distance_at(&flow_field, inside).is_some());
    assert_eq!(distance_at(&flow_field, outside), None);
}

#[test]
fn distances_towards_old_target_are_reset() {
    let map = Obstacles::new(Rect::new(0., 0., FLOW_FIELD_RADIUS * 2. + 100., 40.));
    let left = Vec2::new(10., 20.);
    let mut flow_field = field(&[&map], SMALL, left);
    flow_field.set_target(left + Vec2::X * (FLOW_FIELD_RADIUS * 2.), [SMALL], &[&map]);

    // only reached from the old target
    assert_eq!(distance_at(&flow_field, left), None);
}

#[test]
fn only_grids_used_by_units_are_computed() {
    let mut flow_field = FlowField::new([5., 12.]);
    let map = map();
    let target = Vec2::new(150., 80.);
    let computed = |flow_field: &FlowField| {
        flow_field
            .grids()
            .iter()
            .map(|grid| grid.max_distance().is_some())
            .collect::<Vec<_>>()
    };

    flow_field.set_target(target, [4.], &[&map]);
    assert_eq!(computed(&flow_field), [true, false]);

    // the target is the same, but the large grid wasn't computed for it yet
    flow_field.set_target(target, [10.], &[&map]);
    assert_eq!(computed(&flow_field), [false, true]);
    assert!(flow_field.direction(Vec2::new(10., 80.), 10.).is_some());

    // units larger than all clearances use the largest grid
    flow_field.set_target(target, [20.], &[&map]);
    assert_eq!(computed(&flow_field), [false, true]);

    flow_field.set_target(target, [], &[&map]);
    assert_eq!(computed(&flow_field), [false, false]);
}

#[test]
fn obstacle_at_the_edge_of_a_map_blocks_the_neighbor_map() {
    let mut west = map();
    let east = east_map();
    block(&mut west, Rect::new(150., 60., 160., 100.));
    let flow_field = field(&[&west, &east], SMALL, Vec2::new(300., 80.));
    let grid = &flow_field.grids()[0];

    // 2 and 10 px right of the obstacle, on the east map
    assert!(cell_at(grid, Vec2::new(162., 80.)).unwrap().blocked);
    assert!(!cell_at(grid, Vec2::new(170., 80.)).unwrap().blocked);
}

#[test]
fn path_across_a_map_border_is_found() {
    let map = map();
    let mut east = east_map();
    // wall on the east map with a gap at the top
    block(&mut east, Rect::new(236., 0., 244., 120.));
    let target = Vec2::new(300., 20.);
    let mut flow_field = FlowField::new([SMALL]);
    flow_field.set_target(target, [SMALL], &[&map, &east]);

    let (closest, end) = follow(&flow_field, SMALL, Vec2::new(10., 20.), target);
    assert!(closest < 3., "stuck at {end}, {closest:.1} px away");
}

#[test]
fn cells_outside_the_loaded_maps_are_blocked() {
    let flow_field = field(&[&map()], SMALL, Vec2::new(150., 80.));
    let grid = &flow_field.grids()[0];

    assert!(!cell_at(grid, Vec2::new(158., 80.)).unwrap().blocked);
    assert!(cell_at(grid, Vec2::new(162., 80.)).unwrap().blocked);
    assert!(cell_at(grid, Vec2::new(150., -2.)).unwrap().blocked);
}

#[test]
fn unloaded_map_leaves_the_window() {
    let west = map();
    let east = east_map();
    let target = Vec2::new(150., 80.);
    let mut flow_field = field(&[&west, &east], SMALL, target);
    assert!(distance_at(&flow_field, Vec2::new(200., 80.)).is_some());

    flow_field.forget_targets();
    flow_field.set_target(target, [SMALL], &[&west]);

    assert!(distance_at(&flow_field, Vec2::new(150., 20.)).is_some());
    assert_eq!(distance_at(&flow_field, Vec2::new(200., 80.)), None);
    assert!(
        cell_at(&flow_field.grids()[0], Vec2::new(200., 80.))
            .unwrap()
            .blocked
    );
}

#[test]
fn walking_distance_follows_the_way_around_obstacles() {
    let mut map = map();
    wall_with_gap(&mut map, 120., 160.);
    let mut flow_field = FlowField::new([SMALL, LARGE]);
    flow_field.set_target(Vec2::new(150., 20.), [SMALL], &[&map]);

    // straight on the target's side of the wall
    let near = flow_field
        .walking_distance(Vec2::new(110., 20.), SMALL)
        .unwrap();
    assert!((near - 40.).abs() < CELL_SIZE, "{near}");
    // through the gap at the top from the other side
    let behind = flow_field
        .walking_distance(Vec2::new(40., 20.), SMALL)
        .unwrap();
    assert!(behind > 200., "{behind}");
    // in the clearance around the wall
    assert_eq!(
        flow_field.walking_distance(Vec2::new(86., 20.), SMALL),
        None
    );
    // the grid for large units isn't computed
    assert!(!flow_field.grid(LARGE).unwrap().is_computed());
    assert_eq!(
        flow_field.walking_distance(Vec2::new(110., 20.), LARGE),
        None
    );
}

#[test]
fn reachable_cells_are_free_and_have_their_walking_distance() {
    let mut map = map();
    wall_with_gap(&mut map, 120., 160.);
    let flow_field = field(&[&map], SMALL, Vec2::new(150., 20.));
    let grid = flow_field.grid(SMALL).unwrap();
    assert!(grid.is_computed());

    let reachable: Vec<_> = grid.reachable().collect();
    for &(position, distance) in &reachable {
        assert!(!cell_at(grid, position).unwrap().blocked, "{position}");
        assert_eq!(flow_field.walking_distance(position, SMALL), Some(distance));
    }
    // on both sides of the wall
    assert!(reachable.iter().any(|(position, _)| position.x < 70.));
    assert!(reachable.iter().any(|(position, _)| position.x > 90.));
}
