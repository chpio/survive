use crate::{
    combat::Health,
    perf::{Averages, MAIN_THREAD, Node, RENDER_THREAD},
};
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
}

/// Requests from the HUD to the game.
pub enum Action {
    SpawnEnemy,
    /// Shows the next flow field grid, or hides the overlay after the last one
    CycleFlowField,
    ShowColliders(bool),
    ShowTimings(bool),
}

const STYLE: &str = r#"
.hud {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 12px;
    border-radius: 6px;
    background: rgba(20, 20, 20, 0.75);
    color: white;
    font-family: sans-serif;
    font-size: 14px;
}

.hud button {
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    background: #3a6ea5;
    color: white;
}

.hud button:hover {
    background: #4a84c4;
}

.hud label {
    display: flex;
    align-items: center;
    gap: 6px;
}

.timings {
    display: flex;
    flex-direction: column;
    font-size: 12px;
}

.timings .row {
    display: flex;
    justify-content: space-between;
    gap: 16px;
}

.timings .children {
    margin-left: 3px;
    padding-left: 8px;
    border-left: 1px solid #666;
}

/* Blitz paints checkboxes in the text color, which is white in the HUD */
.hud input {
    color: #3a6ea5;
}
"#;

pub fn app() -> Element {
    let state = use_context::<Signal<State>>().read().clone();
    let actions = use_context::<mpsc::Sender<Action>>();
    // `Copy`, unlike the sender, so every event handler can use it
    let send = use_callback(move |action| {
        let _ = actions.send(action);
    });
    let health = match state.player_health {
        Some(health) => format!("{:.0} / {:.0}", health.current, health.max),
        None => "–".to_string(),
    };
    let flow_field = match state.flow_field_grid {
        Some(clearance) => format!("{clearance} px"),
        None => "aus".to_string(),
    };

    rsx! {
        style { {STYLE} }
        div { class: "hud",
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
                    {row("Frame", timings.frame)}
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
