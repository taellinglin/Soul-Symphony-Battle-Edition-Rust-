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
#[allow(dead_code)]
pub struct Monster {
    pub variant: MonsterVariant,
    pub state: AiState,
    
    // AI Parameters
    pub attack_mult: f32,
    pub defense: f32,
    pub critical_chance: f32,
    
    // Distances
    pub hunt_range: f32,
    pub attack_range: f32,
    pub guard_range: f32,
    
    // Movement
    pub speed_boost: f32,
    
    // Timers
    pub ai_state_timer: f32,
    pub jump_cooldown: f32,
    
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

#[derive(Component)]
pub struct Mob;

#[derive(Component)]
#[allow(dead_code)]
pub struct Boss {
    pub state: AiState,
    pub dash_cooldown: Timer,
}

#[derive(Component)]
#[allow(dead_code)]
pub struct StashedDungeon;

// A marker component to ensure we only spawn once per map load
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
