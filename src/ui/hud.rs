use dioxus::prelude::*;
use std::sync::mpsc;

/// Game state shown in the HUD, written by the game every frame.
#[derive(Clone, Copy)]
pub struct State {
    pub enemy_count: Signal<usize>,
}

/// Requests from the HUD to the game.
pub enum Action {
    SpawnEnemy,
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
"#;

pub fn app() -> Element {
    let state = use_context::<State>();
    let actions = use_context::<mpsc::Sender<Action>>();

    rsx! {
        style { {STYLE} }
        div { class: "hud",
            span { "Gegner: {state.enemy_count}" }
            button {
                onclick: move |_| {
                    let _ = actions.send(Action::SpawnEnemy);
                },
                "Gegner spawnen"
            }
        }
    }
}
