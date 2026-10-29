# TODO

## Performance

- Low priority, only if the flow field's search turns out to be costly: replace the `BinaryHeap`
  in `Grid::compute_distances` with a bucket queue (Dial's algorithm). Steps cost only 10 or 14, so
  the waiting distances always lie within 15 consecutive values; a ring of 15 `Vec`s indexed by
  `distance % 15` makes push and pop O(1), without the heap's sifting and mispredicted branches.
  The distances stay the same.

- Optional FPS limit in the settings, e.g. as a power saving mode. `bevy_framepace` isn't available
  for Bevy 0.20, so build it like it does: sleep at the end of the render world's frame until the
  next frame is due.

## Gameplay

- Balance the combat values in `player::combat()` and `Enemy::combat`: five enemies touching the
  player kill it within a few seconds.

## HUD

- Hotbar for the inventory: a bar of slots along the bottom edge of the screen, always visible,
  with items the player assigns to it and keys (e.g. 1–9) to use them. The inventory window stays
  for the full list.

## Input

- Pens under Wayland: the compositor only sends them through the tablet protocol, which winit
  0.30 (Bevy 0.20) doesn't support, so the game gets no events at all (under X11 they arrive as
  mouse input). Check again with the next Bevy/winit update, which may bring a pointer API with
  pen support; until then, the only workaround would be running under XWayland.

## Map

- Give the tileset's buildings, gravestones and bridge railings colliders before they are used on
  the map.

## Releases

- Build release executables in CI on `v*` tags and attach them to the release: Linux natively,
  Windows cross-compiled with `x86_64-pc-windows-gnu` (MinGW and the target added to the CI image).
  Before that:
  - Move `dynamic_linking` and `file_watcher` into a `dev` feature (with a cargo alias so
    `cargo run` stays the same), since a dynamically linked executable can't be shipped.
  - Embed all assets in release builds, so the executable is a single file: a `build.rs` lists the
    files in `assets/` as `include_bytes!`, and outside the `dev` feature they replace the default
    asset source through Bevy's `MemoryAssetReader` (`bevy_embedded_assets` doesn't support Bevy
    0.20 yet). Check that `bevy_ecs_tiled` still finds the tileset and its image.
  - Hide the console window on Windows with `#![windows_subsystem = "windows"]` in release builds.
  - Try the Windows build locally first.

## Code

- Let combat trigger a `Died` event instead of handling the deaths of players and enemies itself, so
  `combat` doesn't need to know about the objects. The combat tests would need the observers then.
- Bodies sliding along an edge of the map's triangulated colliders are still pushed off its end
  by up to 0.8 px, depending on where they pass the corner within a physics step
  (`body_slides_past_corner_of_triangulated_collider` passes only for some starting points).
