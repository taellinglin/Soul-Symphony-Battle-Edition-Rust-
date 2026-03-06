use bevy::prelude::*;

use super::types::CeilingEntity;

pub(crate) fn setup_ceiling(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
    /* // Enabled for mirrored parity in arena mode
    if config.layout_mode == "arena" {
        return;
    }
    */
    commands.spawn((
        CeilingEntity,
        MaterialMeshBundle {
            mesh: meshes.add(Plane3d::default().mesh().size(2500.0, 2500.0)),
            material: materials.add(crate::rendering::CeilingMaterial {
                base: StandardMaterial {
                    base_color: Color::srgb(0.1, 0.1, 0.12),
                    unlit: true,
                    cull_mode: None, // Visible from below
                    ..default()
                },
                extension: crate::rendering::CeilingExtension {
                    settings: crate::rendering::CeilingSettings::default(),
                },
            }),
            transform: Transform::from_xyz(0.0, 50.0, 0.0).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
            ..default()
        },
    ));
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
