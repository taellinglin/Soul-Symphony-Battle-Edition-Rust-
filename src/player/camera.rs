use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::map::GenerationConfig;
use crate::components::{Spatial4D, CompressionState};

use super::components::*;
use super::physics::rotate_around_axis;

pub(crate) fn sync_camera(
    player_query: Query<(Entity, &Transform, &Spatial4D, &Velocity, &CompressionState), (With<Player>, Without<PlayerCamera>, Without<InvertedEchoCamera>, Without<FloatingTextCamera>, Without<ForegroundCamera>)>,
    mut camera_query: Query<(&mut Transform, &mut Projection), (With<PlayerCamera>, Without<InvertedEchoCamera>, Without<FloatingTextCamera>, Without<ForegroundCamera>)>,
    mut materials: ResMut<Assets<crate::rendering::HyperSliceMaterial>>,
    mut ball_materials: ResMut<Assets<crate::rendering::BallMaterial>>,
    mut water_materials: ResMut<Assets<crate::rendering::WaterSurfaceMaterial>>,
    mut orbit: ResMut<CameraOrbitState>,
    hyper: Res<HyperspaceState>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    rapier_context: Res<RapierContext>,
    dungeon_graph: Res<crate::map::DungeonGraph>,
    config: Res<GenerationConfig>,
    time: Res<Time>,
    obstacle_query: Query<Entity, With<crate::components::CameraObstacle>>,
) {
    let Ok((player_entity, player_tf, spatial, player_vel, comp_state)) = player_query.get_single() else { return };
    let dt = time.delta_seconds();
    let compression = comp_state.factor_smoothed;
    let ball_pos = player_tf.translation;

    // Standard gravity-up in arena mode (Python: base Z-up -> Bevy Y-up)
    let gravity_up = Vec3::Y;

    // ── ref_forward: Panda3D forward (0,1,0) maps to Bevy -Z ──
    // Python: ref_forward = Vec3(0, 1, 0), then projects out gravity component
    // In Bevy (Y-up), the equivalent reference forward is -Z
    let mut ref_forward = -Vec3::Z;
    if ref_forward.dot(gravity_up).abs() > 0.95 {
        ref_forward = Vec3::X;
    }
    ref_forward = ref_forward - gravity_up * ref_forward.dot(gravity_up);
    if ref_forward.length_squared() < 1e-8 {
        ref_forward = Vec3::X;
    }
    ref_forward = ref_forward.normalize();

    let mut raw_h = 0.0_f32;
    let mut _raw_p = 0.0_f32;
    for ev in mouse_motion.read() {
        // Use mouse X to control heading (horizontal orbit)
        raw_h -= ev.delta.x * 0.16;
        // Ignore mouse Y for normal chase camera (match original main.py orbit_dir usage)
        _raw_p += 0.0 * ev.delta.y;
    }

    let smooth = 0.62_f32; // original parity: mouse_look_smooth
    let keep = 1.0 - smooth;
    orbit.heading_input = orbit.heading_input * smooth + raw_h * keep;
    // Freeze pitch in normal chase mode; do not drive it from mouse
    orbit.pitch_input = 0.0;

    if orbit.heading_input.abs() > 1e-6 {
        orbit.heading += orbit.heading_input;
        orbit.manual_turn_hold = 0.22; // Keep auto-align disabled while moving
    } else {
        orbit.manual_turn_hold = (orbit.manual_turn_hold - dt).max(0.0);
    }

    // ── Auto-align heading to velocity (original lines 16942-16960) ──
    // Only auto-align when not manually turning (parity with Python line 16942)
    if orbit.manual_turn_hold <= 0.0 {
        let planar_vel = player_vel.linvel - gravity_up * player_vel.linvel.dot(gravity_up);
        let planar_speed = planar_vel.length();

        if planar_speed > CAMERA_AUTO_ALIGN_MIN_SPEED {
            let desired_planar_dir = planar_vel.normalize();
            // Python line 16956-16958: cross/dot approach against ref_forward
            let sin_term = gravity_up.dot(ref_forward.cross(desired_planar_dir));
            let cos_term = ref_forward.dot(desired_planar_dir).clamp(-1.0, 1.0);
            let desired_heading = sin_term.atan2(cos_term).to_degrees();
            let mut delta = desired_heading - orbit.heading;
            // Wrap to [-180, 180]
            delta = ((delta + 180.0) % 360.0 + 360.0) % 360.0 - 180.0;
            orbit.heading += delta * (dt * CAMERA_AUTO_ALIGN_SPEED).min(1.0);
        }
    }

    // ── Dynamic camera parameters (original lines 16970-16971) ──
    let follow_dist = CAMERA_FOLLOW_DISTANCE * (1.0 + (compression - 1.0) * 0.5);
    let height = CAMERA_HEIGHT_OFFSET * (1.0 + (compression - 1.0) * 0.2);
    let fov = CAMERA_FOV_BASE * (1.0 - (compression - 1.0) * 0.1);

    // ── Orbit direction (matches original main.py L16979–16986) ──
    let yaw_rad = orbit.heading.to_radians();
    let mut orbit_dir = rotate_around_axis(-ref_forward, gravity_up, yaw_rad);
    if orbit_dir.length_squared() < 1e-8 {
        orbit_dir = -Vec3::Z;
    } else {
        orbit_dir = orbit_dir.normalize();
    }
    orbit.smoothed_dir = orbit_dir;

    if let Ok((mut camera_tf, mut projection)) = camera_query.get_single_mut() {
        // Original: target at fixed height above ball, camera at fixed follow distance along orbit_dir
        let target = ball_pos + gravity_up * height;
        let desired_cam_pos = target + orbit_dir * follow_dist;

        // ── Smooth camera follow (original line 16976) ──
        let mut cam_pos = match orbit.smoothed_pos {
            Some(prev) => {
                // Python parity: snap camera if teleported (distance > room size threshold)
                if prev.distance(desired_cam_pos) > 20.0 {
                    desired_cam_pos
                } else {
                    let alpha = 1.0 - (-dt * 8.5_f32).exp();
                    prev + (desired_cam_pos - prev) * alpha
                }
            }
            None => desired_cam_pos,
        };

        // If this is the first frame (no smoothed pos), snap to desired
        if orbit.smoothed_pos.is_none() {
            cam_pos = desired_cam_pos;
        }

        // ── Camera collision ray (original _resolve_camera_tight lines 3536-3551) ──
        let ray_origin = ball_pos + Vec3::Y * 0.5;
        let ray_to_cam = cam_pos - ray_origin;
        let ray_dist = ray_to_cam.length();
        let mut resolved_cam = cam_pos;

        if ray_dist > 0.1 {
            let ray_dir = ray_to_cam / ray_dist;
            // Only hit Group 32 (Obstacles) to avoid snapping to water surface
            if let Some((_, toi)) = rapier_context.cast_ray(
                ray_origin, ray_dir, ray_dist, true,
                QueryFilter::default()
                    .exclude_collider(player_entity)
                    .exclude_sensors()
                    .predicate(&|e| obstacle_query.contains(e)),
            ) {
                // Snap camera closer on collision (original: _resolve_camera_tight)
                let min_dist = 0.5_f32; // camera_min_distance parity
                resolved_cam = ray_origin + ray_dir * (toi - 0.18).max(min_dist);
            }
        }

        // ── Reverse ray check: if camera ended up on wrong side of wall, pull it back ──
        {
            let rev_dir = (ray_origin - resolved_cam).normalize_or_zero();
            let rev_dist = (ray_origin - resolved_cam).length();
            if rev_dist > 0.1 {
                if let Some((_, _rev_toi)) = rapier_context.cast_ray(
                    resolved_cam, rev_dir, rev_dist, true,
                    QueryFilter::default()
                        .exclude_collider(player_entity)
                        .exclude_sensors()
                        .predicate(&|e| obstacle_query.contains(e)),
                ) {
                    // Camera is on the wrong side of a wall — snap it closer
                    let min_dist = 0.5_f32;
                    resolved_cam = ray_origin + (resolved_cam - ray_origin).normalize_or_zero() * min_dist;
                }
            }
        }

        // ── Enforce camera above ball (original _enforce_camera_above_ball lines 3624-3637) ──
        let min_up_offset = (CAMERA_HEIGHT_OFFSET * 0.45).max(0.5);
        let cam_up_dot = (resolved_cam - ball_pos).dot(Vec3::Y);
        if cam_up_dot < min_up_offset {
            resolved_cam.y = ball_pos.y + min_up_offset;
        }

        // ── Minimum planar distance (original lines 16990-17000) ──
        let mut to_cam_planar = resolved_cam - ball_pos;
        to_cam_planar.y = 0.0;
        let cam_dist_planar = to_cam_planar.length();
        if cam_dist_planar < CAMERA_BALL_CLEARANCE {
            if cam_dist_planar < 1e-6 {
                to_cam_planar = orbit.smoothed_dir;
            }
            let push = to_cam_planar.normalize() * CAMERA_BALL_CLEARANCE;
            resolved_cam.x = ball_pos.x + push.x;
            resolved_cam.z = ball_pos.z + push.z;
        }

        // ── Room-bounds clamping (Python: _clamp_camera_to_current_room_bounds) ──
        // Find the room containing the ball and clamp camera XZ within it
        if config.layout_mode != "arena" {
            let margin = 0.5_f32;
            let ball_xz = Vec2::new(ball_pos.x, ball_pos.z);
            let mut best_room: Option<&crate::map::Room> = None;
            let mut best_dist = f32::MAX;
            for room in &dungeon_graph.rooms {
                // Room coordinates: x, y are 2D map coords; in Bevy, x stays x, y becomes z
                let room_center = Vec2::new(room.x + room.w * 0.5, room.y + room.h * 0.5);
                let d = room_center.distance(ball_xz);
                if d < best_dist {
                    best_dist = d;
                    best_room = Some(room);
                }
            }
            if let Some(room) = best_room {
                let x_min = room.x + margin;
                let x_max = room.x + room.w - margin;
                let z_min = room.y + margin;   // room.y maps to Bevy Z
                let z_max = room.y + room.h - margin;

                // Only clamp if the ball is actually within this room (or we are in a non-Arena mode where bounds are strict)
                if ball_xz.x >= room.x - 0.1 && ball_xz.x <= room.x + room.w + 0.1
                   && ball_xz.y >= room.y - 0.1 && ball_xz.y <= room.y + room.h + 0.1 {
                    resolved_cam.x = resolved_cam.x.clamp(x_min, x_max);
                    resolved_cam.z = resolved_cam.z.clamp(z_min, z_max);
                }
            }
        }

        orbit.smoothed_pos = Some(resolved_cam);
        camera_tf.translation = resolved_cam;
        camera_tf.look_at(target, gravity_up);

        if let Projection::Perspective(ref mut persp) = *projection {
            persp.fov = fov.to_radians();
        }
    }

    // ── Sync HyperSlice material player_w ──
    let speed = player_vel.linvel.length();
    let hyperspace_active = hyper.is_active(spatial.w);
    let mut thickness = 1.0 + speed * 0.42;
    if hyperspace_active {
        thickness *= 2.0;
    }

    for (_, material) in materials.iter_mut() {
        material.extension.settings.player_w = spatial.w;
        material.extension.settings.thickness = thickness;
    }

    for (_, material) in ball_materials.iter_mut() {
        material.extension.settings.player_w = spatial.w;
        material.extension.settings.object_w = spatial.w; // Player is always at their own W
        material.extension.settings.thickness = thickness;
    }
    for (_, material) in water_materials.iter_mut() {
        material.extension.settings.player_w = spatial.w;
        material.extension.settings.fog_color = LinearRgba::BLACK;
        material.extension.settings.fog_start = 0.0;
        material.extension.settings.fog_end = 35.0;
    }
}

/// Syncs floating text, foreground, and inverted echo cameras to the main player camera.
pub(crate) fn sync_overlay_cameras(
    player_cam_query: Query<(&Transform, &Projection), With<PlayerCamera>>,
    mut floating_text_cam_query: Query<(&mut Transform, &mut Projection), (With<FloatingTextCamera>, Without<PlayerCamera>, Without<InvertedEchoCamera>, Without<ForegroundCamera>)>,
    mut foreground_cam_query: Query<(&mut Transform, &mut Projection), (With<ForegroundCamera>, Without<PlayerCamera>, Without<InvertedEchoCamera>, Without<FloatingTextCamera>)>,
    mut inverted_query: Query<(&mut Transform, &mut Projection), (With<InvertedEchoCamera>, Without<PlayerCamera>, Without<FloatingTextCamera>, Without<ForegroundCamera>)>,
) {
    let Ok((cam_tf, cam_proj)) = player_cam_query.get_single() else { return };

    let cam_pos = cam_tf.translation;
    let cam_rot = cam_tf.rotation;

    // Extract current FOV from the player camera (if perspective).
    let mut fov_rad = CAMERA_FOV_BASE.to_radians();
    if let Projection::Perspective(persp) = cam_proj {
        fov_rad = persp.fov;
    }

    // Sync Floating Text Camera
    if let Ok((mut ft_cam_tf, mut ft_proj)) = floating_text_cam_query.get_single_mut() {
        ft_cam_tf.translation = cam_pos;
        ft_cam_tf.rotation = cam_rot;
        if let Projection::Perspective(ref mut ft_persp) = *ft_proj {
            ft_persp.fov = fov_rad;
        }
    }

    // Sync Foreground Camera
    if let Ok((mut fg_cam_tf, mut fg_proj)) = foreground_cam_query.get_single_mut() {
        fg_cam_tf.translation = cam_pos;
        fg_cam_tf.rotation = cam_rot;
        if let Projection::Perspective(ref mut fg_persp) = *fg_proj {
            fg_persp.fov = fov_rad;
        }
    }

    // Inverted echo camera — mirror main camera across the echo plane.
    if let Ok((mut inv_camera_tf, mut inv_proj)) = inverted_query.get_single_mut() {
        let reflection_plane_y = 12.0;
        // Bevy 0.14 forward() returns a Dir3; convert to Vec3 explicitly.
        let forward: Vec3 = cam_tf.forward().into();
        let target = cam_pos + forward;

        let mirrored_pos = Vec3::new(cam_pos.x, 2.0 * reflection_plane_y - cam_pos.y, cam_pos.z);
        let mirrored_target = Vec3::new(target.x, 2.0 * reflection_plane_y - target.y, target.z);

        inv_camera_tf.translation = mirrored_pos;
        inv_camera_tf.look_at(mirrored_target, -Vec3::Y);

        if let Projection::Perspective(ref mut inv_persp) = *inv_proj {
            inv_persp.fov = fov_rad;
        }
    }
}
