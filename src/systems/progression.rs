use bevy::prelude::*;
use crate::player::Player;
use serde::{Serialize, Deserialize};
use std::fs;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum GameState {
    #[default]
    Playing,
    GameOver,
}

pub struct ProgressionPlugin;

impl Plugin for ProgressionPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<GameState>()
           .init_resource::<MonsterStats>()
           .init_resource::<KillProtection>()
           .add_event::<GainXpEvent>()
           .add_event::<PlayerLevelUpEvent>()
           .add_event::<HealEvent>()
           .add_event::<SwordPowerupEvent>()
           .add_systems(Startup, load_progress)
           .add_systems(Update, (
               xp_gain_handler,
               level_up_handler,
               heal_handler,
               sword_pickup_handler,
               tick_skill_buffs,
               player_death_check,
               update_combat_multipliers,
               save_progress_on_level_up,
           ).run_if(in_state(GameState::Playing)))
           .add_systems(Update, game_over_countdown.run_if(in_state(GameState::GameOver)));
    }
}

#[derive(Serialize, Deserialize, Default, Resource)]
pub struct SaveData {
    pub level: u32,
    pub xp: f32,
    pub xp_next: f32,
    pub atk: u32,
    pub def: u32,
    pub dex: u32,
    pub sta: u32,
    pub int: u32,
    pub sword_dmg_mult: f32,
    pub dmg_taken_mult: f32,
    pub hp_max: f32,
    pub crit_chance: f32,
    pub crit_mult: f32,
    pub haste_rem: f32,
    pub fury_rem: f32,
    pub longblade_rem: f32,
    pub critical_rem: f32,
}

#[derive(Resource)]
pub struct GameOverTimer(pub Timer);

#[derive(Resource)]
pub struct KillProtection {
    pub stacks: u32,
    pub max_stacks: u32,
}

impl Default for KillProtection {
    fn default() -> Self {
        Self { stacks: 0, max_stacks: 24 }
    }
}

#[derive(Resource, Default)]
pub struct MonsterStats {
    pub total: usize,
    pub slain: usize,
}

fn tick_skill_buffs(
    time: Res<Time>,
    mut query: Query<&mut PlayerSkillBuffs, With<Player>>,
) {
    if let Ok(mut buffs) = query.get_single_mut() {
        buffs.haste.tick(time.delta());
        buffs.fury.tick(time.delta());
        buffs.longblade.tick(time.delta());
        buffs.critical.tick(time.delta());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Components
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Component, Serialize, Deserialize, Clone)]
pub struct PlayerProgression {
    pub level: u32,
    pub xp: f32,
    pub xp_next: f32,
    pub xp_growth: f32,
    pub stat_cycle_idx: usize,
}

impl Default for PlayerProgression {
    fn default() -> Self {
        Self {
            level: 1,
            xp: 0.0,
            xp_next: 12.0,
            xp_growth: 1.28,
            stat_cycle_idx: 0,
        }
    }
}

#[derive(Component, Default, Serialize, Deserialize, Clone)]
pub struct PlayerCombatStats {
    pub atk: u32,
    pub def: u32,
    pub dex: u32,
    pub sta: u32,
    pub int: u32,
    pub sword_dmg_mult: f32,
    pub dmg_taken_mult: f32,
    pub hp_max: f32,
    pub crit_chance: f32,
    pub crit_mult: f32,
}

#[derive(Component)]
pub struct PlayerSkillBuffs {
    pub haste: Timer,
    pub fury: Timer,
    pub longblade: Timer,
    pub critical: Timer,
}

impl Default for PlayerSkillBuffs {
    fn default() -> Self {
        let mut t_haste = Timer::from_seconds(0.0, TimerMode::Once);
        t_haste.set_elapsed(Duration::from_secs_f32(0.0));
        let mut t_fury = Timer::from_seconds(0.0, TimerMode::Once);
        t_fury.set_elapsed(Duration::from_secs_f32(0.0));
        let mut t_longblade = Timer::from_seconds(0.0, TimerMode::Once);
        t_longblade.set_elapsed(Duration::from_secs_f32(0.0));
        let mut t_critical = Timer::from_seconds(0.0, TimerMode::Once);
        t_critical.set_elapsed(Duration::from_secs_f32(0.0));

        Self {
            haste: t_haste,
            fury: t_fury,
            longblade: t_longblade,
            critical: t_critical,
        }
    }
}

use std::time::Duration;

// ─────────────────────────────────────────────────────────────────────────────
// Events
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Event)]
pub struct GainXpEvent {
    pub amount: f32,
}

#[derive(Event)]
pub struct PlayerLevelUpEvent {
    pub _new_level: u32,
    pub stat_boosted: String,
}

#[derive(Event)]
pub struct HealEvent {
    pub amount: f32,
}

#[derive(Event)]
pub struct SwordPowerupEvent {
    pub powerup_type: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Systems
// ─────────────────────────────────────────────────────────────────────────────

fn xp_gain_handler(
    mut events: EventReader<GainXpEvent>,
    mut query: Query<(&mut PlayerProgression, &PlayerCombatStats), With<Player>>,
    mut lv_events: EventWriter<PlayerLevelUpEvent>,
) {
    if let Ok((mut progression, stats)) = query.get_single_mut() {
        for event in events.read() {
            let xp_gain = event.amount * (1.0 + (stats.int as f32 * 0.03));
            progression.xp += xp_gain;
            
            while progression.xp >= progression.xp_next {
                progression.xp -= progression.xp_next;
                progression.level += 1;
                progression.xp_next *= progression.xp_growth;
                
                let stats_list = ["attack", "defense", "dex", "sta", "int"];
                let stat = stats_list[progression.stat_cycle_idx % stats_list.len()];
                progression.stat_cycle_idx += 1;
                
                lv_events.send(PlayerLevelUpEvent {
                    _new_level: progression.level,
                    stat_boosted: stat.to_string(),
                });
            }
        }
    }
}

fn level_up_handler(
    mut lv_events: EventReader<PlayerLevelUpEvent>,
    mut query: Query<(&mut PlayerCombatStats, &mut crate::player::PlayerStats, &Transform), With<Player>>,
    mut fx_events: EventWriter<crate::effects::FloatingTextEvent>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
) {
    for event in lv_events.read() {
        if let Ok((mut stats, mut player_stats, tf)) = query.get_single_mut() {
            sfx_events.send(crate::effects::audio::PlaySfxEvent {
                kind: crate::effects::audio::SfxKind::LevelUp,
                volume: 0.9, pitch: 1.0, position: Some(tf.translation),
            });
            let mut color = Color::srgba(0.5, 1.0, 0.6, 1.0);
            let text = format!("LEVEL UP: {}", event.stat_boosted.to_uppercase());
            match event.stat_boosted.as_str() {
                "attack" => {
                    stats.atk += 1;
                    stats.sword_dmg_mult += 0.06;
                }
                "defense" => {
                    stats.def += 1;
                    stats.dmg_taken_mult *= 0.95;
                }
                "dex" => {
                    stats.dex += 1;
                    // Lower attack cooldowns?
                }
                "sta" => {
                    stats.sta += 1;
                    stats.hp_max += 6.0;
                    player_stats.max_hp = stats.hp_max;
                    player_stats.hp += 8.0; // Heal on sta level
                }
                "int" => {
                    stats.int += 1;
                    color = Color::srgba(0.78, 0.55, 1.0, 1.0);
                }
                _ => {}
            }
            player_stats.max_hp = stats.hp_max;
            player_stats.hp = player_stats.hp.min(player_stats.max_hp);

            fx_events.send(crate::effects::FloatingTextEvent {
                pos: tf.translation + Vec3::Y * 0.75,
                text,
                color,
                scale: 0.3,
                life: 1.1,
            });
        }
    }
}

fn heal_handler(
    mut heal_events: EventReader<HealEvent>,
    mut query: Query<(&mut crate::player::PlayerStats, &Transform), With<Player>>,
    mut sfx_events: EventWriter<crate::effects::audio::PlaySfxEvent>,
) {
    for event in heal_events.read() {
        if let Ok((mut p_stats, tf)) = query.get_single_mut() {
            p_stats.hp += event.amount;
            p_stats.hp = p_stats.hp.min(p_stats.max_hp);
            
            sfx_events.send(crate::effects::audio::PlaySfxEvent {
                kind: crate::effects::audio::SfxKind::Heal,
                volume: 0.8, pitch: 1.1, position: Some(tf.translation),
            });
        }
    }
}

fn sword_pickup_handler(
    mut events: EventReader<SwordPowerupEvent>,
    mut query: Query<(&mut PlayerCombatStats, &mut PlayerSkillBuffs, &mut crate::player::PlayerStats), With<Player>>,
) {
    if let Ok((mut stats, mut buffs, mut p_stats)) = query.get_single_mut() {
        for event in events.read() {
            match event.powerup_type.as_str() {
                "attack" => {
                    stats.atk += 1;
                    stats.sword_dmg_mult += 0.16;
                }
                "defense" => {
                    stats.def += 1;
                    stats.dmg_taken_mult *= 0.94;
                }
                "dex" => {
                    stats.dex += 1;
                }
                "sta" => {
                    stats.sta += 1;
                    stats.hp_max += 6.0;
                    p_stats.max_hp = stats.hp_max;
                    p_stats.hp += 8.0;
                }
                "int" => {
                    stats.int += 1;
                }
                "haste" => {
                    let dur = 12.0 + (stats.int as f32 * 0.4).min(8.0);
                    buffs.haste.set_duration(Duration::from_secs_f32(dur));
                    buffs.haste.unpause();
                    buffs.haste.reset();
                }
                "longblade" => {
                    let dur = 10.0 + (stats.int as f32 * 0.3).min(7.0);
                    buffs.longblade.set_duration(Duration::from_secs_f32(dur));
                    buffs.longblade.unpause();
                    buffs.longblade.reset();
                }
                "fury" => {
                    let dur = 8.0 + (stats.int as f32 * 0.25).min(6.0);
                    buffs.fury.set_duration(Duration::from_secs_f32(dur));
                    buffs.fury.unpause();
                    buffs.fury.reset();
                }
                "crit_core" => {
                    stats.crit_chance += 0.05;
                    stats.crit_mult += 0.1;
                }
                "critical" => {
                    let dur = 9.0 + (stats.int as f32 * 0.5).min(9.0);
                    buffs.critical.set_duration(Duration::from_secs_f32(dur));
                    buffs.critical.unpause();
                    buffs.critical.reset();
                    stats.crit_chance += 0.1;
                }
                _ => {}
            }
            p_stats.max_hp = stats.hp_max;
            p_stats.hp = p_stats.hp.min(p_stats.max_hp);
        }
    }
}

fn player_death_check(
    mut query: Query<(&mut crate::player::PlayerStats, &mut PlayerProgression), With<Player>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut commands: Commands,
) {
    if let Ok((stats, mut progression)) = query.get_single_mut() {
        if stats.hp <= 0.0 {
            progression.xp *= 0.5; // XP Penalty
            next_state.set(GameState::GameOver);
            commands.insert_resource(GameOverTimer(Timer::from_seconds(10.0, TimerMode::Once)));
        }
    }
}

fn game_over_countdown(
    time: Res<Time>,
    mut timer: ResMut<GameOverTimer>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player_query: Query<(&mut crate::player::PlayerStats, &mut Transform), With<Player>>,
) {
    timer.0.tick(time.delta());
    if timer.0.finished() {
        if let Ok((mut stats, mut tf)) = player_query.get_single_mut() {
            stats.hp = stats.max_hp;
            tf.translation = Vec3::ZERO; // Reset to start
            next_state.set(GameState::Playing);
        }
    }
}

fn update_combat_multipliers(
    mut query: Query<(&PlayerCombatStats, &PlayerSkillBuffs, &PlayerProgression), With<Player>>,
) {
    if let Ok((_stats, _buffs, _progression)) = query.get_single_mut() {
        // Haste buff increases movement speed or similar?
        // In the original, it might affect dexterity or attack cooldowns.
        // For now, let's just show it's integrated by checking if timers are active.
        // let _haste_active = buffs.haste.remaining_secs() > 0.0;
        // let _fury_active = buffs.fury.remaining_secs() > 0.0;
        
        // This system can be expanded to modify PlayerStats components dynamically.
    }
}

fn save_progress_on_level_up(
    mut lv_events: EventReader<PlayerLevelUpEvent>,
    query: Query<(&PlayerProgression, &PlayerCombatStats, &PlayerSkillBuffs), With<Player>>,
) {
    if !lv_events.is_empty() {
        lv_events.clear();
        if let Ok((progression, stats, buffs)) = query.get_single() {
            let data = SaveData {
                level: progression.level,
                xp: progression.xp,
                xp_next: progression.xp_next,
                atk: stats.atk,
                def: stats.def,
                dex: stats.dex,
                sta: stats.sta,
                int: stats.int,
                sword_dmg_mult: stats.sword_dmg_mult,
                dmg_taken_mult: stats.dmg_taken_mult,
                hp_max: stats.hp_max,
                crit_chance: stats.crit_chance,
                crit_mult: stats.crit_mult,
                haste_rem: buffs.haste.remaining_secs(),
                fury_rem: buffs.fury.remaining_secs(),
                longblade_rem: buffs.longblade.remaining_secs(),
                critical_rem: buffs.critical.remaining_secs(),
            };
            if let Ok(json) = serde_json::to_string_pretty(&data) {
                let _ = fs::write("assets/data/save_state.json", json);
            }
        }
    }
}

fn load_progress(
    mut commands: Commands,
) {
    if let Ok(json) = fs::read_to_string("assets/data/save_state.json") {
        if let Ok(data) = serde_json::from_str::<SaveData>(&json) {
            // We'll update the player components when they spawn, 
            // but since they spawn in Startup, we might need a Resource to hold this.
            commands.insert_resource(data);
        }
    }
}
