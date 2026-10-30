use std::num::NonZero;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;
use survival::{
    combat, flowfield, items, movement, objects, perf, settings, sight,
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
            objects::enemy::Plugin,
            objects::player::Plugin,
            objects::well::Plugin,
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
        ))
        .add_plugins((
            sight::Plugin,
            sight::render::Plugin,
            world::Plugin,
            spawning::Plugin,
        ))
        .add_plugins((settings::Plugin, ui::Plugin, perf::Plugin))
        .run();
}

fn startup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn((
        Camera2d,
        // the only camera mesh picking uses (see `items::render::Plugin`)
        bevy::picking::mesh_picking::MeshPickingCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::AutoMax {
                max_width: 500.,
                max_height: 500.,
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
        Mesh2d(meshes.add(objects::player::SHAPE)),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.4, 0.9))),
        // above the map layers
        Transform::from_translation(objects::player::START.extend(10.)),
    ));

    // bevy_ecs_tiled gives the settings to each map it loads, and the maps' events bubble up to
    // the world
    commands
        .spawn((
            TiledWorld(asset_server.load("world.world")),
            // grows with the view in `world::update_chunking`
            TiledWorldChunking::new(world::MIN_LOADED_DISTANCE, world::MIN_LOADED_DISTANCE),
            TilemapRenderSettings {
                render_chunk_size: UVec2::new(3, 1),
                y_sort: true,
            },
            // the maps' positions are those in the .world file, with y pointing up
            TilemapAnchor::TopLeft,
            // layers 2 apart: y-sorting moves the chunks by up to 1.5, and the upper layers stay
            // below the darkening outside the sight (z = 8), the items and the units
            TiledMapLayerZOffset(2.),
            // filled colliders as in Tiled, the default polyline backend only keeps the outlines
            TiledPhysicsSettings::<TiledPhysicsAvianBackend> {
                backend: TiledPhysicsAvianBackend::Triangulation,
                ..default()
            },
        ))
        .observe(
            |collider_created: On<TiledEvent<ColliderCreated>>,
             mut commands: Commands,
             assets: Res<Assets<TiledMapAsset>>| {
                let mut collider = commands.entity(collider_created.event().origin);

                // Automatically insert a `RigidBody::Static` component on all the colliders entities from the map
                collider.insert(RigidBody::Static);

                // a tile layer's colliders are merged into one, so the property applies to the
                // whole layer. Objects can have it themselves or on their tile.
                let object = collider_created.event().object(&assets);
                let blocks_sight = collider_created
                    .event()
                    .layer(&assets)
                    .is_some_and(|layer| blocks_sight(&layer.properties))
                    || object.as_ref().is_some_and(|obj| {
                        blocks_sight(&obj.properties)
                            || obj
                                .get_tile()
                                .and_then(|obj_tile| obj_tile.get_tile())
                                .is_some_and(|tile| blocks_sight(&tile.properties))
                    });
                if blocks_sight {
                    collider.insert(sight::BlocksSight);
                }

                if let Some(obj) = object {
                    let user_type = Some(obj.user_type.clone())
                        .filter(|t| !t.is_empty())
                        .or_else(|| {
                            obj.get_tile()
                                .and_then(|obj_tile| obj_tile.get_tile())
                                .map(|tile| tile.user_type.clone())
                                .filter(|t| !t.is_empty())
                        });

                    match user_type.as_deref() {
                        Some("well") => {
                            collider.insert((objects::well::Well, Sensor, CollisionEventsEnabled));
                        }
                        // the ambush is the object layer the zone is on
                        Some("ambush_trigger") => {
                            if let (Some(layer), Some(layer_data)) = (
                                collider_created.event().layer_entity(),
                                collider_created.event().layer(&assets),
                            ) {
                                collider.insert((
                                    AmbushTrigger::from_layer(layer, &layer_data.properties),
                                    Sensor,
                                ));
                            }
                        }
                        _ => {}
                    }
                };
            },
        )
        // objects without a collider, like points
        .observe(
            |object_created: On<TiledEvent<ObjectCreated>>,
             mut commands: Commands,
             assets: Res<Assets<TiledMapAsset>>| {
                let event = object_created.event();
                let (Some(object), Some(layer)) = (event.object(&assets), event.layer_entity())
                else {
                    return;
                };
                #[expect(
                    clippy::single_match,
                    reason = "new map-driven objects get an arm here"
                )]
                match object.user_type.as_str() {
                    "ambush_spawn" => {
                        commands
                            .entity(event.origin)
                            .insert(SpawnPoint::from_object(layer, &object.properties));
                    }
                    _ => {}
                }
            },
        );
}

/// Whether the Tiled `properties` mark something as blocking the sight.
fn blocks_sight(properties: &tiled::Properties) -> bool {
    matches!(
        properties.get("blocks_sight"),
        Some(tiled::PropertyValue::BoolValue(true))
    )
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
