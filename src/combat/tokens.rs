//! Attack tokens (see `docs/schwarm.md`): units that use them only start an attack on the player
//! while they hold one, and only [`AttackTokens::max`] of them exist. That caps the damage per
//! second no matter how many enemies are around. A token is a component of its holder, so it is
//! given back when the holder is despawned or the component removed. Who gets a free token and
//! when it is given back is up to the units' behavior, e.g. `enemy::swarm`; whether a unit
//! attacks is up to [`super::Striking`].

use bevy::prelude::*;

/// How many [`AttackToken`]s exist for the player, set from the swarm's values.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct AttackTokens {
    pub max: usize,
}

impl AttackTokens {
    /// How many tokens are free while `held` are held.
    pub fn free(&self, held: usize) -> usize {
        self.max.saturating_sub(held)
    }
}

/// Permission to start an attack on the player.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct AttackToken;
