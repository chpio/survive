//! Dioxus Native UI on top of the game.
//!
//! The Dioxus `VirtualDom` drives a Blitz document, which is laid out and painted into a Vello
//! scene. The render world renders that scene with Bevy's own wgpu device into an `Image`, which
//! is shown as a fullscreen `ImageNode`. Both only happen when the HUD changed (after input, a
//! re-render, a resize or a redraw request from Blitz); otherwise the image keeps its content.

mod hud;

use crate::{
    combat::Health,
    flowfield::{FlowField, debug::FlowFieldDebug},
    objects::{enemy, player::Player},
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
            Extent3d, TextureDimension, TextureFormat, TextureId, TextureUsages,
            TextureViewDescriptor,
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
    shell::{ColorScheme, ShellProvider, Viewport},
};
use dioxus::prelude::{Modifiers, ReadableExt, ScopeId, Signal, VirtualDom, WritableExt};
use dioxus_native_dom::DioxusDocument;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Instant,
};

/// The Dioxus document and the channels between it and the game. Not `Send`, so it is stored as
/// a non-send resource.
struct UiDocument {
    doc: DioxusDocument,
    started: Instant,
    cursor: Vec2,
    buttons: MouseEventButtons,
    state: Signal<hud::State>,
    actions: mpsc::Receiver<hud::Action>,
    /// Whether the document has to be painted again, e.g. after input
    needs_paint: bool,
    redraw_request: Arc<RedrawRequest>,
}

/// Set by Blitz when the document has to be painted again for reasons of its own.
#[derive(Default)]
struct RedrawRequest(AtomicBool);

impl ShellProvider for RedrawRequest {
    fn request_redraw(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// The painted UI and the image it is rendered into, extracted to the render world whenever it is
/// painted.
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
    let state = vdom.in_scope(ScopeId::ROOT, || Signal::new(hud::State::default()));
    vdom.provide_root_context(state);

    let redraw_request = Arc::new(RedrawRequest::default());
    let mut doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            shell_provider: Some(redraw_request.clone()),
            ..default()
        },
    );
    doc.initial_build();

    world.insert_non_send(UiDocument {
        doc,
        started: Instant::now(),
        cursor: Vec2::ZERO,
        buttons: MouseEventButtons::None,
        state,
        actions: action_rx,
        needs_paint: true,
        redraw_request,
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
        ui.needs_paint = true;
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
        ui.needs_paint = true;
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
    player: Query<&Health, With<Player>>,
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

    let new_state = hud::State {
        player_health: player.single().ok().copied(),
        enemy_count: enemies.iter().count(),
        flow_field_grid: flow_field_debug
            .grid
            .and_then(|i| flow_field.grids().get(i))
            .map(|grid| grid.clearance()),
        show_colliders: gizmos.config::<PhysicsGizmos>().0.enabled,
    };
    let mut state = ui.state;
    // only on changes, since setting the signal re-renders the HUD
    if *state.peek() != new_state {
        ui.doc.vdom.in_scope(ScopeId::ROOT, || state.set(new_state));
    }
}

fn paint_ui(
    mut ui: NonSendMut<UiDocument>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut ui_scene: ResMut<UiScene>,
    mut images: ResMut<Assets<Image>>,
) {
    let ui = &mut *ui;
    // true after a re-render, e.g. when `sync_state` changed the state
    ui.needs_paint |= ui.doc.poll(None);
    ui.needs_paint |= ui.redraw_request.0.swap(false, Ordering::Relaxed);

    let (width, height) = (window.physical_width(), window.physical_height());
    let scale = window.scale_factor();
    let mut doc = ui.doc.inner_mut();

    let viewport = doc.viewport();
    if viewport.window_size != (width, height) || viewport.hidpi_scale != scale {
        ui.needs_paint = true;
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

    if !ui.needs_paint {
        return;
    }
    ui.needs_paint = false;

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

/// Renders the scene when it was painted again, or when the texture was recreated (after a
/// resize, possibly a frame after the scene for the new size).
fn render_ui(
    ui_scene: Res<UiScene>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    renderer: Res<VelloRenderer>,
    mut rendered_texture: Local<Option<TextureId>>,
) {
    let Some(gpu_image) = gpu_images.get(&ui_scene.image) else {
        return;
    };
    let size = gpu_image.texture_descriptor.size;
    if size.width == 0 || size.height == 0 {
        return;
    }
    let texture = Some(gpu_image.texture.id());
    if !ui_scene.is_changed() && *rendered_texture == texture {
        return;
    }
    *rendered_texture = texture;

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
