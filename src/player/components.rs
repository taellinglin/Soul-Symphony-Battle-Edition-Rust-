use bevy::prelude::*;

#[derive(Component)]
pub struct Player;

#[derive(Component)]
pub struct PlayerCamera;

#[derive(Component)]
pub struct InvertedEchoCamera;

#[derive(Component)]
pub struct FloatingTextCamera;

#[derive(Component, Default)]
pub struct PlayerStats {
    pub hp: f32,
    pub max_hp: f32,
    pub hyperbomb_cooldown: f32,
    pub magic_missile_cooldown: f32,
    #[allow(dead_code)]
    pub magic_missile_count_bonus: u32,
}

#[derive(Component)]
pub struct LocalPlayerHpBar;

#[derive(Component)]
pub struct LocalPlayerXpBar;

// Player stats are now largely handled in progression.rs

// ─────────────────────────────────────────────────────────────────────────────
// Resources
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Resource)]
pub struct CameraOrbitState {
    pub smoothed_dir: Vec3,
    pub heading: f32,
    pub heading_input: f32,
    pub pitch_input: f32,
    pub manual_turn_hold: f32,
    pub smoothed_pos: Option<Vec3>,
    pub follow: f32,
    pub height: f32,
    pub fov_deg: f32,
}

impl Default for CameraOrbitState {
    fn default() -> Self {
        Self {
            smoothed_dir: -Vec3::Z,
            heading: 0.0,
            heading_input: 0.0,
            pitch_input: 0.0,
            manual_turn_hold: 0.0,
            smoothed_pos: None,
            follow: CAMERA_FOLLOW_DISTANCE,
            height: CAMERA_HEIGHT_OFFSET,
            fov_deg: CAMERA_FOV_BASE,
        }
    }
}

#[derive(Resource)]
pub struct GravityDirection(pub Vec3);

impl Default for GravityDirection {
    fn default() -> Self {
        // Original Python parity: zone gravity magnitude 19.62 mapped to Y-down
        // (PARITY_AUDIT §3.1 notes this matches `gravity_magnitude` rather than Bullet's -9.62 world gravity)
        Self(Vec3::new(0.0, -19.62, 0.0))
    }
}

#[derive(Resource)]
pub struct HyperspaceState {
    pub threshold: f32,
    pub w_limit: f32,
    pub force_strength: f32,
    pub force_lift: f32,
    pub gravity_hold: bool,
    pub shift_brake_drag: f32,
    pub ball_friction_default: f32,
    pub ball_friction_shift: f32,
}

impl Default for HyperspaceState {
    fn default() -> Self {
        Self {
            threshold: 0.2,
            w_limit: 7.2,
            force_strength: 42.0,
            force_lift: 0.18,
            gravity_hold: false,
            shift_brake_drag: 4.1,
            ball_friction_default: 0.02,
            ball_friction_shift: 0.05,
        }
    }
}

impl HyperspaceState {
    /// Whether hyperspace is currently active based on player W coordinate
    pub fn is_active(&self, player_w: f32) -> bool {
        player_w.abs() > self.threshold
    }
}

#[derive(Resource)]
pub struct JumpState {
    pub grounded: bool,
    pub prev_grounded: bool,
    pub jumps_used: u32,
    pub max_jumps: u32,
    pub infinite_jumps: bool,
    pub jump_queued: bool,
    pub jump_float_timer: f32,
    pub jump_float_duration: f32,
    pub jump_float_drag: f32,
    pub float_fall_drag: f32,
    pub hit_cooldown: f32,
    pub attack_cooldown: f32,
    pub player_damage_cooldown: f32,
}

impl Default for JumpState {
    fn default() -> Self {
        Self {
            grounded: false,
            prev_grounded: false,
            jumps_used: 0,
            max_jumps: 2,
            infinite_jumps: true,
            jump_queued: false,
            jump_float_timer: 0.0,
            jump_float_duration: 0.56,
            jump_float_drag: 5.8,
            float_fall_drag: 2.2,
            hit_cooldown: 0.0,
            attack_cooldown: 0.0,
            player_damage_cooldown: 0.0,
        }
    }
}

#[derive(Resource, Default)]
pub struct PrevBallState {
    pub position: Vec3,
    pub velocity: Vec3,
}

#[derive(Resource)]
pub struct PhysicsTimers {
    pub roll_time: f32,
    pub last_move_dir: Vec3,
    pub monster_contact_sfx_cooldown: f32,
    pub warp_cooldown: f32,
}

impl Default for PhysicsTimers {
    fn default() -> Self {
        Self {
            roll_time: 0.0,
            monster_contact_sfx_cooldown: 0.0,
            warp_cooldown: 0.0,
            last_move_dir: -Vec3::Z,
        }
    }
}

pub(crate) const BALL_RADIUS: f32 = 0.68;
pub(crate) const MAX_BALL_SPEED: f32 = 15.25;
pub(crate) const LINK_CONTROL_GAIN: f32 = 32.0;
pub(crate) const LINK_BRAKE_DRAG: f32 = 2.6;
pub(crate) const ROLL_FORCE: f32 = 18.0;
pub(crate) const ROLL_TORQUE: f32 = 14.0;
pub(crate) const JUMP_IMPULSE: f32 = 24.0;
pub(crate) const JUMP_RISE_BOOST: f32 = 1.22;
pub(crate) const SPACE_BOOST_IMPULSE: f32 = 2.4;
pub(crate) const CAMERA_FOLLOW_DISTANCE: f32 = 6.8; // Python parity: self.camera_follow_distance = 6.8
pub(crate) const CAMERA_HEIGHT_OFFSET: f32 = 2.35;
pub(crate) const CAMERA_FOV_BASE: f32 = 108.0;
pub(crate) const CAMERA_AUTO_ALIGN_SPEED: f32 = 3.5;
pub(crate) const CAMERA_AUTO_ALIGN_MIN_SPEED: f32 = 0.08;
pub(crate) const CAMERA_BALL_CLEARANCE: f32 = 0.16;
pub(crate) const COMPRESSION_SMOOTH_SPEED: f32 = 8.0;
pub(crate) const SPACE_COMPRESS_3D_STRENGTH: f32 = 0.1;
pub(crate) const CAMERA_DIMENSION_BLEND_SPEED: f32 = 6.5;
pub(crate) const TUNNEL_MIN_TRAVEL: f32 = 0.04; // ball_radius * 0.06 ≈ 0.04
