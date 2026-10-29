//! Dioxus Native UI on top of the game.
//!
//! The Dioxus `VirtualDom` drives a Blitz document, which is laid out and painted into a Vello
//! scene. The render world renders that scene with Bevy's own wgpu device into an `Image`, which
//! is shown as a fullscreen `ImageNode`. Both only happen when the HUD changed (after a mouse
//! button, a re-render, a resize or a redraw request from Blitz); otherwise the image keeps its content.
//!
//! Blitz gets all mouse input and touches. Where the HUD [blocks the pointer](blocks_pointer), it
//! reports a picking hit on its image above the sprites, so the game world only gets the pointer
//! elsewhere (see `docs/ui-events.md`).

pub mod hud;
pub mod stylesheet;

use crate::{
    combat::Health,
    flowfield::{FlowField, debug::FlowFieldDebug},
    items::{
        DropItem, Inventory, Item, ItemDefinitions, ItemId, Stack, UseItem,
        hotbar::{AssignSlot, Hotbar, SLOTS, UseSlot},
    },
    objects::{enemy, player::Player},
    perf,
    settings::{self, Menu, Settings},
    spawning::out_of_sight::SpawnOutOfSight,
};
use anyrender_vello::VelloScenePainter;
use avian2d::prelude::PhysicsGizmos;
use bevy::{
    input::{
        ButtonState,
        mouse::MouseButtonInput,
        touch::{TouchInput, TouchPhase},
    },
    picking::{
        PickingSystems,
        backend::{HitData, PointerHits},
        pointer::{PointerId, PointerLocation},
    },
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
use blitz_dom::{BaseDocument, Document as _, DocumentConfig, local_name};
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
use style::selector_parser::RestyleDamage;
use stylesheet::{Stylesheet, StylesheetLoader};

/// The Dioxus document and the channels between it and the game. Not `Send`, so it is stored as
/// a non-send resource.
struct UiDocument {
    doc: DioxusDocument,
    started: Instant,
    cursor: Vec2,
    buttons: MouseEventButtons,
    state: Signal<hud::State>,
    /// Empty until the stylesheet is loaded
    stylesheet: Signal<Stylesheet>,
    stylesheet_handle: Handle<Stylesheet>,
    actions: mpsc::Receiver<hud::Action>,
    /// Whether the document has to be painted again, e.g. after input
    needs_paint: bool,
    show_timings: bool,
    show_inventory: bool,
    /// Inventory tile or hotbar slot the user selected
    selected: Option<hud::Target>,
    /// Inventory tile or hotbar slot under the pointer, for the hotbar's keys
    hovered: Option<hud::Target>,
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

/// The fullscreen image showing the HUD. Picking hits it wherever the HUD blocks the pointer, so
/// it is in the `HoverMap` then.
#[derive(Component)]
pub struct Hud;

/// Class of the HUD elements that take the pointer from the game world, along with their children.
pub const BLOCKS_POINTER: &str = "blocks-pointer";

/// Whether the HUD takes the pointer at `position` (in the document's CSS pixels: Bevy's logical
/// pixels divided by [`Settings::ui_scale`]): whether the topmost element there or one of its ancestors has the class
/// [`BLOCKS_POINTER`]. Uses the last layout, so the document has to be resolved before.
pub fn blocks_pointer(doc: &BaseDocument, position: Vec2) -> bool {
    doc.elements_from_point(position.x, position.y)
        .into_iter()
        .filter_map(|id| doc.get_node(id)?.element_data()?.attr(local_name!("class")))
        .any(|class| {
            class
                .split_ascii_whitespace()
                .any(|name| name == BLOCKS_POINTER)
        })
}

fn setup(world: &mut World) {
    let (action_tx, action_rx) = mpsc::channel();
    let vdom = VirtualDom::new(hud::app).with_root_context(action_tx);
    let state = vdom.in_scope(ScopeId::ROOT, || Signal::new(hud::State::default()));
    vdom.provide_root_context(state);
    let stylesheet = vdom.in_scope(ScopeId::ROOT, || Signal::new(Stylesheet::default()));
    vdom.provide_root_context(stylesheet);

    let redraw_request = Arc::new(RedrawRequest::default());
    let mut doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            shell_provider: Some(redraw_request.clone()),
            // no browser defaults: `assets/ui/style.css` sets everything the HUD needs explicitly
            ua_stylesheets: Vec::new(),
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
        stylesheet,
        stylesheet_handle: world.resource::<AssetServer>().load("ui/style.css"),
        actions: action_rx,
        needs_paint: true,
        show_timings: true,
        show_inventory: false,
        selected: None,
        hovered: None,
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
        Hud,
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
    mut cursor_left: MessageReader<CursorLeft>,
    mut mouse_buttons: MessageReader<MouseButtonInput>,
    mut touches: MessageReader<TouchInput>,
    settings: Res<Settings>,
) {
    let ui = &mut *ui;
    // Blitz's coordinates are CSS pixels, which the UI scale makes larger than logical ones
    let css = |position: Vec2| position / settings.ui_scale;

    // no repaint of its own: Blitz requests a redraw when a move changes something (hover,
    // selection), so moving over the game world doesn't repaint the HUD
    for event in cursor_moved.read() {
        ui.cursor = event.position;
        let event = pointer_event(
            BlitzPointerId::Mouse,
            css(ui.cursor),
            MouseEventButton::Main,
            ui.buttons,
        );
        ui.doc.handle_ui_event(UiEvent::PointerMove(event));
    }

    // ends the hover, otherwise an element left through the window's edge would stay hovered
    if cursor_left.read().count() > 0 {
        let event = pointer_event(
            BlitzPointerId::Mouse,
            css(ui.cursor),
            MouseEventButton::Main,
            ui.buttons,
        );
        ui.doc.handle_ui_event(UiEvent::PointerLeave(event));
    }

    for event in mouse_buttons.read() {
        let (button, flag) = match event.button {
            MouseButton::Left => (MouseEventButton::Main, MouseEventButtons::Primary),
            MouseButton::Right => (MouseEventButton::Secondary, MouseEventButtons::Secondary),
            MouseButton::Middle => (MouseEventButton::Auxiliary, MouseEventButtons::Auxiliary),
            _ => continue,
        };
        // unlike moves, Blitz doesn't request a redraw for everything a click changes (the end
        // of `:active`, a toggled checkbox)
        ui.needs_paint = true;
        let event = match event.state {
            ButtonState::Pressed => {
                ui.buttons |= flag;
                UiEvent::PointerDown(pointer_event(
                    BlitzPointerId::Mouse,
                    css(ui.cursor),
                    button,
                    ui.buttons,
                ))
            }
            ButtonState::Released => {
                ui.buttons -= flag;
                UiEvent::PointerUp(pointer_event(
                    BlitzPointerId::Mouse,
                    css(ui.cursor),
                    button,
                    ui.buttons,
                ))
            }
        };
        ui.doc.handle_ui_event(event);
    }

    // a finger (or a pen the system reports as touch) is a pointer that is only there while it
    // touches, pressing the main button
    for touch in touches.read() {
        let id = BlitzPointerId::Finger(touch.id);
        let event = match touch.phase {
            TouchPhase::Started => UiEvent::PointerDown(pointer_event(
                id,
                css(touch.position),
                MouseEventButton::Main,
                MouseEventButtons::Primary,
            )),
            TouchPhase::Moved => UiEvent::PointerMove(pointer_event(
                id,
                css(touch.position),
                MouseEventButton::Main,
                MouseEventButtons::Primary,
            )),
            TouchPhase::Ended => UiEvent::PointerUp(pointer_event(
                id,
                css(touch.position),
                MouseEventButton::Main,
                MouseEventButtons::None,
            )),
            TouchPhase::Canceled => UiEvent::PointerCancel(pointer_event(
                id,
                css(touch.position),
                MouseEventButton::Main,
                MouseEventButtons::None,
            )),
        };
        // like clicks, touching and lifting change more than Blitz requests redraws for
        if touch.phase != TouchPhase::Moved {
            ui.needs_paint = true;
        }
        ui.doc.handle_ui_event(event);
    }
}

/// Picking backend for the HUD: reports a hit on the [`Hud`] image for each pointer the HUD blocks.
/// Without `Pickable`, the image blocks all hits below it.
fn pick_hud(
    ui: NonSend<UiDocument>,
    settings: Res<Settings>,
    hud: Single<Entity, With<Hud>>,
    camera: Single<(Entity, &Camera), With<Camera2d>>,
    pointers: Query<(&PointerId, &PointerLocation)>,
    mut hits: MessageWriter<PointerHits>,
) {
    let (camera_entity, camera) = *camera;
    let doc = ui.doc.inner();
    for (pointer, location) in &pointers {
        let Some(location) = &location.location else {
            continue;
        };
        if blocks_pointer(&doc, location.position / settings.ui_scale) {
            let hit = HitData::new(camera_entity, 0., Some(location.position.extend(0.)), None);
            // above the sprites, which use the camera's order, since the HUD is drawn above them
            let order = camera.order as f32 + 1.;
            hits.write(PointerHits::new(*pointer, vec![(*hud, hit)], order));
        }
    }
}

fn pointer_event(
    id: BlitzPointerId,
    position: Vec2,
    button: MouseEventButton,
    buttons: MouseEventButtons,
) -> BlitzPointerEvent {
    BlitzPointerEvent {
        id,
        // also each finger, so Blitz ends the hover when it is lifted
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

/// What putting the tile or slot `from` down on `to` (`None`: outside the tiles and slots) does to
/// the hotbar, the same for a drag and for clicking `from` and then `to`: the slot to change and its
/// new type. A usable type put down on a slot goes there (swapping with the slot it was in, see
/// [`AssignSlot`]), a slot put down anywhere but on a slot is emptied, everything else does
/// nothing. `usable_type` gives the type of a tile or slot if it can go into the hotbar.
pub fn put_down(
    from: hud::Target,
    to: Option<hud::Target>,
    usable_type: impl Fn(hud::Target) -> Option<ItemId>,
) -> Option<(usize, Option<ItemId>)> {
    match to {
        _ if to == Some(from) => None,
        Some(hud::Target::Slot(slot)) => Some((slot, Some(usable_type(from)?))),
        _ => match from {
            hud::Target::Slot(slot) => Some((slot, None)),
            hud::Target::Tile(_) => None,
        },
    }
}

/// Keys of the hotbar's slots.
const SLOT_KEYS: [KeyCode; SLOTS] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
];

fn sync_state(
    mut ui: NonSendMut<UiDocument>,
    player: Query<(Entity, &Health, Option<&Inventory>, Option<&Hotbar>), With<Player>>,
    items: Query<(&Item, Option<&Stack>)>,
    definitions: Res<ItemDefinitions>,
    mut keyboard_input: ResMut<ButtonInput<KeyCode>>,
    // one parameter, since a system has at most 16
    (mut drop_item, mut use_item, mut assign_slot, mut use_slot): (
        MessageWriter<DropItem>,
        MessageWriter<UseItem>,
        MessageWriter<AssignSlot>,
        MessageWriter<UseSlot>,
    ),
    enemies: Query<(), With<enemy::Enemy>>,
    flow_field: Res<FlowField>,
    mut flow_field_debug: ResMut<FlowFieldDebug>,
    mut gizmos: ResMut<GizmoConfigStore>,
    mut frozen: ResMut<enemy::Frozen>,
    timings: Option<Res<perf::Timings>>,
    mut spawn_enemy: MessageWriter<SpawnOutOfSight>,
    mut settings: ResMut<Settings>,
    menu: Res<State<Menu>>,
) {
    let ui = &mut *ui;
    let player = player.single().ok();
    // Escape closes the topmost window: the menu above everything (in `settings`), then the
    // selection's popup, then the inventory. Taken away here, so the menu doesn't open as well.
    if *menu.get() == Menu::Closed && keyboard_input.just_pressed(KeyCode::Escape) {
        let closed = if ui.selected.is_some() {
            ui.selected = None;
            true
        } else {
            std::mem::replace(&mut ui.show_inventory, false)
        };
        if closed {
            keyboard_input.clear_just_pressed(KeyCode::Escape);
        }
    }
    if keyboard_input.just_pressed(KeyCode::Tab) {
        ui.show_inventory = !ui.show_inventory;
    }
    let (mut clicks, mut drops) = (Vec::new(), Vec::new());
    for action in ui.actions.try_iter() {
        match action {
            hud::Action::SpawnEnemy => {
                spawn_enemy.write(SpawnOutOfSight(default()));
            }
            hud::Action::CycleFlowField => {
                flow_field_debug.cycle(flow_field.grids().len());
            }
            hud::Action::ShowColliders(show) => {
                gizmos.config_mut::<PhysicsGizmos>().0.enabled = show;
            }
            hud::Action::FreezeEnemies(freeze) => {
                frozen.set_if_neq(enemy::Frozen(freeze));
            }
            hud::Action::ShowTimings(show) => {
                ui.show_timings = show;
            }
            hud::Action::ChangeSettings(new_settings) => {
                // only on changes, since changing the settings saves them
                settings.set_if_neq(new_settings);
            }
            hud::Action::ShowInventory(show) => {
                ui.show_inventory = show;
            }
            hud::Action::DropItem { item, count } => {
                drop_item.write(DropItem { item, count });
            }
            hud::Action::UseItem(item) => {
                use_item.write(UseItem { item });
            }
            hud::Action::Click(target) => {
                clicks.push(target);
            }
            hud::Action::Drop { from, to } => {
                drops.push((from, to));
            }
            hud::Action::Hover { target, over } => {
                if over {
                    ui.hovered = Some(target);
                } else if ui.hovered == Some(target) {
                    ui.hovered = None;
                }
            }
        }
    }
    // the tiles are gone, without telling that the pointer left them
    if !ui.show_inventory {
        ui.selected = None;
        ui.hovered = None;
    }

    let held = player
        .and_then(|(_, _, inventory, _)| inventory)
        .map(Inventory::items)
        .unwrap_or_default();
    // usable items first, then by name, so the tiles keep their places
    let mut rows: Vec<_> = held
        .iter()
        .filter_map(|&item| {
            let (Item(id), stack) = items.get(item).ok()?;
            let definition = definitions.get(id);
            let count = stack.map_or(1, |stack| stack.0);
            Some(hud::InventoryRow {
                item,
                id: id.clone(),
                name: definition.map_or_else(|| id.0.clone(), |d| d.name.clone()),
                color: definition.map_or((1., 0., 1.), |d| d.color),
                count,
                weight: definition.map_or(0., |d| d.weight) * count as f32,
                usable: definition.is_some_and(|d| d.effect.is_some()),
            })
        })
        .collect();
    rows.sort_by(|a, b| (!a.usable, &a.name, a.item).cmp(&(!b.usable, &b.name, b.item)));
    let slots = player
        .and_then(|(.., hotbar)| hotbar)
        .map(|hotbar| hotbar.0.clone())
        .unwrap_or_default();

    if let Some((owner, ..)) = player {
        // the item type of a tile or slot, if it can go into the hotbar
        let usable_type = |target| match target {
            hud::Target::Tile(item) => rows
                .iter()
                .find(|row| row.item == item && row.usable)
                .map(|row| row.id.clone()),
            hud::Target::Slot(slot) => slots.get(slot).cloned().flatten(),
        };
        let has_content = |target| match target {
            hud::Target::Tile(item) => rows.iter().any(|row| row.item == item),
            hud::Target::Slot(slot) => slots.get(slot).is_some_and(Option::is_some),
        };
        let mut put = |from, to| {
            if let Some((slot, id)) = put_down(from, to, usable_type) {
                assign_slot.write(AssignSlot { owner, slot, id });
            }
        };

        // In the game, a click on a slot uses it. With the inventory open, clicks work like a
        // drag: the first one picks up a tile or slot (selecting it), the next one puts it down
        // where it clicks.
        for click in clicks {
            if !ui.show_inventory {
                if let Some(hud::Target::Slot(slot)) = click {
                    use_slot.write(UseSlot { owner, slot });
                }
                continue;
            }
            match ui.selected.take() {
                Some(from) => put(from, click),
                None => ui.selected = click.filter(|&target| has_content(target)),
            }
        }
        for (from, to) in drops {
            put(from, to);
        }

        // In the game, a slot's key uses it. With the inventory open, it puts the tile or slot
        // under the pointer down on it (like in Minecraft), or else the selected one. Items are
        // only used there with a right click.
        for (slot, key) in SLOT_KEYS.into_iter().enumerate() {
            if !keyboard_input.just_pressed(key) {
                continue;
            }
            if !ui.show_inventory {
                use_slot.write(UseSlot { owner, slot });
                continue;
            }
            let to = Some(hud::Target::Slot(slot));
            match ui.hovered.filter(|&hovered| usable_type(hovered).is_some()) {
                Some(from) => put(from, to),
                None => {
                    if let Some(from) = ui.selected.take() {
                        put(from, to);
                    }
                }
            }
        }
    }

    let hotbar = slots.map(|id| {
        id.map(|id| {
            let mut held = rows.iter().filter(|row| row.id == id);
            let first = held.next().cloned();
            let definition = definitions.get(&id);
            hud::HotbarSlot {
                name: definition.map_or_else(|| id.0.clone(), |d| d.name.clone()),
                color: definition.map_or((1., 0., 1.), |d| d.color),
                count: first.iter().chain(held).map(|row| row.count).sum(),
                held: first,
                id,
            }
        })
    });
    let inventory = ui.show_inventory.then_some(rows);

    let new_state = hud::State {
        player_health: player.map(|(_, health, ..)| *health),
        enemy_count: enemies.iter().count(),
        flow_field_grid: flow_field_debug
            .grid
            .and_then(|i| flow_field.grids().get(i))
            .map(|grid| grid.clearance()),
        show_colliders: gizmos.config::<PhysicsGizmos>().0.enabled,
        enemies_frozen: frozen.0,
        show_timings: ui.show_timings,
        // only while shown, since they change twice a second and each change repaints the HUD
        timings: timings
            .filter(|_| ui.show_timings)
            .and_then(|timings| timings.averages.clone()),
        menu_open: *menu.get() == Menu::Open,
        settings: *settings,
        inventory,
        selected: ui.selected,
        hotbar,
    };
    let mut state = ui.state;
    // only on changes, since setting the signal re-renders the HUD
    if *state.peek() != new_state {
        ui.doc.vdom.in_scope(ScopeId::ROOT, || state.set(new_state));
    }
}

/// Hands the stylesheet to the HUD when it is loaded, and again after each change to the file.
fn update_stylesheet(
    mut ui: NonSendMut<UiDocument>,
    mut events: MessageReader<AssetEvent<Stylesheet>>,
    stylesheets: Res<Assets<Stylesheet>>,
) {
    let ui = &mut *ui;
    let handle = &ui.stylesheet_handle;
    if !events
        .read()
        .any(|event| event.is_loaded_with_dependencies(handle) || event.is_modified(handle))
    {
        return;
    }
    if let Some(new_stylesheet) = stylesheets.get(handle) {
        let mut stylesheet = ui.stylesheet;
        // re-renders the HUD, which hands the new styles to Blitz
        ui.doc
            .vdom
            .in_scope(ScopeId::ROOT, || stylesheet.set(new_stylesheet.clone()));
    }
}

fn paint_ui(
    mut ui: NonSendMut<UiDocument>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut ui_scene: ResMut<UiScene>,
    mut images: ResMut<Assets<Image>>,
    settings: Res<Settings>,
) {
    let ui = &mut *ui;

    // true after a re-render, e.g. when `sync_state` changed the state
    ui.needs_paint |= ui.doc.poll(None);
    ui.needs_paint |= ui.redraw_request.0.swap(false, Ordering::Relaxed);

    let (width, height) = (window.physical_width(), window.physical_height());
    // the HUD is drawn larger than the window's scale by the UI scale
    let scale = window.scale_factor() * settings.ui_scale;
    let mut doc = ui.doc.inner_mut();

    let viewport = doc.viewport();
    if viewport.window_size != (width, height) || viewport.hidpi_scale != scale {
        ui.needs_paint = true;
        resize(
            &mut doc,
            Viewport::new(width, height, scale, ColorScheme::Dark),
        );
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

/// Sets the document's viewport after the window changed its size or scale, so that the next
/// `resolve` lays out and hit-tests everything for the new size.
pub fn resize(doc: &mut BaseDocument, viewport: Viewport) {
    doc.set_viewport(viewport);
    let ids: Vec<_> = doc.tree().iter().map(|(id, _)| id).collect();
    for id in ids {
        if let Some(node) = doc.get_node_mut(id) {
            // Blitz aligns a node's inline layout in every measure pass. After a resize, taffy
            // serves the final layout of unchanged nodes from its cache, so centered text (e.g. in
            // buttons) keeps the alignment of a measure pass with a larger width and lands outside
            // its box. A full relayout avoids that.
            node.invalidate_layout_cache();
            // Hits only reach a node within the area its subtree covers, which Blitz only
            // recomputes for damaged nodes and not on a resize. After the window grew, the HUD
            // (positioned at the right edge) would lie outside the old area and miss all hits.
            node.insert_damage(RestyleDamage::RECALCULATE_OVERFLOW);
        }
    }
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
            .init_asset::<Stylesheet>()
            .init_asset_loader::<StylesheetLoader>()
            .add_systems(Startup, setup)
            .add_systems(
                PreUpdate,
                (forward_input, pick_hud.in_set(PickingSystems::Backend)),
            )
            .add_systems(
                Update,
                (
                    update_stylesheet,
                    // before the menu, so Escape closes the HUD's windows first
                    sync_state.before(settings::toggle_menu),
                ),
            )
            .add_systems(PostUpdate, paint_ui);

        app.sub_app_mut(RenderApp)
            .add_systems(RenderStartup, init_vello_renderer)
            .add_systems(Render, render_ui.in_set(RenderSystems::Prepare));
    }
}
