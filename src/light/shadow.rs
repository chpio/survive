//! The sun's shadows: colliders with [`CastsShadow`] are rasterized per map into its [`Heights`],
//! and [`ShadowHeights`] puts those of the loaded maps together whenever they change. [`SunShadow`]
//! computes from them which cells lie in the shadow, once per cell rather than per pixel in the
//! shader (at 4K, a cell covers hundreds of pixels), and only again once the sun has moved the
//! shadows' tips by a pixel. `render` draws it.

use super::{DayNight, LightValues, Sun, running, shadows_on};
use crate::{
    cells::{self, CellRect, Raster},
    world::{self, MapRaster, MapRasters},
};
use avian2d::prelude::*;
use bevy::prelude::*;

/// How far shadows reach in px at most, however low the sun is.
pub const MAX_SHADOW_LENGTH: f32 = 64.;

/// Distance in px between the points along a shadow that are marked, half a cell so no cell on its
/// line is skipped.
const SHADOW_STEP: f32 = cells::CELL_SIZE / 2.;

/// How far in px the tip of the longest shadow moves before the shadows are computed again.
const SHADOW_TOLERANCE: f32 = 1.;

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
        for cell in cells::inside(collider, position, rotation, self.0.rect()) {
            self.0[cell] = self.0[cell].max(height);
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

    /// Area of the world the heights cover.
    pub fn rect(&self) -> Rect {
        self.heights.rect().world_rect()
    }
}

/// Puts the heights together again when a map is loaded, unloaded or rasterized again.
fn update_heights(mut maps: MapRasters<Heights>, mut heights: ResMut<ShadowHeights>) {
    if maps.changed() {
        *heights = ShadowHeights::new(&maps.all());
    }
}

/// Which cells of the [`ShadowHeights`] lie in the sun's shadow.
#[derive(Resource, Clone, Debug, Default)]
pub struct SunShadow {
    /// 255 in the shadow, 0 in the sun, over the heights' rectangle
    mask: Option<Raster<u8>>,
    /// Where the tip of the longest shadow was, relative to its caster, when the mask was computed
    tip: Vec2,
}

impl SunShadow {
    /// The tip of the shadow of the highest caster in `heights` under `sun`, relative to the
    /// caster.
    fn tip(heights: &ShadowHeights, sun: Sun) -> Vec2 {
        -sun.direction * shadow_length(heights.max(), sun)
    }

    /// Whether the shadows under `sun` differ visibly from those of the mask.
    pub fn is_outdated(&self, heights: &ShadowHeights, sun: Sun) -> bool {
        self.mask.is_none() || self.tip.distance(Self::tip(heights, sun)) > SHADOW_TOLERANCE
    }

    /// Computes the mask: each caster marks the cells along its shadow, away from the sun and as
    /// long as its height under the sun's slope (at most [`MAX_SHADOW_LENGTH`]). The casters' own
    /// cells stay in the sun, like the tops of trees and roofs.
    pub fn compute(heights: &ShadowHeights, sun: Sun) -> Self {
        let rect = heights.heights.rect();
        let mut mask = Raster::new(rect, 0);
        let step = -sun.direction * SHADOW_STEP;
        for (i, &height) in heights.heights.values().iter().enumerate() {
            if height == 0 {
                continue;
            }
            let caster = cells::center(rect.cell(i));
            let steps = (shadow_length(height as f32, sun) / SHADOW_STEP) as u32;
            for n in 1..=steps {
                let cell = cells::cell(caster + step * n as f32);
                if let Some(j) = rect.index(cell)
                    && heights.heights[j] == 0
                {
                    mask[j] = 255;
                }
            }
        }
        Self {
            mask: Some(mask),
            tip: Self::tip(heights, sun),
        }
    }

    /// Whether `position` lies in the shadow.
    pub fn shades(&self, position: Vec2) -> bool {
        self.mask
            .as_ref()
            .and_then(|mask| mask.get(cells::cell(position)))
            .is_some_and(|&shadow| shadow > 0)
    }

    /// The mask, row by row from the bottom, `None` until it is computed.
    pub fn mask(&self) -> Option<&Raster<u8>> {
        self.mask.as_ref()
    }
}

/// Length in px of the shadow of something `height` px high under `sun`.
fn shadow_length(height: f32, sun: Sun) -> f32 {
    if sun.slope <= 0. {
        return MAX_SHADOW_LENGTH;
    }
    (height / sun.slope).min(MAX_SHADOW_LENGTH)
}

/// Computes the shadows again when the heights changed or the sun moved them visibly. Only marks
/// the resource as changed then, so the texture is only written when the mask changed. Without
/// sun, the mask stays as it was: the light layer doesn't draw it then.
fn update_sun_shadow(
    heights: Res<ShadowHeights>,
    day_night: Res<DayNight>,
    values: Res<LightValues>,
    mut shadow: ResMut<SunShadow>,
) {
    let Some(sun) = day_night.sun(&values) else {
        return;
    };
    if heights.is_changed() || shadow.is_outdated(&heights, sun) {
        *shadow = SunShadow::compute(&heights, sun);
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShadowHeights>()
            .init_resource::<SunShadow>()
            .add_systems(
                PreUpdate,
                (
                    world::rasterize::<Heights, With<CastsShadow>>,
                    update_heights,
                )
                    .chain(),
            )
            .add_systems(Update, update_sun_shadow.run_if(running).run_if(shadows_on));
    }
}
