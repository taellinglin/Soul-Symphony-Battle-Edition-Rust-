use bevy::prelude::*;
use bevy::pbr::FogSettings;
use bevy_hanabi::prelude::*;
use bevy_rapier3d::prelude::*;
use bevy::window::CursorGrabMode;

mod components;
mod map;
mod rendering;
mod player;
mod ai;
mod effects;
mod ui;
mod world;
mod systems;
mod weapon_system;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()).set(WindowPlugin {
            primary_window: Some(Window {
                title: "Soul Symphony (Battle Edition)".into(),
                resolution: bevy::window::WindowResolution::new(1920.0, 1080.0)
                    .with_scale_factor_override(1.0),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(HanabiPlugin)
        .add_plugins(rendering::RenderingPlugin)
        .add_plugins(map::DungeonGeneratorPlugin)
        .add_plugins(RapierPhysicsPlugin::<NoUserData>::default())
        .add_plugins(player::PlayerPlugin)
        .add_plugins(ai::AiPlugin)
        .add_plugins(effects::FxPlugin)
        .add_plugins(ui::UiPlugin)
        .add_plugins(weapon_system::WeaponSystemPlugin)
        .add_plugins(world::WorldPlugin)
        .add_plugins(systems::SystemsPlugin)
        .add_systems(PreStartup, (setup_camera_light, cursor_grab_system))
        .add_systems(Update, auto_screenshot)
        .run();
}

fn auto_screenshot(
    mut timer: Local<f32>,
    time: Res<Time>,
    main_window: Query<Entity, With<Window>>,
    mut screenshot_manager: ResMut<bevy::render::view::screenshot::ScreenshotManager>,
    mut done: Local<bool>,
    player_q: Query<(&Transform, &crate::components::Spatial4D), With<crate::player::Player>>,
    camera_q: Query<&Transform, With<crate::player::PlayerCamera>>,
) {
    if *done { return; }
    *timer += time.delta_seconds();
    if *timer > 4.0 {
        // Debug: log player and camera positions at screenshot time
        if let Ok((p_tf, p_sp)) = player_q.get_single() {
            info!("DEBUG SCREENSHOT: Player pos={:?} w={}", p_tf.translation, p_sp.w);
        }
        if let Ok(c_tf) = camera_q.get_single() {
            info!("DEBUG SCREENSHOT: Camera pos={:?}", c_tf.translation);
        }
        if let Ok(window_entity) = main_window.get_single() {
            let _ = screenshot_manager.save_screenshot_to_disk(window_entity, "/tmp/bevy_screen.png");
            *done = true;
        }
    }
}

fn cursor_grab_system(
    mut windows: Query<&mut Window>,
) {
    for mut window in windows.iter_mut() {
        window.cursor.grab_mode = CursorGrabMode::Locked;
        window.cursor.visible = false;
    }
}

fn setup_camera_light(
    mut commands: Commands,
    mut _meshes: ResMut<Assets<Mesh>>,
    mut _materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 250.0,
    });

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 8000.0,
            shadows_enabled: false,
            ..default()
        },
        transform: Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_4)),
        ..default()
    });

    // Main Camera (Draws on Top)
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                order: 0,
                clear_color: bevy::render::camera::ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 75.0_f32.to_radians(),
                near: 0.012,
                far: 1500.0,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 2.35, 6.8).looking_at(Vec3::new(0.0, 0.32, 0.0), Vec3::Y),
            ..default()
        },
        player::PlayerCamera,
        effects::CrtSettings::default(),
        // Python parity: fog from camera.py L14-17: color(0.1, 0.12, 0.17)
        // Bevy fog range tuned to match Panda3D visual (not 1:1 values due to engine differences)
        FogSettings {
            color: Color::BLACK,
            falloff: FogFalloff::Linear { start: 20.0, end: 120.0 }, // Void parity
            ..default()
        },
    ));

    // Floating Text Camera (Syncs with Main Camera, draws floating text on layer 1)
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                order: 1,
                clear_color: bevy::render::camera::ClearColorConfig::None,
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 75.0_f32.to_radians(),
                near: 0.012,
                far: 1500.0,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 2.35, 6.8).looking_at(Vec3::new(0.0, 0.32, 0.0), Vec3::Y),
            ..default()
        },
        player::FloatingTextCamera,
        bevy::render::view::RenderLayers::layer(1),
    ));

    // Reflection Texture for Inverted Echo
    let size = bevy::render::render_resource::Extent3d {
        width: 1024,
        height: 1024,
        ..default()
    };
    let mut image = Image {
        texture_descriptor: bevy::render::render_resource::TextureDescriptor {
            label: None,
            size,
            dimension: bevy::render::render_resource::TextureDimension::D2,
            format: bevy::render::render_resource::TextureFormat::Bgra8UnormSrgb,
            usage: bevy::render::render_resource::TextureUsages::RENDER_ATTACHMENT | bevy::render::render_resource::TextureUsages::TEXTURE_BINDING | bevy::render::render_resource::TextureUsages::COPY_DST,
            view_formats: &[],
            mip_level_count: 1,
            sample_count: 1,
        },
        ..default()
    };
    image.resize(size);
    let reflection_image_handle = images.add(image);
    commands.insert_resource(rendering::ReflectionTexture(reflection_image_handle.clone()));

    // Inverted Echo Camera (Low-quality mirrored reflection camera)
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                order: -1, // Render before main camera
                target: bevy::render::camera::RenderTarget::Image(reflection_image_handle),
                clear_color: bevy::render::camera::ClearColorConfig::Custom(Color::srgb(0.05, 0.06, 0.08)),
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 75.0_f32.to_radians(),
                near: 0.012,
                far: 1500.0,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, -2.0, 0.0).looking_at(Vec3::ZERO, Vec3::Y),
            ..default()
        },
        player::InvertedEchoCamera,
        // Draw standard world layer (0), but NOT floating text (1)
        bevy::render::view::RenderLayers::layer(0),
    ));

}
