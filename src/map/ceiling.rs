use bevy::prelude::*;

use super::types::CeilingEntity;

pub(crate) fn setup_ceiling(
    _commands: Commands,
    _meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
    // Original parity uses an inverted level echo (mirrored world) rather than a bespoke ceiling plane.
    // The mirrored look is approximated by spawning a second water surface above in `map/mod.rs`,
    // so we disable the standalone ceiling plane to avoid conflicting visuals.
    // Intentionally no-op.
}

pub(crate) fn update_ceiling(
    time: Res<Time>,
    player_query: Query<(&Transform, &crate::components::Spatial4D), (With<crate::player::Player>, Without<CeilingEntity>)>,
    mut ceiling_query: Query<(&mut Transform, &Handle<crate::rendering::CeilingMaterial>), (With<CeilingEntity>, Without<crate::player::Player>)>,
    mut materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
    let Ok((player_tf, spatial)) = player_query.get_single() else { return };
    let Ok((mut ceiling_tf, mat_handle)) = ceiling_query.get_single_mut() else { return };
    
    // Follow player in XZ
    ceiling_tf.translation.x = player_tf.translation.x;
    ceiling_tf.translation.z = player_tf.translation.z;
    
    if let Some(mat) = materials.get_mut(mat_handle) {
        mat.extension.settings.time = time.elapsed_seconds();
        mat.extension.settings.player_w = spatial.w;
    }
}
