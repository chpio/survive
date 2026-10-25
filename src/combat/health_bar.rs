//! Health bars above units that have taken damage. Separate from the combat logic, since it needs
//! rendering.

use super::Health;
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

/// Shows the bars of damaged units and sets their fill.
fn update_health_bars(
    units: Query<(&Health, &Children), Changed<Health>>,
    mut bars: Query<(&HealthBar, &mut Visibility)>,
    mut fills: Query<&mut Sprite, Without<HealthBar>>,
) {
    for (health, children) in &units {
        for &child in children {
            let Ok((bar, mut visibility)) = bars.get_mut(child) else {
                continue;
            };
            visibility.set_if_neq(if health.current < health.max {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            });
            if let Ok(mut fill) = fills.get_mut(bar.fill) {
                let fraction = (health.current / health.max).clamp(0., 1.);
                fill.custom_size = Some(Vec2::new(bar.width * fraction, HEIGHT));
            }
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_health_bar)
            .add_systems(PostUpdate, update_health_bars);
    }
}
