#![allow(dead_code)]
use bevy::prelude::*;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;
use crate::map::DungeonGraph;
use crate::components::Spatial4D;
use crate::player::Player;
use crate::rendering::{HyperSliceMaterial, HyperSliceSettings, HyperSliceExtension};

pub struct NavigationPlugin;

impl Plugin for NavigationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PathfindingState>()
           .add_systems(Update, (calculate_path, update_visual_trail).chain());
    }
}

#[derive(Component)]
pub struct Goal;

#[derive(Component)]
pub struct NavWaypoint;

#[derive(Resource, Default)]
pub struct PathfindingState {
    pub current_path: Vec<(Vec3, f32)>, // Pos, W-coordinate
    pub recalculation_timer: Timer,
}

#[derive(Copy, Clone, PartialEq)]
struct AStarNode {
    index: usize,
    f_cost: f32, // G + H
}

impl Eq for AStarNode {}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.f_cost.partial_cmp(&self.f_cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn calculate_path(
    time: Res<Time>,
    mut state: ResMut<PathfindingState>,
    graph: Res<DungeonGraph>,
    player_query: Query<(&Transform, &Spatial4D), With<Player>>,
    goal_query: Query<(&Transform, &Spatial4D), With<Goal>>,
) {
    if graph.rooms.is_empty() { return; }

    state.recalculation_timer.tick(time.delta());
    if !state.recalculation_timer.just_finished() && !state.recalculation_timer.duration().is_zero() {
        return;
    }
    state.recalculation_timer = Timer::from_seconds(1.2, TimerMode::Repeating); // Throttled update

    if let Ok((player_tf, player_sp)) = player_query.get_single() {
        if let Ok((goal_tf, goal_sp)) = goal_query.get_single() {
            let start_pos = player_tf.translation;
            let start_w = player_sp.w;
            let end_pos = goal_tf.translation;
            let end_w = goal_sp.w;

            // 1. Find nearest graph nodes
            let mut start_idx = 0;
            let mut start_dist = f32::MAX;
            let mut end_idx = 0;
            let mut end_dist = f32::MAX;

            for (i, room) in graph.rooms.iter().enumerate() {
                let room_center = Vec3::new(room.center().x, 0.0, room.center().y);
                let room_w = (room.w_layer as f32) * 5.0;

                let d_start = start_pos.distance(room_center) + (start_w - room_w).abs();
                if d_start < start_dist {
                    start_dist = d_start;
                    start_idx = i;
                }

                let d_end = end_pos.distance(room_center) + (end_w - room_w).abs();
                if d_end < end_dist {
                    end_dist = d_end;
                    end_idx = i;
                }
            }

            // 2. A* Pathfinding using explicit edges (already partially implemented but needs cleaning)
            let mut open_set = BinaryHeap::new();
            let mut g_score: HashMap<usize, f32> = HashMap::new();
            let mut came_from: HashMap<usize, usize> = HashMap::new();

            g_score.insert(start_idx, 0.0);
            open_set.push(AStarNode { index: start_idx, f_cost: 0.0 });

            while let Some(current) = open_set.pop() {
                if current.index == end_idx {
                    let mut path = Vec::new();
                    let mut curr = current.index;
                    while curr != start_idx {
                        let room = &graph.rooms[curr];
                        path.push((Vec3::new(room.center().x, 0.0, room.center().y), (room.w_layer as f32) * 5.0));
                        if let Some(&prev) = came_from.get(&curr) { curr = prev; } else { break; }
                    }
                    path.reverse();
                    state.current_path = path;
                    return;
                }

                let current_score = *g_score.get(&current.index).unwrap_or(&f32::MAX);
                for &(a, b) in &graph.edges {
                    let n_idx = if a == current.index { b } else if b == current.index { a } else { continue };
                    
                    let neighbor = &graph.rooms[n_idx];
                    let current_room = &graph.rooms[current.index];
                    let dist = Vec3::new(current_room.center().x, 0.0, current_room.center().y).distance(Vec3::new(neighbor.center().x, 0.0, neighbor.center().y))
                             + ((neighbor.w_layer - current_room.w_layer) as f32).abs() * 5.0;

                    let tentative_g = current_score + dist;
                    if tentative_g < *g_score.get(&n_idx).unwrap_or(&f32::MAX) {
                        came_from.insert(n_idx, current.index);
                        g_score.insert(n_idx, tentative_g);
                        let h = Vec3::new(neighbor.center().x, 0.0, neighbor.center().y).distance(Vec3::new(graph.rooms[end_idx].center().x, 0.0, graph.rooms[end_idx].center().y));
                        open_set.push(AStarNode { index: n_idx, f_cost: tentative_g + h });
                    }
                }
            }
        }
    }
}

fn update_visual_trail(
    mut commands: Commands,
    state: Res<PathfindingState>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HyperSliceMaterial>>,
    existing_waypoints: Query<Entity, With<NavWaypoint>>,
) {
    if state.is_changed() {
        for ent in existing_waypoints.iter() {
            commands.entity(ent).despawn_recursive();
        }

        for (pos, w) in &state.current_path {
            commands.spawn((
                NavWaypoint,
                Spatial4D {
                    w: *w, target_w: *w, layer: (*w / 5.0).round() as i32, is_folded: false
                },
                MaterialMeshBundle {
                    mesh: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                    material: materials.add(HyperSliceMaterial {
                        base: StandardMaterial {
                            base_color: Color::srgb(1.0, 0.84, 0.0), // Golden glowing path
                            emissive: LinearRgba::new(1.0, 0.84, 0.0, 1.0),
                            ..default()
                        },
                        extension: HyperSliceExtension {
                            settings: HyperSliceSettings {
                                player_w: 0.0,
                                object_w: *w,
                                thickness: 5.0, // Fade out softly via WGSL shader
                                edge_color: LinearRgba::new(1.0, 1.0, 0.0, 1.0),
                                room_uv_scale: 1.0,
                                ..default()
                            },
                            base_texture: None,
                        },
                    }),
                    transform: Transform::from_xyz(pos.x, 2.0, pos.z),
                    ..default()
                },
            ));
        }
    }
}
