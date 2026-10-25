//! Flow field towards the player, used by enemies to walk around obstacles.
//!
//! The map is divided into a grid of square cells. Cells an enemy can't stand in without
//! overlapping a static, solid collider are blocked. A Dijkstra search from the player's cell
//! stores the walking distance to the player in every free cell, and units follow the decreasing
//! distance. Since larger enemies don't fit through every gap, there is one grid per clearance in
//! [`CLEARANCES`]. Units with a free line of sight to
//! the player walk straight towards them instead, since the eight directions between cells would
//! otherwise bend their path.
//!
//! The obstacles of a grid cover the whole map and are only computed when it changes. The walking
//! distances only cover a square window of a fixed size around the player's cell (see
//! [`FLOW_FIELD_RADIUS`]), which moves along whenever they are computed again. So their memory and
//! cost don't grow with the size of the map. Only the grids used by enemies (or shown by the debug
//! overlay) are computed at all. Enemies outside the window have no field and walk straight at the
//! player.
//!
//! The player can stand closer to an obstacle than a grid's clearance. So that the player stays
//! reachable then, the cells around the player that are only blocked by the clearance can be
//! walked through as well, but never cells inside an obstacle.

use crate::{
    objects::{enemy::Enemy, player},
    perf,
};
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use std::{cmp::Reverse, collections::BinaryHeap};

pub mod debug;

use debug::FlowFieldDebug;

/// Edge length of a cell in pixels (a quarter tile). Small enough that a corridor one tile wide
/// still has free cells for small enemies after the obstacles around it are inflated.
pub const CELL_SIZE: f32 = 4.;

/// Clearances in px with a grid each. An enemy uses the grid with the smallest clearance that is
/// at least its radius, so it avoids gaps it would only just fit through. Enemies larger than all
/// clearances use the largest one and may get stuck in narrow gaps.
pub const CLEARANCES: &[f32] = &[5., 8., 12., 16.];

/// Half the edge length in px of the square window around the player in which the walking
/// distances are computed. A bit more than half the diagonal of the view at the default zoom
/// (500 × 281 px at 16:9), so enemies just outside the view still find their way.
pub const FLOW_FIELD_RADIUS: f32 = 320.;

/// Cells from the player's cell to the edge of the window, and the window's edge length in cells.
const WINDOW_RADIUS: i32 = (FLOW_FIELD_RADIUS / CELL_SIZE) as i32;
const WINDOW_SIZE: i32 = 2 * WINDOW_RADIUS + 1;

/// Cost of a straight and a diagonal step between cells (roughly 1 : √2).
const STRAIGHT_COST: u32 = 10;
const DIAGONAL_COST: u32 = 14;

const NEIGHBORS: [IVec2; 8] = [
    IVec2::new(1, 0),
    IVec2::new(-1, 0),
    IVec2::new(0, 1),
    IVec2::new(0, -1),
    IVec2::new(1, 1),
    IVec2::new(1, -1),
    IVec2::new(-1, 1),
    IVec2::new(-1, -1),
];

#[derive(Resource, Default)]
pub struct FlowField {
    /// Grids for different clearances, sorted by clearance. Empty until a map is loaded.
    grids: Vec<Grid>,
}

pub struct Grid {
    /// Distance a unit's center keeps from obstacles, so paths only lead through gaps it fits in
    clearance: f32,
    /// World position of the bottom-left corner of the grid
    origin: Vec2,
    size: IVec2,
    blocked: Vec<bool>,
    /// Cells whose center lies inside an obstacle, which are blocked as well
    solid: Vec<bool>,
    /// Walking distance to the target per cell of the window, `u32::MAX` if unreachable
    distance: Vec<u32>,
    /// Cell of the grid at the bottom-left corner of the window, centered on the target cell
    window_origin: IVec2,
    /// Target cell the distances were computed for, `None` while they are outdated
    target: Option<IVec2>,
    target_position: Vec2,
}

/// State of one cell of a [`Grid`], see [`Grid::cells`].
pub struct Cell {
    /// Column and row, counted from the bottom-left corner of the grid
    pub position: IVec2,
    /// World position of the center
    pub center: Vec2,
    pub blocked: bool,
    /// Walking distance to the target, `None` if unreachable
    pub distance: Option<u32>,
    /// Direction towards the neighbor closest to the target, `None` at the target or if it can't
    /// be reached
    pub direction: Option<Vec2>,
}

impl Grid {
    /// Grid covering the area from `origin` (bottom-left corner) to `origin + world_size`, with
    /// no obstacles and no target, for units keeping `clearance` from obstacles.
    pub fn new(origin: Vec2, world_size: Vec2, clearance: f32) -> Self {
        let size = (world_size / CELL_SIZE).ceil().as_ivec2();
        let len = (size.x * size.y) as usize;
        Self {
            clearance,
            origin,
            size,
            blocked: vec![false; len],
            solid: vec![false; len],
            distance: vec![u32::MAX; (WINDOW_SIZE * WINDOW_SIZE) as usize],
            window_origin: IVec2::ZERO,
            target: None,
            target_position: Vec2::ZERO,
        }
    }

    pub fn clearance(&self) -> f32 {
        self.clearance
    }

    /// World position of the bottom-left corner.
    pub fn origin(&self) -> Vec2 {
        self.origin
    }

    /// Number of columns and rows.
    pub fn size(&self) -> IVec2 {
        self.size
    }

    /// All cells row by row, starting at the bottom-left corner.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        (0..self.size.y)
            .flat_map(move |y| (0..self.size.x).map(move |x| IVec2::new(x, y)))
            .map(|position| Cell {
                position,
                center: self.cell_center(position),
                blocked: !self.is_free(position),
                distance: Some(self.distance(position)).filter(|&d| d != u32::MAX),
                direction: self.cell_direction(position),
            })
    }

    /// Highest walking distance of a reachable cell, `None` if no cell is reachable.
    pub fn max_distance(&self) -> Option<u32> {
        self.target?;
        self.distance
            .iter()
            .copied()
            .filter(|&d| d != u32::MAX)
            .max()
    }

    fn cell(&self, position: Vec2) -> IVec2 {
        ((position - self.origin) / CELL_SIZE).floor().as_ivec2()
    }

    fn cell_center(&self, cell: IVec2) -> Vec2 {
        self.origin + (cell.as_vec2() + 0.5) * CELL_SIZE
    }

    fn index(&self, cell: IVec2) -> Option<usize> {
        (cell.cmpge(IVec2::ZERO).all() && cell.cmplt(self.size).all())
            .then(|| (cell.y * self.size.x + cell.x) as usize)
    }

    /// Index of `cell` in the window of distances, `None` outside the window.
    fn window_index(&self, cell: IVec2) -> Option<usize> {
        let local = cell - self.window_origin;
        (local.cmpge(IVec2::ZERO).all() && local.cmplt(IVec2::splat(WINDOW_SIZE)).all())
            .then(|| (local.y * WINDOW_SIZE + local.x) as usize)
    }

    /// Inverse of [`Self::window_index`].
    fn window_cell(&self, index: usize) -> IVec2 {
        self.window_origin + IVec2::new(index as i32 % WINDOW_SIZE, index as i32 / WINDOW_SIZE)
    }

    fn is_free(&self, cell: IVec2) -> bool {
        self.index(cell).is_some_and(|i| !self.blocked[i])
    }

    /// Whether paths towards the target may lead through `cell`: free cells, and cells within the
    /// clearance around the target cell that are only blocked by the clearance, so the target
    /// stays reachable when it is closer to an obstacle than the clearance.
    fn is_walkable(&self, cell: IVec2) -> bool {
        let Some(i) = self.index(cell) else {
            return false;
        };
        if !self.blocked[i] {
            return true;
        }
        !self.solid[i]
            && self.target.is_some_and(|target| {
                (cell - target).length_squared() as f32 * CELL_SIZE.powi(2) < self.clearance.powi(2)
            })
    }

    fn distance(&self, cell: IVec2) -> u32 {
        if self.target.is_none() {
            return u32::MAX;
        }
        self.window_index(cell)
            .map_or(u32::MAX, |i| self.distance[i])
    }

    /// Blocks all cells whose center is closer than the clearance to the shape of `collider` at
    /// `position` and `rotation`. Only the shape counts, not its bounding box, since the map's
    /// tiles are merged into one collider per layer that spans the whole map.
    pub fn block_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        block(std::slice::from_mut(self), collider, position, rotation);
    }

    /// Indices and centers of the cells whose center lies in `area`.
    fn cells_in(&self, area: ColliderAabb) -> impl Iterator<Item = (usize, Vec2)> + '_ {
        let min = ((area.min - self.origin) / CELL_SIZE - 0.5)
            .ceil()
            .as_ivec2()
            .max(IVec2::ZERO);
        let max = ((area.max - self.origin) / CELL_SIZE - 0.5)
            .floor()
            .as_ivec2()
            .min(self.size - 1);
        (min.y..=max.y)
            .flat_map(move |y| (min.x..=max.x).map(move |x| IVec2::new(x, y)))
            .map(|cell| (self.index(cell).unwrap(), self.cell_center(cell)))
    }

    /// Removes all obstacles. The distances are computed again on the next [`Self::set_target`].
    pub fn clear(&mut self) {
        self.blocked.fill(false);
        self.solid.fill(false);
        self.target = None;
    }

    /// Neighbors of `cell` that can be walked to, with the cost of the step. Diagonal steps are
    /// only allowed if both adjacent straight neighbors are walkable, so paths don't cut corners.
    fn steps(&self, cell: IVec2) -> impl Iterator<Item = (IVec2, u32)> + '_ {
        NEIGHBORS.into_iter().filter_map(move |offset| {
            let next = cell + offset;
            if !self.is_walkable(next) {
                return None;
            }
            if offset.x != 0 && offset.y != 0 {
                let corners_walkable = self.is_walkable(cell + IVec2::new(offset.x, 0))
                    && self.is_walkable(cell + IVec2::new(0, offset.y));
                corners_walkable.then_some((next, DIAGONAL_COST))
            } else {
                Some((next, STRAIGHT_COST))
            }
        })
    }

    /// Computes the walking distances towards `position`, if it isn't the current target already.
    pub fn set_target(&mut self, position: Vec2) {
        self.target_position = position;
        let target = self.cell(position);
        if self.target != Some(target) {
            self.compute_distances(target);
        }
    }

    /// Forgets the target, so the distances count as outdated until the next
    /// [`Self::set_target`].
    fn forget_target(&mut self) {
        self.target = None;
    }

    /// Dijkstra search from `target` within the window, which is moved to be centered on it.
    fn compute_distances(&mut self, target: IVec2) {
        self.target = Some(target);
        self.window_origin = target - IVec2::splat(WINDOW_RADIUS);
        self.distance.fill(u32::MAX);
        if self.index(target).is_none() {
            return;
        }
        let start = self.window_index(target).unwrap();
        self.distance[start] = 0;

        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        while let Some(Reverse((distance, index))) = queue.pop() {
            if distance > self.distance[index] {
                continue;
            }
            // copied out, since the steps borrow the grid
            let mut steps = [(0, 0); NEIGHBORS.len()];
            let mut count = 0;
            for (next, cost) in self.steps(self.window_cell(index)) {
                if let Some(next_index) = self.window_index(next) {
                    steps[count] = (next_index, cost);
                    count += 1;
                }
            }
            for &(next_index, cost) in &steps[..count] {
                let next_distance = distance + cost;
                if next_distance < self.distance[next_index] {
                    self.distance[next_index] = next_distance;
                    queue.push(Reverse((next_distance, next_index)));
                }
            }
        }
    }

    /// Whether all cells on the line from `from` to `to` are walkable.
    fn line_of_sight(&self, from: Vec2, to: Vec2) -> bool {
        // samples half a cell apart can only skip a cell at its corner, which is still within
        // the clearance around an obstacle
        let steps = (from.distance(to) / (CELL_SIZE / 2.)).ceil().max(1.);
        (0..=steps as u32).all(|i| self.is_walkable(self.cell(from.lerp(to, i as f32 / steps))))
    }

    /// Unit vector from `cell` towards its neighbor closest to the target, if there is one.
    fn cell_direction(&self, cell: IVec2) -> Option<Vec2> {
        let current = self.distance(cell);
        // blocked cells point to any reachable neighbor, so units pushed into the clearance
        // around an obstacle find their way out
        let (next, distance) = NEIGHBORS
            .into_iter()
            .map(|offset| cell + offset)
            .map(|next| (next, self.distance(next)))
            .min_by_key(|(_, distance)| *distance)?;
        (distance < current).then(|| (next - cell).as_vec2().normalize())
    }

    /// Direction to walk at `position` to reach the target: straight if it is in sight, otherwise
    /// interpolated between the four nearest cells. `None` outside the grid, at the target, or if
    /// the target can't be reached.
    pub fn direction(&self, position: Vec2) -> Option<Vec2> {
        let cell = self.cell(position);
        if self.index(cell).is_none() || self.target.is_none() {
            return None;
        }
        if self.line_of_sight(position, self.target_position) {
            return (self.target_position - position).try_normalize();
        }

        let local = (position - self.origin) / CELL_SIZE - 0.5;
        let base = local.floor().as_ivec2();
        let t = local - base.as_vec2();
        let mut sum = Vec2::ZERO;
        for (offset, weight) in [
            (IVec2::new(0, 0), (1. - t.x) * (1. - t.y)),
            (IVec2::new(1, 0), t.x * (1. - t.y)),
            (IVec2::new(0, 1), (1. - t.x) * t.y),
            (IVec2::new(1, 1), t.x * t.y),
        ] {
            if let Some(direction) = self.cell_direction(base + offset) {
                sum += direction * weight;
            }
        }
        // interpolation can cancel out at forks, fall back to the unit's own cell then
        sum.try_normalize().or_else(|| self.cell_direction(cell))
    }
}

impl FlowField {
    /// Grids covering the area from `origin` (bottom-left corner) to `origin + world_size`, one
    /// per clearance.
    pub fn new(origin: Vec2, world_size: Vec2, clearances: impl IntoIterator<Item = f32>) -> Self {
        let mut grids: Vec<_> = clearances
            .into_iter()
            .map(|clearance| Grid::new(origin, world_size, clearance))
            .collect();
        grids.sort_by(|a, b| a.clearance.total_cmp(&b.clearance));
        Self { grids }
    }

    /// Grids sorted by clearance.
    pub fn grids(&self) -> &[Grid] {
        &self.grids
    }

    /// See [`Grid::block_collider`].
    pub fn block_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        block(&mut self.grids, collider, position, rotation);
    }

    /// See [`Grid::clear`].
    pub fn clear(&mut self) {
        for grid in &mut self.grids {
            grid.clear();
        }
    }

    /// Index of the grid for units with `radius`: the one with the smallest clearance that is at
    /// least `radius`, or the largest one for larger units.
    fn grid_index(&self, radius: f32) -> Option<usize> {
        self.grids
            .iter()
            .position(|grid| grid.clearance >= radius)
            .or(self.grids.len().checked_sub(1))
    }

    /// See [`Grid::set_target`], only for the grids used by units with the given `radii`. All
    /// other grids forget their target, so they compute it again once a unit needs them.
    pub fn set_target(&mut self, position: Vec2, radii: impl IntoIterator<Item = f32>) {
        let mut used = vec![false; self.grids.len()];
        for radius in radii {
            if let Some(i) = self.grid_index(radius) {
                used[i] = true;
            }
        }
        for (grid, used) in self.grids.iter_mut().zip(used) {
            if used {
                grid.set_target(position);
            } else {
                grid.forget_target();
            }
        }
    }

    /// Direction for a unit with `radius` to walk at `position` to reach the target, using the
    /// grid for its radius (see [`Self::grid_index`]). `None` if the position is outside the map,
    /// at the target, or the target can't be reached from there.
    pub fn direction(&self, position: Vec2, radius: f32) -> Option<Vec2> {
        self.grids[self.grid_index(radius)?].direction(position)
    }
}

/// Blocks the cells near `collider` in `grids`, which cover the same cells and are sorted by
/// clearance. The distance of each cell to the collider is computed once for all of them.
fn block(grids: &mut [Grid], collider: &Collider, position: Vec2, rotation: Rotation) {
    let Some(widest) = grids.last() else {
        return;
    };
    let area = collider.aabb(position, rotation, widest.clearance);
    for (i, center) in widest.cells_in(area).collect::<Vec<_>>() {
        let distance = collider.distance_to_point(position, rotation, center, true);
        for grid in &mut *grids {
            grid.blocked[i] |= distance < grid.clearance;
            grid.solid[i] |= distance <= 0.;
        }
    }
}

/// World position of the bottom-left corner and size of the map that `event` reports as created.
pub(crate) fn map_area(
    event: &TiledEvent<MapCreated>,
    maps: &Query<(&Transform, &TilemapAnchor)>,
    assets: &Res<Assets<TiledMapAsset>>,
) -> Option<(Vec2, Vec2)> {
    let asset = event.get_map_asset(assets)?;
    let (transform, anchor) = maps.get(event.origin).ok()?;
    let bottom_left =
        asset.world_space_from_tiled_position(anchor, Vec2::new(0., asset.rect.height()));
    Some((transform.translation.xy() + bottom_left, asset.rect.size()))
}

/// Creates the grids when a map is (re)loaded, covering the whole map.
fn create_grids(
    mut map_created: MessageReader<TiledEvent<MapCreated>>,
    maps: Query<(&Transform, &TilemapAnchor)>,
    assets: Res<Assets<TiledMapAsset>>,
    mut flow_field: ResMut<FlowField>,
) {
    for event in map_created.read() {
        if let Some((origin, size)) = map_area(event, &maps, &assets) {
            *flow_field = FlowField::new(origin, size, CLEARANCES.iter().copied());
        }
    }
}

/// Marks cells near static, solid colliders as blocked and updates the distances whenever the
/// obstacles or the player's cell change, in the grids used by enemies or the debug overlay.
fn update_flow_field(
    mut flow_field: ResMut<FlowField>,
    colliders: Query<
        (
            &Collider,
            &Position,
            &Rotation,
            Ref<ColliderAabb>,
            &ColliderOf,
        ),
        Without<Sensor>,
    >,
    bodies: Query<&RigidBody>,
    player: Single<&Transform, With<player::Player>>,
    enemies: Query<&Enemy>,
    debug: Option<Res<FlowFieldDebug>>,
) {
    let grids_created = flow_field.is_changed();
    let flow_field = flow_field.bypass_change_detection();

    let obstacles = colliders.iter().filter(|(.., collider_of)| {
        bodies
            .get(collider_of.body)
            .is_ok_and(|body| body.is_static())
    });
    let obstacles_changed = grids_created
        || obstacles
            .clone()
            .any(|(_, _, _, aabb, _)| aabb.is_changed());
    if obstacles_changed {
        flow_field.clear();
        for (collider, position, rotation, ..) in obstacles {
            flow_field.block_collider(collider, position.0, *rotation);
        }
    }

    // the shown grid is updated by its own clearance, which selects it
    let shown = debug
        .and_then(|debug| debug.grid)
        .and_then(|i| flow_field.grids().get(i))
        .map(Grid::clearance);
    let radii = enemies.iter().map(|enemy| enemy.radius).chain(shown);
    flow_field.set_target(player.translation.xy(), radii);
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        // in PreUpdate, so the field is up to date when enemies move in Update
        app.init_resource::<FlowField>().add_systems(
            PreUpdate,
            (create_grids, update_flow_field)
                .chain()
                .in_set(perf::Step::FlowField),
        );
    }
}
