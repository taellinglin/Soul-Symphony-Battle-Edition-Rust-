use crate::components::Spatial4D;
use bevy::prelude::*;

use crate::ai::{Boss, Monster};
use bevy_rapier3d::prelude::*;

pub const BOSS_ARENA_MAJOR_RADIUS: f32 = 18.0;
pub const BOSS_ARENA_MINOR_RADIUS: f32 = 6.0;
pub const BOSS_ARENA_WALL_HEIGHT: f32 = 8.0;

pub struct BossPlugin;

impl Plugin for BossPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BossArenaState>()
            .add_systems(Update, (check_boss_trigger, update_boss_platforms));
    }
}

#[derive(Resource, Default)]
pub struct BossArenaState {
    pub is_active: bool,
    pub arena_center: Option<Vec3>,
    pub return_pos: Option<Vec3>,
    pub stored_environment: bool, // When true, normal dungeon is hidden
}

#[derive(Component)]
pub struct BossArenaEntity;

#[derive(Component)]
pub struct BossPlatformMover {
    pub base_pos: Vec3,
    pub axis: Vec3,
    pub amp: f32,
    pub speed: f32,
    pub phase: f32,
}

#[derive(bevy::ecs::system::SystemParam)]
struct BossTriggerParams<'w, 's> {
    state: ResMut<'w, BossArenaState>,
    player_query: Query<
        'w,
        's,
        (
            &'static mut Transform,
            &'static crate::systems::progression::PlayerProgression,
        ),
        With<crate::player::Player>,
    >,
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
    boss_materials: ResMut<'w, Assets<crate::rendering::BossHyperMaterial>>,
    monster_query: Query<'w, 's, &'static crate::ai::Monster>,
    map_geometry_query: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Visibility,
            Option<&'static crate::map::StashedMap>,
        ),
        With<crate::map::MapGeometry>,
    >,
    boss_arena_query: Query<'w, 's, Entity, With<BossArenaEntity>>,
    fx_events: EventWriter<'w, crate::effects::FloatingTextEvent>,
    config: Res<'w, crate::map::GenerationConfig>,
}

fn check_boss_trigger(mut params: BossTriggerParams) {
    if params.state.is_active {
        let boss_alive = params.monster_query.iter().any(|m| m.is_boss);
        if !boss_alive && params.state.stored_environment {
            for (entity, mut vis, stashed) in params.map_geometry_query.iter_mut() {
                if let Some(stash) = stashed {
                    *vis = stash.0;
                    params
                        .commands
                        .entity(entity)
                        .remove::<crate::map::StashedMap>();
                }
            }
            params.state.stored_environment = false;
            params.state.is_active = false;

            if let Ok((mut pt, _)) = params.player_query.get_single_mut() {
                if let Some(return_p) = params.state.return_pos {
                    pt.translation = return_p;
                }
            }

            for entity in params.boss_arena_query.iter() {
                params.commands.entity(entity).despawn_recursive();
            }
        }
        return;
    }

    let condition_met = if let Ok((_, player_lvl)) = params.player_query.get_single() {
        player_lvl.level > 2 || (player_lvl.xp > 30.0 && params.monster_query.is_empty())
    } else {
        false
    };

    if condition_met {
        if let Ok((mut pt, _)) = params.player_query.get_single_mut() {
            params.state.return_pos = Some(pt.translation);
            params.state.is_active = true;

            let isolated_center = Vec3::new(1000.0, 1000.0, 1000.0);
            params.state.arena_center = Some(isolated_center);

            for (entity, mut vis, stashed) in params.map_geometry_query.iter_mut() {
                if stashed.is_none() {
                    params
                        .commands
                        .entity(entity)
                        .insert(crate::map::StashedMap(*vis));
                    *vis = Visibility::Hidden;
                }
            }
            params.state.stored_environment = true;

            build_hex_boss_arena(
                &mut params.commands,
                &mut params.meshes,
                &mut params.materials,
                &mut params.boss_materials,
                isolated_center,
                params.config.scale,
            );

            params.fx_events.send(crate::effects::FloatingTextEvent {
                pos: isolated_center + Vec3::new(0.0, BOSS_ARENA_WALL_HEIGHT * 0.625, 0.0),
                text: "BOSS ROOM".to_string(),
                color: Color::srgba(1.0, 0.45, 0.9, 1.0),
                scale: 0.34,
                life: 1.6,
            });

            pt.translation = isolated_center + Vec3::new(0.0, 2.0, 0.0);
        }
    }
}

fn update_boss_platforms(time: Res<Time>, mut query: Query<(&mut Transform, &BossPlatformMover)>) {
    let t = time.elapsed_seconds();
    for (mut tf, mover) in query.iter_mut() {
        let offset = mover.axis * (t * mover.speed + mover.phase).sin() * mover.amp;
        tf.translation = mover.base_pos + offset;
    }
}

pub fn build_hex_boss_arena(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    mut _boss_materials: &mut ResMut<Assets<crate::rendering::BossHyperMaterial>>,
    center: Vec3,
    _arena_scale: f32,
) {
    // Spawn Boss Monster on the hub
    let boss_spawn_pos = Vec3::new(center.x, 1.2, center.z);

    // Create base 4D Boss component
    let boss_health = 2700.0;
    commands
        .spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(2.4, 3.2, 2.4)), // Boss size proxy
                material: materials.add(StandardMaterial {
                    base_color: Color::srgba(0.9, 0.1, 0.2, 1.0),
                    emissive: LinearRgba::new(0.8, 0.05, 0.1, 1.0),
                    ..default()
                }),
                transform: Transform::from_translation(boss_spawn_pos),
                ..default()
            },
            RigidBody::Dynamic,
            Collider::cuboid(1.2, 1.6, 1.2),
            LockedAxes::ROTATION_LOCKED, // keep upright
            Velocity::default(),
            ExternalForce::default(),
            ExternalImpulse::default(),
            crate::ai::Mob,
            Monster {
                variant: crate::ai::MonsterVariant::Juggernaut, // Or a dedicated Boss variant
                state: crate::ai::AiState::Guarding,            // Starting state
                attack_mult: 2.8,
                defense: 1.25,
                hunt_range: BOSS_ARENA_MINOR_RADIUS * 2.5,
                attack_range: 3.5,
                guard_range: BOSS_ARENA_MAJOR_RADIUS + 4.0,
                speed_boost: 1.4,
                ai_state_timer: 0.0,
                is_docile: false,
                awakened: true,
                is_boss: true,
                teleport_enabled: true,
                teleport_cooldown: 5.0,
                liminal_enabled: true,
                fold_jump_cooldown: 3.0,
                ranged_enabled: true,
                ranged_cooldown: 1.5,
                cosmic_warp_cooldown: 0.0,
                last_announced_state: None,
            },
            Boss {
                dash_cooldown: Timer::from_seconds(3.0, TimerMode::Repeating),
            },
            crate::components::Health {
                current: boss_health,
                max: boss_health,
            },
            Spatial4D {
                w: 0.0,
                target_w: 0.0,
                layer: 0,
                is_folded: false,
            },
            crate::components::Velocity4D {
                lin_v: Vec3::ZERO,
                w_v: 0.0,
            },
            crate::components::Collision4D {
                radius: 1.5,
                mask: 2,
            },
            BossArenaEntity,
        ))
        .insert(crate::ai::KnockbackVel::default());
}
