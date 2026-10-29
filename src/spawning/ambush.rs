//! Ambushes placed in Tiled (see "Hinterhalte" in `docs/spawnen.md`).
//!
//! An ambush is an object layer with trigger zones (objects of type `ambush_trigger`) and spawn
//! points (points of type `ambush_spawn`). Each spawn point sets its enemies with the properties
//! `radius` (px, 5 by default), `count` (1) and `delay` (s after the trigger, 0), and the layer
//! may limit how many spawn points are used with `spawn_points`. When the player enters one of the
//! zones, the ambush is triggered once: all its zones get [`Triggered`]. Once a spawn point's
//! delay is over, it is used if the player sees no part of its enemies there and their walking
//! distance to the player is at least [`MIN_DISTANCE`], the closest ones first. Otherwise it is
//! skipped. So the same ambush plays out differently depending on where the player comes from.

use crate::{
    flowfield::{self, FlowField},
    objects::{
        enemy::{Enemy, SpawnEnemy},
        player::Player,
    },
    sight::Sight,
};
use avian2d::prelude::*;
use bevy::{platform::collections::HashSet, prelude::*};
use bevy_ecs_tiled::prelude::*;
use std::time::Duration;

/// Shortest walking distance in px from the player of a used spawn point. Measured along the flow
/// field, so the enemies don't spawn right behind a thin wall, and only reachable spawn points
/// are used.
pub const MIN_DISTANCE: f32 = 80.;

/// A zone that triggers an ambush when the player enters it, on a sensor collider.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct AmbushTrigger {
    /// The ambush's object layer
    pub ambush: Entity,
    /// How many spawn points are used at most, all that fit if `None`
    pub spawn_points: Option<u32>,
}

impl AmbushTrigger {
    /// Trigger zone of the ambush on the object `layer` with the Tiled `properties`.
    pub fn from_layer(layer: Entity, properties: &tiled::Properties) -> Self {
        Self {
            ambush: layer,
            spawn_points: int(properties, "spawn_points").map(|n| n.max(0) as u32),
        }
    }
}

/// Marks the zones of an ambush that was triggered, so it is only triggered once.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct Triggered;

/// A place where enemies of an ambush may spawn, at the entity's position.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct SpawnPoint {
    /// The ambush's object layer
    pub ambush: Entity,
    pub enemy: Enemy,
    pub count: u32,
    /// Time from the trigger until the point is used
    pub delay: Duration,
}

impl SpawnPoint {
    /// Spawn point of the ambush on the object `layer`, with the Tiled `properties` of the point.
    pub fn from_object(layer: Entity, properties: &tiled::Properties) -> Self {
        Self {
            ambush: layer,
            enemy: Enemy {
                radius: float(properties, "radius").unwrap_or(Enemy::default().radius),
            },
            count: int(properties, "count").map_or(1, |n| n.max(0) as u32),
            delay: Duration::from_secs_f32(float(properties, "delay").unwrap_or(0.).max(0.)),
        }
    }
}

fn int(properties: &tiled::Properties, name: &str) -> Option<i32> {
    match properties.get(name)? {
        tiled::PropertyValue::IntValue(value) => Some(*value),
        _ => None,
    }
}

/// Also takes integers, so Tiled's property type doesn't matter.
fn float(properties: &tiled::Properties, name: &str) -> Option<f32> {
    match properties.get(name)? {
        tiled::PropertyValue::FloatValue(value) => Some(*value),
        tiled::PropertyValue::IntValue(value) => Some(*value as f32),
        _ => None,
    }
}

/// A triggered ambush whose spawn points still wait for their delay or their grid. Requests the
/// grids for their radii ([`flowfield::Request`]), so the walking distances exist.
#[derive(Component, Debug)]
pub struct Pending {
    /// Spawn points not used or skipped yet, with their position
    points: Vec<(Vec2, SpawnPoint)>,
    /// How many more spawn points may be used, unlimited if `None`
    remaining: Option<u32>,
    /// Time since the trigger
    elapsed: Duration,
}

/// Triggers the ambushes with a zone the player touches. Asks the contact graph rather than
/// reading `CollisionStart`, since a zone only triggers once anyway, and Avian only reads
/// `CollisionEventsEnabled` when a collider is added, before the observer for Tiled's colliders
/// could insert it.
pub fn trigger(
    collisions: Collisions,
    player: Single<Entity, With<Player>>,
    zones: Query<(Entity, &AmbushTrigger), Without<Triggered>>,
    points: Query<(&GlobalTransform, &SpawnPoint)>,
    mut commands: Commands,
) {
    // zones are marked by commands, so two zones of one ambush touched at once still count once
    let mut triggered = HashSet::new();
    for (entity, &zone) in &zones {
        if triggered.contains(&zone.ambush)
            || !collisions
                .get(entity, *player)
                .is_some_and(ContactPair::is_touching)
        {
            continue;
        }
        triggered.insert(zone.ambush);

        for (entity, other) in &zones {
            if other.ambush == zone.ambush {
                commands.entity(entity).insert(Triggered);
            }
        }
        let points: Vec<_> = points
            .iter()
            .filter(|(_, point)| point.ambush == zone.ambush)
            .map(|(transform, &point)| (transform.translation().xy(), point))
            .collect();
        let mut radii: Vec<_> = points.iter().map(|(_, point)| point.enemy.radius).collect();
        radii.sort_by(f32::total_cmp);
        radii.dedup();
        commands.spawn((
            Name::new("Pending ambush"),
            Pending {
                points,
                remaining: zone.spawn_points,
                elapsed: Duration::ZERO,
            },
            flowfield::Request(radii),
        ));
    }
}

/// Uses the spawn points of triggered ambushes whose delay is over and whose grid is computed.
pub fn spawn(
    time: Res<Time>,
    flow_field: Res<FlowField>,
    sight: Res<Sight>,
    mut ambushes: Query<(Entity, &mut Pending)>,
    mut spawn_enemy: MessageWriter<SpawnEnemy>,
    mut commands: Commands,
) {
    for (entity, mut ambush) in &mut ambushes {
        ambush.elapsed += time.delta();
        let elapsed = ambush.elapsed;
        let (due, waiting): (Vec<_>, Vec<_>) = ambush.points.drain(..).partition(|(_, point)| {
            point.delay <= elapsed
                && flow_field
                    .grid(point.enemy.radius)
                    .is_some_and(flowfield::Grid::is_computed)
        });
        ambush.points = waiting;

        let mut fitting: Vec<_> = due
            .into_iter()
            .filter_map(|(position, point)| {
                let radius = point.enemy.radius;
                let distance = flow_field.walking_distance(position, radius)?;
                (distance >= MIN_DISTANCE && !sight.sees(position, radius))
                    .then_some((distance, position, point))
            })
            .collect();
        fitting.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, position, point) in fitting {
            if let Some(remaining) = &mut ambush.remaining {
                let Some(left) = remaining.checked_sub(1) else {
                    break;
                };
                *remaining = left;
            }
            for _ in 0..point.count {
                spawn_enemy.write(SpawnEnemy {
                    enemy: point.enemy,
                    position,
                    asleep: false,
                });
            }
        }

        if ambush.points.is_empty() {
            commands.entity(entity).despawn();
        }
    }
}
