use bevy::prelude::*;

use super::types::CeilingEntity;

pub(crate) fn setup_ceiling(
    _commands: Commands,
    _meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
}

type PlayerCeilingQuery<'w, 's> = Query<
    'w,
    's,
    (&'static Transform, &'static crate::components::Spatial4D),
    (With<crate::player::Player>, Without<CeilingEntity>),
>;

type CeilingQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut Transform,
        &'static Handle<crate::rendering::CeilingMaterial>,
    ),
    (With<CeilingEntity>, Without<crate::player::Player>),
>;

pub(crate) fn update_ceiling(
    _time: Res<Time>,
    _player_query: PlayerCeilingQuery,
    _ceiling_query: CeilingQuery,
    _materials: ResMut<Assets<crate::rendering::CeilingMaterial>>,
) {
}
