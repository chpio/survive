//! Health bars above units that have taken damage. Separate from the combat logic, since it needs
//! rendering. Enemies only show theirs while they are in the player's sight.

use super::Health;
use crate::{objects::enemy::Enemy, sight::Sight};
use avian2d::prelude::*;
use bevy::{prelude::*, sprite::Anchor};

/// Height of a bar and its gap to the unit's collider, in px.
const HEIGHT: f32 = 2.;
const GAP: f32 = 3.;

/// Background of a bar, a child of the unit. The unit must not rotate, or the bar turns with it.
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
            ChildOf(add.entity),
        ))
        .add_child(fill);
}

/// Sets the fill of the bars whose unit's health changed.
fn update_fills(
    units: Query<(&Health, &Children), Changed<Health>>,
    bars: Query<&HealthBar>,
    mut fills: Query<&mut Sprite, Without<HealthBar>>,
) {
    for (health, children) in &units {
        for bar in children.iter().filter_map(|child| bars.get(child).ok()) {
            if let Ok(mut fill) = fills.get_mut(bar.fill) {
                let fraction = (health.current / health.max).clamp(0., 1.);
                fill.custom_size = Some(Vec2::new(bar.width * fraction, HEIGHT));
            }
        }
    }
}

/// Shows the bars of damaged units, of enemies only while the middle of the enemy is in sight
/// (the bars are sprites, which can't be masked like the enemies' bodies).
fn show_bars(
    units: Query<(&Health, &Transform, Has<Enemy>, &Children)>,
    mut bars: Query<&mut Visibility, With<HealthBar>>,
    sight: Option<Res<Sight>>,
) {
    for (health, transform, enemy, children) in &units {
        let in_sight = !enemy
            || sight
                .as_ref()
                .is_none_or(|sight| sight.visibility(transform.translation.xy()) >= 0.5);
        let shown = health.current < health.max && in_sight;
        for &child in children {
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
