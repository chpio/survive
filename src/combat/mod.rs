//! Health and melee combat between the player and the enemies.
//!
//! A unit with an [`Attack`] hits every opponent whose collider is within its reach as soon as its
//! cooldown is over: the player hits all enemies around it, an enemy hits the player. A hit deals
//! damage and knocks the target away from the attacker with an impulse, so lighter units fly
//! further. Enemies without health are despawned. When the player dies, the round starts over:
//! all enemies are removed and the player is healed.

use crate::{
    objects::{enemy::Enemy, player::Player},
    perf,
};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::time::Duration;

pub mod health_bar;

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

/// Units with `Attacker` hit units with `Target` that are in reach. There are few attackers or
/// few targets (the player on one side), so checking every pair is cheap.
fn attack<Attacker: Component, Target: Component>(
    time: Res<Time>,
    mut attackers: Query<(&Transform, &mut Attack), With<Attacker>>,
    mut targets: Query<
        (&Transform, &Collider, &mut Health, Forces),
        (With<Target>, Without<Attacker>),
    >,
) {
    for (transform, mut attack) in &mut attackers {
        attack.cooldown.tick(time.delta());
        if !attack.cooldown.is_finished() {
            continue;
        }

        let position = transform.translation.xy();
        for (target_transform, collider, mut health, mut forces) in &mut targets {
            let target_position = target_transform.translation.xy();
            // units don't rotate, enemies only turn their visible body
            let distance =
                collider.distance_to_point(target_position, Rotation::IDENTITY, position, true);
            if distance > attack.reach {
                continue;
            }

            attack.cooldown.reset();
            health.current = (health.current - attack.damage).max(0.);
            // units on the same spot are pushed in any direction
            let away = (target_position - position)
                .try_normalize()
                .unwrap_or(Vec2::X);
            forces.apply_linear_impulse(away * attack.knockback);
        }
    }
}

fn despawn_dead_enemies(mut commands: Commands, enemies: Query<(Entity, &Health), With<Enemy>>) {
    for (entity, health) in &enemies {
        if health.is_dead() {
            commands.entity(entity).despawn();
        }
    }
}

/// Starts the round over when the player dies: removes all enemies and heals the player.
fn restart_on_player_death(
    mut commands: Commands,
    mut player: Query<&mut Health, With<Player>>,
    enemies: Query<Entity, With<Enemy>>,
) {
    let Ok(mut health) = player.single_mut() else {
        return;
    };
    if !health.is_dead() {
        return;
    }
    info!("player died, starting over");
    health.current = health.max;
    for enemy in &enemies {
        // may already be despawned for dying in the same frame
        commands.entity(enemy).try_despawn();
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                attack::<Player, Enemy>,
                attack::<Enemy, Player>,
                despawn_dead_enemies,
                restart_on_player_death,
            )
                .chain()
                .in_set(perf::Step::Combat),
        );
    }
}
