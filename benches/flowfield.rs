//! Time of one flow field search (`FlowField::set_target`) over the whole window, on an open map
//! and on one with scattered obstacles. Each iteration moves the target by a cell, so the grid
//! searches again; the time includes putting the window's blocked cells together.

use avian2d::prelude::*;
use bevy::prelude::*;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use survival::flowfield::{CELL_SIZE, FLOW_FIELD_RADIUS, FlowField, Obstacles};

/// Radius of a small enemy, which uses the grid with the smallest clearance
const RADIUS: f32 = 5.;

/// Map covering the whole window around the origin, so no cell is blocked for lying outside.
fn open_map() -> Obstacles {
    let half = FLOW_FIELD_RADIUS + 64.;
    Obstacles::new(Rect::new(-half, -half, half, half))
}

/// [`open_map`] with 8×8 px blocks every 48 px, staggered row by row, like trees in a forest.
fn forest_map() -> Obstacles {
    let mut map = open_map();
    let block = Collider::rectangle(8., 8.);
    let half = FLOW_FIELD_RADIUS as i32 + 64;
    for (row, y) in (-half..half).step_by(48).enumerate() {
        let offset = if row % 2 == 0 { 0 } else { 24 };
        for x in (-half + offset..half).step_by(48) {
            // keep the targets free
            if x.abs() < 16 && y.abs() < 16 {
                continue;
            }
            map.block_collider(&block, Vec2::new(x as f32, y as f32), Rotation::IDENTITY);
        }
    }
    map
}

fn search(c: &mut Criterion) {
    let mut group = c.benchmark_group("flowfield_search");
    for (name, map) in [("open", open_map()), ("forest", forest_map())] {
        let mut flow_field = FlowField::new([RADIUS]);
        let targets = [Vec2::new(2., 2.), Vec2::new(2. + CELL_SIZE, 2.)];
        let mut i = 0;
        group.bench_function(name, |b| {
            b.iter(|| {
                i += 1;
                flow_field.set_target(black_box(targets[i % 2]), [RADIUS], &[&map]);
            })
        });
        let grid = flow_field.grid(RADIUS).unwrap();
        assert!(grid.is_computed() && grid.reachable().count() > 10_000);
    }
    group.finish();
}

criterion_group!(benches, search);
criterion_main!(benches);
