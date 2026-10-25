//! Force-based movement for dynamic bodies.
//!
//! Systems set a body's [`DesiredVelocity`]. An anchor point moves along at that velocity, and
//! each physics step a critically damped spring pulls the body towards the anchor, with a force
//! limited by the body's [`Strength`]. Bodies pushing each other therefore win by strength and
//! mass, instead of whoever sets their velocity last.
//!
//! Following a position rather than only a velocity lets a body hold its ground against weaker
//! pushers. A pure velocity controller reacts one step late to contact impulses and slowly drifts.

use crate::perf;
use avian2d::prelude::*;
use bevy::prelude::*;

/// Length unit of the physics in px, one tile, about the size of the units. Avian scales its
/// tolerances with it.
pub const LENGTH_UNIT: f32 = 16.;

/// Distance at which Avian already treats shapes as touching, in [`LENGTH_UNIT`]s (0.16 px).
/// Bodies sliding along an obstacle touch the corners where the edges between its triangles end
/// within the default of 0.02, and are pushed away from them.
pub const CONTACT_TOLERANCE: f32 = 0.01;

/// Angular frequency of the spring towards the anchor, in rad/s. Higher values follow the anchor
/// more tightly.
const STIFFNESS: f32 = 45.;

/// Velocity a body tries to move at, in px/s.
#[derive(Component, Default, PartialEq)]
pub struct DesiredVelocity(pub Vec2);

/// Maximum force a body uses to follow its [`DesiredVelocity`], in mass · px/s².
#[derive(Component)]
#[require(DesiredVelocity, Anchor)]
pub struct Strength(pub f32);

/// Position the body is pulled towards, `None` until the first physics step.
#[derive(Component, Default)]
struct Anchor(Option<Vec2>);

/// Dynamic body moved by this module. Its rotation is locked, so contacts don't spin it. It has no
/// friction, so it slides along obstacles and other bodies when pushing against them at an angle,
/// instead of sticking to them.
pub fn body(collider: Collider, mass: f32, strength: f32) -> impl Bundle {
    (
        RigidBody::Dynamic,
        collider,
        LockedAxes::ROTATION_LOCKED,
        Mass(mass),
        NoAutoMass,
        Strength(strength),
        // `Min`, so the friction of what it touches doesn't count either
        Friction::ZERO.with_combine_rule(CoefficientCombine::Min),
    )
}

fn apply_movement_forces(
    time: Res<Time>,
    mut bodies: Query<(
        Forces,
        &Position,
        &ComputedMass,
        &DesiredVelocity,
        &Strength,
        &mut Anchor,
    )>,
) {
    for (mut forces, position, mass, desired, strength, mut anchor) in &mut bodies {
        let mass = mass.value();
        let spring = mass * STIFFNESS * STIFFNESS;
        let damping = 2. * mass * STIFFNESS;

        let target = anchor.0.unwrap_or(position.0) + desired.0 * time.delta_secs();
        let offset = target - position.0;
        let force = offset * spring + (desired.0 - forces.linear_velocity()) * damping;

        anchor.0 = Some(
            if force.length() > strength.0 && force.dot(desired.0) > 0. {
                // accelerating at full strength: keep the anchor at the body, so it doesn't run
                // ahead and make the body overshoot once it reaches its speed
                position.0
            } else {
                // pushed or blocked: hold on to the anchor, but no further away than the spring
                // reaches full strength, so the body doesn't snap back once it is free
                position.0 + offset.clamp_length_max(strength.0 / spring)
            },
        );

        forces.apply_force(force.clamp_length_max(strength.0));
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        // once per physics step, since applied forces are cleared after each step
        app.add_systems(
            FixedUpdate,
            apply_movement_forces.in_set(perf::Step::Movement),
        );
    }
}
