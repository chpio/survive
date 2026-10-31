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

- Balance the game's values together, once enemies have real sprites (their size on screen
  affects how distances and speeds feel):
  - Combat: `player::combat()` and `Enemy::combat` (damage, reach, knockback, cooldown, health).
    Five enemies touching the player kill it within a few seconds.
  - Speeds: the player's and the enemies' `MOVE_SPEED`, `enemy::TURN_SPEED`, and the masses and
    strengths (`player::MASS`, `player::STRENGTH`, `Enemy::physics`), which decide who pushes whom.
  - Distances around the player: the sight radius (`assets/light.ron`, 200 px by day, 120 px at
    night, at most `sight::MAX_SIGHT_RADIUS`), `flowfield::FLOW_FIELD_RADIUS`
    (320 px), `enemy::DEACTIVATION_RADIUS` (400 px), `world::MIN_LOADED_DISTANCE` (340 px), the
    spawn distances (`ambush::MIN_DISTANCE`, `out_of_sight::MIN_DISTANCE` / `MAX_DISTANCE`),
    `items::input::PICKUP_REACH` and `enemy::Separation`. Awake enemies between the loaded distance
    and the deactivation radius can stand on an unloaded map and are despawned instead of falling
    asleep, which is fine for now.
  - Item effects in `assets/items.ron` (healing of the potion and the apple).

- Swarm enemies (`docs/schwarm.md`): release an attack token only a fixed time after the approach
  started (about 2 s) or at the end of the retreat, instead of when the retreat starts. Otherwise
  enemies hold a token only for the approach (about 0.5 s), and with many enemies circling the
  damage per second grows with their number after all (see „Token zu früh frei“).

## Input

- Rebindable keys: the bindings (WASD, Tab, 1–5, the debug keys) in one place, e.g. a resource
  loaded from and saved to the settings, with a section in the settings menu to change them. The
  systems read the binding instead of a fixed `KeyCode`.

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
