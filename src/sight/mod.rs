//! What the player can see (see `docs/sicht.md`).
//!
//! Colliders with [`BlocksSight`] are rasterized per map into its [`OpaqueCells`]. Around the
//! player's cell, a mask stores how visible each cell is: invisible behind an opaque cell or
//! outside the player's [`SightRadius`], fully visible otherwise, fading out towards the radius.
//! The mask covers a square window of a fixed size that moves along with the player. Which cells
//! the player could see up to [`MAX_SIGHT_RADIUS`] is only computed again when the player enters
//! another cell or the loaded maps change; the opaque cells of the window are put together from
//! those of the loaded maps then, and outside of them, nothing blocks the sight. When only the
//! radius changes, the mask is put together again from those cells. [`render`] draws it.

pub mod render;

use crate::{
    cells::{self, CellRect, Raster},
    objects::player::Player,
    world::{self, MapRaster},
};
use avian2d::prelude::*;
use bevy::prelude::*;

pub use crate::cells::CELL_SIZE;

/// Largest distance in px up to which the player sees, which the window is made for. The
/// [`SightRadius`] is capped at it. Well below `enemy::DEACTIVATION_RADIUS`, so enemies the player
/// sees don't fall asleep right away.
pub const MAX_SIGHT_RADIUS: f32 = 300.;

/// Distance in px up to which the player sees until something sets its [`SightRadius`], like the
/// day-night cycle (`light`).
pub const DEFAULT_SIGHT_RADIUS: f32 = 200.;

/// Distance in px before the radius over which the sight fades out.
pub const SIGHT_FADE: f32 = 48.;

/// Cells from the player's cell to the largest sight radius.
const MAX_SIGHT_CELLS: i32 = (MAX_SIGHT_RADIUS / CELL_SIZE) as i32;

/// Cells from the player's cell to the edge of the window: the largest sight radius plus a border
/// of invisible cells, so the mask's texture fades out to invisible at its edges.
const WINDOW_RADIUS: i32 = MAX_SIGHT_CELLS + 1;

/// Edge length of the window in cells.
pub const WINDOW_SIZE: i32 = 2 * WINDOW_RADIUS + 1;

/// Distance in px from the player up to which the maps must be loaded, so the window has all
/// opaque cells.
pub const REACH: f32 = (WINDOW_RADIUS + 1) as f32 * CELL_SIZE;

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

/// How far the player sees in px. Like `Transform` and `GlobalTransform`, it has a base value,
/// which the day-night cycle sets, and the final one, which [`Sight`] uses. Bonuses from
/// equipment will go on top of the base; for now, the final value is the base capped at
/// [`MAX_SIGHT_RADIUS`].
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct SightRadius {
    pub base: f32,
    computed: f32,
}

impl Default for SightRadius {
    fn default() -> Self {
        Self::new(DEFAULT_SIGHT_RADIUS)
    }
}

impl SightRadius {
    pub fn new(base: f32) -> Self {
        Self {
            base,
            computed: Self::compute(base),
        }
    }

    fn compute(base: f32) -> f32 {
        base.clamp(0., MAX_SIGHT_RADIUS)
    }

    /// The final value.
    pub fn get(&self) -> f32 {
        self.computed
    }
}

fn compute_sight_radius(mut radii: Query<&mut SightRadius, Changed<SightRadius>>) {
    for mut radius in &mut radii {
        let computed = SightRadius::compute(radius.base);
        if radius.computed != computed {
            radius.computed = computed;
        }
    }
}

/// Mask values of the window's cells while in sight, by their distance to the player's cell in
/// the middle: fully visible, fading out towards `radius`, 0 beyond it.
fn fade(radius: f32) -> Vec<u8> {
    (0..WINDOW_SIZE * WINDOW_SIZE)
        .map(|i| {
            let offset = IVec2::new(i % WINDOW_SIZE, i / WINDOW_SIZE) - WINDOW_RADIUS;
            let distance = offset.as_vec2().length() * CELL_SIZE;
            let fade = (radius - distance) / SIGHT_FADE;
            (fade.clamp(0., 1.) * 255.).round() as u8
        })
        .collect()
}

/// Cells of one map that block the sight, a component of the map's entity.
#[derive(Component, Clone, Debug)]
pub struct OpaqueCells(Raster<bool>);

impl OpaqueCells {
    /// No opaque cells yet on the map covering `area`.
    pub fn new(area: Rect) -> Self {
        Self(Raster::new(CellRect::covering(area), false))
    }

    /// Makes all cells whose center lies in the shape of `collider` at `position` and `rotation`
    /// opaque.
    pub fn block_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        let area = collider.aabb(position, rotation, 0.);
        let cells =
            CellRect::centers_in(Rect::from_corners(area.min, area.max)).intersect(self.0.rect());
        for cell in cells.cells() {
            if collider.distance_to_point(position, rotation, cells::center(cell), true) <= 0. {
                self.0[cell] = true;
            }
        }
    }
}

impl MapRaster for OpaqueCells {
    fn new(area: Rect) -> Self {
        Self::new(area)
    }

    fn add_collider(&mut self, collider: &Collider, position: Vec2, rotation: Rotation) {
        self.block_collider(collider, position, rotation);
    }
}

/// The mask of visible cells around the player.
#[derive(Resource)]
pub struct Sight {
    /// See [`fade`], for `radius`
    fade: Vec<u8>,
    /// Distance in px up to which the player sees, see [`SightRadius`]
    radius: f32,
    /// Opaque cells of the window, put together from the loaded maps
    opaque: Raster<bool>,
    /// Cells of the window the player could see up to [`MAX_SIGHT_RADIUS`]
    lit: Raster<bool>,
    /// Visibility per cell of the window (0–255)
    mask: Raster<u8>,
    /// Player's cell the mask was computed for, `None` while it is outdated
    center: Option<IVec2>,
}

impl Default for Sight {
    fn default() -> Self {
        let window = CellRect::around(IVec2::ZERO, WINDOW_RADIUS);
        Self {
            fade: fade(DEFAULT_SIGHT_RADIUS),
            radius: DEFAULT_SIGHT_RADIUS,
            opaque: Raster::new(window, false),
            lit: Raster::new(window, false),
            mask: Raster::new(window, 0),
            center: None,
        }
    }
}

impl Sight {
    fn is_opaque(&self, cell: IVec2) -> bool {
        self.opaque.get(cell).is_some_and(|&opaque| opaque)
    }

    /// Computes the mask around `position` with the opaque cells of the loaded `maps`, if it
    /// isn't in the cell it was computed for. Returns whether it was computed.
    pub fn set_center(&mut self, position: Vec2, maps: &[&OpaqueCells]) -> bool {
        let center = cells::cell(position);
        if self.center == Some(center) {
            return false;
        }
        self.center = Some(center);
        let min = center - WINDOW_RADIUS;
        self.opaque.reset(min, false);
        for map in maps {
            for cell in self.opaque.rect().intersect(map.0.rect()).cells() {
                self.opaque[cell] |= map.0[cell];
            }
        }
        self.lit.reset(min, false);
        self.mask.reset(min, 0);
        self.light(center);
        for octant in OCTANTS {
            self.cast(center, 1, 1., 0., octant);
        }
        self.apply_radius();
        true
    }

    /// Puts the mask together again if `radius` (capped at [`MAX_SIGHT_RADIUS`]) changed, without
    /// computing which cells are in sight again. Returns whether it changed.
    pub fn set_radius(&mut self, radius: f32) -> bool {
        let radius = radius.clamp(0., MAX_SIGHT_RADIUS);
        if self.radius == radius {
            return false;
        }
        self.radius = radius;
        self.fade = fade(radius);
        self.apply_radius();
        true
    }

    /// Puts the mask together from the lit cells and the fade towards the radius.
    fn apply_radius(&mut self) {
        for i in 0..self.mask.rect().len() {
            self.mask[i] = if self.lit[i] { self.fade[i] } else { 0 };
        }
    }

    /// Forgets the center, so the mask is computed again on the next [`Self::set_center`], e.g.
    /// after maps were loaded or changed.
    pub fn forget_center(&mut self) {
        self.center = None;
    }

    /// Marks `cell` as in sight.
    fn light(&mut self, cell: IVec2) {
        if let Some(i) = self.lit.rect().index(cell) {
            self.lit[i] = true;
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
        for j in row..=MAX_SIGHT_CELLS {
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
                } else if opaque && j < MAX_SIGHT_CELLS {
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
        self.mask
            .get(cells::cell(position))
            .map_or(0., |&mask| mask as f32 / 255.)
    }

    /// Whether a sprite at `position` is drawn: whether its cell is at least half visible. Only the
    /// map is drawn outside the sight; sprites can't be masked like the enemies' bodies, so they are
    /// shown or hidden as a whole (see [`render::HiddenOutsideSight`]).
    pub fn shows(&self, position: Vec2) -> bool {
        self.visibility(position) >= 0.5
    }

    /// Whether the player sees any part of the circle at `center` with `radius`: whether one of
    /// the cells it overlaps is in sight. Everything outside the window is out of sight.
    pub fn sees(&self, center: Vec2, radius: f32) -> bool {
        if self.center.is_none() {
            return false;
        }
        let area = Rect::from_center_half_size(center, Vec2::splat(radius));
        let cells = CellRect::covering(area).intersect(self.mask.rect());
        cells.cells().any(|cell| {
            let cell_rect =
                Rect::from_center_half_size(cells::center(cell), Vec2::splat(CELL_SIZE / 2.));
            let closest = center.clamp(cell_rect.min, cell_rect.max);
            closest.distance_squared(center) < radius * radius && self.mask[cell] > 0
        })
    }

    /// Visibility per cell of the window (0–255), row by row from the bottom.
    pub fn mask(&self) -> &[u8] {
        self.mask.values()
    }

    /// Area of the world the mask covers.
    pub fn window_rect(&self) -> Rect {
        self.mask.rect().world_rect()
    }
}

/// Computes the mask when the player enters another cell, the loaded maps change or its sight
/// radius changes. Only marks the resource as changed then, so the texture is only written when
/// the mask changed.
fn update_sight(
    mut sight: ResMut<Sight>,
    maps: Query<Ref<OpaqueCells>>,
    mut unloaded: RemovedComponents<OpaqueCells>,
    player: Single<(&Transform, &SightRadius), With<Player>>,
) {
    let (transform, radius) = *player;
    let inner = sight.bypass_change_detection();
    let unloaded = unloaded.read().count() > 0;
    if unloaded || maps.iter().any(|map| map.is_changed()) {
        inner.forget_center();
    }
    let maps: Vec<_> = maps.iter().map(Ref::into_inner).collect();
    // `|`, not `||`: both have to happen
    if inner.set_radius(radius.get()) | inner.set_center(transform.translation.xy(), &maps) {
        sight.set_changed();
    }
}

/// Keeps track of what the player sees. Drawing it needs [`render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sight>().add_systems(
            PreUpdate,
            (
                (
                    world::rasterize::<OpaqueCells, With<BlocksSight>>,
                    compute_sight_radius,
                ),
                update_sight,
            )
                .chain(),
        );
    }
}
