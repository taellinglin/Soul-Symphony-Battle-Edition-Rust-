use crate::player::PlayerStats;
use crate::systems::progression::PlayerProgression;
use bevy::prelude::*;

use super::components::*;

type BarQueryResult = (&'static mut Style, &'static mut BackgroundColor);
type GlowQueryResult = (&'static mut Transform, &'static mut BackgroundColor);

type BarQuery<'w, 's, F1, F2, F3, F4> =
    Query<'w, 's, BarQueryResult, (With<F1>, Without<F2>, Without<F3>, Without<F4>)>;

type GlowQuery<'w, 's, F1, F2, F3, F4> =
    Query<'w, 's, GlowQueryResult, (With<F1>, Without<F2>, Without<F3>, Without<F4>)>;

type HudTextQuery<'w, 's, F1, F2, F3> =
    Query<'w, 's, &'static mut Text, (With<F1>, Without<F2>, Without<F3>)>;

#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct HudUpdateParams<'w, 's> {
    time: Res<'w, Time>,
    player_query: Query<'w, 's, (&'static PlayerStats, &'static PlayerProgression)>,
    hp_bar_query: BarQuery<'w, 's, HpBarFill, XpBarFill, HpBarGlow, XpBarGlow>,
    xp_bar_query: BarQuery<'w, 's, XpBarFill, HpBarFill, XpBarGlow, HpBarGlow>,
    hp_glow_query: GlowQuery<'w, 's, HpBarGlow, HpBarFill, XpBarFill, XpBarGlow>,
    xp_glow_query: GlowQuery<'w, 's, XpBarGlow, XpBarFill, HpBarFill, HpBarGlow>,
    level_text_query: HudTextQuery<'w, 's, LevelHudText, HoloMapTitle, HpLabelText>,
    hp_label_query: HudTextQuery<'w, 's, HpLabelText, LevelHudText, HoloMapTitle>,
    title_query: HudTextQuery<'w, 's, HoloMapTitle, LevelHudText, HpLabelText>,
}

pub(crate) fn setup_ui(mut commands: Commands, asset_server: Res<AssetServer>) {
    // UI is now rendered via the main 3D camera to solve overlay conflicts
    // and ensure the CRT effect applies to the UI as well.
    // Bevy 0.14 handles UI on the main camera by default.

    // Root node
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            ..default()
        })
        .with_children(|root| {
            // --- TOP CENTER: Boss Room Indicator ---
            root.spawn(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(100.0),
                    left: Val::Percent(50.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|parent| {
                parent.spawn((
                    TextBundle::from_section(
                        "BOSS ROOM",
                        TextStyle {
                            font: asset_server.load("fonts/Mine.ttf"),
                            font_size: 48.0,
                            color: Color::srgba(1.0, 0.1, 0.1, 0.0), // Initially transparent
                        },
                    )
                    .with_style(Style {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-120.0), // Center approximation
                        ..default()
                    }),
                    BossRoomText,
                ));
            });

            // --- BOTTOM LEFT: Controls HUD (Graphical) ---
            root.spawn((
                NodeBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        bottom: Val::Px(24.0),
                        left: Val::Px(24.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::End,
                        ..default()
                    },
                    ..default()
                },
                InputHudRoot,
            ))
            .with_children(|parent| {
                // WASD Grid
                parent
                    .spawn(NodeBundle {
                        style: Style {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            margin: UiRect::right(Val::Px(64.0)), // Wider gap
                            ..default()
                        },
                        ..default()
                    })
                    .with_children(|grid| {
                        // Top row: W
                        grid.spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Row,
                                margin: UiRect::bottom(Val::Px(4.0)),
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_hud_button(row, "W", KeyCode::KeyW, &asset_server);
                        });

                        // Bottom row: ASD
                        grid.spawn(NodeBundle {
                            style: Style {
                                flex_direction: FlexDirection::Row,
                                ..default()
                            },
                            ..default()
                        })
                        .with_children(|row| {
                            spawn_hud_button(row, "A", KeyCode::KeyA, &asset_server);
                            spawn_hud_button(row, "S", KeyCode::KeyS, &asset_server);
                            spawn_hud_button(row, "D", KeyCode::KeyD, &asset_server);
                        });
                    });

                // Mouse Icon
                parent
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(120.0), // Tuned down more from 150
                            height: Val::Px(120.0),
                            flex_direction: FlexDirection::Column,
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.12, 0.16, 0.20, 0.85)), // Darker/solid bg
                        ..default()
                    })
                    .with_children(|mouse| {
                        // Top buttons row (L, M, R)
                        mouse
                            .spawn(NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(40.0), // Shorter buttons
                                    flex_direction: FlexDirection::Row,
                                    justify_content: JustifyContent::SpaceBetween,
                                    ..default()
                                },
                                ..default()
                            })
                            .with_children(|buttons| {
                                spawn_mouse_button(
                                    buttons,
                                    "L",
                                    MouseButton::Left,
                                    &asset_server,
                                    Val::Percent(30.0),
                                    true,
                                );
                                spawn_mouse_button(
                                    buttons,
                                    "M",
                                    MouseButton::Middle,
                                    &asset_server,
                                    Val::Percent(30.0),
                                    true,
                                );
                                spawn_mouse_button(
                                    buttons,
                                    "R",
                                    MouseButton::Right,
                                    &asset_server,
                                    Val::Percent(30.0),
                                    true,
                                );
                            });

                        // Body filler (empty solid body)
                        mouse.spawn(NodeBundle {
                            style: Style {
                                width: Val::Percent(100.0),
                                height: Val::Percent(60.0),
                                ..default()
                            },
                            ..default()
                        });
                    });
            });

            // --- TOP LEFT: HP and XP ---
            root.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(350.0), // Tuned down more from 380
                    margin: UiRect::all(Val::Px(24.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|top_left| {
                // HP label
                top_left.spawn((
                    TextBundle::from_section(
                        "HP",
                        TextStyle {
                            font: Handle::default(),
                            font_size: 20.0, // Tuned down more from 24
                            color: Color::WHITE,
                        },
                    )
                    .with_style(Style {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..default()
                    }),
                    HpLabelText,
                ));

                // HP Bar Container
                top_left
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(320.0), // Tuned down more from 350
                            height: Val::Px(24.0), // Tuned down from 28
                            margin: UiRect::bottom(Val::Px(4.0)),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.05, 0.07, 0.1, 0.86)),
                        ..default()
                    })
                    .with_children(|bar_bg| {
                        // Glow
                        bar_bg.spawn((
                            NodeBundle {
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(-2.0),
                                    top: Val::Px(-2.0),
                                    right: Val::Px(-2.0),
                                    bottom: Val::Px(-2.0),
                                    ..default()
                                },
                                background_color: BackgroundColor(Color::srgba(
                                    0.32, 0.95, 1.0, 0.24,
                                )),
                                ..default() // Need additive blending natively but Bevy UI doesn't expose it easily. Approximation via alpha.
                            },
                            HpBarGlow,
                        ));

                        // Fill
                        bar_bg.spawn((
                            NodeBundle {
                                style: Style {
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                background_color: BackgroundColor(Color::srgba(
                                    0.24, 0.95, 0.54, 0.96,
                                )),
                                ..default()
                            },
                            HpBarFill,
                        ));
                    });

                // XP label
                top_left.spawn((
                    TextBundle::from_section(
                        "LV 1 Exp",
                        TextStyle {
                            font: Handle::default(),
                            font_size: 16.0, // Tuned down more from 20
                            color: Color::srgba(0.86, 0.96, 1.0, 0.95),
                        },
                    )
                    .with_style(Style {
                        margin: UiRect::top(Val::Px(10.0)),
                        ..default()
                    }),
                    LevelHudText,
                ));

                // XP Bar Container
                top_left
                    .spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(320.0), // Tuned down more from 350
                            height: Val::Px(12.0), // Tuned down more from 14
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.82)),
                        ..default()
                    })
                    .with_children(|bar_bg| {
                        // Glow
                        bar_bg.spawn((
                            NodeBundle {
                                style: Style {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(-1.5),
                                    top: Val::Px(-1.5),
                                    right: Val::Px(-1.5),
                                    bottom: Val::Px(-1.5),
                                    ..default()
                                },
                                background_color: BackgroundColor(Color::srgba(
                                    0.25, 0.9, 1.0, 0.18,
                                )),
                                ..default()
                            },
                            XpBarGlow,
                        ));

                        // Fill
                        bar_bg.spawn((
                            NodeBundle {
                                style: Style {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                background_color: BackgroundColor(Color::srgba(
                                    0.68, 0.18, 0.12, 0.92,
                                )),
                                ..default()
                            },
                            XpBarFill,
                        ));
                    });
            });

            // --- TOP RIGHT: Monster HUD ---
            root.spawn(NodeBundle {
                style: Style {
                    margin: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                ..default()
            })
            .with_children(|top_right| {
                top_right.spawn((
                    TextBundle::from_section(
                        "Slayed: 0\nLeft: 0",
                        TextStyle {
                            font: asset_server.load("fonts/Mine.ttf"),
                            font_size: 24.0, // Tuned down more from 30
                            color: Color::srgba(0.95, 0.98, 1.0, 0.95),
                        },
                    )
                    .with_text_justify(JustifyText::Right),
                    MonsterHudText,
                ));
            });
        });

    // --- BOTTOM RIGHT: Holo Map ---
    // --- BOTTOM RIGHT: Holo Map Container ---
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(40.0),
                    right: Val::Px(40.0),
                    width: Val::Px(220.0), // Tuned down more from 300
                    height: Val::Px(220.0),
                    ..default()
                },
                ..default()
            },
            HoloMapRoot, // Container for overall scaling/pulsing
        ))
        .with_children(|container| {
            // Holographic Title (Outside the clipper)
            container.spawn((
                TextBundle::from_section(
                    "HOLO MAP",
                    TextStyle {
                        font: asset_server.load("fonts/Mine.ttf"),
                        font_size: 18.0, // Tuned down more from 20
                        color: Color::srgba(0.58, 0.98, 1.0, 0.94),
                    },
                )
                .with_style(Style {
                    position_type: PositionType::Absolute,
                    top: Val::Px(-24.0),
                    right: Val::Px(0.0),
                    ..default()
                }),
                HoloMapTitle,
            ));

            // Circular Clipper
            container
                .spawn((
                    NodeBundle {
                        style: Style {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            border: UiRect::all(Val::Px(1.5)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.3, 0.8, 0.7, 0.15)), // Stronger cyan tint
                        border_color: BorderColor(Color::srgba(0.5, 1.0, 0.9, 0.4)), // Bright teal/cyan border
                        ..default()
                    },
                    HoloMapClipper,
                ))
                .with_children(|parent| {
                    // Crosshairs
                    parent.spawn(NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            top: Val::Percent(50.0),
                            left: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Px(1.0),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.25, 0.85, 1.0, 0.24)),
                        ..default()
                    });
                    parent.spawn(NodeBundle {
                        style: Style {
                            position_type: PositionType::Absolute,
                            top: Val::Px(0.0),
                            left: Val::Percent(50.0),
                            width: Val::Px(1.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        background_color: BackgroundColor(Color::srgba(0.25, 0.85, 1.0, 0.24)),
                        ..default()
                    });
                });
        });
}

pub(crate) fn update_ui(mut params: HudUpdateParams) {
    let t = params.time.elapsed_seconds();
    let hue_shift = (t * 0.42) % 1.0;
    let pulse = 0.5 + 0.5 * (t * 7.2).sin();

    if let Ok((stats, level)) = params.player_query.get_single() {
        if let Ok((mut style, mut bg)) = params.hp_bar_query.get_single_mut() {
            let ratio = (stats.hp / stats.max_hp.max(1e-6)).clamp(0.0, 1.0);
            style.width = Val::Percent(ratio * 100.0);

            let health_hue = (0.0 + ratio * 0.33 + hue_shift * 0.08) % 1.0;
            let hsv = Hsva::new(health_hue * 360.0, 0.88, 1.0, 0.88 + 0.1 * pulse);
            bg.0 = Color::from(hsv);
        }

        if let Ok((mut transform, mut bg)) = params.hp_glow_query.get_single_mut() {
            let glow_hue = (hue_shift + 0.52) % 1.0;
            let hsv = Hsva::new(glow_hue * 360.0, 0.72, 1.0, 0.16 + 0.22 * pulse);
            bg.0 = Color::from(hsv);
            transform.scale = Vec3::new(1.0 + 0.015 * pulse, 1.0, 1.0);
        }

        if let Ok((mut style, mut bg)) = params.xp_bar_query.get_single_mut() {
            let xp_pct = (level.xp / level.xp_next.max(1e-6)).clamp(0.0, 1.0);
            style.width = Val::Percent(xp_pct * 100.0);

            let xp_hue = (t * 0.35 + 0.55) % 1.0;
            let hsv = Hsva::new(xp_hue * 360.0, 0.85, 1.0, 0.9);
            bg.0 = Color::from(hsv);
        }

        if let Ok((mut transform, mut bg)) = params.xp_glow_query.get_single_mut() {
            let glow_hue = (t * 0.22 + 0.82) % 1.0;
            let hsv = Hsva::new(glow_hue * 360.0, 0.7, 1.0, 0.12 + 0.18 * pulse);
            bg.0 = Color::from(hsv);
            transform.scale = Vec3::new(1.0 + 0.012 * pulse, 1.0, 1.0);
        }

        if let Ok(mut text) = params.hp_label_query.get_single_mut() {
            let label_hue = (hue_shift + 0.12) % 1.0;
            let hsv = Hsva::new(label_hue * 360.0, 0.45, 1.0, 0.96);
            text.sections[0].style.color = Color::from(hsv);
        }

        if let Ok(mut text) = params.level_text_query.get_single_mut() {
            text.sections[0].value = format!("LV {} XP", level.level);
            text.sections[0].style.color = Color::srgba(0.86, 0.96, 1.0, 0.92);
        }

        if let Ok(mut text) = params.title_query.get_single_mut() {
            text.sections[0].style.color = Color::srgba(0.58, 0.98, 1.0, 0.94);
        }
    }
}

// Python parity: _update_monster_hud_ui dynamically updates slain/left counter
pub(crate) fn update_monster_hud_text(
    mob_stats: Res<crate::systems::progression::MonsterStats>,
    mut text_query: Query<&mut Text, With<MonsterHudText>>,
) {
    if let Ok(mut text) = text_query.get_single_mut() {
        text.sections[0].value = format!(
            "Slayed: {}\nLeft: {}",
            mob_stats.slain,
            mob_stats.total.saturating_sub(mob_stats.slain)
        );
    }
}

pub(crate) fn update_boss_room_ui(
    boss_state: Res<crate::world::boss::BossArenaState>,
    mut query: Query<&mut Text, With<BossRoomText>>,
) {
    if let Ok(mut text) = query.get_single_mut() {
        let alpha = if boss_state.is_active { 0.95 } else { 0.0 };
        text.sections[0].style.color.set_alpha(alpha);
    }
}

pub(crate) fn update_input_hud(
    mut query: Query<&mut Visibility, With<InputHudRoot>>,
    mut key_nodes: Query<
        (
            &HudKeyNode,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        Without<HudMouseNode>,
    >,
    mut mouse_nodes: Query<
        (
            &HudMouseNode,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        Without<HudKeyNode>,
    >,
    mut text_query: Query<&mut Text>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    time: Res<Time>,
) {
    if keyboard.just_pressed(KeyCode::Tab) {
        if let Ok(mut vis) = query.get_single_mut() {
            *vis = if *vis == Visibility::Visible {
                Visibility::Hidden
            } else {
                Visibility::Visible
            };
        }
    }

    let t = time.elapsed_seconds();
    let pulse = 0.5 + 0.5 * (t * 5.0).sin();

    // Update keys
    for (node, mut bg, mut border, children) in key_nodes.iter_mut() {
        let pressed = keyboard.pressed(node.0);
        if pressed {
            bg.0 = Color::srgba(0.2, 0.45, 0.6, 0.92);
            border.0 = Color::srgba(0.4, 0.9, 1.0, 0.8 + 0.2 * pulse);
        } else {
            bg.0 = Color::srgba(0.06, 0.08, 0.12, 0.64);
            border.0 = Color::srgba(0.25, 0.85, 1.0, 0.22);
        }

        for &child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                text.sections[0].style.color = if pressed {
                    Color::WHITE
                } else {
                    Color::srgba(0.9, 0.94, 1.0, 0.92)
                };
            }
        }
    }

    // Update mouse
    for (node, mut bg, mut border, children) in mouse_nodes.iter_mut() {
        let pressed = mouse_input.pressed(node.0);
        if pressed {
            bg.0 = Color::srgba(0.2, 0.45, 0.6, 0.92);
            border.0 = Color::srgba(0.4, 0.9, 1.0, 0.8 + 0.2 * pulse);
        } else {
            bg.0 = Color::srgba(0.06, 0.08, 0.12, 0.64);
            border.0 = Color::srgba(0.25, 0.85, 1.0, 0.22);
        }

        for &child in children.iter() {
            if let Ok(mut text) = text_query.get_mut(child) {
                text.sections[0].style.color = if pressed {
                    Color::WHITE
                } else {
                    Color::srgba(0.9, 0.94, 1.0, 0.92)
                };
            }
        }
    }
}

pub(crate) fn spawn_hud_button(
    parent: &mut ChildBuilder,
    label_text: &str,
    key: KeyCode,
    asset_server: &Res<AssetServer>,
) {
    parent
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Px(48.0), // Tuned down more from 52
                    height: Val::Px(48.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::all(Val::Px(2.0)),
                    border: UiRect::all(Val::Px(2.5)),
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.08, 0.11, 0.16, 0.75)),
                border_color: BorderColor(Color::srgba(0.2, 0.35, 0.45, 0.8)),
                ..default()
            },
            HudKeyNode(key),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label_text,
                TextStyle {
                    font: asset_server.load("fonts/Mine.ttf"),
                    font_size: 20.0, // Tuned down more from 26
                    color: Color::srgba(0.9, 0.94, 1.0, 0.92),
                },
            ));
        });
}

fn spawn_mouse_button(
    parent: &mut ChildBuilder,
    label_text: &str,
    button: MouseButton,
    asset_server: &Res<AssetServer>,
    width: Val,
    _is_mouse: bool, // Just to change signature, not strictly needed but matches styling below
) {
    parent
        .spawn((
            NodeBundle {
                style: Style {
                    width,
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                background_color: BackgroundColor(Color::srgba(0.06, 0.08, 0.12, 0.85)), // Darker bg for mouse buttons
                border_color: BorderColor(Color::NONE), // No outer border for mouse buttons inside the body
                ..default()
            },
            HudMouseNode(button),
        ))
        .with_children(|btn| {
            btn.spawn(TextBundle::from_section(
                label_text,
                TextStyle {
                    font: asset_server.load("fonts/Mine.ttf"),
                    font_size: 16.0,     // Tuned down more from 20
                    color: Color::WHITE, // Whiter text
                },
            ));
        });
}
