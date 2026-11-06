# TODO

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
  - Swarm enemies in `assets/swarm.ron` (`docs/schwarm.md`): damage, speeds, timings, the
    number of tokens and how long they are held (`token_time`). The circle distance went down from
    60–80 px to 15–20 px, since the swarm stayed too far away, and up again to 25–30 px; check how
    close it should circle once enemies have sprites.

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
  Decide whether to ship x86-64-v3 builds (profile `release-v3`, see CLAUDE.md) alongside or
  instead of the baseline ones; measure first how much they gain (`cargo bench`, frame time with
  many enemies).
  Before that:
  - Embed all assets in release builds, so the executable is a single file: a `build.rs` lists the
    files in `assets/` as `include_bytes!`, and outside the `dev` feature they replace the default
    asset source through Bevy's `MemoryAssetReader` (`bevy_embedded_assets` doesn't support Bevy
    0.20 yet). Check that `bevy_ecs_tiled` still finds the tileset and its image.
  - Run the Windows build on Windows (or under Wine); it cross-compiles and links, see CLAUDE.md.

## Code

- Bodies sliding along an edge of the map's triangulated colliders are still pushed off its end
  by up to 0.8 px, depending on where they pass the corner within a physics step
  (`body_slides_past_corner_of_triangulated_collider` passes only for some starting points).
