//! Draws the sight: the mask is written into a texture, which [`SightMaterial`] samples at each
//! pixel's world position. Enemies use it to only show where they are in sight, and a large dark
//! rectangle uses it inverted to darken the map outside the sight.

use super::{Sight, WINDOW_SIZE};
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};

const SHADER: &str = "shaders/sight.wesl";

/// How dark the map is outside the sight, from 0 (not at all) to 1 (black).
pub const DARKNESS: f32 = 0.6;

/// Between the items (z = 5) and the units (z = 10), so it darkens the map and the items but not
/// the units, which hide themselves.
const DARKNESS_Z: f32 = 8.;

/// Edge length of the darkening rectangle in px, enough to cover the view at any zoom.
const DARKNESS_SIZE: f32 = 8192.;

/// Texture of the mask, one pixel per cell, filtered linearly so edges are soft.
#[derive(Resource)]
pub struct SightMask(pub Handle<Image>);

impl FromWorld for SightMask {
    fn from_world(world: &mut World) -> Self {
        let mut image = Image::new_fill(
            Extent3d {
                width: WINDOW_SIZE as u32,
                height: WINDOW_SIZE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0],
            TextureFormat::R8Unorm,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::linear();
        Self(world.resource_mut::<Assets<Image>>().add(image))
    }
}

/// A color shown only where the mask is visible, or only where it isn't (`invert`).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct SightMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// Bottom-left corner (xy) and size (zw) of the area the mask covers, in world space
    #[uniform(0)]
    mask_area: Vec4,
    /// 0 to show the color where the mask is visible, 1 where it isn't
    #[uniform(0)]
    invert: f32,
    #[texture(1)]
    #[sampler(2)]
    mask: Handle<Image>,
}

impl SightMaterial {
    /// Shows `color` where the mask is visible.
    pub fn new(color: Color, mask: &SightMask) -> Self {
        Self {
            color: color.into(),
            // set to the mask's area once it is computed, invisible until then
            mask_area: Vec4::new(0., 0., 1., 1.),
            invert: 0.,
            mask: mask.0.clone(),
        }
    }

    /// Shows `color` where the mask isn't visible.
    pub fn inverted(color: Color, mask: &SightMask) -> Self {
        Self {
            invert: 1.,
            ..Self::new(color, mask)
        }
    }
}

impl Material2d for SightMaterial {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

#[derive(Component)]
struct Darkness;

fn spawn_darkness(
    mut commands: Commands,
    mask: Res<SightMask>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SightMaterial>>,
) {
    commands.spawn((
        Darkness,
        Name::new("Darkness"),
        Mesh2d(meshes.add(Rectangle::from_length(DARKNESS_SIZE))),
        MeshMaterial2d(materials.add(SightMaterial::inverted(
            Color::BLACK.with_alpha(DARKNESS),
            &mask,
        ))),
        Transform::from_xyz(0., 0., DARKNESS_Z),
    ));
}

/// Writes the mask into its texture and its area into the materials whenever it changed, and
/// centers the darkening on it.
fn write_mask(
    sight: Res<Sight>,
    mask: Res<SightMask>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mut darkness: Single<&mut Transform, With<Darkness>>,
) {
    if !sight.is_changed() {
        return;
    }
    if let Some(mut image) = images.get_mut(&mask.0) {
        // the texture's rows go from the top down, the mask's from the bottom up
        let data = sight
            .mask()
            .chunks(WINDOW_SIZE as usize)
            .rev()
            .flatten()
            .copied()
            .collect();
        image.data = Some(data);
    }

    let area = sight.window_rect();
    let mask_area = Vec4::new(area.min.x, area.min.y, area.width(), area.height());
    for (_, material) in materials.iter_mut() {
        material.mask_area = mask_area;
    }
    darkness.translation = area.center().extend(DARKNESS_Z);
}

/// Draws the sight. Needs [`super::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<SightMaterial>::default())
            .init_resource::<SightMask>()
            .add_systems(Startup, spawn_darkness)
            .add_systems(Update, write_mask);
    }
}
