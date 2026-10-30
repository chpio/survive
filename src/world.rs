//! The world: Tiled maps placed next to each other in a `.world` file, of which only those around
//! the camera are loaded (see `docs/welt.md`).
//!
//! bevy_ecs_tiled spawns and despawns the maps by [`TiledWorldChunking`], which [`update_chunking`]
//! keeps large enough. Each loaded map gets its [`MapArea`], and the flow field and the sight keep
//! a raster of the map's colliders on its entity ([`MapRaster`]), so they disappear with it.

use crate::{flowfield, light, sight};
use avian2d::prelude::*;
use bevy::{
    ecs::{
        query::{QueryFilter, QueryItem, ReadOnlyQueryData},
        system::SystemParam,
    },
    platform::collections::HashMap,
    prelude::*,
};
use bevy_ecs_tiled::prelude::*;

/// Area a loaded map covers in world space, on its entity.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct MapArea(pub Rect);

/// Gives each map its [`MapArea`] when it is created (again).
fn insert_map_area(
    mut map_created: MessageReader<TiledEvent<MapCreated>>,
    maps: Query<(&Transform, &TilemapAnchor)>,
    assets: Res<Assets<TiledMapAsset>>,
    mut commands: Commands,
) {
    for event in map_created.read() {
        let Some(asset) = event.map_asset(&assets) else {
            continue;
        };
        let Ok((transform, anchor)) = maps.get(event.origin) else {
            continue;
        };
        // the transform is relative to the world, which stays at the origin
        let bottom_left = transform.translation.xy()
            + asset.world_space_from_tiled_position(anchor, Vec2::new(0., asset.rect.height()));
        commands
            .entity(event.origin)
            .insert(MapArea(Rect::from_corners(
                bottom_left,
                bottom_left + asset.rect.size(),
            )));
    }
}

/// A raster built from colliders of one map, a component of the map's entity. See [`rasterize`].
pub trait MapRaster: Component {
    /// What the raster needs to know about a collider besides its shape, e.g. a component, `()`
    /// for nothing.
    type Data: ReadOnlyQueryData;

    /// Raster without colliders over the map covering `area`.
    fn new(area: Rect) -> Self;

    /// Adds the shape of `collider` at `position` and `rotation`.
    fn add_collider(
        &mut self,
        collider: &Collider,
        position: Vec2,
        rotation: Rotation,
        data: QueryItem<'_, '_, Self::Data>,
    );
}

/// The rasters `R` of the loaded maps, for systems that put them together and need to know when
/// to do it again.
#[derive(SystemParam)]
pub struct MapRasters<'w, 's, R: MapRaster> {
    rasters: Query<'w, 's, Ref<'static, R>>,
    unloaded: RemovedComponents<'w, 's, R>,
}

impl<R: MapRaster> MapRasters<'_, '_, R> {
    /// Whether a map was loaded, rasterized again or unloaded since the system last ran. Reads the
    /// unloaded maps, so it is only true once for each change.
    pub fn changed(&mut self) -> bool {
        let unloaded = self.unloaded.read().count() > 0;
        unloaded || self.rasters.iter().any(|raster| raster.is_changed())
    }

    /// The rasters of all loaded maps.
    pub fn all(&self) -> Vec<&R> {
        self.rasters.iter().map(Ref::into_inner).collect()
    }
}

/// Builds the raster `R` of each map that has none yet or whose area changed, and again whenever
/// one of the map's colliders matching `F` changes. A collider belongs to the closest ancestor with
/// a [`MapArea`]; colliders of maps are static, so they rarely change.
pub fn rasterize<R: MapRaster, F: QueryFilter>(
    maps: Query<(Entity, Ref<MapArea>, Has<R>)>,
    colliders: Query<
        (
            Entity,
            &Collider,
            &Position,
            &Rotation,
            Ref<ColliderAabb>,
            R::Data,
        ),
        F,
    >,
    parents: Query<&ChildOf>,
    mut commands: Commands,
) {
    let map_of = |collider| {
        parents
            .iter_ancestors(collider)
            .find(|&ancestor| maps.contains(ancestor))
    };

    let mut outdated: HashMap<Entity, R> = maps
        .iter()
        .filter(|(_, area, has_raster)| !has_raster || area.is_changed())
        .map(|(map, area, _)| (map, R::new(area.0)))
        .collect();
    for (collider, _, _, _, aabb, _) in &colliders {
        if aabb.is_changed()
            && let Some(map) = map_of(collider)
            && let Ok((_, area, _)) = maps.get(map)
        {
            outdated.entry(map).or_insert_with(|| R::new(area.0));
        }
    }
    if outdated.is_empty() {
        return;
    }

    for (collider_entity, collider, position, rotation, _, data) in &colliders {
        if let Some(raster) = map_of(collider_entity).and_then(|map| outdated.get_mut(&map)) {
            raster.add_collider(collider, position.0, *rotation, data);
        }
    }
    for (map, raster) in outdated {
        // the map may have been unloaded meanwhile
        commands.entity(map).try_insert(raster);
    }
}

/// Distance in px from the camera in each direction up to which maps must be loaded, besides the
/// view: the flow field and the sight need the obstacles around the player, whom the camera
/// follows.
pub const MIN_LOADED_DISTANCE: f32 = if flowfield::REACH > sight::REACH {
    flowfield::REACH
} else {
    sight::REACH
};

/// Loads the maps that the view (with the shadows falling into it) or [`MIN_LOADED_DISTANCE`]
/// around the camera touch, whichever is larger, so it follows the window size and the zoom.
pub fn update_chunking(
    projection: Single<&Projection, With<Camera2d>>,
    mut worlds: Query<&mut TiledWorldChunking>,
) {
    let Projection::Orthographic(projection) = *projection else {
        return;
    };
    let half_size = (projection.area.half_size() + light::shadow::MAX_SHADOW_LENGTH)
        .max(Vec2::splat(MIN_LOADED_DISTANCE));
    for mut chunking in &mut worlds {
        if chunking.0 != Some(half_size) {
            *chunking = TiledWorldChunking(Some(half_size));
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, insert_map_area).add_systems(
            PostUpdate,
            update_chunking.before(TiledPostUpdateSystems::HandleWorldChunking),
        );
    }
}
