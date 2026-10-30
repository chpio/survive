use crate::common;
use bevy::prelude::*;
use survival::{
    light::{self, DayNight, LightValues, Phase},
    objects::player,
    sight::SightRadius,
};

/// The values of `assets/light.ron`: a day of 600 s, the night from 21:00 to 05:00, dusk and dawn
/// of 1.5 h each.
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
