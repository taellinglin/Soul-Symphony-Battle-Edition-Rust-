use bevy::prelude::*;
use bevy::render::view::{NoFrustumCulling, RenderLayers};
use bevy_rapier3d::prelude::*;
use rand::Rng;
use crate::components::{Spatial4D, Health};
use crate::ai::{Monster, MonsterVariant, AiState};

use super::components::*;

pub(crate) fn update_weapon_state(
    mut commands: Commands,
    mut weapon_set: ParamSet<(
        Query<(&mut Weapon, &mut Transform, &mut Spatial4D)>,
        Query<(Entity, &Weapon, &Transform)>,
    )>,
    player_query: Query<(&Transform, &Spatial4D, Option<&crate::systems::progression::PlayerCombatStats>), (With<crate::player::Player>, Without<Weapon>)>,
    enemy_query: Query<(Entity, &Transform, &Spatial4D, &crate::components::Health, &Monster), (Without<Weapon>, Without<crate::player::Player>)>,
    gravity: Res<crate::player::GravityDirection>,
    orbit: Res<crate::player::CameraOrbitState>,
    timers: Res<crate::player::PhysicsTimers>,
    time: Res<Time>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
    mut damage_events: EventWriter<DamageEvent>,
    mut fx_events: EventWriter<crate::effects::FloatingTextEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_seconds();
    let Ok((player_tf, player_sp, opt_stats)) = player_query.get_single() else { return };
    let dmg_mult = opt_stats
        .map(|s| s.sword_dmg_mult.max(0.5) * 1.0_f32.max(0.55))
        .unwrap_or(0.55);
    let input_up = -gravity.0.normalize_or_zero();

    // 1. Update Weapon Motion
    {
        for (mut weapon, mut tf, mut weapon_sp) in weapon_set.p0().iter_mut() {
            // Sync weapon Spatial4D to player's (so 4D slice doesn't cull it)
            if let Ok((_, player_sp, _)) = player_query.get_single() {
                weapon_sp.w = player_sp.w;
                weapon_sp.target_w = player_sp.target_w;
                weapon_sp.layer = player_sp.layer;
            }
            // Forward blend
            let mut desired_forward = timers.last_move_dir;
            desired_forward -= input_up * desired_forward.dot(input_up);
            if desired_forward.length_squared() < 1e-6 {
                let yaw = orbit.heading.to_radians();
                desired_forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
            } else {
                desired_forward = desired_forward.normalize();
            }

            let forward_blend = (dt * 10.5).min(1.0);
            let mut new_fwd = weapon.weapon_forward + (desired_forward - weapon.weapon_forward) * forward_blend;
            if new_fwd.length_squared() > 1e-6 { new_fwd = new_fwd.normalize(); }
            weapon.weapon_forward = new_fwd;

            // Anchor follow
            let mut right = input_up.cross(new_fwd);
            if right.length_squared() < 1e-8 {
                let yaw = orbit.heading.to_radians();
                right = Vec3::new(yaw.cos(), 0.0, yaw.sin());
            } else {
                right = right.normalize();
            }

            let desired_anchor = player_tf.translation 
                + input_up * UP_OFFSET 
                + new_fwd * FWD_OFFSET 
                + right * SIDE_OFFSET;

            let follow_alpha = 1.0 - (-dt * 19.0).exp();
            let new_anchor = weapon.anchor_pos + (desired_anchor - weapon.anchor_pos) * follow_alpha;
            weapon.anchor_pos = new_anchor;
            // clamp z above floor? Skip for now

            let mut heading = (-new_fwd.x).atan2(-new_fwd.z).to_degrees();
            // Python parity: Panda3D setHpr(heading+yaw_offset, pitch, roll)
            // Panda3D pitch: positive = nose DOWN.  Bevy YXZ pitch: positive = nose UP.
            // Panda3D roll:  positive = CW.          Bevy YXZ roll:  positive = CCW.
            // Therefore we negate pitch and roll when applying to Bevy's euler.
            let mut yaw_offset: f32 = -16.0;
            let mut pitch: f32 = -18.0;  // Panda3D convention (will be negated at apply)
            let mut roll: f32 = 0.0;     // Panda3D convention (will be negated at apply)
            let mut pivot_pos = weapon.anchor_pos;
            let mut emit_echo = false;

            match weapon.state {
                WeaponState::Idle => {
                    weapon.prev_tip_pos = None;
                }
                WeaponState::Swing => {
                    weapon.timer += dt;
                    let total = 0.18;
                    let t = (weapon.timer / total).min(1.0);
                    yaw_offset = -96.0 + 192.0 * t;
                    // Python: pitch = -20.0 + 8.0 * math.sin(t * math.pi)
                    // math.sin returns [-1,1], used directly as degree offset
                    pitch = -20.0 + 8.0 * (t * std::f32::consts::PI).sin();
                    roll = 25.0 * (t * std::f32::consts::PI).sin();
                    emit_echo = true;

                    // Slash trails
                    if t >= 0.22 && t <= 0.82 {
                        weapon.slash_timer -= dt;
                        if weapon.slash_timer <= 0.0 {
                            let curr_tip = tf.translation + tf.forward() * 1.48 * SWORD_GEO_SCALE;
                            if let Some(prev) = weapon.prev_tip_pos {
                                let seg = curr_tip - prev;
                                let seg_len = seg.length();
                                if seg_len >= 0.06 {
                                    let mid = (curr_tip + prev) * 0.5;
                                    let trail_pitch = seg.y.atan2(seg.x.hypot(seg.z)).to_degrees();
                                    let trail_yaw = (-seg.x).atan2(seg.z).to_degrees();

                                    commands.spawn((
                                        MaterialMeshBundle {
                                            mesh: meshes.add(Cuboid::new(0.04, 0.001, 1.8)),
                                            material: materials.add(StandardMaterial {
                                                base_color: Color::srgba(1.0, 1.0, 1.0, 0.72),
                                                unlit: true,
                                                alpha_mode: AlphaMode::Blend,
                                                ..default()
                                            }),
                                            transform: Transform::from_translation(mid)
                                                .with_rotation(Quat::from_euler(EulerRot::YXZ, trail_yaw.to_radians(), trail_pitch.to_radians(), 0.0)),
                                            ..default()
                                        },
                                        SlashTrail { life: 0.16, max_life: 0.16 },
                                        NoFrustumCulling,
                                        RenderLayers::layer(2),
                                    ));
                                }
                            }
                            weapon.prev_tip_pos = Some(curr_tip);
                            weapon.slash_timer = 1.0 / 85.0;
                        }
                    }

                    if t >= 1.0 {
                        weapon.state = WeaponState::Idle;
                        weapon.timer = 0.0;
                        weapon.hit_targets.clear();
                        weapon.prev_tip_pos = None;
                    }
                }
                WeaponState::Spin => {
                    weapon.timer += dt;
                    let total = 0.32;
                    let t = (weapon.timer / total).min(1.0);
                    yaw_offset = -180.0 + 540.0 * t;
                    pitch = -14.0;
                    // Python: roll = 18.0 * math.sin(t * math.tau)
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
                        let back_t = (t - (outbound / total)) / (1.0 - (outbound / total)).max(1e-6);
                        vel_sign = -1.0;
                        (1.0 - back_t) * (1.0 - back_t)
                    };

                    let arc = (t * std::f32::consts::PI).sin() * distance * 0.16;
                    let right_throw = input_up.cross(dir).normalize_or_zero();
                    let throw_pos = origin + dir * (distance * forward_amount) + right_throw * arc;
                    pivot_pos = throw_pos + input_up * (0.08 + 0.12 * (t * std::f32::consts::PI).sin());

                    let fw_heading = (-dir.x).atan2(-dir.z).to_degrees();
                    heading = if vel_sign >= 0.0 { fw_heading } else { fw_heading + 180.0 };
                    yaw_offset = 0.0;
                    // Python: pitch = -8.0 + 5.0 * math.sin(t * math.pi)
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
                    // Spawn Hyperbomb effect
                    let ball_r = 0.68; // BALL_RADIUS
                    commands.spawn((
                        PbrBundle {
                            mesh: meshes.add(Sphere::new(0.1)),
                            material: materials.add(StandardMaterial {
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
                            max_radius: ball_r * 8.2, // hyperbomb_max_scale_factor
                        },
                        RenderLayers::layer(2),
                    ));
                    weapon.state = WeaponState::Idle;
                    weapon.timer = 0.0;
                }
                WeaponState::MagicMissile => {
                    // Spawn Magic Missiles
                    let count = 6; // magic_missile_cast_count
                    let forward = weapon.weapon_forward;
                    let up = Vec3::Y;
                    let right = up.cross(forward).normalize_or_zero();
                    
                    for idx in 0..count {
                        let spread = idx as f32 - (count as f32 - 1.0) * 0.5;
                        let spawn_pos = player_tf.translation + right * (spread * 0.55) + up * (0.16 * spread.abs());
                        
                        commands.spawn((
                            PbrBundle {
                                mesh: meshes.add(Sphere::new(0.15)),
                                material: materials.add(StandardMaterial {
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
                            Spatial4D { w: player_sp.w, target_w: player_sp.w, layer: player_sp.layer, is_folded: false },
                            RenderLayers::layer(2),
                        ));
                    }
                    weapon.state = WeaponState::Idle;
                    weapon.timer = 0.0;
                }
            }

            tf.translation = pivot_pos;
            // Negate pitch and roll to convert from Panda3D convention to Bevy convention
            tf.rotation = Quat::from_euler(EulerRot::YXZ, (heading + yaw_offset).to_radians(), (-pitch).to_radians(), (-roll).to_radians());

            // Emit echo
            if emit_echo {
                weapon.echo_timer -= dt;
                if weapon.echo_timer <= 0.0 {
                    weapon.echo_timer = 1.0 / 120.0;
                    // Echoes are spawned in a separate system based on this state
                }
            }
        }
    }

    // 2. Prosecution & Damage
    let mut hits = Vec::new();
    {
        for (weapon_ent, weapon, tf) in weapon_set.p1().iter() {
            if weapon.state == WeaponState::Idle { continue; }
            let is_spin = weapon.state == WeaponState::Spin;
            let is_throw = weapon.state == WeaponState::Throw;
            
            let throw_rad = 0.95 + SWORD_SCALE * 0.22;
            let swing_fwd = weapon.weapon_forward; // Simplified swing fwd check

            for (enemy_ent, enemy_tf, enemy_sp, enemy_hp, monster) in enemy_query.iter() {
                if enemy_hp.current <= 0.0 { continue; }
                if weapon.hit_targets.contains(&enemy_ent) { continue; }

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
                        AiState::Guarding => 0.58 + if matches!(monster.variant, MonsterVariant::Juggernaut | MonsterVariant::Vanguard) { 0.18 } else { 0.0 },
                        AiState::Attacking => 0.20 + if matches!(monster.variant, MonsterVariant::Juggernaut | MonsterVariant::Vanguard) { 0.18 } else { 0.0 },
                        _ => 0.0,
                    };
                    if guard_chance > 0.0 && rand::thread_rng().gen::<f32>() < guard_chance {
                        sfx_events.send(crate::effects::audio::PlaySfxEvent {
                            kind: crate::effects::audio::SfxKind::MonsterHit,
                            volume: 0.5, pitch: 0.9, position: Some(enemy_tf.translation),
                        });
                        fx_events.send(crate::effects::FloatingTextEvent {
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
                    if away.length_squared() > 1e-6 { away = away.normalize(); }
                    let pop_up = if is_throw { 0.42 } else { 0.35 };
                    let knockback = away * knock_mag + Vec3::Y * pop_up;
                    hits.push((weapon_ent, enemy_ent, applied, knockback));
                }
            }
        }
    }

    // 3. Dispatch Damage Events
    for (weapon_ent, enemy_ent, damage, knockback) in hits {
        if let Ok((mut weapon, _, _)) = weapon_set.p0().get_mut(weapon_ent) {
            weapon.hit_targets.insert(enemy_ent);
        }
        
        // Ensure enemy tf is readable here -> it is.
        if let Ok((_, enemy_tf, _, _, _)) = enemy_query.get(enemy_ent) {
            sfx_events.send(crate::effects::audio::PlaySfxEvent {
                kind: crate::effects::audio::SfxKind::MonsterHit,
                volume: 0.8, pitch: 1.0, position: Some(enemy_tf.translation),
            });
        }
        
        damage_events.send(DamageEvent {
            target: enemy_ent,
            amount: damage,
            knockback,
        });
    }
}

pub(crate) fn apply_damage_events(
    mut events: EventReader<DamageEvent>,
    mut item_events: EventWriter<crate::world::items::SpawnItemEvent>,
    mut commands: Commands,
    mut query: Query<(&mut Health, &Transform, Option<&crate::ai::Monster>, Option<&mut Velocity>, Option<&mut crate::ai::KnockbackVel>)>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
    mut fx_events: EventWriter<crate::effects::FloatingTextEvent>,
    mut monster_stats: ResMut<crate::systems::progression::MonsterStats>,
    mut kill_protection: ResMut<crate::systems::progression::KillProtection>,
) {
    for ev in events.read() {
        if let Ok((mut health, tf, opt_monster, opt_vel, opt_knock)) = query.get_mut(ev.target) {
            health.current -= ev.amount;

            if let Some(mut knock) = opt_knock {
                knock.0 += ev.knockback;
            } else if let Some(mut vel) = opt_vel {
                vel.linvel += ev.knockback;
            }

            fx_events.send(crate::effects::FloatingTextEvent {
                pos: tf.translation + Vec3::new(0.0, 1.1, 0.0),
                text: format!("HP -{}", ev.amount as i32),
                color: Color::srgba(0.3, 0.7, 1.0, 1.0),
                scale: 0.24,
                life: 0.7,
            });

            if health.current <= 0.0 {
                kill_protection.stacks = (kill_protection.stacks + 1).min(kill_protection.max_stacks);
                let base_xp = 2.0 + health.max * 0.04;
                let variant_mult = opt_monster.map(|m| match m.variant {
                    crate::ai::MonsterVariant::Juggernaut | crate::ai::MonsterVariant::Vanguard => 1.45,
                    crate::ai::MonsterVariant::Raider => 1.25,
                    crate::ai::MonsterVariant::Giant => 1.75,
                    crate::ai::MonsterVariant::Normal => 1.0,
                }).unwrap_or(1.0);
                let xp_amount = base_xp * variant_mult;
                item_events.send(crate::world::items::SpawnItemEvent::Exp {
                    pos: tf.translation,
                    amount: xp_amount,
                });
                
                sfx_events.send(crate::effects::audio::PlaySfxEvent {
                    kind: crate::effects::audio::SfxKind::MonsterDie,
                    volume: 0.9, pitch: 1.0, position: Some(tf.translation),
                });
                
                monster_stats.slain += 1;
                
                commands.entity(ev.target).despawn_recursive();
            }
        }
    }
}
