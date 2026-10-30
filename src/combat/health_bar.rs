//! Health bars above units that have taken damage. Separate from the combat logic, since it needs
//! rendering. Like everything but the map, they are only shown while the middle of their unit is in
//! the player's sight.

use super::Health;
use crate::sight::render::HiddenOutsideSight;
use avian2d::prelude::*;
use bevy::{prelude::*, sprite::Anchor};

/// Height of a bar and its gap to the unit's collider, in px.
const HEIGHT: f32 = 2.;
const GAP: f32 = 3.;

/// Background of a bar, a child of a holder in the middle of the unit, which hides it while the
/// unit is out of sight, so the bar's own visibility only depends on the damage. The unit must not
/// rotate, or the bar turns with it.
#[derive(Component)]
struct HealthBar {
    /// Child of the bar, as wide as the remaining health
    fill: Entity,
    width: f32,
}

fn spawn_health_bar(add: On<Add<Health>>, colliders: Query<&Collider>, mut commands: Commands) {
    let Ok(collider) = colliders.get(add.entity) else {
        return;
    };
    let aabb = collider.aabb(Vec2::ZERO, Rotation::IDENTITY, 0.);
    let size = aabb.max - aabb.min;
    let width = size.x.max(8.);

    let holder = commands
        .spawn((
            Name::new("Health bar holder"),
            HiddenOutsideSight,
            Transform::default(),
            Visibility::default(),
            ChildOf(add.entity),
        ))
        .id();
    let fill = commands
        .spawn((
            Sprite::from_color(Color::srgb(0.9, 0.2, 0.2), Vec2::new(width, HEIGHT)),
            Anchor::CENTER_LEFT,
            // from the left edge of the bar, in front of it
            Transform::from_xyz(-width / 2., 0., 0.1),
        ))
        .id();
    commands
        .spawn((
            HealthBar { fill, width },
            Name::new("Health bar"),
            Sprite::from_color(Color::srgb(0.15, 0.15, 0.15), Vec2::new(width, HEIGHT)),
            // above the unit, and in front of the units at z = 10
            Transform::from_xyz(0., size.y / 2. + GAP + HEIGHT / 2., 1.),
            Visibility::Hidden,
            ChildOf(holder),
        ))
        .add_child(fill);
}

/// Sets the fill of the bars whose unit's health changed.
fn update_fills(
    units: Query<(Entity, &Health), Changed<Health>>,
    children: Query<&Children>,
    bars: Query<&HealthBar>,
    mut fills: Query<&mut Sprite, Without<HealthBar>>,
) {
    for (unit, health) in &units {
        for bar in children
            .iter_descendants(unit)
            .filter_map(|child| bars.get(child).ok())
        {
            if let Ok(mut fill) = fills.get_mut(bar.fill) {
                let fraction = (health.current / health.max).clamp(0., 1.);
                fill.custom_size = Some(Vec2::new(bar.width * fraction, HEIGHT));
            }
        }
    }
}

/// Shows the bars of damaged units.
fn show_bars(
    units: Query<(Entity, &Health), Changed<Health>>,
    children: Query<&Children>,
    mut bars: Query<&mut Visibility, With<HealthBar>>,
) {
    for (unit, health) in &units {
        let shown = health.current < health.max;
        for child in children.iter_descendants(unit) {
            if let Ok(mut visibility) = bars.get_mut(child) {
                visibility.set_if_neq(if shown {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                });
            }
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_health_bar)
            .add_systems(PostUpdate, (update_fills, show_bars));
    }
}
