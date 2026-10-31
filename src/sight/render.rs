//! Draws the sight: the mask is written into a texture, which [`SightMaterial`] samples at each
//! pixel's world position. Enemies use it to only show where they are in sight, and a large dark
//! rectangle uses it inverted to darken the map outside the sight. Sprites (items, health bars)
//! can't be masked, so [`HiddenOutsideSight`] hides them as a whole: outside the sight, only the
//! map is drawn.

use super::{Sight, WINDOW_SIZE};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::VisibilitySystems,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin},
};

const SHADER: &str = "shaders/sight.wesl";

/// How dark the map is outside the sight, from 0 (not at all) to 1 (black).
pub const DARKNESS: f32 = 0.6;

/// Above the map, below the items (z = 9) and the units (z = 10), so it only darkens the map: items
/// and units hide themselves with [`SightMaterial`].
const DARKNESS_Z: f32 = 8.;

/// Edge length of the darkening rectangle in px, enough to cover the view at any zoom.
const DARKNESS_SIZE: f32 = 8192.;

/// Radius in px over which the mask is blurred, so the edges of shadows and of enemies partly in
/// sight are soft.
pub const BLUR: f32 = 6.;

/// Hidden while [`Sight::shows`] says the player doesn't see its position, for whatever isn't
/// masked by [`SightMaterial`]. Its children are hidden along with it.
#[derive(Component, Clone, Copy, Default, Debug)]
pub struct HiddenOutsideSight;

fn hide_outside_sight(
    sight: Res<Sight>,
    mut hidden: Query<(&GlobalTransform, &mut Visibility), With<HiddenOutsideSight>>,
) {
    for (transform, mut visibility) in &mut hidden {
        visibility.set_if_neq(if sight.shows(transform.translation().xy()) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Texture of the mask, one pixel per cell, filtered linearly so edges are soft.
#[derive(Resource)]
pub struct SightMask {
    image: Handle<Image>,
    /// Bottom-left corner (xy) and size (zw) of the area it covers in world space, for materials
    /// created after the mask was last written
    area: Vec4,
}

/// A linearly filtered `R8Unorm` texture of a mask with `size` cells, one pixel per cell, from
/// its `values` row by row from the bottom like a [`Raster`](crate::cells::Raster). Also used for
/// the light's masks.
pub fn mask_image(size: UVec2, values: &[u8]) -> Image {
    // the texture's rows go from the top down, the mask's from the bottom up
    let data = values
        .chunks(size.x as usize)
        .rev()
        .flatten()
        .copied()
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();
    image
}

/// Bottom-left corner (xy) and size (zw) of `rect`, as the materials take the area of a mask.
pub fn mask_area(rect: Rect) -> Vec4 {
    Vec4::new(rect.min.x, rect.min.y, rect.width(), rect.height())
}

impl SightMask {
    pub fn image(&self) -> &Handle<Image> {
        &self.image
    }

    /// Bottom-left corner (xy) and size (zw) of the area it covers in world space
    pub fn area(&self) -> Vec4 {
        self.area
    }
}

impl FromWorld for SightMask {
    fn from_world(world: &mut World) -> Self {
        let size = UVec2::splat(WINDOW_SIZE as u32);
        let image = mask_image(size, &vec![0; size.element_product() as usize]);
        Self {
            image: world.resource_mut::<Assets<Image>>().add(image),
            // invisible until the mask is computed
            area: Vec4::new(0., 0., 1., 1.),
        }
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
    /// See [`BLUR`]
    #[uniform(0)]
    blur: f32,
    #[texture(1)]
    #[sampler(2)]
    mask: Handle<Image>,
}

impl SightMaterial {
    /// Shows `color` where the mask is visible.
    pub fn new(color: Color, mask: &SightMask) -> Self {
        Self {
            color: color.into(),
            // updated whenever the mask moves
            mask_area: mask.area,
            invert: 0.,
            blur: BLUR,
            mask: mask.image.clone(),
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
    mut mask: ResMut<SightMask>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mut darkness: Single<&mut Transform, With<Darkness>>,
) {
    if !sight.is_changed() {
        return;
    }
    if let Some(mut image) = images.get_mut(&mask.image) {
        *image = mask_image(UVec2::splat(WINDOW_SIZE as u32), sight.mask());
    }

    let area = sight.window_rect();
    mask.area = mask_area(area);
    for (_, material) in materials.iter_mut() {
        material.mask_area = mask.area;
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
            .add_systems(Update, write_mask)
            // where things are this frame, before their visibility is passed on to their children
            .add_systems(
                PostUpdate,
                hide_outside_sight
                    .after(TransformSystems::Propagate)
                    .before(VisibilitySystems::VisibilityPropagate),
            );
    }
}
