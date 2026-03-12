use bevy::prelude::*;

#[derive(Component)]
pub struct Weapon {
    pub state: WeaponState,
    pub timer: f32,
    pub throw_origin: Option<Vec3>,
    pub throw_dir: Vec3,
    pub hit_targets: std::collections::HashSet<Entity>,
    pub anchor_pos: Vec3,
    pub weapon_forward: Vec3,
    pub prev_tip_pos: Option<Vec3>,
    pub slash_timer: f32,
    pub echo_timer: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            state: WeaponState::Idle,
            timer: 0.0,
            throw_origin: None,
            throw_dir: Vec3::Z,
            hit_targets: std::collections::HashSet::new(),
            anchor_pos: Vec3::ZERO,
            weapon_forward: Vec3::Z,
            prev_tip_pos: None,
            slash_timer: 0.0,
            echo_timer: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeaponState {
    Idle,
    #[allow(dead_code)]
    Swing,
    Spin,
    Throw,
    Hyperbomb,
    MagicMissile,
}

#[derive(Event)]
pub struct DamageEvent {
    pub target: Entity,
    pub amount: f32,
    pub knockback: Vec3,
}

#[derive(Component)]
pub struct BladeEcho {
    pub life: f32,
    pub max_life: f32,
}

#[derive(Component)]
pub struct SlashTrail {
    pub life: f32,
    pub max_life: f32,
}

#[derive(Component)]
pub struct Hyperbomb {
    pub timer: f32,
    pub duration: f32,
    pub radius: f32,
    pub max_radius: f32,
}

#[derive(Component)]
pub struct MagicMissile {
    pub velocity: Vec3,
    pub target: Option<Entity>,
    pub life: f32,
}

// Python parity: setup_weapon_system with ball_radius = 0.68
// sword_scale = max(1.0, ball_radius / 0.4) = 1.7
// up    = max(0.36, ball_radius + 0.16)            = 0.84
// fwd   = max(0.34, 0.34 * sword_scale)            = 0.578
// side  = max(0.22, 0.22 * min(1.25, sword_scale)) = 0.275
// reach = max(1.8,  1.8  * sword_scale)             = 3.06
pub(crate) const SWORD_SCALE: f32 = 1.7;
pub(crate) const SWORD_GEO_SCALE: f32 = 1.7; // Geometry scale for sword mesh parts
pub(crate) const UP_OFFSET: f32 = 0.84;
pub(crate) const FWD_OFFSET: f32 = 0.578;
pub(crate) const SIDE_OFFSET: f32 = 0.275;
pub(crate) const REACH: f32 = 3.06;
