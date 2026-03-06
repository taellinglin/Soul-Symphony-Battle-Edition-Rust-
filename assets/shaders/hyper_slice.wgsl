#import bevy_pbr::pbr_fragment::pbr_input_from_standard_material
#import bevy_pbr::pbr_functions::apply_pbr_lighting
#import bevy_pbr::forward_io::VertexOutput

struct HyperSliceSettings {
    player_w: f32,
    object_w: f32,
    thickness: f32,
    room_uv_scale: f32,
    time: f32,
    persistence: f32,
    fog_start: f32,
    fog_end: f32,
    edge_color: vec4<f32>,
    fog_color: vec4<f32>,
}

#import bevy_pbr::mesh_view_bindings::view

@group(2) @binding(100)
var<uniform> settings: HyperSliceSettings;

@group(2) @binding(101)
var base_texture: texture_2d<f32>;

@group(2) @binding(102)
var base_sampler: sampler;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> @location(0) vec4<f32> {
    // 4D Slice Calculation
    let dist_w = abs(settings.player_w - settings.object_w);
    let edge_width = settings.thickness * 0.22;
    let slice_mask = 1.0 - smoothstep(settings.thickness - edge_width, settings.thickness, dist_w);
    
    // Discard fragments far from player's W coordinate (unless persistent)
    if settings.persistence < 0.5 && dist_w > settings.thickness {
        discard;
    }

    // Default PBR shading
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    // World-space UV mapping (Python parity)
    let n = pbr_input.N;
    var uv = vec2<f32>(0.0);
    if (abs(n.y) > 0.5) {
        uv = in.world_position.xz;
    } else if (abs(n.x) > 0.5) {
        uv = in.world_position.zy;
    } else {
        uv = in.world_position.xy;
    }
    uv = uv * settings.room_uv_scale;

    // 4D Texture Shift (Python parity)
    let w_delta = settings.object_w - settings.player_w;
    let u_off = (settings.time * 0.08 + w_delta * 0.11) % 1.0;
    let v_off = (settings.time * 0.056 - w_delta * 0.09) % 1.0;
    uv += vec2<f32>(u_off, v_off);

    let tex_color = textureSample(base_texture, base_sampler, uv).rgb;
    pbr_input.material.base_color = vec4<f32>(pbr_input.material.base_color.rgb * tex_color, pbr_input.material.base_color.a);
    
    // 4D Edge Glow (Intense visual parity)
    var edge_factor = pow(dist_w / settings.thickness, 4.0);
    if settings.persistence > 0.5 {
        edge_factor = 0.0;
    }
    let glow_intensity = edge_factor * 2.5;
    
    // Mix base color with edge color
    pbr_input.material.base_color = mix(pbr_input.material.base_color, settings.edge_color, edge_factor);
    
    // Additive Emissive Glow
    pbr_input.material.emissive = pbr_input.material.emissive + settings.edge_color * glow_intensity;
    
    // Rim lighting approximation for extra "premium" feel
    let rim = 1.0 - saturate(dot(pbr_input.N, pbr_input.V));
    let rim_glow = pow(rim, 3.0) * settings.edge_color * 0.5;
    pbr_input.material.emissive += rim_glow;

    // Apply final lighting logic
    let pbr_color = apply_pbr_lighting(pbr_input);

    // Removed per-material fog — Bevy's camera FogSettings handles atmospheric fog.
    // Previously, double-fog (camera + shader) caused the scene to go black inside rooms.
    return pbr_color;
}
