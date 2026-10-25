//! Dioxus Native UI on top of the game.
//!
//! The Dioxus `VirtualDom` drives a Blitz document, which is laid out and painted into a Vello
//! scene every frame. The render world renders that scene with Bevy's own wgpu device into an
//! `Image`, which is shown as a fullscreen `ImageNode`.

mod hud;

use crate::{
    flowfield::{FlowField, debug::FlowFieldDebug},
    objects::enemy,
};
use anyrender_vello::VelloScenePainter;
use avian2d::prelude::PhysicsGizmos;
use bevy::{
    input::{ButtonState, mouse::MouseButtonInput},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
    window::PrimaryWindow,
};
use blitz_dom::{Document as _, DocumentConfig};
use blitz_paint::paint_scene;
use blitz_traits::{
    events::{
        BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, Point,
        PointerCoords, PointerDetails, UiEvent,
    },
    shell::{ColorScheme, Viewport},
};
use dioxus::prelude::{Modifiers, ReadableExt, ScopeId, Signal, VirtualDom, WritableExt};
use dioxus_native_dom::DioxusDocument;
use std::{
    sync::{Mutex, mpsc},
    time::Instant,
};

/// The Dioxus document and the channels between it and the game. Not `Send`, so it is stored as
/// a non-send resource.
struct UiDocument {
    doc: DioxusDocument,
    started: Instant,
    cursor: Vec2,
    buttons: MouseEventButtons,
    enemy_count: Signal<usize>,
    flow_field_grid: Signal<Option<f32>>,
    show_colliders: Signal<bool>,
    actions: mpsc::Receiver<hud::Action>,
}

/// The painted UI and the image it is rendered into, extracted to the render world every frame.
#[derive(Resource, ExtractResource, Clone)]
#[extract_app(RenderApp)]
struct UiScene {
    scene: vello::Scene,
    image: Handle<Image>,
}

#[derive(Resource)]
struct VelloRenderer(Mutex<vello::Renderer>);

fn setup(world: &mut World) {
    let (action_tx, action_rx) = mpsc::channel();
    let vdom = VirtualDom::new(hud::app).with_root_context(action_tx);
    let (enemy_count, flow_field_grid, show_colliders) = vdom.in_scope(ScopeId::ROOT, || {
        (Signal::new(0), Signal::new(None), Signal::new(false))
    });
    vdom.provide_root_context(hud::State {
        enemy_count,
        flow_field_grid,
        show_colliders,
    });

    let mut doc = DioxusDocument::new(vdom, DocumentConfig::default());
    doc.initial_build();

    world.insert_non_send(UiDocument {
        doc,
        started: Instant::now(),
        cursor: Vec2::ZERO,
        buttons: MouseEventButtons::None,
        enemy_count,
        flow_field_grid,
        show_colliders,
        actions: action_rx,
    });

    let mut image = Image::new_uninit(
        Extent3d::default(),
        TextureDimension::D2,
        TextureFormat::Rgba8Unorm,
        default(),
    );
    // Vello writes sRGB-encoded values through a storage binding (which can't be sRGB), so Bevy
    // samples the texture through an sRGB view instead
    image.texture_descriptor.usage |= TextureUsages::STORAGE_BINDING;
    image.texture_descriptor.view_formats = &[TextureFormat::Rgba8UnormSrgb];
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        format: Some(TextureFormat::Rgba8UnormSrgb),
        usage: Some(TextureUsages::TEXTURE_BINDING),
        ..default()
    });
    let image = world.resource_mut::<Assets<Image>>().add(image);

    world.spawn((
        Name::new("UI"),
        ImageNode::new(image.clone()),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            ..default()
        },
    ));

    world.insert_resource(UiScene {
        scene: vello::Scene::new(),
        image,
    });
}

fn forward_input(
    mut ui: NonSendMut<UiDocument>,
    mut cursor_moved: MessageReader<CursorMoved>,
    mut mouse_buttons: MessageReader<MouseButtonInput>,
) {
    let ui = &mut *ui;

    for event in cursor_moved.read() {
        ui.cursor = event.position;
        let event = pointer_event(ui.cursor, MouseEventButton::Main, ui.buttons);
        ui.doc.handle_ui_event(UiEvent::PointerMove(event));
    }

    for event in mouse_buttons.read() {
        let (button, flag) = match event.button {
            MouseButton::Left => (MouseEventButton::Main, MouseEventButtons::Primary),
            MouseButton::Right => (MouseEventButton::Secondary, MouseEventButtons::Secondary),
            MouseButton::Middle => (MouseEventButton::Auxiliary, MouseEventButtons::Auxiliary),
            _ => continue,
        };
        let event = match event.state {
            ButtonState::Pressed => {
                ui.buttons |= flag;
                UiEvent::PointerDown(pointer_event(ui.cursor, button, ui.buttons))
            }
            ButtonState::Released => {
                ui.buttons -= flag;
                UiEvent::PointerUp(pointer_event(ui.cursor, button, ui.buttons))
            }
        };
        ui.doc.handle_ui_event(event);
    }
}

fn pointer_event(
    position: Vec2,
    button: MouseEventButton,
    buttons: MouseEventButtons,
) -> BlitzPointerEvent {
    BlitzPointerEvent {
        id: BlitzPointerId::Mouse,
        is_primary: true,
        // the document is never scrolled, so all coordinates are the same
        coords: PointerCoords {
            page_x: position.x,
            page_y: position.y,
            client_x: position.x,
            client_y: position.y,
            screen_x: position.x,
            screen_y: position.y,
        },
        button,
        buttons,
        mods: Modifiers::empty(),
        details: PointerDetails::default(),
        element: Point::default(),
        active_pointers: default(),
    }
}

fn sync_state(
    ui: NonSend<UiDocument>,
    enemies: Query<(), With<enemy::Enemy>>,
    flow_field: Res<FlowField>,
    mut flow_field_debug: ResMut<FlowFieldDebug>,
    mut gizmos: ResMut<GizmoConfigStore>,
    mut spawn_enemy: MessageWriter<enemy::SpawnEnemy>,
) {
    for action in ui.actions.try_iter() {
        match action {
            hud::Action::SpawnEnemy => {
                spawn_enemy.write(enemy::SpawnEnemy(default()));
            }
            hud::Action::CycleFlowField => {
                flow_field_debug.cycle(flow_field.grids().len());
            }
            hud::Action::ShowColliders(show) => {
                gizmos.config_mut::<PhysicsGizmos>().0.enabled = show;
            }
        }
    }

    let count = enemies.iter().count();
    let grid = flow_field_debug
        .grid
        .and_then(|i| flow_field.grids().get(i))
        .map(|grid| grid.clearance());
    let colliders_shown = gizmos.config::<PhysicsGizmos>().0.enabled;
    let (mut enemy_count, mut flow_field_grid, mut show_colliders) =
        (ui.enemy_count, ui.flow_field_grid, ui.show_colliders);
    ui.doc.vdom.in_scope(ScopeId::ROOT, || {
        if *enemy_count.peek() != count {
            enemy_count.set(count);
        }
        if *flow_field_grid.peek() != grid {
            flow_field_grid.set(grid);
        }
        if *show_colliders.peek() != colliders_shown {
            show_colliders.set(colliders_shown);
        }
    });
}

fn paint_ui(
    mut ui: NonSendMut<UiDocument>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut ui_scene: ResMut<UiScene>,
    mut images: ResMut<Assets<Image>>,
) {
    let ui = &mut *ui;
    ui.doc.poll(None);

    let (width, height) = (window.physical_width(), window.physical_height());
    let scale = window.scale_factor();
    let mut doc = ui.doc.inner_mut();

    let viewport = doc.viewport();
    if viewport.window_size != (width, height) || viewport.hidpi_scale != scale {
        doc.set_viewport(Viewport::new(width, height, scale, ColorScheme::Dark));
        // Blitz aligns a node's inline layout in every measure pass. After a resize, taffy serves
        // the final layout of unchanged nodes from its cache, so centered text (e.g. in buttons)
        // keeps the alignment of a measure pass with a larger width and lands outside its box.
        // A full relayout avoids that.
        let ids: Vec<_> = doc.tree().iter().map(|(id, _)| id).collect();
        for id in ids {
            if let Some(node) = doc.get_node_mut(id) {
                node.invalidate_layout_cache();
            }
        }
        if let Some(mut image) = images.get_mut(&ui_scene.image) {
            image.resize(Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            });
        }
    }

    doc.resolve(ui.started.elapsed().as_secs_f64());

    ui_scene.scene.reset();
    paint_scene(
        &mut VelloScenePainter::new(&mut ui_scene.scene),
        &mut doc,
        scale as f64,
        width,
        height,
        0,
        0,
    );
}

fn init_vello_renderer(mut commands: Commands, device: Res<RenderDevice>) {
    let renderer = vello::Renderer::new(device.wgpu_device(), vello::RendererOptions::default())
        .expect("failed to create vello renderer");
    commands.insert_resource(VelloRenderer(Mutex::new(renderer)));
}

fn render_ui(
    ui_scene: Res<UiScene>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    renderer: Res<VelloRenderer>,
) {
    let Some(gpu_image) = gpu_images.get(&ui_scene.image) else {
        return;
    };
    let size = gpu_image.texture_descriptor.size;
    if size.width == 0 || size.height == 0 {
        return;
    }

    let view = gpu_image.texture.create_view(&TextureViewDescriptor {
        format: Some(TextureFormat::Rgba8Unorm),
        ..default()
    });
    renderer
        .0
        .lock()
        .unwrap()
        .render_to_texture(
            device.wgpu_device(),
            &queue,
            &ui_scene.scene,
            &view,
            &vello::RenderParams {
                base_color: vello::peniko::color::AlphaColor::TRANSPARENT,
                width: size.width,
                height: size.height,
                antialiasing_method: vello::AaConfig::Area,
            },
        )
        .expect("failed to render ui");
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<UiScene>::default())
            .add_systems(Startup, setup)
            .add_systems(PreUpdate, forward_input)
            .add_systems(Update, sync_state)
            .add_systems(PostUpdate, paint_ui);

        app.sub_app_mut(RenderApp)
            .add_systems(RenderStartup, init_vello_renderer)
            .add_systems(Render, render_ui.in_set(RenderSystems::Prepare));
    }
}
