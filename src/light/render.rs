//! Draws the light: a dark layer above everything (the map, items, units and health bars, but not
//! the HUD) with its own [`LightMaterial`] (shader `assets/shaders/light.wesl`). It darkens by
//! [`DayNight::darkness`] and the sun's shadows, from the [`SunShadow`] in a linearly filtered
//! texture, and is lifted where the [`Lightmap`] is lit. The darkening outside the sight lies below
//! it; the shadows are only drawn within the sight, so the two don't darken the same place. The
//! plugin also grades the camera's colors ([`super::grading`]).

use super::{DayNight, LightValues, grading, running, shadow::SunShadow, shadows_on};
use crate::{
    settings::Settings,
    sight::render::{SightMask, mask_area, mask_image},
};
use bevy::{
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};

const SHADER: &str = "shaders/light.wesl";

/// Above the units (z = 10) and their health bars.
const LIGHT_Z: f32 = 20.;

/// Edge length of the layer in px, enough to cover the view at any zoom.
const LIGHT_SIZE: f32 = 8192.;

/// How much light there is around the view, from 0 to 1 per pixel. There are no lights yet, so it
/// is a single dark pixel; outside of its area, there is no light anyway.
#[derive(Resource)]
pub struct Lightmap {
    pub image: Handle<Image>,
}

impl FromWorld for Lightmap {
    fn from_world(world: &mut World) -> Self {
        let image = mask_image(UVec2::ONE, &[0]);
        Self {
            image: world.resource_mut::<Assets<Image>>().add(image),
        }
    }
}

/// The night and the sun's shadows, see the module and its shader.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct LightMaterial {
    /// See [`DayNight::darkness`]
    #[uniform(0)]
    darkness: f32,
    /// See [`DayNight::shadow`]
    #[uniform(0)]
    shadow: f32,
    /// Bottom-left corner (xy) and size (zw) of the shadow mask in world space
    #[uniform(0)]
    shadow_area: Vec4,
    /// Bottom-left corner (xy) and size (zw) of the lightmap in world space
    #[uniform(0)]
    lightmap_area: Vec4,
    /// See [`SightMask::area`]
    #[uniform(0)]
    sight_area: Vec4,
    /// See [`SunShadow`], one pixel per cell
    #[texture(1)]
    #[sampler(2)]
    shadow_mask: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    lightmap: Handle<Image>,
    /// Where the shadows are drawn, see [`SightMask`]
    #[texture(5)]
    #[sampler(6)]
    sight_mask: Handle<Image>,
}

impl Material2d for LightMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Component)]
struct LightLayer;

fn spawn_layer(
    mut commands: Commands,
    lightmap: Res<Lightmap>,
    sight_mask: Res<SightMask>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<LightMaterial>>,
) {
    commands.spawn((
        LightLayer,
        Name::new("Light"),
        Mesh2d(meshes.add(Rectangle::from_length(LIGHT_SIZE))),
        MeshMaterial2d(materials.add(LightMaterial {
            darkness: 0.,
            shadow: 0.,
            // no shadows until the mask is written
            shadow_area: mask_area(Rect::new(0., 0., 1., 1.)),
            shadow_mask: images.add(mask_image(UVec2::ONE, &[0])),
            lightmap_area: mask_area(Rect::new(0., 0., 1., 1.)),
            lightmap: lightmap.image.clone(),
            sight_area: sight_mask.area(),
            sight_mask: sight_mask.image().clone(),
        })),
        Transform::from_xyz(0., 0., LIGHT_Z),
        // shown once there is a night or shadows to draw
        Visibility::Hidden,
    ));
}

/// Passes the night and the sun to the layer (without shadows if they are turned off), and hides it
/// while it would draw nothing.
fn update_light(
    day_night: Res<DayNight>,
    values: Res<LightValues>,
    settings: Option<Res<Settings>>,
    layer: Single<(&MeshMaterial2d<LightMaterial>, &mut Visibility), With<LightLayer>>,
    mut materials: ResMut<Assets<LightMaterial>>,
) {
    if !day_night.is_changed()
        && !values.is_changed()
        && !settings.as_ref().is_some_and(Res::is_changed)
    {
        return;
    }
    let (material, mut visibility) = layer.into_inner();
    let darkness = day_night.darkness(&values);
    let shadow = if shadows_on(settings) {
        day_night.shadow(&values)
    } else {
        0.
    };
    visibility.set_if_neq(if darkness > 0. || shadow > 0. {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if let Some(mut material) = materials.get_mut(material)
        && (material.darkness, material.shadow) != (darkness, shadow)
    {
        material.darkness = darkness;
        material.shadow = shadow;
    }
}

/// Writes the shadow mask into its texture whenever it was computed again.
fn write_shadow(
    shadow: Res<SunShadow>,
    layer: Single<&MeshMaterial2d<LightMaterial>, With<LightLayer>>,
    mut materials: ResMut<Assets<LightMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !shadow.is_changed() {
        return;
    }
    let Some(mask) = shadow.mask().filter(|mask| !mask.rect().is_empty()) else {
        return;
    };
    let Some(mut material) = materials.get_mut(*layer) else {
        return;
    };
    if let Some(mut image) = images.get_mut(&material.shadow_mask) {
        *image = mask_image(mask.rect().size.as_uvec2(), mask.values());
    }
    material.shadow_area = mask_area(mask.rect().world_rect());
}

/// Passes the sight mask's area to the layer whenever it moved.
fn write_sight_area(
    sight_mask: Res<SightMask>,
    layer: Single<&MeshMaterial2d<LightMaterial>, With<LightLayer>>,
    mut materials: ResMut<Assets<LightMaterial>>,
) {
    if !sight_mask.is_changed() {
        return;
    }
    if let Some(mut material) = materials.get_mut(*layer)
        && material.sight_area != sight_mask.area()
    {
        material.sight_area = sight_mask.area();
    }
}

/// Keeps the layer around the camera, wherever the player walks.
fn follow_camera(
    camera: Single<&Transform, (With<Camera2d>, Without<LightLayer>)>,
    mut layer: Single<&mut Transform, With<LightLayer>>,
) {
    layer.translation = camera.translation.xy().extend(LIGHT_Z);
}

/// Draws the light. Needs [`super::Plugin`] and [`crate::sight::render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<LightMaterial>::default())
            .init_resource::<Lightmap>()
            .add_systems(Startup, spawn_layer)
            .add_systems(
                Update,
                (
                    (update_light, grading::grade_colors).run_if(running),
                    write_shadow,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    // after the sight wrote its mask in `Update`
                    write_sight_area,
                    follow_camera.before(TransformSystems::Propagate),
                ),
            );
    }
}
