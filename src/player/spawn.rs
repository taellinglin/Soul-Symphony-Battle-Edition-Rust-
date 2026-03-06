use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::components::{Spatial4D, Velocity4D, TransformHistory, CompressionState, PlayerVisuals};
use crate::weapon_system::Weapon;

use super::components::*;

pub(crate) fn spawn_player(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut std_materials: ResMut<Assets<StandardMaterial>>,
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

        buffs.haste.set_duration(std::time::Duration::from_secs_f32(loaded.haste_rem.max(0.01)));
        buffs.haste.set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.haste_rem <= 0.0 { buffs.haste.tick(std::time::Duration::from_secs_f32(0.01)); }

        buffs.fury.set_duration(std::time::Duration::from_secs_f32(loaded.fury_rem.max(0.01)));
        buffs.fury.set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.fury_rem <= 0.0 { buffs.fury.tick(std::time::Duration::from_secs_f32(0.01)); }

        buffs.longblade.set_duration(std::time::Duration::from_secs_f32(loaded.longblade_rem.max(0.01)));
        buffs.longblade.set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.longblade_rem <= 0.0 { buffs.longblade.tick(std::time::Duration::from_secs_f32(0.01)); }

        buffs.critical.set_duration(std::time::Duration::from_secs_f32(loaded.critical_rem.max(0.01)));
        buffs.critical.set_elapsed(std::time::Duration::from_secs_f32(0.0));
        if loaded.critical_rem <= 0.0 { buffs.critical.tick(std::time::Duration::from_secs_f32(0.01)); }
    }

    // Player ball — exact material values from Python ball_visuals.py
    let ball_mesh = meshes.add(Sphere::new(BALL_RADIUS));
    let ball_material = ball_materials.add(crate::rendering::BallMaterial {
        base: StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, 1.0),
            emissive: LinearRgba::new(5.0, 0.0, 0.0, 1.0),
            alpha_mode: AlphaMode::Opaque,
            unlit: true,
            ..default()
        },
        extension: crate::rendering::BallExtension {
            settings: crate::rendering::BallSettings::default(),
        },
    });

    commands.spawn((
        Player,
        p_stats,
        progression,
        stats,
        buffs,
        PlayerVisuals::default(),
        TransformHistory { poses: std::collections::VecDeque::new(), max_len: 12 },
        Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
        Velocity4D { lin_v: Vec3::ZERO, w_v: 0.0 },
        CompressionState { factor: 1.0, factor_smoothed: 1.0 },
        MaterialMeshBundle {
            mesh: ball_mesh,
            material: ball_material,
            transform: Transform::from_xyz(0.0, 5.0, 0.0),
            ..default()
        },
    )).insert((
        RigidBody::Dynamic,
        Collider::ball(BALL_RADIUS),
        ColliderMassProperties::Mass(1.25),
        Restitution::coefficient(0.04),
        Friction::coefficient(0.02),
        GravityScale(0.0),
        ExternalForce::default(),
        ExternalImpulse::default(),
        Velocity::default(),
        Damping { linear_damping: 0.28, angular_damping: 0.72 },
    )).with_children(|parent| {
        // --- In-World UI ---
        parent.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Player", // Will be replaced by local name/display name natively in network update
                    TextStyle {
                        font: asset_server.load("fonts/Mine.ttf"),
                        font_size: 50.0,
                        color: Color::srgba(0.92, 0.98, 1.0, 0.95),
                        ..default()
                    }
                ),
                transform: Transform::from_translation(Vec3::new(0.0, 2.5, 0.0))
                    .with_scale(Vec3::splat(0.015)),
                ..default()
            },
            crate::components::PlayerNameLabel,
        )).with_children(|label_root| {
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

    // Spawn Weapon (Sword)
    commands.spawn((
        Weapon {
            ..default()
        },
        Spatial4D::default(),
        SpatialBundle {
            transform: Transform::from_xyz(0.0, 5.0, 0.0),
            ..default()
        }
    )).with_children(|parent| {
        parent.spawn(PbrBundle {
            mesh: meshes.add(Cuboid::new(0.16, 0.1, 1.48)),
            material: std_materials.add(StandardMaterial {
                base_color: Color::srgb(0.8, 0.9, 1.0),
                emissive: LinearRgba::new(0.2, 0.9, 1.0, 1.0),
                unlit: false,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 0.0, -0.74),
            ..default()
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Teleport on first frame
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn teleport_to_start(
    graph: Res<crate::map::DungeonGraph>,
    mut player_query: Query<(&mut Transform, &mut Spatial4D, &mut Velocity), (With<Player>, Without<Weapon>)>,
    mut weapon_query: Query<(&mut Transform, &mut Spatial4D, &mut Weapon), Without<Player>>,
    mut initialized: Local<bool>,
) {
    if *initialized { return; }

    if let Some(first_room) = graph.rooms.first() {
        let center = first_room.center();
        let w = (first_room.w_layer as f32) * 5.0;
        let layer = first_room.w_layer;

        if let Ok((mut tf, mut sp, mut vel)) = player_query.get_single_mut() {
            info!("Teleporting player to start: x={:.2}, z={:.2}, w={:.1}", center.x, center.y, w);
            tf.translation.x = center.x;
            tf.translation.z = center.y;
            tf.translation.y = 2.0; // Python parity: ball sits on floor at ball_radius + floor_y ≈ 0.68
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
    } else {
        error!("No rooms available in DungeonGraph yet for teleport_to_start");
    }
}
