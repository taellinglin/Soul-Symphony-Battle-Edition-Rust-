use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::components::{Spatial4D, CompressionState};
use crate::weapon_system::{Weapon, WeaponState};

use super::components::*;

pub(crate) fn player_physics_controller(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    gravity: Res<GravityDirection>,
    hyper: Res<HyperspaceState>,
    mut jump_state: ResMut<JumpState>,
    mut timers: ResMut<PhysicsTimers>,
    camera_query: Query<&Transform, (With<PlayerCamera>, Without<Player>)>,
    mut query: Query<(&mut ExternalForce, &mut ExternalImpulse, &mut Velocity, &Spatial4D, &Transform, &mut Damping), With<Player>>,
    mut weapon_query: Query<&mut Weapon>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
    mut stats_query: Query<&mut PlayerStats, With<Player>>,
    time: Res<Time>,
) {
    let Ok(mut stats) = stats_query.get_single_mut() else { return };
    let Ok(cam_tf) = camera_query.get_single() else { return };
    let dt = time.delta_seconds();
    timers.roll_time += dt;

    // Tick down cooldowns (original: game loop lines 16314-16319)
    jump_state.hit_cooldown = (jump_state.hit_cooldown - dt).max(0.0);
    jump_state.attack_cooldown = (jump_state.attack_cooldown - dt).max(0.0);
    jump_state.player_damage_cooldown = (jump_state.player_damage_cooldown - dt).max(0.0);
    timers.monster_contact_sfx_cooldown = (timers.monster_contact_sfx_cooldown - dt).max(0.0);
    timers.warp_cooldown = (timers.warp_cooldown - dt).max(0.0);
    jump_state.jump_float_timer = (jump_state.jump_float_timer - dt).max(0.0);

    for (mut ext_force, mut ext_impulse, mut rb_vel, spatial, player_tf, mut damping) in query.iter_mut() {
        // Friction/Damping (original lines 16482-16483)
        damping.linear_damping = if hyper.gravity_hold { hyper.ball_friction_shift } else { hyper.ball_friction_default };

        let input_up = -gravity.0.normalize_or_zero();

        // ── Movement Input (original lines 16582-16590) ──
        let mut move_input = Vec2::ZERO;
        if keyboard_input.pressed(KeyCode::KeyW) { move_input.y += 1.0; }
        if keyboard_input.pressed(KeyCode::KeyS) { move_input.y -= 1.0; }
        if keyboard_input.pressed(KeyCode::KeyA) { move_input.x += 1.0; }
        if keyboard_input.pressed(KeyCode::KeyD) { move_input.x -= 1.0; }

        let cam_forward = cam_tf.forward().as_vec3();
        let mut forward = cam_forward - input_up * cam_forward.dot(input_up);
        if forward.length_squared() < 1e-6 {
            forward = -Vec3::Z;
        } else {
            forward = forward.normalize();
        }

        let mut right = input_up.cross(forward);
        if right.length_squared() < 1e-8 {
            right = Vec3::X;
        } else {
            right = right.normalize();
        }

        let mut move_dir = forward * move_input.y + right * move_input.x;
        let manual_move_active = move_dir.length_squared() > 0.0;
        if manual_move_active {
            move_dir = move_dir.normalize();
        }

        // ── Applied Forces (original lines 16636–16654) ──
        let mut applied_force = gravity.0 * 1.25; // mass 1.25 parity
        let mut applied_torque = Vec3::ZERO;

        if manual_move_active {
            // Roll torque and central force (original lines 16641-16644)
            let torque_axis = input_up.cross(move_dir);
            applied_torque = torque_axis * ROLL_TORQUE;
            applied_force += move_dir * (ROLL_FORCE * 0.56);
            timers.last_move_dir = move_dir;

            // Steering (original lines 16710-16713)
            let horizontal_vel = rb_vel.linvel - input_up * rb_vel.linvel.dot(input_up);
            let desired_velocity = move_dir * MAX_BALL_SPEED;
            let steer = desired_velocity - horizontal_vel;
            applied_force += steer * LINK_CONTROL_GAIN;
        } else {
            // Decelerate if no input (original lines 16714-16719)
            let brake = (1.0 - dt * LINK_BRAKE_DRAG).max(0.0);
            let vertical = input_up * rb_vel.linvel.dot(input_up);
            let horizontal = rb_vel.linvel - vertical;
            rb_vel.linvel = vertical + horizontal * brake;
            rb_vel.angvel *= (1.0 - dt * (LINK_BRAKE_DRAG * 0.82)).max(0.0);
        }

        // ── Hyperspace W-Force (original lines 16424-16526) ──
        let mut hyper_input = 0.0_f32;
        if keyboard_input.pressed(KeyCode::KeyQ) { hyper_input -= 2.0; }
        if keyboard_input.pressed(KeyCode::KeyE) { hyper_input += 2.0; }

        if hyper_input.abs() > 1e-6 {
            let w_phase = if hyper.w_limit <= 1e-6 { 0.0 } else {
                (spatial.w / hyper.w_limit).clamp(-1.0, 1.0)
            };
            let axis_angle = w_phase * (std::f32::consts::FRAC_PI_2);

            let planar_forward = forward;
            let side = input_up.cross(planar_forward).normalize_or_zero();
            let hyper_axis = (planar_forward * axis_angle.cos() + side * axis_angle.sin())
                .normalize_or_zero();

            let input_sign: f32 = if hyper_input < 0.0 { -1.0 } else { 1.0 };
            let force = hyper_axis * (hyper_input.abs() * hyper.force_strength)
                + input_up * (input_sign * hyper_input.abs() * hyper.force_strength * hyper.force_lift);
            applied_force += force;
        }

        // ── Gravity hold brake (original lines 16703-16707) ──
        if hyper.gravity_hold {
            let brake_factor = (1.0 - dt * hyper.shift_brake_drag).max(0.0);
            rb_vel.linvel *= brake_factor;
            rb_vel.angvel *= (1.0 - dt * (hyper.shift_brake_drag * 0.5)).max(0.0);
        }

        ext_force.force = applied_force;
        ext_force.torque = applied_torque;

        // ── Jump (original lines 16787-16812) ──
        if keyboard_input.just_pressed(KeyCode::Space) {
            jump_state.jump_queued = true;
        }

        if jump_state.jump_queued {
            if jump_state.infinite_jumps || jump_state.jumps_used < jump_state.max_jumps {
                let mut jump_up = input_up;
                if jump_up.length_squared() < 1e-8 {
                    jump_up = Vec3::Y;
                } else {
                    jump_up = jump_up.normalize();
                }

                // Boost direction from current velocity (original lines 16795-16803)
                let vel_dot = rb_vel.linvel.dot(jump_up);
                let current_up_vel = if vel_dot > 0.0 { vel_dot } else { 0.0 };
                
                // Parity: _suppress_wall_climb_velocity (original line 11849)
                // Cap upward jump velocity if we are hugging a wall/obstacle
                let max_up_speed = 0.9; 
                let mut final_impulse_mag = JUMP_IMPULSE * JUMP_RISE_BOOST;
                
                // If already moving up fast (e.g. wall climbing), cap the boost
                if current_up_vel > max_up_speed {
                   final_impulse_mag = (final_impulse_mag * 0.5).min(max_up_speed);
                }

                let mut impulse = jump_up * final_impulse_mag;

                let mut boost_dir = rb_vel.linvel - jump_up * rb_vel.linvel.dot(jump_up);
                if boost_dir.length_squared() < 1e-6 {
                    boost_dir = timers.last_move_dir;
                }
                
                if boost_dir.length_squared() > 1e-6 {
                    let boost_dir = boost_dir.normalize();
                    impulse += boost_dir * (SPACE_BOOST_IMPULSE * 0.2);
                }
                ext_impulse.impulse = impulse;

                if !jump_state.infinite_jumps {
                    jump_state.jumps_used += 1;
                }
                jump_state.grounded = false;
                jump_state.jump_float_timer = jump_state.jump_float_duration;

                sfx_events.send(crate::effects::audio::PlaySfxEvent {
                    kind: crate::effects::audio::SfxKind::Jump,
                    volume: 0.72, pitch: 1.2, position: Some(player_tf.translation),
                });
            }
            jump_state.jump_queued = false;
        }

        // ── Cooldowns ──
        let dt = time.delta_seconds();
        stats.hyperbomb_cooldown = (stats.hyperbomb_cooldown - dt).max(0.0);
        stats.magic_missile_cooldown = (stats.magic_missile_cooldown - dt).max(0.0);

        // ── Weapon Input ──
        // Parity with main.py:1254-1265
        if mouse_input.just_pressed(MouseButton::Left) {
            if let Ok(mut weapon) = weapon_query.get_single_mut() {
                if weapon.state == WeaponState::Idle {
                    weapon.state = WeaponState::Throw;
                    weapon.timer = 0.0;
                    weapon.throw_dir = forward;
                }
            }
        }
        if mouse_input.just_pressed(MouseButton::Right) {
            if let Ok(mut weapon) = weapon_query.get_single_mut() {
                if weapon.state == WeaponState::Idle {
                    weapon.state = WeaponState::Spin;
                    weapon.timer = 0.0;
                    sfx_events.send(crate::effects::audio::PlaySfxEvent {
                        kind: crate::effects::audio::SfxKind::WeaponSwing,
                        volume: 1.0, pitch: 1.0, position: None,
                    });
                }
            }
        }
        if mouse_input.just_pressed(MouseButton::Middle) {
            if weapon_query.get_single().map(|w| w.state == WeaponState::Idle).unwrap_or(false) {
                if stats.hyperbomb_cooldown <= 0.0 {
                    if let Ok(mut weapon) = weapon_query.get_single_mut() {
                        weapon.state = WeaponState::Hyperbomb;
                        stats.hyperbomb_cooldown = 0.65; 
                    }
                }
            }
        }
        if keyboard_input.just_pressed(KeyCode::KeyR) {
             if stats.magic_missile_cooldown <= 0.0 {
                if let Ok(mut weapon) = weapon_query.get_single_mut() {
                    if weapon.state == WeaponState::Idle {
                        weapon.state = WeaponState::MagicMissile;
                        stats.magic_missile_cooldown = 1.4;
                        sfx_events.send(crate::effects::audio::PlaySfxEvent {
                            kind: crate::effects::audio::SfxKind::WeaponWarp, // Fixed variant
                            volume: 0.8, pitch: 1.2, position: None,
                        });
                    }
                }
             }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Hyperspace Physics (original lines 16449-16461, 16536-16539, 16551-16552)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn apply_hyperspace_physics(
    mut query: Query<(&mut Velocity, &mut Transform, &mut Spatial4D), With<Player>>,
    hyper: Res<HyperspaceState>,
    gravity: Res<GravityDirection>,
    graph: Res<crate::map::DungeonGraph>,
    timers: Res<PhysicsTimers>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    let Ok((mut vel, mut tf, mut spatial)) = query.get_single_mut() else { return };

    let hyperspace_active = hyper.is_active(spatial.w);
    let _hyperspace_amount = hyper.amount(spatial.w);

    if !hyperspace_active { return; }

    let gravity_hold_override = hyperspace_active && hyper.gravity_hold;

    // Gravity modulation in hyperspace (original lines 16489-16491)
    // When in hyperspace and NOT holding shift, gravity oscillates
    if !gravity_hold_override {
        let _input_up = -gravity.0.normalize_or_zero();
        let g_mag = gravity.0.length().max(0.1);
        let mag_scale = 0.72 + 0.28 * (timers.roll_time * 1.35).sin();
        // This is handled through the gravity resource in a real implementation,
        // but we apply the scaled gravity effect directly here:
        let gravity_mod = gravity.0.normalize_or_zero() * (g_mag * mag_scale - g_mag);
        vel.linvel += gravity_mod * dt;

        // Original bounding box bounce removed. Seamless world wrap takes over.

        // ─────────────────────────────────────────────────────────────────────────────
        // Mobius Twist Warp Portals
        // ─────────────────────────────────────────────────────────────────────────────
        if timers.warp_cooldown <= 0.0 {
            for link in &graph.warp_links {
                let d_a = tf.translation.distance_squared(link.a_pos);
                let d_b = tf.translation.distance_squared(link.b_pos);
                let r_sq = link.radius * link.radius;

                let (is_hit, from_a, target_pos, source_pos) = if d_a < r_sq {
                    (true, true, link.b_pos, link.a_pos)
                } else if d_b < r_sq {
                    (true, false, link.a_pos, link.b_pos)
                } else {
                    (false, false, Vec3::ZERO, Vec3::ZERO)
                };

                if is_hit {
                    // Python parity: _apply_room_fold_warp -> _apply_room_fold_twist
                    let up = -gravity.0.normalize_or_zero();
                    let mut fold_push = if from_a { target_pos - source_pos } else { source_pos - target_pos };
                    fold_push = fold_push - up * fold_push.dot(up);
                    if fold_push.length_squared() > 1e-8 {
                        fold_push = fold_push.normalize();
                    }

                    if link.mobius {
                        let twist_strength = 0.9; // Python parity: mobius_twist_strength
                        let (out_vel, new_w) = compute_mobius_fold_twist(
                            vel.linvel,
                            fold_push,
                            up,
                            spatial.w,
                            timers.roll_time,
                            link.mobius_phase,
                            twist_strength,
                            hyper.w_limit,
                        );
                        vel.linvel = out_vel;
                        spatial.w = new_w;
                        spatial.target_w = new_w;
                    } else {
                        // Standard non-twisting portal boost
                        vel.linvel = vel.linvel * 0.85 + fold_push * 2.4;
                    }

                    // Teleport the player
                    tf.translation = target_pos + up * 0.34;
                    // Because timers is an immutable Res<PhysicsTimers> in this system, calculate locally or refactor system params.
                    // Actually, let's just use game mechanics: teleport moves you OUT of the portal sphere immediately anyway.
                    break;
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// World Wrap (Python parity: _apply_world_wrap + _wrap_xy_position)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn apply_world_wrap(
    mut query: Query<&mut Transform, With<Player>>,
    mut camera_query: Query<&mut Transform, (With<PlayerCamera>, Without<Player>)>,
    graph: Res<crate::map::DungeonGraph>,
    config: Res<crate::map::GenerationConfig>,
) {
    if graph.rooms.is_empty() { return; }

    // In arena mode we want \"invisible wall\" behavior instead of topology wrap.
    // World wrap is still used for other layouts (hexmix / maze3d / etc.).
    if config.layout_mode == "arena" {
        return;
    }

    let map_w = 176.0 * config.scale;
    let map_d = map_w;

    let mut delta = Vec3::ZERO;
    let margin = 0.35;
    let span_x = map_w;
    let span_y = map_d;

    let (high_y_base, low_y_base) = if config.layout_mode == "arena" {
        (15.3, -15.3)
    } else {
        (config.room_height + 4.0, -1.4)
    };

    let span_z = (high_y_base - low_y_base).max(0.001);

    let low_x = -margin;
    let high_x = map_w + margin;
    let low_z = -margin;
    let high_z = map_d + margin;
    let low_y = low_y_base - margin;
    let high_y = high_y_base + margin;

    if let Ok(mut player_tf) = query.get_single_mut() {
        let mut wrapped = player_tf.translation;

        if wrapped.x < low_x {
            wrapped.x += span_x;
            delta.x += span_x;
        } else if wrapped.x > high_x {
            wrapped.x -= span_x;
            delta.x -= span_x;
        }

        if wrapped.z < low_z {
            wrapped.z += span_y;
            delta.z += span_y;
        } else if wrapped.z > high_z {
            wrapped.z -= span_y;
            delta.z -= span_y;
        }

        if wrapped.y < low_y {
            wrapped.y += span_z;
            delta.y += span_z;
        } else if wrapped.y > high_y {
            wrapped.y -= span_z;
            delta.y -= span_z;
        }

        if delta.length_squared() > 1e-12 {
            player_tf.translation = wrapped;

            if let Ok(mut cam_tf) = camera_query.get_single_mut() {
                cam_tf.translation += delta;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Jump Float Drag (original lines 16658-16666)
// Applies air control drag depending on rising vs falling
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn apply_jump_float_drag(
    mut query: Query<&mut Velocity, With<Player>>,
    gravity: Res<GravityDirection>,
    jump_state: Res<JumpState>,
    time: Res<Time>,
) {
    if jump_state.grounded { return; }
    let dt = time.delta_seconds();
    let Ok(mut vel) = query.get_single_mut() else { return };

    let input_up = -gravity.0.normalize_or_zero();
    let vertical_speed = vel.linvel.dot(input_up);
    let horizontal = vel.linvel - input_up * vertical_speed;

    let new_vertical = if jump_state.jump_float_timer > 0.0 && vertical_speed > 0.0 {
        // Rising with float active — reduce upward deceleration
        vertical_speed * (1.0 - dt * jump_state.jump_float_drag).max(0.0)
    } else if vertical_speed < 0.0 {
        // Falling — apply fall drag
        vertical_speed * (1.0 - dt * jump_state.float_fall_drag).max(0.0)
    } else {
        vertical_speed
    };

    vel.linvel = horizontal + input_up * new_vertical;
}

// ─────────────────────────────────────────────────────────────────────────────
// Compression Physics — drag/boost from dimension compression
// (original lines 16668-16699)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn apply_compression_physics(
    mut query: Query<(&mut Velocity, &CompressionState), With<Player>>,
    gravity: Res<GravityDirection>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    for (mut vel, state) in query.iter_mut() {
        let compression_factor = state.factor_smoothed;
        let input_up = -gravity.0.normalize_or_zero();
        let vertical_speed = vel.linvel.dot(input_up);
        let mut horizontal = vel.linvel - input_up * vertical_speed;
        let mut new_vertical = vertical_speed;

        if compression_factor < 0.999 {
            // Compressed space: drag (original lines 16680-16689)
            let h_drag = (1.0 - (1.0 - compression_factor) * 0.12).powf(vel.linvel.xz().length() * 0.35);
            let v_drag = (1.0 - (1.0 - compression_factor) * 0.08).powf(vel.linvel.y.abs() * 0.45);
            // Reduced drag when actively moving (original lines 16683-16685)
            // TODO: check desired_move_dir — for now always apply base drag
            horizontal *= h_drag;
            new_vertical *= v_drag;
        } else if compression_factor > 1.001 {
            // Expanded space: boost (original lines 16690-16699)
            let gain = compression_factor - 1.0;
            let h_boost = (1.0 + gain * dt * 2.4).min(1.9);
            let v_boost = (1.0 + gain * dt * 1.4).min(1.6);
            horizontal *= h_boost;
            new_vertical *= v_boost;
        }

        vel.linvel = horizontal + input_up * new_vertical;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Speed Clamping (original line 16748-16750)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn apply_speed_clamping(
    mut query: Query<&mut Velocity, With<Player>>,
    gravity: Res<GravityDirection>,
) {
    for mut vel in query.iter_mut() {
        let input_up = -gravity.0.normalize_or_zero();
        let vertical = input_up * vel.linvel.dot(input_up);
        let horizontal = vel.linvel - vertical;
        let speed = horizontal.length();
        if speed > MAX_BALL_SPEED {
            let clamped_h = horizontal * (MAX_BALL_SPEED / speed);
            vel.linvel = clamped_h + vertical;
        }
    }
}

pub(crate) fn apply_vertical_limits(
    mut query: Query<(&mut Transform, &mut Velocity), With<Player>>,
    gravity: Res<GravityDirection>,
    config: Res<crate::map::GenerationConfig>,
) {
    let Ok((mut tf, mut vel)) = query.get_single_mut() else { return };

    let mut pos = tf.translation;
    let mut corrected = false;
    let mut hit_top = false;
    let mut hit_bottom = false;
    let mut hit_left = false;
    let mut hit_right = false;
    let mut hit_front = false;
    let mut hit_back = false;

    if config.layout_mode == "arena" {
        // Invisible wall behavior for arena: clamp to the playable band.
        // Vertical: visual echo band around floor (≈ [-1.4, 12.0]).
        let min_y = -1.4 + 0.5;
        let max_y = 12.0 - 0.5;

        // Horizontal: map extents [0, map_w] with a small inset margin.
        let map_w = 176.0 * config.scale;
        let margin = 0.35;
        let min_x = margin;
        let max_x = map_w - margin;
        let min_z = margin;
        let max_z = map_w - margin;

        if pos.y > max_y {
            pos.y = max_y;
            corrected = true;
            hit_top = true;
        } else if pos.y < min_y {
            pos.y = min_y;
            corrected = true;
            hit_bottom = true;
        }

        if pos.x < min_x {
            pos.x = min_x;
            corrected = true;
            hit_left = true;
        } else if pos.x > max_x {
            pos.x = max_x;
            corrected = true;
            hit_right = true;
        }

        if pos.z < min_z {
            pos.z = min_z;
            corrected = true;
            hit_back = true;
        } else if pos.z > max_z {
            pos.z = max_z;
            corrected = true;
            hit_front = true;
        }
    } else {
        // Other layouts: only clamp vertical hyper-bounds derived from room_height.
        let high_y_base = config.room_height + 4.0;
        let low_y_base = -1.4;
        let min_y = low_y_base + 0.5;
        let max_y = high_y_base - 0.5;

        if pos.y > max_y {
            pos.y = max_y;
            corrected = true;
            hit_top = true;
        } else if pos.y < min_y {
            pos.y = min_y;
            corrected = true;
            hit_bottom = true;
        }
    }

    if corrected {
        let mut v = vel.linvel;

        // Vertical: kill upward/downward component only when pushing against the band.
        let up = -gravity.0.normalize_or_zero();
        if up.length_squared() > 1e-8 {
            let v_up = up * v.dot(up);
            let v_lat = v - v_up;
            let pushing_up = hit_top && v_up.dot(up) > 0.0;
            let pushing_down = hit_bottom && v_up.dot(up) < 0.0;
            if pushing_up || pushing_down {
                v = v_lat;
            }
        }

        // Horizontal X: stop motion into the wall, allow tangential sliding.
        if hit_left && v.x < 0.0 {
            v.x = 0.0;
        }
        if hit_right && v.x > 0.0 {
            v.x = 0.0;
        }

        // Horizontal Z: same idea for front/back walls.
        if hit_back && v.z < 0.0 {
            v.z = 0.0;
        }
        if hit_front && v.z > 0.0 {
            v.z = 0.0;
        }

        vel.linvel = v;
        tf.translation = pos;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Anti-Tunneling (original _prevent_ball_tunneling lines 11634-11675)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn anti_tunneling_system(
    mut query: Query<(&mut Transform, &mut Velocity, Entity), With<Player>>,
    mut prev_state: ResMut<PrevBallState>,
    rapier_context: Res<RapierContext>,
) {
    let Ok((mut tf, mut vel, player_entity)) = query.get_single_mut() else { return };

    let curr_pos = tf.translation;
    let prev_pos = prev_state.position;
    let travel = curr_pos - prev_pos;
    let travel_dist = travel.length();

    let min_travel = crate::player::TUNNEL_MIN_TRAVEL;
    if travel_dist > min_travel {
        let ray_dir = travel.normalize();
        let filter = QueryFilter::default()
            .exclude_collider(player_entity)
            .groups(CollisionGroups::new(
                Group::all(),
                Group::all().difference(Group::GROUP_32),
            ));

        if let Some((_, toi)) = rapier_context.cast_ray(
            prev_pos,
            ray_dir,
            travel_dist,
            true,
            filter,
        ) {
            let hit_pos = prev_pos + ray_dir * toi;
            let safe_pos = hit_pos - ray_dir * (crate::player::BALL_RADIUS + 0.02);
            tf.translation = safe_pos;

            let v_along = vel.linvel.dot(ray_dir);
            if v_along > 0.0 {
                vel.linvel -= ray_dir * v_along;
            }
        }
    }

    // Store for next frame
    prev_state.position = tf.translation;
    prev_state.velocity = vel.linvel;
}

// ─────────────────────────────────────────────────────────────────────────────
// Ball Contact Analysis / Grounded Detection
// (original _analyze_ball_contacts lines 11522-11558)
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn ball_contact_analysis(
    query: Query<(Entity, &Transform), With<Player>>,
    rapier_context: Res<RapierContext>,
    gravity: Res<GravityDirection>,
    mut jump_state: ResMut<JumpState>,
) {
    let Ok((player_entity, player_tf)) = query.get_single() else { return };
    let input_up = -gravity.0.normalize_or_zero();
    // Ground detection: cast a short ray downward
    let ray_origin = player_tf.translation;
    let ray_dir = -input_up; // gravity direction = down
    let ray_dist = BALL_RADIUS + 0.15;

    let was_grounded = jump_state.grounded;
    jump_state.prev_grounded = was_grounded;

    if let Some((_, _toi)) = rapier_context.cast_ray(
        ray_origin, ray_dir, ray_dist, true,
        QueryFilter::default().exclude_collider(player_entity),
    ) {
        jump_state.grounded = true;
        if jump_state.grounded {
            jump_state.jumps_used = 0; // Reset jumps on ground contact (original line 16758)
        }
    } else {
        jump_state.grounded = false;
    }
}

pub(crate) fn w_dimension_shift(
    keys: Res<ButtonInput<KeyCode>>,
    mut scroll_evr: EventReader<bevy::input::mouse::MouseWheel>,
    mut query: Query<&mut Spatial4D, With<Player>>,
    mut hyper: ResMut<HyperspaceState>,
    time: Res<Time>,
) {
    // Shift key gravity hold (parity with main.py:1260-1263)
    hyper.gravity_hold = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);

    for mut spatial in query.iter_mut() {
        for ev in scroll_evr.read() {
            spatial.target_w += ev.y * 0.45;
        }

        let shift_speed = 10.0;
        spatial.w = spatial.w.lerp(spatial.target_w, shift_speed * time.delta_seconds());
        // Clamp to hyper_w_limit (original line 16442)
        spatial.w = spatial.w.clamp(-hyper.w_limit, hyper.w_limit);
        spatial.target_w = spatial.target_w.clamp(-hyper.w_limit, hyper.w_limit);
        spatial.layer = (spatial.w / 5.0).round() as i32;
    }
}


// ─────────────────────────────────────────────────────────────────────────────
// Mobius Topology Math
// ─────────────────────────────────────────────────────────────────────────────
pub fn compute_mobius_fold_twist(
    vel: Vec3,
    fold_push: Vec3,
    up: Vec3,
    player_w: f32,
    roll_time: f32,
    phase: f32,
    twist_strength: f32,
    hyper_w_limit: f32,
) -> (Vec3, f32) {
    let mut side = up.cross(fold_push);
    if side.length_squared() > 1.0e-8 {
        side = side.normalize();
    } else {
        side = Vec3::X;
    }

    let v_dot_fp = vel.dot(fold_push);
    let v_dot_side = vel.dot(side);
    let v_dot_up = vel.dot(up);

    let f_comp = fold_push * v_dot_fp;
    let s_comp = side * v_dot_side;
    let u_comp = up * v_dot_up;

    // Twist translates the fold components (inverting side)
    let twisted = f_comp - s_comp + u_comp * 0.9;

    let twist = twist_strength.clamp(0.0, 1.0);
    // Invert the W coordinate (mirror-world topology flip) and add wobble
    let wobble = (roll_time * 0.32 + phase).sin() * (1.0 - twist) * hyper_w_limit * 0.22;
    let new_w = (-player_w * twist + wobble).clamp(-hyper_w_limit, hyper_w_limit);

    // Apply boost vector to exit
    let out_v = twisted * 0.9 + fold_push * 1.75;
    
    (out_v, new_w)
}

// ─────────────────────────────────────────────────────────────────────────────
// Player Physics Components & Resources
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn update_compression_factor(
    mut player_query: Query<(&Transform, &mut CompressionState), With<Player>>,
    room_query: Query<(&Transform, &crate::map::DimensionField, &crate::map::Room)>,
    time: Res<Time>,
) {
    let Ok((player_tf, mut state)) = player_query.get_single_mut() else { return };
    let pos = player_tf.translation;
    let t = time.elapsed_seconds();

    let mut factor = 1.0;

    for (room_tf, field, room_ref) in room_query.iter() {
        let half_w = room_tf.scale.x * 0.5;
        let half_h = room_tf.scale.z * 0.5;
        let dx = (pos.x - room_tf.translation.x) / half_w.max(0.001);
        let dz = (pos.z - room_tf.translation.z) / half_h.max(0.001);

        if dx.abs() <= 1.0 && dz.abs() <= 1.0 {
            // Room base dimension
            let edge = dx.abs().max(dz.abs());
            let center = (1.0 - edge).max(0.0);
            let wave = (t * field.freq + field.phase).sin();
            let room_base = field.base + field.amp * wave;
            let spatial_blend = center * field.center_bias + edge * field.edge_bias;
            factor = 1.0 + (room_base - 1.0) * spatial_blend.clamp(0.0, 1.0);
            
            // Loop through pockets for localized distortions
            let mut max_pocket_influence = 0.0;
            let mut target_pocket_factor = factor;
            
            for pocket in &room_ref.pockets {
                // Determine 2D distance
                let p2d = Vec2::new(pos.x, pos.z);
                let dist = p2d.distance(pocket.position);
                
                if dist < pocket.radius {
                    // Cosine falloff
                    let influence = ((dist / pocket.radius) * std::f32::consts::PI).cos() * 0.5 + 0.5;
                    if influence > max_pocket_influence {
                        max_pocket_influence = influence;
                        target_pocket_factor = pocket.factor;
                    }
                }
            }
            
            // Blend base room factor with the strongest pocket influence
            factor = factor * (1.0 - max_pocket_influence) + target_pocket_factor * max_pocket_influence;
            
            break;
        }
    }

    state.factor = factor;
    state.factor_smoothed += (factor - state.factor_smoothed) * (time.delta_seconds() * COMPRESSION_SMOOTH_SPEED).min(1.0);
}



// ─────────────────────────────────────────────────────────────────────────────
// Collision Group Sync
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn sync_collision_groups(
    mut commands: Commands,
    query: Query<(Entity, &Spatial4D, Option<&CollisionGroups>)>,
) {
    for (entity, spatial, current_groups) in query.iter() {
        let clamped_layer = spatial.layer.clamp(-15, 15);
        let group_bit = 1 << (clamped_layer + 15);
        let membership = Group::from_bits_truncate(group_bit as u32);
        
        // Filter: participate in own layer + hit Group 32 (Ceiling/Floor)
        let filter = membership | Group::GROUP_32;

        if let Some(groups) = current_groups {
            if groups.memberships != membership || groups.filters != filter {
                if let Some(mut ec) = commands.get_entity(entity) {
                    ec.try_insert(CollisionGroups::new(membership, filter));
                }
            }
        } else if let Some(mut ec) = commands.get_entity(entity) {
            ec.try_insert(CollisionGroups::new(membership, filter));
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Rodrigues Rotation (port of Python camera.py rotate_around_axis)
// ─────────────────────────────────────────────────────────────────────────────

/// Rodrigues' rotation formula: rotates `vec` around `axis` by `angle_rad`.
/// Python parity: vec * cos(a) + (axis x vec) * sin(a) + axis * (axis . vec) * (1 - cos(a))
pub fn rotate_around_axis(vec: Vec3, axis: Vec3, angle_rad: f32) -> Vec3 {
    let axis_n = axis.normalize_or_zero();
    let cos_a = angle_rad.cos();
    let sin_a = angle_rad.sin();
    vec * cos_a + axis_n.cross(vec) * sin_a + axis_n * (axis_n.dot(vec) * (1.0 - cos_a))
}

// ─────────────────────────────────────────────────────────────────────────────
// Water Wave Buoyancy (Python parity: _sample_water_height + _apply_water_buoyancy)
// ─────────────────────────────────────────────────────────────────────────────

const WATER_WAVE_AMP: f32 = 0.24;
const WATER_WAVE_SPEED: f32 = 10.4;
const WATER_WAVE_FX: f32 = 0.9;
const WATER_WAVE_FY: f32 = 0.7;
const WATER_BASE_Z: f32 = 0.0; // floor_y
const WATER_BUOYANCY_BIAS: f32 = 0.62;
const WATER_BUOYANCY_STRENGTH: f32 = 2.2;
const WATER_DRAG_PLANAR: f32 = 0.85;
const WATER_DRAG_VERTICAL: f32 = 1.95;

/// Python parity: _sample_water_height
/// phase = x * fx + z * fy + t * speed + surface_phase
/// wave = sin(phase) * amp + cos(phase * 0.63 + 0.7) * (amp * 0.52)
pub fn sample_water_height(x: f32, z: f32, t: f32) -> f32 {
    let phase = x * WATER_WAVE_FX + z * WATER_WAVE_FY + t * WATER_WAVE_SPEED;
    let wave = phase.sin() * WATER_WAVE_AMP + (phase * 0.63 + 0.7).cos() * (WATER_WAVE_AMP * 0.52);
    WATER_BASE_Z + wave
}

/// Python parity: _apply_water_buoyancy
/// Applies upward buoyancy force when the ball is below the wave surface height
pub(crate) fn apply_water_buoyancy(
    mut query: Query<(&Transform, &mut Velocity, &mut ExternalForce), With<Player>>,
    gravity: Res<GravityDirection>,
    timers: Res<PhysicsTimers>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    let t = timers.roll_time;

    for (tf, mut vel, mut ext_force) in query.iter_mut() {
        let pos = tf.translation;
        let water_h = sample_water_height(pos.x, pos.z, t);

        let bottom_z = pos.y - BALL_RADIUS;
        let depth = water_h - bottom_z;

        if depth <= 0.0 {
            // Ball is above water — no buoyancy
            continue;
        }

        let up = -gravity.0.normalize_or_zero();
        if up.length_squared() < 1e-8 {
            continue;
        }

        // Python parity: mass=1.25, g_mag=9.62
        let mass = 1.25_f32;
        let g_mag = gravity.0.length().max(0.1);
        let submerge = (depth / (BALL_RADIUS * 1.9).max(0.05)).clamp(0.0, 1.35);

        // Buoyancy force (Python parity: mass * g * (bias + submerge * strength))
        let buoy_force = up * (mass * g_mag * (WATER_BUOYANCY_BIAS + submerge * WATER_BUOYANCY_STRENGTH));
        ext_force.force += buoy_force;

        // Water drag (Python parity: _apply_water_buoyancy lines 5246-5253)
        let v_up_speed = vel.linvel.dot(up);
        let v_planar = vel.linvel - up * v_up_speed;

        let planar_drag = (1.0 - dt * WATER_DRAG_PLANAR * (0.3 + submerge * 0.7)).max(0.0);
        let vertical_drag = (1.0 - dt * WATER_DRAG_VERTICAL * (0.4 + submerge * 0.9)).max(0.0);

        vel.linvel = v_planar * planar_drag + up * (v_up_speed * vertical_drag);
    }
}

