use crate::components::{CompressionState, PlayerVisuals, Spatial4D, Velocity4D};
use crate::weapon_system::Weapon;
use bevy::prelude::*;
use bevy::render::view::{NoFrustumCulling, RenderLayers};
use bevy_rapier3d::prelude::*;

type PlayerTeleportQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static mut Spatial4D,
        &'static mut Velocity,
    ),
    (With<Player>, Without<Weapon>),
>;

use super::components::*;

pub(crate) fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
    _hyper_materials: ResMut<Assets<crate::rendering::HyperSliceMaterial>>,
    mut ball_materials: ResMut<Assets<crate::rendering::BallMaterial>>,
    save_data: Option<Res<crate::systems::progression::SaveData>>,
    asset_server: Res<AssetServer>,
) {
    let mut progression = crate::systems::progression::PlayerProgression::default();
    let mut stats = crate::systems::progression::PlayerCombatStats::default();
    let mut p_stats = PlayerStats {
        hp: 100.0,
        max_hp: 100.0,
        hyperbomb_cooldown: 0.0,
        magic_missile_cooldown: 0.0,
        magic_missile_count_bonus: 0,
    };

    let mut buffs = crate::systems::progression::PlayerSkillBuffs::default();
    if let Some(loaded) = save_data {
        progression.level = loaded.level;
        progression.xp = loaded.xp;
        progression.xp_next = loaded.xp_next;
        stats.atk = loaded.atk;
        stats.def = loaded.def;
        stats.dex = loaded.dex;
        stats.sta = loaded.sta;
        stats.int = loaded.int;
        stats.sword_dmg_mult = loaded.sword_dmg_mult;
        stats.dmg_taken_mult = loaded.dmg_taken_mult;
        stats.hp_max = loaded.hp_max;
        p_stats.max_hp = loaded.hp_max;
        p_stats.hp = loaded.hp_max;

        buffs.haste.set_duration(std::time::Duration::from_secs_f32(
            loaded.haste_rem.max(0.01),
        ));
        buffs
            .haste
            .set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.haste_rem <= 0.0 {
            buffs.haste.tick(std::time::Duration::from_secs_f32(0.01));
        }

        buffs.fury.set_duration(std::time::Duration::from_secs_f32(
            loaded.fury_rem.max(0.01),
        ));
        buffs
            .fury
            .set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.fury_rem <= 0.0 {
            buffs.fury.tick(std::time::Duration::from_secs_f32(0.01));
        }

        buffs
            .longblade
            .set_duration(std::time::Duration::from_secs_f32(
                loaded.longblade_rem.max(0.01),
            ));
        buffs
            .longblade
            .set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.longblade_rem <= 0.0 {
            buffs
                .longblade
                .tick(std::time::Duration::from_secs_f32(0.01));
        }

        buffs
            .critical
            .set_duration(std::time::Duration::from_secs_f32(
                loaded.critical_rem.max(0.01),
            ));
        buffs
            .critical
            .set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.critical_rem <= 0.0 {
            buffs
                .critical
                .tick(std::time::Duration::from_secs_f32(0.01));
        }
    }

    // Player ball — exact material values from Python ball_visuals.py
    let ball_mesh = meshes.add(Sphere::new(BALL_RADIUS));
    let ball_material = ball_materials.add(crate::rendering::BallMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 1.0),
            emissive: LinearRgba::new(0.26, 0.33, 0.46, 1.0),
            alpha_mode: AlphaMode::Opaque,
            unlit: false, // Changed to false to allow some PBR shading interaction
            ..default()
        },
        extension: crate::rendering::BallExtension {
            settings: crate::rendering::BallSettings::default(),
        },
    });

    commands
        .spawn((
            Player,
            p_stats,
            progression,
            stats,
            buffs,
            PlayerVisuals::default(),
            Spatial4D {
                w: 0.0,
                target_w: 0.0,
                layer: 0,
                is_folded: false,
            },
            Velocity4D {
                lin_v: Vec3::ZERO,
                w_v: 0.0,
            },
            CompressionState {
                factor: 1.0,
                factor_smoothed: 1.0,
            },
            MaterialMeshBundle {
                mesh: ball_mesh.clone(),
                material: ball_material,
                transform: Transform::from_xyz(0.0, 5.0, 0.0),
                ..default()
            },
            RenderLayers::layer(0),
        ))
        .insert((
            RigidBody::Dynamic,
            Collider::ball(BALL_RADIUS),
            ColliderMassProperties::Mass(1.25),
            Restitution::coefficient(0.04),
            Friction::coefficient(0.02),
            GravityScale(0.0),
            ExternalForce::default(),
            ExternalImpulse::default(),
            Velocity::default(),
            Damping {
                linear_damping: 0.28,
                angular_damping: 0.72,
            },
        ))
        .with_children(|parent| {
            // --- Inverted Hull Outline (Python Parity) ---
            parent.spawn((
                MaterialMeshBundle {
                    mesh: ball_mesh.clone(),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.01, 0.01, 0.015, 1.0),
                        unlit: true,
                        cull_mode: Some(bevy::render::render_resource::Face::Front),
                        ..default()
                    }),
                    transform: Transform::from_scale(Vec3::splat(1.13)), // scale matches Python
                    ..default()
                },
                RenderLayers::layer(0),
            ));

            // --- In-World UI ---
            parent
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "Player", // Will be replaced by local name/display name natively in network update
                            TextStyle {
                                font: asset_server.load("fonts/Mine.ttf"),
                                font_size: 50.0,
                                color: Color::srgba(0.92, 0.98, 1.0, 0.95),
                            },
                        ),
                        transform: Transform::from_translation(Vec3::new(0.0, 2.5, 0.0))
                            .with_scale(Vec3::splat(0.015)),
                        ..default()
                    },
                    crate::components::PlayerNameLabel,
                ))
                .with_children(|label_root| {
                    // HP Background
                    label_root.spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::srgba(0.05, 0.07, 0.1, 0.86),
                            custom_size: Some(Vec2::new(0.44, 0.028)),
                            ..default()
                        },
                        transform: Transform::from_xyz(0.0, -0.015, 0.0), // lower relative to label
                        ..default()
                    });
                    // HP Fill
                    label_root.spawn((
                        SpriteBundle {
                            sprite: Sprite {
                                color: Color::srgba(0.95, 0.22, 0.22, 0.96),
                                custom_size: Some(Vec2::new(0.432, 0.02)),
                                anchor: bevy::sprite::Anchor::CenterLeft,
                                ..default()
                            },
                            transform: Transform::from_xyz(-0.216, -0.015, 0.001),
                            ..default()
                        },
                        LocalPlayerHpBar,
                    ));

                    // XP Background
                    label_root.spawn(SpriteBundle {
                        sprite: Sprite {
                            color: Color::srgba(0.04, 0.06, 0.09, 0.82),
                            custom_size: Some(Vec2::new(0.44, 0.016)),
                            ..default()
                        },
                        transform: Transform::from_xyz(0.0, -0.055, 0.0), // even lower
                        ..default()
                    });
                    // XP Fill
                    label_root.spawn((
                        SpriteBundle {
                            sprite: Sprite {
                                color: Color::srgba(0.25, 0.92, 0.25, 0.92),
                                custom_size: Some(Vec2::new(0.432, 0.012)),
                                anchor: bevy::sprite::Anchor::CenterLeft,
                                ..default()
                            },
                            transform: Transform::from_xyz(-0.216, -0.055, 0.001),
                            ..default()
                        },
                        LocalPlayerXpBar,
                    ));
                });
        });

    // Weapon and other components follow...

    // Spawn Weapon (Sword)
    commands
        .spawn((
            Weapon { ..default() },
            Spatial4D::default(),
            SpatialBundle {
                transform: Transform::from_xyz(0.0, 5.0, 0.0),
                ..default()
            },
            NoFrustumCulling, // Fix root entity culling the children
        ))
        .with_children(|parent| {
            // ═══ Exact Python Parity: 7 sword parts ═══
            // sword_scale = max(1.0, ball_radius / 0.4) = 1.7
            // Panda3D→Bevy: X→X, Panda-Y(fwd)→Bevy -Z, Panda-Z(up)→Bevy Y
            // Panda3D box model: unit cube [-0.5..0.5], setScale = full extents
            // Bevy Cuboid::new = full extents (same as Python setScale)
            let s: f32 = 1.7; // sword_scale

            // 1. Guard  — Python: setPos(0, 0.2*s, 0), setScale(0.26*s, 0.05*s, 0.04*s), color(0.76, 0.84, 0.95, 1)
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.26 * s, 0.04 * s, 0.05 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.76, 0.84, 0.95, 1.0),
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(0.0, 0.0, -0.2 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));

            // 2. Grip   — Python: setPos(0, 0.03*s, 0), setScale(0.05*s, 0.14*s, 0.05*s), color(0.18, 0.22, 0.32, 1)
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.05 * s, 0.05 * s, 0.14 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.18, 0.22, 0.32, 1.0),
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(0.0, 0.0, -0.03 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));

            // 3. Blade  — Python: setPos(0, 0.74*s, 0), setScale(0.082*s, 0.74*s, 0.052*s), color(0.82, 0.94, 1.0, 1)
            //             emissive(0.45, 1.0, 1.0, 1.0)
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.082 * s, 0.052 * s, 0.74 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.82, 0.94, 1.0, 1.0),
                        emissive: LinearRgba::new(0.45, 1.0, 1.0, 1.0),
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(0.0, 0.0, -0.74 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));

            // 4. Tip    — Python: setPos(0, 1.37*s, 0), setScale(0.052*s, 0.12*s, 0.032*s), color(0.9, 0.98, 1.0, 1)
            //             emissive(0.45, 1.0, 1.0, 1.0)
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.052 * s, 0.032 * s, 0.12 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.9, 0.98, 1.0, 1.0),
                        emissive: LinearRgba::new(0.45, 1.0, 1.0, 1.0),
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(0.0, 0.0, -1.37 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));

            // 5. Glow   — Python: setPos(0, 0.84*s, 0), setScale(0.062*s, 0.78*s, 0.034*s), color(0.18, 0.95, 1.0, 0.94)
            //             transparent, depth_write=false, unlit
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.062 * s, 0.034 * s, 0.78 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.18, 0.95, 1.0, 0.94),
                        emissive: LinearRgba::new(0.45, 1.0, 1.0, 1.0),
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        depth_bias: 0.001,
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(0.0, 0.0, -0.84 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));

            // 6. Stripe L — Python: setPos(-0.078*s, 0.8*s, 0), setScale(0.01*s, 0.72*s, 0.039*s), color(0.6, 1.0, 1.0, 0.92)
            //               transparent, depth_write=false, unlit
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.01 * s, 0.039 * s, 0.72 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.6, 1.0, 1.0, 0.92),
                        emissive: LinearRgba::new(0.45, 1.0, 1.0, 1.0),
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        depth_bias: 0.002,
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(-0.078 * s, 0.0, -0.8 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));

            // 7. Stripe R — mirror of Stripe L on +X
            parent.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.01 * s, 0.039 * s, 0.72 * s)),
                    material: std_materials.add(StandardMaterial {
                        base_color: Color::srgba(0.6, 1.0, 1.0, 0.92),
                        emissive: LinearRgba::new(0.45, 1.0, 1.0, 1.0),
                        unlit: true,
                        alpha_mode: AlphaMode::Blend,
                        depth_bias: 0.002,
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                    transform: Transform::from_xyz(0.078 * s, 0.0, -0.8 * s),
                    ..default()
                },
                NoFrustumCulling,
                RenderLayers::layer(0),
            ));
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// Teleport on first frame
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn teleport_to_start(
    graph: Res<crate::map::DungeonGraph>,
    config: Res<crate::map::GenerationConfig>,
    mut player_query: PlayerTeleportQuery,
    mut weapon_query: Query<(&mut Transform, &mut Spatial4D, &mut Weapon), Without<Player>>,
    mut initialized: Local<bool>,
) {
    if *initialized {
        return;
    }

    // Original parity: main startup spawn uses `platform_course_spawn_pos`:
    // (map_w/2, map_d/2, floor_y + 3.0). See `original/main.py:603` and init placement `main.py:1196–1201`.
    let map_w = 176.0 * config.scale;
    let spawn_x = map_w * 0.5;
    let spawn_z = map_w * 0.5;
    let spawn_y = 3.0; // floor_y + 3.0 (Bevy Y is height)
    let w = 0.0;
    let layer = 0;

    if let Ok((mut tf, mut sp, mut vel)) = player_query.get_single_mut() {
        info!(
            "Teleporting player to original spawn: x={:.2}, z={:.2}, y={:.2}, w={:.1} (rooms={})",
            spawn_x,
            spawn_z,
            spawn_y,
            w,
            graph.rooms.len()
        );
        tf.translation.x = spawn_x;
        tf.translation.z = spawn_z;
        tf.translation.y = spawn_y;
        sp.w = w;
        sp.target_w = w;
        sp.layer = layer;
        vel.linvel = Vec3::ZERO;

        // Sync weapon if found
        if let Ok((mut w_tf, mut w_sp, mut weapon)) = weapon_query.get_single_mut() {
            w_tf.translation = tf.translation;
            w_sp.w = w;
            w_sp.target_w = w;
            w_sp.layer = layer;
            weapon.anchor_pos = tf.translation;
        }

        *initialized = true;
    } else {
        error!("Player query failed in teleport_to_start");
    }
}
