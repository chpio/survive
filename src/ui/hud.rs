use dioxus::prelude::*;
use std::sync::mpsc;

/// Game state shown in the HUD, written by the game every frame.
#[derive(Clone, Copy)]
pub struct State {
    pub enemy_count: Signal<usize>,
    /// Clearance of the flow field grid shown in the debug overlay, `None` while it is hidden
    pub flow_field_grid: Signal<Option<f32>>,
    /// Whether the physics debug outlines of the colliders are drawn
    pub show_colliders: Signal<bool>,
}

/// Requests from the HUD to the game.
pub enum Action {
    SpawnEnemy,
    /// Shows the next flow field grid, or hides the overlay after the last one
    CycleFlowField,
    ShowColliders(bool),
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

/* Blitz paints checkboxes in the text color, which is white in the HUD */
.hud input {
    color: #3a6ea5;
}
"#;

pub fn app() -> Element {
    let state = use_context::<State>();
    let actions = use_context::<mpsc::Sender<Action>>();
    // each event handler owns a sender
    let (spawn_actions, flow_field_actions, collider_actions) =
        (actions.clone(), actions.clone(), actions);
    let flow_field = match *state.flow_field_grid.read() {
        Some(clearance) => format!("{clearance} px"),
        None => "aus".to_string(),
    };

    rsx! {
        style { {STYLE} }
        div { class: "hud",
            span { "Gegner: {state.enemy_count}" }
            button {
                onclick: move |_| {
                    let _ = spawn_actions.send(Action::SpawnEnemy);
                },
                "Gegner spawnen"
            }
            button {
                onclick: move |_| {
                    let _ = flow_field_actions.send(Action::CycleFlowField);
                },
                "Flowfield: {flow_field}"
            }
            label {
                input {
                    r#type: "checkbox",
                    checked: *state.show_colliders.read(),
                    oninput: move |event| {
                        let _ = collider_actions.send(Action::ShowColliders(event.checked()));
                    },
                }
                "Kollider"
            }
        }
    }
}
