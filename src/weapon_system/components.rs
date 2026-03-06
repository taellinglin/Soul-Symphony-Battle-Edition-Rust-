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

// Fixed constants from Python for ball_radius = 0.68
pub(crate) const SWORD_SCALE: f32 = 1.7; // max(1.0, 0.68 / 0.4)
pub(crate) const UP_OFFSET: f32 = 0.84; // 0.68 + 0.16
pub(crate) const FWD_OFFSET: f32 = 0.578; // 0.34 * 1.7
pub(crate) const SIDE_OFFSET: f32 = 0.275; // 0.22 * min(1.25, 1.7)
pub(crate) const REACH: f32 = 3.06; // 1.8 * 1.7
