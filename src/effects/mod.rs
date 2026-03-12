#![allow(dead_code)]
pub mod particles;
pub mod audio;
pub mod viscous;
pub mod trails;

use bevy::{
    prelude::*,
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    ecs::query::QueryItem,
    render::{
        extract_component::{ComponentUniforms, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin},
        render_graph::{NodeRunError, RenderGraphContext, ViewNode, RenderLabel},
        render_resource::{binding_types::{sampler, texture_2d, uniform_buffer}, *},
        renderer::{RenderContext, RenderDevice},
        view::{ViewTarget, RenderLayers},
        RenderApp,
    },
};


pub struct FxPlugin;

impl Plugin for FxPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<CrtSettings>::default(),
            UniformComponentPlugin::<CrtSettings>::default(),
            particles::FxParticlesPlugin,
            audio::InternalAudioPlugin,
            viscous::ViscousDistortPlugin,
            trails::TrailPlugin,
        ));

        app.add_event::<FloatingTextEvent>();

        app.add_systems(Update, (
            update_crt_settings,
            floating_text_system,
            spawn_floating_text_handler,
        ));

        let Some(_render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        // NOTE: CRT node is disabled — registering it without connecting it in the
        // edge chain causes frame smearing because the orphan node's post_process_write()
        // does a buffer ping-pong swap that corrupts the frame pipeline.
        // To re-enable: uncomment both the node registration AND add CrtLabel to the edges.
        //
        // render_app
        //     .add_render_graph_node::<ViewNodeRunner<CrtNode>>(Core3d, CrtLabel)
        //     .add_render_graph_edges(
        //         Core3d,
        //         (
        //             Node3d::Tonemapping,
        //             CrtLabel,
        //             Node3d::EndMainPassPostProcessing,
        //         ),
        //     );
    }

    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<CrtPipeline>();
    }
}

// ----------------------------------------------------------------------------
// 1. Post Processing Pipeline (CRT)
// ----------------------------------------------------------------------------

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct CrtSettings {
    pub intensity: f32,
    pub aberration_offset: f32,
    pub time: f32,
    pub warp_strength: f32,
}

impl Default for CrtSettings {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            aberration_offset: 0.0,
            time: 0.0,
            warp_strength: 0.0,
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct CrtLabel;

#[derive(Default)]
pub struct CrtNode;

impl ViewNode for CrtNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static CrtSettings,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _crt_settings): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let crt_pipeline = world.resource::<CrtPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(crt_pipeline.pipeline_id) else {
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<CrtSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "crt_bind_group",
            &crt_pipeline.layout,
            &BindGroupEntries::sequential((
                post_process.source,
                &crt_pipeline.sampler,
                settings_binding,
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("crt_pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: post_process.destination,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_render_pipeline(pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}

#[derive(Resource)]
pub struct CrtPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for CrtPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "crt_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<CrtSettings>(false),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let shader = world.resource::<AssetServer>().load("shaders/crt_post_process.wgsl");

        let pipeline_id = world.resource_mut::<PipelineCache>().queue_render_pipeline(
            RenderPipelineDescriptor {
                label: Some("crt_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::Rgba16Float, // Bevy HDR default
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                push_constant_ranges: vec![],
            },
        );

        Self {
            layout,
            sampler,
            pipeline_id,
        }
    }
}

// ----------------------------------------------------------------------------
// 2. Transiet Visual Effects System (4D Ripples)
// ----------------------------------------------------------------------------


fn update_crt_settings(
    mut query: Query<&mut CrtSettings>,
    time: Res<Time>,
) {
    for mut settings in query.iter_mut() {
        settings.time = time.elapsed_seconds();
        // Warp strength could be linked to player velocity or game state
        // For now, let's keep it at 1.0 if intensity is up
        settings.warp_strength = settings.intensity;
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
                ).with_justify(JustifyText::Center),
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
