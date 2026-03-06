use bevy::prelude::*;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::render::render_resource::{AsBindGroup, ShaderRef, ShaderType};

pub type ThermalMaterial = ExtendedMaterial<StandardMaterial, ThermalExtension>;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct ThermalExtension {
    #[uniform(100)]
    pub settings: ThermalSettings,
}

#[derive(Clone, Default, ShaderType, Debug)]
pub struct ThermalSettings {
    pub time: f32,
    pub uv_scale: f32,
    pub density_contrast: f32,
    pub density_gamma: f32,
    pub thermal_strength: f32,
    pub compression_factor: f32,
    pub fog_start: f32,
    pub fog_end: f32,
}

impl MaterialExtension for ThermalExtension {
    fn fragment_shader() -> ShaderRef {
        "shaders/thermal.wgsl".into()
    }
}
