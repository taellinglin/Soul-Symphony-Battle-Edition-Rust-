use bevy::prelude::*;
use bevy_rapier3d::prelude::Velocity;
use crate::player::Player;

pub struct InternalAudioPlugin;

impl Plugin for InternalAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<PlaySfxEvent>()
           .init_resource::<ActiveBgm>()
           .add_systems(Startup, load_audio_assets)
           .add_systems(Update, (handle_sfx_events, manage_player_roll_sound, manage_bgm));
    }
}

#[derive(Event)]
pub struct PlaySfxEvent {
    pub kind: SfxKind,
    pub volume: f32,
    pub pitch: f32,
    pub position: Option<Vec3>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SfxKind {
    Hit,
    Jump,
    WeaponSwing,
    WeaponWarp,
    Pickup,
    Heal,
    MonsterHit,
    MonsterDie,
    LevelUp,
}

#[derive(Resource)]
pub struct GameAudioAssets {
    pub hit: Handle<AudioSource>,
    pub jump: Handle<AudioSource>,
    pub swing: Handle<AudioSource>,
    pub warp: Handle<AudioSource>,
    pub pickup: Handle<AudioSource>,
    pub heal: Handle<AudioSource>,
    pub monster_hit: Handle<AudioSource>,
    pub monster_die: Handle<AudioSource>,
    pub level_up: Handle<AudioSource>,
    pub roll: Handle<AudioSource>,
    pub bgm_exploration: Handle<AudioSource>,
    pub bgm_boss: Handle<AudioSource>,
}

fn load_audio_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    // In Bevy, missing assets will just print a warning and not crash, 
    // which helps if actual files are missing during parity porting.
    let assets = GameAudioAssets {
        hit: asset_server.load("soundfx/monsterhit.wav"),
        jump: asset_server.load("soundfx/qigongjump.wav"),
        swing: asset_server.load("soundfx/attack.wav"),
        warp: asset_server.load("soundfx/warp.wav"),
        pickup: asset_server.load("soundfx/qigongbounce.wav"),
        heal: asset_server.load("soundfx/pickuphealth.wav"),
        monster_hit: asset_server.load("soundfx/monsterhit.wav"),
        monster_die: asset_server.load("soundfx/monsterdie.wav"),
        level_up: asset_server.load("soundfx/levelup_/level_up_01.wav"), 
        roll: asset_server.load("soundfx/water.wav"), 
        bgm_exploration: asset_server.load("bgm/Soundtrack.mp3"),
        bgm_boss: asset_server.load("bgm/Boss.mp3"),
    };
    commands.insert_resource(assets);
}

fn handle_sfx_events(
    mut commands: Commands,
    mut events: EventReader<PlaySfxEvent>,
    assets: Option<Res<GameAudioAssets>>,
) {
    let Some(assets) = assets else { return };
    for ev in events.read() {
        let source = match ev.kind {
            SfxKind::Hit => assets.hit.clone(),
            SfxKind::Jump => assets.jump.clone(),
            SfxKind::WeaponSwing => assets.swing.clone(),
            SfxKind::WeaponWarp => assets.warp.clone(),
            SfxKind::Pickup => assets.pickup.clone(),
            SfxKind::Heal => assets.heal.clone(),
            SfxKind::MonsterHit => assets.monster_hit.clone(),
            SfxKind::MonsterDie => assets.monster_die.clone(),
            SfxKind::LevelUp => assets.level_up.clone(),
        };

        let mut ent = commands.spawn((
            AudioSourceBundle {
                source,
                settings: PlaybackSettings {
                    volume: bevy::audio::Volume::new(ev.volume),
                    speed: ev.pitch,
                    mode: bevy::audio::PlaybackMode::Despawn,
                    ..default()
                },
            },
        ));

        if let Some(pos) = ev.position {
            ent.insert(TransformBundle::from_transform(Transform::from_translation(pos)));
        }
    }
}

// Marker for the rolling sound entity attached to the player
#[derive(Component)]
pub struct PlayerRollSound;

fn manage_player_roll_sound(
    mut commands: Commands,
    player_query: Query<(&Transform, &Velocity), With<Player>>,
    mut sound_query: Query<(&mut Transform, &mut AudioSink), (With<PlayerRollSound>, Without<Player>)>,
    assets: Option<Res<GameAudioAssets>>,
) {
    let Ok((player_tf, player_vel)) = player_query.get_single() else { return };
    let Some(assets) = assets else { return };

    let speed = player_vel.linvel.length();
    let target_volume = if speed > 1.0 { (speed / 15.0).clamp(0.0, 0.8) } else { 0.0 };
    let target_pitch = 0.8 + (speed / 20.0).clamp(0.0, 1.2);

    if sound_query.is_empty() {
        // Spawn it if it doesn't exist yet
        commands.spawn((
            PlayerRollSound,
            AudioSourceBundle {
                source: assets.roll.clone(),
                settings: PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::new(0.0),
                    ..default()
                },
            },
            TransformBundle::from_transform(*player_tf),
        ));
    } else {
        for (mut tf, sink) in sound_query.iter_mut() {
            tf.translation = player_tf.translation;
            let current_vol = sink.volume();
            // Smooth volume
            sink.set_volume(current_vol + (target_volume - current_vol) * 0.1);
            sink.set_speed(target_pitch);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BGM Management
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default, PartialEq)]
pub enum BgmType {
    #[default]
    None,
    Exploration,
    Boss,
}

#[derive(Resource, Default)]
pub struct ActiveBgm(pub BgmType);

#[derive(Component)]
pub struct MainBgm;

fn manage_bgm(
    mut commands: Commands,
    assets: Option<Res<GameAudioAssets>>,
    boss_state: Option<Res<crate::world::boss::BossArenaState>>,
    mut active_bgm: ResMut<ActiveBgm>,
    bgm_query: Query<(Entity, &AudioSink), With<MainBgm>>,
) {
    let Some(assets) = assets else { return };
    
    let is_boss = boss_state.map_or(false, |s| s.is_active);
    let target_type = if is_boss { BgmType::Boss } else { BgmType::Exploration };

    if active_bgm.0 != target_type {
        // Stop current
        for (entity, _) in bgm_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        
        // Start new
        let source = if target_type == BgmType::Boss {
            assets.bgm_boss.clone()
        } else {
            assets.bgm_exploration.clone()
        };

        commands.spawn((
            MainBgm,
            AudioSourceBundle {
                source,
                settings: PlaybackSettings {
                    mode: bevy::audio::PlaybackMode::Loop,
                    volume: bevy::audio::Volume::new(0.45),
                    ..default()
                },
            },
        ));
        
        active_bgm.0 = target_type;
    }
}
