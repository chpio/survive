//! Assets the game copies into its own state when they are loaded and again after each change to
//! their file, like `assets/items.ron`, `assets/light.ron` and the HUD's stylesheet.

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use serde::de::DeserializeOwned;
use std::marker::PhantomData;

/// Whether the asset of `handle` was loaded or changed since the last read of `events`. Reads all
/// of them.
pub fn loaded_or_modified<A: Asset>(
    events: &mut MessageReader<AssetEvent<A>>,
    handle: &Handle<A>,
) -> bool {
    events
        .read()
        .any(|event| event.is_loaded_with_dependencies(handle) || event.is_modified(handle))
}

/// An asset read from a RON file with the extension [`Self::EXTENSION`] by [`RonLoader`].
pub trait RonAsset: Asset + DeserializeOwned {
    /// Without the dot, e.g. `"items.ron"`, so each type gets its own loader
    const EXTENSION: &'static str;
}

/// Loads a [`RonAsset`], registered with `app.init_asset_loader::<RonLoader<T>>()`.
#[derive(TypePath)]
pub struct RonLoader<T: RonAsset>(PhantomData<fn() -> T>);

impl<T: RonAsset> Default for RonLoader<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: RonAsset> AssetLoader for RonLoader<T> {
    type Asset = T;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<T, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(ron::de::from_bytes(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        std::slice::from_ref(&T::EXTENSION)
    }
}
