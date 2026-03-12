mod components;
mod combat;

pub use components::*;

use bevy::prelude::*;
use bevy::render::view::{NoFrustumCulling, RenderLayers};
use crate::components::Spatial4D;
use std::collections::VecDeque;

pub struct WeaponSystemPlugin;

#[derive(Resource, Default)]
struct BladeEchoQueue {
    ents: VecDeque<Entity>,
}

impl Plugin for WeaponSystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DamageEvent>()
           .init_resource::<BladeEchoQueue>()
           .add_systems(Update, (
               combat::update_weapon_state,
               combat::apply_damage_events,
               spawn_blade_echoes,
               update_blade_echoes,
               update_slash_trails,
               update_hyperbomb,
               update_magic_missiles,
           ));
    }
}

fn spawn_blade_echoes(
    mut query: Query<(&mut Weapon, &Transform)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut queue: ResMut<BladeEchoQueue>,
    time: Res<Time>,
) {
    for (mut weapon, tf) in query.iter_mut() {
        // Python parity: blade echoes emit during swing/spin only
        if !matches!(weapon.state, WeaponState::Swing | WeaponState::Spin) {
            continue;
        }

        weapon.echo_timer -= time.delta_seconds();
        if weapon.echo_timer > 0.0 {
            continue;
        }
        weapon.echo_timer = 1.0 / 120.0;

        // Python parity: 7-color cycle
        const COLORS: [Vec3; 7] = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.5, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.29, 0.0, 0.51),
            Vec3::new(0.56, 0.0, 1.0),
        ];
        let idx = (weapon.echo_cycle as usize) % COLORS.len();
        weapon.echo_cycle = weapon.echo_cycle.wrapping_add(1);
        let c = COLORS[idx];

        let mat = materials.add(StandardMaterial {
            base_color: Color::srgba(c.x, c.y, c.z, 0.86),
            emissive: LinearRgba::new(c.x.min(1.0), c.y.min(1.0), c.z.min(1.0), 1.0),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });

        // Python parity: stretched blade-shaped box
        let echo_scale = Vec3::new(0.095 * SWORD_SCALE, 0.072 * SWORD_SCALE, 0.74 * SWORD_SCALE);
        let ent = commands.spawn((
            PbrBundle {
                mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                material: mat,
                transform: Transform::from_translation(tf.translation)
                    .with_rotation(tf.rotation)
                    .with_scale(echo_scale),
                ..default()
            },
            BladeEcho { life: 0.4, max_life: 0.4, color: c },
            NoFrustumCulling,
            RenderLayers::layer(2),
        )).id();

        queue.ents.push_back(ent);
        while queue.ents.len() > 16 {
            if let Some(old) = queue.ents.pop_front() {
                commands.entity(old).despawn_recursive();
            }
        }
    }
}

fn update_blade_echoes(
    mut query: Query<(Entity, &mut BladeEcho, &mut Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let dt = time.delta_seconds();
    for (entity, mut echo, mat_handle) in query.iter_mut() {
        echo.life -= dt;
        if echo.life <= 0.0 {
            commands.entity(entity).despawn_recursive();
        } else if let Some(mat) = materials.get_mut(mat_handle.id()) {
            let t = (1.0 - (echo.life / echo.max_life)).clamp(0.0, 1.0);
            let fade = (1.0 - t).max(0.0);
            let white_mix = (1.0 - t * 1.8).max(0.0);
            let c = echo.color;
            let r = c.x * (1.0 - white_mix) + 1.0 * white_mix;
            let g = c.y * (1.0 - white_mix) + 1.0 * white_mix;
            let b = c.z * (1.0 - white_mix) + 1.0 * white_mix;
            mat.base_color = Color::srgba(r, g, b, fade * 0.86);
        }
    }
}

fn update_slash_trails(
    mut query: Query<(Entity, &mut SlashTrail, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let dt = time.delta_seconds();
    for (entity, mut trail, mat_handle) in query.iter_mut() {
        trail.life -= dt;
        if trail.life <= 0.0 {
            commands.entity(entity).despawn_recursive();
        } else if let Some(mat) = materials.get_mut(mat_handle.id()) {
            let alpha = (trail.life / trail.max_life).max(0.0) * 0.72;
            mat.base_color.set_alpha(alpha);
        }
    }
}

fn update_hyperbomb(
    mut commands: Commands,
    mut bomb_query: Query<(Entity, &mut Hyperbomb, &mut Transform), Without<crate::ai::Monster>>,
    monster_query: Query<(Entity, &Transform, &Spatial4D), (With<crate::ai::Monster>, Without<Hyperbomb>)>,
    time: Res<Time>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    let dt = time.delta_seconds();
    for (entity, mut bomb, mut tf) in bomb_query.iter_mut() {
        bomb.timer += dt;
        if bomb.timer >= bomb.duration {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        let _t = (bomb.timer / bomb.duration).min(1.0);
        bomb.radius = bomb.radius + (bomb.max_radius - bomb.radius) * (1.0 - (-dt * 4.2).exp());
        tf.scale = Vec3::splat(bomb.radius);

        if bomb.timer % 0.08 < dt {
             for (m_entity, m_tf, _) in monster_query.iter() {
                 let dist = tf.translation.distance(m_tf.translation);
                 if dist < bomb.radius {
                     damage_events.send(DamageEvent {
                         target: m_entity,
                         amount: 36.0,
                         knockback: (m_tf.translation - tf.translation).normalize_or_zero() * 5.0,
                     });
                 }
             }
        }
    }
}

fn update_magic_missiles(
    mut commands: Commands,
    mut missile_query: Query<(Entity, &mut MagicMissile, &mut Transform, &Spatial4D), Without<crate::ai::Monster>>,
    monster_query: Query<(Entity, &Transform, &Spatial4D), (With<crate::ai::Monster>, Without<MagicMissile>)>,
    time: Res<Time>,
    mut damage_events: EventWriter<DamageEvent>,
) {
    let dt = time.delta_seconds();
    for (entity, mut missile, mut tf, sp) in missile_query.iter_mut() {
        missile.life -= dt;
        if missile.life <= 0.0 {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        if missile.target.is_none() {
            let mut best_dist = 30.0;
            let mut best_target = None;
            for (m_entity, m_tf, m_sp) in monster_query.iter() {
                if (m_sp.layer - sp.layer).abs() > 0 { continue; }
                let dist = tf.translation.distance(m_tf.translation);
                if dist < best_dist {
                    best_dist = dist;
                    best_target = Some(m_entity);
                }
            }
            missile.target = best_target;
        }

        if let Some(target_entity) = missile.target {
            if let Ok((_, target_tf, _)) = monster_query.get(target_entity) {
                let target_dir = (target_tf.translation - tf.translation).normalize_or_zero();
                missile.velocity = missile.velocity + (target_dir * 25.0 - missile.velocity) * (dt * 7.6);
            } else {
                missile.target = None;
            }
        }

        tf.translation += missile.velocity * dt;
        let target_pos = tf.translation + missile.velocity;
        tf.look_at(target_pos, Vec3::Y);

        for (m_entity, m_tf, m_sp) in monster_query.iter() {
             if (m_sp.layer - sp.layer).abs() > 0 { continue; }
             if tf.translation.distance(m_tf.translation) < 0.8 {
                 damage_events.send(DamageEvent {
                     target: m_entity,
                     amount: 34.0,
                     knockback: missile.velocity.normalize_or_zero() * 3.0,
                 });
                 commands.entity(entity).despawn_recursive();
                 break;
             }
        }
    }
}
