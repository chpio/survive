//! Draws the night: a dark layer above everything (the map, items, units and health bars, but not
//! the HUD), as opaque as [`DayNight::darkness`]. It is a [`SightMaterial`] like the darkening
//! outside the sight, but reads the [`Lightmap`] instead of the sight's mask and is lifted where it
//! is lit. Both layers lie on top of each other.

use super::{DayNight, LightValues};
use crate::sight::render::SightMaterial;
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// Above the units (z = 10) and their health bars.
const NIGHT_Z: f32 = 20.;

/// Edge length of the layer in px, enough to cover the view at any zoom.
const NIGHT_SIZE: f32 = 8192.;

/// How much light there is around the view, from 0 to 1 per pixel. There are no lights yet, so it
/// is a single dark pixel; outside of its area, the layer darkens anyway.
#[derive(Resource)]
pub struct Lightmap {
    pub image: Handle<Image>,
}

impl FromWorld for Lightmap {
    fn from_world(world: &mut World) -> Self {
        let mut image = Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0],
            TextureFormat::R8Unorm,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::linear();
        Self {
            image: world.resource_mut::<Assets<Image>>().add(image),
        }
    }
}

#[derive(Component)]
struct Night;

fn spawn_night(
    mut commands: Commands,
    lightmap: Res<Lightmap>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SightMaterial>>,
) {
    commands.spawn((
        Night,
        Name::new("Night"),
        Mesh2d(meshes.add(Rectangle::from_length(NIGHT_SIZE))),
        MeshMaterial2d(materials.add(SightMaterial::inverted_other(
            Color::BLACK.with_alpha(0.),
            lightmap.image.clone(),
            Rect::new(0., 0., 1., 1.),
        ))),
        Transform::from_xyz(0., 0., NIGHT_Z),
        // shown once it gets dark
        Visibility::Hidden,
    ));
}

/// Makes the layer as opaque as the night is dark, and hides it by day, so it isn't drawn at all.
fn darken(
    day_night: Res<DayNight>,
    values: Res<LightValues>,
    night: Single<(&MeshMaterial2d<SightMaterial>, &mut Visibility), With<Night>>,
    mut materials: ResMut<Assets<SightMaterial>>,
) {
    if !day_night.is_changed() && !values.is_changed() {
        return;
    }
    let (material, mut visibility) = night.into_inner();
    let darkness = day_night.darkness(&values);
    visibility.set_if_neq(if darkness > 0. {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if let Some(mut material) = materials.get_mut(material)
        && material.color.alpha != darkness
    {
        material.color.alpha = darkness;
    }
}

/// Keeps the layer around the camera, wherever the player walks.
fn follow_camera(
    camera: Single<&Transform, (With<Camera2d>, Without<Night>)>,
    mut night: Single<&mut Transform, With<Night>>,
) {
    night.translation = camera.translation.xy().extend(NIGHT_Z);
}

/// Draws the night. Needs [`super::Plugin`] and `sight::render::Plugin`.
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Lightmap>()
            .add_systems(Startup, spawn_night)
            .add_systems(
                Update,
                darken
                    .run_if(resource_exists::<LightValues>)
                    .run_if(resource_exists::<DayNight>),
            )
            .add_systems(
                PostUpdate,
                follow_camera.before(TransformSystems::Propagate),
            );
    }
}
