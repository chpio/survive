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

Tests live in `tests/` (never inline) and run with `cargo test`; filter with `cargo test --test it enemy`. They form a single test binary `it` (`autotests = false`; `tests/main.rs` only declares the modules), so the game and its dependencies are linked once; the lib and binary have no test targets of their own (`test = false` in `Cargo.toml`). All tests run as threads of one process, so they must not share process-wide state (statics, environment variables, the current directory). Game code is in the `survival` lib crate (`src/lib.rs`) so tests can use it; `src/main.rs` only builds the `App`. `tests/common.rs` builds a headless physics app; `common::app_with` takes extra plugins, which can't be added once the app is finished.

Build notes:
- Bevy uses `default-features = false` with a hand-picked feature list in `Cargo.toml`. The commented-out lines mirror Bevy's default features. If a Bevy type or plugin is missing, the feature that provides it is probably commented out.
- The fps overlay needs the `render_dev_tools` feature, which also adds `RenderDebugOverlayPlugin` to `DefaultPlugins`. That plugin requires `bevy_pbr`, so `main.rs` disables it.
- Gizmos (physics debug outlines, flow field arrows) need both `bevy_gizmos` and `bevy_gizmos_render`; without the latter they are recorded but never drawn.
- The `dynamic_linking` feature is always on, including in release builds.
- Dev builds use `opt-level = 1` for the crate and `3` for dependencies.

Open work is collected in `TODO.md`; remove entries once they are done.

## Architecture

- `src/main.rs` builds the `App`: window/image settings, FPS overlay, Tiled + Avian plugins (with `Gravity::ZERO`, top-down; physics runs at Bevy's fixed 64 Hz and interpolates all bodies' `Transform`s in between, so code reading `Transform` sees positions up to one step behind the physics), and the game object plugins. `startup` spawns the camera, the player, and the `TiledMap`.
- `src/objects/` holds one module per game object. Each module defines a marker `Component`, its systems, and a `pub struct Plugin` that registers them. New objects follow this pattern and get added to the plugin tuple in `main.rs`.
- `src/flowfield/` keeps one grid per clearance in `flowfield::CLEARANCES` over the map (4 px cells); enemies of any radius (`Enemy::radius`) use the grid with the smallest clearance at least their radius. Cells closer than the clearance to a static, non-sensor collider are blocked, and a Dijkstra search from the player's cell gives each free cell its walking distance. Blocked cells within the clearance around the player that aren't inside a collider stay walkable, so the player is reachable while standing closer to an obstacle than the clearance. Enemies with a free line of sight walk straight at the player, the others follow the field via `FlowField::direction` and steer away from each other while their personal spaces (`enemy::Separation`, radius and strength per enemy) overlap. `flowfield/debug.rs` draws one grid as an overlay (blocked cells, distances, field directions); `FlowFieldDebug::cycle` switches between the grids.
- Movement is force-based (`src/movement.rs`). Systems set a `DesiredVelocity` on Avian bodies (player and enemies: `RigidBody::Dynamic` with `Mass` and `Strength`) instead of changing `Transform` or `LinearVelocity` directly. In `FixedUpdate`, a spring towards an anchor moving at that velocity applies a force limited by `Strength`, so heavier and stronger bodies win when pushing each other. Avian runs with `movement::LENGTH_UNIT` (16 px, one tile) and `movement::CONTACT_TOLERANCE`, in `main.rs` and `tests/common.rs` alike; with a larger contact tolerance, bodies sliding along the map's triangulated colliders get pushed away at corners. Bodies are built with `movement::body`, have no friction (so they slide along obstacles instead of sticking at corners) and never rotate; an enemy's visible `enemy::Body` is a child that turns towards its `enemy::Heading` (the direction its navigation wants to walk, unaffected by knockback; `enemy::face_heading`, limited by `TURN_SPEED`), so other children like health bars stay upright. All enemies share one unit-circle mesh and their materials (`EnemyAssets`), scaled to their radius. The camera follows the player's `Transform` and keeps its own z.
- Combat (`src/combat/`): units with `Health` and an `Attack` hit every opponent whose collider is within the attack's reach once per cooldown (the player all enemies around it, an enemy the player). A hit deals damage and knocks the target back with an impulse, so lighter units fly further. Dead enemies are despawned; when the player dies, all enemies are removed and the player is healed. `player::combat()` and `Enemy::combat` hold the values. `combat::health_bar` draws sprite bars above damaged units and is kept apart from the logic, since it needs rendering.
- The HUD (`src/ui/`) is a Dioxus Native document (Blitz, painted with Vello) rendered into a fullscreen image. Its styles are in `assets/ui/style.css`, loaded as a `Stylesheet` asset and hot-reloaded like the other assets; the HUD stays empty until it is loaded. `sync_state` writes the game state into one `hud::State` signal and handles the `hud::Action`s the HUD sends back over a channel. The HUD is only painted and rendered again after input, a state change, a resize or a redraw request from Blitz; anything else that should change its look (e.g. animations) has to set `needs_paint`. Blitz gets all mouse input; where the topmost element or an ancestor has the class `blocks-pointer` (`ui::blocks_pointer`), the picking backend `pick_hud` reports a hit on the HUD's image (`ui::Hud`) above the sprites, so picking in the game world only happens elsewhere (`docs/ui-events.md`). Bevy's own `ui_picking` is off, since it would hit the fullscreen image everywhere.
- Settings (`src/settings.rs`): the `Settings` resource (VSync, fullscreen, camera zoom) is the only source of truth. It is loaded from `settings.ron` when the plugin is built (so the window opens in the right mode), applied to the primary window and the 2D camera's projection scale whenever it changes, and saved then. The file lives next to the executable (`target/debug/` under `cargo run`). Escape toggles the `Menu` state; while it is `Open`, `Time<Virtual>` is paused, which stops physics and everything driven by game time. The HUD shows the menu and sends the whole changed `Settings` back as a `hud::Action`.
- `src/perf.rs` measures the time per frame of the game's main steps. Most steps are `perf::Step` system sets; marker systems before and after them record the wall time, and measured steps within one schedule run one after another so their times don't include each other. Steps inside a single system (the HUD's update, layout and scene in `paint_ui`) are recorded with `Timings::add`. Put new costly systems into a step (or add one, and list it in `perf::MAIN_THREAD` or `RENDER_THREAD`) to see them in the HUD.

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
- Alt+Enter: toggle borderless fullscreen (a setting, so it is saved).
- Escape: open/close the settings menu, which pauses the game.

The HUD button "Gegner spawnen" spawns a small enemy, "Flowfield" cycles the flow field overlay through the grids (5, 8, 12, 16 px clearance) and off.

The HUD checkbox "Performance" shows the time per frame of each `perf::Step` as a tree, averaged over half a second. "Warten (VSync/Render)" is the time between two frames, mostly waiting for the render thread, which waits for VSync; "Sonstiges" is the rest of the frame (Bevy itself, preparing the rendering).

`PhysicsDebugPlugin` draws collider outlines. They start hidden; the HUD checkbox "Kollider" shows them.
