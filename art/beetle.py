"""Builds the beetle all enemies are drawn as, with a walk cycle.

Run headless from the repository root:

    blender -b --factory-startup -P art/beetle.py -- [--preview DIR]

Writes `assets/characters/beetle.glb` for the game and `art/beetle.blend` to look at in Blender;
the script is the source. With `--preview`, writes an animated GIF per animation into DIR. The
conventions are in `art/figures.py`; the beetle is made for a collider radius of 5 px.
"""

import math
import sys
from pathlib import Path

from mathutils import Vector

sys.path.insert(0, str(Path(__file__).resolve().parent))
from figures import Figure, preview_dir  # noqa: E402

COLORS = {
    "shell": (0.72, 0.20, 0.18),
    "shell_dark": (0.50, 0.12, 0.11),
    "head": (0.38, 0.09, 0.08),
    "limb": (0.24, 0.07, 0.06),
}

Z = (0, 0, 1)

BONES = {
    "root": ((0, 0, 0), (1, 0, 0), None),
    "thorax": ((0.6, 0, 1.7), (2.4, 0, 1.7), "root"),
    "abdomen": ((0.6, 0, 1.8), (-1.0, 0, 1.8), "root"),
    "head": ((2.4, 0, 1.5), (3.6, 0, 1.5), "thorax"),
}
# legs: x of the pivot, angle (degrees from +X, left side) and length of the upper and the
# lower part, seen from above; the knee is raised, the foot on the ground
LEG_SHAPES = ((2.1, 55, 1.6, 20, 1.9), (1.3, 95, 1.7, 120, 1.9), (0.5, 140, 1.9, 165, 2.4))
LEGS = []
for side, s in (("l", 1), ("r", -1)):
    for i, (x, upper_angle, upper, lower_angle, lower) in enumerate(LEG_SHAPES):
        name = f"leg_{side}{i + 1}"
        pivot = Vector((x, 1.2 * s, 1.3))
        a = math.radians(upper_angle) * s
        knee = pivot + Vector((upper * math.cos(a), upper * math.sin(a), 0.7))
        b = math.radians(lower_angle) * s
        foot = Vector((knee.x + lower * math.cos(b), knee.y + lower * math.sin(b), 0.1))
        BONES[name] = (tuple(pivot), tuple(knee), "thorax")
        BONES[f"{name}_lower"] = (tuple(knee), tuple(foot), name)
        LEGS.append((name, s, i))
    BONES[f"antenna_{side}"] = ((3.9, 0.6 * s, 2.0), (6.0, 2.0 * s, 3.0), "head")
    BONES[f"mandible_{side}"] = ((4.3, 0.55 * s, 1.2), (5.3, 0.15 * s, 1.1), "head")


def build():
    fig = Figure("beetle", BONES, COLORS)
    fig.ellipsoid("abdomen", (-1.4, 0, 1.8), (3.2, 2.6, 1.5), "shell", "abdomen")
    # the seam between the wing covers, on top of the shell
    fig.capsule("seam", (-4.3, 0, 3.0), (0.5, 0, 3.2), 0.18, "shell_dark", "abdomen", bias=1.0)
    fig.ellipsoid("thorax", (1.5, 0, 1.7), (1.4, 1.9, 1.2), "shell_dark", "thorax")
    fig.ellipsoid("head", (3.4, 0, 1.5), (1.1, 1.2, 0.9), "head", "head")
    for name, _, _ in LEGS:
        for bone, radius in ((name, 0.3), (f"{name}_lower", 0.24)):
            head, tail, _ = BONES[bone]
            # under the shell, which rises above them; their centers alone would put the legs
            # of the near side on top of it
            fig.capsule(bone, head, tail, radius, "limb", bone, bias=-2.0)
    for side in "lr":
        for kind, radius in (("antenna", 0.15), ("mandible", 0.22)):
            head, tail, _ = BONES[f"{kind}_{side}"]
            fig.capsule(f"{kind}_{side}", head, tail, radius, "limb", f"{kind}_{side}")
    return fig


def walk(fig, phase):
    """Tripod gait: the left front, right middle and left rear legs swing together, the other
    three half a cycle later; a leg swinging forward is lifted."""
    for name, s, index in LEGS:
        tripod = (index + (s < 0)) % 2
        swing = math.sin(phase + math.pi * tripod)
        # forward is towards +X, which is clockwise for the left legs
        fig.turn(name, Z, -s * 16 * swing)
        lift = max(0.0, math.cos(phase + math.pi * tripod))
        fig.shift(f"{name}_lower", (0, 0, 0.5 * lift))
    fig.turn("thorax", Z, 3 * math.sin(phase))
    fig.turn("abdomen", Z, -4 * math.sin(phase))
    fig.turn("head", Z, -2 * math.sin(phase))
    for side, s in (("l", 1), ("r", -1)):
        fig.turn(f"antenna_{side}", Z, s * 8 * math.sin(phase + 0.8))
        fig.turn(f"mandible_{side}", Z, s * 10 * max(0.0, math.sin(phase * 2)))


fig = build()
fig.animate("walk", 16, walk)
fig.build(preview_dir())
