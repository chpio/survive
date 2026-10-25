use super::stylesheet::Stylesheet;
use crate::{
    combat::Health,
    perf::{Averages, MAIN_THREAD, Node, RENDER_THREAD},
    settings::Settings,
};
use bevy::ecs::entity::Entity;
use dioxus::prelude::*;
use std::{sync::mpsc, time::Duration};

/// Game state shown in the HUD, written by the game whenever it changes.
#[derive(Clone, PartialEq, Default)]
pub struct State {
    /// `None` while there is no player
    pub player_health: Option<Health>,
    pub enemy_count: usize,
    /// Clearance of the flow field grid shown in the debug overlay, `None` while it is hidden
    pub flow_field_grid: Option<f32>,
    /// Whether the physics debug outlines of the colliders are drawn
    pub show_colliders: bool,
    pub show_timings: bool,
    /// Time per frame of the game's steps, `None` while hidden or not measured yet
    pub timings: Option<Averages>,
    pub menu_open: bool,
    pub settings: Settings,
    /// The player's items, `None` while the inventory is closed
    pub inventory: Option<Vec<InventoryRow>>,
}

/// An item or a stack in the inventory.
#[derive(Clone, PartialEq)]
pub struct InventoryRow {
    pub item: Entity,
    pub name: String,
    pub count: u32,
    /// Weight of all items in the row in kg
    pub weight: f32,
}

/// Requests from the HUD to the game.
pub enum Action {
    SpawnEnemy,
    /// Shows the next flow field grid, or hides the overlay after the last one
    CycleFlowField,
    ShowColliders(bool),
    ShowTimings(bool),
    /// Replaces the settings
    ChangeSettings(Settings),
    ShowInventory(bool),
    /// Puts down `count` of the item (the whole stack if it has fewer)
    DropItem {
        item: Entity,
        count: u32,
    },
}

pub fn app() -> Element {
    let state = use_context::<Signal<State>>().read().clone();
    let stylesheet = use_context::<Signal<Stylesheet>>().read().0.clone();
    let actions = use_context::<mpsc::Sender<Action>>();
    // `Copy`, unlike the sender, so every event handler can use it
    let send = use_callback(move |action| {
        let _ = actions.send(action);
    });
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

    rsx! {
        style { {stylesheet} }
        div { class: "hud blocks-pointer",
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
                        for node in MAIN_THREAD {
                            {node_rows(&timings, node)}
                        }
                        {row("Sonstiges", timings.rest())}
                    }
                    {row("Render-Thread (parallel)", RENDER_THREAD.iter().map(|&step| timings.step(step)).sum())}
                    div { class: "children",
                        for & step in RENDER_THREAD {
                            {row(step.label(), timings.step(step))}
                        }
                    }
                }
            }
        }
        if let Some(rows) = state.inventory {
            {inventory(rows, send)}
        }
        if state.menu_open {
            {menu(state.settings, send)}
        }
    }
}

fn inventory(rows: Vec<InventoryRow>, send: Callback<Action>) -> Element {
    let weight: f32 = rows.iter().map(|row| row.weight).sum();

    rsx! {
        div { class: "inventory blocks-pointer",
            div { class: "header",
                span { class: "title", "Inventar" }
                button { onclick: move |_| send(Action::ShowInventory(false)), "×" }
            }
            if rows.is_empty() {
                span { class: "empty", "Leer" }
            }
            for row in rows {
                div { class: "item", key: "{row.item}",
                    span { class: "name", "{row.name}" }
                    span { class: "count", "{row.count}×" }
                    span { class: "weight", "{row.weight:.1} kg" }
                    if row.count > 1 {
                        button { onclick: move |_| send(Action::DropItem { item: row.item, count: 1 }), "1 ablegen" }
                    }
                    button { onclick: move |_| send(Action::DropItem { item: row.item, count: row.count }), "Ablegen" }
                }
            }
            span { class: "total", "Gesamt: {weight:.1} kg" }
        }
    }
}

fn menu(settings: Settings, send: Callback<Action>) -> Element {
    let zoom = format!("{:.0} %", settings.zoom * 100.);
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
            }
        }
    }
}

/// A step, or a group with its steps below it.
fn node_rows(timings: &Averages, node: &Node) -> Element {
    match node {
        Node::Step(step) => row(step.label(), timings.step(*step)),
        Node::Group(label, steps) => rsx! {
            {row(label, timings.node(node))}
            div { class: "children",
                for & step in *steps {
                    {row(step.label(), timings.step(step))}
                }
            }
        },
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
