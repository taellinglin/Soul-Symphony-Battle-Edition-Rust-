mod types;
mod bsp;
mod walls;
mod mesh;
mod ceiling;

// Re-export all public types
pub use types::*;

use bevy::prelude::*;
use bevy_rapier3d::prelude::*;
use crate::components::*;
use rand::{Rng, seq::SliceRandom, thread_rng};

use bsp::BspState;
use walls::*;
use mesh::*;

pub struct DungeonGeneratorPlugin;

impl Plugin for DungeonGeneratorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DungeonGraph>()
           .init_resource::<GenerationConfig>()
           .add_systems(Startup, (generate_dungeon, ceiling::setup_ceiling))
           .add_systems(Update, (ceiling::update_ceiling, update_color_cycles));
    }
}

pub(crate) fn generate_dungeon(
    mut commands: Commands,
    config: Res<GenerationConfig>,
    mut graph: ResMut<DungeonGraph>,
    mut meshes: ResMut<Assets<Mesh>>,
    _materials: ResMut<Assets<StandardMaterial>>,
    mut floor_materials: ResMut<Assets<crate::rendering::thermal::ThermalMaterial>>,
    mut water_materials: ResMut<Assets<crate::rendering::WaterSurfaceMaterial>>,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    reflection_tex: Res<crate::rendering::ReflectionTexture>,
) {
    let unit_cube = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let _unit_plane = meshes.add(Plane3d::default());
    
    // Python parity: map_w = int(176 * scale)
    let map_w = (176.0 * config.scale) as i32;
    let state = BspState { width: map_w, depth: map_w, config: config.clone() };
    
    let avg_scaled = config.average_room_size * config.scale;
    
    let (mut rooms, edges) = match config.layout_mode.as_str() {
        "arena" => {
            let hub_center_x = map_w as f32 * 0.5;
            let hub_center_z = map_w as f32 * 0.5;
            let hub_room = Room {
                x: hub_center_x - 2000.0,
                y: hub_center_z - 2000.0,
                w: 4000.0,
                h: 4000.0,
                w_layer: 0,
                _id: 0,
                dimension_field: DimensionField::default(),
                pockets: Vec::new(),
                doors: RoomDoors { top: vec![], bottom: vec![], left: vec![], right: vec![] },
            };
            (vec![hub_room], Vec::new())
        },
        "hexmix" => {
            let cell_size = (avg_scaled * 0.96) as i32;
            state.generate_hex_mixed(cell_size)
        },
        "snake3d" => {
            state.generate_snake3d(config.snake_cell_size, config.snake_layers)
        },
        "maze3d" => {
            state.generate_maze3d(config.maze_cell_size, config.maze_layers, config.maze_loop_chance, config.maze_vertical_link_chance)
        },
        "labyrinth" | _ => {
            let cell_size = (avg_scaled * 1.35) as i32;
            state.generate_labyrinth(cell_size)
        },
    };

    graph.edges = edges.clone();
    graph.rooms = rooms.clone();
    println!("Dungeon generated: {} rooms, {} edges", rooms.len(), edges.len());
    if let Some(first) = rooms.first() {
        println!("Room 0 center: {:?}, W: {}", first.center(), first.w_layer);
    }

    // Python parity: _plan_hallways_and_doors — populate doors for wall segmentation
    for &(a, b) in &edges {
        let ca = rooms[a].center();
        let cb = rooms[b].center();

        let (_, _, a_side, a_pos) = get_room_anchor(&rooms[a], cb.x, cb.y);
        let (_, _, b_side, b_pos) = get_room_anchor(&rooms[b], ca.x, ca.y);

        match a_side {
            "top" => rooms[a].doors.top.push(a_pos),
            "bottom" => rooms[a].doors.bottom.push(a_pos),
            "left" => rooms[a].doors.left.push(a_pos),
            "right" => rooms[a].doors.right.push(a_pos),
            _ => {}
        }
        match b_side {
            "top" => rooms[b].doors.top.push(b_pos),
            "bottom" => rooms[b].doors.bottom.push(b_pos),
            "left" => rooms[b].doors.left.push(b_pos),
            "right" => rooms[b].doors.right.push(b_pos),
            _ => {}
        }
    }

    // Python parity: wall/floor/corridor constants
    // Using config values for dimensions

    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
    let mut checker_data = vec![0u8; 256 * 256 * 4];
    for y in 0..256 {
        for x in 0..256 {
            let cx = x / (256 / 8);
            let cy = y / (256 / 8);
            let idx = (y * 256 + x) * 4;
            if (cx + cy) % 2 == 0 {
                // color_a: (0.0, 0.0, 0.0, 1.0)
                checker_data[idx] = 0;
                checker_data[idx + 1] = 0;
                checker_data[idx + 2] = 0;
                checker_data[idx + 3] = 255;
            } else {
                // color_b: (0.0, 0.0, 0.0, 0.0)
                checker_data[idx] = 0;
                checker_data[idx + 1] = 0;
                checker_data[idx + 2] = 0;
                checker_data[idx + 3] = 0;
            }
        }
    }
    let checker_img = Image::new(
        Extent3d { width: 256, height: 256, depth_or_array_layers: 1 },
        TextureDimension::D2,
        checker_data,
        TextureFormat::Rgba8UnormSrgb,
        bevy::render::render_asset::RenderAssetUsages::RENDER_WORLD,
    );
    let _checker_tex = images.add(checker_img);
    
    // Python parity: floor color (0.19, 0.22, 0.28), wall color (0.78, 0.82, 0.88)
    let room_tex: Handle<Image> = asset_server.load("graphics/rooms/00110-1042801366.png");

    let mat_water = water_materials.add(crate::rendering::WaterSurfaceMaterial {
        base: StandardMaterial {
            alpha_mode: bevy::prelude::AlphaMode::Blend,
            cull_mode: None,
            unlit: true,
            ..default()
        },
        extension: crate::rendering::WaterSurfaceExtension {
            settings: crate::rendering::WaterSurfaceSettings {
                uv_scale: 1.0,
                alpha: 0.2, // Python parity: water_color_cycle_alpha = 0.2
                rainbow_strength: 1.0,
                diffusion_strength: 0.18,
                spec_strength: 0.72,
                room_tex_strength: 0.32,
                room_tex_desat: 0.85,
                thermal_mode: 1.0,
                thermal_strength: 0.92,
                compression_factor: 1.0,
                compression_thermal_strength: 0.85,
                density_contrast: 1.35,
                density_gamma: 0.85,
                fog_start: 0.0,
                fog_end: 150.0,
                ..default()
            },
            room_texture: Some(room_tex.clone()),
            reflection_texture: Some(reflection_tex.0.clone()),
        }
    });

    let mat_corridor_floor = floor_materials.add(crate::rendering::thermal::ThermalMaterial {
        base: StandardMaterial::default(),
        extension: crate::rendering::thermal::ThermalExtension {
            settings: crate::rendering::thermal::ThermalSettings {
                time: 0.0,
                uv_scale: 15.0,
                density_contrast: 1.15,
                density_gamma: 0.85,
                thermal_strength: 0.8,
                compression_factor: 1.0,
                fog_start: 20.0,
                fog_end: 150.0,
                fog_color: bevy::color::LinearRgba::new(0.1, 0.12, 0.17, 1.0),
            },
        },
    });

    let mat_corridor_wall = floor_materials.add(crate::rendering::thermal::ThermalMaterial {
        base: StandardMaterial::default(),
        extension: crate::rendering::thermal::ThermalExtension {
            settings: crate::rendering::thermal::ThermalSettings {
                time: 0.0,
                uv_scale: 30.0,
                density_contrast: 1.35,
                density_gamma: 0.85,
                thermal_strength: 1.0,
                compression_factor: 1.0,
                fog_start: 20.0,
                fog_end: 150.0,
                fog_color: bevy::color::LinearRgba::new(0.1, 0.12, 0.17, 1.0),
            },
        },
    });



    // Spawn the Physics Colliders
    for r in rooms.iter() {
        let _wall_h = config.room_height;
        let _wall_t = config.wall_thickness;
        let cx_w = r.w_layer as f32 * 5.0;
        // Compute 4D collision group for this layer
        let clamped_layer = r.w_layer.clamp(-15, 15);
        let group_bit = 1 << (clamped_layer + 15);
        let layer_group = bevy_rapier3d::prelude::Group::from_bits_truncate(group_bit as u32);
        let collision_groups = bevy_rapier3d::prelude::CollisionGroups::new(layer_group, bevy_rapier3d::prelude::Group::all());

        // Spawn the Room Entity itself for particles/logic
        commands.spawn((r.clone(), SpatialBundle::default()));

        // Floor — simple dark surface (Python parity: floor color 0.19, 0.22, 0.28)
        // Water effect comes from separate map-wide water surface at Y=0.28
        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        let cz = 0.0;

        // Visual wall height capped for performance — fog hides everything past ~50 units
        // Config room_height stays at 4096 for game logic (open-space feel)
        let _visual_wall_h = config.room_height.min(50.0);

        let mut rng = thread_rng();

        commands.spawn((
            Spatial4D {
                w: r.w_layer as f32 * 5.0,
                target_w: r.w_layer as f32 * 5.0,
                layer: r.w_layer,
                is_folded: false,
            },
            MaterialMeshBundle {
                mesh: unit_cube.clone(),
                material: floor_materials.add(crate::rendering::thermal::ThermalMaterial {
                    base: StandardMaterial::default(),
                    extension: crate::rendering::thermal::ThermalExtension {
                        settings: crate::rendering::thermal::ThermalSettings {
                            time: 0.0,
                            uv_scale: 30.0,
                            density_contrast: 1.35,
                            density_gamma: 0.85,
                            thermal_strength: 1.0,
                            compression_factor: 1.0,
                            fog_start: 20.0,
                            fog_end: 150.0,
                            fog_color: bevy::color::LinearRgba::new(0.1, 0.12, 0.17, 1.0),
                        },
                    },
                }),
                transform: Transform::from_xyz(cx, cz - config.floor_thickness * 0.5, cy).with_scale(Vec3::new(r.w, config.floor_thickness, r.h)),
                ..default()
            },
            RigidBody::Fixed,
            Collider::cuboid(0.5, 0.5, 0.5),
            collision_groups,
            r.dimension_field.clone(),
            MapGeometry,
        ));        let water_raise = 0.28; 
        let water_t = (config.floor_thickness * 0.18).max(0.01);
        commands.spawn((
            Spatial4D {
                w: r.w_layer as f32 * 5.0,
                target_w: r.w_layer as f32 * 5.0,
                layer: r.w_layer,
                is_folded: false,
            },
            MaterialMeshBundle {
                mesh: unit_cube.clone(),
                material: water_materials.add(crate::rendering::WaterSurfaceMaterial {
                    base: StandardMaterial {
                        alpha_mode: bevy::prelude::AlphaMode::Blend,
                        base_color: Color::srgb(0.0, 0.0, 0.0),
                        cull_mode: None,
                        ..default()
                    },
                    extension: crate::rendering::WaterSurfaceExtension {
                        reflection_texture: Some(reflection_tex.0.clone()),
                        room_texture: Some(room_tex.clone()),
                        settings: crate::rendering::WaterSurfaceSettings {
                            uv_scale: 1.0,
                            alpha: 1.0,
                            rainbow_strength: 0.1,
                            diffusion_strength: 0.2,
                            spec_strength: 0.5,
                            player_w: 0.0,
                            ..default()
                        },
                    }
                }),
                transform: Transform::from_xyz(cx, cz + water_raise, cy)
                    .with_scale(Vec3::new(r.w, water_t, r.h)),
                ..default()
            },
            MapGeometry,
        ));




        // Python parity: _add_angled_room_walls — diagonal walls at corners (12% chance for large rooms)
        if r.w >= 14.0 && r.h >= 14.0 && rng.gen_bool(0.12) {
            let cut = r.w.min(r.h) * rng.gen_range(0.18..0.25);
            let half_len = cut * 0.3;
            let thickness = config.wall_thickness * 0.76;
            let wz = cz + config.room_height * 0.5;
            
            let mut corners: Vec<(f32, f32, f32)> = vec![
                (cx - r.w * 0.5 + cut * 0.58, cy - r.h * 0.5 + cut * 0.58, 45.0_f32.to_radians()),
                (cx + r.w * 0.5 - cut * 0.58, cy - r.h * 0.5 + cut * 0.58, -45.0_f32.to_radians()),
                (cx - r.w * 0.5 + cut * 0.58, cy + r.h * 0.5 - cut * 0.58, -45.0_f32.to_radians()),
                (cx + r.w * 0.5 - cut * 0.58, cy + r.h * 0.5 - cut * 0.58, 45.0_f32.to_radians()),
            ];
            corners.shuffle(&mut rng);
            let use_count = *[1_usize, 2, 2, 3].choose(&mut rng).unwrap();
            
            let mut angled_boxes = Vec::new();


            for &(acx, acy, angle) in corners.iter().take(use_count) {
                let pos = Vec3::new(acx, wz, acy);
                let rot = Quat::from_rotation_y(angle);
                angled_boxes.push((pos, rot, Vec3::new(half_len, config.room_height, thickness)));

            }

            if !angled_boxes.is_empty() {
                let mat_angled = floor_materials.add(crate::rendering::thermal::ThermalMaterial {
                    base: StandardMaterial::default(),
                    extension: crate::rendering::thermal::ThermalExtension {
                        settings: crate::rendering::thermal::ThermalSettings {
                            time: 0.0,
                            uv_scale: 30.0,
                            density_contrast: 1.35,
                            density_gamma: 0.85,
                            thermal_strength: 1.0,
                            compression_factor: 1.0,
                            fog_start: 20.0,
                            fog_end: 150.0,
                            fog_color: bevy::color::LinearRgba::new(0.1, 0.12, 0.17, 1.0),
                        },
                    },
                });

                commands.spawn((
                    MaterialMeshBundle {
                        mesh: meshes.add(create_merged_box_mesh_rotated(&angled_boxes)),
                        material: mat_angled,
                        transform: Transform::IDENTITY,
                        ..default()
                    },
                    MapGeometry,
                    CameraObstacle,
                    Spatial4D { w: cx_w, target_w: cx_w, layer: r.w_layer, is_folded: false },
                ));

            }
        }



    }

    // Spawn corridors (Python parity: _build_corridor_segment with walls + decor)
    let visual_wall_h = config.room_height.min(50.0);
    let mut rng = thread_rng();
    for &(a, b) in &edges {
        let ra = &rooms[a];
        let rb = &rooms[b];
        let ca = ra.center();
        let cb = rb.center();

        let clamped_layer = ra.w_layer.clamp(-15, 15);
        let group_bit = 1 << (clamped_layer + 15);
        let layer_group = Group::from_bits_truncate(group_bit as u32);
        // Walls include GROUP_32 so camera ray hits them, plus their layer group
        let wall_group = layer_group | Group::GROUP_32;
        let collision_groups = CollisionGroups::new(layer_group, layer_group);
        let wall_collision_groups = CollisionGroups::new(wall_group, wall_group);
        let cz = 0.0;
        let spatial = Spatial4D { w: ra.w_layer as f32 * 5.0, target_w: ra.w_layer as f32 * 5.0, layer: ra.w_layer, is_folded: false };

        let mid_x = cb.x;
        let mid_y = ca.y;

        let mut corridor_floor_boxes = Vec::new();
        let mut corridor_wall_boxes = Vec::new();


        // Segment 1: Horizontal (ca -> mid)
        let seg1_len = (mid_x - ca.x).abs();
        if seg1_len > 0.6 {
            let sx = (ca.x + mid_x) * 0.5;
            let sy = ca.y;
            corridor_floor_boxes.push((Vec3::new(sx, cz - config.floor_thickness * 0.5, sy), Vec3::new(seg1_len, config.floor_thickness, config.corridor_width + config.wall_thickness)));
            for &side in &[1.0_f32, -1.0] {
                corridor_wall_boxes.push((Vec3::new(sx, cz + visual_wall_h * 0.5, sy + side * (config.corridor_width * 0.5 + config.wall_thickness * 0.5)), Vec3::new(seg1_len + config.wall_thickness, visual_wall_h, config.wall_thickness)));

            }
        }

        // Segment 2: Vertical (mid -> cb)
        let seg2_len = (cb.y - mid_y).abs();
        if seg2_len > 0.6 {
            let sx = mid_x;
            let sy = (mid_y + cb.y) * 0.5;
            corridor_floor_boxes.push((Vec3::new(sx, cz - config.floor_thickness * 0.5, sy), Vec3::new(config.corridor_width + config.wall_thickness, config.floor_thickness, seg2_len)));
            for &side in &[1.0_f32, -1.0] {
                corridor_wall_boxes.push((Vec3::new(sx + side * (config.corridor_width * 0.5 + config.wall_thickness * 0.5), cz + visual_wall_h * 0.5, sy), Vec3::new(config.wall_thickness, visual_wall_h, seg2_len + config.wall_thickness)));

            }
        }

        // Joint
        let joint_size = config.corridor_width + config.wall_thickness;
        corridor_floor_boxes.push((Vec3::new(mid_x, cz - config.floor_thickness * 0.5, mid_y), Vec3::new(joint_size, config.floor_thickness, joint_size)));
        let ox = joint_size * 0.5;
        let oy = joint_size * 0.5;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                corridor_wall_boxes.push((Vec3::new(mid_x + sx * ox, cz + visual_wall_h * 0.5, mid_y + sy * oy), Vec3::new(config.wall_thickness, visual_wall_h, config.wall_thickness)));
            }
        }

        // Spawn Merged Floor
        if !corridor_floor_boxes.is_empty() {
            let floor_mesh = meshes.add(create_merged_box_mesh(&corridor_floor_boxes));
            let floor_parts: Vec<(Vec3, Quat, Collider)> = corridor_floor_boxes.iter()
                .map(|(p, s)| (*p, Quat::IDENTITY, Collider::cuboid(s.x * 0.5, s.y * 0.5, s.z * 0.5)))
                .collect();
            commands.spawn((
                RigidBody::Fixed,
                Collider::compound(floor_parts),
                Friction::coefficient(1.35),
                MaterialMeshBundle {
                    mesh: floor_mesh,
                    material: mat_corridor_floor.clone(),
                    transform: Transform::IDENTITY,
                    ..default()
                },
                collision_groups,
                spatial.clone(),
                MapGeometry,
            ));

            // Spawn water overlay for corridor
            let water_raise = 0.28;
            let water_t = (config.floor_thickness * 0.18).max(0.01);
            let mut corridor_water_boxes = Vec::new();
            for (p, s) in &corridor_floor_boxes {
                corridor_water_boxes.push((
                    Vec3::new(p.x, cz + water_raise, p.z),
                    Vec3::new(s.x, water_t, s.z),
                ));
            }
            commands.spawn((
                MaterialMeshBundle {
                    mesh: meshes.add(create_merged_box_mesh(&corridor_water_boxes)),
                    material: mat_water.clone(),
                    transform: Transform::IDENTITY,
                    ..default()
                },
                spatial.clone(),
                MapGeometry,
            ));
        }

        // Spawn Merged Walls
        if !corridor_wall_boxes.is_empty() {
            for (pos, size) in corridor_wall_boxes {
                commands.spawn((
                    MaterialMeshBundle {
                        mesh: meshes.add(Cuboid::new(size.x, size.y, size.z)),
                        material: mat_corridor_wall.clone(),
                        transform: Transform::from_translation(pos),
                        ..default()
                    },
                    spatial.clone(),
                    RigidBody::Fixed,
                    Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
                    wall_collision_groups.clone(),
                    MapGeometry,
                    CameraObstacle,
                    ColorCycleTarget { 
                        base_color: Color::srgb(0.74, 0.79, 0.87), 
                        speed: rng.gen_range(0.045..0.11), 
                        phase: rng.gen_range(0.0..std::f32::consts::TAU) 
                    },
                ));
            }
        }


    }

    graph.rooms = rooms;
    
    // Python parity: _build_floor_and_bounds creates a map-wide transparent water surface
    // at floor_y + water_surface_raise = 0.28, covering the entire map (map_w x map_d)
    // This is the surface that produces the vibrant ROYGBIV thermal color floor patterns
    let map_half = map_w as f32 * 0.5;
    let water_overscan = 2500.0; // Python: water_loop_overscan = 6.0 — increased to 2500 for infinite void
    
    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::splat(water_overscan))),
            material: mat_water.clone(),
            transform: Transform::from_xyz(map_half, 0.0, map_half), // mesh at y=0.0.
            ..default()
        },
        Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
        MapGeometry,
    ));

    commands.spawn((
        RigidBody::Fixed,
        Collider::cuboid(water_overscan, 10.0, water_overscan), // 20 units deep
        Transform::from_xyz(map_half, -10.0, map_half), // Top surface at y=0.0
        GlobalTransform::default(),
        // Floor hits all layers (bits 0-30) but EXCLUDES Group 32 (bit 31) to ignore camera ray
        CollisionGroups::new(Group::from_bits_truncate(0x7FFFFFFF), Group::from_bits_truncate(0x7FFFFFFF)), 
        MapGeometry,
    ));

    // ==========================================
    // PHYSICAL WORLD BOUNDARIES (Height Cap Parity)
    // ==========================================
    let (high_y, low_y) = if config.layout_mode == "arena" {
        (15.3, -15.3)
    } else {
        (config.room_height + 4.0, -1.4)
    };

    // Top physical ceiling collider
    commands.spawn((
        RigidBody::Fixed,
        Collider::cuboid(water_overscan, 0.11, water_overscan),
        Transform::from_xyz(map_half, high_y + 0.11, map_half),
        GlobalTransform::default(),
        // Parity: isolate boundaries to bit 31 (Group 32)
        CollisionGroups::new(Group::GROUP_32, Group::GROUP_32),
        MapGeometry,
    ));

    // Bottom physical floor collider
    commands.spawn((
        RigidBody::Fixed,
        Collider::cuboid(water_overscan, 0.11, water_overscan),
        Transform::from_xyz(map_half, low_y - 0.11, map_half),
        GlobalTransform::default(),
        CollisionGroups::new(Group::GROUP_32, Group::GROUP_32),
        MapGeometry,
    ));

    // ==========================================
    // ARENA MODE GENERATION
    // ==========================================
}



// --- Phase 6: Environmental Helpers & Subtractive Maze ---



// Python parity: _register_color_cycle — continuous HSV color shifting on surfaces
fn update_color_cycles(
    time: Res<Time>,
    mut query: Query<(&ColorCycleTarget, &Handle<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_seconds();
    for (cycle, mat_handle) in query.iter_mut() {
        if let Some(mat) = materials.get_mut(mat_handle) {
            let hue_shift = (t * cycle.speed + cycle.phase).sin() * 0.08;
            let Srgba { red, green, blue, alpha } = cycle.base_color.to_srgba();
            mat.base_color = Color::srgba(
                (red + hue_shift).clamp(0.0, 1.0),
                (green + hue_shift * 1.2).clamp(0.0, 1.0),
                (blue + hue_shift * 0.7).clamp(0.0, 1.0),
                alpha,
            );
        }
    }
}
