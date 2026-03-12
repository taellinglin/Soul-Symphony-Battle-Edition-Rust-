use bevy::prelude::*;

use crate::components::Spatial4D;
use crate::player::Player;
use rand::Rng;
use bevy_rapier3d::prelude::*;

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnItemEvent>()
           .add_systems(Update, (
               spawn_monsters_items_handler,
               spawn_item_handler,
               item_bobbing,
               item_attraction,
               item_pickup_collision,
               update_water_crystals,
           ));
    }
}

#[derive(Component)]
pub struct MapItemsSpawned;

// ─────────────────────────────────────────────────────────────────────────────
// Components
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Component)]
pub struct Item;

#[derive(Component)]
pub struct ExpOrb {
    pub value: f32,
    pub phase: f32,
    pub speed: f32,
    pub base_y: f32,
}

#[derive(Component)]
pub struct HealthPowerup {
    pub heal: f32,
    pub phase: f32,
    pub speed: f32,
    pub base_y: f32,
}

#[derive(Component)]
pub struct SwordPowerup {
    pub powerup_type: String,
    pub phase: f32,
    pub speed: f32,
    pub base_y: f32,
}

#[derive(Component)]
pub struct WaterCrystal {
    pub state: CrystalState,
    pub state_age: f32,
    pub fall_speed: f32,
    pub base_y: f32,
}

#[derive(PartialEq)]
pub enum CrystalState {
    Falling,
    Stuck,
    Floating,
    Fading,
}

// ─────────────────────────────────────────────────────────────────────────────
// Events
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Event)]
pub enum SpawnItemEvent {
    Exp { pos: Vec3, amount: f32 },
    Health { pos: Vec3, heal: f32 },
    Sword { pos: Vec3, powerup_type: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

fn spawn_monsters_items_handler(
    mut commands: Commands,
    graph: Res<crate::map::DungeonGraph>,
    query: Query<(), With<MapItemsSpawned>>,
    mut item_events: EventWriter<SpawnItemEvent>,
) {
    if !query.is_empty() { return; }
    if graph.rooms.is_empty() { return; }

    let mut rng = rand::thread_rng();
    
    // Spawn Health Powerups
    for (i, room) in graph.rooms.iter().enumerate() {
        let area = room.w * room.h;
        let scale_factor = (area / 400.0).max(1.0) as u32; // Assuming 20x20 is a standard room

        // Spawn 1-4 health powerups per standard room area (Python parity)
        let base_count = rng.gen_range(1..5);
        let count = (base_count * scale_factor).min(80); // Cap to prevent insane amounts in huge rooms

        for _ in 0..count {
            let x = rng.gen_range(room.x + 1.0 .. room.x + room.w - 1.0);
            let z = rng.gen_range(room.y + 1.0 .. room.y + room.h - 1.0);
            let y = 1.25;
            
            item_events.send(SpawnItemEvent::Health {
                pos: Vec3::new(x, y, z),
                heal: rng.gen_range(12.0..20.0),
            });
        }
        
        // Spawn Sword Powerup (Rarely - Python 22% parity)
        if rng.gen_bool(0.22) || i == 0 {
            let sword_count = (1 * scale_factor).max(1).min(10); // Multiple swords for huge rooms
            
            for _ in 0..sword_count {
                let x = rng.gen_range(room.x + 1.0 .. room.x + room.w - 1.0);
                let z = rng.gen_range(room.y + 1.0 .. room.y + room.h - 1.0);
                let y = 1.25;
                
                let pickup_catalog = [
                    ("attack", 100), ("defense", 95), ("dex", 90), ("sta", 95), 
                    ("int", 88), ("haste", 56), ("longblade", 52), ("fury", 48), 
                    ("crit_core", 45), ("critical", 42),
                ];
                let total_weight: i32 = pickup_catalog.iter().map(|e| e.1).sum();
                let mut roll = rng.gen_range(0..total_weight);
                let mut p_type = "attack".to_string();
                for (name, weight) in pickup_catalog {
                    if roll < weight {
                        p_type = name.to_string();
                        break;
                    }
                    roll -= weight;
                }
                
                item_events.send(SpawnItemEvent::Sword {
                    pos: Vec3::new(x, y, z),
                    powerup_type: p_type,
                });
            }
        }
    }

    commands.spawn(MapItemsSpawned);
}

fn spawn_item_handler(
    mut commands: Commands,
    mut events: EventReader<SpawnItemEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();

    for event in events.read() {
        match event {
            SpawnItemEvent::Exp { pos, amount } => {
                let mut remaining = *amount;
                while remaining > 0.0 {
                    let val = (remaining * rng.gen_range(0.35..0.7)).max(1.0).min(remaining);
                    remaining -= val;
                    
                    let jitter = Vec3::new(rng.gen_range(-0.4..0.4), rng.gen_range(0.0..0.5), rng.gen_range(-0.4..0.4));
                    let scale = 0.12 + 0.03 * val.sqrt();
                    
                    commands.spawn((
                        PbrBundle {
                            mesh: meshes.add(Sphere::new(scale)),
                            material: materials.add(StandardMaterial {
                                base_color: Color::srgba(0.28, 0.9, 1.0, 0.95),
                                emissive: LinearRgba::new(0.24, 0.8, 1.0, 1.0) * 2.0,
                                ..default()
                            }),
                            transform: Transform::from_translation(*pos + jitter),
                            ..default()
                        },
                        Item,
                        ExpOrb {
                            value: val,
                            phase: rng.gen_range(0.0..std::f32::consts::TAU),
                            speed: rng.gen_range(1.2..2.4),
                            base_y: pos.y + jitter.y,
                        },
                        Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
                    ));
                }
            }
            SpawnItemEvent::Health { pos, heal } => {
                // Cross shape or sphere for now
                commands.spawn((
                    PbrBundle {
                        mesh: meshes.add(Cuboid::new(0.5, 0.15, 0.15)),
                        material: materials.add(StandardMaterial {
                            base_color: Color::srgba(0.28, 1.0, 0.42, 0.98),
                            emissive: LinearRgba::new(0.2, 1.0, 0.4, 1.0) * 1.5,
                            ..default()
                        }),
                        transform: Transform::from_translation(*pos),
                        ..default()
                    },
                    Item,
                    HealthPowerup {
                        heal: *heal,
                        phase: rng.gen_range(0.0..std::f32::consts::TAU),
                        speed: rng.gen_range(0.5..1.3),
                        base_y: pos.y,
                    },
                    Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
                ));
            }
            SpawnItemEvent::Sword { pos, powerup_type } => {
                let color = match powerup_type.as_str() {
                    "attack" => Color::srgba(1.0, 0.45, 0.32, 1.0),
                    "defense" => Color::srgba(0.42, 0.9, 1.0, 1.0),
                    "dex" => Color::srgba(1.0, 0.93, 0.36, 1.0),
                    "sta" => Color::srgba(0.44, 1.0, 0.45, 1.0),
                    "int" => Color::srgba(0.78, 0.56, 1.0, 1.0),
                    "haste" => Color::srgba(1.0, 0.84, 0.26, 1.0),
                    "longblade" => Color::srgba(0.32, 0.96, 1.0, 1.0),
                    "fury" => Color::srgba(1.0, 0.42, 0.66, 1.0),
                    "crit_core" => Color::srgba(1.0, 0.54, 0.18, 1.0),
                    "critical" => Color::srgba(1.0, 0.3, 0.24, 1.0),
                    _ => Color::srgba(1.0, 1.0, 1.0, 1.0),
                };
                
                commands.spawn((
                    PbrBundle {
                        mesh: meshes.add(Cuboid::new(0.4, 0.4, 0.4)),
                        material: materials.add(StandardMaterial {
                            base_color: color,
                            emissive: (LinearRgba::from(color) * 2.5),
                            ..default()
                        }),
                        transform: Transform::from_translation(*pos),
                        ..default()
                    },
                    Item,
                    SwordPowerup {
                        powerup_type: powerup_type.clone(),
                        phase: rng.gen_range(0.0..std::f32::consts::TAU),
                        speed: rng.gen_range(1.0..2.0),
                        base_y: pos.y,
                    },
                    Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
                ));
            }
        }
    }
}

fn item_bobbing(
    time: Res<Time>,
    mut query: Query<(&mut Transform, Option<&ExpOrb>, Option<&HealthPowerup>, Option<&SwordPowerup>), With<Item>>,
) {
    let t = time.elapsed_seconds();
    for (mut transform, exp, health, sword) in query.iter_mut() {
        let (phase, speed, amp, base_y) = if let Some(e) = exp {
            (e.phase, e.speed, 0.16, e.base_y) // Python exp_orb bob is 0.16
        } else if let Some(h) = health {
            (h.phase, h.speed, 0.42, h.base_y) // Python health_powerup bob is 0.42
        } else if let Some(s) = sword {
            (s.phase, s.speed, 0.38, s.base_y)
        } else {
            continue;
        };
        
        transform.translation.y = base_y + (t * speed * 5.4 + phase).sin() * amp;
        transform.rotation = Quat::from_rotation_y(t * speed * std::f32::consts::PI);
    }
}

fn item_attraction(
    time: Res<Time>,
    player_query: Query<&Transform, With<Player>>,
    mut item_query: Query<(&mut Transform, Option<&ExpOrb>, Option<&HealthPowerup>), (With<Item>, Without<Player>)>,
) {
    let dt = time.delta_seconds();
    if let Ok(player_tf) = player_query.get_single() {
        let p_pos = player_tf.translation;
        
        for (mut i_tf, exp, health) in item_query.iter_mut() {
            let dist = p_pos.distance(i_tf.translation);
            
            let (radius, speed, is_exp) = if exp.is_some() {
                (6.8, 18.5, true) // Original exp attraction constants
            } else if health.is_some() {
                (3.1, 8.2, false) // Original health attraction constants
            } else {
                continue;
            };
            
            if dist > 0.0 && dist < radius {
                let proximity = 1.0 - (dist / radius);
                // Python curves parity
                let pull = if is_exp {
                    1.25 + 2.75 * (proximity * proximity)
                } else {
                    0.45 + 0.55 * proximity
                };
                let move_dist = (speed * dt * pull).min(dist);
                let dir = (p_pos - i_tf.translation).normalize();
                i_tf.translation += dir * move_dist;
            }
        }
    }
}

fn item_pickup_collision(
    mut commands: Commands,
    player_query: Query<(&Transform, &crate::systems::progression::PlayerProgression), With<Player>>,
    mut item_query: Query<(Entity, &Transform, Option<&ExpOrb>, Option<&HealthPowerup>, Option<&SwordPowerup>), With<Item>>,
    mut xp_events: EventWriter<crate::systems::progression::GainXpEvent>,
    mut heal_events: EventWriter<crate::systems::progression::HealEvent>,
    mut sword_events: EventWriter<crate::systems::progression::SwordPowerupEvent>,
    mut fx_events: EventWriter<crate::effects::FloatingTextEvent>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
) {
    if let Ok((player_tf, _progression)) = player_query.get_single() {
        let p_pos = player_tf.translation;
        let p_radius = 0.6; // Ball radius approx
        
        for (entity, i_tf, exp, health, sword) in item_query.iter_mut() {
            let dist = p_pos.distance(i_tf.translation);
            let pickup_radius = if exp.is_some() { 0.6 } else { 0.85 };
            
            if dist < (p_radius + pickup_radius) {
                sfx_events.send(crate::effects::audio::PlaySfxEvent {
                    kind: crate::effects::audio::SfxKind::Pickup,
                    volume: 0.7, pitch: 1.0 + rand::random::<f32>() * 0.2, position: Some(i_tf.translation),
                });
                if let Some(e) = exp {
                    xp_events.send(crate::systems::progression::GainXpEvent { amount: e.value });
                    fx_events.send(crate::effects::FloatingTextEvent {
                        pos: i_tf.translation + Vec3::Y * 0.4,
                        text: format!("XP +{:.0}", e.value),
                        color: Color::srgba(0.3, 0.9, 1.0, 1.0),
                        scale: 0.25,
                        life: 0.8,
                    });
                    commands.entity(entity).despawn_recursive();
                } else if let Some(h) = health {
                    heal_events.send(crate::systems::progression::HealEvent { amount: h.heal });
                    fx_events.send(crate::effects::FloatingTextEvent {
                        pos: i_tf.translation + Vec3::Y * 0.4,
                        text: format!("HP +{:.0}", h.heal),
                        color: Color::srgba(0.35, 1.0, 0.35, 1.0),
                        scale: 0.28,
                        life: 0.9,
                    });
                    commands.entity(entity).despawn_recursive();
                } else if let Some(s) = sword {
                    sword_events.send(crate::systems::progression::SwordPowerupEvent { powerup_type: s.powerup_type.clone() });
                    
                    let (color, text) = match s.powerup_type.as_str() {
                        "fury" => (Color::srgba(1.0, 0.3, 0.3, 1.0), "SWORD FURY".to_string()),
                        "longblade" => (Color::srgba(0.3, 1.0, 0.8, 1.0), "LONGBLADE".to_string()),
                        "crit_core" => (Color::srgba(1.0, 0.9, 0.1, 1.0), "CRIT CORE".to_string()),
                        "critical" => (Color::srgba(1.0, 0.7, 0.0, 1.0), "CRITICAL".to_string()),
                         _ => (Color::srgba(1.0, 0.88, 0.38, 1.0), s.powerup_type.to_uppercase()),
                    };

                    fx_events.send(crate::effects::FloatingTextEvent {
                        pos: i_tf.translation + Vec3::Y * 0.4,
                        text,
                        color,
                        scale: 0.27,
                        life: 0.95,
                    });
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
    }
}

fn update_water_crystals(
    time: Res<Time>,
    mut commands: Commands,
    player_query: Query<&Transform, With<Player>>,
    mut crystal_query: Query<(Entity, &mut Transform, &mut WaterCrystal, &mut Handle<StandardMaterial>), (With<WaterCrystal>, Without<Player>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    rapier_context: Res<bevy_rapier3d::prelude::RapierContext>,
    mut spawn_timer: Local<f32>,
) {
    let dt = time.delta_seconds();
    let roll_time = time.elapsed_seconds() as f32;
    
    // 1. Spawning
    *spawn_timer -= dt;
    if let Ok(player_tf) = player_query.get_single() {
        let mut rng = rand::thread_rng();
        // Python parity: interval = 0.3s
        if *spawn_timer <= 0.0 {
            let count = crystal_query.iter().count();
            if count < 180 {
                *spawn_timer += 0.3; // Reset timer

                let local_spread = 60.0;
                let x = player_tf.translation.x + rng.gen_range(-local_spread..local_spread);
                let z = player_tf.translation.z + rng.gen_range(-local_spread..local_spread);
                let start_y = player_tf.translation.y + rng.gen_range(16.0..32.0);
            
            let color = Color::hsv(rng.gen_range(0.0..360.0), rng.gen_range(0.7..1.0), 1.0);
            
            commands.spawn((
                PbrBundle {
                    mesh: meshes.add(Cuboid::new(0.4, 0.4, 1.0)),
                    material: materials.add(StandardMaterial {
                        base_color: color,
                        emissive: (LinearRgba::from(color) * 2.0),
                        ..default()
                    }),
                    transform: Transform::from_translation(Vec3::new(x, start_y, z))
                        .with_rotation(Quat::from_rotation_x(rng.gen_range(0.0..std::f32::consts::TAU))),
                    ..default()
                },
                Item,
                WaterCrystal {
                    state: CrystalState::Falling,
                    state_age: 0.0,
                    fall_speed: rng.gen_range(0.0..1.0), // Python parity: initially 0.0 to 1.0
                    base_y: 0.0, // Set when landing
                },
                Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
            ));
        } else {
            // Cap reached, just reset timer so we attempt again later
            *spawn_timer += 0.3;
        }
    }
    }

    // 2. State Machine Update
    for (entity, mut tf, mut crystal, mat_handle) in crystal_query.iter_mut() {
        crystal.state_age += dt;
        
        // Use Rapier to find the floor directly below the crystal
        // Use Rapier to find the floor directly below the crystal
        let mut water_h = tf.translation.y - 100.0; // fallback far below
        let ray_origin = tf.translation + Vec3::Y * 0.1; // start slightly above center to avoid intersecting from inside
        if let Some((_, toi)) = rapier_context.cast_ray(
            ray_origin, -Vec3::Y, 150.0, true,
            QueryFilter::default().exclude_sensors().groups(CollisionGroups::new(Group::all(), Group::all().difference(Group::GROUP_32)))
        ) {
            water_h = ray_origin.y - toi;
        }

        match crystal.state {
            CrystalState::Falling => {
                tf.translation.y -= crystal.fall_speed * dt;
                crystal.fall_speed += 14.5 * dt; // Gravity
                if tf.translation.y <= water_h {
                    tf.translation.y = water_h;
                    crystal.state = CrystalState::Stuck;
                    crystal.state_age = 0.0;
                    crystal.base_y = water_h;
                }
            }
            CrystalState::Stuck => {
                if crystal.state_age >= 0.9 {
                    crystal.state = CrystalState::Floating;
                    crystal.state_age = 0.0;
                }
            }
            CrystalState::Floating => {
                let bob = 0.02 * (roll_time * 2.4).sin();
                tf.translation.y = crystal.base_y + bob;
                if crystal.state_age >= 10.0 {
                    crystal.state = CrystalState::Fading;
                    crystal.state_age = 0.0;
                }
            }
            CrystalState::Fading => {
                let alpha = (1.0 - crystal.state_age / 1.5).clamp(0.0, 1.0);
                if let Some(mat) = materials.get_mut(mat_handle.id()) {
                    mat.base_color.set_alpha(alpha);
                }
                if crystal.state_age >= 1.5 {
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        tf.rotate_y(0.5 * dt);
    }
}
