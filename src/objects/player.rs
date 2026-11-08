use crate::{
    combat::{Attack, Died, Health, health_bar::UnitHeight},
    figure::{FigureControl, FigureRoot},
    movement::{self, DesiredVelocity},
    objects::enemy::Enemy,
    sight::SightRadius,
};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::time::Duration;

#[derive(Default, Clone, Component)]
#[require(SightRadius)]
pub struct Player;

/// Where the player starts, on the first map.
pub const START: Vec2 = Vec2::new(218., -386.);

/// Shape of the collider, at the figure's feet.
pub const SHAPE: Circle = Circle::new(5.);

pub const MASS: f32 = 3.;
/// Reaches full speed in 0.1 s and holds its ground against six enemies pushing together.
pub const STRENGTH: f32 = 6000.;

/// Physics components of the player.
pub fn physics() -> impl Bundle {
    movement::body(SHAPE.collider(), MASS, STRENGTH)
}

pub const MAX_HEALTH: f32 = 100.;

/// Health and attack of the player, which hits all enemies around it twice a second.
pub fn combat() -> impl Bundle {
    (
        Health::new(MAX_HEALTH),
        Attack::new(10., SHAPE.radius + 6., 300., Duration::from_millis(500)),
        // the figure's head
        UnitHeight(17.),
    )
}

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut desired: Single<&mut DesiredVelocity, With<Player>>,
) {
    const MOVE_SPEED: f32 = 100.;

    let mut direction = Vec2::ZERO;

    if keyboard_input.pressed(KeyCode::KeyD) {
        direction.x += 1.;
    }

    if keyboard_input.pressed(KeyCode::KeyA) {
        direction.x += -1.;
    }

    if keyboard_input.pressed(KeyCode::KeyW) {
        direction.y += 1.;
    }

    if keyboard_input.pressed(KeyCode::KeyS) {
        direction.y += -1.;
    }

    desired.set_if_neq(DesiredVelocity(direction.normalize_or_zero() * MOVE_SPEED));
}

/// The player's death starts the round over: all enemies are removed and the player is healed.
pub fn start_over_on_death(
    died: On<Died>,
    mut players: Query<&mut Health, With<Player>>,
    enemies: Query<Entity, With<Enemy>>,
    mut commands: Commands,
) {
    let Ok(mut health) = players.get_mut(died.entity) else {
        return;
    };
    info!("player died, starting over");
    health.current = health.max;
    for enemy in &enemies {
        // may already be despawned for dying in the same frame
        commands.entity(enemy).try_despawn();
    }
}

/// Visible body of the player, a child showing its figure.
#[derive(Component)]
pub struct Body;

/// The player's figure as a child, for its bundle.
pub fn body(asset_server: &AssetServer) -> impl Bundle {
    children![(
        Body,
        FigureRoot(asset_server.load("characters/player.glb")),
        Transform::default(),
        Visibility::default(),
    )]
}

/// Speed in px/s at which the walk animation plays at its own pace.
const WALK_ANIMATION_SPEED: f32 = 80.;

/// Shows the player walking where it wants to go, or standing.
fn animate(
    player: Single<(&DesiredVelocity, &LinearVelocity), With<Player>>,
    mut body: Single<&mut FigureControl, With<Body>>,
) {
    let (desired, velocity) = *player;
    let control = &mut **body;
    let (animation, speed) = if desired.0 == Vec2::ZERO {
        ("idle", 1.)
    } else {
        control.facing = desired.0.normalize();
        (
            "walk",
            (velocity.length() / WALK_ANIMATION_SPEED).clamp(0.5, 4.),
        )
    };
    control.animation = animation;
    if (control.speed - speed).abs() > 0.01 {
        control.speed = speed;
    }
}

pub struct Plugin;

impl bevy::app::Plugin for Plugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (move_player, animate))
            .add_observer(start_over_on_death);
    }
}
