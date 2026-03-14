use bevy::prelude::*;

#[derive(Component, Default, Clone, Copy, Debug, Reflect)]
pub struct Spatial4D {
    pub w: f32,        // Current W coordinate
    pub target_w: f32, // Target W for interpolation

    // Derived values for fast bucket categorization
    pub layer: i32,      // Discretized layer (e.g., w = 5 -> layer 1)
    pub is_folded: bool, // Whether the entity is currently subject to Mobius folding
}

#[derive(Clone, Debug)]
pub struct WarpLink {
    pub a_pos: Vec3,
    pub b_pos: Vec3,
    pub radius: f32,
    pub mobius: bool,
    pub mobius_phase: f32,
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Velocity4D {
    pub lin_v: Vec3, // 3D linear velocity
    pub w_v: f32,    // W-dimension velocity
}

#[derive(Component, Clone, Copy, Debug, Reflect)]
pub struct Collision4D {
    pub radius: f32, // 4D collision sphere radius
    pub mask: u32,   // Collision mask (Player, Enemy, Level, etc.)
}

#[derive(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

#[derive(Component, Default, Clone, Copy, Debug, Reflect)]
pub struct CompressionState {
    pub factor: f32,
    pub factor_smoothed: f32,
}

#[derive(Component, Default)]
pub struct PlayerVisuals {
    pub cycle_timer: f32,
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
pub struct CameraObstacle;
