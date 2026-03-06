use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::components::Spatial4D;
use crate::player::Player;

pub struct BuoyancyPlugin;

impl Plugin for BuoyancyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_water_buoyancy);
    }
}

/// Simple definition for water surfaces
#[derive(Component)]
pub struct WaterSurface {
    pub half_extents: Vec2,
    pub height: f32,
}

fn apply_water_buoyancy(
    mut query: Query<(&mut ExternalForce, &mut Velocity, &ColliderMassProperties, &Transform, &Spatial4D), With<Player>>,
    water_query: Query<(&Transform, &WaterSurface)>,
    gravity: Res<crate::player::GravityDirection>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds();
    let Ok((mut ext_force, mut vel, mass_props, player_tf, _spatial)) = query.get_single_mut() else { return };
    
    let ball_pos = player_tf.translation;
    let ball_radius = 0.68; // Player ball radius
    let _bottom_z = ball_pos.y - ball_radius; // Assuming Y is up, but we'll use gravity direction for proper math
    
    let input_up = gravity.0.normalize_or_zero();
    let g_mag = gravity.0.length().max(0.1);
    
    let mass = match mass_props {
        ColliderMassProperties::Mass(m) => *m,
        ColliderMassProperties::Density(d) => *d * (4.0/3.0) * std::f32::consts::PI * ball_radius.powi(3),
        ColliderMassProperties::MassProperties(_) => 1.25, // Fallback
    };
    
    // Find highest water surface we are currently intersecting horizontally
    let mut max_water_h = None;
    for (water_tf, surface) in water_query.iter() {
        // Simple AABB check horizontally
        let dx = (ball_pos.x - water_tf.translation.x).abs();
        let dz = (ball_pos.z - water_tf.translation.z).abs();
        
        if dx <= surface.half_extents.x && dz <= surface.half_extents.y {
            // Include wave height if we had a wave sampler, for now use base height
            let h = water_tf.translation.y + surface.height;
            if max_water_h.map_or(true, |prev_h| h > prev_h) {
                max_water_h = Some(h);
            }
        }
    }
    
    if let Some(water_h) = max_water_h {
        let bottom_dist = ball_pos.dot(input_up) - ball_radius;
        let depth = water_h - bottom_dist;
        
        if depth > 0.0 {
            // Apply buoyancy (original lines 5219-5236)
            let submerge = (depth / (ball_radius * 1.9).max(0.05)).clamp(0.0, 1.35);
            let buoy_bias = 0.62;
            let buoy_strength = 2.2;
            
            let buoy_force = input_up * (mass * g_mag * (buoy_bias + submerge * buoy_strength));
            ext_force.force += buoy_force; // Add to existing forces
            
            // Apply water drag
            let v_up = vel.linvel.dot(input_up);
            let mut v_planar = vel.linvel - input_up * v_up;
            let mut new_v_up = v_up;
            
            let planar_drag_coeff = 0.85;
            let vertical_drag_coeff = 1.95;
            
            let planar_drag = (1.0 - dt * planar_drag_coeff * (0.3 + submerge * 0.7)).max(0.0);
            let vertical_drag = (1.0 - dt * vertical_drag_coeff * (0.4 + submerge * 0.9)).max(0.0);
            
            v_planar *= planar_drag;
            new_v_up *= vertical_drag;
            
            vel.linvel = v_planar + input_up * new_v_up;
        }
    }
}
