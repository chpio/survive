//! The values of the day-night cycle from `assets/light.ron`. The file is hot-reloaded like the
//! other assets and copied into [`LightValues`] whenever it changes, so everything else (and the
//! tests) only deal with the resource. The first time, it also starts the clock ([`DayNight`]);
//! later changes set the clock's day length again, which ends a time-lapse.

use super::{DayNight, LightValues};
use crate::hot_reload::{self, RonAsset, RonLoader};
use bevy::prelude::*;

impl RonAsset for LightValues {
    const EXTENSION: &'static str = "light.ron";
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
    if !hot_reload::loaded_or_modified(&mut events, handle) {
        return;
    }
    let Some(values) = files.get(handle) else {
        return;
    };
    match day_night {
        Some(mut day_night) => {
            day_night.day_length = values.day_length;
            day_night.derive(values);
        }
        None => commands.insert_resource(DayNight::new(values.start, values)),
    }
    commands.insert_resource(values.clone());
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<LightValues>()
            .init_asset_loader::<RonLoader<LightValues>>()
            .add_systems(Startup, load)
            .add_systems(Update, update_values);
    }
}
