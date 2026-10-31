//! Swarm enemies (see `docs/schwarm.md`): small enemies circle the player and wait for an
//! [`AttackToken`], then run at the player, hit once and retreat to the side, so the next attack
//! comes from another direction. A token is only given back [`SwarmValues::token_time`] after the
//! approach began, so each token allows one hit per that time at most, and the damage per second
//! doesn't grow with the number of enemies. Each state only sets the
//! [`DesiredVelocity`]; the force-based movement brakes and turns. The values come from
//! `assets/swarm.ron` ([`values`]).

pub mod values;

use super::{Asleep, Crowd, Enemy, Heading, MOVE_SPEED, Separation, towards_player};
use crate::{
    combat::{
        Attack, Hit, Restrained, Striking,
        tokens::{AttackToken, AttackTokens},
    },
    flowfield::FlowField,
    movement::DesiredVelocity,
    objects::player,
};
use avian2d::prelude::*;
use bevy::{ecs::entity::EntityHashSet, prelude::*};
use serde::Deserialize;
use std::time::Duration;

/// Enemies up to this radius in px attack as a swarm, the small ones.
pub const MAX_RADIUS: f32 = 5.;

/// Distance in px to the retreat point at which the enemy counts as there.
const REACHED: f32 = 4.;

/// Speed in px/s below which a retreating enemy counts as not moving.
const STUCK_SPEED: f32 = 10.;

/// How long a retreating enemy may not move before it gives up, longer than it takes to turn
/// around after a hit.
const STUCK_TIME: Duration = Duration::from_millis(300);

/// Steps in which the retreat point is turned back towards the direction straight away from the
/// player while it is blocked, in degrees.
const RETREAT_ANGLE_STEP: f32 = 15.;

/// Distance in px from the circle distance over which a circling enemy goes from walking around
/// the player to walking straight towards or away from it.
const CIRCLE_BAND: f32 = 20.;

/// The swarm's values, from `assets/swarm.ron`. Speeds are relative to [`MOVE_SPEED`], ranges are
/// `(min, max)`, picked from at random.
#[derive(Asset, Resource, TypePath, Deserialize, Clone, PartialEq, Debug)]
pub struct SwarmValues {
    /// Damage per hit
    pub damage: f32,
    /// Speed while running at the player
    pub approach_speed: f32,
    /// Speed while circling the player
    pub circle_speed: f32,
    /// Speed while retreating
    pub retreat_speed: f32,
    /// Seconds after which an enemy that didn't reach the player gives up
    pub approach_timeout: f32,
    /// Seconds from the start of an approach until the enemy gives its token back, whatever it
    /// does by then
    pub token_time: f32,
    /// Seconds a retreat lasts at most
    pub retreat_time: (f32, f32),
    /// Degrees the retreat point is turned from the direction of the attack, to either side
    pub retreat_angle: (f32, f32),
    /// Distance in px to the player at which enemies circle it, per enemy
    pub circle_distance: (f32, f32),
    /// How many enemies attack at once, see [`AttackTokens`]
    pub max_tokens: usize,
}

/// Random numbers of the swarm. Tests insert one with a fixed seed.
#[derive(Resource, Default)]
pub struct SwarmRng(pub fastrand::Rng);

impl SwarmRng {
    fn range(&mut self, (min, max): (f32, f32)) -> f32 {
        min + self.0.f32() * (max - min)
    }

    /// 1 or -1.
    fn sign(&mut self) -> f32 {
        if self.0.bool() { 1. } else { -1. }
    }
}

/// An enemy that attacks as part of a swarm, given to small enemies when they spawn. It gets a
/// [`SwarmState`] while it is awake.
#[derive(Component, Default, Clone, Copy, PartialEq, Debug)]
#[require(Restrained)]
pub struct Swarm;

/// How an awake swarm enemy circles the player, picked when it wakes up.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Orbit {
    /// Distance to the player in px
    pub distance: f32,
    /// 1 to circle counterclockwise, -1 clockwise
    pub turn: f32,
}

/// When a swarm enemy gives its [`AttackToken`] back, held along with it.
#[derive(Component, Clone, PartialEq, Debug)]
pub struct TokenHold(pub Timer);

/// What an awake swarm enemy is doing. It gets an [`AttackToken`] when it starts approaching and
/// keeps it for [`SwarmValues::token_time`], also while retreating or circling afterwards. A hit
/// isn't a state of its own: the enemy is [`Striking`] while approaching, so it attacks the
/// player once it is in reach, and retreats right after.
#[derive(Component, Clone, PartialEq, Debug)]
pub enum SwarmState {
    /// Circles the player at its [`Orbit`] and waits for a token, once it gave its last one back.
    Circle {
        /// Elapsed game time when it started circling, so the one waiting longest goes first
        since: Duration,
    },
    /// Runs at the player to hit it.
    Approach { timer: Timer },
    /// Walks to a point to the side of where it attacked from.
    Retreat {
        /// Where it walks to, relative to the player, so it moves along with them
        target: Vec2,
        timer: Timer,
        /// How long it hasn't moved although it wants to
        stuck: Duration,
    },
}

/// Gives swarm enemies that woke up or spawned awake a state and an orbit, and takes them away
/// with their token from those that fell asleep. The commands of the swarm's systems may come
/// after combat despawned the enemy, so they don't fail for missing entities.
pub fn start_and_stop(
    mut commands: Commands,
    time: Res<Time>,
    values: Res<SwarmValues>,
    mut rng: ResMut<SwarmRng>,
    woken: Query<Entity, (With<Swarm>, Without<Asleep>, Without<SwarmState>)>,
    asleep: Query<Entity, (With<SwarmState>, With<Asleep>)>,
) {
    for entity in &woken {
        commands.entity(entity).try_insert((
            SwarmState::Circle {
                since: time.elapsed(),
            },
            Orbit {
                distance: rng.range(values.circle_distance),
                turn: rng.sign(),
            },
        ));
    }
    for entity in &asleep {
        commands
            .entity(entity)
            .try_remove::<(SwarmState, Orbit, Striking, AttackToken, TokenHold)>();
    }
}

/// Whether a unit with `radius` can walk to `point`, as far as the flow field knows. Without a
/// computed grid, everything counts as free.
fn is_free(flow_field: &FlowField, point: Vec2, radius: f32) -> bool {
    match flow_field.grid(radius) {
        Some(grid) if grid.is_computed() => flow_field.walking_distance(point, radius).is_some(),
        _ => true,
    }
}

/// The retreat point of an enemy with `radius` at `position` that attacked the player at
/// `player`, relative to the player: at `distance`, turned by `angle` (in radians) from the
/// direction of the attack, or less while that point is blocked, or else straight away from the
/// player.
pub fn retreat_target(
    flow_field: &FlowField,
    radius: f32,
    position: Vec2,
    player: Vec2,
    distance: f32,
    angle: f32,
) -> Vec2 {
    let away = (position - player).try_normalize().unwrap_or(Vec2::X);
    let steps = (angle.abs() / RETREAT_ANGLE_STEP.to_radians()).ceil() as u32;
    (0..steps)
        .map(|step| Vec2::from_angle(angle * (1. - step as f32 / steps as f32)))
        .map(|rotation| rotation.rotate(away) * distance)
        .find(|&offset| is_free(flow_field, player + offset, radius))
        .unwrap_or(away * distance)
}

/// Switches the states of the swarm enemies: hits and timeouts end an approach, the end of a
/// retreat leads back to circling, tokens are given back after their time, and free ones go to
/// the enemies circling longest.
pub fn update_states(
    mut commands: Commands,
    time: Res<Time>,
    values: Res<SwarmValues>,
    tokens: Res<AttackTokens>,
    mut rng: ResMut<SwarmRng>,
    flow_field: Res<FlowField>,
    mut hits: MessageReader<Hit>,
    player: Single<&Transform, With<player::Player>>,
    held: Query<(), With<AttackToken>>,
    mut enemies: Query<(
        Entity,
        &Enemy,
        &Transform,
        &LinearVelocity,
        &DesiredVelocity,
        &Orbit,
        &mut Attack,
        &mut SwarmState,
        Option<&mut TokenHold>,
    )>,
) {
    let player = player.translation.xy();
    // hit the player or hit by it
    let hit: EntityHashSet = hits
        .read()
        .flat_map(|hit| [hit.attacker, hit.target])
        .collect();
    let mut held = held.iter().count();
    let mut waiting = vec![];

    for (entity, enemy, transform, velocity, desired, orbit, _, mut state, hold) in &mut enemies {
        let position = transform.translation.xy();
        let holds_token = match hold {
            Some(mut hold) => {
                hold.0.tick(time.delta());
                let given_back = hold.0.is_finished();
                if given_back {
                    commands
                        .entity(entity)
                        .try_remove::<(AttackToken, TokenHold)>();
                    held = held.saturating_sub(1);
                }
                !given_back
            }
            None => false,
        };
        match &mut *state {
            SwarmState::Circle { since } => {
                if !holds_token {
                    waiting.push((*since, entity));
                }
            }
            SwarmState::Approach { timer } => {
                timer.tick(time.delta());
                if hit.contains(&entity) || timer.is_finished() {
                    let angle = rng.range(values.retreat_angle).to_radians() * rng.sign();
                    *state = SwarmState::Retreat {
                        target: retreat_target(
                            &flow_field,
                            enemy.radius,
                            position,
                            player,
                            orbit.distance,
                            angle,
                        ),
                        timer: Timer::from_seconds(rng.range(values.retreat_time), TimerMode::Once),
                        stuck: Duration::ZERO,
                    };
                    commands.entity(entity).try_remove::<Striking>();
                }
            }
            SwarmState::Retreat {
                target,
                timer,
                stuck,
            } => {
                timer.tick(time.delta());
                if desired.0 != Vec2::ZERO && velocity.length() < STUCK_SPEED {
                    *stuck += time.delta();
                } else {
                    *stuck = Duration::ZERO;
                }
                if timer.is_finished()
                    || position.distance(player + *target) < REACHED
                    || *stuck >= STUCK_TIME
                {
                    *state = SwarmState::Circle {
                        since: time.elapsed(),
                    };
                }
            }
        }
    }

    waiting.sort();
    for (_, entity) in waiting.into_iter().take(tokens.free(held)) {
        let Ok((.., mut attack, mut state, _)) = enemies.get_mut(entity) else {
            continue;
        };
        // the values may have changed since its last attack
        attack.damage = values.damage;
        *state = SwarmState::Approach {
            timer: Timer::from_seconds(values.approach_timeout, TimerMode::Once),
        };
        commands.entity(entity).try_insert((
            AttackToken,
            TokenHold(Timer::from_seconds(values.token_time, TimerMode::Once)),
            Striking,
        ));
    }
}

/// Lets the swarm enemies walk by their state, away from each other like all enemies.
pub fn move_swarm(
    player: Single<&Transform, With<player::Player>>,
    values: Res<SwarmValues>,
    flow_field: Res<FlowField>,
    crowd: Crowd,
    mut enemies: Query<(
        Entity,
        &Enemy,
        &Transform,
        &Separation,
        &Orbit,
        &SwarmState,
        &mut Heading,
        &mut DesiredVelocity,
    )>,
) {
    let player = player.translation.xy();
    let max_radius = crowd.max_radius();
    for (entity, enemy, transform, separation, orbit, state, mut heading, mut desired) in
        &mut enemies
    {
        let position = transform.translation.xy();
        let (direction, speed) = match state {
            SwarmState::Circle { .. } => {
                let offset = position - player;
                let away = offset.normalize_or_zero();
                // > 0 when too far away, < 0 when too close
                let error = ((offset.length() - orbit.distance) / CIRCLE_BAND).clamp(-1., 1.);
                let radial = if error > 0. {
                    // around walls between it and the player
                    towards_player(&flow_field, enemy, position, player) * error
                } else {
                    away * -error
                };
                let around = away.perp() * orbit.turn * (1. - error.abs());
                // walks up at the normal speed from further away
                let speed = values.circle_speed.lerp(1., error.max(0.));
                (radial + around, speed)
            }
            SwarmState::Approach { .. } => (
                towards_player(&flow_field, enemy, position, player),
                values.approach_speed,
            ),
            SwarmState::Retreat { target, .. } => {
                let offset = player + *target - position;
                // slows down on the last px, so it doesn't overshoot
                let slow = (offset.length() / (REACHED * 4.)).min(1.);
                (offset.normalize_or_zero(), values.retreat_speed * slow)
            }
        };
        if direction != Vec2::ZERO {
            heading.set_if_neq(Heading(direction.normalize()));
        }

        let push = crowd.push(entity, position, separation, max_radius);
        desired.set_if_neq(DesiredVelocity(
            (direction + push * separation.strength).clamp_length_max(1.) * speed * MOVE_SPEED,
        ));
    }
}

/// The swarm's behavior. Its values are loaded by [`values::Plugin`]; tests insert
/// [`SwarmValues`] and [`AttackTokens`] directly.
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SwarmRng>()
            // also registered by `combat::Plugin`
            .add_message::<Hit>()
            .add_systems(
                Update,
                (
                    start_and_stop,
                    (update_states, move_swarm).chain().run_if(super::active),
                )
                    .chain()
                    .after(super::wake_and_sleep)
                    .run_if(resource_exists::<SwarmValues>),
            );
    }
}
