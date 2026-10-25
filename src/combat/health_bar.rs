//! Health bars above units that have taken damage. Separate from the combat logic, since it needs
//! meshes and materials.

use super::Health;
use avian2d::prelude::*;
use bevy::prelude::*;

/// Height of a bar and its gap to the unit's collider, in px.
const HEIGHT: f32 = 2.;
const GAP: f32 = 3.;

/// Background of a bar, a child of the unit with the [`Fill`] as its own child. Scaled to the
/// bar's size, so the fill works in units of the bar.
#[derive(Component)]
struct HealthBar {
    /// Distance of the bar's center above the unit's center
    offset: f32,
}

#[derive(Component)]
struct Fill;

fn spawn_health_bar(
    add: On<Add<Health>>,
    colliders: Query<&Collider>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Ok(collider) = colliders.get(add.entity) else {
        return;
    };
    let aabb = collider.aabb(Vec2::ZERO, Rotation::IDENTITY, 0.);
    let size = aabb.max - aabb.min;
    let mesh = meshes.add(Rectangle::new(1., 1.));

    commands.entity(add.entity).with_child((
        HealthBar {
            offset: size.y / 2. + GAP + HEIGHT / 2.,
        },
        Name::new("Health bar"),
        Mesh2d(mesh.clone()),
        MeshMaterial2d(materials.add(Color::srgb(0.15, 0.15, 0.15))),
        Transform::from_scale(Vec3::new(size.x.max(8.), HEIGHT, 1.)),
        Visibility::Hidden,
        children![(
            Fill,
            Mesh2d(mesh),
            MeshMaterial2d(materials.add(Color::srgb(0.9, 0.2, 0.2))),
            Transform::from_xyz(0., 0., 0.1),
        )],
    ));
}

/// Shows the bars of damaged units, sets their fill, and keeps them upright above the unit, which
/// may be rotated.
fn update_health_bars(
    units: Query<(&Health, &Transform), Without<HealthBar>>,
    mut bars: Query<
        (
            &HealthBar,
            &ChildOf,
            &Children,
            &mut Transform,
            &mut Visibility,
        ),
        Without<Fill>,
    >,
    mut fills: Query<&mut Transform, (With<Fill>, Without<Health>)>,
) {
    for (bar, child_of, children, mut transform, mut visibility) in &mut bars {
        let Ok((health, unit_transform)) = units.get(child_of.parent()) else {
            continue;
        };
        visibility.set_if_neq(if health.current < health.max {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });

        // above the units at z = 10
        let unrotate = unit_transform.rotation.inverse();
        transform.rotation = unrotate;
        transform.translation = unrotate * Vec3::new(0., bar.offset, 1.);

        let fraction = (health.current / health.max).clamp(0., 1.);
        for &child in children {
            if let Ok(mut fill) = fills.get_mut(child) {
                fill.scale.x = fraction;
                // left-aligned in the bar, which is 1 wide in its own units
                fill.translation.x = (fraction - 1.) / 2.;
            }
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_observer(spawn_health_bar).add_systems(
            PostUpdate,
            update_health_bars.before(TransformSystems::Propagate),
        );
    }
}
