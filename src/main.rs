use bevy::prelude::*;
use bevy::pbr::FogSettings;
use bevy::core_pipeline::bloom::BloomSettings;
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
                resolution: bevy::window::WindowResolution::new(1920.0, 1080.0),
                present_mode: bevy::window::PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }).set(bevy::render::RenderPlugin {
            render_creation: bevy::render::settings::RenderCreation::Automatic(bevy::render::settings::WgpuSettings {
                backends: Some(bevy::render::settings::Backends::VULKAN),
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
) {
    if *done { return; }
    *timer += time.delta_seconds();
    // Some Linux/NVIDIA setups close the window early; grab a reference frame quickly.
    if *timer > 3.5 {
        if let Ok(window_entity) = main_window.get_single() {
            let _ = screenshot_manager.save_screenshot_to_disk(window_entity, "/tmp/original_screen.png");
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
        color: Color::srgb(0.62, 0.67, 0.74),
        brightness: 120.0, // Adjusted for Bevy's PBR intensity vs Panda3D
    });

    commands.spawn((
        DirectionalLightBundle {
            directional_light: DirectionalLight {
                color: Color::srgb(0.9, 0.97, 1.0),
                illuminance: 9200.0,
                shadows_enabled: false,
                ..default()
            },
            // Panda3D HPR (18, -62, 0) -> Bevy: pitch=-62, yaw=18
            transform: Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, 18.0_f32.to_radians(), -62.0_f32.to_radians(), 0.0)),
            ..default()
        },
        bevy::render::view::RenderLayers::from_layers(&[0, 2]),
    ));

    // Main Camera (Layer 0 - World + Distortion)
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                order: 0,
                clear_color: bevy::render::camera::ClearColorConfig::Custom(Color::srgb(0.03, 0.04, 0.06)),
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 108.0_f32.to_radians(),
                near: 0.012,
                far: 1500.0,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 2.35, 6.8).looking_at(Vec3::new(0.0, 0.32, 0.0), Vec3::Y),
            ..default()
        },
        player::PlayerCamera,
        effects::CrtSettings::default(),
        effects::viscous::ViscousSettings::default(),
        // Python parity (main.py _setup_camera): black fog from 0.0 to 35.0
        FogSettings {
            color: Color::BLACK,
            falloff: FogFalloff::Linear { start: 0.0, end: 35.0 },
            ..default()
        },
        BloomSettings {
            intensity: 0.35,
            prefilter_settings: bevy::core_pipeline::bloom::BloomPrefilterSettings {
                threshold: 0.52, // Matched to u_bloom_threshold
                threshold_softness: 0.2,
            },
            ..default()
        },
        bevy::render::view::RenderLayers::layer(0),
    ));

    // Foreground Camera (Layer 2 - Player/Weapon, NO DISTORTION)
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                order: 2, // Drawn after main scene (0) and world-distort
                clear_color: bevy::render::camera::ClearColorConfig::None,
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 75.0_f32.to_radians(),
                near: 0.05, // Closer near plane for weapon transparency
                far: 2000.0,
                ..default()
            }),
            transform: Transform::from_xyz(0.0, 2.35, 6.8).looking_at(Vec3::new(0.0, 0.32, 0.0), Vec3::Y),
            ..default()
        },
        player::ForegroundCamera,
        // MUST have same fog as main camera to blend correctly
        FogSettings {
            color: Color::BLACK,
            falloff: FogFalloff::Linear { start: 0.0, end: 35.0 },
            ..default()
        },
        BloomSettings {
            intensity: 0.35,
            prefilter_settings: bevy::core_pipeline::bloom::BloomPrefilterSettings {
                threshold: 0.52,
                threshold_softness: 0.2,
            },
            ..default()
        },
        bevy::render::view::RenderLayers::layer(2),
    ));

    // Floating Text Camera (Layer 1 - UI overlays)
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                order: 3, // Drawn last
                clear_color: bevy::render::camera::ClearColorConfig::None,
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 75.0_f32.to_radians(),
                near: 0.1,
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
