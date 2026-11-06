use bevy::prelude::*;
use survival::figure::{self, FigureData, Keyframes};

fn beetle() -> FigureData {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/characters/beetle.glb"
    ))
    .unwrap();
    figure::parse(&bytes).unwrap()
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

#[test]
fn beetle_has_its_parts_and_walk_cycle() {
    let beetle = beetle();
    // body, head, seam, 6 two-part legs, 2 antennae and 2 mandibles
    assert_eq!(beetle.parts.len(), 20);
    let names: Vec<_> = beetle.animations.iter().map(|a| &a.name).collect();
    assert_eq!(names, ["walk"]);
    let end = beetle.animations[0]
        .channels
        .iter()
        .flat_map(|c| c.times.iter().copied())
        .fold(0., f32::max);
    assert!((end - 16. / 24.).abs() < 1e-3, "walk cycle ends at {end} s");
}

#[test]
fn parents_come_before_their_children() {
    for (i, node) in beetle().nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            assert!(parent < i, "{} comes before its parent", node.name);
        }
    }
}

#[test]
fn part_depths_are_taken_out_of_the_vertices() {
    for part in beetle().parts {
        assert!((0. ..1.).contains(&part.z), "depth {}", part.z);
        assert!(part.positions.iter().all(|p| p[2] == 0.));
    }
}

#[test]
fn animations_only_turn_around_z() {
    for channel in &beetle().animations[0].channels {
        if let Keyframes::Rotation(rotations) = &channel.keyframes {
            assert!(
                rotations
                    .iter()
                    .all(|r| r.x.abs() < 1e-4 && r.y.abs() < 1e-4)
            );
        }
    }
}

#[test]
fn beetle_fits_around_a_collider_of_its_radius() {
    let beetle = beetle();
    let transforms = world_transforms(&beetle);
    let reach = beetle
        .parts
        .iter()
        .flat_map(|part| {
            let transform = transforms[part.node];
            part.positions
                .iter()
                .map(move |&p| transform.transform_point(Vec3::from_array(p)).xy())
        })
        .map(Vec2::length)
        .fold(0., f32::max);
    // made for a radius of 5 px; legs and antennae may reach a bit beyond the collider
    assert!(
        (4.5..7.).contains(&reach),
        "reaches {reach} px from the center"
    );
}
