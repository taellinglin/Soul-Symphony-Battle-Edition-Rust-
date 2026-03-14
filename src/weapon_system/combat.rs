use crate::ai::{AiState, Monster, MonsterVariant};
use crate::components::{Health, Spatial4D};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_rapier3d::prelude::*;
use rand::Rng;

use super::components::*;

type WeaponQueryResult = (
    &'static mut Weapon,
    &'static mut Transform,
    &'static mut Spatial4D,
);

type PlayerQueryResult = (
    &'static Transform,
    &'static Spatial4D,
    Option<&'static crate::systems::progression::PlayerCombatStats>,
);

type EnemyQueryResult = (
    Entity,
    &'static Transform,
    &'static Spatial4D,
    &'static Health,
    &'static Monster,
);

type DamageQueryResult = (
    &'static mut Health,
    &'static Transform,
    Option<&'static crate::ai::Monster>,
    Option<&'static mut Velocity>,
    Option<&'static mut crate::ai::KnockbackVel>,
);

type WeaponSet<'w, 's> = ParamSet<
    'w,
    's,
    (
        Query<'w, 's, WeaponQueryResult>,
        Query<'w, 's, (Entity, &'static Weapon, &'static Transform)>,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct WeaponUpdateParams<'w, 's> {
    commands: Commands<'w, 's>,
    weapon_set: WeaponSet<'w, 's>,
    player_query: Query<'w, 's, PlayerQueryResult, (With<crate::player::Player>, Without<Weapon>)>,
    enemy_query: Query<'w, 's, EnemyQueryResult, (Without<Weapon>, Without<crate::player::Player>)>,
    gravity: Res<'w, crate::player::GravityDirection>,
    orbit: Res<'w, crate::player::CameraOrbitState>,
    timers: Res<'w, crate::player::PhysicsTimers>,
    time: Res<'w, Time>,
    sfx_events: EventWriter<'w, crate::effects::audio::PlaySfxEvent>,
    damage_events: EventWriter<'w, DamageEvent>,
    fx_events: EventWriter<'w, crate::effects::FloatingTextEvent>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<StandardMaterial>>,
}

pub(crate) fn update_weapon_state(mut params: WeaponUpdateParams) {
    let dt = params.time.delta_seconds();
    let Ok((player_tf, player_sp, opt_stats)) = params.player_query.get_single() else {
        return;
    };
    let dmg_mult = opt_stats
        .map(|s| s.sword_dmg_mult.max(0.5) * 1.0_f32.max(0.55))
        .unwrap_or(0.55);
    let input_up = -params.gravity.0.normalize_or_zero();

    {
        for (mut weapon, mut tf, mut weapon_sp) in params.weapon_set.p0().iter_mut() {
            if let Ok((_, player_sp, _)) = params.player_query.get_single() {
                weapon_sp.w = player_sp.w;
                weapon_sp.target_w = player_sp.target_w;
                weapon_sp.layer = player_sp.layer;
            }

            let mut desired_forward = params.timers.last_move_dir;
            desired_forward -= input_up * desired_forward.dot(input_up);
            if desired_forward.length_squared() < 1e-6 {
                let yaw = params.orbit.heading.to_radians();
                desired_forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
            } else {
                desired_forward = desired_forward.normalize();
            }

            let forward_blend = (dt * 10.5).min(1.0);
            let mut new_fwd =
                weapon.weapon_forward + (desired_forward - weapon.weapon_forward) * forward_blend;
            if new_fwd.length_squared() > 1e-6 {
                new_fwd = new_fwd.normalize();
            }
            weapon.weapon_forward = new_fwd;

            let mut right = input_up.cross(new_fwd);
            if right.length_squared() < 1e-8 {
                let yaw = params.orbit.heading.to_radians();
                right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
            } else {
                right = right.normalize();
            }

            let desired_anchor = player_tf.translation
                + input_up * UP_OFFSET
                + new_fwd * FWD_OFFSET
                + right * SIDE_OFFSET;

            let follow_alpha = 1.0 - (-dt * 19.0).exp();
            let new_anchor =
                weapon.anchor_pos + (desired_anchor - weapon.anchor_pos) * follow_alpha;
            weapon.anchor_pos = new_anchor;

            let mut heading = (-new_fwd.x).atan2(-new_fwd.z).to_degrees();
            let mut yaw_offset: f32 = -16.0;
            let mut pitch: f32 = -18.0;
            let mut roll: f32 = 0.0;
            let mut pivot_pos = weapon.anchor_pos;
            let mut emit_echo = false;

            match weapon.state {
                WeaponState::Idle => {
                    weapon.prev_tip_pos = None;
                }
                WeaponState::Spin => {
                    weapon.timer += dt;
                    let total = 0.32;
                    let t = (weapon.timer / total).min(1.0);
                    yaw_offset = -180.0 + 540.0 * t;
                    pitch = -14.0;
                    roll = 18.0 * (t * std::f32::consts::TAU).sin();
                    emit_echo = true;

                    if t >= 1.0 {
                        weapon.state = WeaponState::Idle;
                        weapon.timer = 0.0;
                        weapon.hit_targets.clear();
                        weapon.prev_tip_pos = None;
                    }
                }
                WeaponState::Throw => {
                    weapon.timer += dt;
                    let total = 0.56;
                    let outbound = 0.22;
                    let distance = REACH * 2.6;

                    if weapon.throw_origin.is_none() {
                        weapon.throw_origin = Some(weapon.anchor_pos);
                    }
                    let origin = weapon.throw_origin.unwrap();
                    let dir = weapon.throw_dir;

                    let t = (weapon.timer / total).min(1.0);
                    let mut vel_sign = 1.0;

                    let forward_amount = if t <= (outbound / total) {
                        let out_t = t / (outbound / total).max(1e-6);
                        1.0 - (1.0 - out_t) * (1.0 - out_t)
                    } else {
                        let back_t =
                            (t - (outbound / total)) / (1.0 - (outbound / total)).max(1e-6);
                        vel_sign = -1.0;
                        (1.0 - back_t) * (1.0 - back_t)
                    };

                    let arc = (t * std::f32::consts::PI).sin() * distance * 0.16;
                    let right_throw = input_up.cross(dir).normalize_or_zero();
                    let throw_pos = origin + dir * (distance * forward_amount) + right_throw * arc;
                    pivot_pos =
                        throw_pos + input_up * (0.08 + 0.12 * (t * std::f32::consts::PI).sin());

                    let fw_heading = (-dir.x).atan2(-dir.z).to_degrees();
                    heading = if vel_sign >= 0.0 {
                        fw_heading
                    } else {
                        fw_heading + 180.0
                    };
                    yaw_offset = 0.0;
                    pitch = -8.0 + 5.0 * (t * std::f32::consts::PI).sin();
                    roll = (weapon.timer * 1080.0) % 360.0;
                    emit_echo = true;

                    if t >= 1.0 {
                        weapon.state = WeaponState::Idle;
                        weapon.timer = 0.0;
                        weapon.throw_origin = None;
                        weapon.hit_targets.clear();
                        weapon.prev_tip_pos = None;
                    }
                }
                WeaponState::Hyperbomb => {
                    let ball_r = 0.68;
                    params.commands.spawn((
                        PbrBundle {
                            mesh: params.meshes.add(Sphere::new(0.1)),
                            material: params.materials.add(StandardMaterial {
                                base_color: Color::srgba(0.2, 0.9, 1.0, 0.45),
                                emissive: LinearRgba::new(0.5, 1.0, 1.0, 1.0),
                                unlit: true,
                                alpha_mode: AlphaMode::Blend,
                                ..default()
                            }),
                            transform: Transform::from_translation(player_tf.translation),
                            ..default()
                        },
                        Hyperbomb {
                            timer: 0.0,
                            duration: 6.0,
                            radius: 0.1,
                            max_radius: ball_r * 8.2,
                        },
                        RenderLayers::layer(0),
                    ));
                    weapon.state = WeaponState::Idle;
                    weapon.timer = 0.0;
                }
                WeaponState::MagicMissile => {
                    let count = 6;
                    let forward = weapon.weapon_forward;
                    let up = Vec3::Y;
                    let right = up.cross(forward).normalize_or_zero();

                    for idx in 0..count {
                        let spread = idx as f32 - (count as f32 - 1.0) * 0.5;
                        let spawn_pos = player_tf.translation
                            + right * (spread * 0.55)
                            + up * (0.16 * spread.abs());

                        params.commands.spawn((
                            PbrBundle {
                                mesh: params.meshes.add(Sphere::new(0.15)),
                                material: params.materials.add(StandardMaterial {
                                    base_color: Color::srgba(1.0, 0.4, 0.2, 0.8),
                                    emissive: LinearRgba::new(1.0, 0.5, 0.2, 1.0),
                                    unlit: true,
                                    alpha_mode: AlphaMode::Blend,
                                    ..default()
                                }),
                                transform: Transform::from_translation(spawn_pos),
                                ..default()
                            },
                            MagicMissile {
                                velocity: forward * 25.0,
                                target: None,
                                life: 4.2,
                            },
                            Spatial4D {
                                w: player_sp.w,
                                target_w: player_sp.w,
                                layer: player_sp.layer,
                                is_folded: false,
                            },
                            RenderLayers::layer(0),
                        ));
                    }
                    weapon.state = WeaponState::Idle;
                    weapon.timer = 0.0;
                }
            }

            tf.translation = pivot_pos;
            tf.rotation = Quat::from_euler(
                EulerRot::YXZ,
                (heading + yaw_offset).to_radians(),
                (-pitch).to_radians(),
                (-roll).to_radians(),
            );

            if emit_echo {
                weapon.echo_timer -= dt;
                if weapon.echo_timer <= 0.0 {
                    weapon.echo_timer = 1.0 / 120.0;
                }
            }
        }
    }

    let mut hits = Vec::new();
    {
        for (weapon_ent, weapon, tf) in params.weapon_set.p1().iter() {
            if weapon.state == WeaponState::Idle {
                continue;
            }
            let is_spin = weapon.state == WeaponState::Spin;
            let is_throw = weapon.state == WeaponState::Throw;

            let throw_rad = 0.95 + SWORD_SCALE * 0.22;
            let swing_fwd = weapon.weapon_forward;

            for (enemy_ent, enemy_tf, enemy_sp, enemy_hp, monster) in params.enemy_query.iter() {
                if enemy_hp.current <= 0.0 {
                    continue;
                }
                if weapon.hit_targets.contains(&enemy_ent) {
                    continue;
                }

                let to_enemy = enemy_tf.translation - tf.translation;
                let mut planar = to_enemy;
                planar.y = 0.0;
                let planar_dist = planar.length();
                let enemy_radius = 1.0;

                let dw_scaled = (enemy_sp.w - player_sp.w) * 4.0;
                let dist_4d = (planar_dist * planar_dist + dw_scaled * dw_scaled).sqrt();

                let mut register_hit = false;
                let mut damage = 0.0;
                let mut knock_mag = 0.0;
                let mut away = to_enemy;

                if is_throw {
                    let max_hit = throw_rad + enemy_radius;
                    if dist_4d <= max_hit {
                        register_hit = true;
                        damage = 44.0 * dmg_mult;
                        knock_mag = 4.8;
                    }
                } else {
                    let max_reach = REACH * 1.0 + enemy_radius * 0.65;
                    if dist_4d <= max_reach {
                        if !is_spin {
                            if planar_dist > 1e-6 {
                                let planar_dir = planar.normalize();
                                if planar_dir.dot(swing_fwd) >= 0.12 {
                                    register_hit = true;
                                    damage = 36.0 * dmg_mult;
                                    knock_mag = 3.8;
                                }
                            }
                        } else {
                            register_hit = true;
                            damage = 54.0 * dmg_mult;
                            knock_mag = 5.4;
                        }
                    }
                }

                if register_hit {
                    let guard_chance = match monster.state {
                        AiState::Guarding => {
                            0.58 + if matches!(
                                monster.variant,
                                MonsterVariant::Juggernaut | MonsterVariant::Vanguard
                            ) {
                                0.18
                            } else {
                                0.0
                            }
                        }
                        AiState::Attacking => {
                            0.20 + if matches!(
                                monster.variant,
                                MonsterVariant::Juggernaut | MonsterVariant::Vanguard
                            ) {
                                0.18
                            } else {
                                0.0
                            }
                        }
                        _ => 0.0,
                    };
                    if guard_chance > 0.0 && rand::thread_rng().gen::<f32>() < guard_chance {
                        params.sfx_events.send(crate::effects::audio::PlaySfxEvent {
                            kind: crate::effects::audio::SfxKind::MonsterHit,
                            volume: 0.5,
                            pitch: 0.9,
                            position: Some(enemy_tf.translation),
                        });
                        params.fx_events.send(crate::effects::FloatingTextEvent {
                            pos: enemy_tf.translation + Vec3::new(0.0, 1.1, 0.0),
                            text: "GUARD".to_string(),
                            color: Color::srgba(0.9, 0.85, 0.3, 1.0),
                            scale: 0.28,
                            life: 0.5,
                        });
                        continue;
                    }
                    let applied = (damage / monster.defense).max(1.0);
                    away.y = 0.0;
                    if away.length_squared() > 1e-6 {
                        away = away.normalize();
                    }
                    let pop_up = if is_throw { 0.42 } else { 0.35 };
                    let knockback = away * knock_mag + Vec3::Y * pop_up;
                    hits.push((weapon_ent, enemy_ent, applied, knockback));
                }
            }
        }
    }

    for (weapon_ent, enemy_ent, damage, knockback) in hits {
        if let Ok((mut weapon, _, _)) = params.weapon_set.p0().get_mut(weapon_ent) {
            weapon.hit_targets.insert(enemy_ent);
        }

        if let Ok((_, enemy_tf, _, _, _)) = params.enemy_query.get(enemy_ent) {
            params.sfx_events.send(crate::effects::audio::PlaySfxEvent {
                kind: crate::effects::audio::SfxKind::MonsterHit,
                volume: 0.8,
                pitch: 1.0,
                position: Some(enemy_tf.translation),
            });
        }

        params.damage_events.send(DamageEvent {
            target: enemy_ent,
            amount: damage,
            knockback,
        });
    }
}

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct DamageEventParams<'w, 's> {
    events: EventReader<'w, 's, DamageEvent>,
    item_events: EventWriter<'w, crate::world::items::SpawnItemEvent>,
    commands: Commands<'w, 's>,
    query: Query<'w, 's, DamageQueryResult>,
    sfx_events: EventWriter<'w, crate::effects::audio::PlaySfxEvent>,
    fx_events: EventWriter<'w, crate::effects::FloatingTextEvent>,
    monster_stats: ResMut<'w, crate::systems::progression::MonsterStats>,
    kill_protection: ResMut<'w, crate::systems::progression::KillProtection>,
}

pub(crate) fn apply_damage_events(mut params: DamageEventParams) {
    for ev in params.events.read() {
        if let Ok((mut health, tf, opt_monster, opt_vel, opt_knock)) =
            params.query.get_mut(ev.target)
        {
            health.current -= ev.amount;

            if let Some(mut knock) = opt_knock {
                knock.0 += ev.knockback;
            } else if let Some(mut vel) = opt_vel {
                vel.linvel += ev.knockback;
            }

            params.fx_events.send(crate::effects::FloatingTextEvent {
                pos: tf.translation + Vec3::new(0.0, 1.1, 0.0),
                text: format!("HP -{}", ev.amount as i32),
                color: Color::srgba(0.3, 0.7, 1.0, 1.0),
                scale: 0.24,
                life: 0.7,
            });

            if health.current <= 0.0 {
                params.kill_protection.stacks =
                    (params.kill_protection.stacks + 1).min(params.kill_protection.max_stacks);
                let base_xp = 2.0 + health.max * 0.04;
                let variant_mult = opt_monster
                    .map(|m| match m.variant {
                        crate::ai::MonsterVariant::Juggernaut
                        | crate::ai::MonsterVariant::Vanguard => 1.45,
                        crate::ai::MonsterVariant::Raider => 1.25,
                        crate::ai::MonsterVariant::Giant => 1.75,
                        crate::ai::MonsterVariant::Normal => 1.0,
                    })
                    .unwrap_or(1.0);
                let xp_amount = base_xp * variant_mult;
                params
                    .item_events
                    .send(crate::world::items::SpawnItemEvent::Exp {
                        pos: tf.translation,
                        amount: xp_amount,
                    });

                params.sfx_events.send(crate::effects::audio::PlaySfxEvent {
                    kind: crate::effects::audio::SfxKind::MonsterDie,
                    volume: 0.9,
                    pitch: 1.0,
                    position: Some(tf.translation),
                });

                params.monster_stats.slain += 1;

                params.commands.entity(ev.target).despawn_recursive();
            }
        }
    }
}
