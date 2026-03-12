use bevy::prelude::*;

#[derive(Component)]
pub struct CeilingEntity;

#[derive(Component)]
pub struct MapGeometry;

#[derive(Component)]
pub struct ColorCycleTarget {
    pub base_color: Color,
    pub speed: f32,
    pub phase: f32,
}

#[derive(Component)]
pub struct StashedMap(pub Visibility);

#[derive(Resource, Clone, Debug)]
pub struct GenerationConfig {
    pub scale: f32,
    pub layout_mode: String,
    pub snake_cell_size: i32,
    pub snake_layers: i32,
    pub maze_cell_size: i32,
    pub maze_layers: i32,
    pub maze_loop_chance: f32,
    pub maze_vertical_link_chance: f32,
    pub average_room_size: f32,
    pub room_size_jitter: f32,
    pub room_height: f32,
    pub wall_thickness: f32,
    pub floor_thickness: f32,
    pub corridor_width: f32,
    #[allow(dead_code)]
    pub room_density: f32,
    pub corridor_density: f32,
    #[allow(dead_code)]
    pub decor_density: f32,
    #[allow(dead_code)]
    pub angled_room_ratio: f32,
    pub base_cube_unit: f32,
    pub max_rooms: i32,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            scale: 2.4,
            layout_mode: "arena".to_string(),
            snake_cell_size: 8,
            snake_layers: 1,
            maze_cell_size: 32,
            maze_layers: 1,
            maze_loop_chance: 0.15,
            maze_vertical_link_chance: 0.13,
            average_room_size: 120.0, // MASSIVE open space feel
            room_size_jitter: 0.85,
            room_height: 4096.0, 
            wall_thickness: 0.2, 
            floor_thickness: 0.2,
            corridor_width: 32.5, 
            room_density: 0.88,
            corridor_density: 0.15, // Fewer narrow corridors
            decor_density: 0.1,
            angled_room_ratio: 0.1,
            base_cube_unit: 1.0,
            max_rooms: 12,
        }
    }
}

#[derive(Resource, Default)]
#[allow(dead_code)]
pub struct DungeonGraph {
    pub rooms: Vec<Room>,
    pub corridors: Vec<Corridor>,
    pub edges: Vec<(usize, usize)>,
    pub warp_links: Vec<crate::components::WarpLink>,
}

#[derive(Component, Debug, Clone, Copy)]
pub struct DimensionField {
    pub base: f32,
    pub amp: f32,
    pub freq: f32,
    pub phase: f32,
    pub center_bias: f32,
    pub edge_bias: f32,
}

impl Default for DimensionField {
    fn default() -> Self {
        Self {
            base: 1.0,
            amp: 0.1,
            freq: 1.0,
            phase: 0.0,
            center_bias: 1.0,
            edge_bias: 0.0,
        }
    }
}

#[derive(Clone, Debug, Copy)]
pub struct CompressionPocket {
    pub position: Vec2,
    pub radius: f32,
    pub factor: f32, // < 1.0 (compression) or > 1.0 (dilation)
}

#[derive(Default, Clone, Debug)]
pub struct RoomDoors {
    pub top: Vec<f32>,
    pub bottom: Vec<f32>,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

#[derive(Clone, Debug, Component)]
pub struct Room {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub w_layer: i32,
    pub _id: usize,
    pub dimension_field: DimensionField,
    pub pockets: Vec<CompressionPocket>,
    pub doors: RoomDoors,
}

impl Room {
    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

#[allow(dead_code)]
pub struct Corridor {
    pub start: Vec3,
    pub end: Vec3,
    pub w_layer: i32,
}
