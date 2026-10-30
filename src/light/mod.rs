//! Day and night (see `docs/licht.md`). [`DayNight`] keeps the time of day and derives from it how
//! dark the night is, which [`render`] draws as a layer above everything and which shrinks the
//! player's [`SightRadius`]. The values come from `assets/light.ron` ([`values`]).

pub mod render;
pub mod values;

use crate::{objects::player::Player, sight::SightRadius};
use bevy::prelude::*;
use serde::Deserialize;

/// The values of the day-night cycle, from `assets/light.ron`. All times of day are in game hours
/// (0–24).
#[derive(Asset, Resource, TypePath, Deserialize, Clone, PartialEq, Debug)]
pub struct LightValues {
    /// Real seconds per day
    pub day_length: f32,
    /// Time of day the game starts at
    pub start: f32,
    /// Time of day the night begins, in the middle of the dusk
    pub night_start: f32,
    /// Part of the day that is night, from the middle of the dusk to the middle of the dawn
    pub night_share: f32,
    /// Hours the dusk and the dawn each last
    pub twilight: f32,
    /// Brightness of the world at night, from 0 (black) to 1 (as by day)
    pub night_brightness: f32,
    /// Sight radius in px by day, see [`SightRadius`]
    pub day_sight_radius: f32,
    /// Sight radius in px at night
    pub night_sight_radius: f32,
}

impl LightValues {
    fn night_length(&self) -> f64 {
        24. * self.night_share as f64
    }

    /// Hours since the night began at the time of day `hour`, from half a day before the middle of
    /// the night to half a day after it.
    fn since_night_start(&self, hour: f64) -> f64 {
        let since = (hour - self.night_start as f64).rem_euclid(24.);
        if since > self.night_length() / 2. + 12. {
            since - 24.
        } else {
            since
        }
    }

    /// Hours since the night began at which the dusk, the night, the dawn and the day begin.
    fn phase_starts(&self) -> [(Phase, f64); 4] {
        let half = self.twilight as f64 / 2.;
        let night = self.night_length();
        [
            (Phase::Dusk, -half),
            (Phase::Night, half),
            (Phase::Dawn, night - half),
            (Phase::Day, night + half),
        ]
    }
}

/// Part of the day, see [`LightValues`].
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum Phase {
    #[default]
    Day,
    Dusk,
    Night,
    Dawn,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Day => "Tag",
            Phase::Dusk => "Abenddämmerung",
            Phase::Night => "Nacht",
            Phase::Dawn => "Morgendämmerung",
        }
    }
}

/// The clock and what follows from it. Every change to the time goes through a method that takes
/// the [`LightValues`], so the derived values always fit the time.
#[derive(Resource, Clone, PartialEq, Debug)]
pub struct DayNight {
    /// Game hours since midnight before the first day
    time: f64,
    /// Whether the clock stands still (a dev tool in the HUD)
    pub paused: bool,
    phase: Phase,
    /// How far it is night, from 0 (day) to 1 (night)
    night: f32,
}

impl DayNight {
    /// The clock at the time of day `hour` on the first day.
    pub fn new(hour: f32, values: &LightValues) -> Self {
        let mut day_night = Self {
            time: hour as f64,
            paused: false,
            phase: Phase::Day,
            night: 0.,
        };
        day_night.derive(values);
        day_night
    }

    /// Derives the phase and the night from the time, e.g. after the values changed.
    pub fn derive(&mut self, values: &LightValues) {
        let since = values.since_night_start(self.hour() as f64);
        let [(_, dusk), (_, night), (_, dawn), (_, day)] = values.phase_starts();
        self.phase = values
            .phase_starts()
            .into_iter()
            .rev()
            .find(|&(_, start)| since >= start)
            .map_or(Phase::Day, |(phase, _)| phase);
        self.night = (smoothstep(dusk, night, since) * (1. - smoothstep(dawn, day, since))) as f32;
    }

    /// Moves the clock on by `hours`.
    pub fn advance(&mut self, hours: f64, values: &LightValues) {
        self.time += hours;
        self.derive(values);
    }

    /// Moves the clock on to the start of the next phase.
    pub fn skip_to_next_phase(&mut self, values: &LightValues) {
        let since = values.since_night_start(self.hour() as f64);
        let next = values
            .phase_starts()
            .into_iter()
            .map(|(_, start)| start)
            .find(|&start| start > since)
            .unwrap_or(values.phase_starts()[0].1 + 24.);
        // a little past the start, so rounding can't leave it in the previous phase
        self.advance(next - since + 1e-6, values);
    }

    /// The day since the start, beginning with 1.
    pub fn day(&self) -> u32 {
        (self.time / 24.).floor() as u32 + 1
    }

    /// Time of day in hours (0–24).
    pub fn hour(&self) -> f32 {
        self.time.rem_euclid(24.) as f32
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// How far it is night, from 0 (day) to 1 (night), easing in and out over the twilight.
    pub fn night(&self) -> f32 {
        self.night
    }

    /// How much the world is darkened, from 0 (not at all) to 1 (black).
    pub fn darkness(&self, values: &LightValues) -> f32 {
        self.night * (1. - values.night_brightness)
    }

    /// The base of the player's [`SightRadius`].
    pub fn sight_radius(&self, values: &LightValues) -> f32 {
        values
            .day_sight_radius
            .lerp(values.night_sight_radius, self.night)
    }
}

/// 0 up to `from`, 1 from `to` on, easing in and out between them.
fn smoothstep(from: f64, to: f64, x: f64) -> f64 {
    if from >= to {
        return if x >= to { 1. } else { 0. };
    }
    let t = ((x - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Game hours the debug key moves the clock on.
pub const SKIP_HOURS: f64 = 1.;

/// Runs the clock with the game time, so it stands still while the game is paused.
fn run_clock(
    time: Res<Time>,
    keyboard_input: Option<Res<ButtonInput<KeyCode>>>,
    values: Res<LightValues>,
    mut day_night: ResMut<DayNight>,
) {
    let mut hours = 0.;
    if !day_night.paused {
        hours += time.delta_secs_f64() * 24. / values.day_length as f64;
    }
    if keyboard_input.is_some_and(|input| input.just_pressed(KeyCode::KeyN)) {
        hours += SKIP_HOURS;
    }
    if hours > 0. {
        day_night.advance(hours, &values);
    }
}

/// Sets the base of the player's sight radius by the time of day.
fn set_sight_radius(
    day_night: Res<DayNight>,
    values: Res<LightValues>,
    mut radius: Single<&mut SightRadius, With<Player>>,
) {
    let base = day_night.sight_radius(&values);
    if radius.base != base {
        radius.base = base;
    }
}

/// Runs the day-night cycle once [`LightValues`] and [`DayNight`] exist ([`values::Plugin`]
/// inserts them). Drawing the night needs [`render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (run_clock, set_sight_radius)
                .chain()
                .run_if(resource_exists::<LightValues>)
                .run_if(resource_exists::<DayNight>),
        );
    }
}
