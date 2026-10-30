//! How items on the ground look: a square in the placeholder color of their type, drawn with
//! [`SightMaterial`] like the enemies, so only their parts in the player's sight are shown.
//! Hovering one draws it lighter. Separate from the item logic, since it needs rendering.

use super::{HeldBy, Item, ItemDefinitions, ItemId, SIZE};
use crate::sight::render::{SightMask, SightMaterial};
use bevy::{
    color::Luminance,
    picking::{
        events::{PointerOut, PointerOver},
        mesh_picking::{MeshPickingPlugin, MeshPickingSettings},
    },
    platform::collections::HashMap,
    prelude::*,
};

/// How much lighter a hovered item is drawn.
const HIGHLIGHT: f32 = 0.2;

/// The square all items share, scaled to [`SIZE`], and a normal and a highlighted material per
/// item type, created when the type first comes to the ground.
#[derive(Resource)]
struct ItemAssets {
    square: Handle<Mesh>,
    materials: HashMap<ItemId, [Handle<SightMaterial>; 2]>,
}

impl ItemAssets {
    fn material(
        &mut self,
        id: &ItemId,
        highlighted: bool,
        definitions: &ItemDefinitions,
        materials: &mut Assets<SightMaterial>,
        mask: &SightMask,
    ) -> Handle<SightMaterial> {
        let [normal, lighter] = self.materials.entry(id.clone()).or_insert_with(|| {
            let color = definitions.color(id);
            [
                materials.add(SightMaterial::new(color, mask)),
                materials.add(SightMaterial::new(color.lighter(HIGHLIGHT), mask)),
            ]
        });
        if highlighted { lighter } else { normal }.clone()
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
        let material = assets.material(id, false, &definitions, &mut materials, &mask);
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
    for (id, [normal, lighter]) in &assets.materials {
        let color = definitions.color(id);
        if let Some(mut material) = materials.get_mut(normal) {
            material.color = color.into();
        }
        if let Some(mut material) = materials.get_mut(lighter) {
            material.color = color.lighter(HIGHLIGHT).into();
        }
    }
}

/// Draws a hovered item lighter, or normal again once the pointer left it.
fn highlight(
    entity: Entity,
    highlighted: bool,
    items: &mut Query<(&Item, &mut MeshMaterial2d<SightMaterial>), Without<HeldBy>>,
    assets: &mut ItemAssets,
    definitions: &ItemDefinitions,
    materials: &mut Assets<SightMaterial>,
    mask: &SightMask,
) {
    if let Ok((Item(id), mut material)) = items.get_mut(entity) {
        material.0 = assets.material(id, highlighted, definitions, materials, mask);
    }
}

fn highlight_on_over(
    over: On<PointerOver>,
    mut items: Query<(&Item, &mut MeshMaterial2d<SightMaterial>), Without<HeldBy>>,
    mut assets: ResMut<ItemAssets>,
    definitions: Res<ItemDefinitions>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mask: Res<SightMask>,
) {
    let (definitions, mask) = (&*definitions, &*mask);
    highlight(
        over.entity,
        true,
        &mut items,
        &mut assets,
        definitions,
        &mut materials,
        mask,
    );
}

fn unhighlight_on_out(
    out: On<PointerOut>,
    mut items: Query<(&Item, &mut MeshMaterial2d<SightMaterial>), Without<HeldBy>>,
    mut assets: ResMut<ItemAssets>,
    definitions: Res<ItemDefinitions>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mask: Res<SightMask>,
) {
    let (definitions, mask) = (&*definitions, &*mask);
    highlight(
        out.entity,
        false,
        &mut items,
        &mut assets,
        definitions,
        &mut materials,
        mask,
    );
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
            .add_systems(Update, (show_on_ground, update_colors))
            .add_observer(highlight_on_over)
            .add_observer(unhighlight_on_out);
    }
}
