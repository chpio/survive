# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A top-down 2D survival game built with Bevy 0.20, Avian2D 0.8 physics, and `bevy_ecs_tiled` 0.15 (with the `avian` feature) for Tiled maps. These three must stay on mutually compatible versions; check with `cargo outdated --root-deps-only`.

## Commands

```sh
cargo run          # run the game (assets are loaded from ./assets, hot-reloaded via file_watcher)
cargo check        # fast type check
cargo clippy       # lint
cargo fmt          # format
cargo bench --no-default-features   # Criterion benchmarks in benches/, see docs/architecture/benchmarks.md
cargo tracy        # release build with Bevy's spans sent to the Tracy profiler (alias in .cargo/config.toml)
```

`cargo tracy` enables `bevy/trace_tracy`: every system and schedule becomes a span in the Tracy profiler (version 0.14). Use it for the time of single systems, which the HUD's "Performance" view doesn't measure; the spans slow down the frame a bit.

Tests live in `tests/` (never inline) and run with `cargo test`; filter with `cargo test --test it enemy`. They form a single test binary `it` (`autotests = false`; `tests/main.rs` only declares the modules), so the game and its dependencies are linked once; the lib and binary have no test or bench targets of their own (`test = false`, `bench = false` in `Cargo.toml`). All tests run as threads of one process, so they must not share process-wide state (statics, environment variables, the current directory). Game code is in the `survival` lib crate (`src/lib.rs`) so tests can use it; `src/main.rs` only builds the `App`. `tests/common.rs` builds a headless physics app; `common::app_with` takes extra plugins, which can't be added once the app is finished.

Build notes:
- Bevy uses `default-features = false` with a hand-picked feature list in `Cargo.toml`. The commented-out lines mirror Bevy's default features. If a Bevy type or plugin is missing, the feature that provides it is probably commented out.
- Gizmos (physics debug outlines, flow field arrows) need both `bevy_gizmos` and `bevy_gizmos_render`; without the latter they are recorded but never drawn.
- The default feature `dev` turns on Bevy's `dynamic_linking` and `file_watcher`, in release builds too. A build to ship turns it off with `--no-default-features`; a dynamically linked executable can't be shipped.
- Windows builds cross-compile from Linux with MinGW (`mingw-w64-gcc` on Arch) and `rustup target add x86_64-pc-windows-gnu`: `cargo build --release --target x86_64-pc-windows-gnu --no-default-features`. The executable needs `assets/` next to it.
- Dev builds use `opt-level = 1` for the crate and `3` for dependencies.

Open work is collected in `TODO.md`; remove entries once they are done.

## Architecture

- `src/main.rs` builds the `App`: window/image settings, FPS overlay, Tiled + Avian plugins (with `Gravity::ZERO`, top-down; physics runs at Bevy's fixed 64 Hz and interpolates all bodies' `Transform`s in between, so code reading `Transform` sees positions up to one step behind the physics), and the game object plugins. `startup` spawns the camera, the player, and the `TiledWorld`.
- `src/objects/` holds one module per game object. Each module defines a marker `Component`, its systems, and a `pub struct Plugin` that registers them. New objects follow this pattern and get added to the plugin tuple in `main.rs`.

The details of each area are in `docs/architecture/`; read the area's file before changing it. The German concept docs in `docs/` explain the design behind it.

| Area | Code | Details | Concept |
|---|---|---|---|
| World, map loading, cell raster | `src/world.rs`, `src/cells.rs` | `world.md` | `welt.md` |
| Tiled map → ECS | observers in `src/main.rs` | `tiled.md` | `welt.md` |
| Flow field | `src/flowfield/` | `flowfield.md` | |
| Movement | `src/movement.rs` | `movement.md` | |
| Combat, swarm enemies | `src/combat/`, `src/objects/enemy/swarm/` | `combat.md` | `schwarm.md` |
| HUD | `src/ui/` | `ui.md` | `ui-events.md` |
| Items | `src/items/` | `items.md` | `items.md` |
| Sight | `src/sight/` | `sight.md` | `sicht.md` |
| Day and night, shadows, color grading | `src/light/` | `light.md` | `licht.md` |
| Spawning | `src/spawning/` | `spawning.md` | `spawnen.md` |
| Settings, performance measuring | `src/settings.rs`, `src/perf.rs` | `settings.md` | |
| Benchmarks | `benches/` | `benchmarks.md` | |
| Debug keys and HUD dev tools | | `debug.md` | |

### Rules across areas

- Physics runs at Bevy's fixed 64 Hz and interpolates all bodies' `Transform`s, so code reading `Transform` sees positions up to one step behind the physics.
- Movement is force-based: systems set a `DesiredVelocity` instead of changing a body's `Transform` or `LinearVelocity`; bodies are built with `movement::body`. Avian runs with `movement::LENGTH_UNIT` and `movement::CONTACT_TOLERANCE`, in `main.rs` and `tests/common.rs` alike.
- Values for balancing come from hot-reloaded RON assets (`hot_reload::RonLoader`, `RonAsset`) and are copied into resources, so tests insert the resources directly.
- Commands on enemies use `try_insert`/`try_remove` where combat may despawn them in the same frame.
- A tile layer's colliders are merged into one whose AABB spans the whole map: code dealing with obstacles uses the collider's shape, not its `ColliderAabb`.
- New map-driven objects get a type in Tiled and a match arm in the observers in `main.rs`; properties for tiles go on their layer (see `tiled.md`).
- Z order: map layers 2 apart (y-sorting moves chunks by up to 1.5), the darkening outside the sight at 8, items at 9, units at 10 (health bars up to 11.1), the light layer at 20; the HUD is drawn above everything.
- The HUD has no user-agent stylesheet (new kinds of elements may need an entry in the base section of `assets/ui/style.css`) and only paints again after input, a state change or a resize; anything else that changes its look has to set `needs_paint`.
- The `Settings` resource is the only source of truth for the player's settings.

Debug keys: WASD moves the player, Escape opens the settings menu; all keys and the HUD's dev tools are in `docs/architecture/debug.md`.
