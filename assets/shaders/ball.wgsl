#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::apply_pbr_lighting
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::view

fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453123);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash21(i + vec2<f32>(0.0, 0.0)), hash21(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash21(i + vec2<f32>(0.0, 1.0)), hash21(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

struct BallSettings {
    time: f32,
    hue_shift: f32,
    player_w: f32,
    object_w: f32,
    thickness: f32,
    hyper_slice: f32,
    hyper_falloff: f32,
    pad: f32,
    edge_color: vec4<f32>,
    layer0_scroll: vec2<f32>,
    layer1_scroll: vec2<f32>,
    layer2_scroll: vec2<f32>,
    layer0_pulse: vec4<f32>,
    layer1_pulse: vec4<f32>,
    layer2_pulse: vec4<f32>,
}

@group(2) @binding(100)
var<uniform> settings: BallSettings;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    let t = settings.time;
    let uv = in.uv;

    // 4D Slice Calculation (1:1 Parity)
    let diff = abs(settings.object_w - settings.player_w);
    let slice_limit = settings.hyper_slice + settings.hyper_falloff;
    
    // Player ball dims out but doesn't discard (for visibility in mirror world)
    let slice_alpha = 1.0 - smoothstep(settings.hyper_slice, slice_limit, diff);
    
    let edge_factor = smoothstep(settings.hyper_slice, slice_limit, diff);
    let edge_glow = pow(edge_factor, 2.0) * settings.edge_color.rgb * 6.0;

    // Layer 0: scrolling noise pattern
    let uv0 = uv * 3.0 + settings.layer0_scroll * t;
    let n0 = noise2d(uv0 * 4.0) * 0.6 + noise2d(uv0 * 8.0) * 0.3 + noise2d(uv0 * 16.0) * 0.1;
    let pulse0 = mix(settings.layer0_pulse.z, settings.layer0_pulse.w,
        0.5 + 0.5 * sin(t * settings.layer0_pulse.x + settings.layer0_pulse.y));

    // Layer 1: different scroll direction
    let uv1 = uv * 2.5 + settings.layer1_scroll * t;
    let n1 = noise2d(uv1 * 5.0) * 0.5 + noise2d(uv1 * 10.0) * 0.35 + noise2d(uv1 * 20.0) * 0.15;
    let pulse1 = mix(settings.layer1_pulse.z, settings.layer1_pulse.w,
        0.5 + 0.5 * sin(t * settings.layer1_pulse.x + settings.layer1_pulse.y));

    // Layer 2: fine detail
    let uv2 = uv * 4.0 + settings.layer2_scroll * t;
    let n2 = noise2d(uv2 * 6.0) * 0.4 + noise2d(uv2 * 12.0) * 0.4 + noise2d(uv2 * 24.0) * 0.2;
    let pulse2 = mix(settings.layer2_pulse.z, settings.layer2_pulse.w,
        0.5 + 0.5 * sin(t * settings.layer2_pulse.x + settings.layer2_pulse.y));

    // Additive combination of layers
    let combined = n0 * pulse0 + n1 * pulse1 + n2 * pulse2;

    // HSV color cycling — produces shifting rainbow/plasma look
    let hue = fract(settings.hue_shift + combined * 0.3 + 0.1 * sin(t * 0.5));
    let sat = 0.65 + 0.2 * sin(t * 0.8 + combined * 2.0);
    let val = 0.6 + combined * 0.4;
    let color = hsv2rgb(vec3<f32>(hue, sat, val));

    // Mix into PBR base color — keep some of the original material color for lighting
    // -----------------------------------------------------------
    // Final Compositing
    // -----------------------------------------------------------
    let base_layers = max(n0 * pulse0, max(n1 * pulse1, n2 * pulse2)); // Use nX * pulseX for individual layer contributions
    var final_col = mix(settings.edge_color.rgb, color, 0.85); // Start with original color mix
    final_col += edge_glow; // Add 4D edge glow

    pbr_input.material.base_color = vec4<f32>(final_col * slice_alpha, slice_alpha);
    pbr_input.material.emissive   = vec4<f32>(final_col * 2.5 * slice_alpha, 1.0); // Boost for bloom

    return apply_pbr_lighting(pbr_input);
}
