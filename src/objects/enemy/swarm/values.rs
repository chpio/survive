//! The swarm's values from `assets/swarm.ron`, hot-reloaded like the other assets and copied into
//! [`SwarmValues`] and [`AttackTokens`] whenever the file changes, so everything else (and the
//! tests) only deal with the resources.

use super::SwarmValues;
use crate::{
    combat::tokens::AttackTokens,
    hot_reload::{self, RonAsset, RonLoader},
};
use bevy::prelude::*;

impl RonAsset for SwarmValues {
    const EXTENSION: &'static str = "swarm.ron";
}

#[derive(Resource)]
struct ValuesHandle(Handle<SwarmValues>);

fn load(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ValuesHandle(asset_server.load("swarm.ron")));
}

fn update_values(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<SwarmValues>>,
    handle: Res<ValuesHandle>,
    files: Res<Assets<SwarmValues>>,
) {
    let handle = &handle.0;
    if !hot_reload::loaded_or_modified(&mut events, handle) {
        return;
    }
    let Some(values) = files.get(handle) else {
        return;
    };
    commands.insert_resource(AttackTokens {
        max: values.max_tokens,
    });
    commands.insert_resource(values.clone());
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<SwarmValues>()
            .init_asset_loader::<RonLoader<SwarmValues>>()
            .add_systems(Startup, load)
            .add_systems(Update, update_values);
    }
}
