use bevy::prelude::*;
use survival::interact::{Interactable, grown_outline};

#[test]
fn outline_grows_a_square_on_all_sides() {
    let square = vec![
        Vec2::new(-4., -4.),
        Vec2::new(4., -4.),
        Vec2::new(4., 4.),
        Vec2::new(-4., 4.),
        // inner points don't matter
        Vec2::ZERO,
    ];
    let mut grown = grown_outline(square, 1.);
    grown.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let expected = [
        Vec2::new(-5., -5.),
        Vec2::new(-5., 5.),
        Vec2::new(5., -5.),
        Vec2::new(5., 5.),
    ];
    assert_eq!(grown.len(), 4);
    for (g, e) in grown.iter().zip(expected) {
        assert!(g.distance(e) < 1e-4, "{grown:?}");
    }
}

#[test]
fn sharp_corners_are_cut_short() {
    let spike = vec![Vec2::new(0., 0.), Vec2::new(20., 0.5), Vec2::new(0., 1.)];
    let grown = grown_outline(spike.clone(), 1.);
    assert_eq!(grown.len(), 3);
    for g in &grown {
        let closest = spike
            .iter()
            .map(|p| g.distance(*p))
            .fold(f32::MAX, f32::min);
        assert!(closest <= 2. + 1e-4, "{grown:?}");
    }
}

#[test]
fn interaction_needs_reach() {
    let chest = Interactable {
        reach: 24.,
        radius: 4.,
    };
    let at = Vec2::new(100., 50.);
    // without a sight everything is seen
    assert!(chest.can_interact(at, at + Vec2::new(0., 24.), None));
    assert!(!chest.can_interact(at, at + Vec2::new(0., 24.5), None));
}
