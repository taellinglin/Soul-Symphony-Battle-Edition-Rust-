use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiState {
    Wandering,
    Guarding,
    Hunting,
    Attacking,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonsterVariant {
    Normal,
    Juggernaut,
    Vanguard,
    Raider,
    Giant,
}

#[derive(Component)]
pub struct Monster {
    pub variant: MonsterVariant,
    pub state: AiState,

    // AI Parameters
    pub attack_mult: f32,
    pub defense: f32,
    // Critical chance is currently unused; remove when crit logic is implemented.
    // Keeping structure minimal avoids dead code.

    // Distances
    pub hunt_range: f32,
    pub attack_range: f32,
    pub guard_range: f32,

    // Movement
    pub speed_boost: f32,

    // Timers
    pub ai_state_timer: f32,
    // Jump cooldown is currently unused; remove when jump-attack logic is implemented.

    // Original metadata
    pub is_docile: bool,
    pub awakened: bool,
    pub is_boss: bool,

    // Teleport & Liminal (New Parity Gaps)
    pub teleport_enabled: bool,
    pub teleport_cooldown: f32,
    pub liminal_enabled: bool,
    pub fold_jump_cooldown: f32,

    pub ranged_enabled: bool,
    pub ranged_cooldown: f32,
    pub cosmic_warp_cooldown: f32,
    pub last_announced_state: Option<AiState>,
}

#[derive(Component, Default)]
pub struct KnockbackVel(pub Vec3);

#[derive(Component)]
pub struct Mob;

#[derive(Component)]
pub struct Boss {
    pub dash_cooldown: Timer,
}

#[derive(Component)]
pub struct MapMonstersSpawned;

#[derive(Component)]
pub struct MonsterPart {
    pub base_offset: Vec3,
    pub min_scale: f32,
    pub max_scale: f32,
    pub phase: f32,
    pub speed: f32,
}
