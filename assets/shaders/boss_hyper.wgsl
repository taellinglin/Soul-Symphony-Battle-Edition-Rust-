#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::apply_pbr_lighting
#import bevy_pbr::forward_io::VertexOutput
#import bevy_pbr::mesh_view_bindings::globals

struct BossHyperSettings {
    intensity: f32,
    variant: f32,
    hyper_w: f32,
    pad: f32,
}

@group(2) @binding(100) var<uniform> settings: BossHyperSettings;
@group(2) @binding(101) var p3d_texture: texture_2d<f32>;
@group(2) @binding(102) var p3d_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    
    let time = globals.time;
    let t = time * (0.65 + settings.intensity * 0.25);
    let w = settings.hyper_w * 0.35;
    let v_world = in.world_position.xyz;

    // --- Python Parity: uv_distort logic ---
    var uv = in.uv * (1.8 + 0.22 * settings.variant);
    uv.x += 0.12 * sin(v_world.y * 0.15 + t * 0.8 + w * 2.1);
    uv.y += 0.12 * cos(v_world.x * 0.15 + t * 0.9 - w * 1.8);
    
    // Multiple distortion octaves
    uv.x += 0.05 * sin(uv.y * 12.0 + t * 2.2);
    uv.y += 0.05 * cos(uv.x * 12.0 + t * 2.4);

    let base = pbr_input.material.base_color.rgb;

    // Complex interference patterns (Python parity: noise components)
    let n1 = sin(v_world.x * 0.22 + t * 2.2 + w);
    let n2 = cos(v_world.y * 0.28 - t * 1.9 - w * 0.7);
    let n3 = sin((v_world.x + v_world.y + v_world.z) * 0.14 + t * 1.3);
    let pulse = 0.5 + 0.5 * sin(t * 3.2 + v_world.z * 0.6);
    let noise = (n1 + n2 + n3) / 3.0;

    // Palette: Cyan, Pink, Yellow (Original colors)
    let c0 = vec3<f32>(0.20, 0.95, 1.00);
    let c1 = vec3<f32>(1.00, 0.25, 0.85);
    let c2 = vec3<f32>(0.95, 1.00, 0.20);

    let m0 = 0.5 + 0.5 * sin(t + noise * 2.2 + settings.variant);
    let m1 = 0.5 + 0.5 * cos(t * 0.8 - noise * 2.7 - settings.variant);

    var trippy = mix(c0, c1, m0);
    trippy = mix(trippy, c2, m1 * 0.45 + pulse * 0.2);

    // Final color accumulation
    var color = mix(base, trippy, 0.58 + 0.22 * settings.intensity);
    color += trippy * (0.14 + 0.12 * pulse);
    
    // Add holographic scanlines
    let scanline = 0.5 + 0.5 * sin(in.position.y * 1.2 + t * 15.0);
    color *= 0.92 + 0.08 * scanline;

    pbr_input.material.base_color = vec4<f32>(color, 1.0);
    pbr_input.material.emissive = vec4<f32>(trippy * pulse * 0.35, 1.0);
    
    return apply_pbr_lighting(pbr_input);
}
