//! Chests placed in Tiled: a point object with the type `chest`. Clicking a closed chest within
//! reach opens it: its lid swings open and its content falls out in front of it. Whether a chest
//! is open isn't saved; a map that is loaded again brings its chests back closed.

use crate::{
    figure::{Figure, FigureControl, FigureRoot},
    items::{self, ItemId},
    light::shadow::CastsShadow,
    objects::player::Player,
    sight::Sight,
};
use avian2d::prelude::*;
use bevy::{
    picking::{events::PointerClick, pointer::PointerButton},
    prelude::*,
};
use bevy_ecs_tiled::prelude::tiled;

/// Distance in px from the player's center to the chest's center up to which it opens.
pub const REACH: f32 = 24.;
/// Footprint of the chest facing the viewer, in px (the figure in `art/chest.py`).
pub const SIZE: Vec2 = Vec2::new(12., 8.);
/// Height in px, for its shadow.
const HEIGHT: f32 = 8.5;
/// z of the figure's holder: that of the units, so the chest sorts with them.
const UNIT_Z: f32 = 10.;

/// A chest, on the Tiled object's entity.
#[derive(Component, Clone, Debug, Default)]
pub struct Chest {
    /// Items and their counts, from the property `content`
    pub content: Vec<(ItemId, u32)>,
    pub open: bool,
}

/// Reads the property `content`: item ids separated by commas, each with an optional count
/// after `*`, e.g. `apple*3, healing_potion`. Parts that don't parse are skipped.
pub fn parse_content(text: &str) -> Vec<(ItemId, u32)> {
    text.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter_map(|part| match part.split_once('*') {
            Some((id, count)) => Some((ItemId::from(id.trim()), count.trim().parse().ok()?)),
            None => Some((ItemId::from(part), 1)),
        })
        .collect()
}

/// Holds the figure at the units' z, whatever the z of the map layer the chest is on.
#[derive(Component)]
struct Holder;

/// The figure of the chest, child of its [`Holder`].
#[derive(Component)]
struct ChestFigure;

#[derive(Resource)]
struct ChestAsset(Handle<Figure>);

/// Makes the Tiled object `entity` a chest.
pub fn insert(commands: &mut Commands, entity: Entity, properties: &tiled::Properties) {
    let content = match properties.get("content") {
        Some(tiled::PropertyValue::StringValue(text)) => parse_content(text),
        _ => Vec::new(),
    };
    commands.entity(entity).insert((
        Chest {
            content,
            open: false,
        },
        Name::new("Chest"),
        RigidBody::Static,
        Collider::rectangle(SIZE.x, SIZE.y),
        CastsShadow(HEIGHT),
        Visibility::default(),
    ));
}

/// Gives new chests their figure, closed: the animation `open` held at its start.
fn add_figure(mut commands: Commands, chests: Query<Entity, Added<Chest>>, asset: Res<ChestAsset>) {
    for chest in &chests {
        commands.entity(chest).with_child((
            Holder,
            Transform::default(),
            Visibility::default(),
            children![(
                ChestFigure,
                FigureRoot(asset.0.clone()),
                FigureControl {
                    animation: "open",
                    speed: 0.,
                    repeat: false,
                    ..default()
                },
                Pickable::default(),
                Transform::default(),
                Visibility::default(),
            )],
        ));
    }
}

/// Moves each holder to the units' z, against the z of the chest's map layer.
fn hold_at_unit_z(
    mut holders: Query<(&mut Transform, &ChildOf), With<Holder>>,
    parents: Query<&GlobalTransform>,
) {
    for (mut transform, child_of) in &mut holders {
        let Ok(parent) = parents.get(child_of.parent()) else {
            continue;
        };
        let z = UNIT_Z - parent.translation().z;
        if transform.translation.z != z {
            transform.translation.z = z;
        }
    }
}

fn open_on_click(
    click: On<PointerClick>,
    mut commands: Commands,
    mut chests: Query<(&mut Chest, &GlobalTransform)>,
    mut figures: Query<(&mut FigureControl, &ChildOf), With<ChestFigure>>,
    holders: Query<&ChildOf, With<Holder>>,
    player: Single<&Transform, With<Player>>,
    sight: Option<Res<Sight>>,
) {
    if click.button != PointerButton::Primary {
        return;
    }
    // the click bubbles up from the parts; act once, on the figure
    let Ok((mut control, holder)) = figures.get_mut(click.entity) else {
        return;
    };
    let Ok(chest_entity) = holders.get(holder.parent()).map(ChildOf::parent) else {
        return;
    };
    let Ok((mut chest, transform)) = chests.get_mut(chest_entity) else {
        return;
    };
    let position = transform.translation().xy();
    // the material hides a chest out of sight, but picking would still hit it
    let seen = sight.is_none_or(|sight| sight.sees(position, SIZE.y / 2.));
    if chest.open || !seen || position.distance(player.translation.xy()) > REACH {
        return;
    }
    chest.open = true;
    control.speed = 1.;
    for (i, (id, count)) in chest.content.iter().enumerate() {
        // in a row in front of the chest
        let offset = (i as f32 - (chest.content.len() - 1) as f32 / 2.) * items::SIZE * 1.5;
        let at = position + Vec2::new(offset, -SIZE.y / 2. - items::SIZE);
        items::spawn_on_ground(&mut commands, id, *count, at);
    }
}

fn load(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ChestAsset(asset_server.load("characters/chest.glb")));
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load)
            .add_systems(Update, (add_figure, hold_at_unit_z))
            .add_observer(open_on_click);
    }
}
