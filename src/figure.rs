//! Animated figures: the player and the enemies, modeled in 3D and animated in Blender (`art/`),
//! exported as flat parts per direction of the game's oblique view and loaded from `.glb` files
//! (see `docs/architecture/figure.md`).
//!
//! Every node of the glTF may only rotate around Z and keep its Z scale, so a part's depth can be
//! moved from its vertices into its entity, where Bevy's 2D sorting needs it. The parts are drawn
//! with a [`SightMaterial`] in their glTF base color, anchored at the figure's feet. Animations
//! become Bevy [`AnimationClip`]s, one per animation and direction, in one [`AnimationGraph`].
//!
//! Put [`FigureRoot`] on an entity to show a figure as its children and steer it with its
//! [`FigureControl`]. Saving the `.glb` again rebuilds the figures.

use crate::sight::render::{SightMask, SightMaterial};
use bevy::{
    animation::{AnimatedBy, AnimationTargetId, animated_field},
    asset::{AssetLoader, LoadContext, RenderAssetUsages, io::Reader},
    curve::UnevenSampleAutoCurve,
    mesh::{Indices, MeshTag, PrimitiveTopology},
    platform::collections::HashMap,
    prelude::*,
};
use std::{
    f32::consts::{PI, TAU},
    ops::Range,
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
/// A loaded figure, ready to be spawned with [`FigureRoot`].
#[derive(Asset, TypePath, Debug)]
pub struct Figure {
    nodes: Vec<NodeData>,
    /// Node, mesh, color and depth of each part
    parts: Vec<(usize, Handle<Mesh>, Color, f32)>,
    /// Nodes of each direction: its node and its descendants, which follow it
    directions: Vec<Range<usize>>,
    pub graph: Handle<AnimationGraph>,
    /// Names of the animations, in the file's order
    names: Vec<String>,
    /// Graph node of each animation and direction
    clips: HashMap<(String, usize), AnimationNodeIndex>,
}

impl Figure {
    /// The graph node of the animation `name` in `direction`.
    pub fn clip(&self, name: &str, direction: usize) -> Option<AnimationNodeIndex> {
        self.clips.get(&(name.to_owned(), direction)).copied()
    }

    /// Number of directions the figure was drawn in.
    pub fn directions(&self) -> usize {
        self.directions.len()
    }
}

/// The direction of `count` (evenly spread, the first facing +X, counterclockwise) to show for
/// `facing`. The `current` one stays until `facing` is a few degrees closer to another one, so
/// a figure walking just between two directions doesn't flicker.
pub fn direction_for(facing: Vec2, current: Option<usize>, count: usize) -> usize {
    const HYSTERESIS: f32 = 0.1;
    let step = TAU / count as f32;
    let angle = facing.to_angle();
    if let Some(current) = current {
        let offset = (angle - current as f32 * step + PI).rem_euclid(TAU) - PI;
        if offset.abs() <= step / 2. + HYSTERESIS {
            return current;
        }
    }
    (angle / step).round().rem_euclid(count as f32) as usize % count
}

/// `MeshTag` that makes the sight material sample the mask at `position` (rounded to whole px)
/// for the whole mesh, instead of at each pixel: x and y offset by 2^15 in the low and high half.
/// Covers positions within ±32767 px; 0 (never produced) means sampling at each pixel.
pub fn anchor_tag(position: Vec2) -> u32 {
    let x = (position.x.round() + 32768.).clamp(0., 65535.) as u32;
    let y = (position.y.round() + 32768.).clamp(0., 65535.) as u32;
    (y << 16 | x).max(1)
}

/// Z within a unit (at z = 10) of a figure at height `y` with the camera at `camera_y`: lower on
/// screen is in front, 1 px apart is 0.001 apart, more than a figure's parts span. Figures up to
/// 450 px above or below the camera are sorted, which covers the view.
pub fn sorted_z(y: f32, camera_y: f32) -> f32 {
    (0.45 + (camera_y - y) * 0.001).clamp(0., 0.9)
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
        let mut names = Vec::new();
        let mut clips = HashMap::new();
        for (animation, index) in data.animations.into_iter().zip(indices) {
            let (name, direction) = animation
                .name
                .rsplit_once('/')
                .and_then(|(name, d)| Some((name.to_owned(), d.parse().ok()?)))
                .ok_or_else(|| {
                    format!(
                        "animation {} isn't named <name>/<direction>",
                        animation.name
                    )
                })?;
            if !names.contains(&name) {
                names.push(name.clone());
            }
            clips.insert((name, direction), index);
        }

        // the top node holds the directions, whose subtrees follow each other
        let starts: Vec<_> = (0..data.nodes.len())
            .filter(|&i| data.nodes[i].parent == Some(0))
            .collect();
        if starts.is_empty() {
            return Err("the figure has no directions".into());
        }
        let directions = starts
            .iter()
            .zip(starts.iter().skip(1).copied().chain([data.nodes.len()]))
            .map(|(&start, end)| start..end)
            .collect();

        Ok(Figure {
            nodes: data.nodes,
            parts,
            directions,
            graph,
            names,
            clips,
        })
    }

    fn extensions(&self) -> &[&str] {
        &["glb"]
    }
}

/// Shows the figure as children of this entity, which needs a `Transform` and `Visibility`. The
/// entity's z is set to sort it by its height on screen (see [`sorted_z`]).
#[derive(Component, Clone, Debug)]
#[require(FigureControl)]
pub struct FigureRoot(pub Handle<Figure>);

/// What a figure shows: which animation, facing where, how fast.
#[derive(Component, Clone, Debug)]
pub struct FigureControl {
    /// Name of the animation; empty for the figure's first one
    pub animation: &'static str,
    /// Direction the figure faces on the ground; the closest drawn direction is shown
    pub facing: Vec2,
    /// Speed of the animation, 1 for its own pace
    pub speed: f32,
}

impl Default for FigureControl {
    fn default() -> Self {
        Self {
            animation: "",
            // towards the viewer
            facing: Vec2::NEG_Y,
            speed: 1.,
        }
    }
}

/// The spawned direction of a figure.
#[derive(Component)]
struct Instance {
    direction: usize,
    animation: String,
    /// Node of the direction, holding all others
    top: Entity,
    parts: Vec<Entity>,
    /// Current [`anchor_tag`] of the parts
    tag: u32,
}

/// One material per color, shared by all figures.
#[derive(Resource, Default)]
struct Materials(HashMap<[u32; 4], Handle<SightMaterial>>);

/// Spawns each figure's current direction, switches direction and animation as its
/// [`FigureControl`] asks, sorts it by its height on screen and anchors its sight at its feet.
#[allow(clippy::type_complexity)]
fn update(
    mut commands: Commands,
    mut roots: Query<(
        Entity,
        &FigureRoot,
        &FigureControl,
        &GlobalTransform,
        &mut Transform,
        Option<&mut Instance>,
        Option<&mut AnimationPlayer>,
    )>,
    camera: Query<&GlobalTransform, (With<Camera2d>, Without<FigureRoot>)>,
    figures: Res<Assets<Figure>>,
    mut materials: ResMut<Assets<SightMaterial>>,
    mut cache: ResMut<Materials>,
    mask: Res<SightMask>,
) {
    let camera_y = camera.single().map_or(0., |c| c.translation().y);
    for (root, FigureRoot(handle), control, global, mut transform, instance, player) in &mut roots {
        let Some(figure) = figures.get(handle) else {
            continue;
        };
        let position = global.translation().xy();
        let z = sorted_z(position.y, camera_y);
        if transform.translation.z != z {
            transform.translation.z = z;
        }
        let tag = anchor_tag(position);
        let animation = match control.animation {
            "" => figure.names.first().map_or("", String::as_str),
            name => name,
        };
        let current = instance.as_ref().map(|i| i.direction);
        let direction = direction_for(control.facing, current, figure.directions());

        if let Some(mut instance) = instance {
            if instance.direction == direction && instance.animation == animation {
                if let Some(mut player) = player {
                    for (_, active) in player.playing_animations_mut() {
                        if active.speed() != control.speed {
                            active.set_speed(control.speed);
                        }
                    }
                }
                if instance.tag != tag {
                    instance.tag = tag;
                    for &part in &instance.parts {
                        commands.entity(part).insert(MeshTag::new(tag));
                    }
                }
                continue;
            }
            if instance.direction == direction {
                // only the animation changes
                let Some(clip) = figure.clip(animation, direction) else {
                    continue;
                };
                instance.animation = animation.to_owned();
                commands
                    .entity(root)
                    .insert(new_player(clip, 0., control.speed));
                continue;
            }
            commands.entity(instance.top).despawn();
        }

        let Some(clip) = figure.clip(animation, direction) else {
            continue;
        };
        // a new direction of the same animation goes on where the old one was
        let time = player
            .as_ref()
            .and_then(|p| p.playing_animations().next().map(|(_, a)| a.seek_time()))
            .filter(|_| current.is_some())
            .unwrap_or(0.);

        let range = figure.directions[direction].clone();
        let mut entities = Vec::with_capacity(range.len());
        for node in &figure.nodes[range.clone()] {
            let parent = node
                .parent
                .filter(|p| range.contains(p))
                .map_or(root, |p| entities[p - range.start]);
            entities.push(
                commands
                    .spawn((
                        Name::new(node.name.clone()),
                        node.transform,
                        Visibility::default(),
                        target_id(node),
                        AnimatedBy(root),
                        ChildOf(parent),
                    ))
                    .id(),
            );
        }
        let mut parts = Vec::new();
        for (node, mesh, color, z) in &figure.parts {
            if !range.contains(node) {
                continue;
            }
            let key = color.to_linear().to_f32_array().map(f32::to_bits);
            let material = cache
                .0
                .entry(key)
                .or_insert_with(|| materials.add(SightMaterial::new(*color, &mask)))
                .clone();
            parts.push(
                commands
                    .spawn((
                        Mesh2d(mesh.clone()),
                        MeshMaterial2d(material),
                        MeshTag::new(tag),
                        Transform::from_xyz(0., 0., *z),
                        ChildOf(entities[node - range.start]),
                    ))
                    .id(),
            );
        }
        commands.entity(root).insert((
            Instance {
                direction,
                animation: animation.to_owned(),
                top: entities[0],
                parts,
                tag,
            },
            new_player(clip, time, control.speed),
            AnimationGraphHandle(figure.graph.clone()),
        ));
    }
}

fn new_player(clip: AnimationNodeIndex, time: f32, speed: f32) -> AnimationPlayer {
    let mut player = AnimationPlayer::default();
    player.play(clip).repeat().seek_to(time).set_speed(speed);
    player
}

/// Removes the figures of a changed `.glb` file, so [`update`] builds them again.
fn rebuild_changed(
    mut commands: Commands,
    mut events: MessageReader<AssetEvent<Figure>>,
    roots: Query<(Entity, &FigureRoot, &Instance)>,
) {
    for event in events.read() {
        let AssetEvent::Modified { id } = event else {
            continue;
        };
        for (root, FigureRoot(handle), instance) in &roots {
            if handle.id() == *id {
                commands.entity(instance.top).despawn();
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
            .add_systems(Update, (rebuild_changed, update).chain());
    }
}
