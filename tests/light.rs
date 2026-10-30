use crate::common;
use avian2d::prelude::*;
use bevy::prelude::*;
use survival::{
    light::{
        self, DayNight, LightValues, Phase, Sun,
        shadow::{Heights, ShadowHeights, SunShadow},
    },
    objects::player,
    sight::SightRadius,
};

/// The values of `assets/light.ron`: a day of 600 s, the night from 21:00 to 05:00, dusk and dawn
/// of 1.5 h each, the sun 50° high at noon.
fn values() -> LightValues {
    LightValues {
        day_length: 600.,
        start: 8.,
        night_start: 21.,
        night_share: 1. / 3.,
        twilight: 1.5,
        night_brightness: 0.5,
        day_sight_radius: 200.,
        night_sight_radius: 120.,
        sun_elevation: 50.,
        shadow_strength: 0.3,
    }
}

#[test]
fn night_eases_in_over_the_dusk() {
    let values = values();
    let at = |hour| DayNight::new(hour, &values);

    assert_eq!(at(8.).phase(), Phase::Day);
    assert_eq!(at(8.).night(), 0.);
    assert_eq!(at(20.).phase(), Phase::Day);
    assert_eq!(at(20.5).phase(), Phase::Dusk);
    assert!((at(21.).night() - 0.5).abs() < 1e-4);
    assert_eq!(at(22.).phase(), Phase::Night);
    assert_eq!(at(1.).night(), 1.);
    assert_eq!(at(4.5).phase(), Phase::Dawn);
    assert_eq!(at(6.).phase(), Phase::Day);
    assert_eq!(at(6.).night(), 0.);
}

#[test]
fn night_darkens_and_shrinks_the_sight() {
    let values = values();
    let day = DayNight::new(12., &values);
    let night = DayNight::new(0., &values);

    assert_eq!(day.darkness(&values), 0.);
    assert_eq!(day.sight_radius(&values), 200.);
    assert_eq!(night.darkness(&values), 0.5);
    assert_eq!(night.sight_radius(&values), 120.);
}

#[test]
fn skipping_goes_through_the_phases_into_the_next_day() {
    let values = values();
    let mut day_night = DayNight::new(8., &values);

    let mut phases = Vec::new();
    for _ in 0..4 {
        day_night.skip_to_next_phase(&values);
        phases.push((day_night.phase(), (day_night.hour() * 60.).round() / 60.));
    }

    assert_eq!(
        phases,
        [
            (Phase::Dusk, 20.25),
            (Phase::Night, 21.75),
            (Phase::Dawn, 4.25),
            (Phase::Day, 5.75),
        ]
    );
    assert_eq!(day_night.day(), 2);
}

#[test]
fn sun_goes_from_east_to_west_over_the_south() {
    let values = values();
    let sun = |hour| DayNight::new(hour, &values).sun(&values);

    let morning = sun(8.).unwrap();
    assert!(morning.direction.x > 0.5 && morning.direction.y < 0.);
    // halfway between 05:00 and 21:00
    let noon = sun(13.).unwrap();
    assert!(noon.direction.distance(Vec2::NEG_Y) < 1e-4);
    assert!((noon.slope - 50f32.to_radians().tan()).abs() < 1e-4);
    assert!(morning.slope < noon.slope);
    let evening = sun(18.).unwrap();
    assert!(evening.direction.x < -0.5 && evening.direction.y < 0.);
    // set at the end of the dusk
    assert!(sun(21.7).is_some());
    assert_eq!(sun(21.8), None);
    assert_eq!(sun(23.), None);
}

#[test]
fn shadows_fade_out_with_the_dusk() {
    let values = values();
    let shadow = |hour| DayNight::new(hour, &values).shadow(&values);

    assert_eq!(shadow(13.), 0.3);
    assert!((shadow(21.) - 0.15).abs() < 1e-3, "{}", shadow(21.));
    assert!(shadow(21.7) < 0.01);
    assert_eq!(shadow(23.), 0.);
}

#[test]
fn heights_of_the_loaded_maps_are_put_together() {
    let mut west = Heights::new(Rect::new(-100., 0., 0., 100.));
    let east = Heights::new(Rect::new(0., 0., 100., 100.));
    let tree = Collider::rectangle(16., 16.);
    west.add(&tree, Vec2::new(-50., 50.), Rotation::IDENTITY, 24.);
    // a lower one in the same place doesn't lower it
    west.add(&tree, Vec2::new(-50., 50.), Rotation::IDENTITY, 8.);

    let heights = ShadowHeights::new(&[&west, &east]);

    assert_eq!(heights.height(Vec2::new(-50., 50.)), 24.);
    assert_eq!(heights.height(Vec2::new(-30., 50.)), 0.);
    assert_eq!(heights.height(Vec2::new(150., 50.)), 0.);
    assert_eq!(heights.max(), 24.);
    assert_eq!(heights.rect(), Rect::new(-100., 0., 100., 100.));
}

/// A tree 24 px high on a map 200×200 px around the origin, filling the cell at the origin.
fn tree() -> ShadowHeights {
    let mut map = Heights::new(Rect::new(-100., -100., 100., 100.));
    map.add(
        &Collider::rectangle(4., 4.),
        Vec2::splat(2.),
        Rotation::IDENTITY,
        24.,
    );
    ShadowHeights::new(&[&map])
}

#[test]
fn shadow_falls_away_from_the_sun() {
    // in the south, 45° high: the shadow is as long as the tree is high
    let sun = Sun {
        direction: Vec2::NEG_Y,
        slope: 1.,
    };
    let shadow = SunShadow::compute(&tree(), sun);

    assert!(!shadow.shades(Vec2::splat(2.)), "the tree's top is lit");
    assert!(shadow.shades(Vec2::new(2., 10.)));
    assert!(shadow.shades(Vec2::new(2., 24.)));
    assert!(!shadow.shades(Vec2::new(2., 34.)));
    assert!(!shadow.shades(Vec2::new(2., -10.)));
    assert!(!shadow.shades(Vec2::new(14., 10.)));
}

#[test]
fn shadow_is_computed_again_once_its_tip_moved() {
    let heights = tree();
    let sun = |angle: f32| Sun {
        direction: Vec2::from_angle(angle),
        slope: 1.,
    };
    let shadow = SunShadow::compute(&heights, sun(0.));

    // the tip 24 px away moves by about 0.5 px
    assert!(!shadow.is_outdated(&heights, sun(0.02)));
    assert!(shadow.is_outdated(&heights, sun(0.1)));
}

fn app() -> (App, Entity) {
    let mut app = common::app_with(light::Plugin);
    let values = values();
    app.insert_resource(DayNight::new(values.start, &values))
        .insert_resource(values);
    let player = common::spawn(&mut app, player::Player, Vec2::ZERO);
    (app, player)
}

#[test]
fn clock_runs_with_the_game_time() {
    let (mut app, _) = app();

    // 25 s are an hour
    common::run(&mut app, 25.);

    let hour = app.world().resource::<DayNight>().hour();
    assert!((hour - 9.).abs() < 0.01, "{hour}");
}

#[test]
fn time_lapse_cycles_through_its_speeds() {
    let values = values();
    let mut day_night = DayNight::new(8., &values);
    assert_eq!(day_night.time_lapse(&values), 1.);

    let mut speeds = Vec::new();
    for _ in 0..3 {
        day_night.cycle_time_lapse(&values);
        speeds.push(day_night.time_lapse(&values));
    }

    assert_eq!(speeds, [10., 60., 1.]);
    assert_eq!(day_night.day_length, 600.);
}

#[test]
fn time_lapse_runs_the_clock_faster() {
    let (mut app, _) = app();
    let values = values();
    app.world_mut()
        .resource_mut::<DayNight>()
        .cycle_time_lapse(&values);

    // an hour at ten times the speed
    common::run(&mut app, 2.5);

    let hour = app.world().resource::<DayNight>().hour();
    assert!((hour - 9.).abs() < 0.01, "{hour}");
}

#[test]
fn paused_clock_stands_still() {
    let (mut app, _) = app();
    app.world_mut().resource_mut::<DayNight>().paused = true;

    common::run(&mut app, 1.);

    assert_eq!(app.world().resource::<DayNight>().hour(), 8.);
}

#[test]
fn night_sets_the_players_sight_radius() {
    let (mut app, player) = app();
    let values = values();
    app.world_mut()
        .resource_mut::<DayNight>()
        .advance(16., &values);

    app.update();

    let radius = app.world().get::<SightRadius>(player).unwrap();
    assert_eq!(radius.base, 120.);
}
