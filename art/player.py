"""Builds the player's figure: a survivor with a backpack, with an idle animation (breathing) and a
walk cycle.

Run headless from the repository root:

    blender -b --factory-startup -P art/player.py -- [--preview DIR]

Writes `assets/characters/player.glb` for the game and `art/player.blend` to look at in Blender;
the script is the source. With `--preview`, writes an animated GIF per animation into DIR. The
conventions are in `art/figures.py`.
"""

import math
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from figures import Figure, preview_dir  # noqa: E402

COLORS = {
    "skin": (0.87, 0.67, 0.52),
    "hair": (0.33, 0.21, 0.12),
    "eye": (0.12, 0.09, 0.08),
    "shirt": (0.22, 0.46, 0.66),
    "sleeve": (0.18, 0.39, 0.57),
    "pants": (0.30, 0.29, 0.25),
    "boot": (0.20, 0.16, 0.13),
    "pack": (0.47, 0.33, 0.18),
    "strap": (0.33, 0.22, 0.11),
}

Y = (0, 1, 0)
Z = (0, 0, 1)

# bone: (head, tail, parent); the figure faces +X, left is +Y, up is +Z, the feet stand at z = 0
BONES = {
    "root": ((0, 0, 0), (0, 0, 1), None),
    "hips": ((0, 0, 6), (0, 0, 7), "root"),
    "torso": ((0, 0, 6), (0, 0, 12), "hips"),
    "head": ((0, 0, 12.5), (0, 0, 17), "torso"),
}
for side, s in (("l", 1), ("r", -1)):
    BONES[f"leg_{side}"] = ((0, 1.1 * s, 6), (0, 1.1 * s, 1), "hips")
    BONES[f"foot_{side}"] = ((0, 1.1 * s, 1), (1, 1.1 * s, 0.5), f"leg_{side}")
    BONES[f"arm_{side}"] = ((0, 2.9 * s, 11.2), (0, 3.1 * s, 8.6), "torso")
    BONES[f"forearm_{side}"] = ((0, 3.1 * s, 8.6), (0.3, 3.1 * s, 6.2), f"arm_{side}")


def build():
    fig = Figure("player", BONES, COLORS)
    for side, s in (("l", 1), ("r", -1)):
        fig.capsule(f"leg_{side}", (0, 1.1 * s, 5.8), (0, 1.1 * s, 1.2), 0.8, "pants", f"leg_{side}")
        fig.box(f"foot_{side}", (0.5, 1.1 * s, 0.5), (2.4, 1.3, 1.0), "boot", f"foot_{side}")
        # arms hang at the sides; the bias keeps them in front of the torso when seen from the
        # front or back, where their centers are as deep as its center
        fig.capsule(f"arm_{side}", (0, 2.9 * s, 11), (0, 3.1 * s, 8.6), 0.8, "sleeve",
                    f"arm_{side}", bias=0.5)
        fig.capsule(f"forearm_{side}", (0, 3.1 * s, 8.6), (0.3, 3.1 * s, 6.7), 0.6, "skin",
                    f"forearm_{side}", bias=0.5)
        fig.ellipsoid(f"hand_{side}", (0.35, 3.1 * s, 6.1), (0.7, 0.6, 0.7), "skin",
                      f"forearm_{side}", bias=0.6)
        fig.capsule(f"strap_{side}", (1.3, 1.5 * s, 11.3), (1.35, 1.5 * s, 7.8), 0.35, "strap",
                    "torso", bias=0.2)
        fig.ellipsoid(f"eye_{side}", (1.95, 0.8 * s, 14.9), (0.3, 0.35, 0.45), "eye", "head")
    fig.ellipsoid("torso", (0, 0, 9.2), (1.5, 2.4, 3.2), "shirt", "torso")
    # behind the torso when seen from the side, where its center is as deep as the torso's
    fig.box("pack", (-1.9, 0, 9.3), (1.6, 3.4, 4.0), "pack", "torso", bias=-1.0)
    fig.ellipsoid("head", (0, 0, 14.6), (2.3, 2.3, 2.4), "skin", "head")
    # set back and up, so the face shows in front of it
    fig.ellipsoid("hair", (-1.0, 0, 14.8), (2.35, 2.4, 2.3), "hair", "head")
    return fig


def idle(fig, phase):
    breath = math.sin(phase)
    fig.shift("torso", (0, 0, 0.15 * breath))
    fig.turn("head", Y, 2 * breath)
    for side, s in (("l", 1), ("r", -1)):
        fig.turn(f"arm_{side}", (1, 0, 0), 2 * breath * s)
        fig.turn(f"forearm_{side}", Y, -8)


def walk(fig, phase):
    swing = math.sin(phase)
    for side, s in (("l", 1), ("r", -1)):
        # a negative turn around Y moves a hanging limb forwards; the left leg steps forward
        # while the left arm swings back
        angle = -25 * swing * s
        fig.turn(f"leg_{side}", Y, angle)
        # the foot stays flat on the ground
        fig.turn(f"foot_{side}", Y, -angle)
        fig.turn(f"arm_{side}", Y, -0.8 * angle)
        fig.turn(f"forearm_{side}", Y, -15)
    # the hips are lowest when both feet touch the ground, twice per cycle
    fig.shift("hips", (0, 0, -0.3 * (1 + math.cos(2 * phase)) / 2))
    fig.turn("torso", Z, -4 * swing)
    fig.turn("head", Z, 3 * swing)


fig = build()
fig.animate("idle", 48, idle)
fig.animate("walk", 16, walk)
fig.build(preview_dir())
