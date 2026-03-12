use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::Rng;
use crate::components::Spatial4D;

pub struct AnomalyPlugin;

impl Plugin for AnomalyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_anomalies_deferred)
           .add_systems(Update, (anomaly_roam, anomaly_apply_forces, anomaly_update_visuals));
    }
}

#[derive(Component)]
#[allow(dead_code)]
pub struct Anomaly {
    pub kind: AnomalyKind,
    pub radius: f32,         
    pub pull_strength: f32,  
    pub phase: f32,          
    pub vel: Vec3,           
    pub w: f32,
    pub room_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnomalyKind {
    Suck,
    Blow,
}

#[derive(Component)]
pub struct AnomalyCore;

fn spawn_anomalies_deferred(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    graph: Res<crate::map::DungeonGraph>,
) {
    let mut rng = rand::thread_rng();
    let rooms = &graph.rooms;
    if rooms.is_empty() { return; }

    // Python parity: black_hole_count = 14 (perf) or 28 (default), blower_ratio = 0.46
    let count = 28;
    let blower_ratio: f32 = 0.46;
    let influence_radius = 15.5; // black_hole_influence_radius
    let pull_strength = 292.0;   // black_hole_pull_strength
    let visual_radius = 2.05;    // black_hole_visual_radius
    let roam_speed = 2.1;        // black_hole_roam_speed

    let core_mesh = meshes.add(Sphere::new(visual_radius * 0.52));
    let lens_mesh = meshes.add(Sphere::new(visual_radius * 1.18));
    let corona_mesh = meshes.add(Sphere::new(visual_radius * 1.62));

    for _idx in 0..count {
        let room = &rooms[rng.gen_range(0..rooms.len())];
        let _center = room.center();
        let margin = 1.1;
        let x = rng.gen_range((room.x as f32 + margin)..(room.x as f32 + room.w as f32 - margin));
        let z = rng.gen_range((room.y as f32 + margin)..(room.y as f32 + room.h as f32 - margin));
        let y = 1.35; // base_z + 1.35

        let kind = if rng.gen::<f32>() < blower_ratio { AnomalyKind::Blow } else { AnomalyKind::Suck };
        let phase = rng.gen::<f32>();
        let w = room.w_layer as f32 * 5.0;

        // Roaming velocity
        let dir = Vec3::new(rng.gen_range(-1.0..1.0), 0.0, rng.gen_range(-1.0..1.0)).normalize_or_zero();
        let vel = if dir.length_squared() < 1e-6 { Vec3::new(1.0, 0.0, 0.0) } else { dir } * roam_speed;

        // Core colors from Python
        let (core_color, lens_color, corona_color) = match kind {
            AnomalyKind::Blow => (
                Color::srgba(0.0, 0.0, 0.0, 0.97),
                Color::srgba(0.06, 0.08, 0.12, 0.26),
                Color::srgba(0.18, 0.22, 0.36, 0.16),
            ),
            AnomalyKind::Suck => (
                Color::srgba(0.92, 0.26, 1.0, 0.95),
                Color::srgba(0.72, 0.18, 0.92, 0.32),
                Color::srgba(0.52, 0.12, 0.76, 0.18),
            ),
        };

        let _anomaly_entity = commands.spawn((
            Anomaly {
                kind,
                radius: influence_radius,
                pull_strength,
                phase,
                vel,
                w,
                room_idx: rng.gen_range(0..rooms.len()),
            },
            Spatial4D { w, target_w: w, layer: room.w_layer, is_folded: false },
            TransformBundle::from(Transform::from_xyz(x, y, z)),
            VisibilityBundle::default(),
        )).with_children(|parent| {
            // Core sphere
            parent.spawn((
                AnomalyCore,
                PbrBundle {
                    mesh: core_mesh.clone(),
                    material: materials.add(StandardMaterial {
                        base_color: core_color,
                        emissive: LinearRgba::new(0.3, 0.1, 0.5, 1.0),
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        ..default()
                    }),
                    ..default()
                },
            ));
            // Lens sphere
            parent.spawn(PbrBundle {
                mesh: lens_mesh.clone(),
                material: materials.add(StandardMaterial {
                    base_color: lens_color,
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                }),
                ..default()
            });
            // Corona sphere
            parent.spawn(PbrBundle {
                mesh: corona_mesh.clone(),
                material: materials.add(StandardMaterial {
                    base_color: corona_color,
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                }),
                ..default()
            });
        }).id();
    }
}

/// Roaming movement for anomalies — they drift slowly within room bounds
fn anomaly_roam(
    mut query: Query<(&mut Transform, &mut Anomaly)>,
    time: Res<Time>,
    graph: Res<crate::map::DungeonGraph>,
) {
    let dt = time.delta_seconds();
    for (mut tf, mut anomaly) in query.iter_mut() {
        tf.translation += anomaly.vel * dt;
        
        // Wall bouncing (Python parity)
        if let Some(room) = graph.rooms.get(anomaly.room_idx) {
            let margin = 1.0;
            let x_min = room.x as f32 + margin;
            let x_max = (room.x + room.w) as f32 - margin;
            let z_min = room.y as f32 + margin;
            let z_max = (room.y + room.h) as f32 - margin;

            if tf.translation.x < x_min || tf.translation.x > x_max {
                anomaly.vel.x = -anomaly.vel.x;
            }
            if tf.translation.z < z_min || tf.translation.z > z_max {
                anomaly.vel.z = -anomaly.vel.z;
            }
            tf.translation.x = tf.translation.x.clamp(x_min, x_max);
            tf.translation.z = tf.translation.z.clamp(z_min, z_max);
        }

        // Slow oscillation to keep them moving interestingly
        let t = time.elapsed_seconds() + anomaly.phase * 100.0;
        tf.translation.y = 1.35 + 0.42 * (t * 1.8).sin(); // Python uses 1.8 speed, 0.42 amp
    }
}

/// Apply suck/blow forces to the player ball
fn anomaly_apply_forces(
    anomaly_query: Query<(&Transform, &Anomaly), Without<crate::ai::Monster>>,
    mut player_query: Query<(&Transform, &mut ExternalForce), (With<crate::player::Player>, Without<crate::ai::Monster>, Without<Anomaly>)>,
    mut monster_query: Query<(Entity, &mut Transform, &mut crate::ai::Monster, &mut Spatial4D, Option<&mut crate::ai::KnockbackVel>), (With<crate::ai::Monster>, Without<crate::player::Player>, Without<Anomaly>)>,
    graph: Res<crate::map::DungeonGraph>,
    hyper: Res<crate::player::HyperspaceState>,
) {
    let mut rng = rand::thread_rng();

    for (anomaly_tf, anomaly) in anomaly_query.iter() {
        let pos = anomaly_tf.translation;

        // 1. Force to Player
        if let Ok((player_tf, mut ext_force)) = player_query.get_single_mut() {
            let player_pos = player_tf.translation;
            let to_anomaly = pos - player_pos;
            let dist = to_anomaly.length().max(1e-5);

            if dist <= anomaly.radius {
                let direction = to_anomaly / dist;
                let proximity = 1.0 - (dist / anomaly.radius);

                let force_dir = match anomaly.kind {
                    AnomalyKind::Blow => -direction,
                    AnomalyKind::Suck => direction,
                };

                let proximity_mult = match anomaly.kind {
                    AnomalyKind::Blow => 1.95,
                    AnomalyKind::Suck => 1.7,
                };

                let mut force_mag = anomaly.pull_strength * (0.16 + proximity * proximity * proximity_mult);

                if anomaly.kind == AnomalyKind::Suck {
                    // Soften near center to allow escape
                    let soften = 0.38;
                    let soften_dist = (anomaly.radius * soften).max(0.3);
                    let center_scale = (dist / soften_dist).clamp(0.0, 1.0);
                    force_mag *= 0.22 + 0.78 * center_scale;
                    force_mag = force_mag.min(165.0); // suck_max_force
                }

                ext_force.force += force_dir * force_mag;
            }
        }

        // 2. Force and Warp to Monsters
        for (_ent, mut m_tf, mut monster, mut m_sp, opt_knock) in monster_query.iter_mut() {
            let m_pos = m_tf.translation;
            let to_anomaly_m = pos - m_pos;
            let m_dist = to_anomaly_m.length().max(1e-5);

            if m_dist > anomaly.radius { continue; }

            let m_dir = to_anomaly_m / m_dist;
            let proximity_m = 1.0 - (m_dist / anomaly.radius);
            let move_dir = match anomaly.kind {
                AnomalyKind::Blow => -m_dir,
                AnomalyKind::Suck => m_dir,
            };

            // Python parity: shove = pull_strength * (0.14 + proximity_m * proximity_m * 1.65) * monster_force_scale
            let shove = anomaly.pull_strength * (0.14 + proximity_m * proximity_m * 1.65) * 0.032;
            if let Some(mut knock) = opt_knock {
                knock.0 += move_dir * shove;
            } else {
                m_tf.translation += move_dir * shove * 0.1;
            }

            // Suck and Warp (Python parity: proximity >= 0.94)
            if anomaly.kind == AnomalyKind::Suck && proximity_m >= 0.94 && monster.cosmic_warp_cooldown <= 0.0 {
                let target_room_idx = rng.gen_range(0..graph.rooms.len());
                if let Some(room) = graph.rooms.get(target_room_idx) {
                    let out_x = room.x as f32 + rng.gen_range(1.0..room.w as f32 - 1.0);
                    let out_z = room.y as f32 + rng.gen_range(1.0..room.h as f32 - 1.0);
                    let out_y = 1.2 + rng.gen_range(0.32..0.82);
                    
                    m_tf.translation = Vec3::new(out_x, out_y, out_z);
                    m_sp.w = rng.gen_range(-(hyper.w_limit * 0.95)..(hyper.w_limit * 0.95));
                    m_sp.target_w = m_sp.w;
                    m_sp.layer = (m_sp.w / 5.0).round() as i32;
                    
                    monster.cosmic_warp_cooldown = 2.4; // black_hole_monster_warp_cooldown
                }
            }
        }
    }
}

/// Visual updates: pulsing, rotation, glow
fn anomaly_update_visuals(
    mut query: Query<(&mut Transform, &Anomaly)>,
    time: Res<Time>,
) {
    let t = time.elapsed_seconds();
    for (mut tf, anomaly) in query.iter_mut() {
        let _pulse = 0.5 + 0.5 * (t * 2.2 + anomaly.phase * std::f32::consts::TAU).sin();
        // Gentle rotation
        tf.rotate_y(time.delta_seconds() * 0.8);
    }
}
