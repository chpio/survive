//! Sources of enemies besides the debug keys (see `docs/spawnen.md`). They decide what spawns
//! where and send [`SpawnEnemy`] messages, which [`crate::objects::enemy`] turns into entities.

use crate::{objects::enemy::SpawnEnemy, perf};
use bevy::prelude::*;

pub mod ambush;
pub mod out_of_sight;

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SpawnEnemy>()
            .add_message::<out_of_sight::SpawnOutOfSight>()
            .add_systems(
                Update,
                (
                    ambush::trigger,
                    ambush::spawn,
                    out_of_sight::request,
                    out_of_sight::spawn,
                )
                    .in_set(perf::Step::Spawning),
            );
    }
}
