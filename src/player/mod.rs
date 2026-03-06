mod components;
mod spawn;
mod physics;
mod camera;

// Re-export all public types
pub use components::*;

use bevy::prelude::*;

use crate::components::{PlayerVisuals};
use bevy::sprite::Sprite;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GravityDirection>()
           .init_resource::<CameraOrbitState>()
           .init_resource::<HyperspaceState>()
           .init_resource::<JumpState>()
           .init_resource::<PhysicsTimers>()
           .init_resource::<PrevBallState>()
           .add_systems(Startup, (
               spawn::spawn_player,
               spawn::teleport_to_start.after(crate::map::generate_dungeon),
           ))
           .add_systems(Update, (
                physics::player_physics_controller,
                physics::apply_hyperspace_physics,
                physics::apply_jump_float_drag,
                physics::apply_compression_physics,
                physics::apply_speed_clamping,
                physics::anti_tunneling_system,
                physics::ball_contact_analysis,
                physics::update_compression_factor,
                physics::w_dimension_shift,
                physics::sync_collision_groups,
                camera::sync_camera,
                update_billboard_ui,
                update_local_player_bars,
                emit_floor_pulses,
                update_player_visuals,
           ).chain());
    }
}

fn update_player_visuals(
    mut query: Query<(&mut PlayerVisuals, &Handle<crate::rendering::BallMaterial>), With<Player>>,
    mut materials: ResMut<Assets<crate::rendering::BallMaterial>>,
    time: Res<Time>,
) {
    let Ok((mut visuals, mat_handle)) = query.get_single_mut() else { return };
    let t = time.elapsed_seconds();

    visuals.cycle_timer = t;

    if let Some(material) = materials.get_mut(mat_handle) {
        material.extension.settings.time = t;
        // Python parity: slow color cycle
        material.extension.settings.hue_shift = (t * 0.15).rem_euclid(1.0);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Billboard UI Systems
// ─────────────────────────────────────────────────────────────────────────────
pub fn update_billboard_ui(
    camera_query: Query<&Transform, With<PlayerCamera>>,
    mut ui_query: Query<&mut Transform, (With<Sprite>, Without<PlayerCamera>)>,
    mut text_query: Query<&mut Transform, (With<crate::effects::FloatingText>, Without<PlayerCamera>, Without<Sprite>)>,
) {
    if let Ok(camera_tf) = camera_query.get_single() {
        for mut tf in ui_query.iter_mut() {
            tf.rotation = camera_tf.rotation;
        }
        for mut tf in text_query.iter_mut() {
            tf.rotation = camera_tf.rotation;
        }
    }
}

pub fn update_local_player_bars(
    player_query: Query<(&PlayerStats, &crate::systems::progression::PlayerProgression), With<Player>>,
    mut hp_query: Query<&mut Transform, (With<LocalPlayerHpBar>, Without<LocalPlayerXpBar>)>,
    mut xp_query: Query<&mut Transform, (With<LocalPlayerXpBar>, Without<LocalPlayerHpBar>)>,
) {
    if let Ok((stats, prog)) = player_query.get_single() {
        let hp_ratio = (stats.hp / stats.max_hp.max(1.0)).clamp(0.0, 1.0);
        let xp_ratio = (prog.xp / prog.xp_next.max(1.0)).clamp(0.0, 1.0);
        for mut t in hp_query.iter_mut() {
            t.scale.x = hp_ratio;
        }
        for mut t in xp_query.iter_mut() {
            t.scale.x = xp_ratio;
        }
    }
}

pub fn emit_floor_pulses() {}
