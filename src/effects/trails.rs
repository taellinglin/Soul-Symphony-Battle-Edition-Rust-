use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy::render::view::RenderLayers;

#[derive(Resource)]
pub struct TrailAssets {
    pub motion_trail_handle: Handle<EffectAsset>,
    pub weapon_trail_handle: Handle<EffectAsset>,
}

pub struct TrailPlugin;

impl Plugin for TrailPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_trail_assets)
           .add_systems(Update, (
                spawn_motion_trails,
                spawn_weapon_trails,
                despawn_finished_effects,
            ).run_if(in_state(crate::systems::progression::GameState::Playing)));
    }
}

fn setup_trail_assets(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    mut images: ResMut<Assets<Image>>,
) {
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    use bevy::render::render_asset::RenderAssetUsages;
    
    let size = 64;
    let mut data = vec![0u8; size * size * 4];
    let center = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = (1.0 - (dist / center)).max(0.0).powf(1.5);
            let a_byte = (alpha * 255.0) as u8;
            let idx = (y * size + x) * 4;
            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = a_byte;
        }
    }
    let circle_tex = images.add(Image::new(
        Extent3d { width: size as u32, height: size as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    ));

    // 1. Motion Trail Asset
    let mut color_gradient = Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.35, 0.82, 1.0, 0.42));
    color_gradient.add_key(1.0, Vec4::new(0.35, 0.82, 1.0, 0.0));

    let writer = ExprWriter::new();
    let init_pos = SetPositionCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Y).expr(),
        radius: writer.lit(0.1).expr(),
        dimension: ShapeDimension::Surface,
    };
    let init_lifetime = SetAttributeModifier {
        attribute: Attribute::LIFETIME,
        value: writer.lit(0.22).expr(),
    };
    let init_velocity = SetAttributeModifier {
        attribute: Attribute::VELOCITY,
        value: writer.lit(Vec3::ZERO).expr(),
    };
    let rotation = Some(writer.lit(0.0).expr());

    let motion_effect = EffectAsset::new(vec![1], Spawner::once(1.0.into(), true), writer.finish())
        .with_name("motion_trail")
        .init(init_pos)
        .init(init_lifetime)
        .init(init_velocity)
        .render(ColorOverLifetimeModifier { gradient: color_gradient })
        .render(SizeOverLifetimeModifier { 
            gradient: Gradient::constant(Vec2::splat(0.66)),
            screen_space_size: false 
        })
        .render(ParticleTextureModifier {
            texture: circle_tex.clone(),
            sample_mapping: ImageSampleMapping::Modulate,
        })

        .render(OrientModifier {
            mode: OrientMode::ParallelCameraDepthPlane,
            rotation,
        });

    // 2. Weapon Trail Asset
    let mut weapon_color = Gradient::new();
    weapon_color.add_key(0.0, Vec4::new(0.28, 0.95, 1.0, 0.36));
    weapon_color.add_key(1.0, Vec4::new(0.28, 0.95, 1.0, 0.0));

    let writer = ExprWriter::new();
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(0.0).expr(),
        dimension: ShapeDimension::Surface,
    };
    let init_lifetime = SetAttributeModifier {
        attribute: Attribute::LIFETIME,
        value: writer.lit(0.16).expr(),
    };
    let init_velocity = SetAttributeModifier {
        attribute: Attribute::VELOCITY,
        value: writer.lit(Vec3::ZERO).expr(),
    };

    let weapon_effect = EffectAsset::new(vec![1], Spawner::once(1.0.into(), true), writer.finish())
        .with_name("weapon_trail")
        .init(init_pos)
        .init(init_lifetime)
        .init(init_velocity)
        .render(ColorOverLifetimeModifier { gradient: weapon_color })
        .render(SizeOverLifetimeModifier { 
            gradient: Gradient::constant(Vec2::splat(0.08)),
            screen_space_size: false 
        })
        .render(ParticleTextureModifier {
            texture: circle_tex.clone(),
            sample_mapping: ImageSampleMapping::Modulate,
        })

        .render(OrientModifier {
            mode: OrientMode::ParallelCameraDepthPlane,
            rotation,
        });

    commands.insert_resource(TrailAssets {
        motion_trail_handle: effects.add(motion_effect),
        weapon_trail_handle: effects.add(weapon_effect),
    });
}

#[derive(Component)]
pub struct MotionTrailEmitter {
    pub timer: f32,
}

#[derive(Component)]
pub struct WeaponTrailEmitter;

#[derive(Component)]
pub struct EffectLifetime(pub Timer);

fn spawn_motion_trails(
    mut commands: Commands,
    trail_assets: Res<TrailAssets>,
    query: Query<(Entity, &Transform, &bevy_rapier3d::prelude::Velocity), With<crate::player::Player>>,
    mut timer: Local<f32>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    *timer -= dt;

    if let Ok((_entity, transform, velocity)) = query.get_single() {
        let speed = velocity.linvel.length();
        if speed > 8.5 && *timer <= 0.0 {
            commands.spawn((
                ParticleEffectBundle {
                    effect: ParticleEffect::new(trail_assets.motion_trail_handle.clone()),
                    transform: Transform::from_translation(transform.translation + Vec3::new(0.0, -0.02, 0.0)),
                    ..default()
                },
                EffectLifetime(Timer::from_seconds(0.5, TimerMode::Once)),
                RenderLayers::layer(2),
            ));

            *timer = 0.032;
        }
    }
}

fn spawn_weapon_trails(
    mut commands: Commands,
    trail_assets: Res<TrailAssets>,
    player_query: Query<&crate::weapon_system::Weapon>,
    weapon_query: Query<(Entity, &GlobalTransform), With<crate::weapon_system::Weapon>>,
    mut timer: Local<f32>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    *timer -= dt;

    if let Ok(weapon) = player_query.get_single() {
        if weapon.state == crate::weapon_system::WeaponState::Spin && *timer <= 0.0 {
            for (_entity, gt) in weapon_query.iter() {
                commands.spawn((
                    ParticleEffectBundle {
                        effect: ParticleEffect::new(trail_assets.weapon_trail_handle.clone()),
                        transform: Transform::from_translation(gt.translation()),
                        ..default()
                    },
                    EffectLifetime(Timer::from_seconds(0.5, TimerMode::Once)),
                    RenderLayers::layer(2),
                ));
            }
            *timer = 0.018;
        }
    }
}

fn despawn_finished_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut EffectLifetime)>,
) {
    for (entity, mut life) in query.iter_mut() {
        life.0.tick(time.delta());
        if life.0.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
