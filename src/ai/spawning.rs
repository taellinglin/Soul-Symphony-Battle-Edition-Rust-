use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use rand::{Rng, thread_rng};
use std::f32::consts::TAU;

use crate::components::{Spatial4D, Health};
use crate::rendering::{HyperSliceMaterial, HyperSliceSettings, HyperSliceExtension};
use crate::map::DungeonGraph;

use super::components::*;

pub(crate) fn spawn_monsters_on_map_load(
    mut commands: Commands,
    graph: Res<DungeonGraph>,
    query: Query<(), With<MapMonstersSpawned>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HyperSliceMaterial>>,
    asset_server: Res<AssetServer>,
    mut monster_stats: ResMut<crate::systems::progression::MonsterStats>,
) {
    // Only spawn once when rooms exist
    if graph.rooms.is_empty() { return; }
    if !query.is_empty() { return; }

    // Mark as spawned
    commands.spawn(MapMonstersSpawned);

    let mut rng = thread_rng();
    let num_monsters = 24; // Default count from original
    let rooms = &graph.rooms;
    let room_count = rooms.len();
    if room_count == 0 { return; }

    let mut spawn_plan = Vec::new();
    let valid_rooms = &rooms[1..]; // Skip starting room (index 0)
    if valid_rooms.is_empty() {
        // Arena mode: spawn monsters around the arena center instead
        let center = rooms[0].center();
        let center_x = center.x;
        let center_z = center.y;
        for _ in 0..num_monsters {
            let px = center_x + rng.gen_range(-30.0..30.0_f32);
            let pz = center_z + rng.gen_range(-30.0..30.0_f32);
            spawn_plan.push(Vec3::new(px, 8.0, pz));
        }
    } else {
        for _ in 0..num_monsters {
            let room_idx = rng.gen_range(0..valid_rooms.len());
            let room = &valid_rooms[room_idx];
            
            let spread = (room.w.min(room.h) * 0.14).max(0.45);
            let cx = room.x + room.w * 0.5;
            let cy = room.y + room.h * 0.5;
            let px = cx + rng.gen_range(-spread..spread);
            let py = cy + rng.gen_range(-spread..spread);
            let pos = Vec3::new(px, 1.2, py); // Using y axis for up
            spawn_plan.push(pos);
        }
    }

    if spawn_plan.len() > num_monsters {
        use rand::seq::SliceRandom;
        spawn_plan.shuffle(&mut rng);
        spawn_plan.truncate(num_monsters);
    }
    
    monster_stats.total = spawn_plan.len();
    monster_stats.slain = 0;

    let monster_max_hp = 100.0;
    
    for (idx, pos) in spawn_plan.into_iter().enumerate() {
        let mut variant = MonsterVariant::Normal;
        let mut hp_mult: f32 = 1.0;
        let mut defense_mult: f32 = 1.0;
        let mut speed_mult: f32 = 1.0;
        let mut guard_mult: f32 = 1.0;
        let mut attack_mult: f32 = 1.0;

        let roll: f32 = rng.gen();
        if roll < 0.12 {
            variant = MonsterVariant::Juggernaut;
            hp_mult = 3.2; defense_mult = 2.8; speed_mult = 0.78; guard_mult = 1.55; attack_mult = 1.85;
        } else if roll < 0.24 {
            variant = MonsterVariant::Vanguard;
            hp_mult = 2.35; defense_mult = 2.2; speed_mult = 0.95; guard_mult = 2.4; attack_mult = 1.45;
        } else if roll < 0.34 {
            variant = MonsterVariant::Raider;
            hp_mult = 1.5; defense_mult = 1.0; speed_mult = 2.2; guard_mult = 1.3; attack_mult = 1.2;
        }

        // Giant override
        if rng.gen_bool(0.08) {
            variant = MonsterVariant::Giant;
            hp_mult = hp_mult.max(6.5 * 0.75);
            defense_mult = defense_mult.max(2.4 * 0.8);
            speed_mult = speed_mult.min(0.62);
            guard_mult = guard_mult.max(0.9);
            attack_mult = attack_mult.max(1.5);
        }

        let size_scale = rng.gen_range(0.72..1.35);
        let mut speed_scale = rng.gen_range(0.66..1.42);
        let hp_scale = rng.gen_range(0.76..1.78) * hp_mult;
        let _defense = rng.gen_range(0.75..1.45) * defense_mult;
        
        let mut _detect_range_mult = rng.gen_range(0.72..1.6);
        let range_roll: f32 = rng.gen();
        if range_roll < 0.24 { _detect_range_mult *= rng.gen_range(0.62..0.88); }
        else if range_roll > 0.76 { _detect_range_mult *= rng.gen_range(1.2..1.65); }

        let _crit = match variant {
            MonsterVariant::Raider => rng.gen_range(0.18..0.34),
            MonsterVariant::Juggernaut | MonsterVariant::Vanguard => rng.gen_range(0.06..0.18),
            MonsterVariant::Giant => rng.gen_range(0.02..0.08),
            MonsterVariant::Normal => rng.gen_range(0.04..0.2),
        };

        let fast_speed_boost = if rng.gen_bool(0.22) { rng.gen_range(2.0..3.4) } else { 1.0 };
        speed_scale *= speed_mult * fast_speed_boost;

        let hyper_w_limit = 15.0;
        let w = rng.gen_range(-hyper_w_limit * 0.85 .. hyper_w_limit * 0.85);
        let radius = rng.gen_range(0.85..1.35) * size_scale;
        
        let mut initial_state = AiState::Wandering;
        if matches!(variant, MonsterVariant::Juggernaut | MonsterVariant::Vanguard) {
            initial_state = AiState::Guarding;
        }

        let is_docile = matches!(variant, MonsterVariant::Giant);

        // Compute color
        let hue: f32 = rng.gen();
        let sat = rng.gen_range(0.55..0.9);
        let val = rng.gen_range(0.72..1.0);
        let color = bevy::color::Color::hsl(hue * 360.0, sat, val);
        
        let mut teleport_enabled = false;
        let mut liminal_enabled = false;
        let mut ranged_enabled = false;

        match variant {
            MonsterVariant::Raider => {
                teleport_enabled = rng.gen_bool(0.32);
                liminal_enabled = rng.gen_bool(0.22);
                ranged_enabled = true;
            }
            MonsterVariant::Vanguard => {
                teleport_enabled = rng.gen_bool(0.28);
                liminal_enabled = rng.gen_bool(0.28);
                ranged_enabled = true;
            }
            MonsterVariant::Juggernaut => {
                teleport_enabled = rng.gen_bool(0.14);
                liminal_enabled = rng.gen_bool(0.12);
            }
            MonsterVariant::Giant => {
                liminal_enabled = rng.gen_bool(0.18);
                ranged_enabled = true;
            }
            MonsterVariant::Normal => {
                ranged_enabled = rng.gen_bool(0.22);
            }
        }

        let parent_entity = commands.spawn((
            Name::new(format!("Monster_{}", idx)),
            Monster {
                variant,
                state: initial_state,
                attack_mult,
                defense: defense_mult,
                critical_chance: rng.gen_range(0.01..0.05),
                hunt_range: 11.5 * guard_mult,
                attack_range: rng.gen_range(1.6..2.6) * attack_mult,
                guard_range: 17.0 * guard_mult,
                speed_boost: speed_mult,
                ai_state_timer: rng.gen_range(0.5..1.5),
                jump_cooldown: 0.0,
                is_docile,
                awakened: false,
                is_boss: false,
                teleport_enabled,
                teleport_cooldown: rng.gen_range(2.2..4.2),
                liminal_enabled,
                fold_jump_cooldown: rng.gen_range(0.2..0.5),
                ranged_enabled,
                ranged_cooldown: rng.gen_range(1.4..2.8),
                cosmic_warp_cooldown: 0.0,
                last_announced_state: None,
            },
            Health { current: monster_max_hp * hp_scale, max: monster_max_hp * hp_scale },
            Spatial4D {
                w, target_w: w,
                layer: (w / 5.0).round() as i32,
                is_folded: false,
            },
            crate::components::Velocity4D {
                lin_v: Vec3::ZERO,
                w_v: 0.0,
            },
            Transform::from_translation(pos).with_scale(Vec3::splat(size_scale)),
            GlobalTransform::default(),
            VisibilityBundle::default(),
            // Physics
            RigidBody::Dynamic,
            Collider::ball(radius),
            LockedAxes::ROTATION_LOCKED,
            Damping { linear_damping: 1.0, angular_damping: 4.0 },
            Velocity {
                linvel: Vec3::new(rng.gen_range(-2.2..2.2), 0.0, rng.gen_range(-2.2..2.2)) * speed_scale,
                angvel: Vec3::ZERO,
            },
            ExternalForce::default(),
            ExternalImpulse::default(),
        )).id();
        
        // Spawn 2-4 parts
        let part_count = rng.gen_range(2..=4);
        for _ in 0..part_count {
            let mut axis = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(-0.8..0.8));
            if axis.length_squared() < 1e-6 { axis = Vec3::X; }
            axis = axis.normalize();
            
            let base_offset = axis * rng.gen_range(0.08..0.5);
            let min_s = rng.gen_range(0.08..0.16);
            let max_s = rng.gen_range(0.22..0.5);
            let phase = rng.gen_range(0.0..TAU);
            let speed = rng.gen_range(2.0..4.7);

            let part = commands.spawn((
                MaterialMeshBundle {
                    mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                    material: materials.add(HyperSliceMaterial {
                        base: StandardMaterial {
                            base_color: color,
                            ..default()
                        },
                        extension: HyperSliceExtension {
                            settings: HyperSliceSettings { player_w: 0.0, object_w: w, thickness: 1.0, edge_color: LinearRgba::new(0.2, 0.9, 1.0, 1.0), ..default() },
                            base_texture: None,
                        }
                    }),
                    transform: Transform::from_translation(base_offset).with_scale(Vec3::splat(min_s)),
                    ..default()
                },
                MonsterPart { base_offset, min_scale: min_s, max_scale: max_s, phase, speed },
            )).id();
            commands.entity(parent_entity).push_children(&[part]);
        }

        // --- In-World UI ---
        // Spawn Background HP Bar
        let hp_bg = commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(0.02, 0.05, 0.08, 0.8),
                    custom_size: Some(Vec2::new(1.8, 0.2)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0.0, 1.8, 0.0)),
                ..default()
            },
        )).id();
        
        // Spawn Foreground HP Fill
        let hp_fill = commands.spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::srgba(0.25, 1.0, 0.25, 0.92),
                    custom_size: Some(Vec2::new(1.7, 0.15)),
                    anchor: bevy::sprite::Anchor::CenterLeft,
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(-0.85, 1.801, 0.0)),
                ..default()
            },
            crate::components::MonsterHpBarFill,
        )).id();
        
        // Spawn Monster State Text
        let state_str = match variant {
            MonsterVariant::Giant => "GIANT",
            MonsterVariant::Normal => "WANDER",
            _ => "ELITE",
        };
        
        let state_text = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    state_str,
                    TextStyle {
                        font: asset_server.load("fonts/Mine.ttf"),
                        font_size: 40.0,
                        color: Color::srgba(0.65, 0.8, 1.0, 0.85),
                        ..default()
                    }
                ),
                transform: Transform::from_translation(Vec3::new(0.0, 2.2, 0.0))
                    .with_scale(Vec3::splat(0.015)),
                ..default()
            },
            crate::components::MonsterStateText,
        )).id();

        commands.entity(parent_entity).push_children(&[hp_bg, hp_fill, state_text]);
    }
    info!("Spawned {} monsters with fully parity logic.", num_monsters);
}
