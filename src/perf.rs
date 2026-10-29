//! Frame time and the time of Avian's physics step, shown in the HUD. For the time of single
//! systems, run the game with Tracy (`cargo tracy`).
//!
//! Marker systems before and after Avian's sets measure the wall time of the physics, summed up
//! over the zero or more fixed steps per frame. [`Timings::averages`] holds the averages per frame
//! over the last [`PERIOD`].

use avian2d::prelude::PhysicsSystems;
use bevy::prelude::*;
use std::time::{Duration, Instant};

/// How often the averages are updated.
pub const PERIOD: Duration = Duration::from_millis(500);

/// Averages per frame over one [`PERIOD`].
#[derive(Clone, PartialEq, Debug)]
pub struct Averages {
    pub frame: Duration,
    pub physics: Duration,
}

#[derive(Resource, Default)]
pub struct Timings {
    /// `None` until the first period is over
    pub averages: Option<Averages>,
    physics_started: Option<Instant>,
    /// Sums since the start of the period
    physics: Duration,
    frame: Duration,
    frames: u32,
}

/// Adds the frame, and publishes the averages once a period is over.
fn finish_frame(mut timings: ResMut<Timings>, time: Res<Time<Real>>) {
    timings.frame += time.delta();
    timings.frames += 1;

    if timings.frame >= PERIOD {
        let frames = timings.frames;
        timings.averages = Some(Averages {
            frame: timings.frame / frames,
            physics: timings.physics / frames,
        });
        timings.physics = Duration::ZERO;
        timings.frame = Duration::ZERO;
        timings.frames = 0;
    }
}

fn begin_physics(mut timings: ResMut<Timings>) {
    timings.physics_started = Some(Instant::now());
}

fn end_physics(mut timings: ResMut<Timings>) {
    if let Some(started) = timings.physics_started.take() {
        timings.physics += started.elapsed();
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Timings>()
            .add_systems(Last, finish_frame)
            // around Avian's own sets
            .add_systems(
                FixedPostUpdate,
                (
                    begin_physics.before(PhysicsSystems::First),
                    end_physics.after(PhysicsSystems::Last),
                ),
            );
    }
}
