use bevy::prelude::*;

use super::types::CeilingEntity;

pub(crate) fn setup_ceiling(
    _commands: Commands,
    _meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
}

pub(crate) fn update_ceiling(
    _time: Res<Time>,
    _player_query: Query<(&Transform, &crate::components::Spatial4D), (With<crate::player::Player>, Without<CeilingEntity>)>,
    _ceiling_query: Query<(&mut Transform, &Handle<crate::rendering::CeilingMaterial>), (With<CeilingEntity>, Without<crate::player::Player>)>,
    _materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
}
