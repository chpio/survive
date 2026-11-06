//! Animated figures: rigid, single-colored 2D parts on a skeleton, modeled and animated in Blender
//! (`art/`) and loaded from `.glb` files.
//!
//! A figure faces +X and lies on the XY plane; Z only orders its parts. Every node of the glTF
//! (bones and parts) may only rotate around Z and keep its Z scale, so a part's depth can be moved
//! from its vertices into its entity, where Bevy's 2D sorting needs it. The parts are drawn with a
//! [`SightMaterial`] in their glTF base color, so only what is in sight shows. Animations become
//! Bevy [`AnimationClip`]s in one [`AnimationGraph`] per figure.
//!
//! Put [`FigureRoot`] on an entity to show a figure as its children; its first animation plays in
//! a loop on the entity's [`AnimationPlayer`]. Saving the `.glb` again rebuilds the figures.

use crate::sight::render::{SightMask, SightMaterial};
use bevy::{
    animation::{AnimatedBy, AnimationTargetId, animated_field},
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    curve::UnevenSampleAutoCurve,
    mesh::{Indices, PrimitiveTopology},
    platform::collections::HashMap,
    prelude::*,
};

/// A node of a figure: a bone, or the holder of a part.
#[derive(Clone, Debug)]
pub struct NodeData {
    pub name: String,
    /// Index of the parent node, `None` for the figure's top nodes
    pub parent: Option<usize>,
    pub transform: Transform,
    /// Names of the nodes from the top down to this one, which identify it for the animations
    pub path: Vec<String>,
}

/// A flat, single-colored part, attached to a node.
#[derive(Clone, Debug)]
pub struct PartData {
    pub node: usize,
    pub color: Color,
    /// Depth of the part, taken out of its vertices
    pub z: f32,
    /// Vertices in the node's space, at z = 0
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug)]
pub enum Keyframes {
    Translation(Vec<Vec3>),
    Rotation(Vec<Quat>),
    Scale(Vec<Vec3>),
}

/// Keyframes of one property of one node.
#[derive(Clone, Debug)]
pub struct Channel {
    pub node: usize,
    pub times: Vec<f32>,
    pub keyframes: Keyframes,
}

#[derive(Clone, Debug)]
pub struct AnimationData {
    pub name: String,
    pub channels: Vec<Channel>,
}

/// A figure as read from a `.glb` file, before it becomes Bevy assets.
#[derive(Clone, Debug)]
pub struct FigureData {
    /// Parents come before their children
    pub nodes: Vec<NodeData>,
    pub parts: Vec<PartData>,
    pub animations: Vec<AnimationData>,
}

/// Tolerance for the checks that a node only rotates around Z.
const EPSILON: f32 = 1e-4;

/// Reads a figure from the bytes of a `.glb` file.
pub fn parse(bytes: &[u8]) -> Result<FigureData, BevyError> {
    let gltf::Gltf { document, blob } = gltf::Gltf::from_slice(bytes)?;
    let blob = blob.as_deref();
    let scene = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .ok_or("the file has no scene")?;

    let mut nodes: Vec<NodeData> = Vec::new();
    let mut parts = Vec::new();
    // our node index of each glTF node
    let mut index_of = HashMap::new();
    let mut stack: Vec<_> = scene.nodes().map(|node| (node, None)).collect();
    stack.reverse();
    while let Some((node, parent)) = stack.pop() {
        let name = node
            .name()
            .map_or_else(|| format!("node{}", node.index()), str::to_owned);
        let (translation, rotation, scale) = node.transform().decomposed();
        let rotation = Quat::from_array(rotation);
        if rotation.x.abs() > EPSILON || rotation.y.abs() > EPSILON {
            return Err(format!("node {name} rotates around another axis than Z").into());
        }
        if (scale[2] - 1.).abs() > EPSILON {
            return Err(format!("node {name} is scaled along Z").into());
        }
        let mut path = parent.map_or_else(Vec::new, |p: usize| nodes[p].path.clone());
        path.push(name.clone());
        let index = nodes.len();
        index_of.insert(node.index(), index);
        nodes.push(NodeData {
            name,
            parent,
            transform: Transform {
                translation: Vec3::from_array(translation),
                rotation,
                scale: Vec3::from_array(scale),
            },
            path,
        });

        if let Some(mesh) = node.mesh() {
            for primitive in mesh.primitives() {
                parts.push(read_part(index, &primitive, blob)?);
            }
        }
        for child in node.children().collect::<Vec<_>>().into_iter().rev() {
            stack.push((child, Some(index)));
        }
    }

    let mut animations = Vec::new();
    for (i, animation) in document.animations().enumerate() {
        let name = animation
            .name()
            .map_or_else(|| format!("animation{i}"), str::to_owned);
        let mut channels = Vec::new();
        for channel in animation.channels() {
            let target = channel.target().node();
            // nodes outside the scene don't matter
            let Some(&node) = index_of.get(&target.index()) else {
                continue;
            };
            let interpolation = channel.sampler().interpolation();
            if interpolation == gltf::animation::Interpolation::CubicSpline {
                return Err(format!("animation {name} uses cubic splines").into());
            }
            let reader = channel.reader(|b| buffer(b, blob));
            let times: Vec<f32> = reader
                .read_inputs()
                .ok_or_else(|| format!("animation {name} has no keyframe times"))?
                .collect();
            use gltf::animation::util::ReadOutputs;
            let keyframes = match reader.read_outputs() {
                Some(ReadOutputs::Translations(values)) => {
                    Keyframes::Translation(values.map(Vec3::from_array).collect())
                }
                Some(ReadOutputs::Rotations(values)) => {
                    Keyframes::Rotation(values.into_f32().map(Quat::from_array).collect())
                }
                Some(ReadOutputs::Scales(values)) => {
                    Keyframes::Scale(values.map(Vec3::from_array).collect())
                }
                // morph targets, which flat parts don't have
                Some(ReadOutputs::MorphTargetWeights(_)) | None => continue,
            };
            channels.push(Channel {
                node,
                times,
                keyframes,
            });
        }
        animations.push(AnimationData { name, channels });
    }

    Ok(FigureData {
        nodes,
        parts,
        animations,
    })
}

/// Data of `buffer`, which can only be the binary chunk of a `.glb` file.
fn buffer<'s>(buffer: gltf::Buffer, blob: Option<&'s [u8]>) -> Option<&'s [u8]> {
    match buffer.source() {
        gltf::buffer::Source::Bin => blob,
        gltf::buffer::Source::Uri(_) => None,
    }
}

fn read_part(
    node: usize,
    primitive: &gltf::Primitive,
    blob: Option<&[u8]>,
) -> Result<PartData, BevyError> {
    if primitive.mode() != gltf::mesh::Mode::Triangles {
        return Err("parts must consist of triangles".into());
    }
    let reader = primitive.reader(|b| buffer(b, blob));
    let mut positions: Vec<[f32; 3]> = reader
        .read_positions()
        .ok_or("a part has no vertex positions")?
        .collect();
    let indices = match reader.read_indices() {
        Some(indices) => indices.into_u32().collect(),
        None => (0..positions.len() as u32).collect(),
    };
    let z = positions.iter().map(|p| p[2]).sum::<f32>() / positions.len().max(1) as f32;
    for position in &mut positions {
        position[2] = 0.;
    }
    let [r, g, b, a] = primitive
        .material()
        .pbr_metallic_roughness()
        .base_color_factor();
    Ok(PartData {
        node,
        color: Color::linear_rgba(r, g, b, a),
        z,
        positions,
        indices,
    })
}

/// A loaded figure, ready to be spawned with [`FigureRoot`].
#[derive(Asset, TypePath, Debug)]
pub struct Figure {
    nodes: Vec<NodeData>,
    /// Node, mesh, color and depth of each part
    parts: Vec<(usize, Handle<Mesh>, Color, f32)>,
    pub graph: Handle<AnimationGraph>,
    /// Graph node of each animation, by name, in the file's order
    pub animations: Vec<(String, AnimationNodeIndex)>,
}

impl Figure {
    /// The graph node of the animation `name`.
    pub fn animation(&self, name: &str) -> Option<AnimationNodeIndex> {
        self.animations
            .iter()
            .find(|(n, _)| n == name)
            .map(|&(_, index)| index)
    }
}

fn target_id(node: &NodeData) -> AnimationTargetId {
    AnimationTargetId::from_iter(&node.path)
}

#[derive(Default, TypePath)]
struct FigureLoader;

impl AssetLoader for FigureLoader {
    type Asset = Figure;
    type Settings = ();
    type Error = BevyError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        load_context: &mut LoadContext<'_>,
    ) -> Result<Figure, BevyError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let data = parse(&bytes)?;

        let parts = data
            .parts
            .into_iter()
            .enumerate()
            .map(|(i, part)| {
                let normals = vec![[0., 0., 1.]; part.positions.len()];
                let mesh = Mesh::new(
                    PrimitiveTopology::TriangleList,
                    RenderAssetUsages::default(),
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, part.positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
                .with_inserted_indices(Indices::U32(part.indices));
                let mesh = load_context.add_labeled_asset(format!("part{i}"), mesh);
                (part.node, mesh, part.color, part.z)
            })
            .collect();

        let mut clips = Vec::new();
        for animation in &data.animations {
            let mut clip = AnimationClip::default();
            for channel in &animation.channels {
                let target = target_id(&data.nodes[channel.node]);
                let times = channel.times.iter().copied();
                let error = |e| format!("animation {}: {e}", animation.name);
                match &channel.keyframes {
                    Keyframes::Translation(values) => clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            animated_field!(Transform::translation),
                            UnevenSampleAutoCurve::new(times.zip(values.iter().copied()))
                                .map_err(error)?,
                        ),
                    ),
                    Keyframes::Rotation(values) => clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            animated_field!(Transform::rotation),
                            UnevenSampleAutoCurve::new(times.zip(values.iter().copied()))
                                .map_err(error)?,
                        ),
                    ),
                    Keyframes::Scale(values) => clip.add_curve_to_target(
                        target,
                        AnimatableCurve::new(
                            animated_field!(Transform::scale),
                            UnevenSampleAutoCurve::new(times.zip(values.iter().copied()))
                                .map_err(error)?,
                        ),
                    ),
                }
            }
            clips.push(
                load_context.add_labeled_asset(format!("animation:{}", animation.name), clip),
            );
        }
        let (graph, indices) = AnimationGraph::from_clips(clips);
        let graph = load_context.add_labeled_asset("graph".to_owned(), graph);
        let animations = data
            .animations
            .into_iter()
            .map(|animation| animation.name)
            .zip(indices)
            .collect();

        Ok(Figure {
            nodes: data.nodes,
            parts,
            graph,
            animations,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["glb"]
    }
}

/// Shows the figure as children of this entity, which needs a `Transform` and `Visibility`.
#[derive(Component, Clone, Debug)]
pub struct FigureRoot(pub Handle<Figure>);

/// The figure's top nodes, spawned once the figure was loaded.
#[derive(Component)]
struct Instance(Vec<Entity>);

/// One material per color, shared by all figures.
#[derive(Resource, Default)]
struct Materials(HashMap<[u32; 4], Handle<SightMaterial>>);

fn instantiate(
    mut commands: Commands,
    roots: Query<(Entity, &FigureRoot), Without<Instance>>,
    figures: Res<Assets<Figure>>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mut cache: ResMut<Materials>,
    mask: Res<SightMask>,
) {
    for (root, FigureRoot(handle)) in &roots {
        let Some(figure) = figures.get(handle) else {
            continue;
        };
        let mut entities = Vec::with_capacity(figure.nodes.len());
        let mut top = Vec::new();
        for node in &figure.nodes {
            let parent = node.parent.map_or(root, |p| entities[p]);
            let entity = commands
                .spawn((
                    Name::new(node.name.clone()),
                    node.transform,
                    Visibility::default(),
                    target_id(node),
                    AnimatedBy(root),
                    ChildOf(parent),
                ))
                .id();
            if node.parent.is_none() {
                top.push(entity);
            }
            entities.push(entity);
        }
        for (node, mesh, color, z) in &figure.parts {
            let key = color.to_linear().to_f32_array().map(f32::to_bits);
            let material = cache
                .0
                .entry(key)
                .or_insert_with(|| materials.add(SightMaterial::new(*color, &mask)))
                .clone();
            commands.spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material),
                Transform::from_xyz(0., 0., *z),
                ChildOf(entities[*node]),
            ));
        }

        let mut player = AnimationPlayer::default();
        if let Some(&(_, animation)) = figure.animations.first() {
            player.play(animation).repeat();
        }
        commands.entity(root).insert((
            Instance(top),
            player,
            AnimationGraphHandle(figure.graph.clone()),
        ));
    }
}

/// Removes the figures of a changed `.glb` file, so [`instantiate`] builds them again.
fn rebuild_changed(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<Figure>>,
    roots: Query<(Entity, &FigureRoot, &Instance)>,
) {
    for event in events.read() {
        let AssetEvent::Modified { id } = event else {
            continue;
        };
        for (root, FigureRoot(handle), Instance(top)) in &roots {
            if handle.id() == *id {
                for &entity in top {
                    commands.entity(entity).despawn();
                }
                commands.entity(root).remove::<Instance>();
            }
        }
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<Figure>()
            .init_asset_loader::<FigureLoader>()
            .init_resource::<Materials>()
            .add_systems(Update, (rebuild_changed, instantiate).chain());
    }
}
