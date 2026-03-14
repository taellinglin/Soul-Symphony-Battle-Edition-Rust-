use bevy::prelude::*;

#[derive(Resource)]
pub struct HoloMapUpdateTimer {
    pub cooldown: f32,
}

impl Default for HoloMapUpdateTimer {
    fn default() -> Self {
        Self { cooldown: 0.0 }
    }
}

#[derive(Resource)]
pub struct NetworkUiUpdateTimer {
    pub cooldown: f32,
}

impl Default for NetworkUiUpdateTimer {
    fn default() -> Self {
        Self { cooldown: 0.2 }
    }
}

#[derive(Component)]
pub struct HpBarFill;

#[derive(Component)]
pub struct HpBarGlow;

#[derive(Component)]
pub struct XpBarFill;

#[derive(Component)]
pub struct XpBarGlow;

#[derive(Component)]
pub struct MonsterHudText;

#[derive(Component)]
pub struct LevelHudText;

#[derive(Component)]
pub struct HpLabelText;

#[derive(Component)]
pub struct HoloMapRoot;

#[derive(Component)]
pub struct HoloMapClipper;

#[derive(Component)]
pub struct HoloMapTitle;

#[derive(Component)]
pub struct MiniMapDot {
    pub _entity_ref: Entity,
}

#[derive(Component)]
pub struct GameOverUi;

#[derive(Component)]
pub struct WinUi;

#[derive(Component)]
pub struct NetworkMenuUi;

#[derive(Component)]
pub struct BossRoomText;

#[derive(Component)]
pub struct InputHudRoot;

#[derive(Component)]
pub struct HudKeyNode(pub KeyCode);

#[derive(Component)]
pub struct HudMouseNode(pub MouseButton);
