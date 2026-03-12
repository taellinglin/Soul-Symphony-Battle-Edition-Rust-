mod components;
mod hud;
mod minimap;

pub use components::*;

use bevy::prelude::*;
use crate::player::{PlayerStats, Player};
use crate::systems::progression::{PlayerProgression, GameState};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GameOverCountdown>()
           .init_resource::<HoloMapUpdateTimer>()
           .init_resource::<NetworkUiUpdateTimer>()
           .add_systems(Startup, (
            hud::setup_ui, setup_game_over_ui, 
            setup_win_ui, setup_network_menu, setup_client_list_ui, 
            setup_network_debug_ui
        ))
           .add_systems(Update, (
               hud::update_ui, minimap::update_minimap,
               update_game_over_ui_visibility,
               update_client_list_ui, update_network_debug_ui,
               update_game_over_countdown, hud::update_monster_hud_text,
               hud::update_boss_room_ui, hud::update_input_hud
           ));
    }
}

#[derive(Component)]
pub struct GameOverPrompt;

#[derive(Resource)]
pub struct GameOverCountdown {
    pub active: bool,
    pub timer: f32,
}

impl Default for GameOverCountdown {
    fn default() -> Self {
        Self { active: false, timer: 10.0 }
    }
}




fn setup_game_over_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.78)),
            visibility: Visibility::Hidden,
            ..default()
        },
        GameOverUi,
    )).with_children(|root| {
        root.spawn(TextBundle::from_section(
            "GAME OVER",
            TextStyle {
                font: asset_server.load("fonts/Mine.ttf"), font_size: 80.0,
                color: Color::srgba(0.96, 0.2, 0.24, 0.98),
                ..default()
            },
        ).with_style(Style { margin: UiRect::bottom(Val::Px(20.0)), ..default() }));

        root.spawn((
            TextBundle::from_section(
                "Press R to restart",
                TextStyle {
                    font: asset_server.load("fonts/Mine.ttf"), font_size: 40.0,
                    color: Color::srgba(0.92, 0.97, 1.0, 0.92),
                    ..default()
                },
            ),
            GameOverPrompt,
        ));
    });
}

fn update_game_over_ui_visibility(
    state: Res<State<GameState>>,
    mut query: Query<&mut Visibility, With<GameOverUi>>,
) {
    let visible = *state.get() == GameState::GameOver;
    for mut vis in query.iter_mut() {
        *vis = if visible { Visibility::Visible } else { Visibility::Hidden };
    }
}

fn update_game_over_countdown(
    time: Res<Time>,
    mut countdown: ResMut<GameOverCountdown>,
    mut prompt_query: Query<&mut Text, With<GameOverPrompt>>,
) {
    if !countdown.active { return; }
    let dt = time.delta_seconds();
    countdown.timer = (countdown.timer - dt).max(0.0);
    let secs_left = countdown.timer.ceil() as i32;

    if let Ok(mut text) = prompt_query.get_single_mut() {
        text.sections[0].value = format!("Press R to restart\nAuto replay AI in {}", secs_left);
    }
}

fn setup_win_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.02, 0.06, 0.04, 0.78)),
            visibility: Visibility::Hidden,
            ..default()
        },
        WinUi,
    )).with_children(|root| {
        root.spawn(TextBundle::from_section(
            "YOU WIN",
            TextStyle {
                font: asset_server.load("fonts/Mine.ttf"), font_size: 80.0,
                color: Color::srgba(0.38, 1.0, 0.62, 0.98),
                ..default()
            },
        ).with_style(Style { margin: UiRect::bottom(Val::Px(20.0)), ..default() }));

        root.spawn(TextBundle::from_section(
            "Press R to restart",
            TextStyle {
                font: asset_server.load("fonts/Mine.ttf"), font_size: 40.0,
                color: Color::srgba(0.92, 0.97, 1.0, 0.92),
                ..default()
            },
        ));
    });
}

fn setup_network_menu(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                right: Val::Px(300.0),
                width: Val::Px(300.0),
                height: Val::Px(200.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(15.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.02, 0.05, 0.08, 0.92)),
            visibility: Visibility::Hidden,
            ..default()
        },
        NetworkMenuUi,
    )).with_children(|root| {
        root.spawn(TextBundle::from_section(
            "NETWORK",
            TextStyle {
                font: asset_server.load("fonts/Mine.ttf"), font_size: 24.0,
                color: Color::srgba(0.6, 0.95, 1.0, 1.0),
                ..default()
            },
        ).with_style(Style { margin: UiRect::bottom(Val::Px(10.0)), ..default() }));
    });
}

#[derive(Component)]
pub struct ClientListUi;

#[derive(Component)]
pub struct ClientListText;

fn setup_client_list_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(85.0),
                right: Val::Px(20.0),
                width: Val::Px(380.0),
                height: Val::Px(300.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::srgba(0.02, 0.05, 0.08, 0.55)),
            visibility: Visibility::Hidden,
            ..default()
        },
        ClientListUi,
    )).with_children(|root| {
        root.spawn((
            TextBundle::from_section(
                "CLIENTS",
                TextStyle {
                    font: asset_server.load("fonts/Mine.ttf"), font_size: 16.0,
                    color: Color::srgba(0.82, 0.95, 1.0, 0.95),
                    ..default()
                },
            ).with_text_justify(JustifyText::Right),
            ClientListText,
        ));
    });
}

#[derive(Component)]
pub struct NetworkDebugUi;

#[derive(Component)]
pub struct NetworkDebugText;

fn setup_network_debug_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                top: Val::Px(20.0),
                right: Val::Px(20.0),
                width: Val::Px(200.0),
                height: Val::Px(60.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(5.0)),
                ..default()
            },
            background_color: BackgroundColor(Color::NONE),
            visibility: Visibility::Hidden,
            ..default()
        },
        NetworkDebugUi,
    )).with_children(|root| {
        root.spawn((
            TextBundle::from_section(
                "PING: --\nLAST MSG: --",
                TextStyle {
                    font: asset_server.load("fonts/Mine.ttf"), font_size: 18.0,
                    color: Color::srgba(0.75, 0.9, 1.0, 0.9),
                    ..default()
                },
            ).with_text_justify(JustifyText::Right),
            NetworkDebugText,
        ));
    });
}

fn update_client_list_ui(
    time: Res<Time>,
    mut timer: ResMut<NetworkUiUpdateTimer>,
    player_query: Query<(&PlayerStats, &PlayerProgression), With<Player>>,
    mut text_query: Query<&mut Text, With<ClientListText>>,
) {
    let dt = time.delta_seconds();
    timer.cooldown -= dt;
    if timer.cooldown > 0.0 { return; }
    timer.cooldown = 0.2;

    if let Ok((stats, level)) = player_query.get_single() {
        if let Ok(mut text) = text_query.get_single_mut() {
            let mut lines = vec!["CLIENTS".to_string()];
            let local_name = "Player";
            
            lines.push(format!(
                "{}  Lv {}  HP {:.0}/{:.0}  XP {:.0}/{:.0}",
                local_name, level.level,
                stats.hp, stats.max_hp,
                level.xp, level.xp_next
            ));
            
            // Mocking remote connections until networking is fully implemented
            // for remote in remote_players.values() { ... }
            
            text.sections[0].value = lines.join("\n");
        }
    }
}

fn update_network_debug_ui(
    mut query: Query<&mut Text, With<NetworkDebugText>>,
    timer: Res<NetworkUiUpdateTimer>,
) {
    if timer.cooldown > 0.0 { return; }
    for mut text in query.iter_mut() {
        text.sections[0].value = "NET: Offline\nlast: N/A".to_string();
    }
}
