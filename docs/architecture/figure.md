# Figures

Figures (`src/figure.rs`) are animated characters made of rigid, single-colored 2D parts on a skeleton (the decision is in `docs/ideen/ideen.md`, section "Grafik"). They are built and animated in Blender by scripts in `art/` (run headless with `blender -b --factory-startup -P art/<name>.py`; the script is the source, the generated `.blend` isn't checked in) and exported as `.glb` into `assets/characters/`.

Conventions: 1 Blender unit is 1 px of the game world, a figure faces +X (heading angle 0) and lies on the XY plane, and Z only orders its parts (within 0..1, above the unit's own z of 10 and below the health bars). The glTF keeps Blender's Z-up axes (`export_yup=False`). Every node may only rotate around Z and must keep its Z scale; the loader rejects other files.

Bevy's own glTF loader would pull in its 3D renderer, so `figure::FigureLoader` reads the `.glb` with the `gltf` crate: `figure::parse` turns it into plain data (nodes with parents before children, parts with their color, depth and vertices, animations as keyframes per node and property; tests use it directly), and the loader turns that into a `figure::Figure` asset with a mesh per part, an `AnimationClip` per animation and one `AnimationGraph`. A part's depth is moved from its vertices into its entity, where Bevy's 2D sorting needs it. Animation targets are identified by the path of node names from the top, since a bone and the part on it share a name.

`FigureRoot(handle)` on an entity (with `Transform` and `Visibility`) spawns the figure's nodes as its children once the asset is loaded, each part as a `Mesh2d` with a `SightMaterial` in its glTF base color (one material per color), and gives the entity an `AnimationPlayer` that loops the first animation. Saving the `.glb` again despawns and rebuilds its figures (hot reload).

Swarm enemies are beetles (`art/small_enemy.py`): their `enemy::Body` carries the `FigureRoot`, scaled by the enemy's radius over 5 px, and `face_heading` turns it. `enemy::animate_walk` plays the walk animation as fast as the enemy moves (normal pace at 40 px/s, at most four times that), so it stops with the enemy. The other enemies are still circles.
