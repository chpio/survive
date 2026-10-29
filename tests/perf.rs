use crate::common;
use avian2d::prelude::*;
use bevy::prelude::*;
use std::time::Duration;
use survival::perf::{self, Timings};

#[test]
fn measures_time_per_frame_of_the_physics() {
    let mut app = common::app_with(perf::Plugin);
    app.add_systems(
        FixedPostUpdate,
        (|| std::thread::sleep(Duration::from_millis(2))).in_set(PhysicsSystems::StepSimulation),
    );

    // the averages are published after half a second, which the test app advances by one fixed
    // step of 1/64 s per update
    let mut updates = 0;
    while app.world().resource::<Timings>().averages.is_none() {
        app.update();
        updates += 1;
        assert!(updates < 1000, "no averages after {updates} updates");
    }

    let averages = app.world().resource::<Timings>().averages.clone().unwrap();
    let physics = averages.physics;
    assert!(
        physics >= Duration::from_millis(2) && physics < Duration::from_millis(10),
        "physics {physics:?}"
    );
    assert!(averages.frame >= physics, "frame {:?}", averages.frame);
}
