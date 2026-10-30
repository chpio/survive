//! What casts the sun's shadows: colliders with [`CastsShadow`] are rasterized per map into its
//! [`Heights`], and [`ShadowHeights`] puts those of the loaded maps together whenever they change.
//! The shadows themselves are computed in the shader of the light layer (`render`), so the sun can
//! move on every frame without computing anything here.

use crate::{
    cells::{self, CellRect, Raster},
    world::{self, MapRaster},
};
use avian2d::prelude::*;
use bevy::prelude::*;

/// How far shadows reach in px at most, however low the sun is. The shader searches up to this far
/// (`STEP * STEPS` in `assets/shaders/light.wesl`).
pub const MAX_SHADOW_LENGTH: f32 = 64.;

/// Makes a collider cast a shadow in the sun, as high as its value in px (Tiled's `height`). Its
/// shape is the shadow's footprint, like that of the opaque cells for the sight.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct CastsShadow(pub f32);

/// Height in px per cell of one map, from the colliders that cast shadows, a component of the
/// map's entity.
#[derive(Component, Clone, Debug)]
pub struct Heights(Raster<u8>);

impl Heights {
    /// No shadow casters yet on the map covering `area`.
    pub fn new(area: Rect) -> Self {
        Self(Raster::new(CellRect::covering(area), 0))
    }

    /// Raises all cells whose center lies in the shape of `collider` at `position` and `rotation`
    /// to `height` (at most 255 px).
    pub fn add(&mut self, collider: &Collider, position: Vec2, rotation: Rotation, height: f32) {
        let height = height.clamp(0., 255.).round() as u8;
        let area = collider.aabb(position, rotation, 0.);
        let cells =
            CellRect::centers_in(Rect::from_corners(area.min, area.max)).intersect(self.0.rect());
        for cell in cells.cells() {
            if collider.distance_to_point(position, rotation, cells::center(cell), true) <= 0. {
                self.0[cell] = self.0[cell].max(height);
            }
        }
    }
}

impl MapRaster for Heights {
    type Data = &'static CastsShadow;

    fn new(area: Rect) -> Self {
        Self::new(area)
    }

    fn add_collider(
        &mut self,
        collider: &Collider,
        position: Vec2,
        rotation: Rotation,
        casts_shadow: &CastsShadow,
    ) {
        self.add(collider, position, rotation, casts_shadow.0);
    }
}

/// The heights of all loaded maps, in the smallest rectangle around them.
#[derive(Resource, Clone, Debug)]
pub struct ShadowHeights {
    heights: Raster<u8>,
    max: u8,
}

impl Default for ShadowHeights {
    fn default() -> Self {
        Self {
            heights: Raster::new(CellRect::new(IVec2::ZERO, IVec2::ZERO), 0),
            max: 0,
        }
    }
}

impl ShadowHeights {
    /// Puts the heights of the loaded `maps` together.
    pub fn new(maps: &[&Heights]) -> Self {
        let Some(rect) = maps.iter().map(|map| map.0.rect()).reduce(|a, b| {
            let min = a.min.min(b.min);
            CellRect::new(min, a.max().max(b.max()) - min)
        }) else {
            return Self::default();
        };
        let mut heights = Raster::new(rect, 0);
        for map in maps {
            for cell in map.0.rect().cells() {
                heights[cell] = heights[cell].max(map.0[cell]);
            }
        }
        let max = heights.values().iter().copied().max().unwrap_or(0);
        Self { heights, max }
    }

    /// Height in px at `position`, 0 outside the loaded maps.
    pub fn height(&self, position: Vec2) -> f32 {
        self.heights
            .get(cells::cell(position))
            .map_or(0., |&height| height as f32)
    }

    /// The highest shadow caster in px.
    pub fn max(&self) -> f32 {
        self.max as f32
    }

    /// Height per cell in px, row by row from the bottom.
    pub fn values(&self) -> &[u8] {
        self.heights.values()
    }

    /// Number of columns and rows.
    pub fn size(&self) -> UVec2 {
        self.heights.rect().size.as_uvec2()
    }

    /// Area of the world the heights cover.
    pub fn rect(&self) -> Rect {
        self.heights.rect().world_rect()
    }
}

/// Puts the heights together again when a map is loaded, unloaded or rasterized again.
fn update_heights(
    maps: Query<Ref<Heights>>,
    mut unloaded: RemovedComponents<Heights>,
    mut heights: ResMut<ShadowHeights>,
) {
    let unloaded = unloaded.read().count() > 0;
    if !unloaded && !maps.iter().any(|map| map.is_changed()) {
        return;
    }
    let maps: Vec<_> = maps.iter().map(Ref::into_inner).collect();
    *heights = ShadowHeights::new(&maps);
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShadowHeights>().add_systems(
            PreUpdate,
            (
                world::rasterize::<Heights, With<CastsShadow>>,
                update_heights,
            )
                .chain(),
        );
    }
}
