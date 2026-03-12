mod components;
mod spawning;

pub use components::*;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{Rng, thread_rng};
use std::f32::consts::TAU;

use crate::components::{Spatial4D, Health};
use crate::player::Player;

pub struct AiPlugin;


impl Plugin for AiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            spawning::spawn_monsters_on_map_load,
            update_monster_ai,
            update_monster_overhead_ui,
            boss_hypercube_dash,
            update_monster_state_text,
            move_enemy_projectiles,
            update_lifetimes,
            apply_monster_w_velocity,
            monster_knockback_decay,
            monster_collision_damage,
        ).chain());
    }
}

fn update_monster_ai(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    player_query: Query<(&Transform, &Spatial4D, &Health), (With<Player>, Without<Monster>)>,
    mut monster_query: Query<(Entity, &mut Monster, &mut Transform, &mut Spatial4D, &mut crate::components::Velocity4D, &mut ExternalForce, &mut Velocity, &Health), (Without<Player>, With<Monster>)>,
    mut part_query: Query<(&mut Transform, &mut MonsterPart), (Without<Monster>, Without<Player>)>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
    rapier_context: Res<RapierContext>,
) {
    let dt = time.delta_seconds();
    let roll_time = time.elapsed_seconds();

    // Animate parts
    for (mut tf, mut part) in part_query.iter_mut() {
        let t = (roll_time * part.speed + part.phase).sin() * 0.5 + 0.5;
        let scale = part.min_scale + (part.max_scale - part.min_scale) * t;
        tf.scale = Vec3::splat(scale);
        
        // Find floor via raycast
        let mut floor_y = tf.translation.y - 100.0;
        let ray_origin = tf.translation + Vec3::Y * 0.1;
        if let Some((_, toi)) = rapier_context.cast_ray(
            ray_origin, -Vec3::Y, 150.0, true,
            QueryFilter::default().exclude_sensors().groups(CollisionGroups::new(Group::all(), Group::all().difference(Group::GROUP_32)))
        ) {
            floor_y = ray_origin.y - toi;
        }

        // Apply pseudo-physics falling
        if part.base_offset.y > floor_y {
            part.base_offset.y -= part.speed * 2.0 * dt;
            if part.base_offset.y < floor_y {
                part.base_offset.y = floor_y;
            }
        }
        
        tf.translation = part.base_offset + Vec3::Y * (t * 0.1); // slight breathing on floor
        tf.rotate_local_y(dt * part.speed);
    }

    let Ok((player_tf, player_sp, player_hp)) = player_query.get_single() else { return };
    if player_hp.current <= 0.0 { return; }

    for (_ent, mut monster, mut match_tf, mut mob_sp, _mob_vel4d, mut ext_force, _vel, mob_hp) in monster_query.iter_mut() {
        let hp_ratio = mob_hp.current / mob_hp.max;
        let to_player = player_tf.translation - match_tf.translation;
        let mut planar = to_player;
        planar.y = 0.0;
        let dist_3d_sq = planar.length_squared();

        let w_scale = 4.0; 
        let dw_scaled = (player_sp.w - mob_sp.w) * w_scale;
        let dist_4d_sq = dist_3d_sq + dw_scaled * dw_scaled;
        
        let guard_sq = monster.guard_range * monster.guard_range;
        let hunt_sq = monster.hunt_range * monster.hunt_range;
        let attack_sq = monster.attack_range * monster.attack_range;

        // Awaken logic
        if monster.is_docile && !monster.awakened {
            if hp_ratio < 1.0 {
                monster.awakened = true;
            } else {
                monster.state = AiState::Wandering;
            }
        }

        // Logic timers
        monster.teleport_cooldown = (monster.teleport_cooldown - dt).max(0.0);
        monster.fold_jump_cooldown = (monster.fold_jump_cooldown - dt).max(0.0);
        monster.ranged_cooldown = (monster.ranged_cooldown - dt).max(0.0);
        monster.cosmic_warp_cooldown = (monster.cosmic_warp_cooldown - dt).max(0.0);

        // State Machine
        if !monster.is_docile || monster.awakened {
            let mut rng = thread_rng();
            
            // 1. Teleport Logic (Parity Check)
            if monster.teleport_enabled && monster.teleport_cooldown <= 0.0 && monster.state != AiState::Wandering {
                if rng.gen_bool(0.12 * dt as f64) {
                    let angle = rng.gen_range(0.0..TAU);
                    let dist = rng.gen_range(2.0..6.5);
                    let offset = Vec3::new(angle.cos() * dist, 0.0, angle.sin() * dist);
                    match_tf.translation = player_tf.translation + offset;
                    monster.teleport_cooldown = rng.gen_range(3.0..7.0);
                    sfx_events.send(crate::effects::audio::PlaySfxEvent {
                        kind: crate::effects::audio::SfxKind::WeaponWarp,
                        volume: 0.8, pitch: 1.0, position: Some(match_tf.translation),
                    });
                }
            }

            // 2. Liminal Folding Logic (W-layer jumping)
            if monster.liminal_enabled && monster.fold_jump_cooldown <= 0.0 {
                if dw_scaled.abs() > 2.0 && rng.gen_bool(0.24 * dt as f64) {
                    mob_sp.target_w = player_sp.w + rng.gen_range(-1.2..1.2);
                    monster.fold_jump_cooldown = rng.gen_range(1.5..4.0);
                    sfx_events.send(crate::effects::audio::PlaySfxEvent {
                        kind: crate::effects::audio::SfxKind::WeaponWarp,
                        volume: 0.6, pitch: 1.2, position: Some(match_tf.translation),
                    });
                }
            }

            if dist_4d_sq > guard_sq {
                if monster.state == AiState::Attacking {
                    monster.state = AiState::Wandering;
                }
                monster.ai_state_timer -= dt;
                if monster.ai_state_timer <= 0.0 {
                    let mut rng = thread_rng();
                    let roll = rng.gen::<f32>();
                    if hp_ratio < 0.24 {
                        if roll < 0.6 { monster.state = AiState::Wandering; }
                        else if roll < 0.8 { monster.state = AiState::Guarding; }
                        else { monster.state = AiState::Running; }
                    } else {
                        if roll < 0.6 { monster.state = AiState::Wandering; }
                        else if roll < 0.85 { monster.state = AiState::Guarding; }
                        else { monster.state = AiState::Hunting; }
                    }
                    monster.ai_state_timer = rng.gen_range(2.5..6.0);
                }
            } else {
                if hp_ratio < 0.24 && dist_4d_sq < (monster.hunt_range * 1.6).powi(2) {
                    monster.state = AiState::Running;
                } else if dist_4d_sq <= attack_sq {
                    monster.state = AiState::Attacking;
                } else if dist_4d_sq <= hunt_sq {
                    monster.state = AiState::Hunting;
                } else if dist_4d_sq <= guard_sq {
                    monster.state = AiState::Guarding;
                } else {
                    monster.state = AiState::Wandering;
                }
            }

            // 3. Ranged Attack Logic
            if monster.ranged_enabled && monster.ranged_cooldown <= 0.0 && matches!(monster.state, AiState::Hunting | AiState::Attacking) {
                if dist_4d_sq > 4.0 && dist_4d_sq < 400.0 {
                    // Spawn projectile (Issue 11 parity)
                    commands.spawn((
                        PbrBundle {
                            mesh: meshes.add(Cuboid::new(0.4, 0.4, 0.4)).into(),
                            material: materials.add(StandardMaterial {
                                base_color: Color::srgba(1.0, 0.2, 0.2, 0.9),
                                emissive: LinearRgba::from(Color::srgba(1.0, 0.1, 0.1, 1.0)) * 5.0,
                                ..default()
                            }),
                            transform: Transform::from_translation(match_tf.translation + Vec3::Y * 0.5),
                            ..default()
                        },
                        crate::components::Velocity4D {
                            lin_v: (player_tf.translation - match_tf.translation).normalize() * 12.0,
                            w_v: 0.0,
                        },
                        crate::components::EnemyProjectile,
                        crate::components::LifeTime(4.0),
                        Spatial4D {
                            w: mob_sp.w,
                            target_w: mob_sp.w,
                            layer: mob_sp.layer,
                            is_folded: false,
                        },
                    ));
                    monster.ranged_cooldown = rng.gen_range(2.0..4.0);
                    sfx_events.send(crate::effects::audio::PlaySfxEvent {
                        kind: crate::effects::audio::SfxKind::WeaponSwing,
                        volume: 0.7, pitch: 0.85, position: Some(match_tf.translation),
                    });
                }
            }
        }

        // Movement application
        let _max_speed = 8.0 * monster.speed_boost;
        let accel = 15.0;
        
        match monster.state {
            AiState::Wandering => {
                // Minimal random movement, damp out
                ext_force.force = Vec3::ZERO;
            },
            AiState::Guarding => {
                // Move slowly towards player if far, back off if close
                let dir = to_player.normalize_or_zero();
                if dist_4d_sq > (monster.guard_range * 0.5).powi(2) {
                    ext_force.force = dir * accel * 0.4;
                } else {
                    ext_force.force = -dir * accel * 0.4;
                }
            },
            AiState::Hunting | AiState::Attacking => {
                // Move directly towards player
                let dir = to_player.normalize_or_zero();
                ext_force.force = dir * accel;
            },
            AiState::Running => {
                // Flee
                let dir = -to_player.normalize_or_zero();
                ext_force.force = dir * accel * 1.2;
            }
        }
    }
}

fn update_monster_overhead_ui(
    player_query: Query<&Transform, With<Player>>,
    monster_query: Query<(&Transform, &Health, &Children), With<Monster>>,
    mut hp_fill_query: Query<&mut Sprite, With<crate::components::MonsterHpBarFill>>,
    mut visibility_query: Query<&mut Visibility>,
) {
    let Ok(player_tf) = player_query.get_single() else { return };
    let p_pos = player_tf.translation;

    for (m_tf, health, children) in monster_query.iter() {
        let dist = p_pos.distance(m_tf.translation);
        let visible = if dist > 35.0 { Visibility::Hidden } else { Visibility::Inherited };
        let ratio = (health.current / health.max).clamp(0.0, 1.0);
        
        for &child in children.iter() {
            if let Ok(mut sprite) = hp_fill_query.get_mut(child) {
                sprite.custom_size = Some(Vec2::new(1.7 * ratio, 0.15));
            }
            if let Ok(mut vis) = visibility_query.get_mut(child) {
                *vis = visible;
            }
        }
    }
}

fn boss_hypercube_dash(
    time: Res<Time>,
    mut boss_query: Query<(&mut Boss, &mut Monster, &mut Spatial4D, &mut ExternalForce, &Transform)>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Ok(player_tf) = player_query.get_single() {
        for (mut boss, mut monster, mut spatial, mut force, boss_tf) in boss_query.iter_mut() {
            boss.dash_cooldown.tick(time.delta());

            if monster.state == AiState::Hunting || monster.state == AiState::Attacking {
                if boss.dash_cooldown.just_finished() {
                    // 1. Shift W-Layer erratically
                    let mut rng = thread_rng();
                    let random_w = rng.gen_range(-15.0..15.0);
                    spatial.target_w = random_w;
                    info!("Boss initiated Hypercube Dash! Shifting to W: {:.1}", random_w);

                    // 2. Launch massive impulse towards player across 3D space
                    let mut dir = (player_tf.translation - boss_tf.translation).normalize_or_zero();
                    dir.y = 0.0; // Keep horizontal
                    let dash_force = 1200.0 * monster.speed_boost;
                    force.force = dir * dash_force;
                    
                    // Temporarily increase attack range for the dash
                    monster.state = AiState::Attacking;
                } else {
                    // Standard hunting pursuit
                    let mut dir = (player_tf.translation - boss_tf.translation).normalize_or_zero();
                    dir.y = 0.0;
                    force.force = dir * 18.0 * monster.speed_boost;
                }
            } else if monster.state == AiState::Running {
                // Fleeing gracefully
                let mut dir = -(player_tf.translation - boss_tf.translation).normalize_or_zero();
                dir.y = 0.0;
                force.force = dir * 25.0 * monster.speed_boost;
                
                // Panic dash
                if boss.dash_cooldown.just_finished() {
                    spatial.target_w = if spatial.w < 0.0 { 15.0 } else { -15.0 };
                    force.force = dir * 1500.0 * monster.speed_boost;
                }
            } else {
                force.force = Vec3::ZERO;
            }
            
            // Fast hypercube lerping
            spatial.w = spatial.w.lerp(spatial.target_w, 20.0 * time.delta_seconds());
            spatial.layer = (spatial.w / 5.0).round() as i32;
        }
    }
}

// Issues 9 & 10: Dynamic Monster Text & State Announcement
fn update_monster_state_text(
    mut monster_query: Query<(&mut Monster, &Transform, &Children)>,
    mut text_query: Query<&mut Text, With<crate::components::MonsterStateText>>,
    mut fx: EventWriter<crate::effects::FloatingTextEvent>,
) {
    for (mut monster, transform, children) in monster_query.iter_mut() {
        if monster.last_announced_state != Some(monster.state) {
            let (text_str, color) = match monster.state {
                AiState::Wandering => ("WANDER", Color::srgba(0.65, 0.8, 1.0, 0.85)),
                AiState::Guarding => ("GUARD", Color::srgba(0.36, 1.0, 0.85, 0.9)),
                AiState::Hunting => ("HUNT", Color::srgba(1.0, 0.85, 0.3, 0.95)),
                AiState::Attacking => ("ATTACK", Color::srgba(1.0, 0.28, 0.28, 1.0)),
                AiState::Running => ("RUN", Color::srgba(1.0, 1.0, 0.35, 0.95)),
            };

            // Update overhead static text (Issue 9)
            for &child in children.iter() {
                if let Ok(mut text) = text_query.get_mut(child) {
                    text.sections[0].value = text_str.to_string();
                    text.sections[0].style.color = color;
                }
            }

            // Fire floating announcement event if it's a major tactical state (Issue 10)
            if monster.last_announced_state.is_some() {
                if matches!(monster.state, AiState::Guarding | AiState::Hunting | AiState::Attacking | AiState::Running) {
                    fx.send(crate::effects::FloatingTextEvent {
                        pos: transform.translation + Vec3::new(0.0, 1.3, 0.0),
                        text: text_str.to_string(),
                        color,
                        scale: 0.2, // matching Python scale=0.2
                        life: 0.8,
                    });
                }
            }

        monster.last_announced_state = Some(monster.state);
        }
    }
}

fn move_enemy_projectiles(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &crate::components::Velocity4D), With<crate::components::EnemyProjectile>>,
) {
    let dt = time.delta_seconds();
    for (mut tf, vel) in query.iter_mut() {
        tf.translation += vel.lin_v * dt;
    }
}

fn update_lifetimes(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut crate::components::LifeTime)>,
) {
    for (entity, mut lifetime) in query.iter_mut() {
        lifetime.0 -= time.delta_seconds();
        if lifetime.0 <= 0.0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}

fn apply_monster_w_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Spatial4D, &crate::components::Velocity4D), With<Monster>>,
) {
    let dt = time.delta_seconds();
    for (mut spatial, _vel) in query.iter_mut() {
        // Monster W movement is currently primarily lerping to target_w set by AI
        let lerp_speed = 4.5;
        spatial.w = spatial.w.lerp(spatial.target_w, (lerp_speed * dt).min(1.0));
        spatial.layer = (spatial.w / 5.0).round() as i32;
    }
}

fn monster_knockback_decay(
    time: Res<Time>,
    mut query: Query<(&mut Velocity, &mut KnockbackVel), With<Monster>>,
) {
    let dt = time.delta_seconds();
    let decay = (1.0 - dt * 7.5).max(0.0);
    for (mut vel, mut knock) in query.iter_mut() {
        vel.linvel += knock.0;
        knock.0 *= decay;
    }
}

fn monster_collision_damage(
    mut player_query: Query<(&Transform, &Spatial4D, &mut crate::player::PlayerStats), With<Player>>,
    mut jump: ResMut<crate::player::JumpState>,
    mut kill_protection: ResMut<crate::systems::progression::KillProtection>,
    monster_query: Query<(&Transform, &Spatial4D, &Monster)>,
    projectile_query: Query<(Entity, &Transform, &Spatial4D), With<crate::components::EnemyProjectile>>,
    mut fx_events: EventWriter<crate::effects::FloatingTextEvent>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
    mut commands: Commands,
) {
    let Ok((player_tf, player_sp, mut stats)) = player_query.get_single_mut() else { return };
    if jump.player_damage_cooldown > 0.0 { return; }

    if kill_protection.stacks > 0 {
        kill_protection.stacks -= 1;
        return;
    }

    let p_pos = player_tf.translation;

    // 1. Monster Contact Damage
    for (m_tf, m_sp, monster) in monster_query.iter() {
        if (m_sp.layer - player_sp.layer).abs() > 0 { continue; }
        
        let dist = p_pos.distance(m_tf.translation);
        if dist < 1.4 { // ball_radius(0.68) + monster_radius(0.7 approx)
            let dmg = 12.0 * monster.attack_mult;
            stats.hp -= dmg;
            jump.player_damage_cooldown = 0.45;

            fx_events.send(crate::effects::FloatingTextEvent {
                pos: p_pos + Vec3::Y * 1.5,
                text: format!("HP -{}", dmg as i32),
                color: Color::srgba(1.0, 0.2, 0.2, 1.0),
                scale: 0.3,
                life: 1.0,
            });
            
            sfx_events.send(crate::effects::audio::PlaySfxEvent {
                kind: crate::effects::audio::SfxKind::MonsterHit,
                volume: 1.0, pitch: 0.8, position: Some(p_pos),
            });
            return; // One hit per frame max
        }
    }

    // 2. Projectile Damage
    for (entity, proj_tf, proj_sp) in projectile_query.iter() {
         if (proj_sp.layer - player_sp.layer).abs() > 0 { continue; }
         let dist = p_pos.distance(proj_tf.translation);
         if dist < 1.1 {
            stats.hp -= 15.0;
            jump.player_damage_cooldown = 0.45;

            fx_events.send(crate::effects::FloatingTextEvent {
                pos: p_pos + Vec3::Y * 1.5,
                text: format!("HP -15"),
                color: Color::srgba(1.0, 0.2, 0.2, 1.0),
                scale: 0.3,
                life: 1.0,
            });
            
            sfx_events.send(crate::effects::audio::PlaySfxEvent {
                kind: crate::effects::audio::SfxKind::MonsterHit,
                volume: 1.0, pitch: 0.7, position: Some(p_pos),
            });
            
            commands.entity(entity).despawn_recursive();
            return;
         }
    }
}
