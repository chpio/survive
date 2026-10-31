//! Player settings, edited in the settings menu (opened with Escape, pauses the game) and saved to
//! a RON file.

use bevy::{
    camera::Hdr,
    prelude::*,
    render::view::Msaa,
    window::{MonitorSelection, PresentMode, PrimaryWindow, WindowMode},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Zoom levels the menu steps through, as a factor on the camera's default view.
pub const ZOOM_LEVELS: [f32; 7] = [0.5, 0.75, 1., 1.25, 1.5, 1.75, 2.];

/// Sizes of the HUD the menu steps through, as a factor on its size for the window (see
/// `ui::hud_scale`).
pub const UI_SCALES: [f32; 8] = [0.25, 0.5, 0.75, 1., 1.25, 1.5, 1.75, 2.];

/// Frame rate limits the menu steps through, `None` for no limit.
pub const FPS_LIMITS: [Option<u32>; 5] = [None, Some(30), Some(60), Some(120), Some(144)];

/// Samples per pixel the menu steps through for MSAA, 1 for none.
pub const MSAA_SAMPLES: [u32; 3] = [1, 2, 4];

#[derive(Resource, Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
// missing fields (e.g. settings added later) keep their defaults
#[serde(default)]
pub struct Settings {
    pub vsync: bool,
    /// Borderless fullscreen on the current monitor
    pub fullscreen: bool,
    /// Above 1 the camera shows less of the map, below 1 more
    pub zoom: f32,
    /// Size of the HUD, as a factor on its size for the window (see `ui::hud_scale`)
    pub ui_scale: f32,
    /// Highest frame rate, `None` for no limit (but the display's with VSync), see
    /// [`limit_frame_rate`]
    pub fps_limit: Option<u32>,
    /// Samples per pixel of the camera's MSAA, 1 for none (see [`MSAA_SAMPLES`]). It only smooths
    /// the edges of meshes, and 4 samples doubled the frame time at 4K with color grading (HDR).
    pub msaa: u32,
    /// Colors graded by the time of day (`light::grading`). It needs an HDR camera, which costs
    /// about 1.3 ms per frame at 4K.
    pub color_grading: bool,
    /// The sun's shadows (`light::shadow`)
    pub shadows: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            vsync: true,
            fullscreen: false,
            zoom: 1.,
            ui_scale: 1.,
            fps_limit: None,
            msaa: 1,
            color_grading: true,
            shadows: true,
        }
    }
}

impl Settings {
    /// Reads the settings from `path`, or the defaults if there is no readable file.
    pub fn load(path: &Path) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound {
                    warn!("failed to read {}: {error}", path.display());
                }
                return default();
            }
        };
        ron::from_str(&text).unwrap_or_else(|error| {
            warn!("failed to parse {}: {error}", path.display());
            default()
        })
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let text = ron::ser::to_string_pretty(self, default()).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// The next zoom level in [`ZOOM_LEVELS`] in the direction of `zoom_in`, or the current zoom at
    /// the end.
    pub fn next_zoom(&self, zoom_in: bool) -> f32 {
        next_level(&ZOOM_LEVELS, self.zoom, zoom_in)
    }

    /// The next HUD size in [`UI_SCALES`] in the direction of `larger`, or the current size at the
    /// end.
    pub fn next_ui_scale(&self, larger: bool) -> f32 {
        next_level(&UI_SCALES, self.ui_scale, larger)
    }

    /// The next frame rate limit in [`FPS_LIMITS`] in the direction of `higher` (no limit is the
    /// lowest), or the current one at the end.
    pub fn next_fps_limit(&self, higher: bool) -> Option<u32> {
        next_level(&FPS_LIMITS, self.fps_limit, higher)
    }

    /// The next MSAA samples in [`MSAA_SAMPLES`] in the direction of `more`, or the current ones at
    /// the end.
    pub fn next_msaa(&self, more: bool) -> u32 {
        next_level(&MSAA_SAMPLES, self.msaa, more)
    }

    /// The camera's MSAA, none for unsupported samples.
    pub fn msaa(&self) -> Msaa {
        match self.msaa {
            2 => Msaa::Sample2,
            4 => Msaa::Sample4,
            _ => Msaa::Off,
        }
    }

    fn present_mode(&self) -> PresentMode {
        if self.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        }
    }

    fn window_mode(&self) -> WindowMode {
        if self.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        }
    }
}

/// The first of the sorted `levels` above `current` (`up`) or below it, or `current` at the end.
/// A value between the levels (e.g. edited by hand) goes to the neighboring level.
fn next_level<T: PartialOrd + Copy>(levels: &[T], current: T, up: bool) -> T {
    let next = if up {
        levels.iter().find(|&&level| level > current)
    } else {
        levels.iter().rfind(|&&level| level < current)
    };
    next.copied().unwrap_or(current)
}

/// `settings.ron` next to the executable.
pub fn default_path() -> PathBuf {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    dir.join("settings.ron")
}

/// The file the settings are saved to.
#[derive(Resource)]
pub struct SettingsFile(pub PathBuf);

/// Whether the settings menu is open. While it is, the game is paused.
#[derive(States, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Menu {
    #[default]
    Closed,
    Open,
}

/// Escape opens and closes the menu. Windows the HUD shows close first and take the key away
/// (see `ui::sync_state`), so the menu only opens once none is left.
pub fn toggle_menu(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    menu: Res<State<Menu>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    if keyboard_input.just_pressed(KeyCode::Escape) {
        next_menu.set(match menu.get() {
            Menu::Closed => Menu::Open,
            Menu::Open => Menu::Closed,
        });
    }
}

fn toggle_fullscreen(keyboard_input: Res<ButtonInput<KeyCode>>, mut settings: ResMut<Settings>) {
    if keyboard_input.any_pressed([KeyCode::AltLeft, KeyCode::AltRight])
        && keyboard_input.just_pressed(KeyCode::Enter)
    {
        settings.fullscreen = !settings.fullscreen;
    }
}

fn apply_to_window(settings: &Settings, window: &mut Window) {
    window.present_mode = settings.present_mode();
    window.mode = settings.window_mode();
}

fn apply(
    settings: Res<Settings>,
    mut commands: Commands,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
    camera: Single<(Entity, &mut Projection, Option<&Msaa>, Has<Hdr>), With<Camera2d>>,
) {
    apply_to_window(&settings, &mut window);
    let (camera, mut projection, msaa, hdr) = camera.into_inner();
    if let Projection::Orthographic(projection) = &mut *projection {
        projection.scale = 1. / settings.zoom;
    }
    // only changed when needed, since the camera's pipelines are specialized again then
    let mut camera = commands.entity(camera);
    if msaa != Some(&settings.msaa()) {
        camera.insert(settings.msaa());
    }
    if settings.color_grading != hdr {
        if settings.color_grading {
            camera.insert(Hdr);
        } else {
            camera.remove::<Hdr>();
        }
    }
}

/// Waits at the end of the frame until the next one is due by [`Settings::fps_limit`]. A frame
/// that took longer isn't made up for by shorter ones.
pub fn limit_frame_rate(settings: Res<Settings>, mut next_frame: Local<Option<Instant>>) {
    let Some(limit) = settings.fps_limit.filter(|&limit| limit > 0) else {
        *next_frame = None;
        return;
    };
    let now = Instant::now();
    let due = next_frame.unwrap_or(now).max(now);
    std::thread::sleep(due - now);
    *next_frame = Some(due + Duration::from_secs_f64(1. / limit as f64));
}

fn save(settings: Res<Settings>, file: Res<SettingsFile>) {
    // just loaded
    if settings.is_added() {
        return;
    }
    if let Err(error) = settings.save(&file.0) {
        warn!("failed to save {}: {error}", file.0.display());
    }
}

/// Loads the settings from [`SettingsFile`] (inserted before, or [`default_path`]) and applies
/// them to the primary window and the 2D camera. Needs Bevy's `StatesPlugin`.
pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        let path = match app.world().get_resource::<SettingsFile>() {
            Some(file) => file.0.clone(),
            None => default_path(),
        };
        let settings = Settings::load(&path);

        // the window is spawned by `WindowPlugin` and created by winit after startup, so it opens
        // in the right mode instead of switching on the first frame
        let mut windows = app
            .world_mut()
            .query_filtered::<&mut Window, With<PrimaryWindow>>();
        for mut window in windows.iter_mut(app.world_mut()) {
            apply_to_window(&settings, &mut window);
        }

        app.insert_resource(settings)
            .insert_resource(SettingsFile(path))
            .init_state::<Menu>()
            .add_systems(OnEnter(Menu::Open), |mut time: ResMut<Time<Virtual>>| {
                time.pause();
            })
            .add_systems(OnExit(Menu::Open), |mut time: ResMut<Time<Virtual>>| {
                time.unpause();
            })
            .add_systems(
                Update,
                (
                    (toggle_menu, toggle_fullscreen),
                    apply.run_if(resource_changed::<Settings>),
                    save.run_if(resource_changed::<Settings>),
                )
                    .chain(),
            )
            .add_systems(Last, limit_frame_rate);
    }
}
