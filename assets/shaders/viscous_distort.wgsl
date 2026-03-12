#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct ViscousSettings {
    time: f32,
    speed_norm: f32,
    strength: f32,
    bloom_strength: f32,
    bloom_radius: f32,
    bloom_threshold: f32,
    quantize_steps: f32,
    outline_strength: f32,
}
@group(0) @binding(2) var<uniform> settings: ViscousSettings;

fn hash21(p_in: vec2<f32>) -> f32 {
    var p = fract(p_in * vec2<f32>(123.34, 456.21));
    p += dot(p, p + 34.345);
    return fract(p.x * p.y);
}

@fragment
fn fragment(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = settings.time;
    let spd = clamp(settings.speed_norm, 0.0, 1.0);
    let strength = max(0.0, settings.strength) * (0.45 + 0.95 * spd);

    let c1 = vec2<f32>(0.35 + 0.12 * sin(t * 0.67), 0.56 + 0.10 * cos(t * 0.51));
    let c2 = vec2<f32>(0.66 + 0.09 * cos(t * 0.83), 0.38 + 0.11 * sin(t * 0.59));

    let p1 = uv - c1;
    let p2 = uv - c2;
    let r1 = length(p1) + 1e-5;
    let r2 = length(p2) + 1e-5;

    let bulge1 = exp(-r1 * 8.0) * sin(t * 2.7 - r1 * 34.0);
    let bulge2 = exp(-r2 * 9.2) * cos(t * 2.1 - r2 * 30.0);

    let radial1 = p1 / r1;
    let radial2 = p2 / r2;

    var flow = sin((uv.x * 11.0 + uv.y * 14.0) + t * 1.8)
             + cos((uv.x * 17.0 - uv.y * 9.0) - t * 1.35);
    flow *= 0.5;

    let n = hash21(uv * 42.0 + t * 0.08) - 0.5;

    // --- Exact Visual Parity: Radial Bulge + Flow + Noise Warp ---
    var warp = radial1 * bulge1 * 0.036 * strength;
    warp -= radial2 * bulge2 * 0.03 * strength;
    warp += vec2<f32>(flow * 0.01, -flow * 0.009) * strength;
    warp += vec2<f32>(n, -n) * 0.0032 * strength;

    // --- Visual Parity: Chromatic Aberration (Blurry look) ---
    let uv2 = clamp(uv + warp, vec2<f32>(0.001), vec2<f32>(0.999));
    let col = textureSample(screen_texture, texture_sampler, uv2);

    // --- Post-Warp Pipeline ---
    let px = vec2<f32>(1.0) / vec2<f32>(textureDimensions(screen_texture, 0));
    
    // 1. Toon Outlines (Color-based Edge Detection)
    // Original separation=1.2 matches this logic
    var edge = 0.0;
    if (settings.outline_strength > 0.0) {
        let sample_center = col.rgb;
        let sample_up    = textureSample(screen_texture, texture_sampler, clamp(uv2 + vec2<f32>(0.0, px.y), vec2<f32>(0.001), vec2<f32>(0.999))).rgb;
        let sample_down  = textureSample(screen_texture, texture_sampler, clamp(uv2 - vec2<f32>(0.0, px.y), vec2<f32>(0.001), vec2<f32>(0.999))).rgb;
        let sample_left  = textureSample(screen_texture, texture_sampler, clamp(uv2 - vec2<f32>(px.x, 0.0), vec2<f32>(0.001), vec2<f32>(0.999))).rgb;
        let sample_right = textureSample(screen_texture, texture_sampler, clamp(uv2 + vec2<f32>(px.x, 0.0), vec2<f32>(0.001), vec2<f32>(0.999))).rgb;
        
        let diff = abs(sample_center - sample_up) + abs(sample_center - sample_down) + abs(sample_center - sample_left) + abs(sample_center - sample_right);
        edge = clamp(dot(diff, vec3<f32>(1.0)) * settings.outline_strength, 0.0, 1.0);
    }

    // 2. Gaussian Bloom Pass
    let bloom_radius = max(0.3, settings.bloom_radius) * (0.65 + 0.8 * spd);
    var bloom_acc = vec3<f32>(0.0);
    var bloom_wsum = 0.0;
    
    for (var ix: i32 = -2; ix <= 2; ix++) {
        for (var iy: i32 = -2; iy <= 2; iy++) {
            let tap_off = vec2<f32>(f32(ix), f32(iy)) * px * bloom_radius;
            let tap_uv = clamp(uv2 + tap_off, vec2<f32>(0.001), vec2<f32>(0.999));
            let tap = textureSample(screen_texture, texture_sampler, tap_uv).rgb;
            
            let luma = dot(tap, vec3<f32>(0.2126, 0.7152, 0.0722));
            let bright = smoothstep(settings.bloom_threshold, 1.0, luma);
            let w = exp(-f32(ix * ix + iy * iy) * 0.42) * bright;
            
            bloom_acc += tap * w;
            bloom_wsum += w;
        }
    }

    var bloom = vec3<f32>(0.0);
    if (bloom_wsum > 1e-4) {
        bloom = bloom_acc / bloom_wsum;
    }
    
    let bloom_mix = max(0.0, settings.bloom_strength) * (0.7 + 0.6 * strength);
    var final_rgb = col.rgb * (1.0 + 0.16 * strength);
    final_rgb += bloom * bloom_mix;
    
    // Apply Outline Darkening (Optional, if outline_strength > 0)
    final_rgb *= (1.0 - edge * 0.5); 
    
    return vec4<f32>(clamp(final_rgb, vec3<f32>(0.0), vec3<f32>(1.0)), col.a);
}
