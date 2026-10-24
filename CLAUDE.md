# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

A top-down 2D survival game built with Bevy 0.20, Avian2D 0.8 physics, and `bevy_ecs_tiled` 0.14 (with the `avian` feature) for Tiled maps. These three must stay on mutually compatible versions; check with `cargo outdated --root-deps-only`.

## Commands

```sh
cargo run          # run the game (assets are loaded from ./assets, hot-reloaded via file_watcher)
cargo check        # fast type check
cargo clippy       # lint
cargo fmt          # format
```

There are no tests yet.

Build notes:
- Bevy uses `default-features = false` with a hand-picked feature list in `Cargo.toml`. The commented-out lines mirror Bevy's default features. If a Bevy type or plugin is missing, the feature that provides it is probably commented out.
- The fps overlay needs the `render_dev_tools` feature, which also adds `RenderDebugOverlayPlugin` to `DefaultPlugins`. That plugin requires `bevy_pbr`, so `main.rs` disables it.
- The `dynamic_linking` feature is always on, including in release builds.
- Dev builds use `opt-level = 1` for the crate and `3` for dependencies.

## Architecture

- `src/main.rs` builds the `App`: window/image settings, FPS overlay, Tiled + Avian plugins (with `Gravity::ZERO`, top-down), and the game object plugins. `startup` spawns the camera, the player, and the `TiledMap`.
- `src/objects/` holds one module per game object. Each module defines a marker `Component`, its systems, and a `pub struct Plugin` that registers them. New objects follow this pattern and get added to the plugin tuple in `main.rs`.
- Movement is velocity-based. Systems set `LinearVelocity` on Avian bodies (player: `RigidBody::Dynamic`, enemies: `RigidBody::Kinematic` + `Sensor`) instead of changing `Transform` directly. The camera follows the player's `Transform` and keeps its own z.

### Tiled map → ECS

- `assets/map.tmx` (16×16 tiles, tileset `assets/grass_biome.tsx`) is loaded in `startup`.
- Colliders defined in the tileset or map are created by `TiledPhysicsPlugin`. An observer on `TiledEvent<ColliderCreated>` makes each one `RigidBody::Static`, then reads the object's Tiled `type` (or the tile's type as a fallback) to attach game components. For example, `type="well"` (tile 197 in the tileset) becomes `Well + Sensor + CollisionEventsEnabled`.
- To add a new map-driven object: give the tile or object a type in Tiled, then add a match arm in that observer.
- Collision handling reads Avian's `CollisionStart` messages (via `MessageReader`; Bevy's buffered events are called messages). Only colliders with `CollisionEventsEnabled` emit them.

### Debug controls

- WASD: move the player.
- E: spawn an enemy.
- P: toggle the FPS overlay.
- F: toggle borderless fullscreen.

`PhysicsDebugPlugin` is always on and draws collider outlines.
