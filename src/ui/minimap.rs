use bevy::prelude::*;
use crate::player::Player;

use super::components::*;

pub(crate) fn update_minimap(
    time: Res<Time>,
    mut map_timer: ResMut<HoloMapUpdateTimer>,
    player_query: Query<(&Transform, &crate::components::Spatial4D), With<Player>>,
    mob_query: Query<(Entity, &Transform, &crate::ai::Monster)>,
    dot_query: Query<(Entity, &MiniMapDot)>,
    mut root_query: Query<(Entity, &mut Style), With<HoloMapRoot>>,
    clipper_query: Query<Entity, With<HoloMapClipper>>,
    health_query: Query<(Entity, &Transform), With<crate::world::items::HealthPowerup>>,
    sword_query: Query<(Entity, &Transform), With<crate::world::items::SwordPowerup>>,
    anomaly_query: Query<(Entity, &Transform, &crate::world::anomalies::Anomaly)>,
    mut commands: Commands,
) {
    // Throttled update
    let dt = time.delta_seconds();
    map_timer.cooldown -= dt;
    if map_timer.cooldown > 0.0 { return; }
    map_timer.cooldown = 1.0 / 18.0; // Optimized for performance

    let Ok(player_data) = player_query.get_single() else { return };
    let (player_tf, _player_sp) = player_data;
    let Ok((_root_ent, mut root_style)) = root_query.get_single_mut() else { return };
    let Ok(clipper_ent) = clipper_query.get_single() else { return };

    let t = time.elapsed_seconds();
    let panel_pulse = 0.5 + 0.5 * (t * 2.7).sin();
    let scale = 1.0 + panel_pulse * 0.01;
    root_style.width = Val::Px(200.0 * scale);
    root_style.height = Val::Px(200.0 * scale);

    let center = player_tf.translation;
    let radius = 36.0;
    let inv_radius = 1.0 / radius;
    let map_scale = 100.0 * scale; 

    struct DotInfo {
        pos: Vec2,
        size: Vec2,
        color: Color,
        radius: f32, // for circle nodes
    }
    let mut desired_dots = Vec::new();

    let project = |world_pos: Vec3| -> Option<Vec2> {
        let delta = world_pos - center;
        let dist = Vec2::new(delta.x, delta.z).length();
        if dist > radius { return None; }
        Some(Vec2::new(delta.x * inv_radius * map_scale, -delta.z * inv_radius * map_scale))
    };

    // 1. Gather all desired dots (Python Parity: Entity Radar Only)
    // Player
    desired_dots.push(DotInfo { pos: Vec2::ZERO, size: Vec2::splat(15.0), color: Color::srgba(1.0, 1.0, 1.0, 1.0), radius: 0.0 });
    
    // Mobs, Powerups, etc (Dots)
    for (_, tf, _) in mob_query.iter() {
        if let Some(pos) = project(tf.translation) {
            desired_dots.push(DotInfo { pos, size: Vec2::splat(6.0), color: Color::srgba(1.0, 0.45, 0.45, 1.0), radius: 0.0 });
        }
    }
    for (_, tf) in health_query.iter() {
        if let Some(pos) = project(tf.translation) {
            desired_dots.push(DotInfo { pos, size: Vec2::splat(4.5), color: Color::srgba(0.4, 1.0, 0.4, 0.95), radius: 0.0 });
        }
    }
    for (_, tf) in sword_query.iter() {
        if let Some(pos) = project(tf.translation) {
            desired_dots.push(DotInfo { pos, size: Vec2::splat(4.5), color: Color::srgba(1.0, 0.85, 0.3, 0.95), radius: 0.0 });
        }
    }
    for (_, tf, anomaly) in anomaly_query.iter() {
        if let Some(pos) = project(tf.translation) {
            let color = match anomaly.kind {
                crate::world::anomalies::AnomalyKind::Blow => Color::srgba(0.5, 0.9, 1.0, 0.95),
                crate::world::anomalies::AnomalyKind::Suck => Color::srgba(1.0, 0.6, 0.9, 0.95),
            };
            desired_dots.push(DotInfo { pos, size: Vec2::splat(5.0), color, radius: 0.0 });
        }
    }


    // 2. Map existing entities to desired dots
    let existing_dots: Vec<Entity> = dot_query.iter().map(|(e, _)| e).collect();
    
    for (i, dot) in desired_dots.iter().enumerate() {
        if i < existing_dots.len() {
            let ent = existing_dots[i];
            commands.entity(ent).insert(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    left: Val::Px(map_scale + dot.pos.x - dot.size.x * 0.5),
                    top: Val::Px(map_scale + dot.pos.y - dot.size.y * 0.5),
                    width: Val::Px(dot.size.x),
                    height: Val::Px(dot.size.y),
                    ..default()
                },
                background_color: BackgroundColor(dot.color),
                border_radius: BorderRadius::all(Val::Px(dot.radius)),
                ..default()
            });
        } else {
            // Spawn new dot
            let new_dot = commands.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(map_scale + dot.pos.x - dot.size.x * 0.5),
                        top: Val::Px(map_scale + dot.pos.y - dot.size.y * 0.5),
                        width: Val::Px(dot.size.x),
                        height: Val::Px(dot.size.y),
                        ..default()
                    },
                    background_color: BackgroundColor(dot.color),
            border_radius: BorderRadius::all(Val::Px(dot.radius)),
                    ..default()
                },
                MiniMapDot { _entity_ref: Entity::PLACEHOLDER },
            )).id();
            commands.entity(clipper_ent).add_child(new_dot);
        }
    }

    // 3. Despawn excess dots
    if existing_dots.len() > desired_dots.len() {
        for &ent in &existing_dots[desired_dots.len()..] {
            commands.entity(ent).despawn_recursive();
        }
    }
}
