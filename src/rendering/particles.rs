use bevy::prelude::*;
use bevy_hanabi::prelude::*;

pub struct AtmosphericParticlesPlugin;

impl Plugin for AtmosphericParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (
            spawn_gravity_fields,
            spawn_background_stars,
        ).run_if(in_state(crate::systems::progression::GameState::Playing)));
    }
}

#[derive(Component)]
pub struct GravityFieldEmitter;

#[derive(Component)]
pub struct BackgroundStarsEmitter;

fn spawn_gravity_fields(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    rooms_query: Query<(Entity, &crate::map::Room), Added<crate::map::Room>>,
) {
    for (_entity, room) in rooms_query.iter() {
        let _size = Vec3::new(room.w, 15.0, room.h);
        
        let mut color_gradient = Gradient::new();
        color_gradient.add_key(0.0, Vec4::new(0.3, 0.8, 1.0, 0.0));
        color_gradient.add_key(0.2, Vec4::new(0.3, 0.8, 1.0, 0.6));
        color_gradient.add_key(0.8, Vec4::new(0.3, 0.8, 1.0, 0.4));
        color_gradient.add_key(1.0, Vec4::new(0.3, 0.8, 1.0, 0.0));

        let mut size_gradient = Gradient::new();
        size_gradient.add_key(0.0, Vec2::splat(0.0));
        size_gradient.add_key(0.1, Vec2::splat(0.024));
        size_gradient.add_key(0.9, Vec2::splat(0.024));
        size_gradient.add_key(1.0, Vec2::splat(0.0));

        let writer = ExprWriter::new();
        let init_pos = SetPositionSphereModifier {
            center: writer.lit(Vec3::ZERO).expr(),
            radius: writer.lit(room.w.max(room.h) * 0.5).expr(),
            dimension: ShapeDimension::Volume,
        };
        let init_vel = SetVelocityCircleModifier {
            center: writer.lit(Vec3::ZERO).expr(),
            axis: writer.lit(Vec3::Y).expr(),
            speed: writer.lit(0.15).expr(),
        };
        let init_lifetime = SetAttributeModifier {
            attribute: Attribute::LIFETIME,
            value: writer.lit(8.0).expr(),
        };
        let init_velocity = SetAttributeModifier {
            attribute: Attribute::VELOCITY,
            value: writer.lit(Vec3::ZERO).expr(),
        };
        let drag = LinearDragModifier::new(writer.lit(0.1).expr());

        let rotation = Some(writer.lit(0.0).expr());

        let effect = EffectAsset::new(vec![1024], Spawner::rate(24.0.into()), writer.finish())
            .with_name("gravity_field")
            .init(init_pos)
            .init(init_vel)
            .init(init_lifetime)
            .init(init_velocity)
            .update(drag)
            .render(ColorOverLifetimeModifier { gradient: color_gradient })
            .render(SizeOverLifetimeModifier { gradient: size_gradient, screen_space_size: false })
            .render(OrientModifier {
                mode: OrientMode::ParallelCameraDepthPlane,
                rotation,
            });

        let effect_handle = effects.add(effect);

        let center = Vec3::new(room.x + room.w * 0.5, 0.0, room.y + room.h * 0.5);

        commands.spawn((
            ParticleEffectBundle {
                effect: ParticleEffect::new(effect_handle),
                transform: Transform::from_translation(center),
                ..default()
            },
            GravityFieldEmitter,
        ));
    }
}

fn spawn_background_stars(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<Entity, Added<crate::player::PlayerCamera>>,
) {
    for _camera_entity in query.iter() {
        let mut color_gradient = Gradient::new();
        color_gradient.add_key(0.0, Vec4::new(1.0, 1.0, 1.0, 0.0));
        color_gradient.add_key(0.5, Vec4::new(1.0, 1.0, 1.0, 0.8));
        color_gradient.add_key(1.0, Vec4::new(1.0, 1.0, 1.0, 0.0));

        let writer = ExprWriter::new();
        let init_pos = SetPositionSphereModifier {
            center: writer.lit(Vec3::ZERO).expr(),
            radius: writer.lit(400.0).expr(),
            dimension: ShapeDimension::Volume,
        };
        let init_lifetime = SetAttributeModifier {
            attribute: Attribute::LIFETIME,
            value: writer.lit(100.0).expr(), // Long lifetime for stars
        };
        let init_velocity = SetAttributeModifier {
            attribute: Attribute::VELOCITY,
            value: writer.lit(Vec3::ZERO).expr(),
        };

        let rotation = Some(writer.lit(0.0).expr());

        let effect = EffectAsset::new(vec![512], Spawner::once(256.0.into(), true), writer.finish())
            .with_name("background_stars")
            .init(init_pos)
            .init(init_lifetime)
            .init(init_velocity)
            .render(ColorOverLifetimeModifier { gradient: color_gradient })
            .render(SizeOverLifetimeModifier { 
                gradient: Gradient::constant(Vec2::splat(0.12)),
                screen_space_size: false 
            })
            .render(OrientModifier { 
                mode: OrientMode::ParallelCameraDepthPlane, 
                rotation 
            });

        let effect_handle = effects.add(effect);

        commands.spawn((
            ParticleEffectBundle {
                effect: ParticleEffect::new(effect_handle),
                transform: Transform::from_xyz(211.0, 0.0, 211.0),
                ..default()
            },
            BackgroundStarsEmitter,
        ));
    }
}
