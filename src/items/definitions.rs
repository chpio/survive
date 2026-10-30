//! The item types from `assets/items.ron`. The file is hot-reloaded like the other assets and
//! copied into [`ItemDefinitions`] whenever it changes, so everything else (and the tests) only
//! deal with the resource.

use super::{ItemDefinition, ItemDefinitions, ItemId};
use crate::hot_reload::{self, RonAsset, RonLoader};
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
struct DefinitionsFile(BTreeMap<ItemId, ItemDefinition>);

impl RonAsset for DefinitionsFile {
    const EXTENSION: &'static str = "items.ron";
}

#[derive(Resource)]
struct DefinitionsHandle(Handle<DefinitionsFile>);

fn load(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(DefinitionsHandle(asset_server.load("items.ron")));
}

fn update_definitions(
    mut events: MessageReader<AssetEvent<DefinitionsFile>>,
    handle: Res<DefinitionsHandle>,
    files: Res<Assets<DefinitionsFile>>,
    mut definitions: ResMut<ItemDefinitions>,
) {
    let handle = &handle.0;
    if !hot_reload::loaded_or_modified(&mut events, handle) {
        return;
    }
    if let Some(file) = files.get(handle) {
        definitions.0 = file.0.clone();
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<DefinitionsFile>()
            .init_asset_loader::<RonLoader<DefinitionsFile>>()
            .init_resource::<ItemDefinitions>()
            .add_systems(Startup, load)
            .add_systems(Update, update_definitions);
    }
}
