//! The values of the day-night cycle from `assets/light.ron`. The file is hot-reloaded like the
//! other assets and copied into [`LightValues`] whenever it changes, so everything else (and the
//! tests) only deal with the resource. The first time, it also starts the clock ([`DayNight`]).

use super::{DayNight, LightValues};
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};

#[derive(TypePath, Default)]
struct ValuesLoader;

impl AssetLoader for ValuesLoader {
    type Asset = LightValues;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<LightValues, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &["light.ron"]
    }
}

#[derive(Resource)]
struct ValuesHandle(Handle<LightValues>);

fn load(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ValuesHandle(asset_server.load("light.ron")));
}

fn update_values(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<LightValues>>,
    handle: Res<ValuesHandle>,
    files: Res<Assets<LightValues>>,
    day_night: Option<ResMut<DayNight>>,
) {
    let handle = &handle.0;
    if !events
        .read()
        .any(|event| event.is_loaded_with_dependencies(handle) || event.is_modified(handle))
    {
        return;
    }
    let Some(values) = files.get(handle) else {
        return;
    };
    match day_night {
        Some(mut day_night) => day_night.derive(values),
        None => commands.insert_resource(DayNight::new(values.start, values)),
    }
    commands.insert_resource(values.clone());
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LightValues>()
            .init_asset_loader::<ValuesLoader>()
            .add_systems(Startup, load)
            .add_systems(Update, update_values);
    }
}
