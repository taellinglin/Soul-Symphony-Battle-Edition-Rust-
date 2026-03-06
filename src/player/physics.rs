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
        let mut applied_force = gravity.0 * 12.025; // mass 1.25 * 9.62
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
                let mut boost_dir = rb_vel.linvel - jump_up * rb_vel.linvel.dot(jump_up);
                if boost_dir.length_squared() < 1e-6 {
                    boost_dir = timers.last_move_dir;
                }
                if boost_dir.length_squared() > 1e-6 {
                    boost_dir = boost_dir.normalize();
                }

                let mut impulse = jump_up * (JUMP_IMPULSE * JUMP_RISE_BOOST);
                if boost_dir.length_squared() > 1e-6 {
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
                        stats.hyperbomb_cooldown = 1.75; 
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
    mut query: Query<(&mut Velocity, &mut Transform, &Spatial4D), With<Player>>,
    hyper: Res<HyperspaceState>,
    gravity: Res<GravityDirection>,
    graph: Res<crate::map::DungeonGraph>,
    timers: Res<PhysicsTimers>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    let Ok((mut vel, mut tf, spatial)) = query.get_single_mut() else { return };

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

        // Hyperspace bounce off world bounds (original lines 11445-11487)
        let r = BALL_RADIUS;
        // Use dungeon boundary from graph
        let (map_w, map_d) = if let (Some(w), Some(d)) = (
            graph.rooms.iter().map(|r| r.x + r.w).reduce(f32::max),
            graph.rooms.iter().map(|r| r.y + r.h).reduce(f32::max),
        ) { (w.max(500.0), d.max(500.0)) } else { (500.0, 500.0) };
        
        let min_x = 0.55 + r;
        let max_x = map_w - 0.55 - r;
        let min_z = 0.55 + r;
        let max_z = map_d - 0.55 - r;
        let min_y = r - 0.05; // Slightly submerged is okay for water feel
        let max_y = 50.0 - r - 0.12; // Adjusted for new ceiling height

        let pos = &mut tf.translation;
        let v = &mut vel.linvel;
        let bg = hyper.bounce_gain;

        if pos.x < min_x && v.x < 0.0 { pos.x = min_x; v.x = v.x.abs() * bg; }
        else if pos.x > max_x && v.x > 0.0 { pos.x = max_x; v.x = -(v.x.abs() * bg); }
        if pos.z < min_z && v.z < 0.0 { pos.z = min_z; v.z = v.z.abs() * bg; }
        else if pos.z > max_z && v.z > 0.0 { pos.z = max_z; v.z = -(v.z.abs() * bg); }
        if pos.y < min_y && v.y < 0.0 { pos.y = min_y; v.y = v.y.abs() * bg; }
        else if pos.y > max_y && v.y > 0.0 { pos.y = max_y; v.y = -(v.y.abs() * bg); }
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

    // Only check if we moved significantly (original line 11641)
    if travel_dist > TUNNEL_MIN_TRAVEL {
        let ray_dir = travel.normalize();
        // Cast ray from previous position toward current position
        if let Some((_, toi)) = rapier_context.cast_ray(
            prev_pos, ray_dir, travel_dist, true,
            QueryFilter::default().exclude_collider(player_entity),
        ) {
            // We tunneled through something — snap back (original lines 11652-16675)
            let hit_pos = prev_pos + ray_dir * toi;
            let safe_pos = hit_pos - ray_dir * (BALL_RADIUS + 0.06);
            tf.translation = safe_pos;

            // Kill velocity in travel direction (original lines 11668-11671)
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
            spatial.target_w += ev.y * 0.42; // Slower, more precise wheel shift
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
// Compression Factor Update
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn update_compression_factor(
    mut player_query: Query<(&Transform, &mut CompressionState), With<Player>>,
    room_query: Query<(&Transform, &crate::map::DimensionField)>,
    time: Res<Time>,
) {
    let Ok((player_tf, mut state)) = player_query.get_single_mut() else { return };
    let pos = player_tf.translation;
    let t = time.elapsed_seconds();

    let mut factor = 1.0;

    for (room_tf, field) in room_query.iter() {
        let half_w = room_tf.scale.x * 0.5;
        let half_h = room_tf.scale.z * 0.5;
        let dx = (pos.x - room_tf.translation.x) / half_w.max(0.001);
        let dz = (pos.z - room_tf.translation.z) / half_h.max(0.001);

        if dx.abs() <= 1.0 && dz.abs() <= 1.0 {
            let edge = dx.abs().max(dz.abs());
            let center = (1.0 - edge).max(0.0);
            let wave = (t * field.freq + field.phase).sin();
            let room_base = field.base + field.amp * wave;
            let spatial_blend = center * field.center_bias + edge * field.edge_bias;
            factor = 1.0 + (room_base - 1.0) * spatial_blend.clamp(0.0, 1.0);
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
        let desired_group = Group::from_bits_truncate(group_bit as u32);

        if let Some(groups) = current_groups {
            if groups.memberships != desired_group || groups.filters != desired_group {
                if let Some(mut ec) = commands.get_entity(entity) {
                    ec.try_insert(CollisionGroups::new(desired_group, desired_group));
                }
            }
        } else if let Some(mut ec) = commands.get_entity(entity) {
            ec.try_insert(CollisionGroups::new(desired_group, desired_group));
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
