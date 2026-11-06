# Benchmarks

Benchmarks live in `benches/`, one file per area, and use Criterion (`harness = false`, a `[[bench]]` entry each in `Cargo.toml`). They measure single pieces of game code without an `App`, for what `cargo tracy` and the HUD's "Performance" view can't isolate. The lib and binary have `bench = false`, so `cargo bench` only runs Criterion, which accepts the arguments after `--`.

```sh
cargo bench --no-default-features                       # all benchmarks, like a shipped build
cargo bench --no-default-features --bench flowfield     # one file
cargo bench --no-default-features -- open               # filter by name
```

`--no-default-features` turns off the `dev` feature, so Bevy is linked statically as in a shipped build; it builds into the same `target/release/` as `cargo build --release --no-default-features`.

Criterion keeps the last run in `target/criterion/` and reports the change against it, so running a benchmark before and after a change shows its effect. For trends over a longer time, save a named baseline and compare against it later:

```sh
cargo bench --no-default-features -- --save-baseline before-x
cargo bench --no-default-features -- --baseline before-x
```

Numbers are only comparable on the same machine with the same build settings, and a busy machine makes them noisy (the report says when a change is within the noise). When writing numbers down, e.g. in a commit message, say what was measured on.

## What is measured

- `flowfield`: one flow field search (`FlowField::set_target`) for the grid with the smallest clearance, over the whole window (160 × 160 cells), on an open map and on a map with 8 × 8 px blocks every 48 px. Each iteration moves the target by a cell, so the grid searches again; the time includes putting the window's blocked cells together. Each grid used by awake enemies searches once whenever the player enters another cell.
