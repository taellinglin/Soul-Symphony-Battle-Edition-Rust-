#![allow(dead_code)]
pub mod thermal;
pub mod particles;
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderRef, ShaderType},
};

#[derive(Resource)]
pub struct ReflectionTexture(pub Handle<Image>);

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            MaterialPlugin::<HyperSliceMaterial>::default(),
            MaterialPlugin::<FloorWetMaterial>::default(),
            MaterialPlugin::<WaterSurfaceMaterial>::default(),
            MaterialPlugin::<CeilingMaterial>::default(),
            MaterialPlugin::<BallMaterial>::default(),
            MaterialPlugin::<BossHyperMaterial>::default(),
            MaterialPlugin::<thermal::ThermalMaterial>::default(),
            particles::AtmosphericParticlesPlugin,
        ));
        app.add_systems(Update, (
            sync_material_uniforms,
        ).run_if(in_state(crate::systems::progression::GameState::Playing)));
    }
}

pub fn update_floor_wetness(
    player_query: Query<(&Transform, &bevy_rapier3d::prelude::Velocity), With<crate::player::Player>>,
    monster_query: Query<(&Transform, &bevy_rapier3d::prelude::Velocity), With<crate::ai::Monster>>,
    mut floor_materials: ResMut<Assets<FloorWetMaterial>>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    let t = time.elapsed_seconds();

    if let Ok((player_tf, player_vel)) = player_query.get_single() {
        let pos = player_tf.translation;
        let speed = player_vel.linvel.length();

        let ground_y = crate::player::BALL_RADIUS; 
        let is_grounded = pos.y < ground_y + 0.15; 

        for (_, material) in floor_materials.iter_mut() {
            let settings = &mut material.extension.settings;
            settings.time = t;
            
            if is_grounded {
                let target_uv = Vec2::new(pos.x, pos.z);
                if settings.wake_strength < 0.02 {
                    settings.contact_uv = target_uv;
                }
                let follow = (dt * 18.0).min(1.0);
                settings.contact_uv += (target_uv - settings.contact_uv) * follow;
                
                let speed_norm = (speed / 15.25).min(1.0);
                let strength_inc = (0.22 + speed_norm * 0.78) * dt * 7.0;
                settings.wake_strength = (settings.wake_strength + strength_inc).min(1.0);
            } else {
                settings.wake_strength = (settings.wake_strength - dt * 3.6).max(0.0);
            }

            // --- Monster Contact Pulses ---
            for (m_tf, m_vel) in monster_query.iter() {
                let m_pos = m_tf.translation;
                let m_speed = m_vel.linvel.length();
                // Monsters also splash if low enough
                if m_pos.y < 2.0 && m_speed > 1.0 {
                    // Find an empty pulse slot or oldest pulse
                    let mut best_idx = 0;
                    let mut min_strength = 1.0;
                    for i in 0..8 {
                        if settings.pulses[i].w < min_strength {
                            min_strength = settings.pulses[i].w;
                            best_idx = i;
                        }
                    }
                    if min_strength < 0.02 {
                        let m_uv = Vec2::new(m_pos.x, m_pos.z);
                        settings.pulses[best_idx] = Vec4::new(m_uv.x, m_uv.y, t, 0.45);
                    }
                }
            }

            // Decay extra pulses
            for i in 0..8 {
                let strength = settings.pulses[i].w;
                if strength > 0.0 {
                    settings.pulses[i].w = (strength - dt * 1.2).max(0.0);
                }
            }
        }
    }
}

pub fn sync_material_uniforms(
    player_query: Query<&crate::components::Spatial4D, With<crate::player::Player>>,
    mut hyper_slice_materials: ResMut<Assets<HyperSliceMaterial>>,
    mut floor_materials: ResMut<Assets<crate::rendering::thermal::ThermalMaterial>>,
    mut water_materials: ResMut<Assets<WaterSurfaceMaterial>>,
    mut ceiling_materials: ResMut<Assets<CeilingMaterial>>,
    mut ball_materials: ResMut<Assets<BallMaterial>>,
    mut boss_materials: ResMut<Assets<BossHyperMaterial>>,
    time: Res<Time>,
    // Per-entity W-layer syncing
    spatial_query: Query<&crate::components::Spatial4D>,
    material_query: Query<(Entity, &Handle<HyperSliceMaterial>, Option<&Parent>)>,
) {
    if let Ok(player_spatial) = player_query.get_single() {
        let t = time.elapsed_seconds();
        let w = player_spatial.w;

        // 1. Sync per-entity object_w (Monsters, Projectiles, etc.)
        for (entity, handle, parent) in material_query.iter() {
            if let Some(mat) = hyper_slice_materials.get_mut(handle) {
                // Try to get Spatial4D from the entity itself or its parent
                let spatial = spatial_query.get(entity).ok()
                    .or_else(|| parent.and_then(|p| spatial_query.get(p.get()).ok()));
                
                if let Some(s) = spatial {
                    mat.extension.settings.object_w = s.w;
                }
            }
        }

        // 2. Sync global/batch material uniforms
        for (_, mat) in hyper_slice_materials.iter_mut() {
            mat.extension.settings.player_w = w;
            mat.extension.settings.time = t;
            mat.extension.settings.hyper_slice = 2.45;
            mat.extension.settings.hyper_falloff = 1.95;
            // NOTE: thickness is 1.0 by default in the struct, no need to overwrite here 
            // unless we want it dynamic.
        }
        for (_, mat) in floor_materials.iter_mut() {
            mat.extension.settings.time = t;
        }
        for (_, mat) in water_materials.iter_mut() {
            mat.extension.settings.player_w = w;
            mat.extension.settings.time = t;
        }
        for (_, mat) in ceiling_materials.iter_mut() {
            mat.extension.settings.player_w = w;
            mat.extension.settings.time = t;
        }
        for (_, mat) in ball_materials.iter_mut() {
            mat.extension.settings.player_w = w;
            mat.extension.settings.hyper_slice = 2.45;
            mat.extension.settings.hyper_falloff = 1.95;
            mat.extension.settings.object_w = w; // Keep player ball visible in its own slice
            mat.extension.settings.time = t;
        }
        for (_, mat) in boss_materials.iter_mut() {
            mat.extension.settings.hyper_w = w;
        }
    }
}

pub type HyperSliceMaterial = ExtendedMaterial<StandardMaterial, HyperSliceExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct HyperSliceExtension {
    #[uniform(100)]
    pub settings: HyperSliceSettings,
    #[texture(101)]
    #[sampler(102)]
    pub base_texture: Option<Handle<Image>>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct HyperSliceSettings {
    pub player_w: f32,
    pub object_w: f32,
    pub thickness: f32,
    pub hyper_slice: f32,
    pub hyper_falloff: f32,
    pub room_uv_scale: f32, // Byte 12
    pub time: f32,          // Byte 16
    pub persistence: f32,   // Byte 20
    pub fog_start: f32,     // Byte 24
    pub fog_end: f32,       // Byte 28
    pub edge_color: LinearRgba, // Byte 32 (Offset 32 is aligned to 16)
    pub fog_color: LinearRgba,  // Byte 48 (Offset 48 is aligned to 16)
}

impl Default for HyperSliceSettings {
    fn default() -> Self {
        Self {
            player_w: 0.0,
            object_w: 0.0,
            thickness: 1.0,
            hyper_slice: 2.45,
            hyper_falloff: 1.95,
            room_uv_scale: 0.32,
            time: 0.0,
            persistence: 0.0,
            fog_start: 0.8,
            fog_end: 18.0, // Match original Python parity
            edge_color: LinearRgba::new(0.0, 1.0, 1.0, 1.0),
            fog_color: LinearRgba::new(0.1, 0.12, 0.17, 1.0),
        }
    }
}

impl MaterialExtension for HyperSliceExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/hyper_slice.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/hyper_slice.wgsl".into()
    }
}

pub type FloorWetMaterial = ExtendedMaterial<StandardMaterial, FloorWetExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct FloorWetExtension {
    #[uniform(100)]
    pub settings: FloorWetSettings,
    #[texture(101)]
    #[sampler(102)]
    pub base_texture: Option<Handle<Image>>,
}

#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct FloorWetSettings {
    pub room_uv_scale: f32,
    pub wake_strength: f32,
    pub pulse_count: u32,
    pub player_w: f32,   // Byte 12
    pub object_w: f32,   // Byte 16
    pub thickness: f32,  // Byte 20
    pub time: f32,       // Byte 24
    pub pad0: f32,       // Byte 28 -> padding for contact_uv
    pub contact_uv: Vec2, // Byte 32 (Offset 32 is aligned to 8)
    pub pad1: Vec2,      // Byte 40-47 -> padding for edge_color
    pub edge_color: LinearRgba, // Byte 48 (Offset 48 is aligned to 16)
    pub pulses: [Vec4; 8], // Byte 64 (Offset 64 is aligned to 16)
}

impl Default for FloorWetSettings {
    fn default() -> Self {
        Self {
            room_uv_scale: 0.32,
            wake_strength: 0.0,
            pulse_count: 0,
            player_w: 0.0,
            object_w: 0.0,
            thickness: 0.15,
            time: 0.0,
            pad0: 0.0,
            contact_uv: Vec2::ZERO,
            pad1: Vec2::ZERO,
            edge_color: LinearRgba::new(0.2, 0.9, 1.0, 1.0),
            pulses: [Vec4::ZERO; 8],
        }
    }
}

impl MaterialExtension for FloorWetExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/floor_wet.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/floor_wet.wgsl".into()
    }
}

pub type WaterSurfaceMaterial = ExtendedMaterial<StandardMaterial, WaterSurfaceExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct WaterSurfaceExtension {
    #[uniform(100)]
    pub settings: WaterSurfaceSettings,
    #[texture(101)]
    #[sampler(102)]
    pub room_texture: Option<Handle<Image>>,
    #[texture(103)]
    #[sampler(104)]
    pub reflection_texture: Option<Handle<Image>>,
}

#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct WaterSurfaceSettings {
    pub uv_scale: f32,
    pub alpha: f32,
    pub rainbow_strength: f32,
    pub diffusion_strength: f32,
    
    pub spec_strength: f32,
    pub room_tex_strength: f32,
    pub room_tex_desat: f32,
    pub thermal_mode: f32,
    
    pub thermal_strength: f32,
    pub compression_factor: f32,
    pub compression_thermal_strength: f32,
    pub density_contrast: f32,
    
    pub density_gamma: f32,
    pub player_w: f32,
    pub corridor_w: f32,
    pub level_z_step: f32,
    
    pub static_uv: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub reflection_strength: f32,
    pub time: f32,
    pub fog_color: LinearRgba,
}

impl Default for WaterSurfaceSettings {
    fn default() -> Self {
        Self {
            uv_scale: 1.0,
            alpha: 1.0, // Sync opacity with ceiling for matching vibrancy
            rainbow_strength: 1.0, // water_thermal_cycle_strength
            diffusion_strength: 0.18, // water_thermal_cycle_speed
            spec_strength: 0.72, // water_specular_strength
            room_tex_strength: 0.32, // water_room_tex_strength
            room_tex_desat: 0.85, // water_room_tex_desat
            thermal_mode: 1.0, // water_density_thermal_mode
            thermal_strength: 0.92, // water_density_thermal_strength
            compression_factor: 1.0,
            compression_thermal_strength: 0.85, // water_compression_thermal_strength
            density_contrast: 1.35, // water_density_contrast
            density_gamma: 0.85, // water_density_gamma
            player_w: 0.0,
            corridor_w: 2.45,   // Python parity: u_corridor_w
            level_z_step: 6.0,
            static_uv: 1.0, // Python: water_static_uv = True
            fog_start: 0.8, // Python parity: u_fog_start
            fog_end: 18.0,  // Python parity: u_fog_end
            reflection_strength: 0.0, // Python parity: disabled for clean ocean feel
            time: 0.0,
            fog_color: LinearRgba::new(0.1, 0.12, 0.17, 1.0),
        }
    }
}

impl MaterialExtension for WaterSurfaceExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/water_surface.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/water_surface.wgsl".into()
    }
}

pub type CeilingMaterial = ExtendedMaterial<StandardMaterial, CeilingExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct CeilingExtension {
    #[uniform(100)]
    pub settings: CeilingSettings,
}

#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct CeilingSettings {
    pub time: f32,
    pub player_w: f32,
    pub base_color: LinearRgba,
}

impl Default for CeilingSettings {
    fn default() -> Self {
        Self {
            time: 0.0,
            player_w: 0.0,
            base_color: LinearRgba::new(0.19, 0.22, 0.28, 1.0), // Python parity: same as floor
        }
    }
}

impl MaterialExtension for CeilingExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/ceiling.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/ceiling.wgsl".into()
    }
}

pub type BallMaterial = ExtendedMaterial<StandardMaterial, BallExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BallExtension {
    #[uniform(100)]
    pub settings: BallSettings,
}

#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct BallSettings {
    pub time: f32,
    pub hue_shift: f32, // Global color cycle
    pub player_w: f32,
    pub object_w: f32,
    pub thickness: f32,
    pub hyper_slice: f32,
    pub hyper_falloff: f32,
    pub pad: f32,
    pub edge_color: LinearRgba,
    pub layer0_scroll: Vec2,
    pub layer1_scroll: Vec2,
    pub layer2_scroll: Vec2,
    pub layer0_pulse: Vec4, // speed, phase, min, max
    pub layer1_pulse: Vec4,
    pub layer2_pulse: Vec4,
}

impl Default for BallSettings {
    fn default() -> Self {
        Self {
            time: 0.0,
            hue_shift: 0.0,
            player_w: 0.0,
            object_w: 0.0,
            thickness: 1.0,
            hyper_slice: 2.45,
            hyper_falloff: 1.95,
            pad: 0.0,
            edge_color: LinearRgba::new(0.2, 0.9, 1.0, 1.0),
            layer0_scroll: Vec2::new(0.018, 0.013),
            layer1_scroll: Vec2::new(-0.022, 0.015),
            layer2_scroll: Vec2::new(0.01, -0.018),
            layer0_pulse: Vec4::new(1.2, 0.0, 0.5, 1.0),
            layer1_pulse: Vec4::new(0.8, 1.0, 0.4, 0.9),
            layer2_pulse: Vec4::new(1.5, 2.0, 0.3, 0.8),
        }
    }
}

impl MaterialExtension for BallExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/ball.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/ball.wgsl".into()
    }
}

pub type BossHyperMaterial = ExtendedMaterial<StandardMaterial, BossHyperExtension>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct BossHyperExtension {
    #[uniform(100)]
    pub settings: BossHyperSettings,
}

#[derive(Clone, Copy, ShaderType, Debug, Reflect)]
pub struct BossHyperSettings {
    pub intensity: f32,
    pub variant: f32,
    pub hyper_w: f32,
    pub pad: f32,
}

impl Default for BossHyperSettings {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            variant: 0.0,
            hyper_w: 0.0,
            pad: 0.0,
        }
    }
}

impl MaterialExtension for BossHyperExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/boss_hyper.wgsl".into()
    }

    fn deferred_fragment_shader() -> ShaderRef {
        "shaders/boss_hyper.wgsl".into()
    }
}
