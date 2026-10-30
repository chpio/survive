use super::stylesheet::Stylesheet;
use crate::{
    combat::Health,
    items::{ItemId, hotbar::SLOTS},
    light::Phase,
    perf::Averages,
    settings::Settings,
};
use bevy::ecs::entity::Entity;
use dioxus::{html::input_data::MouseButton, prelude::*};
use std::{sync::mpsc, time::Duration};

/// Game state shown in the HUD, written by the game whenever it changes.
#[derive(Clone, PartialEq, Default)]
pub struct State {
    /// `None` until the day-night cycle's values are loaded
    pub clock: Option<Clock>,
    /// `None` while there is no player
    pub player_health: Option<Health>,
    pub enemy_count: usize,
    /// Clearance of the flow field grid shown in the debug overlay, `None` while it is hidden
    pub flow_field_grid: Option<f32>,
    /// Whether the physics debug outlines of the colliders are drawn
    pub show_colliders: bool,
    pub enemies_frozen: bool,
    pub show_timings: bool,
    /// Time per frame of the game's steps, `None` while hidden or not measured yet
    pub timings: Option<Averages>,
    pub menu_open: bool,
    pub settings: Settings,
    /// The player's items in the order they are shown, `None` while the inventory is closed
    pub inventory: Option<Vec<InventoryRow>>,
    /// The selected inventory tile or hotbar slot, whose details are shown
    pub selected: Option<Target>,
    /// The player's hotbar, always shown
    pub hotbar: [Option<HotbarSlot>; SLOTS],
}

/// The time of day, to the minute.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Clock {
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub phase: Phase,
    /// Whether the clock stands still
    pub paused: bool,
}

/// An item or a stack in the inventory.
#[derive(Clone, PartialEq)]
pub struct InventoryRow {
    pub item: Entity,
    pub id: ItemId,
    pub name: String,
    /// sRGB color of the placeholder icon
    pub color: (f32, f32, f32),
    pub count: u32,
    /// Weight of all items in the row in kg
    pub weight: f32,
    /// Whether the item has an effect when used
    pub usable: bool,
}

/// The item type in a slot of the hotbar.
#[derive(Clone, PartialEq)]
pub struct HotbarSlot {
    pub id: ItemId,
    pub name: String,
    /// sRGB color of the placeholder icon
    pub color: (f32, f32, f32),
    /// Items of the type the player holds
    pub count: u32,
    /// The first held stack of the type, which a right click and the details' buttons act on,
    /// `None` while the player holds none
    pub held: Option<InventoryRow>,
}

/// An inventory tile or a hotbar slot. With the inventory open, both are used the same way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// The tile of an item or stack
    Tile(Entity),
    Slot(usize),
}

/// Requests from the HUD to the game.
pub enum Action {
    /// Moves the clock on by an hour
    SkipHour,
    /// Moves the clock on to the start of the next phase
    SkipPhase,
    PauseClock(bool),
    SpawnEnemy,
    /// Shows the next flow field grid, or hides the overlay after the last one
    CycleFlowField,
    ShowColliders(bool),
    FreezeEnemies(bool),
    ShowTimings(bool),
    /// Replaces the settings
    ChangeSettings(Settings),
    ShowInventory(bool),
    /// Puts down `count` of the item (the whole stack if it has fewer)
    DropItem {
        item: Entity,
        count: u32,
    },
    /// Uses one of the item
    UseItem(Entity),
    /// A click on a tile or slot, `None` elsewhere (only sent while one is selected). The game
    /// decides what it does: select it, put the selected one down there (see [`super::put_down`]),
    /// or use a slot while the inventory is closed.
    Click(Option<Target>),
    /// A tile or slot dragged onto another one, `None` elsewhere. Does the same as clicking `from`
    /// and then `to`.
    Drop {
        from: Target,
        to: Option<Target>,
    },
    /// The pointer entered (`over`) or left a tile or slot
    Hover {
        target: Target,
        over: bool,
    },
}

/// Distance in CSS px the pointer has to move after pressing before it drags instead of clicking.
const DRAG_THRESHOLD: f64 = 4.;

/// A press of the pointer on an inventory tile or a hotbar slot, until it is released. Blitz has
/// no drag and drop events and sends a click to wherever the pointer is released, so the HUD tells
/// clicks and drags apart itself: released on the element it was pressed on without moving beyond
/// [`DRAG_THRESHOLD`], it is a click, otherwise it drags the item type.
#[derive(Clone, PartialEq)]
struct Press {
    on: Target,
    /// What it drags, shown like in its tile or slot, `None` if it can't be dragged (an empty slot,
    /// or an item that can't go into the hotbar)
    dragged: Option<Cell>,
    /// Pointer position in CSS px where it was pressed
    start: (f64, f64),
    /// Current pointer position once it moved beyond [`DRAG_THRESHOLD`], which makes it a drag
    position: Option<(f64, f64)>,
}

impl Press {
    fn new(on: Target, dragged: Option<Cell>, event: &Event<PointerData>) -> Self {
        Self {
            on,
            dragged,
            start: pointer_position(event),
            position: None,
        }
    }

    fn is_click_on(&self, target: Target) -> bool {
        self.on == target && self.position.is_none()
    }
}

/// The pointer's state in the HUD, shared by the inventory and the hotbar.
#[derive(Clone, Copy)]
struct Pointer {
    press: Signal<Option<Press>>,
    /// Item type of the tile or slot under the pointer, highlighted in both
    hovered: Signal<Option<ItemId>>,
}

impl Pointer {
    fn enter(mut self, id: &ItemId) {
        if self.hovered.peek().as_ref() != Some(id) {
            self.hovered.set(Some(id.clone()));
        }
    }

    fn leave(mut self, id: &ItemId) {
        if self.hovered.peek().as_ref() == Some(id) {
            self.hovered.set(None);
        }
    }

    fn highlights(self, id: &ItemId) -> bool {
        self.hovered.read().as_ref() == Some(id)
    }
}

fn pointer_position(event: &Event<PointerData>) -> (f64, f64) {
    let point = event.client_coordinates();
    (point.x, point.y)
}

fn is_right_click(event: &Event<PointerData>) -> bool {
    event.trigger_button() == Some(MouseButton::Secondary)
}

/// How an item looks in the HUD, the same in the inventory, the hotbar and while dragged: so far a
/// square in the placeholder color of its type, with the count for stacks.
fn item_icon(cell: &Cell) -> Element {
    let (r, g, b) = cell.color;
    let color = format!("rgb({:.0}, {:.0}, {:.0})", r * 255., g * 255., b * 255.);
    rsx! {
        div { class: "icon", style: "background: {color}" }
        if cell.count > 1 {
            span { class: "count", "{cell.count}" }
        }
    }
}

pub fn app() -> Element {
    let state = use_context::<Signal<State>>().read().clone();
    let stylesheet = use_context::<Signal<Stylesheet>>().read().0.clone();
    let actions = use_context::<mpsc::Sender<Action>>();
    // `Copy`, unlike the sender, so every event handler can use it
    let send = use_callback(move |action| {
        let _ = actions.send(action);
    });
    let mut press = use_signal(|| None::<Press>);
    let pointer = Pointer {
        press,
        hovered: use_signal(|| None),
    };
    // after the hooks, which have to run on every render. Without styles, the HUD would show up
    // unstyled until the stylesheet is loaded.
    if stylesheet.is_empty() {
        return rsx! {};
    }
    let health = match state.player_health {
        Some(health) => format!("{:.0} / {:.0}", health.current, health.max),
        None => "–".to_string(),
    };
    let flow_field = match state.flow_field_grid {
        Some(clearance) => format!("{clearance} px"),
        None => "aus".to_string(),
    };
    // the item type being moved, by clicking it (selected) or dragging it, which both work alike,
    // and whether it is usable, so putting it down on a slot puts it there
    let dragged = press().filter(|pressed| pressed.dragged.is_some() && pressed.position.is_some());
    let moving = match dragged.map(|pressed| pressed.on).or(state.selected) {
        Some(Target::Tile(item)) => state
            .inventory
            .iter()
            .flatten()
            .find(|row| row.item == item)
            .map(|row| (row.id.clone(), row.usable)),
        Some(Target::Slot(slot)) => state
            .hotbar
            .get(slot)
            .cloned()
            .flatten()
            .map(|slot| (slot.id, true)),
        None => None,
    };
    let assignable = moving.as_ref().is_some_and(|&(_, usable)| usable);
    let marks = Marks {
        moving: moving.map(|(id, _)| id),
        popup: state.selected,
    };

    rsx! {
        style { {stylesheet} }
        // covers the whole window without blocking the world's picking, so it gets the pointer's
        // moves and releases anywhere, also those bubbling up from its children
        div { class: "root",
            onpointermove: move |event| {
                let position = pointer_position(&event);
                let moved = match &*press.peek() {
                    Some(pressed) if pressed.dragged.is_some() => {
                        pressed.position.is_some()
                            || (position.0 - pressed.start.0).hypot(position.1 - pressed.start.1)
                                > DRAG_THRESHOLD
                    }
                    _ => return,
                };
                if moved && let Some(pressed) = press.write().as_mut() {
                    pressed.position = Some(position);
                }
            },
            // released anywhere but on a tile or slot (which stop the event)
            onpointerup: move |_| {
                let pressed = press.peek().clone();
                match pressed {
                    Some(pressed) => {
                        press.set(None);
                        if pressed.position.is_some() {
                            send(Action::Drop { from: pressed.on, to: None });
                        }
                    }
                    // a click elsewhere puts the selected tile or slot down there
                    None if state.selected.is_some() => send(Action::Click(None)),
                    None => {}
                }
            },
            div { class: "hud blocks-pointer",
                if let Some(clock) = state.clock {
                    span { {format!("Tag {}, {:02}:{:02} · {}", clock.day, clock.hour, clock.minute, clock.phase.name())} }
                    div { class: "buttons",
                        button { onclick: move |_| send(Action::SkipHour), "+1 h (N)" }
                        button { onclick: move |_| send(Action::SkipPhase), "Nächste Phase" }
                    }
                    label {
                        input {
                            r#type: "checkbox",
                            checked: clock.paused,
                            oninput: move |event| send(Action::PauseClock(event.checked())),
                        }
                        "Uhr anhalten"
                    }
                }
                span { "Leben: {health}" }
                span { "Gegner: {state.enemy_count}" }
                button { onclick: move |_| send(Action::SpawnEnemy), "Gegner spawnen" }
                button { onclick: move |_| send(Action::CycleFlowField), "Flowfield: {flow_field}" }
                label {
                    input {
                        r#type: "checkbox",
                        checked: state.show_colliders,
                        oninput: move |event| send(Action::ShowColliders(event.checked())),
                    }
                    "Kollider"
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: state.enemies_frozen,
                        oninput: move |event| send(Action::FreezeEnemies(event.checked())),
                    }
                    "Gegner einfrieren (G)"
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: state.show_timings,
                        oninput: move |event| send(Action::ShowTimings(event.checked())),
                    }
                    "Performance"
                }
                if let Some(timings) = state.timings {
                    div { class: "timings",
                        div { class: "row",
                            span { "Frame" }
                            span { {format!("{:.0} FPS / {}", 1. / timings.frame.as_secs_f64(), ms(timings.frame))} }
                        }
                        div { class: "children",
                            {row("Physik", timings.physics)}
                        }
                    }
                }
            }
            // the inventory right above the hotbar
            div { class: "bottom",
                if let Some(rows) = state.inventory {
                    {inventory(rows, &marks, pointer, send)}
                }
                {hotbar(state.hotbar, assignable, &marks, pointer, send)}
            }
            if state.menu_open {
                {menu(state.settings, send)}
            }
            // next to the pointer instead of below it, so it doesn't hide the slot it is released on
            if let Some(Press { dragged: Some(cell), position: Some((x, y)), .. }) = press() {
                div { class: "drag", style: "left: {x + 8.}px; top: {y + 8.}px",
                    {item_icon(&cell)}
                }
            }
        }
    }
}

/// The hotbar along the bottom edge. Dragging an item type onto a slot puts it there, dragging a
/// slot out of the hotbar empties it.
fn hotbar(
    slots: [Option<HotbarSlot>; SLOTS],
    assignable: bool,
    marks: &Marks,
    pointer: Pointer,
    send: Callback<Action>,
) -> Element {
    rsx! {
        div { class: if assignable { "hotbar blocks-pointer assignable" } else { "hotbar blocks-pointer" },
            for (slot, content) in slots.into_iter().enumerate() {
                {cell(Target::Slot(slot), content.map(Cell::from), marks, pointer, send)}
            }
        }
    }
}

/// The inventory window: a tile per item or stack, sorted by the game. Usable tiles can be dragged
/// onto the hotbar, or put there with the hotbar's keys while the pointer is over them.
fn inventory(
    rows: Vec<InventoryRow>,
    marks: &Marks,
    pointer: Pointer,
    send: Callback<Action>,
) -> Element {
    let weight: f32 = rows.iter().map(|row| row.weight).sum();

    rsx! {
        div { class: "inventory blocks-pointer",
            div { class: "header",
                span { class: "title", "Inventar" }
                button { onclick: move |_| send(Action::ShowInventory(false)), "×" }
            }
            if rows.is_empty() {
                span { class: "empty", "Leer" }
            } else {
                div { class: "grid",
                    for row in rows {
                        {cell(Target::Tile(row.item), Some(Cell::from(row)), marks, pointer, send)}
                    }
                }
            }
            span { class: "total", "Gesamt: {weight:.1} kg" }
        }
    }
}

/// What the tiles and slots mark: the item type being moved, wherever it is, and the selected tile
/// or slot showing its popup (not a dragged one, where it would be in the way).
struct Marks {
    moving: Option<ItemId>,
    popup: Option<Target>,
}

/// What a tile or slot shows: a stack in the inventory, or all items of a type in a slot.
#[derive(Clone, PartialEq)]
struct Cell {
    id: ItemId,
    name: String,
    color: (f32, f32, f32),
    count: u32,
    /// The stack a right click and the details' buttons act on, `None` while none is held
    held: Option<InventoryRow>,
    /// Whether it can be dragged onto the hotbar
    usable: bool,
}

impl From<InventoryRow> for Cell {
    fn from(row: InventoryRow) -> Self {
        Self {
            id: row.id.clone(),
            name: row.name.clone(),
            color: row.color,
            count: row.count,
            usable: row.usable,
            held: Some(row),
        }
    }
}

impl From<HotbarSlot> for Cell {
    fn from(slot: HotbarSlot) -> Self {
        Self {
            id: slot.id,
            name: slot.name,
            color: slot.color,
            count: slot.count,
            held: slot.held,
            // only usable types are in the hotbar
            usable: true,
        }
    }
}

/// A tile or slot, `content` `None` for an empty slot. A click goes to the game (which selects it
/// with the inventory open, showing the details in a popup above it), a right click uses it, and
/// usable items can be dragged onto slots.
fn cell(
    target: Target,
    content: Option<Cell>,
    marks: &Marks,
    pointer: Pointer,
    send: Callback<Action>,
) -> Element {
    let mut press = pointer.press;
    let is_selected = marks.popup == Some(target);
    let (mut class, key) = match target {
        Target::Tile(item) => (String::from("tile"), format!("tile-{item}")),
        Target::Slot(slot) => (String::from("slot"), format!("slot-{slot}")),
    };
    if let Some(content) = &content {
        if content.count == 0 {
            class += " unavailable";
        }
        if marks.moving.as_ref() == Some(&content.id) {
            class += " selected";
        }
        if pointer.highlights(&content.id) {
            class += " highlighted";
        }
    }
    let dragged = content.clone().filter(|content| content.usable);
    let used = content
        .as_ref()
        .and_then(|content| content.held.as_ref())
        .map(|held| held.item);
    let entered = content.as_ref().map(|content| content.id.clone());
    let left = entered.clone();
    let details = content.clone().filter(|_| is_selected);

    rsx! {
        div { class, key: "{key}",
            onpointerdown: move |event| {
                if !is_right_click(&event) {
                    press.set(Some(Press::new(target, dragged.clone(), &event)));
                } else if let Some(item) = used {
                    send(Action::UseItem(item));
                }
            },
            onpointerup: move |event| {
                let Some(pressed) = press.peek().clone() else {
                    return;
                };
                event.stop_propagation();
                press.set(None);
                if pressed.is_click_on(target) {
                    send(Action::Click(Some(target)));
                } else if pressed.position.is_some() {
                    send(Action::Drop { from: pressed.on, to: Some(target) });
                }
            },
            onpointerenter: move |_| {
                if let Some(id) = &entered {
                    pointer.enter(id);
                }
                send(Action::Hover { target, over: true });
            },
            onpointerleave: move |_| {
                if let Some(id) = &left {
                    pointer.leave(id);
                }
                send(Action::Hover { target, over: false });
            },
            if let Target::Slot(slot) = target {
                span { class: "key", "{slot + 1}" }
            }
            if let Some(content) = &content {
                {item_icon(content)}
            }
            if let Some(details) = details {
                {item_details(details, send)}
            }
        }
    }
}

/// Name, count and weight of the selected item, with what can be done with it. A popup above its
/// tile or slot, so the inventory window keeps its size. Its pointer events stay with it, so they
/// don't count as presses on the tile or slot.
fn item_details(cell: Cell, send: Callback<Action>) -> Element {
    rsx! {
        div { class: "details",
            onpointerdown: move |event| event.stop_propagation(),
            onpointerup: move |event| event.stop_propagation(),
            span { class: "name", "{cell.name}" }
            if let Some(held) = cell.held {
                span { class: "info", "{held.count}× · {held.weight:.1} kg" }
                div { class: "buttons",
                    if held.usable {
                        button { onclick: move |_| send(Action::UseItem(held.item)), "Benutzen" }
                    }
                    if held.count > 1 {
                        button { onclick: move |_| send(Action::DropItem { item: held.item, count: 1 }), "1 ablegen" }
                    }
                    button { onclick: move |_| send(Action::DropItem { item: held.item, count: held.count }), "Ablegen" }
                }
            } else {
                span { class: "info", "Keine im Inventar" }
            }
        }
    }
}

fn menu(settings: Settings, send: Callback<Action>) -> Element {
    let zoom = format!("{:.0} %", settings.zoom * 100.);
    let ui_scale = format!("{:.0} %", settings.ui_scale * 100.);
    let change = move |settings| send(Action::ChangeSettings(settings));

    rsx! {
        div { class: "menu blocks-pointer",
            div { class: "panel",
                span { class: "title", "Einstellungen" }
                label {
                    input {
                        r#type: "checkbox",
                        checked: settings.vsync,
                        oninput: move |event| change(Settings { vsync: event.checked(), ..settings }),
                    }
                    "VSync"
                }
                label {
                    input {
                        r#type: "checkbox",
                        checked: settings.fullscreen,
                        oninput: move |event| change(Settings { fullscreen: event.checked(), ..settings }),
                    }
                    "Vollbild"
                }
                div { class: "setting",
                    span { "Zoom" }
                    div { class: "stepper",
                        button { onclick: move |_| change(Settings { zoom: settings.next_zoom(false), ..settings }), "−" }
                        span { "{zoom}" }
                        button { onclick: move |_| change(Settings { zoom: settings.next_zoom(true), ..settings }), "+" }
                    }
                }
                div { class: "setting",
                    span { "UI-Skalierung" }
                    div { class: "stepper",
                        button { onclick: move |_| change(Settings { ui_scale: settings.next_ui_scale(false), ..settings }), "−" }
                        span { "{ui_scale}" }
                        button { onclick: move |_| change(Settings { ui_scale: settings.next_ui_scale(true), ..settings }), "+" }
                    }
                }
            }
        }
    }
}

fn row(label: &str, duration: Duration) -> Element {
    rsx! {
        div { class: "row", key: "{label}",
            span { "{label}" }
            span { {ms(duration)} }
        }
    }
}

fn ms(duration: Duration) -> String {
    format!("{:.2} ms", duration.as_secs_f64() * 1000.)
}
