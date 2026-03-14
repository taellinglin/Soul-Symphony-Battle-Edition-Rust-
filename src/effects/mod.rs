pub mod audio;
pub mod particles;
pub mod viscous;

use bevy::{prelude::*, render::view::RenderLayers};

pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            particles::FxParticlesPlugin,
            audio::InternalAudioPlugin,
            viscous::ViscousDistortPlugin,
        ));

        app.add_event::<FloatingTextEvent>();

        app.add_systems(Update, (floating_text_system, spawn_floating_text_handler));
    }

    fn finish(&self, app: &mut App) {
        let _ = app;
    }
}

// ----------------------------------------------------------------------------
// 3. Floating Text
// ----------------------------------------------------------------------------

#[derive(Component)]
pub struct FloatingText {
    pub velocity: Vec3,
    pub life: Timer,
    pub base_scale: f32,
}

#[derive(Event)]
pub struct FloatingTextEvent {
    pub pos: Vec3,
    pub text: String,
    pub color: Color,
    pub scale: f32,
    pub life: f32,
}

fn spawn_floating_text_handler(
    mut commands: Commands,
    mut events: EventReader<FloatingTextEvent>,
    asset_server: Res<AssetServer>,
) {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    for event in events.read() {
        // Python: vel = Vec3(uniform(-0.2, 0.2), uniform(-0.2, 0.2), uniform(0.7, 1.05))
        let vel = Vec3::new(
            rng.gen_range(-0.2..0.2),
            rng.gen_range(0.7..1.05),
            rng.gen_range(-0.2..0.2),
        );
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    &event.text,
                    TextStyle {
                        font: asset_server.load("fonts/Mine.ttf"),
                        font_size: 60.0,
                        color: event.color,
                    },
                )
                .with_justify(JustifyText::Center),
                transform: Transform::from_translation(event.pos)
                    .with_scale(Vec3::splat(event.scale * 0.01)),
                ..default()
            },
            RenderLayers::layer(1),
            FloatingText {
                velocity: vel,
                life: Timer::from_seconds(event.life, TimerMode::Once),
                base_scale: event.scale * 0.01,
            },
        ));
    }
}

fn floating_text_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut FloatingText, &mut Text)>,
) {
    let dt = time.delta_seconds();
    for (entity, mut transform, mut ft, mut text) in query.iter_mut() {
        ft.life.tick(time.delta());
        if ft.life.finished() {
            commands.entity(entity).despawn();
            continue;
        }

        let t = ft.life.fraction(); // 0..1 progress

        // Python: pos += vel * dt
        transform.translation += ft.velocity * dt;
        // Python: vel *= max(0.0, 1.0 - dt * 1.7)
        ft.velocity *= (1.0 - dt * 1.7).max(0.0);

        // Python: scale = base_scale * (1.0 + 0.35 * t)
        let current_scale = ft.base_scale * (1.0 + 0.35 * t);
        transform.scale = Vec3::splat(current_scale);

        // Python: alpha = (1.0 - t)^2 — quadratic fade
        let fade = 1.0 - t;
        let alpha = (fade * fade).max(0.0);
        for section in text.sections.iter_mut() {
            section.style.color.set_alpha(alpha);
        }
    }
}
