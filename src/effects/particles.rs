use bevy::prelude::*;
use bevy_hanabi::prelude::*;
#[allow(unused_imports)]
use crate::components::Spatial4D;

pub struct FxParticlesPlugin;

impl Plugin for FxParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (setup_star_particles, setup_gravity_particles));
    }
}

#[derive(Component)]
pub struct StarParticles;

#[derive(Component)]
#[allow(dead_code)]
pub struct GravityParticles;

fn setup_star_particles(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    let mut color_gradient = Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.84, 0.9, 1.0, 0.42));
    color_gradient.add_key(0.5, Vec4::new(0.84, 0.9, 1.0, 0.1));
    color_gradient.add_key(1.0, Vec4::new(0.84, 0.9, 1.0, 0.42));

    let mut size_gradient = Gradient::new();
    size_gradient.add_key(0.0, Vec2::splat(0.02));
    size_gradient.add_key(1.0, Vec2::splat(0.02));

    let writer = ExprWriter::new();

    // Position in a huge volume
    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(150.0).expr(),
        dimension: ShapeDimension::Volume,
    };

    // Slow drift
    let init_vel = SetVelocitySphereModifier {
       center: writer.lit(Vec3::ZERO).expr(),
       speed: writer.lit(0.12).expr(),
    };

    let effect = EffectAsset::new(vec![256], Spawner::once(128.0.into(), true), writer.finish())
        .with_name("StarParticles")
        .init(init_pos)
        .init(init_vel)
        .render(ColorOverLifetimeModifier { gradient: color_gradient })
        .render(SizeOverLifetimeModifier { gradient: size_gradient, screen_space_size: false });

    let effect_handle = effects.add(effect);

    commands.spawn((
        ParticleEffectBundle {
            effect: ParticleEffect::new(effect_handle),
            ..default()
        },
        StarParticles,
    ));
}

fn setup_gravity_particles(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    // This is a template for gravity particles. 
    // We might need to spawn one per room or use a global one with dynamic gravity.
    // For now, let's create a system that can be used by rooms.
    
    let mut color_gradient = Gradient::new();
    color_gradient.add_key(0.0, Vec4::new(0.5, 0.8, 1.0, 0.0));
    color_gradient.add_key(0.2, Vec4::new(0.5, 0.8, 1.0, 0.6));
    color_gradient.add_key(0.8, Vec4::new(0.5, 0.8, 1.0, 0.6));
    color_gradient.add_key(1.0, Vec4::new(0.5, 0.8, 1.0, 0.0));

    let writer = ExprWriter::new();

    let init_pos = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(8.0).expr(), // Room size approximation
        dimension: ShapeDimension::Volume,
    };

    // We'll update Acceleration per frame based on room gravity later
    let accel = AccelModifier::new(writer.lit(Vec3::new(0.0, -9.81, 0.0)).expr());

    let effect = EffectAsset::new(vec![1024], Spawner::rate(24.0.into()), writer.finish())
        .with_name("GravityParticles")
        .init(init_pos)
        .update(accel)
        .render(ColorOverLifetimeModifier { gradient: color_gradient })
        .render(SizeOverLifetimeModifier { gradient: Gradient::constant(Vec2::splat(0.05)), screen_space_size: false });

    let effect_handle = effects.add(effect);
    
    // We don't spawn it globally yet, rooms will spawn it.
    commands.insert_resource(GravityEffect(effect_handle));
}


#[derive(Resource)]
pub struct GravityEffect(pub Handle<EffectAsset>);
