//! Health and melee combat between the player and the enemies.
//!
//! A unit with an [`Attack`] hits every opponent whose collider is within its reach as soon as its
//! cooldown is over: the player hits all enemies around it, an awake enemy hits the player. A hit
//! deals damage and knocks the target away from the attacker with an impulse, so lighter units
//! fly further. A hit that takes the last health triggers [`Died`] on the target; the objects
//! decide what dying means (enemies are despawned, the player starts the round over). Used items
//! with a healing effect heal their user. [`Restrained`] units only attack while they are [`Striking`], e.g. swarm enemies
//! during their approach, which needs an attack token ([`tokens`]).

use crate::{
    items::{Effect, ItemUsed},
    objects::{
        enemy::{self, Asleep, Enemy},
        player::Player,
    },
};
use avian2d::prelude::*;
use bevy::{ecs::query::QueryFilter, prelude::*};
use std::time::Duration;

pub mod health_bar;
pub mod tokens;

#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    /// Full health.
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.
    }
}

/// Melee attack of a unit.
#[derive(Component, Clone, Debug)]
pub struct Attack {
    pub damage: f32,
    /// Distance in px from the attacker's center up to which targets' colliders are hit
    pub reach: f32,
    /// Impulse in mass · px/s that pushes each target away from the attacker
    pub knockback: f32,
    /// Time until the next hit, finished while the attack is ready
    pub cooldown: Timer,
}

impl Attack {
    /// Attack that is ready right away and then waits `cooldown` after each hit.
    pub fn new(damage: f32, reach: f32, knockback: f32, cooldown: Duration) -> Self {
        let mut cooldown = Timer::new(cooldown, TimerMode::Once);
        cooldown.finish();
        Self {
            damage,
            reach,
            knockback,
            cooldown,
        }
    }
}

/// A unit that only attacks while it is [`Striking`].
#[derive(Component, Default, Clone, Copy, PartialEq, Debug)]
pub struct Restrained;

/// A [`Restrained`] unit that attacks right now.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Striking;

/// A unit with an [`Attack`] hit another one.
#[derive(Message, Clone, Copy, PartialEq, Debug)]
pub struct Hit {
    pub attacker: Entity,
    pub target: Entity,
}

/// A hit took the last health of `entity`. Triggered once per death; the target's own observers
/// handle it, e.g. by despawning or healing it.
#[derive(EntityEvent, Clone, Copy, PartialEq, Debug)]
pub struct Died {
    pub entity: Entity,
}

/// Units matching `Attacker` hit units matching `Target` that are in reach. There are few
/// attackers or few targets (the player on one side), so checking every pair is cheap.
fn attack<Attacker: QueryFilter, Target: QueryFilter>(
    time: Res<Time>,
    mut attackers: Query<(Entity, &Transform, &mut Attack), Attacker>,
    mut targets: Query<(Entity, &Transform, &Collider, &mut Health, Forces), Target>,
    mut hits: MessageWriter<Hit>,
    mut commands: Commands,
) {
    for (attacker, transform, mut attack) in &mut attackers {
        attack.cooldown.tick(time.delta());
        if !attack.cooldown.is_finished() {
            continue;
        }

        let position = transform.translation.xy();
        for (target, target_transform, collider, mut health, mut forces) in &mut targets {
            let target_position = target_transform.translation.xy();
            // units don't rotate, enemies only turn their visible body
            let distance =
                collider.distance_to_point(target_position, Rotation::IDENTITY, position, true);
            if distance > attack.reach {
                continue;
            }

            attack.cooldown.reset();
            let was_alive = !health.is_dead();
            health.current = (health.current - attack.damage).max(0.);
            // units on the same spot are pushed in any direction
            let away = (target_position - position)
                .try_normalize()
                .unwrap_or(Vec2::X);
            forces.apply_linear_impulse(away * attack.knockback);
            hits.write(Hit { attacker, target });
            if was_alive && health.is_dead() {
                commands.trigger(Died { entity: target });
            }
        }
    }
}

/// Applies the effects of used items to their users' health.
fn apply_item_effects(mut used: MessageReader<ItemUsed>, mut healths: Query<&mut Health>) {
    for &ItemUsed { by, effect } in used.read() {
        let Ok(mut health) = healths.get_mut(by) else {
            continue;
        };
        match effect {
            Effect::Heal(amount) => health.current = (health.current + amount).min(health.max),
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        // also registered by `items::Plugin`, but combat works without items
        app.add_message::<ItemUsed>()
            .add_message::<Hit>()
            .add_systems(
                Update,
                (
                    apply_item_effects,
                    attack::<With<Player>, With<Enemy>>,
                    // sleeping enemies can be hit, but don't attack
                    attack::<
                        (
                            With<Enemy>,
                            Without<Asleep>,
                            Or<(Without<Restrained>, With<Striking>)>,
                        ),
                        With<Player>,
                    >
                        .run_if(enemy::active),
                )
                    .chain(),
            );
    }
}
