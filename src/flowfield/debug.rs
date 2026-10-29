//! Debug overlay for the window of one grid of the flow field. Blocked cells are red, unreachable
//! ones dark, and reachable ones colored by their walking distance to the player (green near,
//! purple far). An arrow in each cell shows the direction the field leads in. Units with a free
//! line of sight walk straight at the player instead, which the arrows don't show.

use super::{CELL_SIZE, FlowField};
use bevy::{
    asset::RenderAssetUsages,
    image::ToExtents,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat},
    sprite::Anchor,
};

/// Index of the grid shown in [`FlowField::grids`], `None` while the overlay is hidden.
#[derive(Resource, Default)]
pub struct FlowFieldDebug {
    pub grid: Option<usize>,
}

impl FlowFieldDebug {
    /// Shows the next of `grid_count` grids, or hides the overlay after the last one.
    pub fn cycle(&mut self, grid_count: usize) {
        self.grid = match self.grid {
            None if grid_count > 0 => Some(0),
            Some(i) if i + 1 < grid_count => Some(i + 1),
            _ => None,
        };
    }
}

/// Sprite with one pixel per cell, stretched over the grid's window.
#[derive(Component)]
struct Overlay;

/// Between the map at z = 0 and the units at z = 10.
const OVERLAY_Z: f32 = 5.;

fn spawn_overlay(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.spawn((
        Overlay,
        Name::new("Flow field overlay"),
        Sprite::from_image(images.add(cell_image(UVec2::ONE))),
        Anchor::BOTTOM_LEFT,
        Visibility::Hidden,
    ));
}

fn cell_image(size: UVec2) -> Image {
    Image::new_fill(
        size.to_extents(),
        TextureDimension::D2,
        &[0; 4],
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

fn draw_overlay(
    debug: Res<FlowFieldDebug>,
    flow_field: Res<FlowField>,
    overlay: Single<(&mut Sprite, &mut Transform, &mut Visibility), With<Overlay>>,
    mut images: ResMut<Assets<Image>>,
    mut gizmos: Gizmos,
) {
    let (mut sprite, mut transform, mut visibility) = overlay.into_inner();
    let Some(grid) = debug.grid.and_then(|i| flow_field.grids().get(i)) else {
        visibility.set_if_neq(Visibility::Hidden);
        return;
    };
    visibility.set_if_neq(Visibility::Inherited);

    let window = grid.window();
    let size = window.size.as_uvec2();
    transform.translation = window.world_rect().min.extend(OVERLAY_Z);
    sprite.custom_size = Some(size.as_vec2() * CELL_SIZE);
    let Some(mut image) = images.get_mut(&sprite.image) else {
        return;
    };
    if image.size() != size {
        image.resize(size.to_extents());
    }

    let max_distance = grid.max_distance();
    for cell in grid.cells() {
        let color = match (cell.blocked, cell.distance, max_distance) {
            (true, ..) => Color::srgba(0.9, 0.1, 0.1, 0.5),
            (false, Some(distance), Some(max)) => {
                let t = distance as f32 / max.max(1) as f32;
                Color::hsla(120. + t * 180., 0.9, 0.5, 0.35)
            }
            _ => Color::srgba(0., 0., 0., 0.4),
        };
        // image rows go from top to bottom, grid rows from bottom to top
        let local = (cell.position - window.min).as_uvec2();
        let pixel = UVec2::new(local.x, size.y - 1 - local.y);
        let _ = image.set_color_at(pixel.x, pixel.y, color);

        if let Some(direction) = cell.direction {
            let offset = direction * CELL_SIZE * 0.4;
            gizmos
                .arrow_2d(
                    cell.center - offset,
                    cell.center + offset,
                    Color::srgba(1., 1., 1., 0.6),
                )
                .with_tip_length(CELL_SIZE * 0.3);
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        // after the field is updated in PreUpdate
        app.init_resource::<FlowFieldDebug>()
            .add_systems(Startup, spawn_overlay)
            .add_systems(Update, draw_overlay);
    }
}
