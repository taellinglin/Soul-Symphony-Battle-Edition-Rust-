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
            .add_systems(Startup, (generate_dungeon, build_inverted_echo_world, ceiling::setup_ceiling).chain())
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
    mut floor_wet_materials: ResMut<Assets<crate::rendering::FloorWetMaterial>>,
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
    
    // In the original, default play uses four_d_obstacle_arena_mode, which calls
    // _build_four_d_obstacle_arena instead of the BSP/hexmix dungeon build. That
    // path defines four quadrant rooms but does NOT build room/corridor walls.
    // We mirror that by treating layout_mode == "arena" as obstacle-arena mode
    // and skipping per-room geometry when build_room_geometry is false.
    let (mut rooms, edges, build_room_geometry) = match config.layout_mode.as_str() {
        "arena" => {
            let margin = 8.0_f32;
            let inner_w = map_w as f32 - margin * 2.0;
            let inner_h = map_w as f32 - margin * 2.0;
            let half_w = inner_w * 0.5;
            let half_h = inner_h * 0.5;

            let make_room = |x: f32, y: f32, w: f32, h: f32, id: usize| Room {
                x,
                y,
                w,
                h,
                w_layer: 0,
                _id: id,
                dimension_field: DimensionField::default(),
                pockets: Vec::new(),
                doors: RoomDoors::default(),
            };

            let r0 = make_room(margin, margin, half_w, half_h, 0);
            let r1 = make_room(margin + half_w, margin, half_w, half_h, 1);
            let r2 = make_room(margin, margin + half_h, half_w, half_h, 2);
            let r3 = make_room(margin + half_w, margin + half_h, half_w, half_h, 3);

            let edges = vec![(0, 1), (0, 2), (1, 3), (2, 3)];
            (vec![r0, r1, r2, r3], edges, false)
        }
        "hexmix" => {
            let cell_size = (avg_scaled * 0.96) as i32;
            let (rooms, edges) = state.generate_hex_mixed(cell_size);
            (rooms, edges, true)
        }
        "snake3d" => {
            let (rooms, edges) = state.generate_snake3d(config.snake_cell_size, config.snake_layers);
            (rooms, edges, true)
        }
        "maze3d" => {
            let (rooms, edges) = state.generate_maze3d(
                config.maze_cell_size,
                config.maze_layers,
                config.maze_loop_chance,
                config.maze_vertical_link_chance,
            );
            (rooms, edges, true)
        }
        "labyrinth" | _ => {
            let cell_size = (avg_scaled * 1.35) as i32;
            let (rooms, edges) = state.generate_labyrinth(cell_size);
            (rooms, edges, true)
        }
    };

    graph.edges = edges.clone();
    graph.rooms = rooms.clone();
    println!("Dungeon generated: {} rooms, {} edges", rooms.len(), edges.len());
    if let Some(first) = rooms.first() {
        println!("Room 0 center: {:?}, W: {}", first.center(), first.w_layer);
    }

    // Python parity: _plan_hallways_and_doors — populate doors for wall segmentation.
    // In obstacle-arena mode we do not build room/corridor walls at all, so there
    // are no doors to compute.
    if build_room_geometry {
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
    let mat_floor_wet = floor_wet_materials.add(crate::rendering::FloorWetMaterial {
        base: StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            cull_mode: None,
            ..default()
        },
        extension: crate::rendering::FloorWetExtension {
            settings: crate::rendering::FloorWetSettings::default(),
            base_texture: Some(room_tex.clone()),
        },
    });

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
                alpha: 0.2,
                rainbow_strength: 0.0,
                diffusion_strength: 0.0,
                spec_strength: 1.15,
                room_tex_strength: 0.0,
                room_tex_desat: 0.15,
                thermal_mode: 1.0,
                thermal_strength: 1.0,
                compression_factor: 1.0,
                compression_thermal_strength: 0.85,
                density_contrast: 1.35,
                density_gamma: 0.85,
                // Original parity (main.py): black fog, range 0..35
                fog_start: 0.0,
                fog_end: 35.0,
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
                uv_scale: 1.0,
                density_contrast: 1.35,
                density_gamma: 0.85,
                thermal_strength: 1.0,
                compression_factor: 1.0,
                fog_start: 0.0,
                fog_end: 35.0,
                fog_color: bevy::color::LinearRgba::BLACK,
            },
        },
    });

    let mat_corridor_wall = floor_materials.add(crate::rendering::thermal::ThermalMaterial {
        base: StandardMaterial::default(),
        extension: crate::rendering::thermal::ThermalExtension {
            settings: crate::rendering::thermal::ThermalSettings {
                time: 0.0,
                uv_scale: 1.0,
                density_contrast: 1.35,
                density_gamma: 0.85,
                thermal_strength: 1.0,
                compression_factor: 1.0,
                fog_start: 0.0,
                fog_end: 35.0,
                fog_color: bevy::color::LinearRgba::BLACK,
            },
        },
    });



    // Spawn the Physics Colliders.
    // In obstacle-arena mode (layout_mode == "arena"), the original does not build
    // per-room floors/ceilings/walls at all; only the global water surface and
    // hyper-bounds colliders exist. To match that open startup view, we skip
    // per-room geometry when build_room_geometry is false.
    if build_room_geometry {
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

        // Visual wall height matches original wall_h (room_height)
        // Config room_height is 4096.0 in the original for an open, cavernous feel.
        let _visual_wall_h = config.room_height;

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
                            uv_scale: 1.0,
                            density_contrast: 1.35,
                            density_gamma: 0.85,
                            thermal_strength: 1.0,
                            compression_factor: 1.0,
                            fog_start: 0.0,
                            fog_end: 35.0,
                            fog_color: bevy::color::LinearRgba::BLACK,
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
            EchoSource,
        ));

        // Python parity: floor_wet shader overlays on floors (ripples/wakes).
        // Keep thermal floor, but add a thin FloorWetMaterial overlay so update_floor_wetness drives visible wakes.
        let wet_t = (config.floor_thickness * 0.12).max(0.01);
        commands.spawn((
            Spatial4D {
                w: r.w_layer as f32 * 5.0,
                target_w: r.w_layer as f32 * 5.0,
                layer: r.w_layer,
                is_folded: false,
            },
            MaterialMeshBundle {
                mesh: unit_cube.clone(),
                material: mat_floor_wet.clone(),
                transform: Transform::from_xyz(cx, cz + wet_t * 0.5 + 0.01, cy)
                    .with_scale(Vec3::new(r.w, wet_t, r.h)),
                ..default()
            },
            MapGeometry,
            EchoSource,
        ));

        // Original parity: water surface is map-wide (not per-room). Avoid stacking per-room water meshes to prevent flicker/z-fighting.

        // Python parity: _build_room_walls_with_doors — build full room perimeter walls with door cutouts
        // Door positions are precomputed into r.doors.* in _plan_hallways_and_doors logic above.
        // Use full room_height (wall_h) so walls/pillars are effectively “infinite” like the original.
        let visual_wall_h = config.room_height;
        let wall_t = config.wall_thickness;
        let wall_y = cz + visual_wall_h * 0.5;
        let door_half = config.corridor_width * 0.5 + wall_t * 0.5; // ensure corridor fits cleanly through the gap

        let x0 = r.x;
        let x1 = r.x + r.w;
        let z0 = r.y;
        let z1 = r.y + r.h;

        // Walls include GROUP_32 so camera ray hits them, plus their layer group
        let wall_group = layer_group | Group::GROUP_32;
        let wall_collision_groups = CollisionGroups::new(wall_group, wall_group);

        let mut spawn_wall_segment = |pos: Vec3, size: Vec3| {
            if size.x < 0.6 || size.z < 0.6 || size.y < 0.6 {
                return;
            }
            commands.spawn((
                MaterialMeshBundle {
                    mesh: meshes.add(Cuboid::new(size.x, size.y, size.z)),
                    material: mat_corridor_wall.clone(),
                    transform: Transform::from_translation(pos),
                    ..default()
                },
                Spatial4D { w: cx_w, target_w: cx_w, layer: r.w_layer, is_folded: false },
                RigidBody::Fixed,
                Collider::cuboid(size.x * 0.5, size.y * 0.5, size.z * 0.5),
                wall_collision_groups.clone(),
                MapGeometry,
                CameraObstacle,
                ColorCycleTarget {
                    base_color: Color::srgb(0.78, 0.82, 0.88),
                    speed: rng.gen_range(0.045..0.11),
                    phase: rng.gen_range(0.0..std::f32::consts::TAU),
                },
            ));
        };

        // Top/Bottom walls (doors stored as x positions)
        let mut top_door_xs = r.doors.top.clone();
        top_door_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut bottom_door_xs = r.doors.bottom.clone();
        bottom_door_xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for (z_wall, door_xs) in [
            (z1 + wall_t * 0.5, &top_door_xs),
            (z0 - wall_t * 0.5, &bottom_door_xs),
        ] {
            let mut cursor = x0;
            for &door_x in door_xs {
                let gap_a = (door_x - door_half).clamp(x0, x1);
                let gap_b = (door_x + door_half).clamp(x0, x1);
                if gap_a > cursor + 0.6 {
                    let seg_a = cursor;
                    let seg_b = gap_a;
                    let seg_len = (seg_b - seg_a).abs();
                    let pos = Vec3::new((seg_a + seg_b) * 0.5, wall_y, z_wall);
                    let size = Vec3::new(seg_len + wall_t, visual_wall_h, wall_t);
                    spawn_wall_segment(pos, size);
                }
                cursor = cursor.max(gap_b);
            }
            if x1 > cursor + 0.6 {
                let seg_a = cursor;
                let seg_b = x1;
                let seg_len = (seg_b - seg_a).abs();
                let pos = Vec3::new((seg_a + seg_b) * 0.5, wall_y, z_wall);
                let size = Vec3::new(seg_len + wall_t, visual_wall_h, wall_t);
                spawn_wall_segment(pos, size);
            }
        }

        // Left/Right walls (doors stored as y positions -> world z)
        let mut left_door_zs = r.doors.left.clone();
        left_door_zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut right_door_zs = r.doors.right.clone();
        right_door_zs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for (x_wall, door_zs) in [
            (x0 - wall_t * 0.5, &left_door_zs),
            (x1 + wall_t * 0.5, &right_door_zs),
        ] {
            let mut cursor = z0;
            for &door_z in door_zs {
                let gap_a = (door_z - door_half).clamp(z0, z1);
                let gap_b = (door_z + door_half).clamp(z0, z1);
                if gap_a > cursor + 0.6 {
                    let seg_a = cursor;
                    let seg_b = gap_a;
                    let seg_len = (seg_b - seg_a).abs();
                    let pos = Vec3::new(x_wall, wall_y, (seg_a + seg_b) * 0.5);
                    let size = Vec3::new(wall_t, visual_wall_h, seg_len + wall_t);
                    spawn_wall_segment(pos, size);
                }
                cursor = cursor.max(gap_b);
            }
            if z1 > cursor + 0.6 {
                let seg_a = cursor;
                let seg_b = z1;
                let seg_len = (seg_b - seg_a).abs();
                let pos = Vec3::new(x_wall, wall_y, (seg_a + seg_b) * 0.5);
                let size = Vec3::new(wall_t, visual_wall_h, seg_len + wall_t);
                spawn_wall_segment(pos, size);
            }
        }

        // Python parity: _add_pillars — 4 pillars at 25%/75% room extents, skipped if inside an entry lane.
        // Original sizing: box scale (0.3, 0.3, wall_h/2) at z=base_z+wall_h/2; we cap visual height like walls.
        let pxs = [r.x + r.w * 0.25, r.x + r.w * 0.75];
        let pzs = [r.y + r.h * 0.25, r.y + r.h * 0.75];
        // Mirror original _effective_entry_opening_width / _is_in_room_entry_lane logic.
        let room_min = (r.w.min(r.h)).max(2.2);
        let max_open = (room_min - 1.45).max(1.8);
        let base_open = {
            let max_open0 = (room_min - 2.0).max(1.6);
            let target = config.corridor_width + wall_t * 1.1;
            (target.min(max_open0)).max(1.6)
        };
        let ball_r = crate::player::BALL_RADIUS;
        let clearance = 0.28_f32.max(wall_t * 1.4).max(ball_r * 1.6);
        let door_w = (base_open + clearance).min(max_open);
        let lane_half = (door_w * 0.56).max(config.corridor_width * 0.58) + 0.42;
        let inset = 1.05_f32.max(config.corridor_width * 0.42) + 0.42;

        let is_in_entry_lane = |x: f32, z: f32| -> bool {
            // Equivalent of original room_doors dict checks.
            if x <= r.x + inset {
                for &door_pos in &r.doors.left {
                    if (z - door_pos).abs() <= lane_half {
                        return true;
                    }
                }
            }
            if x >= r.x + r.w - inset {
                for &door_pos in &r.doors.right {
                    if (z - door_pos).abs() <= lane_half {
                        return true;
                    }
                }
            }
            if z <= r.y + inset {
                for &door_pos in &r.doors.bottom {
                    if (x - door_pos).abs() <= lane_half {
                        return true;
                    }
                }
            }
            if z >= r.y + r.h - inset {
                for &door_pos in &r.doors.top {
                    if (x - door_pos).abs() <= lane_half {
                        return true;
                    }
                }
            }
            false
        };

        for &px in &pxs {
            for &pz in &pzs {
                if is_in_entry_lane(px, pz) {
                    continue;
                }
                let pillar_h = visual_wall_h;
                commands.spawn((
                    MaterialMeshBundle {
                        mesh: meshes.add(Cuboid::new(0.3, pillar_h, 0.3)),
                        material: mat_corridor_wall.clone(),
                        transform: Transform::from_xyz(px, cz + pillar_h * 0.5, pz),
                        ..default()
                    },
                    Spatial4D { w: cx_w, target_w: cx_w, layer: r.w_layer, is_folded: false },
                    RigidBody::Fixed,
                    Collider::cuboid(0.15, pillar_h * 0.5, 0.15),
                    wall_collision_groups.clone(),
                    MapGeometry,
                    CameraObstacle,
                ));
            }
        }

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
                            uv_scale: 1.0,
                            density_contrast: 1.35,
                            density_gamma: 0.85,
                            thermal_strength: 1.0,
                            compression_factor: 1.0,
                            fog_start: 0.0,
                            fog_end: 35.0,
                            fog_color: bevy::color::LinearRgba::BLACK,
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
    }

    // Spawn corridors (Python parity: _build_corridor_segment with walls + decor)
    // Only present when we actually built a BSP/hexmix dungeon.
    if build_room_geometry {
    // Corridor wall height matches full room_height for visual parity.
    let visual_wall_h = config.room_height;
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

            // Thin FloorWet overlay for corridor floors
            let wet_t = (config.floor_thickness * 0.12).max(0.01);
            let mut wet_boxes = Vec::new();
            for (p, s) in &corridor_floor_boxes {
                wet_boxes.push((
                    Vec3::new(p.x, cz + wet_t * 0.5 + 0.01, p.z),
                    Vec3::new(s.x, wet_t, s.z),
                ));
            }
            commands.spawn((
                MaterialMeshBundle {
                    mesh: meshes.add(create_merged_box_mesh(&wet_boxes)),
                    material: mat_floor_wet.clone(),
                    transform: Transform::IDENTITY,
                    ..default()
                },
                spatial.clone(),
                MapGeometry,
            ));

            // Original parity: water surface is map-wide (not per-corridor). Avoid stacking corridor water meshes to prevent flicker/z-fighting.
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
    }

    graph.rooms = rooms;
    
    // Python parity: _build_floor_and_bounds creates a map-wide transparent water surface
    // at floor_y + water_surface_raise = 0.28, covering the entire map (map_w x map_d)
    // This is the surface that produces the vibrant ROYGBIV thermal color floor patterns
    let map_half = map_w as f32 * 0.5;
    let water_overscan = 6.0;
    // Half-extents so plane and colliders cover full map + overscan (ORIGINAL_ARENA §3.1: water_half_x/y = map_w/2 + overscan)
    let boundary_half_x = map_w as f32 * 0.5 + water_overscan;
    let boundary_half_z = map_w as f32 * 0.5 + water_overscan;

    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(Plane3d::new(Vec3::Y, Vec2::new(boundary_half_x, boundary_half_z))),
            material: mat_water.clone(),
            transform: Transform::from_xyz(map_half, 0.28, map_half),
            ..default()
        },
        Spatial4D { w: 0.0, target_w: 0.0, layer: 0, is_folded: false },
        MapGeometry,
        EchoSource,
    ));

    commands.spawn((
        RigidBody::Fixed,
        Collider::cuboid(boundary_half_x, 10.0, boundary_half_z),
        Transform::from_xyz(map_half, 0.28 - 10.0, map_half),
        GlobalTransform::default(),
        CollisionGroups::new(Group::from_bits_truncate(0x7FFFFFFF), Group::from_bits_truncate(0x7FFFFFFF)),
        MapGeometry,
    ));

    let (high_y, low_y) = if config.layout_mode == "arena" {
        (15.3, -15.3)
    } else {
        (config.room_height + 4.0, -1.4)
    };

    commands.spawn((
        RigidBody::Fixed,
        Collider::cuboid(boundary_half_x, 0.11, boundary_half_z),
        Transform::from_xyz(map_half, high_y + 0.11, map_half),
        GlobalTransform::default(),
        CollisionGroups::new(Group::GROUP_32, Group::GROUP_32),
        MapGeometry,
    ));

    commands.spawn((
        RigidBody::Fixed,
        Collider::cuboid(boundary_half_x, 0.11, boundary_half_z),
        Transform::from_xyz(map_half, low_y - 0.11, map_half),
        GlobalTransform::default(),
        CollisionGroups::new(Group::GROUP_32, Group::GROUP_32),
        MapGeometry,
    ));
}

fn build_inverted_echo_world(
    mut commands: Commands,
    water_sources: Query<(&Transform, &Handle<Mesh>, &Handle<crate::rendering::WaterSurfaceMaterial>, Option<&Spatial4D>), With<EchoSource>>,
    thermal_sources: Query<(&Transform, &Handle<Mesh>, &Handle<crate::rendering::thermal::ThermalMaterial>, Option<&Spatial4D>), With<EchoSource>>,
    floor_wet_sources: Query<(&Transform, &Handle<Mesh>, &Handle<crate::rendering::FloorWetMaterial>, Option<&Spatial4D>), With<EchoSource>>,
) {
    let root_transform = Transform::from_translation(Vec3::new(0.0, 24.0, 0.0))
        .with_scale(Vec3::new(1.0, -1.0, 1.0));

    let root = commands
        .spawn((
            SpatialBundle {
                transform: root_transform,
                ..default()
            },
            InvertedEchoRoot,
        ))
        .id();

    for (transform, mesh, material, spatial) in water_sources.iter() {
        let spatial4d = spatial.copied().unwrap_or(Spatial4D {
            w: 0.0,
            target_w: 0.0,
            layer: 0,
            is_folded: false,
        });

        let echo = commands
            .spawn((
                spatial4d,
                MaterialMeshBundle {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transform: *transform,
                    ..default()
                },
            ))
            .id();

        commands.entity(echo).set_parent(root);
    }

    for (transform, mesh, material, spatial) in thermal_sources.iter() {
        let spatial4d = spatial.copied().unwrap_or(Spatial4D {
            w: 0.0,
            target_w: 0.0,
            layer: 0,
            is_folded: false,
        });

        let echo = commands
            .spawn((
                spatial4d,
                MaterialMeshBundle {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transform: *transform,
                    ..default()
                },
            ))
            .id();

        commands.entity(echo).set_parent(root);
    }

    for (transform, mesh, material, spatial) in floor_wet_sources.iter() {
        let spatial4d = spatial.copied().unwrap_or(Spatial4D {
            w: 0.0,
            target_w: 0.0,
            layer: 0,
            is_folded: false,
        });

        let echo = commands
            .spawn((
                spatial4d,
                MaterialMeshBundle {
                    mesh: mesh.clone(),
                    material: material.clone(),
                    transform: *transform,
                    ..default()
                },
            ))
            .id();

        commands.entity(echo).set_parent(root);
    }
}

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
