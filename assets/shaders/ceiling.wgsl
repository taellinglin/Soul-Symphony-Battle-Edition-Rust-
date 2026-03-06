#import bevy_pbr::mesh_view_bindings::view
#import bevy_pbr::forward_io::VertexOutput

struct CeilingSettings {
    time: f32,
    player_w: f32,
    base_color: vec4<f32>,
}

@group(2) @binding(100) var<uniform> settings: CeilingSettings;

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
    for (var i = 0u; i < 6u; i = i + 1u) { // Increased to 6 octaves for "more waves"
        f += amp * noise2(p);
        p = m * p;
        amp *= 0.5;
    }
    return f;
}

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

@fragment
fn fragment(
    in: VertexOutput,
) -> @location(0) vec4<f32> {
    let world_pos = in.world_position.xz;
    let t = settings.time * 0.12;
    
    // Frequencies synced with floor compute_level_w_like for mirrored parity
    let n0 = world_pos * 0.018 + vec2<f32>(t * 0.5, t * 0.2);
    let n1 = world_pos * 0.043 - vec2<f32>(t * 0.3, t * 0.7) + vec2<f32>(13.7, -8.9);
    
    let f0 = fbm(n0);
    let f1 = fbm(n1);
    let density = clamp((f0 * 0.65 + f1 * 0.35) * 1.25, 0.0, 1.0);
    
    var color = roygbiv_thermal(density);
    
    // Dim it slightly to not overpower the ground but keep the mirror feel
    color *= 0.42; 
    
    // Background tint
    color = mix(settings.base_color.rgb, color, 0.85);
    
    // Linear Black Fog
    // Bevy fog starts at 20.0, ends at 150.0 in main.rs. We mirror it here.
    let dist = distance(view.world_position, in.world_position.xyz);
    let fog_start = 20.0;
    let fog_end = 150.0;
    let fog_factor = clamp((fog_end - dist) / (fog_end - fog_start), 0.0, 1.0);
    let fogged_color = mix(vec3<f32>(0.0, 0.0, 0.0), color.rgb, fog_factor);
    
    return vec4<f32>(fogged_color, 1.0);
}
