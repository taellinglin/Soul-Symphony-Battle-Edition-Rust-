#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct CrtSettings {
    intensity: f32,
    aberration_offset: f32,
    time: f32,
    warp_strength: f32,
}
@group(0) @binding(2) var<uniform> settings: CrtSettings;

fn hash21(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2<f32>(123.34, 456.21));
    p += dot(p, p + 34.345);
    return fract(p.x * p.y);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    // 0. VISUAL PARITY: Barrel Distortion (Fisheye)
    let warp_power = 1.15; // Power of fisheye
    let barrel_strength = 0.08 * settings.warp_strength;
    
    var barrel_uv = uv * 2.0 - 1.0;
    let barrel_r2 = dot(barrel_uv, barrel_uv);
    let barrel_f = 1.0 + barrel_r2 * (barrel_strength + barrel_strength * barrel_r2);
    barrel_uv *= barrel_f;
    let fisheye_uv = (barrel_uv + 1.0) * 0.5;

    // 0.1 Viscous Distortion (Bulge/Warp) - SIMPLIFIED for performance
    let t = settings.time;
    let strength = settings.warp_strength;
    
    // var warp = radial1 * bulge1 * 0.022 * strength;
    // warp -= radial2 * bulge2 * 0.018 * strength;
    // warp += vec2<f32>(flow * 0.006, -flow * 0.0055) * strength;
    // warp += vec2<f32>(n, -n) * 0.0022 * strength;
    
    let distorted_uv = clamp(fisheye_uv, vec2<f32>(0.001), vec2<f32>(0.999));

    // 1. Chromatic Aberration
    let r_offset = vec2<f32>(settings.aberration_offset, 0.0);
    let b_offset = vec2<f32>(-settings.aberration_offset, 0.0);
    
    let r = textureSample(screen_texture, texture_sampler, distorted_uv + r_offset).r;
    let g = textureSample(screen_texture, texture_sampler, distorted_uv).g;
    let b = textureSample(screen_texture, texture_sampler, distorted_uv + b_offset).b;
    let a = textureSample(screen_texture, texture_sampler, distorted_uv).a;
    
    let glowBoost = 1.0 + 0.18 * strength;
    let color = vec4<f32>(r * glowBoost, g * glowBoost, b * glowBoost, a);

    // 2. Scanlines
    let scanline = sin(uv.y * 800.0) * 0.04 * settings.intensity;
    
    // 3. Vignette
    let dist_from_center = distance(uv, vec2<f32>(0.5, 0.5));
    let vignette = 1.0 - smoothstep(0.4, 0.9, dist_from_center);
    
    let final_color = color - vec4<f32>(scanline, scanline, scanline, 0.0);
    
    return vec4<f32>(final_color.rgb * vignette, a * vignette);
}
