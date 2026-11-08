//! How items on the ground look: a square in the placeholder color of their type, drawn with
//! [`SightMaterial`] like the enemies, so only their parts in the player's sight are shown.
//! Hovering one outlines it (see [`crate::interact`]). Separate from the item logic, since it
//! needs rendering.

use super::{HeldBy, Item, ItemDefinitions, ItemId, SIZE};
use crate::sight::render::{SightMask, SightMaterial};
use bevy::{
    picking::mesh_picking::{MeshPickingPlugin, MeshPickingSettings},
    platform::collections::HashMap,
    prelude::*,
};

/// The square all items share, scaled to [`SIZE`], and a material per item type, created when the
/// type first comes to the ground.
#[derive(Resource)]
struct ItemAssets {
    square: Handle<Mesh>,
    materials: HashMap<ItemId, Handle<SightMaterial>>,
}

impl ItemAssets {
    fn material(
        &mut self,
        id: &ItemId,
        definitions: &ItemDefinitions,
        materials: &mut Assets<SightMaterial>,
        mask: &SightMask,
    ) -> Handle<SightMaterial> {
        self.materials
            .entry(id.clone())
            .or_insert_with(|| materials.add(SightMaterial::new(definitions.color(id), mask)))
            .clone()
    }
}

fn create_assets(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(ItemAssets {
        square: meshes.add(Rectangle::from_length(SIZE)),
        materials: default(),
    });
}

/// Gives items that came to the ground their square. Held items keep it, hidden.
fn show_on_ground(
    mut commands: Commands,
    items: Query<(Entity, &Item), (Without<HeldBy>, Without<Mesh2d>)>,
    mut assets: ResMut<ItemAssets>,
    definitions: Res<ItemDefinitions>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mask: Res<SightMask>,
) {
    for (item, Item(id)) in &items {
        let material = assets.material(id, &definitions, &mut materials, &mask);
        commands
            .entity(item)
            .insert((Mesh2d(assets.square.clone()), MeshMaterial2d(material)));
    }
}

/// Takes changed placeholder colors (`items.ron` is hot-reloaded) into the materials.
fn update_colors(
    assets: Res<ItemAssets>,
    definitions: Res<ItemDefinitions>,
    mut materials: ResMut<Assets<SightMaterial>>,
) {
    if !definitions.is_changed() {
        return;
    }
    for (id, material) in &assets.materials {
        if let Some(mut material) = materials.get_mut(material) {
            material.color = definitions.color(id).into();
        }
    }
}

/// Draws the items on the ground and lets the pointer pick them. Mesh picking only considers
/// entities with `Pickable` (only items on the ground have it) and cameras with
/// `MeshPickingCamera`, so enemies and the player don't block the items below them. Needs
/// [`crate::sight::render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MeshPickingPlugin)
            .insert_resource(MeshPickingSettings {
                require_markers: true,
                ..default()
            })
            .add_systems(Startup, create_assets)
            .add_systems(Update, (show_on_ground, update_colors));
    }
}
