#![allow(dead_code)]
use bevy::prelude::*;

#[derive(Component, Default, Clone, Copy, Debug, Reflect)]
pub struct Spatial4D {
    pub w: f32,                // Current W coordinate
    pub target_w: f32,         // Target W for interpolation
    
    // Derived values for fast bucket categorization
    pub layer: i32,            // Discretized layer (e.g., w = 5 -> layer 1)
    pub is_folded: bool,       // Whether the entity is currently subject to Mobius folding
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Velocity4D {
    pub lin_v: Vec3,           // 3D linear velocity
    pub w_v: f32,              // W-dimension velocity
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Collision4D {
    pub radius: f32,           // 4D collision sphere radius
    pub mask: u32,             // Collision mask (Player, Enemy, Level, etc.)
}

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component)]
pub struct Team {
    pub id: u8,                // 0: Neutral, 1: Player, 2: Enemy
}
use std::collections::VecDeque;

#[derive(Component, Clone)]
pub struct TransformHistory {
    pub poses: VecDeque<Transform>,
    pub max_len: usize,
}

#[derive(Component, Default, Clone, Copy, Debug, Reflect)]
pub struct CompressionState {
    pub factor: f32,
    pub factor_smoothed: f32,
}

#[derive(Component, Default)]
pub struct PlayerVisuals {
    pub tex_scroll_u: f32,
    pub tex_scroll_v: f32,
    pub cycle_timer: f32,
    pub emissive_color: Color,
}

#[derive(Component)]
pub struct MonsterHpBarFill;

#[derive(Component)]
pub struct MonsterStateText;

#[derive(Component)]
pub struct PlayerNameLabel;

#[derive(Component)]
pub struct LifeTime(pub f32);

#[derive(Component)]
pub struct EnemyProjectile;

#[derive(Component)]
pub struct MinimapMarker;
#[derive(Component)]
pub struct CameraObstacle;
