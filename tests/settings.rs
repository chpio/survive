use bevy::{
    prelude::*,
    state::app::StatesPlugin,
    window::{PresentMode, PrimaryWindow, WindowMode},
};
use std::path::PathBuf;
use survival::settings::{self, Menu, Settings, SettingsFile, UI_SCALES, ZOOM_LEVELS};

/// A path in the temp directory that no file exists at yet.
fn temp_file(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "survive-settings-{name}-{}.ron",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    path
}

#[test]
fn missing_or_broken_file_gives_defaults() {
    let path = temp_file("missing");
    assert_eq!(Settings::load(&path), Settings::default());

    std::fs::write(&path, "not ron").unwrap();
    assert_eq!(Settings::load(&path), Settings::default());
}

#[test]
fn saved_settings_load_again() {
    let path = temp_file("roundtrip");
    let settings = Settings {
        vsync: false,
        fullscreen: true,
        zoom: 1.5,
        ui_scale: 1.25,
    };
    settings.save(&path).unwrap();
    assert_eq!(Settings::load(&path), settings);
}

#[test]
fn missing_fields_keep_their_defaults() {
    let path = temp_file("partial");
    std::fs::write(&path, "(zoom: 2.0)").unwrap();
    assert_eq!(
        Settings::load(&path),
        Settings {
            zoom: 2.,
            ..default()
        }
    );
}

#[test]
fn zoom_steps_through_the_levels_and_stops_at_the_ends() {
    let zoom = |zoom| Settings { zoom, ..default() };
    assert_eq!(zoom(1.).next_zoom(true), 1.25);
    assert_eq!(zoom(1.).next_zoom(false), 0.75);
    // a value between the levels (e.g. edited by hand) goes to the neighboring level
    assert_eq!(zoom(1.1).next_zoom(true), 1.25);
    assert_eq!(zoom(1.1).next_zoom(false), 1.);

    let (min, max) = (ZOOM_LEVELS[0], ZOOM_LEVELS[ZOOM_LEVELS.len() - 1]);
    assert_eq!(zoom(max).next_zoom(true), max);
    assert_eq!(zoom(min).next_zoom(false), min);
}

#[test]
fn ui_scale_steps_through_its_levels() {
    let ui_scale = |ui_scale| Settings {
        ui_scale,
        ..default()
    };
    assert_eq!(ui_scale(1.).next_ui_scale(true), 1.25);
    assert_eq!(ui_scale(1.).next_ui_scale(false), 0.75);

    let (min, max) = (UI_SCALES[0], UI_SCALES[UI_SCALES.len() - 1]);
    assert_eq!(ui_scale(max).next_ui_scale(true), max);
    assert_eq!(ui_scale(min).next_ui_scale(false), min);
}

/// App with a primary window and a 2D camera, whose settings are loaded from `path`.
fn app(path: PathBuf) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, StatesPlugin))
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(SettingsFile(path));
    // before the plugin, which applies the settings to the window right away
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.world_mut().spawn((
        Camera2d,
        Projection::Orthographic(OrthographicProjection::default_2d()),
    ));
    app.add_plugins(settings::Plugin);
    app
}

fn window(app: &mut App) -> Window {
    app.world_mut()
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(app.world())
        .unwrap()
        .clone()
}

fn projection_scale(app: &mut App) -> f32 {
    match app
        .world_mut()
        .query::<&Projection>()
        .single(app.world())
        .unwrap()
    {
        Projection::Orthographic(projection) => projection.scale,
        _ => panic!("not orthographic"),
    }
}

/// Presses `keys` together for one update.
fn press(app: &mut App, keys: &[KeyCode]) {
    let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    for &key in keys {
        input.press(key);
    }
    app.update();
    let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    input.release_all();
    input.clear();
}

#[test]
fn loaded_settings_are_applied_to_window_and_camera() {
    let path = temp_file("apply");
    Settings {
        vsync: false,
        fullscreen: true,
        zoom: 2.,
        ..default()
    }
    .save(&path)
    .unwrap();

    let mut app = app(path);
    // before the first update, so the window opens in the right mode
    let window = window(&mut app);
    assert_eq!(window.present_mode, PresentMode::AutoNoVsync);
    assert!(matches!(window.mode, WindowMode::BorderlessFullscreen(_)));

    app.update();
    assert_eq!(projection_scale(&mut app), 0.5);
}

#[test]
fn changed_settings_are_applied_and_saved() {
    let path = temp_file("save");
    let mut app = app(path.clone());
    app.update();
    // the loaded settings aren't written back
    assert!(!path.exists());

    // Enter alone does nothing
    press(&mut app, &[KeyCode::Enter]);
    assert!(!app.world().resource::<Settings>().fullscreen);

    press(&mut app, &[KeyCode::AltLeft, KeyCode::Enter]);
    assert!(app.world().resource::<Settings>().fullscreen);
    assert!(matches!(
        window(&mut app).mode,
        WindowMode::BorderlessFullscreen(_)
    ));
    assert!(Settings::load(&path).fullscreen);

    app.world_mut().resource_mut::<Settings>().zoom = 0.5;
    app.update();
    assert_eq!(projection_scale(&mut app), 2.);
    assert_eq!(Settings::load(&path).zoom, 0.5);
}

#[test]
fn escape_opens_the_menu_and_pauses_the_game() {
    let mut app = app(temp_file("menu"));
    app.update();

    press(&mut app, &[KeyCode::Escape]);
    // the state changes in the next update
    app.update();
    assert_eq!(*app.world().resource::<State<Menu>>().get(), Menu::Open);
    assert!(app.world().resource::<Time<Virtual>>().is_paused());

    press(&mut app, &[KeyCode::Escape]);
    app.update();
    assert_eq!(*app.world().resource::<State<Menu>>().get(), Menu::Closed);
    assert!(!app.world().resource::<Time<Virtual>>().is_paused());
}
