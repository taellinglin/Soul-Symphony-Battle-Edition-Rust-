use bevy::{
    prelude::*,
    core_pipeline::{
        core_3d::graph::{Core3d, Node3d},
        fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    },
    ecs::query::QueryItem,
    render::{
        extract_component::{ComponentUniforms, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin},
        render_graph::{NodeRunError, RenderGraphApp, RenderGraphContext, ViewNode, ViewNodeRunner, RenderLabel},
        render_resource::{binding_types::{sampler, texture_2d, uniform_buffer}, *},
        renderer::{RenderContext, RenderDevice},
        view::ViewTarget,
        RenderApp,
    },
};

pub struct ViscousDistortPlugin;

impl Plugin for ViscousDistortPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractComponentPlugin::<ViscousSettings>::default(),
            UniformComponentPlugin::<ViscousSettings>::default(),
        ));

        app.add_systems(Update, update_viscous_distortion.run_if(in_state(crate::systems::progression::GameState::Playing)));

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app
            .add_render_graph_node::<ViewNodeRunner<ViscousDistortNode>>(Core3d, ViscousDistortLabel)
            .add_render_graph_edges(
                Core3d,
                (
                    Node3d::Tonemapping,
                    ViscousDistortLabel,
                    Node3d::EndMainPassPostProcessing,
                ),
            );
    }
    
    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.init_resource::<ViscousDistortPipeline>();
    }
}

// ----------------------------------------------------------------------------
// 1. Post Processing Pipeline (Viscous Distort)
// ----------------------------------------------------------------------------

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct ViscousSettings {
    pub time: f32,
    pub speed_norm: f32,
    pub strength: f32,
    pub bloom_strength: f32,
    pub bloom_radius: f32,
    pub bloom_threshold: f32,
    pub quantize_steps: f32,
    pub outline_strength: f32,
}

impl Default for ViscousSettings {
    fn default() -> Self {
        Self {
            time: 0.0,
            speed_norm: 0.0,
            strength: 0.0,
            bloom_strength: 0.0,
            bloom_radius: 1.5,
            bloom_threshold: 0.6,
            quantize_steps: 8.0, // Match the "9 bands" aesthetic (roughly)
            outline_strength: 1.2, // Match CartoonInk separation 1.2
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, RenderLabel)]
pub struct ViscousDistortLabel;

#[derive(Default)]
pub struct ViscousDistortNode;

impl ViewNode for ViscousDistortNode {
    type ViewQuery = (
        &'static ViewTarget,
        &'static ViscousSettings,
    );

    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        (view_target, _settings): QueryItem<Self::ViewQuery>,
        world: &World,
    ) -> Result<(), NodeRunError> {
        let pipeline_res = world.resource::<ViscousDistortPipeline>();
        let pipeline_cache = world.resource::<PipelineCache>();
        let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline_res.pipeline_id) else {
            return Ok(());
        };

        let settings_uniforms = world.resource::<ComponentUniforms<ViscousSettings>>();
        let Some(settings_binding) = settings_uniforms.uniforms().binding() else {
            return Ok(());
        };

        let post_process = view_target.post_process_write();

        let bind_group = render_context.render_device().create_bind_group(
            "viscous_distort_bind_group",
            &pipeline_res.layout,
            &BindGroupEntries::sequential((
                post_process.source,
                &pipeline_res.sampler,
                settings_binding,
            )),
        );

        let mut render_pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("viscous_distort_pass"),
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
pub struct ViscousDistortPipeline {
    layout: BindGroupLayout,
    sampler: Sampler,
    pipeline_id: CachedRenderPipelineId,
}

impl FromWorld for ViscousDistortPipeline {
    fn from_world(world: &mut World) -> Self {
        let render_device = world.resource::<RenderDevice>();

        let layout = render_device.create_bind_group_layout(
            "viscous_distort_bind_group_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    uniform_buffer::<ViscousSettings>(false),
                ),
            ),
        );

        let sampler = render_device.create_sampler(&SamplerDescriptor::default());
        let shader = world.resource::<AssetServer>().load("shaders/viscous_distort.wgsl");

        let pipeline_id = world.resource_mut::<PipelineCache>().queue_render_pipeline(
            RenderPipelineDescriptor {
                label: Some("viscous_distort_pipeline".into()),
                layout: vec![layout.clone()],
                vertex: fullscreen_shader_vertex_state(),
                fragment: Some(FragmentState {
                    shader,
                    shader_defs: vec![],
                    entry_point: "fragment".into(),
                    targets: vec![Some(ColorTargetState {
                        format: TextureFormat::Rgba16Float, 
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
// 2. Logic to Compute Distort Strength
// ----------------------------------------------------------------------------

fn update_viscous_distortion(
    mut query: Query<&mut ViscousSettings>,
    player_query: Query<(&bevy_rapier3d::prelude::Velocity, &crate::components::CompressionState), With<crate::player::Player>>,
    time: Res<Time>,
) {
    let t = time.elapsed_seconds();
    
    let (speed, factor) = if let Ok((vel, state)) = player_query.get_single() {
        (vel.linvel.length(), state.factor_smoothed)
    } else {
        (0.0, 1.0)
    };
    
    let speed_norm = (speed / 15.25).clamp(0.0_f32, 1.0_f32);
    // Depth from normal space (1.0 = baseline)
    let timespace_intensity = (factor - 1.0_f32).abs().clamp(0.0_f32, 1.0_f32);
    
    // Parity with the main.py tone-rate modifiers
    let tone_rate = factor.clamp(0.28_f32, 3.4_f32);
    let roll_time = t; // Python uses roll_time for this
    let sine_cycle = 0.5 + 0.5 * (roll_time * (2.2 + tone_rate * 2.9)).sin();
    let sine_signed = (sine_cycle * 2.0) - 1.0;

    let base_mul = 1.0 + timespace_intensity * 1.25;
    let sine_mul = 1.0 + sine_signed * timespace_intensity * 0.52;
    let dynamic_mul = (base_mul * sine_mul).clamp(0.62_f32, 2.75_f32);
    
    // Original swim_speed = (0.4 + 0.95 * speed_norm) * (1.0 + 0.36 * timespace_intensity + black_hole_intensity * 0.55) * (0.92 + 0.24 * sine_cycle)
    let swim_speed = (0.4 + 0.95 * speed_norm) * (1.0 + 0.36 * timespace_intensity) * (0.92 + 0.24 * sine_cycle);
    
    let pulse = 0.92 + 0.4 * (0.5 + 0.5 * (roll_time * (3.7 + tone_rate * 0.8)).sin());
    
    // Original strength = self.video_distort_strength * (1.05 + 0.95 * speed_norm + black_hole_intensity * 1.1) * pulse * dynamic_mul
    let strength = 0.24 * (1.05 + 0.95 * speed_norm) * pulse * dynamic_mul;
    
    let bloom_strength = 0.0; // Original default is 0.0

    for mut settings in query.iter_mut() {
        settings.time = t;
        settings.speed_norm = swim_speed; // u_speed in shader
        settings.strength = strength;
        settings.bloom_strength = bloom_strength;
        settings.bloom_radius = 0.9;
        settings.bloom_threshold = 0.52;
        settings.quantize_steps = 0.0; // Disabled parity
        settings.outline_strength = 0.0; // User requested removal
    }
}
