use crate::common::{self, SMALL};
use avian2d::prelude::*;
use bevy::prelude::*;
use std::time::Duration;
use survival::{
    combat::{
        self, Health, Striking,
        tokens::{AttackToken, AttackTokens},
    },
    flowfield::{self, FlowField},
    objects::{
        enemy::{
            self, Enemy,
            swarm::{self, Orbit, Swarm, SwarmRng, SwarmState, SwarmValues},
        },
        player,
    },
    world::MapArea,
};

/// The values of `assets/swarm.ron`.
fn values() -> SwarmValues {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/swarm.ron");
    ron::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// App with combat and the swarm, with the values of `assets/swarm.ron` but `max_tokens`.
fn app(max_tokens: usize) -> App {
    let mut app = common::app_with((combat::Plugin, swarm::Plugin));
    app.insert_resource(values())
        .insert_resource(AttackTokens { max: max_tokens })
        .insert_resource(SwarmRng(fastrand::Rng::with_seed(7)))
        .init_resource::<FlowField>();
    app
}

/// A player that doesn't attack.
fn spawn_idle_player(app: &mut App) -> Entity {
    common::spawn(
        app,
        (
            player::Player,
            player::physics(),
            Health::new(player::MAX_HEALTH),
        ),
        Vec2::ZERO,
    )
}

fn spawn_swarm_enemy(app: &mut App, position: Vec2) -> Entity {
    common::spawn(
        app,
        (SMALL, SMALL.physics(), SMALL.combat(), Swarm),
        position,
    )
}

/// `count` swarm enemies at 70 px around the origin.
fn spawn_swarm(app: &mut App, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|i| {
            let angle = i as f32 / count as f32 * std::f32::consts::TAU;
            spawn_swarm_enemy(app, Vec2::from_angle(angle) * 70.)
        })
        .collect()
}

fn state(app: &App, enemy: Entity) -> Option<SwarmState> {
    app.world().get::<SwarmState>(enemy).cloned()
}

fn is_approaching(app: &App, enemy: Entity) -> bool {
    matches!(state(app, enemy), Some(SwarmState::Approach { .. }))
}

fn is_retreating(app: &App, enemy: Entity) -> bool {
    matches!(state(app, enemy), Some(SwarmState::Retreat { .. }))
}

fn has_token(app: &App, enemy: Entity) -> bool {
    app.world().get::<AttackToken>(enemy).is_some()
}

fn is_striking(app: &App, enemy: Entity) -> bool {
    app.world().get::<Striking>(enemy).is_some()
}

#[test]
fn enemy_wakes_up_circling() {
    let mut app = app(0);
    spawn_idle_player(&mut app);
    let enemy = spawn_swarm_enemy(&mut app, Vec2::new(70., 0.));

    app.update();

    assert!(matches!(
        state(&app, enemy),
        Some(SwarmState::Circle { .. })
    ));
    let orbit = app.world().get::<Orbit>(enemy).unwrap();
    let (min, max) = values().circle_distance;
    assert!((min..=max).contains(&orbit.distance), "{orbit:?}");
}

#[test]
fn circling_enemy_keeps_its_distance_and_moves_around() {
    let mut app = app(0);
    spawn_idle_player(&mut app);
    let enemy = spawn_swarm_enemy(&mut app, Vec2::new(30., 0.));

    common::run(&mut app, 3.);
    let before = common::position(&app, enemy);
    common::run(&mut app, 0.5);
    let after = common::position(&app, enemy);

    let distance = app.world().get::<Orbit>(enemy).unwrap().distance;
    assert!(
        (after.length() - distance).abs() < 5.,
        "enemy {} px away instead of {distance}",
        after.length()
    );
    let turned = before.angle_to(after).abs();
    assert!(turned > 0.2, "turned {turned} rad around the player");
}

#[test]
fn only_enemies_with_a_token_approach() {
    let mut app = app(2);
    spawn_idle_player(&mut app);
    let enemies = spawn_swarm(&mut app, 5);

    let mut most = 0;
    let mut approached = vec![false; enemies.len()];
    for _ in 0..64 * 10 {
        app.update();
        let mut holding = 0;
        for (i, &enemy) in enemies.iter().enumerate() {
            // tokens are held a while longer than the approach
            if is_approaching(&app, enemy) {
                assert!(has_token(&app, enemy));
                approached[i] = true;
            }
            assert_eq!(is_approaching(&app, enemy), is_striking(&app, enemy));
            holding += has_token(&app, enemy) as usize;
        }
        most = most.max(holding);
    }

    assert_eq!(most, 2);
    // the longest waiting goes first, so nobody waits forever
    assert!(approached.iter().all(|&a| a), "{approached:?}");
}

#[test]
fn enemy_retreats_after_hitting_the_player() {
    let mut app = app(1);
    let player = spawn_idle_player(&mut app);
    let enemy = spawn_swarm_enemy(&mut app, Vec2::new(70., 0.));

    let mut frames = 0;
    while common::health(&app, player).current == player::MAX_HEALTH {
        app.update();
        frames += 1;
        assert!(frames < 64 * 2, "the enemy never hit the player");
    }
    app.update();

    assert_eq!(
        common::health(&app, player).current,
        player::MAX_HEALTH - values().damage
    );
    assert!(is_retreating(&app, enemy), "{:?}", state(&app, enemy));
    assert!(!is_striking(&app, enemy));
}

#[test]
fn token_is_given_back_its_time_after_the_approach_began() {
    let mut app = app(1);
    spawn_idle_player(&mut app);
    let enemies = spawn_swarm(&mut app, 2);

    // frames at which each enemy started approaching
    let mut starts = vec![vec![]; enemies.len()];
    for frame in 0..64 * 6 {
        let before: Vec<_> = enemies.iter().map(|&e| is_approaching(&app, e)).collect();
        app.update();
        for (i, &enemy) in enemies.iter().enumerate() {
            if is_approaching(&app, enemy) && !before[i] {
                starts[i].push(frame);
            }
        }
    }

    let mut all: Vec<_> = starts.concat();
    all.sort();
    assert!(all.len() >= 2, "approaches at frames {all:?}");
    let token_time = (values().token_time * 64.) as i32;
    for pair in all.windows(2) {
        let gap = pair[1] - pair[0];
        // the next approach starts in the frame after the token is given back
        assert!(
            (gap - token_time).abs() <= 2,
            "approaches at frames {all:?}, {gap} frames apart"
        );
    }
}

#[test]
fn hit_by_the_player_while_approaching_the_enemy_retreats() {
    let mut app = app(1);
    let player = common::spawn(
        &mut app,
        (player::Player, player::physics(), player::combat()),
        Vec2::ZERO,
    );
    let enemy = spawn_swarm_enemy(&mut app, Vec2::new(70., 0.));

    let mut frames = 0;
    while common::health(&app, enemy).current == common::health(&app, enemy).max {
        app.update();
        frames += 1;
        assert!(frames < 64 * 2, "the player never hit the enemy");
    }
    app.update();

    // the player reaches further, so it hits first
    assert_eq!(common::health(&app, player).current, player::MAX_HEALTH);
    assert!(is_retreating(&app, enemy), "{:?}", state(&app, enemy));
}

#[test]
fn approach_ends_after_the_timeout() {
    let mut app = app(1);
    spawn_idle_player(&mut app);
    // between the enemy and the player, the enemy walks straight without a flow field
    common::spawn(
        &mut app,
        (RigidBody::Static, Collider::rectangle(4., 100.)),
        Vec2::new(15., 0.),
    );
    let enemy = spawn_swarm_enemy(&mut app, Vec2::new(70., 0.));

    let timeout = values().approach_timeout;
    common::run(&mut app, timeout - 0.2);
    assert!(is_approaching(&app, enemy), "{:?}", state(&app, enemy));
    common::run(&mut app, 0.3);
    assert!(is_retreating(&app, enemy), "{:?}", state(&app, enemy));
}

#[test]
fn stuck_enemy_stops_retreating() {
    // no tokens, so it keeps circling afterwards
    let mut app = app(0);
    spawn_idle_player(&mut app);
    // between the enemy and its retreat point
    common::spawn(
        &mut app,
        (RigidBody::Static, Collider::rectangle(4., 100.)),
        Vec2::new(100., 0.),
    );
    let enemy = spawn_swarm_enemy(&mut app, Vec2::new(70., 0.));
    app.world_mut().entity_mut(enemy).insert((
        Orbit {
            distance: 70.,
            turn: 1.,
        },
        SwarmState::Retreat {
            target: Vec2::new(140., 0.),
            timer: Timer::from_seconds(10., TimerMode::Once),
            stuck: Duration::ZERO,
        },
    ));

    common::run(&mut app, 0.2);
    assert!(is_retreating(&app, enemy), "{:?}", state(&app, enemy));
    common::run(&mut app, 0.8);
    assert!(matches!(
        state(&app, enemy),
        Some(SwarmState::Circle { .. })
    ));
}

#[test]
fn blocked_retreat_point_is_turned_back() {
    let mut app = common::app_with(flowfield::Plugin);
    let map = app
        .world_mut()
        .spawn((
            MapArea(Rect::new(-200., -200., 200., 200.)),
            Transform::default(),
        ))
        .id();
    // covers the points turned by 120° and 105° from +x, but not by 90°
    common::spawn(
        &mut app,
        (
            RigidBody::Static,
            Collider::rectangle(60., 40.),
            ChildOf(map),
        ),
        Vec2::new(-40., 60.),
    );
    common::spawn(&mut app, player::Player, Vec2::ZERO);
    app.world_mut()
        .spawn(flowfield::Request(vec![SMALL.radius]));
    common::run(&mut app, 0.1);

    let flow_field = app.world().resource::<FlowField>();
    let target = |angle: f32| {
        swarm::retreat_target(
            flow_field,
            SMALL.radius,
            Vec2::new(30., 0.),
            Vec2::ZERO,
            70.,
            angle.to_radians(),
        )
    };
    // free to the other side
    let free = target(-120.);
    assert!(free.distance(Vec2::from_angle(-120f32.to_radians()) * 70.) < 0.01);
    let turned = target(120.);
    assert!(turned.distance(Vec2::new(0., 70.)) < 0.01, "{turned}");
}

#[test]
fn sleeping_enemy_gives_its_token_back() {
    let mut app = app(1);
    spawn_idle_player(&mut app);
    let enemies = spawn_swarm(&mut app, 2);
    common::run(&mut app, 0.1);
    let (holder, other) = if has_token(&app, enemies[0]) {
        (enemies[0], enemies[1])
    } else {
        (enemies[1], enemies[0])
    };
    assert!(!has_token(&app, other));

    app.world_mut().entity_mut(holder).insert(enemy::Asleep);
    common::run(&mut app, 0.1);

    assert!(!has_token(&app, holder));
    assert_eq!(state(&app, holder), None);
    assert!(is_approaching(&app, other));
}

#[test]
fn despawned_enemy_gives_its_token_back() {
    let mut app = app(1);
    spawn_idle_player(&mut app);
    let enemies = spawn_swarm(&mut app, 2);
    common::run(&mut app, 0.1);
    let (holder, other) = if has_token(&app, enemies[0]) {
        (enemies[0], enemies[1])
    } else {
        (enemies[1], enemies[0])
    };

    app.world_mut().despawn(holder);
    common::run(&mut app, 0.1);

    assert!(is_approaching(&app, other));
}

/// Seconds until `count` swarm enemies kill a player that doesn't fight back.
fn time_to_death(count: usize) -> f32 {
    let mut app = app(values().max_tokens);
    spawn_idle_player(&mut app);
    spawn_swarm(&mut app, count);

    let mut frames = 0;
    // the round starts over when the player dies, which removes all enemies
    while app
        .world_mut()
        .query_filtered::<(), With<Enemy>>()
        .iter(app.world())
        .next()
        .is_some()
    {
        app.update();
        frames += 1;
        assert!(frames < 64 * 120, "the player survived {count} enemies");
    }
    frames as f32 / 64.
}

#[test]
fn time_to_death_hardly_depends_on_the_number_of_enemies() {
    let five = time_to_death(5);
    let eight = time_to_death(8);
    for time in [five, eight] {
        assert!(
            (20. ..=30.).contains(&time),
            "five enemies: {five:.1} s, eight: {eight:.1} s"
        );
    }
    assert!(
        (five - eight).abs() < five * 0.1,
        "five enemies: {five:.1} s, eight: {eight:.1} s"
    );
}
