use bevy::prelude::*;
use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaterCrystalState {
    Falling,
    Stuck,
    Floating,
    Fading,
}

#[derive(Component)]
struct WaterCrystal {
    state: WaterCrystalState,
    state_age: f32,
    phase: f32,
    hue: f32,
    hue_speed: f32,
    sat: f32,
    val: f32,
    alpha_base: f32,
    transparent: bool,
    fall_speed: f32,
    float_offset: f32,
}

#[derive(Resource)]
struct WaterCrystalTimer {
    timer: f32,
}

impl Default for WaterCrystalTimer {
    fn default() -> Self {
        Self { timer: 0.0 }
    }
}

pub struct WaterCrystalPlugin;

impl Plugin for WaterCrystalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WaterCrystalTimer>()
            .add_systems(Update, (spawn_water_crystals, update_water_crystals)
                .run_if(in_state(crate::systems::progression::GameState::Playing)));
    }
}

// Original-integrated constants (main.py:419–430, 7092–7119)
const SPAWN_ENABLED: bool = true;
const SPAWN_INTERVAL: f32 = 0.24;
const MAX_COUNT: usize = 84;
const FALL_GRAVITY: f32 = 14.5;
const SPAWN_AREA_SCALE: f32 = 2.1;
const SPAWN_HEIGHT_MIN: f32 = 8.0;
const SPAWN_HEIGHT_MAX: f32 = 24.0;
const STUCK_DURATION: f32 = 0.9;
const FLOAT_DURATION: f32 = 10.0;
const FADE_DURATION: f32 = 1.5;

fn spawn_water_crystals(
    time: Res<Time>,
    mut timer: ResMut<WaterCrystalTimer>,
    config: Res<crate::map::GenerationConfig>,
    existing: Query<(), With<WaterCrystal>>,
    player_q: Query<&Transform, With<crate::player::Player>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if !SPAWN_ENABLED {
        return;
    }

    let existing_count = existing.iter().count();
    if existing_count >= MAX_COUNT {
        return;
    }

    timer.timer -= time.delta_seconds();
    if existing_count == 0 && timer.timer <= 0.0 {
        // Original: when none exist, spawn ~10 immediately
        timer.timer = SPAWN_INTERVAL;
        for _ in 0..10 {
            spawn_one_crystal(&config, player_q.get_single().ok(), &mut commands, &mut meshes, &mut materials);
        }
        return;
    }

    while timer.timer <= 0.0 {
        spawn_one_crystal(&config, player_q.get_single().ok(), &mut commands, &mut meshes, &mut materials);
        timer.timer += SPAWN_INTERVAL.max(0.05);
        if existing.iter().count() >= MAX_COUNT {
            break;
        }
    }
}

fn spawn_one_crystal(
    config: &crate::map::GenerationConfig,
    player_tf: Option<&Transform>,
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let mut rng = rand::thread_rng();
    let map_w = 176.0 * config.scale;
    let map_d = 176.0 * config.scale;

    let mut x = rng.gen_range(0.0..map_w);
    let mut z = rng.gen_range(0.0..map_d);
    if let Some(tf) = player_tf {
        let ball_pos = tf.translation;
        let local_spread = (SPAWN_AREA_SCALE * 7.0).max(10.0);
        x = ball_pos.x + rng.gen_range(-local_spread..local_spread);
        z = ball_pos.z + rng.gen_range(-local_spread..local_spread);
    }

    let t = 0.0_f32; // will be sampled in update immediately
    let water_h = crate::player::sample_water_height(x, z, t);
    let float_offset = rng.gen_range(0.02..0.16);
    let start_h = water_h + rng.gen_range(SPAWN_HEIGHT_MIN..SPAWN_HEIGHT_MAX.max(SPAWN_HEIGHT_MIN + 0.1));

    let sx = rng.gen_range(0.26..0.56);
    let sy = rng.gen_range(0.26..0.56);
    let sz = rng.gen_range(0.72..1.45);

    let transparent = rng.gen::<f32>() < 0.52;
    let alpha_base = if transparent { rng.gen_range(0.65..0.9) } else { 1.0 };

    let hue = rng.gen::<f32>();
    let sat = rng.gen_range(0.75..1.0);
    let val = rng.gen_range(0.9..1.0);

    let (r, g, b) = hsv_to_rgb(hue, sat, val);
    let base_color = Color::srgba(r, g, b, alpha_base);

    let alpha_mode = if transparent { AlphaMode::Add } else { AlphaMode::Blend };
    let material = materials.add(StandardMaterial {
        base_color,
        emissive: LinearRgba::new((r * 1.55).min(2.0), (g * 1.55).min(2.0), (b * 1.55).min(2.0), 1.0),
        unlit: true,
        alpha_mode,
        ..default()
    });

    commands.spawn((
        WaterCrystal {
            state: WaterCrystalState::Falling,
            state_age: 0.0,
            phase: rng.gen_range(0.0..std::f32::consts::TAU),
            hue,
            hue_speed: rng.gen_range(0.22..0.7),
            sat,
            val,
            alpha_base,
            transparent,
            fall_speed: rng.gen_range(0.0..1.0),
            float_offset,
        },
        PbrBundle {
            mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
            material,
            transform: Transform::from_xyz(x, start_h, z)
                .with_scale(Vec3::new(sx, sz, sy))
                .with_rotation(Quat::from_euler(
                    EulerRot::YXZ,
                    rng.gen_range(0.0..std::f32::consts::TAU),
                    rng.gen_range(-26.0_f32.to_radians()..26.0_f32.to_radians()),
                    rng.gen_range(-24.0_f32.to_radians()..24.0_f32.to_radians()),
                )),
            ..default()
        },
    ));
}

fn update_water_crystals(
    time: Res<Time>,
    mut q: Query<(Entity, &mut Transform, &mut WaterCrystal, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    let dt = time.delta_seconds();
    let t = time.elapsed_seconds();

    for (ent, mut tf, mut crystal, mat_h) in q.iter_mut() {
        crystal.state_age += dt;

        let water_h = crate::player::sample_water_height(tf.translation.x, tf.translation.z, t);
        let target_h = water_h + crystal.float_offset;

        match crystal.state {
            WaterCrystalState::Falling => {
                crystal.fall_speed += FALL_GRAVITY * dt;
                tf.translation.y -= crystal.fall_speed * dt;
                if tf.translation.y <= target_h {
                    tf.translation.y = target_h;
                    crystal.state = WaterCrystalState::Stuck;
                    crystal.state_age = 0.0;
                }
            }
            WaterCrystalState::Stuck => {
                tf.translation.y += (target_h - tf.translation.y) * (dt * 5.5).min(1.0);
                if crystal.state_age >= STUCK_DURATION {
                    crystal.state = WaterCrystalState::Floating;
                    crystal.state_age = 0.0;
                }
            }
            WaterCrystalState::Floating => {
                let bob = 0.02 * (t * 2.4 + crystal.phase).sin();
                tf.translation.y += ((target_h + bob) - tf.translation.y) * (dt * 4.6).min(1.0);
                if crystal.state_age >= FLOAT_DURATION {
                    crystal.state = WaterCrystalState::Fading;
                    crystal.state_age = 0.0;
                }
            }
            WaterCrystalState::Fading => {
                let fade_t = (crystal.state_age / FADE_DURATION.max(0.05)).clamp(0.0, 1.0);
                tf.translation.y += ((target_h + 0.015) - tf.translation.y) * (dt * 4.0).min(1.0);
                if fade_t >= 1.0 {
                    commands.entity(ent).despawn_recursive();
                    continue;
                }
            }
        }

        let hue = (crystal.hue + t * crystal.hue_speed) % 1.0;
        let (r, g, b) = hsv_to_rgb(hue, crystal.sat.clamp(0.0, 1.0), crystal.val.clamp(0.0, 1.0));

        let mut alpha = crystal.alpha_base;
        if crystal.state == WaterCrystalState::Fading {
            let fade_t = (crystal.state_age / FADE_DURATION.max(0.05)).clamp(0.0, 1.0);
            alpha *= (1.0 - fade_t).powf(1.6);
        }

        let spin: f32 = if crystal.transparent { 36.0 } else { 24.0 };
        tf.rotate_y((spin.to_radians()) * dt);

        if let Some(mat) = materials.get_mut(mat_h) {
            mat.base_color = Color::srgba(r, g, b, alpha.clamp(0.0, 1.0));
            mat.emissive = LinearRgba::new((r * 1.55).min(2.0), (g * 1.55).min(2.0), (b * 1.55).min(2.0), 1.0);
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    // h in [0,1)
    let h6 = (h.fract() * 6.0).clamp(0.0, 5.999_999);
    let i = h6.floor();
    let f = h6 - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    match i as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

