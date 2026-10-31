//! Color grading by the time of day: `assets/light.ron` holds a [`Grading`] per phase
//! ([`Gradings`]), and [`DayNight::grading`] blends them by the clock. With the setting on, the
//! camera renders in HDR, so Bevy's tonemapping pass grades the whole image at once (the tilemap's
//! shader doesn't grade by itself); the HUD is drawn after it and keeps its colors.

use super::{DayNight, LightValues, Phase};
use bevy::{
    camera::Hdr,
    prelude::*,
    render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection},
};
use serde::Deserialize;

/// How the colors are graded at one time of day, see [`ColorGrading`]. A value left out changes
/// nothing.
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(default)]
pub struct Grading {
    /// Exposure offset in stops, negative darkens
    pub exposure: f32,
    /// Positive makes the colors warmer (redder), negative cooler (bluer); a few hundredths are
    /// clearly visible
    pub temperature: f32,
    /// Positive makes the colors more magenta, negative greener
    pub tint: f32,
    /// 1 unchanged, below desaturates down to gray at 0, above saturates
    pub saturation: f32,
    /// 1 unchanged, below moves the colors towards gray, above away from it. Bevy pivots it around
    /// 0.5 in linear colors, so below 1 black turns gray (0.95 already lifts it to 44/255) and above
    /// 1 dark colors turn black.
    pub contrast: f32,
}

impl Default for Grading {
    fn default() -> Self {
        Self {
            exposure: 0.,
            temperature: 0.,
            tint: 0.,
            saturation: 1.,
            contrast: 1.,
        }
    }
}

impl Grading {
    /// `self` at `t` = 0, `other` at `t` = 1.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            exposure: self.exposure.lerp(other.exposure, t),
            temperature: self.temperature.lerp(other.temperature, t),
            tint: self.tint.lerp(other.tint, t),
            saturation: self.saturation.lerp(other.saturation, t),
            contrast: self.contrast.lerp(other.contrast, t),
        }
    }

    /// The camera's grading, the same for shadows, midtones and highlights.
    pub fn color_grading(self) -> ColorGrading {
        ColorGrading::with_identical_sections(
            ColorGradingGlobal {
                exposure: self.exposure,
                temperature: self.temperature,
                tint: self.tint,
                ..default()
            },
            ColorGradingSection {
                saturation: self.saturation,
                contrast: self.contrast,
                ..default()
            },
        )
    }
}

/// The [`Grading`] of each phase, see [`DayNight::grading`].
#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct Gradings {
    pub day: Grading,
    /// In the middle of the dusk, when the night begins
    pub dusk: Grading,
    pub night: Grading,
    /// In the middle of the dawn, when the night ends
    pub dawn: Grading,
}

impl DayNight {
    /// The grading of the colors: the day's and the night's blended by [`Self::night`], turning
    /// into the dusk's or the dawn's towards the middle of the twilight.
    pub fn grading(&self, values: &LightValues) -> Grading {
        let gradings = &values.grading;
        let twilight = match self.phase {
            Phase::Dawn => gradings.dawn,
            _ => gradings.dusk,
        };
        // 1 in the middle of the twilight, 0 by day and at night
        let in_twilight = 4. * self.night * (1. - self.night);
        gradings
            .day
            .lerp(gradings.night, self.night)
            .lerp(twilight, in_twilight)
    }
}

/// Grades the camera's colors by the time of day, while it renders in HDR (see
/// `Settings::color_grading`). Without HDR, only some shaders would grade their colors, the
/// tilemap's not.
pub(super) fn grade_colors(
    day_night: Res<DayNight>,
    values: Res<LightValues>,
    camera: Single<(&mut ColorGrading, Has<Hdr>), With<Camera2d>>,
) {
    let (mut grading, hdr) = camera.into_inner();
    grading.set_if_neq(if hdr {
        day_night.grading(&values).color_grading()
    } else {
        ColorGrading::default()
    });
}
