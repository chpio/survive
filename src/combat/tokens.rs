//! Attack tokens (see `docs/schwarm.md`): units that need one only attack the player while they
//! hold one, and only [`AttackTokens::max`] of them exist. That caps the damage per second no
//! matter how many enemies are around. A token is a component of its holder, so it is given back
//! when the holder is despawned or the component removed. Who gets a free token is up to the
//! units' behavior, e.g. `enemy::swarm`.

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

/// Permission to attack the player, for a unit with [`NeedsToken`].
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct AttackToken;

/// A unit that only attacks while it holds an [`AttackToken`].
#[derive(Component, Default, Clone, Copy, PartialEq, Debug)]
pub struct NeedsToken;
