"""Builds the beetle all enemies are drawn as, out of rigid single-colored 2D parts on an armature, with a walk cycle.

Run headless from the repository root:

    blender -b --factory-startup -P art/beetle.py -- [--preview DIR]

Writes `assets/characters/beetle.glb` for the game and `art/beetle.blend` to look at
in Blender; the script is the source, so changes belong here, not in the `.blend`, which isn't
checked in. With `--preview`, renders the walk cycle's frames as PNGs into DIR.

Conventions for figures: top-down, 1 Blender unit = 1 px of the game world, the figure faces +X
(the game's heading at angle 0) and stands on the XY plane; Z only orders the parts (higher is
drawn on top, all within 0..1). The glTF keeps Blender's Z-up axes (`export_yup=False`), so the
game can use X and Y as they are.
"""

import math
import sys
from pathlib import Path

import bmesh
import bpy
from mathutils import Matrix, Quaternion, Vector

ROOT = Path(__file__).resolve().parent.parent
FPS = 24
# frames of one walk cycle, which loops
CYCLE = 16

# sRGB, like the game's colors
COLORS = {
    "shell": (0.72, 0.20, 0.18),
    "shell_dark": (0.50, 0.12, 0.11),
    "head": (0.38, 0.09, 0.08),
    "limb": (0.24, 0.07, 0.06),
}


def args():
    argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    preview = None
    if "--preview" in argv:
        preview = Path(argv[argv.index("--preview") + 1])
    return preview


def linear(c):
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def material(name):
    # Blender's colors are linear
    color = (*(linear(c) for c in COLORS[name]), 1.0)
    mat = bpy.data.materials.get(name) or bpy.data.materials.new(name)
    # the workbench preview shows diffuse_color, the glTF exports the base color
    mat.diffuse_color = color
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    bsdf.inputs["Base Color"].default_value = color
    bsdf.inputs["Roughness"].default_value = 1.0
    return mat


def ellipse(center, rx, ry, segments=24, angle=0.0):
    c, s = math.cos(angle), math.sin(angle)
    points = []
    for i in range(segments):
        t = 2 * math.pi * i / segments
        x, y = rx * math.cos(t), ry * math.sin(t)
        points.append((center[0] + c * x - s * y, center[1] + s * x + c * y))
    return points


def capsule(start, end, width, segments=8):
    """Outline of a stick from `start` to `end` with round ends."""
    start, end = Vector(start), Vector(end)
    direction = (end - start).normalized()
    angle = math.atan2(direction.y, direction.x)
    r = width / 2
    points = []
    for i in range(segments + 1):
        t = angle - math.pi / 2 + math.pi * i / segments
        points.append(end + r * Vector((math.cos(t), math.sin(t))))
    for i in range(segments + 1):
        t = angle + math.pi / 2 + math.pi * i / segments
        points.append(start + r * Vector((math.cos(t), math.sin(t))))
    return [tuple(p) for p in points]


def part(name, outline, z, color):
    """Flat mesh `name` with the polygon `outline`, at depth `z`, in world coordinates."""
    mesh = bpy.data.meshes.new(name)
    bm = bmesh.new()
    verts = [bm.verts.new((x, y, z)) for x, y in outline]
    bm.faces.new(verts)
    # triangulated, so the game gets plain triangles
    bmesh.ops.triangulate(bm, faces=bm.faces[:])
    bm.to_mesh(mesh)
    bm.free()
    mesh.materials.append(material(color))
    obj = bpy.data.objects.new(name, mesh)
    bpy.context.collection.objects.link(obj)
    return obj


# bone name: (head, tail, parent); bones lie on the XY plane
BONES = {
    "root": ((0, 0), (1, 0), None),
    "thorax": ((0.6, 0), (2.4, 0), "root"),
    "abdomen": ((0.6, 0), (-1.0, 0), "root"),
    "head": ((2.4, 0), (3.6, 0), "thorax"),
}
# legs: side (+1 left, -1 right), index from the front; each has an upper part from the thorax
# outwards and a lower part bent forwards (front legs) or backwards (the others)
LEGS = []
# x of the pivot, angle and length of the upper and lower part (degrees from +X, left side)
LEG_SHAPES = ((2.1, 55, 1.6, 20, 1.9), (1.3, 95, 1.7, 120, 1.9), (0.5, 140, 1.9, 165, 2.4))
for side, label in ((1, "l"), (-1, "r")):
    for i, (x, upper_angle, upper, lower_angle, lower) in enumerate(LEG_SHAPES):
        name = f"leg_{label}{i + 1}"
        pivot = Vector((x, 1.3 * side))
        a = math.radians(upper_angle) * side
        knee = pivot + upper * Vector((math.cos(a), math.sin(a)))
        b = math.radians(lower_angle) * side
        foot = knee + lower * Vector((math.cos(b), math.sin(b)))
        BONES[name] = (tuple(pivot), tuple(knee), "thorax")
        BONES[f"{name}_lower"] = (tuple(knee), tuple(foot), name)
        LEGS.append((name, side, i))
for side, label in ((1, "l"), (-1, "r")):
    BONES[f"antenna_{label}"] = ((3.9, 0.6 * side), (6.2, 2.0 * side), "head")
    BONES[f"mandible_{label}"] = ((4.4, 0.55 * side), (5.4, 0.15 * side), "head")


def build():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps = FPS
    scene.frame_start, scene.frame_end = 0, CYCLE - 1

    armature = bpy.data.armatures.new("beetle")
    rig = bpy.data.objects.new("beetle", armature)
    scene.collection.objects.link(rig)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")
    for name, (head, tail, parent) in BONES.items():
        bone = armature.edit_bones.new(name)
        bone.head = (*head, 0)
        bone.tail = (*tail, 0)
        bone.roll = 0
        if parent:
            bone.parent = armature.edit_bones[parent]
    bpy.ops.object.mode_set(mode="OBJECT")

    parts = [
        ("abdomen", ellipse((-1.4, 0), 3.2, 2.7), 0.3, "shell", "abdomen"),
        # the seam between the wing covers
        ("abdomen_seam", capsule((-4.0, 0), (0.6, 0), 0.25), 0.35, "shell_dark", "abdomen"),
        ("thorax", ellipse((1.5, 0), 1.5, 2.0), 0.4, "shell_dark", "thorax"),
        ("head", ellipse((3.5, 0), 1.2, 1.3), 0.5, "head", "head"),
    ]
    for name, _, _ in LEGS:
        for bone, width in ((name, 0.6), (f"{name}_lower", 0.45)):
            head, tail, _ = BONES[bone]
            parts.append((bone, capsule(head, tail, width), 0.1, "limb", bone))
    for label in "lr":
        for kind, width in (("antenna", 0.3), ("mandible", 0.45)):
            name = f"{kind}_{label}"
            head, tail, _ = BONES[name]
            parts.append((name, capsule(head, tail, width), 0.2, "limb", name))

    for name, outline, z, color, bone in parts:
        obj = part(name, outline, z, color)
        obj.parent = rig
        obj.parent_type = "BONE"
        obj.parent_bone = bone
        bpy.context.view_layer.update()
        # parenting to a bone moves the child to the bone's tail; keep the world position
        obj.matrix_world = Matrix.Identity(4)
    return rig


def turn(rig, bone_name, degrees):
    """Rotates a pose bone by `degrees` around the world's Z axis (counterclockwise)."""
    bone = rig.data.bones[bone_name]
    axis = bone.matrix_local.inverted().to_3x3() @ Vector((0, 0, 1))
    pose = rig.pose.bones[bone_name]
    pose.rotation_mode = "QUATERNION"
    pose.rotation_quaternion = Quaternion(axis, math.radians(degrees))


def animate_walk(rig):
    """Tripod gait: the left front, right middle and left rear legs swing together, the other three
    half a cycle later; the body sways a little against them."""
    for frame in range(CYCLE + 1):
        phase = 2 * math.pi * frame / CYCLE
        for name, side, index in LEGS:
            tripod = (index + (side < 0)) % 2
            swing = math.sin(phase + math.pi * tripod)
            # forward is towards +X, which is clockwise for the left legs; the lower part bends
            # in while the leg swings forward, as if lifted
            turn(rig, name, -side * 16 * swing)
            lift = math.cos(phase + math.pi * tripod)
            turn(rig, f"{name}_lower", side * 10 * max(0.0, lift))
        turn(rig, "thorax", 3 * math.sin(phase))
        turn(rig, "abdomen", -4 * math.sin(phase))
        turn(rig, "head", -2 * math.sin(phase))
        for side, label in ((1, "l"), (-1, "r")):
            turn(rig, f"antenna_{label}", side * 8 * math.sin(phase + 0.8))
            turn(rig, f"mandible_{label}", side * 10 * max(0.0, math.sin(phase * 2)))
        for pose in rig.pose.bones:
            pose.keyframe_insert("rotation_quaternion", frame=frame)
    rig.animation_data.action.name = "walk"


def export(rig):
    bpy.ops.wm.save_as_mainfile(filepath=str(ROOT / "art" / "beetle.blend"))
    out = ROOT / "assets" / "characters" / "beetle.glb"
    out.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(
        filepath=str(out),
        export_format="GLB",
        export_yup=False,
        export_animations=True,
        export_cameras=False,
        export_lights=False,
    )


def render_preview(directory):
    scene = bpy.context.scene
    camera = bpy.data.objects.new("camera", bpy.data.cameras.new("camera"))
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = 16
    camera.location = (0.5, 0, 10)
    scene.collection.objects.link(camera)
    scene.camera = camera
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "FLAT"
    scene.display.shading.color_type = "MATERIAL"
    scene.display.shading.background_type = "VIEWPORT"
    scene.display.shading.background_color = (0.36, 0.55, 0.28)
    scene.render.resolution_x = scene.render.resolution_y = 320
    scene.render.film_transparent = False
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("world")
    world.color = (0.36, 0.55, 0.28)
    scene.world = world
    directory.mkdir(parents=True, exist_ok=True)
    for frame in range(CYCLE):
        scene.frame_set(frame)
        scene.render.filepath = str(directory / f"walk_{frame:02}.png")
        bpy.ops.render.render(write_still=True)


preview = args()
rig = build()
animate_walk(rig)
export(rig)
if preview:
    render_preview(preview)
