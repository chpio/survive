mod common;

use bevy::prelude::*;
use std::time::Duration;
use survival::{
    combat::{self, Attack, Health},
    objects::{enemy::Enemy, player},
};

const SMALL: Enemy = Enemy { radius: 5. };
const LARGE: Enemy = Enemy { radius: 12. };

fn app() -> App {
    common::app_with(combat::Plugin)
}

fn spawn_player(app: &mut App, position: Vec2) -> Entity {
    common::spawn(
        app,
        (player::Player, player::physics(), player::combat()),
        position,
    )
}

fn spawn_enemy(app: &mut App, enemy: Enemy, position: Vec2) -> Entity {
    common::spawn(app, (enemy, enemy.physics(), enemy.combat()), position)
}

fn health(app: &App, entity: Entity) -> Health {
    *app.world().get::<Health>(entity).unwrap()
}

#[test]
fn player_and_enemy_in_reach_hurt_each_other() {
    let mut app = app();
    let player = spawn_player(&mut app, Vec2::ZERO);
    // 1 px between the colliders
    let enemy = spawn_enemy(&mut app, SMALL, Vec2::new(11., 0.));

    common::run(&mut app, 0.1);

    let player_health = health(&app, player);
    let enemy_health = health(&app, enemy);
    assert!(player_health.current < player_health.max);
    assert!(enemy_health.current < enemy_health.max);
}

#[test]
fn units_out_of_reach_are_not_hurt() {
    let mut app = app();
    let player = spawn_player(&mut app, Vec2::ZERO);
    let enemy = spawn_enemy(&mut app, SMALL, Vec2::new(50., 0.));

    common::run(&mut app, 1.);

    assert_eq!(health(&app, player), Health::new(player::MAX_HEALTH));
    let enemy_health = health(&app, enemy);
    assert_eq!(enemy_health.current, enemy_health.max);
}

#[test]
fn attacks_wait_for_their_cooldown() {
    let mut app = app();
    // without knockback, so the target stays in reach
    common::spawn(
        &mut app,
        (
            player::Player,
            player::physics(),
            Attack::new(10., 20., 0., Duration::from_millis(500)),
        ),
        Vec2::ZERO,
    );
    let enemy = common::spawn(
        &mut app,
        (SMALL, SMALL.physics(), Health::new(1000.)),
        Vec2::new(11., 0.),
    );

    common::run(&mut app, 1.2);

    // right away, after 0.5 s and after 1 s
    assert_eq!(health(&app, enemy).current, 1000. - 3. * 10.);
}

#[test]
fn hit_knocks_target_away() {
    let mut app = app();
    spawn_player(&mut app, Vec2::ZERO);
    // can't hit back, so the player stays in place
    let enemy = common::spawn(
        &mut app,
        (SMALL, SMALL.physics(), Health::new(1000.)),
        Vec2::new(11., 0.),
    );

    common::run(&mut app, 0.3);

    let position = common::position(&app, enemy);
    assert!(position.x > 30., "enemy at {position}");
    assert!(position.y.abs() < 1., "enemy at {position}");
}

#[test]
fn heavier_targets_are_knocked_back_less() {
    let knocked_back = |enemy: Enemy| {
        let mut app = app();
        spawn_player(&mut app, Vec2::ZERO);
        let start = Vec2::new(player::SHAPE.radius + enemy.radius + 1., 0.);
        let entity = common::spawn(
            &mut app,
            (enemy, enemy.physics(), Health::new(1000.)),
            start,
        );
        common::run(&mut app, 0.3);
        common::position(&app, entity).x - start.x
    };

    let (small, large) = (knocked_back(SMALL), knocked_back(LARGE));
    assert!(small > large * 2., "small {small} px, large {large} px");
}

#[test]
fn dead_enemies_are_despawned() {
    let mut app = app();
    spawn_player(&mut app, Vec2::ZERO);
    let enemy = common::spawn(
        &mut app,
        (SMALL, SMALL.physics(), Health::new(5.)),
        Vec2::new(11., 0.),
    );

    common::run(&mut app, 0.1);

    assert!(app.world().get_entity(enemy).is_err());
}

#[test]
fn player_death_starts_over() {
    let mut app = app();
    let player = spawn_player(&mut app, Vec2::ZERO);
    app.world_mut().get_mut::<Health>(player).unwrap().current = 1.;
    spawn_enemy(&mut app, SMALL, Vec2::new(11., 0.));
    spawn_enemy(&mut app, SMALL, Vec2::new(-11., 0.));

    common::run(&mut app, 0.1);

    assert_eq!(health(&app, player), Health::new(player::MAX_HEALTH));
    let enemies = app
        .world_mut()
        .query_filtered::<(), With<Enemy>>()
        .iter(app.world())
        .count();
    assert_eq!(enemies, 0);
}
