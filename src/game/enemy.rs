use super::{
    ApplyVelocity, Collider, Health, Home, Level, Planet, Player, SpaceShip, SpaceShipBundle,
    SpawnExplosion, Star, Steering, UpdateSpaceShip, Velocity,
};
use crate::{assets::GameAssets, AppState};
use bevy::prelude::*;
use rand::Rng;

pub struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        // Setup and cleanup
        app.add_systems(OnEnter(AppState::Game), setup);
        app.add_systems(OnExit(AppState::Game), cleanup);

        // Update
        app.add_systems(Update, spawn_enemies.run_if(in_state(AppState::Game)));
        app.add_systems(
            Update,
            (update, despawn_enemies)
                .before(UpdateSpaceShip)
                .before(ApplyVelocity)
                .run_if(in_state(AppState::Game)),
        );
    }
}

#[derive(Debug, Resource)]
struct EnemySpawner {
    timer: Timer,
}

#[derive(Debug, Component)]
pub struct Enemy {
    target: Option<EnemyTarget>,
    last_shot: f32,
}

impl Enemy {
    pub fn new() -> Self {
        Self {
            target: None,
            last_shot: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EnemyTarget {
    Player,
    Home,
}

#[derive(Bundle)]
pub struct EnemyBundle {
    pub enemy: Enemy,
    pub health: Health,
    pub space_ship: SpaceShipBundle,
}

impl EnemyBundle {
    pub fn new(position: Vec3, rotation: f32, level: &Level, assets: &GameAssets) -> Self {
        Self {
            enemy: Enemy::new(),
            health: level.enemy_health,
            space_ship: SpaceShipBundle::new(
                0b10,
                Velocity(Vec3::ZERO),
                position,
                rotation,
                assets.enemy_space_ship_material.clone(),
                assets.enemy_bullet_material.clone(),
                assets,
            ),
        }
    }
}

fn spawn_enemies(
    mut commands: Commands,
    mut enemy_spawner: ResMut<EnemySpawner>,
    time: Res<Time>,
    level: Res<Level>,
    assets: Res<GameAssets>,
) {
    if enemy_spawner.timer.tick(time.delta()).just_finished() {
        let alpha = rand::thread_rng().gen_range(0.0..std::f32::consts::TAU);
        commands.spawn((
            EnemyBundle::new(
                Vec3::new(f32::cos(alpha) * 512.0, f32::sin(alpha) * 512.0, 0.0),
                alpha + std::f32::consts::FRAC_PI_2,
                &level,
                &assets,
            ),
            StateScoped(AppState::Game),
        ));
    }
}

fn update(
    time: Res<Time>,
    level: Res<Level>,
    mut enemies: Query<(&Transform, &mut SpaceShip, &mut Enemy), Without<Player>>,
    players: Query<&Transform, With<Player>>,
    homes: Query<(Entity, &Transform), With<Home>>,
) {
    let Ok(player) = players.get_single() else {
        return;
    };
    let Ok((home_entity, home)) = homes.get_single() else {
        return;
    };

    for (transform, mut space_ship, mut enemy) in &mut enemies {
        let target = *enemy.target.get_or_insert_with(|| {
            let distance_to_player = Vec3::distance(player.translation, transform.translation);
            let distance_to_home = Vec3::distance(home.translation, transform.translation);
            if distance_to_player < distance_to_home {
                EnemyTarget::Player
            } else {
                EnemyTarget::Home
            }
        });
        let (target_transform, throttle_threshold, brake_threshold, shoot_threshold, reload) =
            match target {
                EnemyTarget::Player => (*player, 100.0, 50.0, 250.0, 0.5),
                EnemyTarget::Home => (*home, 300.0, 300.0, 500.0, 1.0),
            };

        let direction = target_transform.translation - transform.translation;
        let distance = direction.length();
        let direction = direction.normalize();
        let angle_between = direction
            .angle_between(space_ship.rot_quat() * Vec3::Y)
            .to_degrees();

        space_ship.steering = match angle_between {
            angle if angle > 5.0 => Steering::Right,
            angle if angle < -5.0 => Steering::Left,
            _ => Steering::None,
        };
        space_ship.throttle = distance > throttle_threshold;
        space_ship.brake = distance < brake_threshold;
        space_ship.shoot = (distance < shoot_threshold
            && angle_between.abs() < 10.0
            && time.elapsed_seconds() - enemy.last_shot > reload)
            .then_some(level.enemy_damage);
        space_ship.shoot_missile_lock = match target {
            EnemyTarget::Player => None,
            EnemyTarget::Home => Some(home_entity),
        };

        if space_ship.shoot.is_some() {
            enemy.last_shot = time.elapsed_seconds();
        }
    }
}

fn despawn_enemies(
    mut commands: Commands,
    mut explosions: EventWriter<SpawnExplosion>,
    enemies: Query<(Entity, &Transform, &Collider, &Health, &SpaceShip), With<Enemy>>,
    planets_and_stars: Query<
        (&Transform, &Collider),
        (Without<Enemy>, Or<(With<Planet>, With<Star>)>),
    >,
) {
    for (entity, transform, collider, health, space_ship) in &enemies {
        let mut despawn = health.current() <= 0.0;

        if !despawn {
            for (obj_transform, obj_collider) in &planets_and_stars {
                if Vec3::distance_squared(transform.translation, obj_transform.translation)
                    <= f32::powi(collider.radius + obj_collider.radius, 2)
                {
                    despawn = true;
                    break;
                }
            }
        }

        if despawn {
            explosions.send(SpawnExplosion {
                position: transform.translation,
                material: space_ship.material(),
            });
            commands.entity(entity).despawn();
        }
    }
}

fn setup(mut commands: Commands, level: Res<Level>) {
    commands.insert_resource(EnemySpawner {
        timer: Timer::from_seconds(level.enemy_spawn_interval, TimerMode::Repeating),
    });
}

fn cleanup(mut commands: Commands) {
    commands.remove_resource::<EnemySpawner>();
}
