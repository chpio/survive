# TODO

## Performance

- Compute the flow field less often or with less work: larger cells (8 instead of 4 px would close
  corridors one tile wide), or only after the player moved a few cells. Measure first with the HUD's
  "Performance" tree.

- Optional FPS limit in the settings, e.g. as a power saving mode. `bevy_framepace` isn't available
  for Bevy 0.20, so build it like it does: sleep at the end of the render world's frame until the
  next frame is due.

## Gameplay

- Ignore game input (spawn keys E / R / T, the HUD's spawn button) while the settings menu pauses
  the game; enemies spawned then only appear frozen until it is closed.

- Balance the combat values in `player::combat()` and `Enemy::combat`: five enemies touching the
  player kill it within a few seconds.

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
- Optionally check the line of sight with a margin of a cell: the straight way to the player may
  pass up to about 2.5 px closer to a corner than the clearance. Since bodies have no friction,
  enemies slide off instead of getting stuck, though.
