#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::pbr_functions::apply_pbr_lighting
#import bevy_pbr::pbr_types::StandardMaterial
#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_core_pipeline::tonemapping::tone_mapping
#import bevy_pbr::mesh_view_bindings::globals
#import bevy_pbr::mesh_view_bindings::view

struct WaterSurfaceSettings {
    uv_scale: f32,
    alpha: f32,
    rainbow_strength: f32,
    diffusion_strength: f32,
    spec_strength: f32,
    room_tex_strength: f32,
    room_tex_desat: f32,
    thermal_mode: f32,
    thermal_strength: f32,
    compression_factor: f32,
    compression_thermal_strength: f32,
    density_contrast: f32,
    density_gamma: f32,
    player_w: f32,
    corridor_w: f32,
    level_z_step: f32,
    static_uv: f32,
    fog_start: f32,
    fog_end: f32,
    reflection_strength: f32,
    time: f32,
    fog_color: vec4<f32>,
}

@group(2) @binding(100) var<uniform> settings: WaterSurfaceSettings;
@group(2) @binding(101) var room_texture: texture_2d<f32>;
@group(2) @binding(102) var room_sampler: sampler;
@group(2) @binding(103) var reflection_texture: texture_2d<f32>;
@group(2) @binding(104) var reflection_sampler: sampler;

fn hash12(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2(127.1, 311.7));
    return fract(sin(h) * 43758.5453123);
}

fn noise2(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash12(i + vec2(0.0, 0.0));
    let b = hash12(i + vec2(1.0, 0.0));
    let c = hash12(i + vec2(0.0, 1.0));
    let d = hash12(i + vec2(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p_in: vec2<f32>) -> f32 {
    var p = p_in;
    var f = 0.0;
    var amp = 0.5;
    let m = mat2x2<f32>(vec2(1.6, 1.2), vec2(-1.2, 1.6));
    for (var i = 0; i < 4; i++) {
        f += amp * noise2(p);
        p = m * p;
        amp *= 0.5;
    }
    return f;
}

// Compute view-space depth along the camera forward axis (equivalent to -v_eye_z in the original).
fn view_space_depth(world_pos: vec3<f32>) -> f32 {
    // Convert world position into view space using the view_from_world matrix.
    let world_pos4 = vec4<f32>(world_pos, 1.0);
    let view_pos4 = view.view_from_world * world_pos4;
    let view_pos = view_pos4.xyz;
    // In view space, -Z is forward, so depth in front of the camera is -view_pos.z.
    return -view_pos.z;
}

fn compute_level_w_like(p: vec3<f32>) -> f32 {
    let n0 = p.xz * 0.018;
    let n1 = p.xz * 0.043 + vec2(13.7, -8.9);
    let f0 = fbm(n0);
    let f1 = fbm(n1);
    let n = (f0 * 0.68 + f1 * 0.32);
    let w = (n * 2.0 - 1.0) * 2.25;
    return clamp(w, -2.25, 2.25);
}

fn roygbiv_thermal(t_in: f32) -> vec3<f32> {
    let x = clamp(t_in, 0.0, 1.0);
    var c0: vec3<f32>;
    var c1: vec3<f32>;
    var local: f32;
    let band = 1.0 / 7.0;
    
    if (x < band) {
        c0 = vec3(1.0, 0.0, 0.0);
        c1 = vec3(1.0, 0.5, 0.0);
        local = x / band;
    } else if (x < band * 2.0) {
        c0 = vec3(1.0, 0.5, 0.0);
        c1 = vec3(1.0, 1.0, 0.0);
        local = (x - band) / band;
    } else if (x < band * 3.0) {
        c0 = vec3(1.0, 1.0, 0.0);
        c1 = vec3(0.0, 1.0, 0.0);
        local = (x - band * 2.0) / band;
    } else if (x < band * 4.0) {
        c0 = vec3(0.0, 1.0, 0.0);
        c1 = vec3(0.0, 0.0, 1.0);
        local = (x - band * 3.0) / band;
    } else if (x < band * 5.0) {
        c0 = vec3(0.0, 0.0, 1.0);
        c1 = vec3(0.29, 0.0, 0.51);
        local = (x - band * 4.0) / band;
    } else if (x < band * 6.0) {
        c0 = vec3(0.29, 0.0, 0.51);
        c1 = vec3(0.56, 0.0, 1.0);
        local = (x - band * 5.0) / band;
    } else {
        c0 = vec3(0.56, 0.0, 1.0);
        c1 = vec3(0.85, 0.45, 1.0);
        local = (x - band * 6.0) / band;
    }
    local = smoothstep(0.0, 1.0, local);
    return mix(c0, c1, local);
}

fn hue_shift(c: vec3<f32>, a: f32) -> vec3<f32> {
    let s = sin(a);
    let co = cos(a);
    let m = mat3x3<f32>(
        vec3(0.299 + 0.701 * co + 0.168 * s, 0.587 - 0.587 * co + 0.330 * s, 0.114 - 0.114 * co - 0.497 * s),
        vec3(0.299 - 0.299 * co - 0.328 * s, 0.587 + 0.413 * co + 0.035 * s, 0.114 - 0.114 * co + 0.292 * s),
        vec3(0.299 - 0.300 * co + 1.250 * s, 0.587 - 0.588 * co - 1.050 * s, 0.114 + 0.886 * co - 0.203 * s)
    );
    return clamp(m * c, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn radar_palette(t_in: f32) -> vec3<f32> {
    let x = clamp(t_in, 0.0, 1.0);
    var c0: vec3<f32>;
    var c1: vec3<f32>;
    var local: f32;
    if (x < 0.25) {
        c0 = vec3(0.05, 0.08, 0.2);
        c1 = vec3(0.05, 0.25, 0.6);
        local = x / 0.25;
    } else if (x < 0.55) {
        c0 = vec3(0.05, 0.25, 0.6);
        c1 = vec3(0.0, 0.65, 0.35);
        local = (x - 0.25) / 0.3;
    } else if (x < 0.8) {
        c0 = vec3(0.0, 0.65, 0.35);
        c1 = vec3(0.85, 0.85, 0.2);
        local = (x - 0.55) / 0.25;
    } else {
        c0 = vec3(0.85, 0.85, 0.2);
        c1 = vec3(0.95, 0.2, 0.2);
        local = (x - 0.8) / 0.2;
    }
    local = smoothstep(0.0, 1.0, local);
    return mix(c0, c1, local);
}

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    var _pbr_input = pbr_input_from_standard_material(in, is_front);
    let time = settings.time;
    let world_pos = in.world_position.xyz;
    let world_xz = world_pos.xz;

    // Structural density field (thermal bands) with optional static-UV camo pass
    var local_w: f32;
    var density: f32;
    if (settings.static_uv > 0.5) {
        let static_xz = world_xz * 6.0;
        let static_pos = vec3(static_xz.x, world_pos.y, static_xz.y);
        local_w = compute_level_w_like(static_pos);
        density = (local_w + 2.25) / 4.5;
        let micro = fbm(static_pos.xz * 0.35 + vec2(9.1, -4.3));
        density = clamp(density * 1.25 + (micro - 0.5) * 0.35 + 0.05, 0.0, 1.0);
    } else {
        local_w = compute_level_w_like(world_pos);
        density = (local_w + 2.25) / 4.5;
        let micro = fbm(world_xz * 0.12 + vec2(5.4, -2.2));
        density = clamp(density + (micro - 0.5) * 0.4, 0.0, 1.0);
    }

    density = clamp(density * settings.density_contrast, 0.0, 1.0);
    density = pow(density, settings.density_gamma);
    density = smoothstep(0.0, 1.0, density);

    var thermal_col = roygbiv_thermal(density);

    // Optional rainbow-strength blend (arena uses 0.0, so effectively off)
    let cycle_mix = clamp(settings.rainbow_strength, 0.0, 1.0);
    if (cycle_mix > 0.001) {
        let cycle_col = roygbiv_thermal(density);
        thermal_col = mix(thermal_col, cycle_col, cycle_mix);
    }

    // Water lighting model (specular highlights + sparkles)
    let uv = world_xz * max(0.02, settings.uv_scale * 0.08);
    let flow_a = vec2(time * 0.09, -time * 0.05);
    let flow_b = vec2(-time * 0.06, time * 0.07);

    let n0 = fbm(uv * 1.25 + flow_a);
    let n1 = fbm(uv * 2.35 + flow_b + vec2(11.3, -4.7));
    let h = n0 * 0.62 + n1 * 0.38;

    let eps = 0.06;
    let hx = fbm((uv + vec2(eps, 0.0)) * 1.25 + flow_a) * 0.62
           + fbm((uv + vec2(eps, 0.0)) * 2.35 + flow_b + vec2(11.3, -4.7)) * 0.38;
    let hy = fbm((uv + vec2(0.0, eps)) * 1.25 + flow_a) * 0.62
           + fbm((uv + vec2(0.0, eps)) * 2.35 + flow_b + vec2(11.3, -4.7)) * 0.38;

    let normal = normalize(vec3((hx - h) * 6.8, (hy - h) * 6.8, 1.0));
    let light_dir = normalize(vec3(0.35, 0.28, 0.89));
    let view_dir = vec3(0.0, 0.0, 1.0);
    let half_vec = normalize(light_dir + view_dir);

    let ndotl = max(dot(normal, light_dir), 0.0);
    let ndotv = max(dot(normal, view_dir), 0.0);
    let spec = pow(max(dot(normal, half_vec), 0.0), 84.0);

    let sparkle_noise = fbm(uv * 9.4 + vec2(time * 0.34, -time * 0.27));
    let sparkle = smoothstep(0.78, 0.95, sparkle_noise) * smoothstep(0.45, 1.0, spec);

    let spec_strength = max(0.2, settings.spec_strength);
    let base = vec3(0.05, 0.08, 0.11) + vec3(0.10, 0.14, 0.18) * ndotl;
    let ndotv_fresnel = pow(1.0 - ndotv, 3.0);
    let highlights = vec3(1.0) * (spec * (0.7 + spec_strength * 0.9) + sparkle * (0.22 + spec_strength * 0.55));

    var water_col = clamp(base + highlights, vec3(0.0), vec3(1.0));
    if (settings.spec_strength <= 0.01
        && settings.room_tex_strength <= 0.01
        && settings.diffusion_strength <= 0.01
        && settings.rainbow_strength <= 0.01) {
        water_col = vec3(0.0);
    }

    // Optional underlying room texture (disabled in arena parity: strength = 0.0)
    var room_desat = vec3(0.0);
    if (settings.room_tex_strength > 0.01) {
        let room_tex = textureSample(room_texture, room_sampler, in.uv).rgb;
        let room_luma = dot(room_tex, vec3(0.299, 0.587, 0.114));
        room_desat = mix(room_tex, vec3(room_luma), clamp(settings.room_tex_desat, 0.0, 1.0));
    }

    let thermal_mix = clamp(settings.thermal_mode, 0.0, 1.0) * clamp(settings.thermal_strength * 0.6, 0.0, 1.0);
    let thermal_only =
        (settings.thermal_mode > 0.5
         && settings.thermal_strength > 0.01
         && settings.room_tex_strength <= 0.01
         && settings.spec_strength <= 0.01
         && settings.diffusion_strength <= 0.01
         && settings.rainbow_strength <= 0.01);

    var final_rgb: vec3<f32>;
    if (thermal_only) {
        final_rgb = thermal_col;
    } else {
        final_rgb = mix(water_col, thermal_col, thermal_mix);
    }

    // Compression thermal overlay
    let compression_intensity = clamp((1.0 - settings.compression_factor) / 0.65, 0.0, 1.0);
    let compression_col = roygbiv_thermal(compression_intensity);
    let compression_mix = clamp(settings.compression_thermal_strength, 0.0, 1.0);
    final_rgb = mix(final_rgb, compression_col, compression_mix);

    if (settings.thermal_mode > 0.5) {
        let field_a = fbm(world_xz * 0.08 + vec2(13.2, -7.4) + vec2(time * 0.01, -time * 0.013));
        let field_b = fbm(world_xz * 0.18 + vec2(-4.7, 9.1) + vec2(-time * 0.012, time * 0.009));
        let field_c = fbm(world_xz * 0.35 + vec2(2.1, -3.6) + vec2(time * 0.008, time * 0.007));
        let field = clamp(0.15 + field_a * 0.55 + field_b * 0.28 + field_c * 0.12, 0.0, 1.0);
        let radar_val = clamp(field + compression_intensity * 0.6, 0.0, 1.0);
        let band_steps = 9.0;
        let band_pos = radar_val * band_steps;
        let banded = floor(band_pos) / band_steps;
        let band_edge = smoothstep(0.35, 0.6, fract(band_pos));
        let band_val = mix(banded, clamp(banded + 1.0 / band_steps, 0.0, 1.0), band_edge);
        let band_col = radar_palette(band_val);
        let contour = smoothstep(0.48, 0.52, fract(band_pos));
        let thermal_band = mix(band_col, vec3(0.95, 0.98, 1.0), contour * 0.2);
        let thermal_blend = clamp(settings.thermal_strength * 0.6, 0.0, 1.0);
        final_rgb = mix(final_rgb, thermal_band, thermal_blend);
    }

    if (!thermal_only && settings.room_tex_strength > 0.01) {
        final_rgb = clamp(final_rgb + room_desat * clamp(settings.room_tex_strength, 0.0, 1.0), vec3(0.0), vec3(1.0));
    }
    final_rgb = clamp(final_rgb, vec3(0.0), vec3(1.0));

    // Reflection (uses offscreen inverted-echo texture when enabled)
    if (settings.reflection_strength > 0.001) {
        let view_vec = normalize(view.world_position - world_pos);
        let fres = pow(1.0 - max(dot(view_vec, vec3(0.0, 1.0, 0.0)), 0.0), 3.0);
        let screen_uv = (in.position.xy) / view.viewport.zw;
        let reflection_sample = textureSample(reflection_texture, reflection_sampler, screen_uv).rgb;
        final_rgb = mix(final_rgb, reflection_sample, settings.reflection_strength * (0.3 + fres * 0.7));
    }

    // Fog with shared 0..35 range and supplied fog color.
    // Match original GLSL, which uses view-space Z (v_eye_z), so the visible floor/ceiling band
    // stays visually parallel instead of “curving” around the camera.
    let eye_depth = view_space_depth(world_pos);
    let fog_range = max(0.001, settings.fog_end - settings.fog_start);
    let fog_factor = clamp((settings.fog_end - eye_depth) / fog_range, 0.0, 1.0);
    final_rgb = mix(settings.fog_color.rgb, final_rgb, fog_factor);

    var out_alpha = clamp(settings.alpha, 0.0, 1.0);
    if (settings.thermal_mode > 0.5) {
        out_alpha = clamp(out_alpha + 0.18, 0.0, 1.0);
    }

    return vec4<f32>(final_rgb, out_alpha);
}
