"""Shared code for the scripts that build figures in Blender (see `docs/architecture/figure.md`).

A figure is modeled in 3D from convex parts (boxes, ellipsoids, capsules), each on a bone of an
armature, and animated once. The export projects it into the game's oblique view for each of
`DIRECTIONS` directions, so the game only draws flat parts:

- 1 Blender unit = 1 px of the game world; the figure stands on the XY plane at the origin (its
  feet, where the game's collider is), faces +X and Z points up.
- The game's view is an oblique projection: a point (x, y, z) shows at (x, y + HEIGHT * z), like
  the tileset, whose ground is seen from above and whose walls from the front. Of overlapping
  parts, the one with the larger depth z - HEIGHT * y is in front.
- Direction d turns the figure by d · 360° / DIRECTIONS counterclockwise, so direction 0 faces
  right (+X) and direction DIRECTIONS / 4 away from the viewer (+Y).
- Each part keeps one flat shape per direction: the outline of its projection at rest. Per frame,
  it is moved, turned and scaled to fit its projection in that frame as well as it can, and its
  depth orders it. Parts turning towards the viewer therefore only change their size, which
  hardly shows on figures this small.

The `.glb` it writes has a top node `figure` with one child per direction (`d0`, `d1`, ...), each
holding that direction's parts as flat meshes, and the animations `<name>/<direction>`, which move
the parts of one direction.
"""

import json
import math
import struct
import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

ROOT = Path(__file__).resolve().parent.parent
FPS = 24
DIRECTIONS = 8
# screen px per px of height
HEIGHT = 1.0
# the game's z per px of depth, small enough that a figure's parts stay within z 0..0.001, so
# figures one pixel apart in y-sorting don't mix
DEPTH_SCALE = 1e-5
DEPTH_OFFSET = 0.0005


def preview_dir():
    """The directory after `--preview` on the command line, if any."""
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    if "--preview" in argv:
        return Path(argv[argv.index("--preview") + 1])
    return None


def linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


class Figure:
    """A figure under construction: armature, parts and animations."""

    def __init__(self, name, bones, colors):
        """`bones`: {name: (head, tail, parent)} in 3D; `colors`: {name: sRGB}."""
        bpy.ops.wm.read_factory_settings(use_empty=True)
        bpy.context.scene.render.fps = FPS
        self.name = name
        self.colors = colors
        self.parts = []  # (object, color, depth bias)
        self.animations = []  # (name, frames, pose)

        data = bpy.data.armatures.new(name)
        self.rig = bpy.data.objects.new(name, data)
        bpy.context.scene.collection.objects.link(self.rig)
        bpy.context.view_layer.objects.active = self.rig
        bpy.ops.object.mode_set(mode="EDIT")
        for bone_name, (head, tail, parent) in bones.items():
            bone = data.edit_bones.new(bone_name)
            bone.head = head
            bone.tail = tail
            if parent:
                bone.parent = data.edit_bones[parent]
        bpy.ops.object.mode_set(mode="OBJECT")

        self.materials = {}
        for color_name, srgb in colors.items():
            color = (*(linear(c) for c in srgb), 1.0)
            mat = bpy.data.materials.new(color_name)
            mat.diffuse_color = color
            mat.node_tree.nodes["Principled BSDF"].inputs["Base Color"].default_value = color
            self.materials[color_name] = mat

    def _part(self, name, bm, color, bone, bias):
        mesh = bpy.data.meshes.new(name)
        bm.to_mesh(mesh)
        bm.free()
        mesh.materials.append(self.materials[color])
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.collection.objects.link(obj)
        obj.parent = self.rig
        obj.parent_type = "BONE"
        obj.parent_bone = bone
        bpy.context.view_layer.update()
        # parenting to a bone moves the child to the bone's tail; keep the world position
        obj.matrix_world = Matrix.Identity(4)
        self.parts.append((obj, color, bias))
        return obj

    def ellipsoid(self, name, center, radii, color, bone, bias=0.0):
        """Ellipsoid part; `bias` is added to its depth, to settle the order of parts whose
        centers are equally deep."""
        bm = bmesh.new()
        bmesh.ops.create_uvsphere(bm, u_segments=12, v_segments=8, radius=1.0)
        bmesh.ops.transform(
            bm,
            matrix=Matrix.Translation(center) @ Matrix.Diagonal((*radii, 1.0)),
            verts=bm.verts,
        )
        return self._part(name, bm, color, bone, bias)

    def box(self, name, center, size, color, bone, bias=0.0):
        bm = bmesh.new()
        bmesh.ops.create_cube(bm, size=1.0)
        bmesh.ops.transform(
            bm,
            matrix=Matrix.Translation(center) @ Matrix.Diagonal((*size, 1.0)),
            verts=bm.verts,
        )
        return self._part(name, bm, color, bone, bias)

    def capsule(self, name, start, end, radius, color, bone, bias=0.0):
        """Stick from `start` to `end` with round ends: the hull of two spheres."""
        bm = bmesh.new()
        for point in (start, end):
            sphere = bmesh.ops.create_uvsphere(bm, u_segments=8, v_segments=6, radius=radius)
            bmesh.ops.translate(bm, vec=Vector(point), verts=sphere["verts"])
        bmesh.ops.convex_hull(bm, input=bm.verts[:])
        # the hull leaves the inner vertices unconnected
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
        return self._part(name, bm, color, bone, bias)

    def animate(self, name, frames, pose):
        """Animation `name` of `frames` frames, looping; `pose(fig, phase)` poses each frame
        with `turn` and `shift`, phase running from 0 to 2π over the cycle."""
        self.animations.append((name, frames, pose))

    def reset(self):
        for bone in self.rig.pose.bones:
            bone.rotation_mode = "QUATERNION"
            bone.rotation_quaternion = Quaternion()
            bone.location = Vector()
            bone.scale = Vector((1, 1, 1))

    def turn(self, bone_name, axis, degrees):
        """Turns a pose bone by `degrees` around the world axis `axis` (right-handed)."""
        bone = self.rig.data.bones[bone_name]
        local = bone.matrix_local.inverted().to_3x3() @ Vector(axis)
        pose = self.rig.pose.bones[bone_name]
        pose.rotation_quaternion = pose.rotation_quaternion @ Quaternion(
            local, math.radians(degrees)
        )

    def shift(self, bone_name, offset):
        """Moves a pose bone by `offset` in world directions."""
        bone = self.rig.data.bones[bone_name]
        pose = self.rig.pose.bones[bone_name]
        pose.location = pose.location + bone.matrix_local.inverted().to_3x3() @ Vector(offset)

    def _pose(self, pose, phase):
        self.reset()
        if pose:
            pose(self, phase)
        bpy.context.view_layer.update()
        depsgraph = bpy.context.evaluated_depsgraph_get()
        result = []
        for obj, _, bias in self.parts:
            matrix = obj.evaluated_get(depsgraph).matrix_world
            result.append([matrix @ v.co for v in obj.data.vertices])
        return result

    def keyframes(self):
        """Keys the animations as actions in NLA tracks, to play them in Blender."""
        self.rig.animation_data_create()
        for name, frames, pose in self.animations:
            action = bpy.data.actions.new(name)
            action.use_fake_user = True
            self.rig.animation_data.action = action
            for frame in range(frames + 1):
                self.reset()
                pose(self, 2 * math.pi * frame / frames)
                for bone in self.rig.pose.bones:
                    bone.keyframe_insert("rotation_quaternion", frame=frame)
                    bone.keyframe_insert("location", frame=frame)
            track = self.rig.animation_data.nla_tracks.new()
            track.name = name
            track.strips.new(name, 0, action)
            self.rig.animation_data.action = None
        self.reset()

    def build(self, preview=None):
        """Writes `art/<name>.blend` and `assets/characters/<name>.glb`, and with `preview` an
        animated preview per animation into that directory."""
        self.keyframes()
        bpy.ops.wm.save_as_mainfile(filepath=str(ROOT / "art" / f"{self.name}.blend"))
        views = project(self)
        out = ROOT / "assets" / "characters" / f"{self.name}.glb"
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_bytes(write_glb(self, views))
        if preview:
            write_preview(self, views, preview)


def view_point(point, direction):
    """A 3D point of the figure turned to `direction`: its position on screen and its depth."""
    angle = 2 * math.pi * direction / DIRECTIONS
    p = Matrix.Rotation(angle, 3, "Z") @ point
    return Vector((p.x, p.y + HEIGHT * p.z)), p.z - HEIGHT * p.y


def hull(points):
    """Convex hull of 2D points, counterclockwise (monotone chain)."""
    points = sorted(set((round(p.x, 5), round(p.y, 5)) for p in points))

    def cross(o, a, b):
        return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

    lower, upper = [], []
    for p in points:
        while len(lower) >= 2 and cross(lower[-2], lower[-1], p) <= 0:
            lower.pop()
        lower.append(p)
    for p in reversed(points):
        while len(upper) >= 2 and cross(upper[-2], upper[-1], p) <= 0:
            upper.pop()
        upper.append(p)
    return [Vector(p) for p in lower[:-1] + upper[:-1]]


def fit(rest, current):
    """Rotation angle, scale and the two centers of the similarity transform that maps the
    points `rest` closest onto `current`."""
    n = len(rest)
    c0 = sum(rest, Vector((0, 0))) / n
    c1 = sum(current, Vector((0, 0))) / n
    dot = cross = norm = 0.0
    for a, b in zip(rest, current):
        a, b = a - c0, b - c1
        dot += a.x * b.x + a.y * b.y
        cross += a.x * b.y - a.y * b.x
        norm += a.length_squared
    angle = math.atan2(cross, dot)
    scale = math.hypot(dot, cross) / norm if norm > 0 else 1.0
    return angle, scale, c0, c1


def project(fig):
    """Per direction: each part's flat outline (around its rest center) and color, and per
    animation the part's translation (with depth as z), angle and scale per frame."""
    rest = fig._pose(None, 0.0)
    views = []
    for d in range(DIRECTIONS):
        parts = []
        for (obj, color, bias), points in zip(fig.parts, rest):
            projected = [view_point(p, d)[0] for p in points]
            center = sum(projected, Vector((0, 0))) / len(projected)
            outline = [p - center for p in hull(projected)]
            parts.append({"name": obj.name, "color": color, "outline": outline})
        animations = {}
        for name, frames, pose in fig.animations:
            tracks = [[] for _ in fig.parts]
            for frame in range(frames + 1):
                posed = fig._pose(pose, 2 * math.pi * frame / frames)
                for i, ((_, _, bias), r, points) in enumerate(zip(fig.parts, rest, posed)):
                    a = [view_point(p, d)[0] for p in r]
                    b = [view_point(p, d)[0] for p in points]
                    angle, scale, _, center = fit(a, b)
                    centroid = sum(points, Vector((0, 0, 0))) / len(points)
                    depth = view_point(centroid, d)[1] + bias
                    z = min(max(DEPTH_OFFSET + depth * DEPTH_SCALE, 0.0), 0.001)
                    tracks[i].append((center.x, center.y, z, angle, scale))
            animations[name] = (frames, tracks)
        views.append({"parts": parts, "animations": animations})
    fig.reset()
    return views


def write_glb(fig, views):
    """The figure's views as a binary glTF."""
    blob = bytearray()
    buffer_views, accessors = [], []

    def accessor(values, fmt, kind, component, target=None, minmax=False):
        while len(blob) % 4:
            blob.append(0)
        start = len(blob)
        width = {"SCALAR": 1, "VEC3": 3, "VEC4": 4}[kind]
        for v in values:
            blob.extend(struct.pack("<" + fmt * width, *(v if width > 1 else (v,))))
        view = {"buffer": 0, "byteOffset": start, "byteLength": len(blob) - start}
        if target:
            view["target"] = target
        buffer_views.append(view)
        acc = {
            "bufferView": len(buffer_views) - 1,
            "componentType": component,
            "count": len(values),
            "type": kind,
        }
        if minmax:
            columns = list(zip(*values)) if width > 1 else [values]
            acc["min"] = [min(c) for c in columns]
            acc["max"] = [max(c) for c in columns]
        accessors.append(acc)
        return len(accessors) - 1

    color_names = list(fig.colors)
    materials = [
        {
            "name": name,
            "pbrMetallicRoughness": {
                "baseColorFactor": [*(linear(c) for c in fig.colors[name]), 1.0],
                "metallicFactor": 0.0,
            },
        }
        for name in color_names
    ]
    nodes = [{"name": "figure", "children": []}]
    meshes, animations = [], {}
    for d, view in enumerate(views):
        direction = len(nodes)
        nodes[0]["children"].append(direction)
        nodes.append({"name": f"d{d}", "children": []})
        part_nodes = []
        for i, part in enumerate(view["parts"]):
            outline = part["outline"]
            positions = [(p.x, p.y, 0.0) for p in outline]
            # a fan over the convex outline
            indices = [k for j in range(1, len(outline) - 1) for k in (0, j, j + 1)]
            meshes.append(
                {
                    "name": f"{part['name']}.d{d}",
                    "primitives": [
                        {
                            "attributes": {
                                "POSITION": accessor(positions, "f", "VEC3", 5126, 34962, True)
                            },
                            "indices": accessor(indices, "I", "SCALAR", 5125, 34963),
                            "material": color_names.index(part["color"]),
                        }
                    ],
                }
            )
            first = view["animations"][fig.animations[0][0]][1][i][0]
            node = len(nodes)
            nodes.append(
                {
                    "name": part["name"],
                    "mesh": len(meshes) - 1,
                    "translation": list(first[:3]),
                    "rotation": [0.0, 0.0, math.sin(first[3] / 2), math.cos(first[3] / 2)],
                    "scale": [first[4], first[4], 1.0],
                }
            )
            nodes[direction]["children"].append(node)
            part_nodes.append(node)
        for name, (frames, tracks) in view["animations"].items():
            times = accessor([f / FPS for f in range(frames + 1)], "f", "SCALAR", 5126, minmax=True)
            samplers, channels = [], []
            for node, track in zip(part_nodes, tracks):
                rotations, previous = [], None
                for t in track:
                    q = (0.0, 0.0, math.sin(t[3] / 2), math.cos(t[3] / 2))
                    # keep neighboring keyframes on the same side, so they turn the short way
                    if previous and q[2] * previous[2] + q[3] * previous[3] < 0:
                        q = tuple(-c for c in q)
                    rotations.append(q)
                    previous = q
                outputs = {
                    "translation": accessor([t[:3] for t in track], "f", "VEC3", 5126),
                    "rotation": accessor(rotations, "f", "VEC4", 5126),
                    "scale": accessor([(t[4], t[4], 1.0) for t in track], "f", "VEC3", 5126),
                }
                for path, output in outputs.items():
                    samplers.append({"input": times, "output": output, "interpolation": "LINEAR"})
                    channels.append(
                        {"sampler": len(samplers) - 1, "target": {"node": node, "path": path}}
                    )
            animations[f"{name}/{d}"] = {
                "name": f"{name}/{d}",
                "samplers": samplers,
                "channels": channels,
            }

    while len(blob) % 4:
        blob.append(0)
    document = {
        "asset": {"version": "2.0", "generator": "survive art/figures.py"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": buffer_views,
        "buffers": [{"byteLength": len(blob)}],
        "animations": list(animations.values()),
    }
    text = json.dumps(document, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    length = 12 + 8 + len(text) + 8 + len(blob)
    return (
        struct.pack("<III", 0x46546C67, 2, length)
        + struct.pack("<II", len(text), 0x4E4F534A)
        + text
        + struct.pack("<II", len(blob), 0x004E4942)
        + bytes(blob)
    )


def write_preview(fig, views, directory, cell=96, zoom=3.5):
    """An animated GIF per animation with all directions side by side, drawn from the same data
    the game gets."""
    import subprocess

    directory.mkdir(parents=True, exist_ok=True)
    for name, frames, _ in fig.animations:
        files = []
        for frame in range(frames):
            shapes = []
            for d, view in enumerate(views):
                ox = cell * (d + 0.5)
                oy = cell * 0.7
                tracks = view["animations"][name][1]
                order = sorted(range(len(view["parts"])), key=lambda i: tracks[i][frame][2])
                for i in order:
                    part = view["parts"][i]
                    x, y, _, angle, scale = tracks[i][frame]
                    c, s = math.cos(angle), math.sin(angle)
                    points = []
                    for p in part["outline"]:
                        px = x + scale * (c * p.x - s * p.y)
                        py = y + scale * (s * p.x + c * p.y)
                        points.append(f"{ox + zoom * px:.2f},{oy - zoom * py:.2f}")
                    rgb = ",".join(str(round(255 * v)) for v in fig.colors[part["color"]])
                    shapes.append(f'<polygon points="{" ".join(points)}" fill="rgb({rgb})"/>')
                # the collider's circle on the ground
                shapes.insert(
                    0,
                    f'<ellipse cx="{ox}" cy="{oy}" rx="{zoom * 5}" ry="{zoom * 5}" '
                    'fill="none" stroke="rgb(70,110,55)" stroke-width="1"/>',
                )
            svg = (
                f'<svg xmlns="http://www.w3.org/2000/svg" width="{cell * DIRECTIONS}" '
                f'height="{cell}"><rect width="100%" height="100%" fill="rgb(150,195,130)"/>'
                + "".join(shapes)
                + "</svg>"
            )
            path = directory / f"{fig.name}_{name}_{frame:02}.svg"
            path.write_text(svg)
            files.append(str(path))
        subprocess.run(
            ["magick", "-delay", str(round(100 / FPS)), "-loop", "0", *files,
             str(directory / f"{fig.name}_{name}.gif")],
            check=True,
        )
