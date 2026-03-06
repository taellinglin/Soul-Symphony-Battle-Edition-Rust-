// Ported 1:1 from Panda3D water_surface.frag (Thermal Mode)
#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::apply_pbr_lighting

struct ThermalSettings {
    time: f32,
    uv_scale: f32,
    density_contrast: f32,
    density_gamma: f32,
    thermal_strength: f32,
    compression_factor: f32,
    fog_start: f32,
    fog_end: f32,
}

@group(2) @binding(100) var<uniform> settings: ThermalSettings;

// --- Noise Functions ---
fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash12(i + vec2<f32>(0.0, 0.0));
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(in_p: vec2<f32>) -> f32 {
    var p = in_p;
    var f = 0.0;
    var amp = 0.5;
    let m = mat2x2<f32>(1.6, 1.2, -1.2, 1.6);
    for (var i = 0u; i < 4u; i = i + 1u) {
        f += amp * noise2(p);
        p = m * p;
        amp *= 0.5;
    }
    return f;
}

// --- Colormaps ---
fn roygbiv_thermal(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    var c0: vec3<f32>;
    var c1: vec3<f32>;
    var local: f32;
    let band = 1.0 / 7.0;
    
    if (x < band) {
        c0 = vec3<f32>(1.0, 0.0, 0.0);
        c1 = vec3<f32>(1.0, 0.5, 0.0);
        local = x / band;
    } else if (x < band * 2.0) {
        c0 = vec3<f32>(1.0, 0.5, 0.0);
        c1 = vec3<f32>(1.0, 1.0, 0.0);
        local = (x - band) / band;
    } else if (x < band * 3.0) {
        c0 = vec3<f32>(1.0, 1.0, 0.0);
        c1 = vec3<f32>(0.0, 1.0, 0.0);
        local = (x - band * 2.0) / band;
    } else if (x < band * 4.0) {
        c0 = vec3<f32>(0.0, 1.0, 0.0);
        c1 = vec3<f32>(0.0, 1.0, 1.0);
        local = (x - band * 3.0) / band;
    } else if (x < band * 5.0) {
        c0 = vec3<f32>(0.0, 1.0, 1.0);
        c1 = vec3<f32>(0.0, 0.0, 1.0);
        local = (x - band * 4.0) / band;
    } else if (x < band * 6.0) {
        c0 = vec3<f32>(0.0, 0.0, 1.0);
        c1 = vec3<f32>(0.5, 0.0, 1.0);
        local = (x - band * 5.0) / band;
    } else {
        c0 = vec3<f32>(0.5, 0.0, 1.0);
        c1 = vec3<f32>(1.0, 0.0, 1.0);
        local = (x - band * 6.0) / band;
    }
    return mix(c0, c1, local);
}

fn radar_palette(t: f32) -> vec3<f32> {
    let x = clamp(t, 0.0, 1.0);
    var m = vec3<f32>(1.0);
    if (x < 0.25) { m = vec3<f32>(1.0, 0.2, 0.8); }
    else if (x < 0.5) { m = vec3<f32>(0.8, 1.0, 0.2); }
    else if (x < 0.75) { m = vec3<f32>(0.2, 0.8, 1.0); }
    else { m = vec3<f32>(1.0, 0.6, 0.2); }
    
    let a = vec3<f32>(0.6, 0.7, 0.75);
    let b = vec3<f32>(0.4, 0.3, 0.25);
    let c = vec3<f32>(3.0, 2.0, 1.5);
    let d = vec3<f32>(0.2, 0.5, 0.8);
    let col = a + b * cos(6.28318 * (c * x + d));
    return clamp(m * col, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn compute_level_w_like(p: vec3<f32>) -> f32 {
    let n0 = p.xz * 0.018;
    let n1 = p.xz * 0.043 + vec2<f32>(13.7, -8.9);
    let f0 = fbm(n0);
    let f1 = fbm(n1);
    let n = (f0 * 0.68 + f1 * 0.32);
    let w = (n * 2.0 - 1.0) * 2.25;
    return clamp(w, -2.25, 2.25);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let world_pos = in.world_position.xyz;
    let world_xy = in.world_position.xy;
    
    // Density calculation (Python water_surface.frag main())
    let local_w = compute_level_w_like(world_pos);
    var density = (local_w + 2.25) / 4.5;
    
    let micro = fbm(world_pos.xz * 0.12 + vec2<f32>(5.4, -2.2));
    density = clamp(density + (micro - 0.5) * 0.4, 0.0, 1.0);
    
    density = clamp(density * settings.density_contrast, 0.0, 1.0);
    density = pow(density, settings.density_gamma);
    density = smoothstep(0.0, 1.0, density);
    
    // Base thermal color
    var thermal_col = roygbiv_thermal(density);
    
    // Thermal band layering (Radar field)
    let compression_intensity = clamp((1.0 - settings.compression_factor) / 0.65, 0.0, 1.0);
    let field_a = fbm(world_pos.xz * 0.08 + vec2<f32>(13.2, -7.4));
    let field_b = fbm(world_pos.xz * 0.18 + vec2<f32>(-4.7, 9.1));
    let field_c = fbm(world_pos.xz * 0.35 + vec2<f32>(2.1, -3.6));
    let field = clamp(0.15 + field_a * 0.55 + field_b * 0.28 + field_c * 0.12, 0.0, 1.0);
    let radar_val = clamp(field + compression_intensity * 0.6, 0.0, 1.0);
    
    let band_steps = 9.0;
    let band_pos = radar_val * band_steps;
    let banded = floor(band_pos) / band_steps;
    let band_edge = smoothstep(0.35, 0.6, fract(band_pos));
    let band_val = mix(banded, clamp(banded + 1.0 / band_steps, 0.0, 1.0), band_edge);
    
    let band_col = radar_palette(band_val);
    let contour = smoothstep(0.48, 0.52, fract(band_pos));
    let thermal_band = mix(band_col, vec3<f32>(0.95, 0.98, 1.0), contour * 0.2);
    let thermal_blend = clamp(settings.thermal_strength * 0.6, 0.0, 1.0);
    
    var final_col = mix(thermal_col, thermal_band, thermal_blend);
    
    // Linear Black Fog
    // View Position Z is strictly Negative (since camera looks down -Z). The distance is the magnitude of Z.
    // However Bevy exposes the world position directly in VertexOutput. We calculate distance to camera.
    let dist = distance(view.world_position, world_pos);
    let fog_range = max(0.001, settings.fog_end - settings.fog_start);
    let fog_factor = clamp((settings.fog_end - dist) / fog_range, 0.0, 1.0);
    final_col = mix(vec3<f32>(0.0, 0.0, 0.0), final_col, fog_factor);
    
    pbr_input.material.base_color = vec4<f32>(clamp(final_col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    
    return apply_pbr_lighting(pbr_input);
}
