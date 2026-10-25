//! The item types from `assets/items.ron`. The file is hot-reloaded like the other assets and
//! copied into [`ItemDefinitions`] whenever it changes, so everything else (and the tests) only
//! deal with the resource.

use super::{ItemDefinition, ItemDefinitions, ItemId};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Asset, TypePath, Deserialize)]
#[serde(transparent)]
struct DefinitionsFile(BTreeMap<ItemId, ItemDefinition>);

#[derive(TypePath, Default)]
struct DefinitionsLoader;

impl AssetLoader for DefinitionsLoader {
    type Asset = DefinitionsFile;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<DefinitionsFile, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["items.ron"]
    }
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
    if !events
        .read()
        .any(|event| event.is_loaded_with_dependencies(handle) || event.is_modified(handle))
    {
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
            .init_asset_loader::<DefinitionsLoader>()
            .init_resource::<ItemDefinitions>()
            .add_systems(Startup, load)
            .add_systems(Update, update_definitions);
    }
}
