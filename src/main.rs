// shipped Windows builds start without a console window
#![cfg_attr(
    all(not(debug_assertions), not(feature = "dev")),
    windows_subsystem = "windows"
)]

use std::num::NonZero;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::{prelude::*, tiled};
use survival::{
    combat, figure, flowfield, interact, items, light, movement, objects, perf, settings, sight,
    spawning::{
        self,
        ambush::{AmbushTrigger, SpawnPoint},
    },
    ui, world,
};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        // queue only one frame on the GPU, so input shows up a frame earlier
                        desired_maximum_frame_latency: NonZero::new(1),
                        ..default()
                    }),
                    ..default()
                }),
        )
        // black beyond the maps, instead of Bevy's dark gray
        .insert_resource(ClearColor(Color::BLACK))
        .add_plugins(TiledPlugin::default())
        .add_systems(Startup, startup)
        .add_systems(Update, update_camera_position)
        .insert_resource(Gravity::ZERO)
        .add_plugins(TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default())
        .add_plugins(
            PhysicsPlugins::default()
                .with_length_unit(movement::LENGTH_UNIT)
                // physics runs at a fixed 64 Hz, so bodies would only move on some frames
                .set(PhysicsInterpolationPlugin::interpolate_all()),
        )
        .insert_resource(NarrowPhaseConfig {
            contact_tolerance: movement::CONTACT_TOLERANCE,
            ..default()
        })
        .add_plugins(PhysicsDebugPlugin)
        // collider outlines start hidden, the HUD shows them
        .insert_gizmo_config(
            PhysicsGizmos::default(),
            GizmoConfig {
                enabled: false,
                ..default()
            },
        )
        .add_plugins((
            figure::Plugin,
            objects::enemy::Plugin,
            objects::enemy::swarm::Plugin,
            objects::enemy::swarm::values::Plugin,
            objects::player::Plugin,
            objects::well::Plugin,
            objects::chest::Plugin,
        ))
        .add_plugins((
            flowfield::Plugin,
            flowfield::debug::Plugin,
            movement::Plugin,
            combat::Plugin,
            combat::health_bar::Plugin,
        ))
        .add_plugins((
            items::Plugin,
            items::definitions::Plugin,
            items::input::Plugin,
            items::render::Plugin,
            interact::Plugin,
        ))
        .add_plugins((
            sight::Plugin,
            sight::render::Plugin,
            light::Plugin,
            light::values::Plugin,
            light::render::Plugin,
            world::Plugin,
            spawning::Plugin,
        ))
        .add_plugins((settings::Plugin, ui::Plugin, perf::Plugin))
        .run();
}

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        // set by `light::grading`; `settings` gives the camera `Hdr` for it and sets its `Msaa`
        bevy::render::view::ColorGrading::default(),
        // the only camera mesh picking uses (see `items::render::Plugin`)
        bevy::picking::mesh_picking::MeshPickingCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMin {
                min_width: world::VIEW_SIZE.x,
                min_height: world::VIEW_SIZE.y,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));

    commands.spawn((
        objects::player::Player,
        Name::new("Player"),
        objects::player::physics(),
        objects::player::combat(),
        items::hotbar::Hotbar::default(),
        // above the map layers
        Transform::from_translation(objects::player::START.extend(10.)),
        // shown through its children
        Visibility::default(),
        objects::player::body(&asset_server),
    ));

    // bevy_ecs_tiled gives the settings to each map it loads, and the maps' events bubble up to
    // the world
    commands
        .spawn((
            TiledWorld(asset_server.load("world.world")),
            // grows with the view in `world::update_streaming`
            TiledWorldStreaming::new(world::MIN_LOADED_DISTANCE, world::MIN_LOADED_DISTANCE),
            TilemapRenderSettings {
                render_chunk_size: UVec2::new(3, 1),
                y_sort: true,
            },
            // the maps' positions are those in the .world file, with y pointing up
            TilemapAnchor::TopLeft,
            // layers 2 apart: y-sorting moves the chunks by up to 1.5, and the upper layers stay
            // below the darkening outside the sight (z = 8), the items and the units
            TiledMapLayerZOffset(2.),
            // filled colliders as in Tiled, the default polyline strategy only keeps the outlines
            TiledPhysicsSettings::<TiledPhysicsAvianBackend> {
                collider_strategy: TiledColliderStrategy::Triangulation,
                ..default()
            },
        ))
        .observe(
            |collider_created: On<TiledEvent<ColliderCreated>>,
             mut commands: Commands,
             assets: Res<Assets<TiledMapAsset>>,
             properties: Query<&TiledProperties>,
             objects: Query<&TiledObject>| {
                let event = collider_created.event();
                // the layer or object the collider is for; with the triangulation and the merged
                // geometry, it holds the only collider itself
                let mut collider = commands.entity(event.source_entity());

                // Automatically insert a `RigidBody::Static` component on all the colliders entities from the map
                collider.insert(RigidBody::Static);

                // a tile layer's colliders are merged into one, so the properties apply to the
                // whole layer. Objects can have them themselves or on their tile.
                let object = event
                    .object_entity()
                    .and_then(|obj| properties.get(obj).ok());
                let layer = event
                    .layer_entity()
                    .and_then(|layer| properties.get(layer).ok());
                let tile = event
                    .object_entity()
                    .and_then(|obj| match objects.get(obj) {
                        Ok(&TiledObject::Tile {
                            tileset_id,
                            tile_id,
                            ..
                        }) => Some((tileset_id, tile_id)),
                        _ => None,
                    })
                    .and_then(|(tileset_id, tile_id)| {
                        event.map_asset(&assets)?.tile(tileset_id, tile_id)
                    });
                let property = |name| {
                    object
                        .and_then(|obj| obj.properties.get(name))
                        .or_else(|| tile.as_ref().and_then(|tile| tile.properties.get(name)))
                        .or_else(|| layer.and_then(|layer| layer.properties.get(name)))
                        .cloned()
                };
                if matches!(
                    property("blocks_sight"),
                    Some(tiled::PropertyValue::BoolValue(true))
                ) {
                    collider.insert(sight::BlocksSight);
                }
                match property("height") {
                    Some(tiled::PropertyValue::FloatValue(height)) => {
                        collider.insert(light::shadow::CastsShadow(height));
                    }
                    Some(tiled::PropertyValue::IntValue(height)) => {
                        collider.insert(light::shadow::CastsShadow(height as f32));
                    }
                    _ => {}
                }

                let user_type = object
                    .map(|obj| obj.class.as_str())
                    .filter(|t| !t.is_empty())
                    .or_else(|| {
                        tile.as_ref()
                            .map(|tile| tile.user_type.as_str())
                            .filter(|t| !t.is_empty())
                    });
                match user_type {
                    Some("well") => {
                        collider.insert((objects::well::Well, Sensor, CollisionEventsEnabled));
                    }
                    // the ambush is the object layer the zone is on
                    Some("ambush_trigger") => {
                        if let (Some(layer_entity), Some(layer)) = (event.layer_entity(), layer) {
                            collider.insert((
                                AmbushTrigger::from_layer(layer_entity, &layer.properties),
                                Sensor,
                            ));
                        }
                    }
                    _ => {}
                }
            },
        )
        // objects without a collider, like points
        .observe(
            |object_created: On<TiledEvent<ObjectCreated>>,
             mut commands: Commands,
             properties: Query<&TiledProperties>| {
                let event = object_created.event();
                let (Ok(object), Some(layer)) =
                    (properties.get(event.origin), event.layer_entity())
                else {
                    return;
                };
                match object.class.as_str() {
                    "ambush_spawn" => {
                        commands
                            .entity(event.origin)
                            .insert(SpawnPoint::from_object(layer, &object.properties));
                    }
                    "chest" => {
                        objects::chest::insert(&mut commands, event.origin, &object.properties);
                    }
                    _ => {}
                }
            },
        );
}

fn update_camera_position(
    player: Single<
        &Transform,
        (
            Changed<Transform>,
            With<objects::player::Player>,
            Without<Camera>,
        ),
    >,
    mut camera: Single<&mut Transform, With<Camera>>,
) {
    // keep camera z-coordinate unchanged
    camera.translation = player.translation.with_z(camera.translation.z);
}
