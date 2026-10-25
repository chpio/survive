//! Flow field towards the player, used by enemies to walk around obstacles.
//!
//! The map is divided into a grid of square cells. Cells an enemy can't stand in without
//! overlapping a static, solid collider are blocked. A Dijkstra search from the player's cell stores the walking distance to the player in
//! every free cell, and units follow the decreasing distance. Units with a free line of sight to
//! the player walk straight towards them instead, since the eight directions between cells would
//! otherwise bend their path.

use crate::objects::{enemy, player};
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use std::{cmp::Reverse, collections::BinaryHeap};

/// Edge length of a cell in pixels (a quarter tile). Small enough that a corridor one tile wide
/// still has free cells after the obstacles around it are inflated by [`CLEARANCE`].
pub const CELL_SIZE: f32 = 4.;

/// Distance an enemy's center keeps from obstacles, so paths only lead through gaps it fits in.
pub const CLEARANCE: f32 = enemy::SHAPE.radius;

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
    grid: Option<Grid>,
}

pub struct Grid {
    /// World position of the bottom-left corner of the grid
    origin: Vec2,
    size: IVec2,
    blocked: Vec<bool>,
    /// Walking distance to the target per cell, `u32::MAX` if unreachable
    distance: Vec<u32>,
    /// Target cell the distances were computed for
    target: Option<IVec2>,
    target_position: Vec2,
}

impl Grid {
    /// Grid covering the area from `origin` (bottom-left corner) to `origin + world_size`, with
    /// no obstacles and no target.
    pub fn new(origin: Vec2, world_size: Vec2) -> Self {
        let size = (world_size / CELL_SIZE).ceil().as_ivec2();
        let len = (size.x * size.y) as usize;
        Self {
            origin,
            size,
            blocked: vec![false; len],
            distance: vec![u32::MAX; len],
            target: None,
            target_position: Vec2::ZERO,
        }
    }

    fn cell(&self, position: Vec2) -> IVec2 {
        ((position - self.origin) / CELL_SIZE).floor().as_ivec2()
    }

    fn index(&self, cell: IVec2) -> Option<usize> {
        (cell.cmpge(IVec2::ZERO).all() && cell.cmplt(self.size).all())
            .then(|| (cell.y * self.size.x + cell.x) as usize)
    }

    fn is_free(&self, cell: IVec2) -> bool {
        self.index(cell).is_some_and(|i| !self.blocked[i])
    }

    fn distance(&self, cell: IVec2) -> u32 {
        self.index(cell).map_or(u32::MAX, |i| self.distance[i])
    }

    /// Blocks all cells whose center is closer than [`CLEARANCE`] to `obstacle`.
    pub fn block(&mut self, obstacle: Rect) {
        let obstacle = obstacle.inflate(CLEARANCE);
        // cells whose center lies in the inflated rect
        let min = ((obstacle.min - self.origin) / CELL_SIZE - 0.5)
            .ceil()
            .as_ivec2();
        let max = ((obstacle.max - self.origin) / CELL_SIZE - 0.5)
            .floor()
            .as_ivec2();
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                if let Some(i) = self.index(IVec2::new(x, y)) {
                    self.blocked[i] = true;
                }
            }
        }
    }

    /// Removes all obstacles.
    pub fn clear(&mut self) {
        self.blocked.fill(false);
    }

    /// Neighbors of `cell` that can be walked to, with the cost of the step. Diagonal steps are
    /// only allowed if both adjacent straight neighbors are free, so paths don't cut corners.
    fn steps(&self, cell: IVec2) -> impl Iterator<Item = (IVec2, u32)> + '_ {
        NEIGHBORS.into_iter().filter_map(move |offset| {
            let next = cell + offset;
            if !self.is_free(next) {
                return None;
            }
            if offset.x != 0 && offset.y != 0 {
                let corners_free = self.is_free(cell + IVec2::new(offset.x, 0))
                    && self.is_free(cell + IVec2::new(0, offset.y));
                corners_free.then_some((next, DIAGONAL_COST))
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

    fn compute_distances(&mut self, target: IVec2) {
        self.target = Some(target);
        self.distance.fill(u32::MAX);
        let Some(start) = self.index(target) else {
            return;
        };
        self.distance[start] = 0;

        let mut queue = BinaryHeap::from([Reverse((0, start))]);
        while let Some(Reverse((distance, index))) = queue.pop() {
            if distance > self.distance[index] {
                continue;
            }
            let cell = IVec2::new(index as i32 % self.size.x, index as i32 / self.size.x);
            let steps: Vec<_> = self.steps(cell).collect();
            for (next, cost) in steps {
                let next_index = self.index(next).unwrap();
                let next_distance = distance + cost;
                if next_distance < self.distance[next_index] {
                    self.distance[next_index] = next_distance;
                    queue.push(Reverse((next_distance, next_index)));
                }
            }
        }
    }

    /// Whether all cells on the line from `from` to `to` are free.
    fn line_of_sight(&self, from: Vec2, to: Vec2) -> bool {
        // samples half a cell apart can only skip a cell at its corner, which is still within
        // the clearance around an obstacle
        let steps = (from.distance(to) / (CELL_SIZE / 2.)).ceil().max(1.);
        (0..=steps as u32).all(|i| self.is_free(self.cell(from.lerp(to, i as f32 / steps))))
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
    /// Direction to walk at `position` to reach the player. `None` if the position is outside
    /// the map, at the player, or the player can't be reached from there.
    pub fn direction(&self, position: Vec2) -> Option<Vec2> {
        self.grid.as_ref()?.direction(position)
    }
}

/// Creates the grid when a map is (re)loaded, covering the whole map.
fn create_grid(
    mut map_created: MessageReader<TiledEvent<MapCreated>>,
    maps: Query<(&Transform, &TilemapAnchor)>,
    assets: Res<Assets<TiledMapAsset>>,
    mut flow_field: ResMut<FlowField>,
) {
    for event in map_created.read() {
        let (Some(asset), Ok((transform, anchor))) =
            (event.get_map_asset(&assets), maps.get(event.origin))
        else {
            continue;
        };
        let bottom_left =
            asset.world_space_from_tiled_position(anchor, Vec2::new(0., asset.rect.height()));
        flow_field.grid = Some(Grid::new(
            transform.translation.xy() + bottom_left,
            asset.rect.size(),
        ));
    }
}

/// Marks cells covered by static, solid colliders as blocked and updates the distances whenever
/// the obstacles or the player's cell change.
fn update_flow_field(
    mut flow_field: ResMut<FlowField>,
    obstacles: Query<(Ref<ColliderAabb>, &RigidBody), Without<Sensor>>,
    player: Single<&Transform, With<player::Player>>,
) {
    let grid_created = flow_field.is_changed();
    let Some(grid) = flow_field.bypass_change_detection().grid.as_mut() else {
        return;
    };

    let obstacles_changed = grid_created
        || obstacles
            .iter()
            .any(|(aabb, body)| body.is_static() && aabb.is_changed());
    if obstacles_changed {
        grid.clear();
        for (aabb, body) in &obstacles {
            if body.is_static() {
                grid.block(Rect::from_corners(aabb.min, aabb.max));
            }
        }
        grid.target = None;
    }

    grid.set_target(player.translation.xy());
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        // in PreUpdate, so the field is up to date when enemies move in Update
        app.init_resource::<FlowField>()
            .add_systems(PreUpdate, (create_grid, update_flow_field).chain());
    }
}
