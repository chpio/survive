//! Draws the light: a dark layer above everything (the map, items, units and health bars, but not
//! the HUD) with its own [`LightMaterial`] (shader `assets/shaders/light.wesl`). It darkens by
//! [`DayNight::darkness`] and the sun's shadows, which the shader finds by walking from each pixel
//! towards the sun over the [`ShadowHeights`], and is lifted where the [`Lightmap`] is lit. The
//! darkening outside the sight lies below it.

use super::{DayNight, LightValues, shadow::ShadowHeights};
use crate::cells::CELL_SIZE;
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
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
        let mut image = single_channel_image(UVec2::ONE, vec![0]);
        image.sampler = ImageSampler::linear();
        Self {
            image: world.resource_mut::<Assets<Image>>().add(image),
        }
    }
}

/// An `R8Unorm` texture of `size` with `data` row by row from the top.
fn single_channel_image(size: UVec2, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    )
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
    /// See [`super::Sun`]
    #[uniform(0)]
    sun_direction: Vec2,
    #[uniform(0)]
    sun_slope: f32,
    /// See [`ShadowHeights::max`]
    #[uniform(0)]
    max_height: f32,
    /// Bottom-left corner (xy) and size (zw) of the heights in world space
    #[uniform(0)]
    heights_area: Vec4,
    /// Bottom-left corner (xy) and size (zw) of the lightmap in world space
    #[uniform(0)]
    lightmap_area: Vec4,
    /// Height in px per cell, read without filtering
    #[texture(1)]
    heights: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    lightmap: Handle<Image>,
}

impl Material2d for LightMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

fn area(rect: Rect) -> Vec4 {
    Vec4::new(rect.min.x, rect.min.y, rect.width(), rect.height())
}

#[derive(Component)]
struct LightLayer;

fn spawn_layer(
    mut commands: Commands,
    lightmap: Res<Lightmap>,
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
            sun_direction: Vec2::X,
            sun_slope: 1.,
            max_height: 0.,
            // no shadow casters until the heights are written
            heights_area: area(Rect::new(0., 0., CELL_SIZE, CELL_SIZE)),
            heights: images.add(single_channel_image(UVec2::ONE, vec![0])),
            lightmap_area: area(Rect::new(0., 0., 1., 1.)),
            lightmap: lightmap.image.clone(),
        })),
        Transform::from_xyz(0., 0., LIGHT_Z),
        // shown once there is a night or shadows to draw
        Visibility::Hidden,
    ));
}

/// Passes the night and the sun to the layer, and hides it while it would draw nothing.
fn update_light(
    day_night: Res<DayNight>,
    values: Res<LightValues>,
    layer: Single<(&MeshMaterial2d<LightMaterial>, &mut Visibility), With<LightLayer>>,
    mut materials: ResMut<Assets<LightMaterial>>,
) {
    if !day_night.is_changed() && !values.is_changed() {
        return;
    }
    let (material, mut visibility) = layer.into_inner();
    let darkness = day_night.darkness(&values);
    let sun = day_night.sun(&values);
    let shadow = day_night.shadow(&values);
    visibility.set_if_neq(if darkness > 0. || shadow > 0. {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    if let Some(mut material) = materials.get_mut(material) {
        material.darkness = darkness;
        material.shadow = shadow;
        if let Some(sun) = sun {
            material.sun_direction = sun.direction;
            material.sun_slope = sun.slope;
        }
    }
}

/// Writes the heights into their texture whenever the loaded maps change.
fn write_heights(
    heights: Res<ShadowHeights>,
    layer: Single<&MeshMaterial2d<LightMaterial>, With<LightLayer>>,
    mut materials: ResMut<Assets<LightMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    if !heights.is_changed() {
        return;
    }
    let Some(mut material) = materials.get_mut(*layer) else {
        return;
    };
    if heights.size().min_element() == 0 {
        material.max_height = 0.;
        return;
    }
    // the texture's rows go from the top down, the heights' from the bottom up
    let data = heights
        .values()
        .chunks(heights.size().x as usize)
        .rev()
        .flatten()
        .copied()
        .collect();
    if let Some(mut image) = images.get_mut(&material.heights) {
        *image = single_channel_image(heights.size(), data);
    }
    material.heights_area = area(heights.rect());
    material.max_height = heights.max();
}

/// Keeps the layer around the camera, wherever the player walks.
fn follow_camera(
    camera: Single<&Transform, (With<Camera2d>, Without<LightLayer>)>,
    mut layer: Single<&mut Transform, With<LightLayer>>,
) {
    layer.translation = camera.translation.xy().extend(LIGHT_Z);
}

/// Draws the light. Needs [`super::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<LightMaterial>::default())
            .init_resource::<Lightmap>()
            .add_systems(Startup, spawn_layer)
            .add_systems(
                Update,
                (
                    update_light
                        .run_if(resource_exists::<LightValues>)
                        .run_if(resource_exists::<DayNight>),
                    write_heights,
                ),
            )
            .add_systems(
                PostUpdate,
                follow_camera.before(TransformSystems::Propagate),
            );
    }
}
