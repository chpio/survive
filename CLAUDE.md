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

Tests live in `tests/` (never inline) and run with `cargo test`. Game code is in the `survival` lib crate (`src/lib.rs`) so tests can use it; `src/main.rs` only builds the `App`.

Build notes:
- Bevy uses `default-features = false` with a hand-picked feature list in `Cargo.toml`. The commented-out lines mirror Bevy's default features. If a Bevy type or plugin is missing, the feature that provides it is probably commented out.
- The fps overlay needs the `render_dev_tools` feature, which also adds `RenderDebugOverlayPlugin` to `DefaultPlugins`. That plugin requires `bevy_pbr`, so `main.rs` disables it.
- Gizmos (physics debug outlines, flow field arrows) need both `bevy_gizmos` and `bevy_gizmos_render`; without the latter they are recorded but never drawn.
- The `dynamic_linking` feature is always on, including in release builds.
- Dev builds use `opt-level = 1` for the crate and `3` for dependencies.

## Architecture

- `src/main.rs` builds the `App`: window/image settings, FPS overlay, Tiled + Avian plugins (with `Gravity::ZERO`, top-down), and the game object plugins. `startup` spawns the camera, the player, and the `TiledMap`.
- `src/objects/` holds one module per game object. Each module defines a marker `Component`, its systems, and a `pub struct Plugin` that registers them. New objects follow this pattern and get added to the plugin tuple in `main.rs`.
- `src/flowfield/` keeps one grid per clearance in `flowfield::CLEARANCES` over the map (4 px cells); enemies of any radius (`Enemy::radius`) use the grid with the smallest clearance at least their radius. Cells closer than the clearance to a static, non-sensor collider are blocked, and a Dijkstra search from the player's cell gives each free cell its walking distance. Enemies with a free line of sight walk straight at the player, the others follow the field via `FlowField::direction` and steer away from each other while their personal spaces (`enemy::Separation`, radius and strength per enemy) overlap. `flowfield/debug.rs` draws one grid as an overlay (blocked cells, distances, field directions); `FlowFieldDebug::cycle` switches between the grids.
- Movement is force-based (`src/movement.rs`). Systems set a `DesiredVelocity` on Avian bodies (player and enemies: `RigidBody::Dynamic` with `Mass` and `Strength`) instead of changing `Transform` or `LinearVelocity` directly. In `FixedUpdate`, a spring towards an anchor moving at that velocity applies a force limited by `Strength`, so heavier and stronger bodies win when pushing each other. Rotation stays locked for the physics; enemies turn towards their `enemy::Heading` (the direction their navigation wants to walk, unaffected by knockback) by setting Avian's `Rotation` directly (`enemy::face_heading`, limited by `TURN_SPEED`). The camera follows the player's `Transform` and keeps its own z.

### Tiled map → ECS

- `assets/map.tmx` (16×16 tiles, tileset `assets/grass_biome.tsx`) is loaded in `startup`.
- Colliders defined in the tileset or map are created by `TiledPhysicsPlugin`. An observer on `TiledEvent<ColliderCreated>` makes each one `RigidBody::Static`, then reads the object's Tiled `type` (or the tile's type as a fallback) to attach game components. For example, `type="well"` (tile 197 in the tileset) becomes `Well + Sensor + CollisionEventsEnabled`.
- The tile colliders of a layer are merged into one collider whose AABB spans the whole map, so code dealing with obstacles must use the collider's shape, not its `ColliderAabb` (see `Grid::block_collider`). The map uses the `Triangulation` backend, so the collider is filled as in Tiled; the default `Polyline` backend would only keep the outlines.
- To add a new map-driven object: give the tile or object a type in Tiled, then add a match arm in that observer.
- Collision handling reads Avian's `CollisionStart` messages (via `MessageReader`; Bevy's buffered events are called messages). Only colliders with `CollisionEventsEnabled` emit them.

### Debug controls

- WASD: move the player.
- E / R / T: spawn a small / medium / large enemy.
- P: toggle the FPS overlay.
- F: toggle borderless fullscreen.

The HUD button "Flowfield" cycles the flow field overlay through the grids (5, 8, 12, 16 px clearance) and off.

`PhysicsDebugPlugin` draws collider outlines. They start hidden; the HUD checkbox "Kollider" shows them.
