use avian2d::prelude::*;
use bevy::{
    dev_tools::{fps_overlay, render_debug},
    prelude::*,
    window,
};
use bevy_ecs_tiled::prelude::*;
use std::time::Duration;
use survival::{combat, flowfield, movement, objects, perf, ui};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .build()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        mode: window::WindowMode::Windowed,
                        present_mode: window::PresentMode::Fifo,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                // enabled by the `render_dev_tools` feature (needed for the fps overlay), but requires bevy_pbr
                .disable::<render_debug::RenderDebugOverlayPlugin>(),
        )
        .add_plugins(fps_overlay::FpsOverlayPlugin {
            config: fps_overlay::FpsOverlayConfig {
                refresh_interval: Duration::from_millis(500),
                ..default()
            },
        })
        .add_plugins(TiledPlugin::default())
        .add_systems(Startup, startup)
        .add_systems(
            Update,
            (update_camera_position, toggle_fps, toggle_fullscreen),
        )
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
        .add_plugins((ui::Plugin, perf::Plugin))
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
        Mesh2d(meshes.add(objects::player::SHAPE)),
        MeshMaterial2d(materials.add(Color::srgb(0.2, 0.4, 0.9))),
        // above the map layer at z = 0
        Transform::from_xyz(50., -50., 10.),
    ));

    commands
        .spawn((
            TiledMap(asset_server.load("map.tmx")),
            TilemapRenderSettings {
                render_chunk_size: UVec2::new(3, 1),
                y_sort: true,
            },
            TilemapAnchor::Center,
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

                if let Some(obj) = collider_created.event().get_object(&assets) {
                    let user_type = Some(obj.user_type.clone())
                        .filter(|t| !t.is_empty())
                        .or_else(|| {
                            obj.get_tile()
                                .and_then(|obj_tile| obj_tile.get_tile())
                                .map(|tile| tile.user_type.clone())
                                .filter(|t| !t.is_empty())
                        });

                    #[expect(
                        clippy::single_match,
                        reason = "new map-driven objects get an arm here"
                    )]
                    match user_type.as_deref() {
                        Some("well") => {
                            collider.insert((objects::well::Well, Sensor, CollisionEventsEnabled));
                        }
                        _ => {}
                    }
                };
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

fn toggle_fps(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut fps_overlay: ResMut<fps_overlay::FpsOverlayConfig>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyP) {
        let enabled = !fps_overlay.enabled;
        fps_overlay.enabled = enabled;
        fps_overlay.frame_time_graph_config.enabled = enabled;
    }
}

fn toggle_fullscreen(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut window: Single<&mut Window, With<window::PrimaryWindow>>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyF) {
        let new_mode = match window.mode {
            window::WindowMode::Windowed => {
                window::WindowMode::BorderlessFullscreen(MonitorSelection::Current)
            }
            _ => window::WindowMode::Windowed,
        };
        window.mode = new_mode;
    }
}
