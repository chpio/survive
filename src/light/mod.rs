//! Day and night (see `docs/licht.md`). [`DayNight`] keeps the time of day and derives from it how
//! dark the night is, which shrinks the player's [`SightRadius`], and where the sun is. [`render`]
//! draws both in a layer above everything, with the sun's shadows of what [`shadow`] rasterizes,
//! and grades the colors by the time of day ([`grading`]). The values come from `assets/light.ron`
//! ([`values`]).

pub mod grading;
pub mod render;
pub mod shadow;
pub mod values;

use crate::{objects::player::Player, settings::Settings, sight::SightRadius};
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
    /// Angle of the sun above the horizon at noon in degrees
    pub sun_elevation: f32,
    /// How much the sun's shadows darken by day, from 0 (not at all) to 1 (black)
    pub shadow_strength: f32,
    /// How the colors are graded in each phase
    pub grading: grading::Gradings,
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
    /// Real seconds per day: the values' `day_length`, unless the time-lapse (a dev tool in the
    /// HUD, see [`Self::cycle_time_lapse`]) shortened it
    pub day_length: f32,
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
            day_length: values.day_length,
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

    /// How many times faster than the values' `day_length` the clock runs.
    pub fn time_lapse(&self, values: &LightValues) -> f32 {
        values.day_length / self.day_length
    }

    /// Lets the clock run at the next speed in [`TIME_LAPSES`], or at the normal speed again after
    /// the last one.
    pub fn cycle_time_lapse(&mut self, values: &LightValues) {
        let current = self.time_lapse(values);
        let next = TIME_LAPSES
            .into_iter()
            .find(|&time_lapse| time_lapse > current + 0.01)
            .unwrap_or(1.);
        self.day_length = values.day_length / next;
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

    /// The sun while it is up, from the start of the dawn to the end of the dusk, so its shadows
    /// fade in and out with the twilight: in the east in the morning, in the south at noon, in the
    /// west in the evening.
    pub fn sun(&self, values: &LightValues) -> Option<Sun> {
        let twilight = values.twilight as f64;
        let sunrise = values.night_start as f64 + values.night_length() - twilight / 2.;
        let since_sunrise = (self.hour() as f64 - sunrise).rem_euclid(24.);
        let progress = (since_sunrise / (24. - values.night_length() + twilight)) as f32;
        if progress >= 1. {
            return None;
        }
        let angle = std::f32::consts::PI * progress;
        let elevation = values.sun_elevation.to_radians() * angle.sin();
        Some(Sun {
            direction: Vec2::new(angle.cos(), -angle.sin()),
            slope: elevation.tan(),
        })
    }

    /// How much the sun's shadows darken, from 0 (not at all) to 1 (black), fading out with the
    /// dusk and in with the dawn.
    pub fn shadow(&self, values: &LightValues) -> f32 {
        if self.sun(values).is_none() {
            return 0.;
        }
        values.shadow_strength * (1. - self.night)
    }

    /// The base of the player's [`SightRadius`].
    pub fn sight_radius(&self, values: &LightValues) -> f32 {
        values
            .day_sight_radius
            .lerp(values.night_sight_radius, self.night)
    }
}

/// Where the sun is, see [`DayNight::sun`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sun {
    /// Direction towards the sun in world space, of unit length
    pub direction: Vec2,
    /// How far the sun's rays rise per px towards it
    pub slope: f32,
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

/// How many times faster the time-lapse lets the clock run, see [`DayNight::cycle_time_lapse`].
pub const TIME_LAPSES: [f32; 2] = [10., 60.];

/// Run condition of the systems of the day-night cycle: whether [`values::Plugin`] inserted the
/// values and the clock (or a test did).
pub fn running(values: Option<Res<LightValues>>, day_night: Option<Res<DayNight>>) -> bool {
    values.is_some() && day_night.is_some()
}

/// Run condition of the sun's shadows: whether [`Settings::shadows`] are on (or there are no
/// settings, as in tests).
pub fn shadows_on(settings: Option<Res<Settings>>) -> bool {
    settings.is_none_or(|settings| settings.shadows)
}

/// Runs the clock with the game time, so it stands still while the game is paused.
fn run_clock(
    time: Res<Time>,
    keyboard_input: Option<Res<ButtonInput<KeyCode>>>,
    values: Res<LightValues>,
    mut day_night: ResMut<DayNight>,
) {
    let mut hours = 0.;
    if !day_night.paused {
        hours += time.delta_secs_f64() * 24. / day_night.day_length as f64;
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
/// inserts them), and rasterizes the shadow casters. Drawing needs [`render::Plugin`].
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(shadow::Plugin).add_systems(
            Update,
            (run_clock, set_sight_radius).chain().run_if(running),
        );
    }
}
