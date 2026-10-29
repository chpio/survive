//! Places out of the player's sight, for spawners without fixed spawn points (see "Ort außer Sicht"
//! in `docs/spawnen.md`).
//!
//! The walking distances only exist for the grids that are in use, so a spawner first requests
//! the grid for the enemy's radius ([`flowfield::Request`]) and spawns once it is computed, a
//! search later.

use crate::{
    flowfield::{self, FlowField},
    objects::{
        enemy::{Enemy, SpawnEnemy},
        player::Player,
    },
    sight::Sight,
};
use bevy::prelude::*;

/// Shortest walking distance in px from the player an enemy spawns at. Measured along the flow
/// field rather than as the crow flies, so enemies don't spawn right behind a thin wall.
pub const MIN_DISTANCE: f32 = 120.;

/// Longest walking distance in px from the player an enemy spawns at.
pub const MAX_DISTANCE: f32 = 250.;

/// Place for an enemy with `radius` where the player sees no part of it, with a walking distance
/// to the `player` between [`MIN_DISTANCE`] and [`MAX_DISTANCE`]. Of those, the one closest to the
/// player as the crow flies, so places behind obstacles win over those in the open just outside
/// the sight. `None` if the grid for the radius isn't computed or no place fits.
pub fn find_place(
    flow_field: &FlowField,
    sight: &Sight,
    player: Vec2,
    radius: f32,
) -> Option<Vec2> {
    flow_field
        .grid(radius)?
        .reachable()
        .filter(|(_, distance)| (MIN_DISTANCE..=MAX_DISTANCE).contains(distance))
        .filter(|&(position, _)| !sight.sees(position, radius))
        .map(|(position, _)| position)
        .min_by(|a, b| {
            a.distance_squared(player)
                .total_cmp(&b.distance_squared(player))
        })
}

/// Spawns an awake enemy out of the player's sight, see [`find_place`].
#[derive(Message, Clone, Copy, Debug)]
pub struct SpawnOutOfSight(pub Enemy);

/// An enemy waiting for the grid of its radius to find its place.
#[derive(Component)]
pub struct Pending(Enemy);

/// Requests the grid for each enemy to spawn.
pub fn request(mut messages: MessageReader<SpawnOutOfSight>, mut commands: Commands) {
    for &SpawnOutOfSight(enemy) in messages.read() {
        commands.spawn((
            Name::new("Pending enemy"),
            Pending(enemy),
            flowfield::Request(vec![enemy.radius]),
        ));
    }
}

/// Spawns the waiting enemies whose grid is computed. Those without a place are dropped.
pub fn spawn(
    pending: Query<(Entity, &Pending)>,
    flow_field: Res<FlowField>,
    sight: Res<Sight>,
    player: Single<&Transform, With<Player>>,
    mut spawn_enemy: MessageWriter<SpawnEnemy>,
    mut commands: Commands,
) {
    for (entity, &Pending(enemy)) in &pending {
        if !flow_field
            .grid(enemy.radius)
            .is_some_and(flowfield::Grid::is_computed)
        {
            continue;
        }
        commands.entity(entity).despawn();
        match find_place(&flow_field, &sight, player.translation.xy(), enemy.radius) {
            Some(position) => {
                spawn_enemy.write(SpawnEnemy {
                    enemy,
                    position,
                    asleep: false,
                });
            }
            None => warn!(
                "no place out of sight for an enemy of radius {}",
                enemy.radius
            ),
        }
    }
}
