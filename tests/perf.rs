mod common;

use bevy::prelude::*;
use std::time::Duration;
use survival::perf::{self, Averages, Node, Step, Timings};

#[test]
fn measures_time_per_frame_of_each_step() {
    let mut app = common::app_with(perf::Plugin);
    app.add_systems(
        Update,
        (|| std::thread::sleep(Duration::from_millis(2))).in_set(Step::Combat),
    );

    // the averages are published after half a second, which the test app advances by 1/64 s per
    // update
    let mut updates = 0;
    while app.world().resource::<Timings>().averages.is_none() {
        app.update();
        updates += 1;
        assert!(updates < 1000, "no averages after {updates} updates");
    }

    let averages = app.world().resource::<Timings>().averages.clone().unwrap();
    let combat = averages.steps[Step::Combat as usize];
    let enemies = averages.steps[Step::Enemies as usize];
    assert!(
        combat >= Duration::from_millis(2) && combat < Duration::from_millis(10),
        "combat {combat:?}"
    );
    // nothing runs in it
    assert!(enemies < Duration::from_micros(200), "enemies {enemies:?}");
    assert!(averages.frame >= combat, "frame {:?}", averages.frame);
}

/// Steps in the HUD's tree, in order.
fn steps_in_tree() -> Vec<Step> {
    perf::MAIN_THREAD
        .iter()
        .flat_map(|node| match node {
            Node::Step(step) => vec![*step],
            Node::Group(_, steps) => steps.to_vec(),
        })
        .chain(perf::RENDER_THREAD.iter().copied())
        .collect()
}

#[test]
fn every_step_is_in_the_tree_once() {
    let mut in_tree = steps_in_tree();
    in_tree.sort_by_key(|&step| step as usize);
    assert_eq!(in_tree, Step::ALL);
}

#[test]
fn groups_sum_up_their_steps_and_the_rest_fills_the_frame() {
    let ms = Duration::from_millis;
    let mut averages = Averages {
        frame: ms(20),
        steps: [Duration::ZERO; Step::ALL.len()],
    };
    for step in Step::ALL {
        averages.steps[step as usize] = ms(1);
    }

    let logic = &perf::MAIN_THREAD[0];
    assert!(matches!(logic, Node::Group(_, steps) if steps.len() == 3));
    assert_eq!(averages.node(logic), ms(3));
    // all steps but the one in the render thread
    assert_eq!(averages.rest(), ms(20 - 7));
}
