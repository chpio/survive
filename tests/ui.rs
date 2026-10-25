use bevy::math::Vec2;
use blitz_dom::{Document as _, DocumentConfig};
use blitz_traits::shell::{ColorScheme, Viewport};
use dioxus::prelude::*;
use dioxus_native_dom::DioxusDocument;
use survival::ui::blocks_pointer;

/// A panel that blocks the pointer and a layout container next to it that doesn't.
fn app() -> Element {
    rsx! {
        div {
            class: "panel blocks-pointer",
            style: "position: absolute; left: 0; top: 0; width: 80px; height: 80px; padding: 10px;",
            div { style: "width: 20px; height: 20px;" }
        }
        div {
            class: "layout",
            style: "position: absolute; left: 200px; top: 0; width: 100px; height: 100px;",
        }
    }
}

/// A resolved document for a window of 400 × 200 logical pixels.
fn document(scale: f32) -> DioxusDocument {
    let mut doc = DioxusDocument::new(VirtualDom::new(app), DocumentConfig::default());
    doc.initial_build();
    let mut inner = doc.inner_mut();
    let (width, height) = ((400. * scale) as u32, (200. * scale) as u32);
    inner.set_viewport(Viewport::new(width, height, scale, ColorScheme::Dark));
    inner.resolve(0.);
    drop(inner);
    doc
}

#[test]
fn panel_blocks_the_pointer() {
    let doc = document(1.);
    let doc = doc.inner();
    // in the padding and on a child
    assert!(blocks_pointer(&doc, Vec2::new(5., 5.)));
    assert!(blocks_pointer(&doc, Vec2::new(15., 15.)));
}

#[test]
fn other_elements_and_the_background_let_the_pointer_through() {
    let doc = document(1.);
    let doc = doc.inner();
    assert!(!blocks_pointer(&doc, Vec2::new(250., 50.)));
    assert!(!blocks_pointer(&doc, Vec2::new(350., 150.)));
    // just outside the panel (100 px including the padding)
    assert!(!blocks_pointer(&doc, Vec2::new(105., 50.)));
}

#[test]
fn positions_are_logical_pixels() {
    let doc = document(2.);
    let doc = doc.inner();
    assert!(blocks_pointer(&doc, Vec2::new(95., 50.)));
    assert!(!blocks_pointer(&doc, Vec2::new(105., 50.)));
}
