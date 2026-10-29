//! Flow field towards the player, used by enemies to walk around obstacles.
//!
//! The world is divided into square cells (see [`crate::cells`]). Cells an enemy can't stand in
//! without overlapping a static, solid collider are blocked. A Dijkstra search from the player's
//! cell stores the walking distance to the player in every free cell, and units follow the
//! decreasing distance. Since larger enemies don't fit through every gap, there is one grid per
//! clearance in [`CLEARANCES`]. Units with a free line of sight to the player walk straight
//! towards them instead, since the eight directions between cells would otherwise bend their path.
//!
//! The obstacles are rasterized per map into its [`Obstacles`], only when the map is loaded or
//! its colliders change. The walking distances only cover a square window of a fixed size around
//! the player's cell (see [`FLOW_FIELD_RADIUS`]), which moves along whenever they are computed
//! again. Its blocked cells are put together from the obstacles of the loaded maps then, and
//! cells outside all of them are blocked. So memory and cost don't grow with the size of the
//! world. Only the grids used by awake enemies, requested by spawners ([`Request`]) or shown by
//! the debug overlay are computed at all. Enemies outside the window have no field and walk
//! straight at the player.
//!
//! The player can stand closer to an obstacle than a grid's clearance. So that the player stays
//! reachable then, the cells around the player that are only blocked by the clearance can be
//! walked through as well, but never cells inside an obstacle.

use crate::{
    cells::{self, CellRect, Raster},
    objects::{
        enemy::{Asleep, Enemy},
        player,
    },
    perf,
    world::{self, MapRaster},
};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::{cmp::Reverse, collections::BinaryHeap};

pub mod debug;

use debug::FlowFieldDebug;

pub use crate::cells::CELL_SIZE;

/// Clearances in px with a grid each, sorted. An enemy uses the grid with the smallest clearance
/// that is at least its radius, so it avoids gaps it would only just fit through. Enemies larger
/// than all clearances use the largest one and may get stuck in narrow gaps.
pub const CLEARANCES: &[f32] = &[5., 8., 12., 16.];

/// The largest clearance a grid may have, since the [`Obstacles`] of a map only reach this far
/// beyond it.
pub const MAX_CLEARANCE: f32 = CLEARANCES[CLEARANCES.len() - 1];

/// Cells around a map that its [`Obstacles`] cover as well: those whose center is closer than
/// [`MAX_CLEARANCE`] to the map's edge.
const MARGIN: i32 = (MAX_CLEARANCE / CELL_SIZE + 0.5) as i32;

/// Half the edge length in px of the square window around the player in which the walking
/// distances are computed. A bit more than half the diagonal of the view at the default zoom
/// (500 × 281 px at 16:9), so enemies just outside the view still find their way.
pub const FLOW_FIELD_RADIUS: f32 = 320.;

/// Cells from the player's cell to the edge of the window.
const WINDOW_RADIUS: i32 = (FLOW_FIELD_RADIUS / CELL_SIZE) as i32;

/// Distance in px from the player up to which the maps must be loaded, so the window has all
/// obstacles: the window and the clearance around it.
pub const REACH: f32 = (WINDOW_RADIUS + 1) as f32 * CELL_SIZE + MAX_CLEARANCE;

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

/// Obstacles of one map, a component of the map's entity: for each cell of the map and of a margin
/// of [`MAX_CLEARANCE`] around it, the distance from its center to the closest static, solid
/// collider of the map. So obstacles at the edge of a map also block the cells of the
/// neighboring map that are too close to them.
#[derive(Component, Clone, Debug)]
pub struct Obstacles {
    /// Cells of the map itself
    map: CellRect,
    /// Distance in px per cell, infinite if no obstacle is within the margin
    distance: Raster<f32>,
}

impl Obstacles {
    /// No obstacles yet on the map covering `area`.
    pub fn new(area: Rect) -> Self {
        let map = CellRect::covering(area);
        Self {
            map,
            distance: Raster::new(map.grow(MARGIN), f32::INFINITY),
        }
    }

    /// Records the distance to the shape of `collider` at `position` and `rotation` for the cells
    /// within [`MAX_CLEARANCE`] of it. Only the shape counts, not its bounding box, since the
    /// map's tiles are merged into one collider per layer that spans the whole map.
    pub fn block_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        let area = collider.aabb(position, rotation, MAX_CLEARANCE);
        let cells = CellRect::centers_in(Rect::from_corners(area.min, area.max))
            .intersect(self.distance.rect());
        for cell in cells.cells() {
            let distance =
                collider.distance_to_point(position, rotation, cells::center(cell), true);
            let closest = &mut self.distance[cell];
            *closest = closest.min(distance);
        }
    }
}

impl MapRaster for Obstacles {
    fn new(area: Rect) -> Self {
        Self::new(area)
    }

    fn add_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        self.block_collider(collider, position, rotation);
    }
}

#[derive(Resource)]
pub struct FlowField {
    /// Grids for different clearances, sorted by clearance
    grids: Vec<Grid>,
}

impl Default for FlowField {
    /// One grid per clearance in [`CLEARANCES`].
    fn default() -> Self {
        Self::new(CLEARANCES.iter().copied())
    }
}

/// The window of the flow field for one clearance.
pub struct Grid {
    /// Distance a unit's center keeps from obstacles, so paths only lead through gaps it fits in
    clearance: f32,
    /// Cells around the target cell
    window: Raster<Node>,
    /// Target cell the distances were computed for, `None` while they are outdated
    target: Option<IVec2>,
    target_position: Vec2,
}

/// State of a cell in the window of a [`Grid`].
#[derive(Clone, Copy)]
struct Node {
    blocked: bool,
    /// Whether its center lies inside an obstacle or outside the loaded maps, which blocks it as
    /// well
    solid: bool,
    /// Walking distance to the target, `u32::MAX` if unreachable
    distance: u32,
}

/// A cell outside the loaded maps.
const OUTSIDE: Node = Node {
    blocked: true,
    solid: true,
    distance: u32::MAX,
};

/// State of one cell of a [`Grid`], see [`Grid::cells`].
pub struct Cell {
    /// Column and row on the world's raster of cells
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
    /// Grid without a target for units keeping `clearance` from obstacles.
    fn new(clearance: f32) -> Self {
        Self {
            clearance,
            window: Raster::new(CellRect::around(IVec2::ZERO, WINDOW_RADIUS), OUTSIDE),
            target: None,
            target_position: Vec2::ZERO,
        }
    }

    pub fn clearance(&self) -> f32 {
        self.clearance
    }

    /// Cells of the window, centered on the target cell the distances were last computed for.
    pub fn window(&self) -> CellRect {
        self.window.rect()
    }

    /// All cells of the window row by row, starting at the bottom-left corner.
    pub fn cells(&self) -> impl Iterator<Item = Cell> + '_ {
        self.window.rect().cells().map(|position| Cell {
            position,
            center: cells::center(position),
            blocked: !self.is_free(position),
            distance: Some(self.distance(position)).filter(|&d| d != u32::MAX),
            direction: self.cell_direction(position),
        })
    }

    /// Whether the distances are computed, i.e. the grid has a target.
    pub fn is_computed(&self) -> bool {
        self.target.is_some()
    }

    /// Walking distance in px from `cell` to the target, `None` if it can't be reached.
    fn walking_distance(&self, cell: IVec2) -> Option<f32> {
        let distance = self.distance(cell);
        (distance != u32::MAX).then(|| distance as f32 / STRAIGHT_COST as f32 * CELL_SIZE)
    }

    /// Centers of the free cells that can reach the target, with their walking distance in px:
    /// the places a unit of the grid's clearance can stand on and walk to the target from.
    pub fn reachable(&self) -> impl Iterator<Item = (Vec2, f32)> + '_ {
        self.window
            .rect()
            .cells()
            .filter(|&cell| self.is_free(cell))
            .filter_map(|cell| Some((cells::center(cell), self.walking_distance(cell)?)))
    }

    /// Highest walking distance of a reachable cell, `None` if no cell is reachable.
    pub fn max_distance(&self) -> Option<u32> {
        self.target?;
        self.window
            .values()
            .iter()
            .map(|node| node.distance)
            .filter(|&d| d != u32::MAX)
            .max()
    }

    /// State of `cell`, [`OUTSIDE`] outside the window.
    fn node(&self, cell: IVec2) -> Node {
        self.window.get(cell).copied().unwrap_or(OUTSIDE)
    }

    fn is_free(&self, cell: IVec2) -> bool {
        !self.node(cell).blocked
    }

    /// Whether paths towards the target may lead through `cell`: free cells, and cells within the
    /// clearance around the target cell that are only blocked by the clearance, so the target
    /// stays reachable when it is closer to an obstacle than the clearance.
    fn is_walkable(&self, cell: IVec2) -> bool {
        let node = self.node(cell);
        if !node.blocked {
            return true;
        }
        !node.solid
            && self.target.is_some_and(|target| {
                (cell - target).length_squared() as f32 * CELL_SIZE.powi(2) < self.clearance.powi(2)
            })
    }

    fn distance(&self, cell: IVec2) -> u32 {
        if self.target.is_none() {
            return u32::MAX;
        }
        self.node(cell).distance
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

    /// Computes the walking distances towards `position` on `maps`, if it isn't the current
    /// target already.
    fn set_target(&mut self, position: Vec2, maps: &[&Obstacles]) {
        self.target_position = position;
        let target = cells::cell(position);
        if self.target != Some(target) {
            self.compute_distances(target, maps);
        }
    }

    /// Forgets the target, so the distances count as outdated until the next
    /// [`Self::set_target`].
    fn forget_target(&mut self) {
        self.target = None;
    }

    /// Centers the window on `target` and puts its blocked cells together from `maps`: a cell is
    /// blocked if it lies outside all of them, or if one of them has an obstacle too close to it.
    fn compose(&mut self, target: IVec2, maps: &[&Obstacles]) {
        self.window.reset(target - WINDOW_RADIUS, OUTSIDE);
        let window = self.window.rect();
        for map in maps {
            for cell in window.intersect(map.map).cells() {
                self.window[cell] = Node {
                    blocked: false,
                    solid: false,
                    distance: u32::MAX,
                };
            }
        }
        for map in maps {
            for cell in window.intersect(map.distance.rect()).cells() {
                let distance = map.distance[cell];
                let node = &mut self.window[cell];
                node.blocked |= distance < self.clearance;
                node.solid |= distance <= 0.;
            }
        }
    }

    /// Dijkstra search from `target` within the window, which is moved to be centered on it.
    fn compute_distances(&mut self, target: IVec2, maps: &[&Obstacles]) {
        self.target = Some(target);
        self.compose(target, maps);
        let window = self.window.rect();
        let start = window.index(target).unwrap();
        self.window[start].distance = 0;

        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        while let Some(Reverse((distance, index))) = queue.pop() {
            if distance > self.window[index].distance {
                continue;
            }
            // copied out, since the steps borrow the grid
            let mut steps = [(0, 0); NEIGHBORS.len()];
            let mut count = 0;
            for (next, cost) in self.steps(window.cell(index)) {
                if let Some(next_index) = window.index(next) {
                    steps[count] = (next_index, cost);
                    count += 1;
                }
            }
            for &(next_index, cost) in &steps[..count] {
                let next_distance = distance + cost;
                if next_distance < self.window[next_index].distance {
                    self.window[next_index].distance = next_distance;
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
        (0..=steps as u32).all(|i| self.is_walkable(cells::cell(from.lerp(to, i as f32 / steps))))
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
    /// interpolated between the four nearest cells. `None` outside the window, at the target, or
    /// if the target can't be reached.
    fn direction(&self, position: Vec2) -> Option<Vec2> {
        let cell = cells::cell(position);
        if self.window.get(cell).is_none() || self.target.is_none() {
            return None;
        }
        if self.line_of_sight(position, self.target_position) {
            return (self.target_position - position).try_normalize();
        }

        let local = position / CELL_SIZE - 0.5;
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
    /// One grid per clearance, none larger than [`MAX_CLEARANCE`].
    pub fn new(clearances: impl IntoIterator<Item = f32>) -> Self {
        let mut grids: Vec<_> = clearances.into_iter().map(Grid::new).collect();
        grids.sort_by(|a, b| a.clearance.total_cmp(&b.clearance));
        Self { grids }
    }

    /// Grids sorted by clearance.
    pub fn grids(&self) -> &[Grid] {
        &self.grids
    }

    /// Index of the grid for units with `radius`: the one with the smallest clearance that is at
    /// least `radius`, or the largest one for larger units.
    fn grid_index(&self, radius: f32) -> Option<usize> {
        self.grids
            .iter()
            .position(|grid| grid.clearance >= radius)
            .or(self.grids.len().checked_sub(1))
    }

    /// The grid for units with `radius`, see [`Self::direction`].
    pub fn grid(&self, radius: f32) -> Option<&Grid> {
        self.grids.get(self.grid_index(radius)?)
    }

    /// Walking distance in px for a unit with `radius` from `position` to the target. `None` if
    /// the grid isn't computed, the position is outside its window or blocked for the unit, or the
    /// target can't be reached from there.
    pub fn walking_distance(&self, position: Vec2, radius: f32) -> Option<f32> {
        let grid = self.grid(radius)?;
        let cell = cells::cell(position);
        if !grid.is_free(cell) {
            return None;
        }
        grid.walking_distance(cell)
    }

    /// Computes the walking distances towards `position` on the loaded `maps`, if it isn't the
    /// current target already, only for the grids used by units with the given `radii`. All other
    /// grids forget their target, so they compute it again once a unit needs them.
    pub fn set_target(
        &mut self,
        position: Vec2,
        radii: impl IntoIterator<Item = f32>,
        maps: &[&Obstacles],
    ) {
        let mut used = vec![false; self.grids.len()];
        for radius in radii {
            if let Some(i) = self.grid_index(radius) {
                used[i] = true;
            }
        }
        for (grid, used) in self.grids.iter_mut().zip(used) {
            if used {
                grid.set_target(position, maps);
            } else {
                grid.forget_target();
            }
        }
    }

    /// Forgets the targets of all grids, so the distances are computed again on the next
    /// [`Self::set_target`], e.g. after maps were loaded or changed.
    pub fn forget_targets(&mut self) {
        for grid in &mut self.grids {
            grid.forget_target();
        }
    }

    /// Direction for a unit with `radius` to walk at `position` to reach the target. Uses the grid
    /// with the smallest clearance that is at least `radius`, or the largest one for larger
    /// units. `None` if the position is outside the window, at the target, or the target can't be
    /// reached from there.
    pub fn direction(&self, position: Vec2, radius: f32) -> Option<Vec2> {
        self.grids[self.grid_index(radius)?].direction(position)
    }
}

/// Requests the grids for units with these radii while it exists, e.g. by a spawner that needs the
/// walking distances before the units exist. They are computed in the next search.
#[derive(Component, Clone, Debug)]
pub struct Request(pub Vec<f32>);

/// Updates the distances whenever the loaded maps, their obstacles or the player's cell change,
/// in the grids used by awake enemies, requests or the debug overlay.
fn update_flow_field(
    mut flow_field: ResMut<FlowField>,
    maps: Query<Ref<Obstacles>>,
    mut unloaded: RemovedComponents<Obstacles>,
    player: Single<&Transform, With<player::Player>>,
    enemies: Query<&Enemy, Without<Asleep>>,
    requests: Query<&Request>,
    debug: Option<Res<FlowFieldDebug>>,
) {
    let unloaded = unloaded.read().count() > 0;
    if unloaded || maps.iter().any(|map| map.is_changed()) {
        flow_field.forget_targets();
    }
    let maps: Vec<_> = maps.iter().map(Ref::into_inner).collect();

    // the shown grid is updated by its own clearance, which selects it
    let shown = debug
        .and_then(|debug| debug.grid)
        .and_then(|i| flow_field.grids().get(i))
        .map(Grid::clearance);
    let requested = requests
        .iter()
        .flat_map(|request| request.0.iter().copied());
    let radii = enemies
        .iter()
        .map(|enemy| enemy.radius)
        .chain(requested)
        .chain(shown);
    flow_field.set_target(player.translation.xy(), radii, &maps);
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        // in PreUpdate, so the field is up to date when enemies move in Update. Colliders with
        // a sensor don't block.
        app.init_resource::<FlowField>().add_systems(
            PreUpdate,
            (
                world::rasterize::<Obstacles, Without<Sensor>>,
                update_flow_field,
            )
                .chain()
                .in_set(perf::Step::FlowField),
        );
    }
}
