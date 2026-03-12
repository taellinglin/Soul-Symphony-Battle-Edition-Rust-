use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

pub type ThermalMaterial = ExtendedMaterial<StandardMaterial, ThermalExtension>;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ThermalExtension {
    #[uniform(100)]
    pub settings: ThermalSettings,
}

#[derive(Clone, ShaderType, Debug)]
pub struct ThermalSettings {
    pub time: f32,
    pub uv_scale: f32,
    pub density_contrast: f32,
    pub density_gamma: f32,
    pub thermal_strength: f32,
    pub compression_factor: f32,
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_color: LinearRgba,
}

impl Default for ThermalSettings {
    fn default() -> Self {
        Self {
            time: 0.0,
            uv_scale: 1.0,
            density_contrast: 1.35,
            density_gamma: 0.85,
            thermal_strength: 1.0,
            compression_factor: 1.0,
            fog_start: 0.0,
            // Original arena parity: black fog, 0..35 range
            fog_end: 35.0,
            fog_color: LinearRgba::BLACK,
        }
    }
}

impl MaterialExtension for ThermalExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/thermal.wgsl".into()
    }
}
