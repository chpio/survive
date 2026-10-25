//! Time the game's main steps take per frame, shown in the HUD.
//!
//! Most steps are [`Step`] system sets. Marker systems before and after them measure the wall
//! time in between. Steps within a single system are recorded by it with [`Timings::add`]. Measured steps in the same schedule run one after another, so their times don't
//! include each other; other systems may still run in parallel. Steps in the fixed timestep run
//! zero or more times per frame and are summed up. Rendering the HUD happens in
//! the render world, which reports its time through a shared counter. The time between the end
//! of one frame's schedules and the start of the next one's is [`Step::Wait`].
//! [`Timings::averages`] holds the averages per frame over the last [`PERIOD`].

use avian2d::prelude::PhysicsSystems;
use bevy::{
    app::MainScheduleOrder,
    ecs::schedule::ScheduleLabel,
    prelude::*,
    render::{Render, RenderApp, RenderSystems},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// How often the averages are updated.
pub const PERIOD: Duration = Duration::from_millis(500);

#[derive(SystemSet, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Step {
    FlowField,
    FlowFieldOverlay,
    Enemies,
    Combat,
    Movement,
    /// Avian's physics step, measured around its own sets
    Physics,
    /// Dioxus applying changes to the HUD, measured in its paint system
    HudUpdate,
    /// Blitz resolving styles and layout
    HudLayout,
    /// Blitz turning the HUD into a Vello scene, a list of drawing commands
    HudScene,
    /// Between two frames on the main thread, mostly waiting for the render thread, which waits
    /// for VSync. Not a set, measured from the end of one frame's schedules to the start of the
    /// next one's.
    Wait,
    /// In the render world
    HudRender,
}

impl Step {
    pub const ALL: [Step; 11] = [
        Step::FlowField,
        Step::FlowFieldOverlay,
        Step::Enemies,
        Step::Combat,
        Step::Movement,
        Step::Physics,
        Step::HudUpdate,
        Step::HudLayout,
        Step::HudScene,
        Step::Wait,
        Step::HudRender,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Step::FlowField => "Flowfield",
            Step::FlowFieldOverlay => "Flowfield-Overlay",
            Step::Enemies => "Gegner",
            Step::Combat => "Kampf",
            Step::Movement => "Bewegung",
            Step::Physics => "Physik",
            Step::HudUpdate => "Aktualisieren",
            Step::HudLayout => "Layout",
            Step::HudScene => "Szene",
            Step::Wait => "Warten (VSync/Render)",
            Step::HudRender => "HUD rendern",
        }
    }
}

/// Entry of the tree the HUD shows the steps in.
pub enum Node {
    Step(Step),
    /// Labeled group of steps, which takes their summed time
    Group(&'static str, &'static [Step]),
}

/// Steps on the main thread, which add up to the frame together with [`Averages::rest`].
pub const MAIN_THREAD: &[Node] = &[
    Node::Group(
        "Spiellogik",
        &[Step::FlowField, Step::Enemies, Step::Combat],
    ),
    Node::Group("Fester Takt", &[Step::Movement, Step::Physics]),
    Node::Step(Step::FlowFieldOverlay),
    Node::Group("HUD", &[Step::HudUpdate, Step::HudLayout, Step::HudScene]),
    Node::Step(Step::Wait),
];

/// Steps in the render world, which runs in parallel to the main thread.
pub const RENDER_THREAD: &[Step] = &[Step::HudRender];

/// Averages per frame over one [`PERIOD`].
#[derive(Clone, PartialEq, Debug)]
pub struct Averages {
    pub frame: Duration,
    /// In the order of [`Step::ALL`]
    pub steps: [Duration; Step::ALL.len()],
}

impl Averages {
    pub fn step(&self, step: Step) -> Duration {
        self.steps[step as usize]
    }

    pub fn node(&self, node: &Node) -> Duration {
        match node {
            Node::Step(step) => self.step(*step),
            Node::Group(_, steps) => steps.iter().map(|&step| self.step(step)).sum(),
        }
    }

    /// Frame time outside the measured steps on the main thread: Bevy itself and preparing the
    /// rendering.
    pub fn rest(&self) -> Duration {
        let measured: Duration = MAIN_THREAD.iter().map(|node| self.node(node)).sum();
        self.frame.saturating_sub(measured)
    }
}

#[derive(Resource, Default)]
pub struct Timings {
    /// `None` until the first period is over
    pub averages: Option<Averages>,
    started: [Option<Instant>; Step::ALL.len()],
    /// Sums since the start of the period
    steps: [Duration; Step::ALL.len()],
    frame: Duration,
    frames: u32,
    /// When the last frame's schedules were done
    frame_end: Option<Instant>,
}

impl Timings {
    fn begin(&mut self, step: Step) {
        self.started[step as usize] = Some(Instant::now());
    }

    fn end(&mut self, step: Step) {
        if let Some(started) = self.started[step as usize].take() {
            self.add(step, started.elapsed());
        }
    }

    pub fn add(&mut self, step: Step, duration: Duration) {
        self.steps[step as usize] += duration;
    }
}

/// Runs before all other schedules of a frame.
#[derive(ScheduleLabel, Clone, PartialEq, Eq, Hash, Debug)]
struct FrameStart;

/// Runs after all other schedules of a frame.
#[derive(ScheduleLabel, Clone, PartialEq, Eq, Hash, Debug)]
struct FrameEnd;

/// Nanoseconds the render world spent on [`Step::HudRender`] since the main world last read it.
#[derive(Resource, Clone, Default)]
struct RenderTime(Arc<AtomicU64>);

/// Start of the current [`Step::HudRender`] in the render world.
#[derive(Resource, Default)]
struct RenderStarted(Option<Instant>);

/// Adds the frame and the render world's time, and publishes the averages once a period is over.
fn finish_frame(mut timings: ResMut<Timings>, time: Res<Time<Real>>, render_time: Res<RenderTime>) {
    let rendered = Duration::from_nanos(render_time.0.swap(0, Ordering::Relaxed));
    timings.add(Step::HudRender, rendered);
    timings.frame += time.delta();
    timings.frames += 1;

    if timings.frame >= PERIOD {
        let frames = timings.frames;
        timings.averages = Some(Averages {
            frame: timings.frame / frames,
            steps: timings.steps.map(|sum| sum / frames),
        });
        timings.steps = default();
        timings.frame = Duration::ZERO;
        timings.frames = 0;
    }
}

fn begin(step: Step) -> impl FnMut(ResMut<Timings>) {
    move |mut timings| timings.begin(step)
}

fn end(step: Step) -> impl FnMut(ResMut<Timings>) {
    move |mut timings| timings.end(step)
}

/// Measures `steps` in `schedule`, which run in this order.
fn measure(app: &mut App, schedule: impl ScheduleLabel + Clone, steps: &[Step]) {
    for (i, &step) in steps.iter().enumerate() {
        app.add_systems(schedule.clone(), end(step).after(step));
        match i.checked_sub(1).map(|previous| steps[previous]) {
            Some(previous) => {
                app.configure_sets(schedule.clone(), step.after(previous));
                // only once the previous step is done, so its time isn't counted
                app.add_systems(schedule.clone(), begin(step).before(step).after(previous));
            }
            None => {
                app.add_systems(schedule.clone(), begin(step).before(step));
            }
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        let render_time = RenderTime::default();
        app.init_resource::<Timings>()
            .insert_resource(render_time.clone())
            .add_systems(Last, finish_frame)
            .add_schedule(Schedule::new(FrameStart))
            .add_schedule(Schedule::new(FrameEnd))
            .add_systems(FrameStart, |mut timings: ResMut<Timings>| {
                if let Some(frame_end) = timings.frame_end.take() {
                    timings.add(Step::Wait, frame_end.elapsed());
                }
            })
            .add_systems(FrameEnd, |mut timings: ResMut<Timings>| {
                timings.frame_end = Some(Instant::now());
            });
        let mut order = app.world_mut().resource_mut::<MainScheduleOrder>();
        order.insert_before(First, FrameStart);
        order.insert_after(Last, FrameEnd);

        measure(app, PreUpdate, &[Step::FlowField]);
        measure(
            app,
            Update,
            &[Step::FlowFieldOverlay, Step::Enemies, Step::Combat],
        );
        measure(app, FixedUpdate, &[Step::Movement]);
        // around Avian's own sets
        app.add_systems(
            FixedPostUpdate,
            (
                begin(Step::Physics).before(PhysicsSystems::First),
                end(Step::Physics).after(PhysicsSystems::Last),
            ),
        );

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .insert_resource(render_time)
                .init_resource::<RenderStarted>()
                .add_systems(
                    Render,
                    (
                        (|mut started: ResMut<RenderStarted>| started.0 = Some(Instant::now()))
                            .before(Step::HudRender),
                        (|mut started: ResMut<RenderStarted>, render_time: Res<RenderTime>| {
                            if let Some(started) = started.0.take() {
                                let nanos = started.elapsed().as_nanos() as u64;
                                render_time.0.fetch_add(nanos, Ordering::Relaxed);
                            }
                        })
                        .after(Step::HudRender),
                    )
                        .in_set(RenderSystems::Prepare),
                );
        }
    }
}
