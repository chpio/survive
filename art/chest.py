"""Builds a wooden chest with iron bands, with an animation that opens its lid.

Run headless from the repository root:

    blender -b --factory-startup -P art/chest.py -- [--preview DIR]

Writes `assets/characters/chest.glb` for the game and `art/chest.blend` to look at in Blender;
the script is the source. With `--preview`, writes an animated GIF per animation into DIR. The
conventions are in `art/figures.py`; the chest's front faces +X, its lid hinges at the back.

`open` runs once from closed to open: its first frame is the closed chest, its last the open one,
so the game plays it without looping and holds the last frame.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from figures import Figure, preview_dir  # noqa: E402

COLORS = {
    "wood": (0.55, 0.35, 0.18),
    "wood_dark": (0.42, 0.26, 0.13),
    "inside": (0.16, 0.10, 0.06),
    "iron": (0.32, 0.32, 0.34),
    "brass": (0.80, 0.64, 0.25),
}

Y = (0, 1, 0)

# depth along X (front at +X), width along Y, the body's top at z = BODY
DEPTH = 8.0
WIDTH = 12.0
BODY = 6.0
LID = 2.5
HINGE = (-DEPTH / 2, 0, BODY)
# opening angle; a negative turn around Y lifts the front of the lid
OPEN = 105.0
PLANKS = 4

BONES = {
    "root": ((0, 0, 0), (0, 0, 1), None),
    "body": ((0, 0, 0), (0, 0, BODY), "root"),
    "lid": (HINGE, (HINGE[0], 0, BODY + LID), "body"),
}


def build():
    fig = Figure("chest", BONES, COLORS)
    fig.box("body", (0, 0, BODY / 2), (DEPTH, WIDTH, BODY), "wood", "body")
    # the dark floor inside, just below the rim; the closed lid covers it
    fig.box("inside", (0, 0, BODY - 0.3), (DEPTH - 1.2, WIDTH - 1.2, 0.4), "inside", "body",
            bias=-0.2)
    # the lid is made of planks across the chest: a part only moves, turns and scales evenly, so a
    # single box couldn't flatten as the lid swings towards the viewer, but narrow planks barely
    # need to
    depth = DEPTH / PLANKS
    for i in range(PLANKS):
        x = -DEPTH / 2 + depth * (i + 0.5)
        color = "wood_dark" if i % 2 else "wood"
        fig.box(f"plank_{i}", (x, 0, BODY + LID / 2), (depth, WIDTH, LID), color, "lid")
    for name, y in (("l", WIDTH / 3), ("r", -WIDTH / 3)):
        # iron fittings on the front and back of the body
        for side, x in (("front", DEPTH / 2), ("back", -DEPTH / 2)):
            fig.box(f"{side}_band_{name}", (x * 1.02, y, BODY / 2), (0.3, 1.0, BODY), "iron",
                    "body")
    # the lock on the front of the body and the hasp on the lid's front edge above it
    fig.box("lock", (DEPTH / 2 + 0.2, 0, BODY - 1.2), (0.5, 1.6, 1.8), "brass", "body", bias=0.5)
    fig.box("hasp", (DEPTH / 2 + 0.2, 0, BODY + 0.6), (0.5, 1.0, 1.4), "iron", "lid", bias=0.5)
    return fig


def ease_out_back(t, overshoot=1.6):
    """0 to 1, shooting a little past 1 before settling."""
    t -= 1
    return 1 + (overshoot + 1) * t**3 + overshoot * t**2


def open_lid(fig, phase):
    t = phase / (2 * math.pi)
    # the lid gives a small jolt as the lock springs, then swings open and settles
    lift = min(t / 0.75, 1.0)
    angle = OPEN * ease_out_back(lift)
    fig.turn("lid", Y, -angle)
    jolt = math.sin(math.pi * min(t / 0.15, 1.0))
    fig.shift("body", (0, 0, 0.3 * jolt))


fig = build()
fig.animate("open", 20, open_lid)
fig.build(preview_dir())
