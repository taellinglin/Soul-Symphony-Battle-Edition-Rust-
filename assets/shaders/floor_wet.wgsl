#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::apply_pbr_lighting
#import bevy_pbr::forward_io::VertexOutput

struct FloorWetSettings {
    room_uv_scale: f32,
    wake_strength: f32,
    pulse_count: u32,
    player_w: f32,
    object_w: f32,
    thickness: f32,
    time: f32,
    pad0: f32,
    contact_uv: vec2<f32>,
    pad1: vec2<f32>,
    edge_color: vec4<f32>,
    pulses: array<vec4<f32>, 8>,
}

@group(2) @binding(100) var<uniform> settings: FloorWetSettings;
@group(2) @binding(101) var base_texture: texture_2d<f32>;
@group(2) @binding(102) var base_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    // Initialize PBR input
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // --- Python Parity: Floor Liquid Logic ---
    let dt = settings.time;
    let world_xz = in.world_position.xz;
    let uv = world_xz * settings.room_uv_scale;

    // Twin-layer flow animation (Python parity: flow_a, flow_b)
    let flow_a = vec2<f32>(dt * 0.038, -dt * 0.023);
    let flow_b = vec2<f32>(-dt * 0.027, dt * 0.031);
    
    let base_a = textureSample(base_texture, base_sampler, fract(uv + flow_a)).rgb;
    let base_b = textureSample(base_texture, base_sampler, fract(uv * 1.31 + flow_b)).rgb;
    let base_mix = mix(base_a, base_b, 0.52);

    // Initial ripple strength from active contact
    let contact_uv_scaled = settings.contact_uv * settings.room_uv_scale;
    let dist_uv = distance(uv, contact_uv_scaled);
    let ring = sin(dist_uv * 66.0 - dt * 15.0);
    let envelope = exp(-dist_uv * 7.8);
    var wake: f32 = (0.5 + 0.5 * ring) * envelope * settings.wake_strength;
    
    // Accumulate independent pulses (Python parity: _update_floor_pulses)
    for (var i = 0u; i < 8u; i = i + 1u) {
        let p = settings.pulses[i];
        if (p.w > 0.0) {
            let age = dt - p.z;
            if (age >= 0.0 && age < 1.5) {
                let p_uv = p.xy * settings.room_uv_scale;
                let p_dist = distance(uv, p_uv);
                let p_ring = sin(p_dist * 66.0 - dt * 15.0); // Python uses world time for ring phase consistency
                let p_envelope = exp(-p_dist * 7.8) * (1.0 - age / 1.5);
                wake = max(wake, (0.5 + 0.5 * p_ring) * p_envelope * p.w);
            }
        }
    }

    // Shimmer (Python parity: shimmer)
    let shimmer = 0.5 + 0.5 * sin((uv.x + uv.y) * 10.5 + dt * 1.8);

    // Final color accumulation (Python parity: water vec3)
    var water = vec3<f32>(0.06, 0.13, 0.2) + base_mix * 0.62;
    water += vec3<f32>(0.16, 0.25, 0.34) * (wake * 1.2 + shimmer * 0.18);

    // Integrate with PBR material
    pbr_input.material.base_color = vec4<f32>(clamp(water, vec3<f32>(0.0), vec3<f32>(1.0)), 0.96);
    pbr_input.material.emissive = vec4<f32>(settings.edge_color.rgb * wake * 0.45, 1.0);
    pbr_input.material.perceptual_roughness = 0.08; 
    pbr_input.material.metallic = 0.02;

    return apply_pbr_lighting(pbr_input);
}
