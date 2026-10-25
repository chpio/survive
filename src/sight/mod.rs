//! What the player can see (see `docs/sicht.md`).
//!
//! Colliders with [`BlocksSight`] are rasterized into a grid of opaque cells over the map. Around
//! the player's cell, a mask stores how visible each cell is: invisible behind an opaque cell or
//! outside [`SIGHT_RADIUS`], fully visible otherwise, fading out towards the radius. The mask
//! covers a square window of a fixed size that moves along with the player, and is only computed
//! again when the player enters another cell. [`render`] draws it.

pub mod render;

use crate::{flowfield, objects::player::Player, perf};
use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

/// Edge length of a cell in px, the same as the flow field's.
pub const CELL_SIZE: f32 = flowfield::CELL_SIZE;

/// Distance in px from the player up to which it sees.
pub const SIGHT_RADIUS: f32 = 200.;

/// Distance in px before the radius over which the sight fades out.
pub const SIGHT_FADE: f32 = 48.;

/// Cells from the player's cell to the sight radius.
const SIGHT_CELLS: i32 = (SIGHT_RADIUS / CELL_SIZE) as i32;

/// Cells from the player's cell to the edge of the window: the sight radius plus a border of
/// invisible cells, so the mask's texture fades out to invisible at its edges.
const WINDOW_RADIUS: i32 = SIGHT_CELLS + 1;

/// Edge length of the window in cells.
pub const WINDOW_SIZE: i32 = 2 * WINDOW_RADIUS + 1;

/// Marks a collider whose shape blocks the sight, see `docs/sicht.md`.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct BlocksSight;

/// The eight octants around the player as `[xx, xy, yx, yy]`: the cell at `(dx, dy)` in the first
/// octant maps to `(dx * xx + dy * xy, dx * yx + dy * yy)` in the others.
const OCTANTS: [[i32; 4]; 8] = [
    [1, 0, 0, 1],
    [0, 1, 1, 0],
    [0, -1, 1, 0],
    [-1, 0, 0, 1],
    [-1, 0, 0, -1],
    [0, -1, -1, 0],
    [0, 1, -1, 0],
    [1, 0, 0, -1],
];

/// Mask values of the window's cells while in sight, by their distance to the player's cell in
/// the middle: fully visible, fading out towards the radius, 0 beyond it.
fn fade() -> Vec<u8> {
    (0..WINDOW_SIZE * WINDOW_SIZE)
        .map(|i| {
            let offset = IVec2::new(i % WINDOW_SIZE, i / WINDOW_SIZE) - WINDOW_RADIUS;
            let distance = offset.as_vec2().length() * CELL_SIZE;
            let fade = (SIGHT_RADIUS - distance) / SIGHT_FADE;
            (fade.clamp(0., 1.) * 255.).round() as u8
        })
        .collect()
}

/// Opaque cells over the map and the mask of visible cells around the player.
#[derive(Resource)]
pub struct Sight {
    /// World position of the bottom-left corner of the grid
    origin: Vec2,
    size: IVec2,
    opaque: Vec<bool>,
    /// See [`fade`]
    fade: Vec<u8>,
    /// Visibility per cell of the window (0–255), row by row from the bottom
    mask: Vec<u8>,
    /// Cell of the grid at the bottom-left corner of the window, centered on the player's cell
    window_origin: IVec2,
    /// Player's cell the mask was computed for, `None` while it is outdated
    center: Option<IVec2>,
}

impl Default for Sight {
    /// No map, so nothing blocks the sight.
    fn default() -> Self {
        Self::new(Vec2::ZERO, Vec2::ZERO)
    }
}

impl Sight {
    /// Grid covering the area from `origin` (bottom-left corner) to `origin + world_size`, without
    /// opaque cells. Outside of it, nothing blocks the sight.
    pub fn new(origin: Vec2, world_size: Vec2) -> Self {
        let size = (world_size / CELL_SIZE).ceil().as_ivec2();
        Self {
            origin,
            size,
            opaque: vec![false; (size.x * size.y) as usize],
            fade: fade(),
            mask: vec![0; (WINDOW_SIZE * WINDOW_SIZE) as usize],
            window_origin: IVec2::ZERO,
            center: None,
        }
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

    fn is_opaque(&self, cell: IVec2) -> bool {
        self.index(cell).is_some_and(|i| self.opaque[i])
    }

    fn window_index(&self, cell: IVec2) -> Option<usize> {
        let local = cell - self.window_origin;
        (local.cmpge(IVec2::ZERO).all() && local.cmplt(IVec2::splat(WINDOW_SIZE)).all())
            .then(|| (local.y * WINDOW_SIZE + local.x) as usize)
    }

    /// Makes all cells whose center lies in the shape of `collider` at `position` and `rotation`
    /// opaque. The mask is computed again on the next [`Self::set_center`].
    pub fn block_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        let area = collider.aabb(position, rotation, 0.);
        let min = self.cell(area.min).max(IVec2::ZERO);
        let max = self.cell(area.max).min(self.size - 1);
        for y in min.y..=max.y {
            for x in min.x..=max.x {
                let cell = IVec2::new(x, y);
                let center = self.cell_center(cell);
                if collider.distance_to_point(position, rotation, center, true) <= 0. {
                    let i = self.index(cell).unwrap();
                    self.opaque[i] = true;
                }
            }
        }
        self.center = None;
    }

    /// Removes all opaque cells.
    pub fn clear(&mut self) {
        self.opaque.fill(false);
        self.center = None;
    }

    /// Computes the mask around `position`, if it isn't in the cell it was computed for. Returns
    /// whether it was computed.
    pub fn set_center(&mut self, position: Vec2) -> bool {
        let center = self.cell(position);
        if self.center == Some(center) {
            return false;
        }
        self.center = Some(center);
        self.window_origin = center - IVec2::splat(WINDOW_RADIUS);
        self.mask.fill(0);
        self.light(center);
        for octant in OCTANTS {
            self.cast(center, 1, 1., 0., octant);
        }
        true
    }

    /// Marks `cell` as in sight.
    fn light(&mut self, cell: IVec2) {
        if let Some(i) = self.window_index(cell) {
            self.mask[i] = self.fade[i];
        }
    }

    /// Recursive shadowcasting in one octant: lights the cells of the rows from `row` on (rows
    /// further from `center`) whose slopes lie between `start` and `end`. Behind each run of
    /// opaque cells, it continues in a narrower slope range, so the shadows keep their exact
    /// angle. Opaque cells are lit themselves, so the edges of obstacles are visible.
    fn cast(&mut self, center: IVec2, row: i32, mut start: f32, end: f32, octant: [i32; 4]) {
        if start < end {
            return;
        }
        let [xx, xy, yx, yy] = octant;
        let mut new_start = start;
        for j in row..=SIGHT_CELLS {
            let dy = -j;
            let mut blocked = false;
            for dx in -j..=0 {
                // slopes of the cell's two edges as seen from the center
                let left = (dx as f32 - 0.5) / (dy as f32 + 0.5);
                let right = (dx as f32 + 0.5) / (dy as f32 - 0.5);
                if start < right {
                    continue;
                }
                if end > left {
                    break;
                }
                let cell = center + IVec2::new(dx * xx + dy * xy, dx * yx + dy * yy);
                self.light(cell);
                let opaque = self.is_opaque(cell);
                if blocked {
                    if opaque {
                        new_start = right;
                    } else {
                        blocked = false;
                        start = new_start;
                    }
                } else if opaque && j < SIGHT_CELLS {
                    blocked = true;
                    self.cast(center, j + 1, start, left, octant);
                    new_start = right;
                }
            }
            if blocked {
                break;
            }
        }
    }

    /// How visible `position` is, from 0 (not at all) to 1, by the mask value of its cell.
    pub fn visibility(&self, position: Vec2) -> f32 {
        if self.center.is_none() {
            return 0.;
        }
        self.window_index(self.cell(position))
            .map_or(0., |i| self.mask[i] as f32 / 255.)
    }

    /// Visibility per cell of the window (0–255), row by row from the bottom.
    pub fn mask(&self) -> &[u8] {
        &self.mask
    }

    /// Area of the world the mask covers.
    pub fn window_rect(&self) -> Rect {
        let min = self.origin + self.window_origin.as_vec2() * CELL_SIZE;
        Rect::from_corners(min, min + Vec2::splat(WINDOW_SIZE as f32 * CELL_SIZE))
    }
}

/// Covers the map with a new grid when it is (re)loaded.
fn create_grid(
    mut map_created: MessageReader<TiledEvent<MapCreated>>,
    maps: Query<(&Transform, &TilemapAnchor)>,
    assets: Res<Assets<TiledMapAsset>>,
    mut sight: ResMut<Sight>,
) {
    for event in map_created.read() {
        if let Some((origin, size)) = flowfield::map_area(event, &maps, &assets) {
            *sight = Sight::new(origin, size);
        }
    }
}

/// Rasterizes the colliders blocking the sight whenever they change, and computes the mask when
/// the player enters another cell. Only marks the resource as changed then, so the texture is only
/// written when the mask changed.
fn update_sight(
    mut sight: ResMut<Sight>,
    blockers: Query<(&Collider, &Position, &Rotation, Ref<ColliderAabb>), With<BlocksSight>>,
    player: Single<&Transform, With<Player>>,
) {
    let grid_created = sight.is_changed();
    let mut changed = false;
    let inner = sight.bypass_change_detection();
    if grid_created || blockers.iter().any(|(.., aabb)| aabb.is_changed()) {
        inner.clear();
        for (collider, position, rotation, _) in &blockers {
            inner.block_collider(collider, position.0, *rotation);
        }
    }
    changed |= inner.set_center(player.translation.xy());
    if changed {
        sight.set_changed();
    }
}

/// Keeps track of what the player sees. Drawing it needs [`render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sight>().add_systems(
            PreUpdate,
            (create_grid, update_sight)
                .chain()
                .in_set(perf::Step::Sight),
        );
    }
}
