//! The HUD's inventory and hotbar driven by pointer events, as the game forwards them to Blitz.

use bevy::{ecs::world::World, math::Vec2, prelude::Entity};
use blitz_dom::{Document as _, DocumentConfig};
use blitz_traits::{
    events::{
        BlitzPointerEvent, BlitzPointerId, MouseEventButton, MouseEventButtons, Point,
        PointerCoords, PointerDetails, UiEvent,
    },
    shell::{ColorScheme, Viewport},
};
use dioxus::prelude::*;
use dioxus_native_dom::DioxusDocument;
use std::sync::mpsc;
use survival::{
    items::{Effect, ItemId},
    ui::{
        hud::{self, Action, HotbarSlot, InventoryRow, Target},
        put_down,
        stylesheet::Stylesheet,
    },
};

struct Hud {
    doc: DioxusDocument,
    actions: mpsc::Receiver<Action>,
}

/// The HUD with the real stylesheet in a window of 800 × 600 logical pixels, showing `state`.
fn hud(state: hud::State) -> Hud {
    let (action_tx, actions) = mpsc::channel();
    let vdom = VirtualDom::new(hud::app).with_root_context(action_tx);
    let state = vdom.in_scope(ScopeId::ROOT, || Signal::new(state));
    vdom.provide_root_context(state);
    let css = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/ui/style.css"))
        .unwrap();
    let stylesheet = vdom.in_scope(ScopeId::ROOT, || Signal::new(Stylesheet(css)));
    vdom.provide_root_context(stylesheet);

    let mut doc = DioxusDocument::new(
        vdom,
        DocumentConfig {
            ua_stylesheets: Vec::new(),
            ..Default::default()
        },
    );
    doc.initial_build();
    doc.inner_mut()
        .set_viewport(Viewport::new(800, 600, 1., ColorScheme::Dark));
    let mut hud = Hud { doc, actions };
    hud.update();
    hud
}

impl Hud {
    /// Renders and lays out what changed.
    fn update(&mut self) {
        self.doc.poll(None);
        self.doc.inner_mut().resolve(0.);
    }

    /// Center of the `index`th element matching `selector`.
    fn center(&self, selector: &str, index: usize) -> Vec2 {
        let doc = self.doc.inner();
        let node = doc.query_selector_all(selector).unwrap()[index];
        let rect = doc.get_client_bounding_rect(node).unwrap();
        Vec2::new(
            (rect.x + rect.width / 2.) as f32,
            (rect.y + rect.height / 2.) as f32,
        )
    }

    fn pointer(&mut self, event: fn(BlitzPointerEvent) -> UiEvent, at: Vec2, right: bool) {
        let (button, buttons) = if right {
            (MouseEventButton::Secondary, MouseEventButtons::Secondary)
        } else {
            (MouseEventButton::Main, MouseEventButtons::Primary)
        };
        self.doc.handle_ui_event(event(BlitzPointerEvent {
            id: BlitzPointerId::Mouse,
            is_primary: true,
            coords: PointerCoords {
                page_x: at.x,
                page_y: at.y,
                client_x: at.x,
                client_y: at.y,
                screen_x: at.x,
                screen_y: at.y,
            },
            button,
            buttons,
            mods: Modifiers::empty(),
            details: PointerDetails::default(),
            element: Point::default(),
            active_pointers: Default::default(),
        }));
        self.update();
    }

    fn click(&mut self, at: Vec2, right: bool) {
        self.pointer(UiEvent::PointerMove, at, right);
        self.pointer(UiEvent::PointerDown, at, right);
        self.pointer(UiEvent::PointerUp, at, right);
    }

    fn drag(&mut self, from: Vec2, to: Vec2) {
        self.pointer(UiEvent::PointerMove, from, false);
        self.pointer(UiEvent::PointerDown, from, false);
        self.pointer(UiEvent::PointerMove, from.lerp(to, 0.5), false);
        self.pointer(UiEvent::PointerMove, to, false);
        self.pointer(UiEvent::PointerUp, to, false);
    }

    fn actions(&self) -> Vec<Action> {
        self.actions.try_iter().collect()
    }
}

/// An open inventory with a usable potion and a sword, and the potion in the first hotbar slot.
fn state() -> (hud::State, Entity, Entity) {
    let mut world = World::new();
    let (potion, sword) = (world.spawn_empty().id(), world.spawn_empty().id());
    let row = |item, id: &str, usable| InventoryRow {
        item,
        id: id.into(),
        name: id.to_string(),
        color: (1., 1., 1.),
        count: 1,
        weight: 1.,
        usable,
        effect: usable.then_some(Effect::Heal(30.)),
    };
    let potion_row = row(potion, "potion", true);
    let mut hotbar: [Option<HotbarSlot>; 5] = Default::default();
    hotbar[0] = Some(HotbarSlot {
        id: "potion".into(),
        name: "potion".into(),
        color: (1., 1., 1.),
        count: 1,
        held: Some(potion_row.clone()),
    });
    let state = hud::State {
        inventory: Some(vec![potion_row, row(sword, "sword", false)]),
        hotbar,
        ..Default::default()
    };
    (state, potion, sword)
}

#[test]
fn click_on_a_tile_selects_it_and_right_click_uses_it() {
    let (state, potion, _) = state();
    let mut hud = hud(state);
    let tile = hud.center(".tile", 0);

    hud.click(tile, false);
    assert!(hud.actions().iter().any(
        |action| matches!(action, Action::Click(Some(Target::Tile(item))) if *item == potion)
    ));

    hud.click(tile, true);
    let actions = hud.actions();
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::UseItem(item) if *item == potion))
    );
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Click(_)))
    );
}

#[test]
fn click_and_right_click_on_a_slot_work_like_on_a_tile() {
    let (state, potion, _) = state();
    let mut hud = hud(state);

    hud.click(hud.center(".slot", 2), false);
    assert!(
        hud.actions()
            .iter()
            .any(|action| matches!(action, Action::Click(Some(Target::Slot(2)))))
    );

    // the potion's slot uses the held potion
    hud.click(hud.center(".slot", 0), true);
    assert!(
        hud.actions()
            .iter()
            .any(|action| matches!(action, Action::UseItem(item) if *item == potion))
    );
}

#[test]
fn selected_slot_shows_the_popup() {
    let (mut state, ..) = state();
    state.selected = Some(Target::Slot(0));
    let hud = hud(state);

    let doc = hud.doc.inner();
    assert_eq!(doc.query_selector_all(".slot .details").unwrap().len(), 1);
    assert_eq!(doc.query_selector_all(".tile .details").unwrap().len(), 0);
}

#[test]
fn dragging_a_usable_tile_onto_a_slot_assigns_it() {
    let (state, ..) = state();
    let mut hud = hud(state);

    hud.drag(hud.center(".tile", 0), hud.center(".slot", 3));

    assert!(hud.actions().iter().any(|action| matches!(
        action,
        Action::Drop {
            from: Target::Tile(_),
            to: Some(Target::Slot(3))
        }
    )));
}

#[test]
fn tiles_that_cannot_be_used_are_not_dragged() {
    let (state, ..) = state();
    let mut hud = hud(state);

    hud.drag(hud.center(".tile", 1), hud.center(".slot", 3));

    let actions = hud.actions();
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Drop { .. } | Action::Click(_)))
    );
}

#[test]
fn dragging_a_slot_out_of_the_hotbar_empties_it() {
    let (state, ..) = state();
    let mut hud = hud(state);

    hud.drag(hud.center(".slot", 0), Vec2::new(400., 100.));

    assert!(hud.actions().iter().any(|action| matches!(
        action,
        Action::Drop {
            from: Target::Slot(0),
            to: None
        }
    )));
}

#[test]
fn buttons_in_the_popup_do_not_count_as_clicks_on_the_tile() {
    let (mut state, potion, _) = state();
    state.selected = Some(Target::Tile(potion));
    let mut hud = hud(state);
    let buttons = hud
        .doc
        .inner()
        .query_selector_all(".details button")
        .unwrap()
        .len();
    assert!(buttons > 0, "no popup with buttons");

    // the last one drops all of it
    hud.click(hud.center(".details button", buttons - 1), false);

    let actions = hud.actions();
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::DropItem { item, count: 1 } if *item == potion))
    );
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Click(_)))
    );
}

#[test]
fn hovering_an_item_type_highlights_it_in_the_inventory_and_the_hotbar() {
    let (state, ..) = state();
    let mut hud = hud(state);
    let highlighted =
        |hud: &Hud, selector| hud.doc.inner().query_selector_all(selector).unwrap().len();

    hud.pointer(UiEvent::PointerMove, hud.center(".tile", 0), false);
    assert_eq!(highlighted(&hud, ".slot.highlighted"), 1);
    assert_eq!(highlighted(&hud, ".tile.highlighted"), 1);

    // the sword is in no slot
    hud.pointer(UiEvent::PointerMove, hud.center(".tile", 1), false);
    assert_eq!(highlighted(&hud, ".slot.highlighted"), 0);

    hud.pointer(UiEvent::PointerMove, hud.center(".slot", 0), false);
    assert_eq!(highlighted(&hud, ".tile.highlighted"), 1);

    hud.pointer(UiEvent::PointerMove, Vec2::new(400., 100.), false);
    assert_eq!(highlighted(&hud, ".highlighted"), 0);
}

#[test]
fn click_elsewhere_puts_the_selection_down_there() {
    let (mut state, potion, _) = state();
    let mut hud = hud(state.clone());
    hud.click(Vec2::new(400., 100.), false);
    assert!(hud.actions().is_empty(), "nothing selected");

    state.selected = Some(Target::Tile(potion));
    let mut hud = self::hud(state);
    hud.click(Vec2::new(400., 100.), false);
    assert!(
        hud.actions()
            .iter()
            .any(|action| matches!(action, Action::Click(None)))
    );
}

#[test]
fn putting_down_works_alike_for_tiles_and_slots() {
    let mut world = World::new();
    let (potion, sword) = (world.spawn_empty().id(), world.spawn_empty().id());
    // the potion is usable and in slot 0, the sword isn't usable
    let usable_type = |target| match target {
        Target::Slot(0) => Some(ItemId::from("potion")),
        Target::Tile(item) if item == potion => Some(ItemId::from("potion")),
        _ => None,
    };
    let put = |from, to| put_down(from, to, usable_type);
    let potion_into = |slot| Some((slot, Some(ItemId::from("potion"))));

    // onto a slot: the type goes there
    assert_eq!(
        put(Target::Tile(potion), Some(Target::Slot(2))),
        potion_into(2)
    );
    assert_eq!(put(Target::Slot(0), Some(Target::Slot(3))), potion_into(3));
    assert_eq!(put(Target::Tile(sword), Some(Target::Slot(2))), None);

    // a slot anywhere else is emptied
    assert_eq!(put(Target::Slot(0), None), Some((0, None)));
    assert_eq!(
        put(Target::Slot(0), Some(Target::Tile(sword))),
        Some((0, None))
    );

    // back where it came from, or a tile anywhere but on a slot: nothing
    assert_eq!(put(Target::Slot(0), Some(Target::Slot(0))), None);
    assert_eq!(put(Target::Tile(potion), None), None);
    assert_eq!(put(Target::Tile(potion), Some(Target::Tile(sword))), None);
}

#[test]
fn selected_item_type_is_marked_in_the_inventory_and_the_hotbar() {
    let (mut state, potion, _) = state();
    state.selected = Some(Target::Tile(potion));
    let hud = hud(state);

    let doc = hud.doc.inner();
    assert_eq!(doc.query_selector_all(".tile.selected").unwrap().len(), 1);
    assert_eq!(doc.query_selector_all(".slot.selected").unwrap().len(), 1);
    // the popup only at the tile that was clicked
    assert_eq!(doc.query_selector_all(".details").unwrap().len(), 1);
}

#[test]
fn tiles_show_the_key_of_their_hotbar_slot() {
    let (state, ..) = state();
    let hud = hud(state);

    // only the potion is in the hotbar, in the first slot
    let doc = hud.doc.inner();
    let keys = doc.query_selector_all(".tile .key").unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(doc.get_node(keys[0]).unwrap().text_content(), "1");
}

#[test]
fn hovering_a_tile_or_slot_shows_its_tooltip() {
    let (state, ..) = state();
    let mut hud = hud(state);
    let count = |hud: &Hud, selector| hud.doc.inner().query_selector_all(selector).unwrap().len();

    hud.pointer(UiEvent::PointerMove, hud.center(".tile", 0), false);
    assert_eq!(count(&hud, ".tile .tooltip"), 1);
    assert_eq!(count(&hud, ".tooltip"), 1);
    // like the popup, without buttons
    assert_eq!(count(&hud, ".tooltip .name"), 1);
    assert_eq!(count(&hud, ".tooltip button"), 0);

    hud.pointer(UiEvent::PointerMove, hud.center(".slot", 0), false);
    assert_eq!(count(&hud, ".slot .tooltip"), 1);
    assert_eq!(count(&hud, ".tooltip"), 1);

    hud.pointer(UiEvent::PointerMove, Vec2::new(400., 100.), false);
    assert_eq!(count(&hud, ".tooltip"), 0);
}

#[test]
fn tooltip_lets_the_pointer_through_to_what_is_below() {
    let (state, ..) = state();
    let mut hud = hud(state);

    hud.pointer(UiEvent::PointerMove, hud.center(".tile", 0), false);
    let at = hud.center(".tooltip", 0);
    {
        let doc = hud.doc.inner();
        let tooltip = doc.query_selector_all(".tooltip").unwrap()[0];
        let hit = doc.elements_from_point(at.x, at.y);
        assert!(!hit.is_empty());
        assert!(!hit.contains(&tooltip), "the tooltip was hit");
    }

    // moving onto the tooltip, above its tile, leaves the tile
    hud.pointer(UiEvent::PointerMove, at, false);
    assert_eq!(
        hud.doc
            .inner()
            .query_selector_all(".tooltip")
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn no_tooltip_on_the_selected_tile() {
    let (mut state, potion, _) = state();
    state.selected = Some(Target::Tile(potion));
    let mut hud = hud(state);

    hud.pointer(UiEvent::PointerMove, hud.center(".tile", 0), false);
    let doc = hud.doc.inner();
    assert_eq!(doc.query_selector_all(".tile .details").unwrap().len(), 1);
    assert_eq!(doc.query_selector_all(".tooltip").unwrap().len(), 0);
}
