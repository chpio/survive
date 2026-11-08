use bevy::prelude::*;
use survival::figure::{self, FigureData, Keyframes, anchor_tag, direction_for, sorted_z};
use survival::objects::chest;

fn load(name: &str) -> FigureData {
    let path = format!(
        "{}/assets/characters/{name}.glb",
        env!("CARGO_MANIFEST_DIR")
    );
    figure::parse(&std::fs::read(path).unwrap()).unwrap()
}

/// Rest transform of each node in the figure's space.
fn world_transforms(figure: &FigureData) -> Vec<GlobalTransform> {
    let mut transforms: Vec<GlobalTransform> = Vec::new();
    for node in &figure.nodes {
        let transform = match node.parent {
            Some(parent) => transforms[parent].mul_transform(node.transform),
            None => GlobalTransform::from(node.transform),
        };
        transforms.push(transform);
    }
    transforms
}

/// Bounds on screen of the parts of direction `d` at rest.
fn bounds(figure: &FigureData, d: usize) -> Rect {
    let transforms = world_transforms(figure);
    let direction = figure
        .nodes
        .iter()
        .position(|n| n.name == format!("d{d}"))
        .unwrap();
    let in_direction = |mut node: usize| loop {
        match figure.nodes[node].parent {
            Some(parent) if parent == direction => return true,
            Some(parent) => node = parent,
            None => return false,
        }
    };
    let mut rect = Rect::EMPTY;
    for part in figure.parts.iter().filter(|p| in_direction(p.node)) {
        for &p in &part.positions {
            let p = transforms[part.node].transform_point(Vec3::from_array(p));
            rect = rect.union_point(p.xy());
        }
    }
    rect
}

fn names(figure: &FigureData) -> Vec<&str> {
    figure.animations.iter().map(|a| a.name.as_str()).collect()
}

#[test]
fn figures_have_their_animations_in_eight_directions() {
    let beetle = load("beetle");
    assert_eq!(
        names(&beetle),
        (0..8).map(|d| format!("walk/{d}")).collect::<Vec<_>>()
    );
    // 20 parts in each direction
    assert_eq!(beetle.parts.len(), 160);

    let player = load("player");
    let expected: Vec<_> = (0..8)
        .flat_map(|d| [format!("idle/{d}"), format!("walk/{d}")])
        .collect();
    let mut found: Vec<_> = names(&player).into_iter().map(str::to_owned).collect();
    found.sort_by_key(|n| (n[n.len() - 1..].to_owned(), n.clone()));
    assert_eq!(found, expected);
}

#[test]
fn walk_cycle_takes_two_thirds_of_a_second() {
    let walk = &load("beetle").animations[0];
    let end = walk
        .channels
        .iter()
        .flat_map(|c| c.times.iter().copied())
        .fold(0., f32::max);
    assert!((end - 16. / 24.).abs() < 1e-3, "walk cycle ends at {end} s");
}

#[test]
fn parents_come_before_their_children() {
    for figure in [load("beetle"), load("player"), load("chest")] {
        for (i, node) in figure.nodes.iter().enumerate() {
            if let Some(parent) = node.parent {
                assert!(parent < i, "{} comes before its parent", node.name);
            }
        }
    }
}

#[test]
fn parts_stay_within_a_thousandth_of_z() {
    for figure in [load("beetle"), load("player"), load("chest")] {
        for part in &figure.parts {
            assert!(part.positions.iter().all(|p| p[2] == 0.));
        }
        for channel in figure.animations.iter().flat_map(|a| &a.channels) {
            if let Keyframes::Translation(values) = &channel.keyframes {
                assert!(values.iter().all(|v| (0. ..=0.001).contains(&v.z)));
            }
            if let Keyframes::Rotation(rotations) = &channel.keyframes {
                assert!(
                    rotations
                        .iter()
                        .all(|r| r.x.abs() < 1e-4 && r.y.abs() < 1e-4)
                );
            }
        }
    }
}

#[test]
fn player_stands_on_its_feet() {
    let player = load("player");
    for d in 0..8 {
        let rect = bounds(&player, d);
        // feet around the origin, where the collider is, the head about a tile above
        assert!((-3. ..0.).contains(&rect.min.y), "direction {d}: {rect:?}");
        assert!((15. ..20.).contains(&rect.max.y), "direction {d}: {rect:?}");
        assert!(
            rect.min.x > -6. && rect.max.x < 6.,
            "direction {d}: {rect:?}"
        );
    }
}

#[test]
fn chest_facing_the_viewer_covers_its_collider() {
    let chest = load("chest");
    assert_eq!(
        names(&chest),
        (0..8).map(|d| format!("open/{d}")).collect::<Vec<_>>()
    );
    // direction 6 faces the viewer (-Y), as the game shows chests
    let rect = bounds(&chest, 6);
    let half = chest::SIZE / 2.;
    assert!(
        (rect.min.x + half.x).abs() < 1. && (rect.max.x - half.x).abs() < 1.,
        "{rect:?}"
    );
    // the front edge on the ground at the collider's lower edge, the lid above
    assert!((rect.min.y + half.y).abs() < 1., "{rect:?}");
    assert!(rect.max.y > 10., "{rect:?}");
}

#[test]
fn beetle_fits_around_a_collider_of_its_radius() {
    let beetle = load("beetle");
    for d in 0..8 {
        let rect = bounds(&beetle, d);
        // made for a radius of 5 px; legs may reach a bit beyond the collider, and the antennae
        // rise above it
        assert!(
            rect.min.cmpgt(Vec2::splat(-8.)).all() && rect.max.cmplt(Vec2::new(8., 10.)).all(),
            "direction {d}: {rect:?}"
        );
    }
}

#[test]
fn closest_direction_is_shown() {
    assert_eq!(direction_for(Vec2::X, None, 8), 0);
    assert_eq!(direction_for(Vec2::Y, None, 8), 2);
    assert_eq!(direction_for(Vec2::NEG_X, None, 8), 4);
    assert_eq!(direction_for(Vec2::NEG_Y, None, 8), 6);
    assert_eq!(direction_for(Vec2::new(1., -1.), None, 8), 7);
}

#[test]
fn direction_holds_near_the_border() {
    // just past the border between 0 and 1 (22.5°)
    let facing = Vec2::from_angle(24f32.to_radians());
    assert_eq!(direction_for(facing, Some(0), 8), 0);
    assert_eq!(direction_for(facing, None, 8), 1);
    let further = Vec2::from_angle(30f32.to_radians());
    assert_eq!(direction_for(further, Some(0), 8), 1);
}

#[test]
fn anchor_tag_holds_rounded_position() {
    let tag = anchor_tag(Vec2::new(218.4, -386.6));
    let x = (tag & 0xffff) as f32 - 32768.;
    let y = (tag >> 16) as f32 - 32768.;
    assert_eq!((x, y), (218., -387.));
    assert_ne!(anchor_tag(Vec2::splat(-40000.)), 0);
}

#[test]
fn lower_figures_are_in_front() {
    assert!(sorted_z(-10., 0.) > sorted_z(10., 0.));
    assert!(sorted_z(-1., 0.) - sorted_z(0., 0.) >= 0.001 - 1e-6);
    assert!((0. ..=0.9).contains(&sorted_z(5000., 0.)));
}
