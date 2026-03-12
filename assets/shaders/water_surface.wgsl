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
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let time = settings.time;
    let v_world_pos = in.world_position.xyz;

    // Standard structural density (clumpy look)
    var local_w = compute_level_w_like(v_world_pos);
    var density = (local_w + 2.25) / 4.5;
    
    // Micro-noise synced with ceiling
    let micro = fbm(v_world_pos.xz * 0.12 + vec2(5.4, -2.2));
    density = clamp(density + (micro - 0.5) * 0.4, 0.0, 1.0);
    
    // Parity: Apply density contrast and gamma before smoothing
    density = clamp(density * settings.density_contrast, 0.0, 1.0);
    density = pow(density, settings.density_gamma);
    density = smoothstep(0.0, 1.0, density);

    var thermal_color = roygbiv_thermal(density);

    // Procedural Ripples (Liquid surface movement)
    let ripple_uv = v_world_pos.xz * 0.12;
    var combined_ripples = 0.0;
    combined_ripples += sin(ripple_uv.x * 3.5 + time * 1.8) * 0.45;
    combined_ripples += sin(ripple_uv.y * 2.8 - time * 1.4) * 0.35;
    combined_ripples += sin((ripple_uv.x + ripple_uv.y) * 4.2 + time * 2.2) * 0.25;
    
    // Final noise for color cycling sync
    let uv = v_world_pos.xz * 0.08; 
    let flow_a = vec2(time * 0.09, -time * 0.05);
    let flow_b = vec2(-time * 0.06, time * 0.07);
    
    let n0 = fbm(uv * 1.15 + flow_a + combined_ripples * 0.05);
    let n1 = fbm(uv * 0.76 - flow_b - combined_ripples * 0.07);
    let combined_noise = (n0 * 0.6 + n1 * 0.4);

    // Base deep water color foundation
    let base_water = vec3(0.06, 0.08, 0.15); // Deep Navy
    
    // Specular highlight boost for ripple visibility
    let spec_highlight = pow(combined_noise, 6.0) * settings.spec_strength;
    
    // Slow HSV hue cycling - speed 0.2
    let hue = fract(time * 0.2 + combined_noise * 0.4);
    let sat = 0.82;
    let val = 0.95;
    let hi = floor(hue * 6.0) % 6.0;
    let f_hue = hue * 6.0 - floor(hue * 6.0);
    let p = val * (1.0 - sat);
    let q = val * (1.0 - sat * f_hue);
    let t_val = val * (1.0 - sat * (1.0 - f_hue));
    var cycle_color: vec3<f32>;
    if (hi < 1.0) {
        cycle_color = vec3(val, t_val, p);
    } else if (hi < 2.0) {
        cycle_color = vec3(q, val, p);
    } else if (hi < 3.0) {
        cycle_color = vec3(p, val, t_val);
    } else if (hi < 4.0) {
        cycle_color = vec3(p, q, val);
    } else if (hi < 5.0) {
        cycle_color = vec3(t_val, p, val);
    } else {
        cycle_color = vec3(val, p, q);
    }

    // Blend thermal ROYGBIV pattern with the slow hue cycle
    let thermal_cycle = mix(cycle_color, thermal_color.rgb, 0.5);
    
    // Use alpha as 'intensity' over the deep navy base
    var final_rgb = mix(base_water, thermal_cycle, settings.alpha);
    final_rgb += spec_highlight;
    
    // Reflection Logic
    let fresnel = pow(1.0 - max(dot(normalize(view.world_position - v_world_pos), vec3(0.0, 1.0, 0.0)), 0.0), 3.0);
    if (settings.reflection_strength > 0.001) {
        let screen_uv = (in.position.xy) / view.viewport.zw;
        let reflection_sample = textureSample(reflection_texture, reflection_sampler, screen_uv).rgb;
        final_rgb = mix(final_rgb, reflection_sample, settings.reflection_strength * (0.3 + fresnel * 0.7));
    }

    // Restore horizon fog (Void parity)
    let dist = distance(view.world_position, v_world_pos);
    let fog_factor = clamp((settings.fog_end - dist) / (settings.fog_end - settings.fog_start), 0.0, 1.0);
    // Fix gray line: use settings.fog_color instead of hardcoded black
    final_rgb = mix(settings.fog_color.rgb, final_rgb, fog_factor);

    return vec4<f32>(final_rgb, 1.0);
}
