//! Things in the world the player can interact with by clicking them: items on the ground and
//! chests. Hovering one outlines it: red while the player can interact with it (within reach and
//! in sight), white while it is too far away.
//!
//! The outline is a copy of each of the thing's meshes, grown by [`OUTLINE_WIDTH`] and drawn
//! right behind it, so a figure gets one around its silhouette.

use crate::{
    objects::player::Player,
    sight::{
        Sight,
        render::{SightMask, SightMaterial},
    },
};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, MeshTag, PrimitiveTopology},
    picking::hover::Hovered,
    platform::collections::HashMap,
    prelude::*,
};

/// Width of the outline in px.
pub const OUTLINE_WIDTH: f32 = 1.;
/// z of the outline below the mesh it surrounds, more than the parts of a figure span.
const OUTLINE_Z: f32 = -0.002;
const IN_REACH: Color = Color::srgb(0.9, 0.12, 0.1);
const OUT_OF_REACH: Color = Color::WHITE;

/// Something the player can interact with, on the entity the pointer hovers (with `Pickable`,
/// or above meshes with it). Without `Pickable`, it has no outline.
#[derive(Component, Clone, Copy, Debug)]
#[require(Hovered)]
pub struct Interactable {
    /// Distance in px from the player's center up to which the player reaches it
    pub reach: f32,
    /// Radius in px of the area that must be in sight
    pub radius: f32,
}

impl Interactable {
    /// Whether it is in sight at `position`; without a sight everything is.
    pub fn seen(&self, position: Vec2, sight: Option<&Sight>) -> bool {
        sight.is_none_or(|sight| sight.sees(position, self.radius))
    }

    /// Whether the player at `player` can interact with it at `position`: in reach and in sight.
    pub fn can_interact(&self, position: Vec2, player: Vec2, sight: Option<&Sight>) -> bool {
        position.distance(player) <= self.reach && self.seen(position, sight)
    }
}

/// The current outline of an interactable thing.
#[derive(Component)]
struct Outline {
    in_reach: bool,
    meshes: Vec<Entity>,
}

/// A mesh of an outline.
#[derive(Component)]
struct OutlineMesh;

/// Grown copies of meshes, by the original's id, and the two outline materials.
#[derive(Resource, Default)]
struct OutlineAssets {
    meshes: HashMap<AssetId<Mesh>, Handle<Mesh>>,
    materials: Option<[Handle<SightMaterial>; 2]>,
}

/// Convex hull of 2D points, counterclockwise (monotone chain).
fn hull(mut points: Vec<Vec2>) -> Vec<Vec2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut lower: Vec<Vec2> = Vec::new();
    for &p in &points {
        while lower.len() >= 2 && cross(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0. {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<Vec2> = Vec::new();
    for &p in points.iter().rev() {
        while upper.len() >= 2 && cross(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0. {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// The convex hull of `points`, grown by `width` on all sides. Sharp corners are cut short at
/// twice `width`.
pub fn grown_outline(points: Vec<Vec2>, width: f32) -> Vec<Vec2> {
    let hull = hull(points);
    let n = hull.len();
    if n < 3 {
        return hull;
    }
    (0..n)
        .map(|i| {
            let (prev, p, next) = (hull[(i + n - 1) % n], hull[i], hull[(i + 1) % n]);
            // outward normals of the edges before and after `p`, counterclockwise
            let n1 = -(p - prev).normalize_or_zero().perp();
            let n2 = -(next - p).normalize_or_zero().perp();
            // the corner of the two grown edges lies along n1 + n2
            let miter = width * (2. / (1. + n1.dot(n2)).max(1e-3)).sqrt();
            p + (n1 + n2).normalize_or_zero() * miter.min(2. * width)
        })
        .collect()
}

fn grown_mesh(mesh: &Mesh) -> Option<Mesh> {
    let points = mesh
        .attribute(Mesh::ATTRIBUTE_POSITION)?
        .as_float3()?
        .iter()
        .map(|p| Vec2::new(p[0], p[1]))
        .collect();
    let outline = grown_outline(points, OUTLINE_WIDTH);
    if outline.len() < 3 {
        return None;
    }
    let positions: Vec<[f32; 3]> = outline.iter().map(|p| [p.x, p.y, 0.]).collect();
    let normals = vec![[0., 0., 1.]; positions.len()];
    let indices = (1..outline.len() as u32 - 1)
        .flat_map(|i| [0, i, i + 1])
        .collect();
    Some(
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices)),
    )
}

/// Outlines hovered interactable things, red in reach and white out of it, and removes the
/// outline once the pointer leaves, the thing can't be clicked any more or is out of sight.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_outlines(
    mut commands: Commands,
    things: Query<
        (
            Entity,
            Option<&Interactable>,
            Option<&Hovered>,
            &GlobalTransform,
            Has<Pickable>,
            Option<&Outline>,
        ),
        Or<(With<Interactable>, With<Outline>)>,
    >,
    descendants: Query<&Children>,
    sources: Query<(&Mesh2d, Option<&MeshTag>), Without<OutlineMesh>>,
    player: Option<Single<&Transform, With<Player>>>,
    sight: Option<Res<Sight>>,
    mut assets: ResMut<OutlineAssets>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mask: Res<SightMask>,
) {
    let player = player.map(|p| p.translation.xy());
    for (entity, interactable, hovered, transform, pickable, outline) in &things {
        let position = transform.translation().xy();
        let wanted = match (interactable, hovered, player) {
            (Some(interactable), Some(Hovered(true)), Some(player))
                if pickable && interactable.seen(position, sight.as_deref()) =>
            {
                Some(interactable.can_interact(position, player, sight.as_deref()))
            }
            _ => None,
        };
        if outline.map(|o| o.in_reach) == wanted {
            continue;
        }
        if let Some(outline) = outline {
            for &mesh in &outline.meshes {
                commands.entity(mesh).try_despawn();
            }
        }
        let Some(in_reach) = wanted else {
            commands.entity(entity).remove::<Outline>();
            continue;
        };

        let [red, white] = assets
            .materials
            .get_or_insert_with(|| {
                [IN_REACH, OUT_OF_REACH]
                    .map(|color| materials.add(SightMaterial::new(color, &mask)))
            })
            .clone();
        let material = if in_reach { red } else { white };
        let mut outline_meshes = Vec::new();
        for source in std::iter::once(entity).chain(descendants.iter_descendants(entity)) {
            let Ok((Mesh2d(mesh), tag)) = sources.get(source) else {
                continue;
            };
            let grown = match assets.meshes.get(&mesh.id()) {
                Some(grown) => grown.clone(),
                None => {
                    let Some(grown) = meshes.get(mesh).and_then(grown_mesh) else {
                        continue;
                    };
                    let grown = meshes.add(grown);
                    assets.meshes.insert(mesh.id(), grown.clone());
                    grown
                }
            };
            let mut child = commands.spawn((
                OutlineMesh,
                Mesh2d(grown),
                MeshMaterial2d(material.clone()),
                Transform::from_xyz(0., 0., OUTLINE_Z),
                ChildOf(source),
            ));
            if let Some(tag) = tag {
                // a figure's parts see the sight where its feet are
                child.insert(tag.clone());
            }
            outline_meshes.push(child.id());
        }
        commands.entity(entity).insert(Outline {
            in_reach,
            meshes: outline_meshes,
        });
    }
}

/// Outlines hovered interactable things. Needs [`crate::sight::render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OutlineAssets>()
            .add_systems(Update, update_outlines);
    }
}
