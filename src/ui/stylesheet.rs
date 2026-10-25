//! CSS files as assets, so the HUD's styles are hot-reloaded like the other assets.

use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};

#[derive(Asset, TypePath, Clone, PartialEq, Default)]
pub struct Stylesheet(pub String);

#[derive(TypePath, Default)]
pub struct StylesheetLoader;

impl AssetLoader for StylesheetLoader {
    type Asset = Stylesheet;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Stylesheet, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(Stylesheet(String::from_utf8(bytes)?))
    }

    fn extensions(&self) -> &[&str] {
        &["css"]
    }
}
