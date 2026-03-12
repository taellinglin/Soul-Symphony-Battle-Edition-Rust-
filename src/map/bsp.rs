use rand::{Rng, seq::SliceRandom, thread_rng};
use std::collections::{HashSet, HashMap};

use bevy::prelude::Vec2;
use super::types::*;

pub(crate) struct BspState {
    pub width: i32,
    pub depth: i32,
    pub config: GenerationConfig,
}

#[allow(dead_code)]
impl BspState {
    fn cube_unit(&self) -> f32 {
        self.config.base_cube_unit.max(0.25)
    }

    fn snap(&self, v: f32) -> f32 {
        let unit = self.cube_unit();
        (v / unit).round() * unit
    }

    fn snap_room(&self, x: f32, y: f32, w: f32, h: f32) -> Room {
        let unit = self.cube_unit();
        let min_size = (unit * 3.0).max(2.5);
        let sw = self.snap(w).max(min_size);
        let sh = self.snap(h).max(min_size);
        let mut sx = self.snap(x);
        let mut sy = self.snap(y);

        sx = sx.clamp(0.0, (self.width as f32 - sw).max(0.0));
        sy = sy.clamp(0.0, (self.depth as f32 - sh).max(0.0));
        
        let mut pockets = Vec::new();
        let area = sw * sh;
        let pocket_count = (area / 120.0).max(1.0) as i32; // ~1 pocket per 120 units squared
        let mut rng = rand::thread_rng();

        for _ in 0..pocket_count {
            if rng.gen_bool(0.65) {
                let px = sx + rng.gen_range(2.0..(sw - 2.0).max(2.1));
                let py = sy + rng.gen_range(2.0..(sh - 2.0).max(2.1));
                let radius = rng.gen_range(4.0..12.0);
                
                // Usually compress (< 1.0), sometimes dilate (> 1.0)
                let factor = if rng.gen_bool(0.8) {
                    rng.gen_range(0.45..0.9)
                } else {
                    rng.gen_range(1.1..1.8)
                };

                pockets.push(CompressionPocket {
                    position: Vec2::new(px, py),
                    radius,
                    factor,
                });
            }
        }

        Room { 
            x: sx, y: sy, w: sw, h: sh, w_layer: 0, _id: 0, 
            dimension_field: DimensionField::default(), 
            pockets,
            doors: RoomDoors::default() 
        }
    }

    pub fn fit_room_to_cell(&self, gx: i32, gy: i32, size: f32, room_w: f32, room_h: f32, pad: f32) -> Room {
        let mut rng = thread_rng();
        let corridor_pad = 0.5f32.max((size * 0.22).min(self.config.corridor_width * 0.18));
        let edge_pad = pad.max(corridor_pad);
        let max_dim = 3.0f32.max(size - edge_pad * 2.0);
        let rw = room_w.max(2.8).min(max_dim);
        let rh = room_h.max(2.8).min(max_dim);

        let rx = gx as f32 * size + (size - rw) * 0.5;
        let ry = gy as f32 * size + (size - rh) * 0.5;
        
        let mut room = self.snap_room(rx, ry, rw, rh);
        room.dimension_field = DimensionField {
            base: rng.gen_range(0.85..1.15),
            amp: rng.gen_range(0.05..0.25),
            freq: rng.gen_range(0.5..2.5),
            phase: rng.gen_range(0.0..std::f32::consts::TAU),
            center_bias: rng.gen_range(0.6..1.4),
            edge_bias: rng.gen_range(-0.4..0.4),
        };
        room
    }

    pub fn generate_hex_mixed(&self, cell_size: i32) -> (Vec<Room>, Vec<(usize, usize)>) {
        let mut rng = thread_rng();
        let size = cell_size.max(12) as f32;
        let step_x = size * 0.9;
        let step_y = size * 0.78;
        let margin = size * 0.6;
        let jitter = self.config.room_size_jitter.clamp(0.0, 0.75);

        let rows = 3.max(((self.depth as f32 - margin * 2.0) / step_y) as i32);
        let cols = 3.max(((self.width as f32 - margin * 2.0) / step_x) as i32);

        let room_budget = self.config.max_rooms.max(8);
        let mut cells = Vec::new();
        for r in 0..rows {
            let x_offset = if r % 2 == 1 { step_x * 0.5 } else { 0.0 };
            for c in 0..cols {
                let cx = margin + (c as f32) * step_x + x_offset;
                let cy = margin + (r as f32) * step_y;
                if cx < margin || cy < margin || cx > (self.width as f32 - margin) || cy > (self.depth as f32 - margin) {
                    continue;
                }
                cells.push((c, r, cx, cy));
            }
        }

        if cells.len() > room_budget as usize {
            cells.shuffle(&mut rng);
            cells.truncate(room_budget as usize);
        }

        let mut rooms = Vec::new();
        let mut index_map = HashMap::new();
        let mut temp_cells = Vec::new();

        for (c, r, cx, cy) in cells.into_iter() {
            let shape_roll: f32 = rng.gen();
            let rw; let rh;
            
            if shape_roll < 0.54 {
                rw = size * rng.gen_range(0.72..0.96); rh = size * rng.gen_range(0.66..0.9);
            } else if shape_roll < 0.68 {
                let base = size * rng.gen_range(0.58..0.82); rw = base; rh = base;
            } else if shape_roll < 0.8 {
                rw = size * rng.gen_range(1.0..1.38); rh = size * rng.gen_range(0.45..0.7);
            } else if shape_roll < 0.92 {
                rw = size * rng.gen_range(0.45..0.7); rh = size * rng.gen_range(1.0..1.38);
            } else {
                rw = size * rng.gen_range(0.6..0.95); rh = size * rng.gen_range(0.6..0.95);
            }

            let rw_jittered = rw * rng.gen_range((1.0 - jitter * 0.45)..(1.0 + jitter * 0.4));
            let rh_jittered = rh * rng.gen_range((1.0 - jitter * 0.45)..(1.0 + jitter * 0.4));

            temp_cells.push((c, r, cx, cy, rw_jittered, rh_jittered));
        }

        for (c, r, cx, cy, rw, rh) in temp_cells {
            let rx = cx - rw * 0.5;
            let ry = cy - rh * 0.5;
            let mut room = self.snap_room(rx, ry, rw, rh);
            room.w_layer = 0;
            
            let idx = rooms.len();
            rooms.push(room);
            index_map.insert((c, r), idx);
        }

        let mut edges_set = HashSet::new();
        for (c, r) in index_map.keys().copied().collect::<Vec<_>>() {
            let a = index_map[&(c, r)];
            let mut neighbors = Vec::new();
            if r % 2 == 0 {
                neighbors.extend_from_slice(&[(c - 1, r), (c + 1, r), (c, r - 1), (c - 1, r - 1), (c, r + 1), (c - 1, r + 1)]);
            } else {
                neighbors.extend_from_slice(&[(c - 1, r), (c + 1, r), (c, r - 1), (c + 1, r - 1), (c, r + 1), (c + 1, r + 1)]);
            }

            for (nc, nr) in neighbors {
                if let Some(&b) = index_map.get(&(nc, nr)) {
                    if a != b { let mut k = [a, b]; k.sort(); edges_set.insert((k[0], k[1])); }
                }
            }

            if rng.gen_bool(0.32) {
                for (nc, nr) in [(c + 2, r), (c - 2, r), (c, r + 2), (c, r - 2)] {
                    if let Some(&b) = index_map.get(&(nc, nr)) {
                        if a != b { let mut k = [a, b]; k.sort(); edges_set.insert((k[0], k[1])); }
                    }
                }
            }
        }

        if edges_set.is_empty() && rooms.len() > 1 {
            for i in 0..(rooms.len() - 1) { edges_set.insert((i, i + 1)); }
        }

        (rooms, edges_set.into_iter().collect())
    }

    pub fn generate_snake3d(&self, cell_size: i32, layers: i32) -> (Vec<Room>, Vec<(usize, usize)>) {
        let mut rng = thread_rng();
        let size = cell_size.max(10) as f32;
        let cols = 3.max((self.width as f32 / size) as i32);
        let rows = 3.max((self.depth as f32 / size) as i32);
        let layer_count = layers.max(2);
        
        let room_budget = self.config.max_rooms.max(8);
        let jitter = self.config.room_size_jitter.clamp(0.0, 0.7);
        
        let pad = 0.45f32.max((size * 0.14).min(2.4));
        let base_room_w = 3.0f32.max(size - pad * 2.0);
        let base_room_h = 3.0f32.max(size - pad * 2.0);

        let mut cells_2d = Vec::new();
        for gy in 0..rows {
            if gy % 2 == 0 {
                for gx in 0..cols { cells_2d.push((gx, gy)); }
            } else {
                for gx in (0..cols).rev() { cells_2d.push((gx, gy)); }
            }
        }

        let mut rooms = Vec::new();
        let mut edge_set = HashSet::new();
        let mut previous_idx: Option<usize> = None;

        for gz in 0..layer_count {
            let layer_cells = if gz % 2 == 0 { cells_2d.clone() } else { cells_2d.iter().copied().rev().collect() };
            for (gx, gy) in layer_cells {
                if rooms.len() >= room_budget as usize { break; }
                
                let mut rw = base_room_w * rng.gen_range((1.0 - jitter * 0.35)..(1.0 + jitter * 0.2));
                let mut rh = base_room_h * rng.gen_range((1.0 - jitter * 0.35)..(1.0 + jitter * 0.2));
                rw = 2.8f32.max(rw).min(size - 0.35);
                rh = 2.8f32.max(rh).min(size - 0.35);

                let idx = rooms.len();
                let mut room = self.fit_room_to_cell(gx, gy, size, rw, rh, pad);
                room.w_layer = gz;
                rooms.push(room);

                if let Some(prev) = previous_idx {
                    let mut k = [prev, idx]; k.sort();
                    edge_set.insert((k[0], k[1]));
                }
                previous_idx = Some(idx);
            }
            if rooms.len() >= room_budget as usize { break; }
        }

        (rooms, edge_set.into_iter().collect())
    }

    pub fn generate_maze3d(&self, cell_size: i32, layers: i32, _loop_chance: f32, v_link_chance: f32) -> (Vec<Room>, Vec<(usize, usize)>) {
        let mut rng = thread_rng();
        let size = cell_size.max(16) as f32;
        let cols = 3.max((self.width as f32 / size) as i32);
        let rows = 3.max((self.depth as f32 / size) as i32);
        let layer_count = layers.max(2);
        
        let _room_budget = self.config.max_rooms.max(8);

        let pad = 0.5f32.max((size * 0.16).min(3.0));
        let base_room_w = 3.0f32.max(size - pad * 2.0);
        let base_room_h = 3.0f32.max(size - pad * 2.0);

        let mut rooms = Vec::new();
        let mut index_map = HashMap::new();

        for gz in 0..layer_count {
            for gy in 0..rows {
                for gx in 0..cols {
                    let mut rw = base_room_w * rng.gen_range(0.72..1.18);
                    let mut rh = base_room_h * rng.gen_range(0.72..1.18);
                    rw = 2.8f32.max(rw).min(size - 0.4);
                    rh = 2.8f32.max(rh).min(size - 0.4);

                    let idx = rooms.len();
                    let mut room = self.fit_room_to_cell(gx, gy, size, rw, rh, pad);
                    room.w_layer = gz;
                    rooms.push(room);
                    index_map.insert((gx, gy, gz), idx);
                }
            }
        }

        let mut visited = HashSet::new();
        let mut stack = vec![(0, 0, 0)];
        visited.insert((0, 0, 0));
        let mut edge_set = HashSet::new();

        while let Some(current) = stack.last().copied() {
            let (cx, cy, cz) = current;
            let mut lateral = Vec::new();
            let mut vertical = Vec::new();

            if cx + 1 < cols { lateral.push((cx + 1, cy, cz)); }
            if cx > 0 { lateral.push((cx - 1, cy, cz)); }
            if cy + 1 < rows { lateral.push((cx, cy + 1, cz)); }
            if cy > 0 { lateral.push((cx, cy - 1, cz)); }
            if cz + 1 < layer_count { vertical.push((cx, cy, cz + 1)); }
            if cz > 0 { vertical.push((cx, cy, cz - 1)); }

            lateral.shuffle(&mut rng);
            vertical.shuffle(&mut rng);
            let mut unvisited_lat: Vec<_> = lateral.into_iter().filter(|n| !visited.contains(n)).collect();
            let mut unvisited_vert: Vec<_> = vertical.into_iter().filter(|n| !visited.contains(n)).collect();

            let nxt = if !unvisited_lat.is_empty() && (unvisited_vert.is_empty() || rng.gen::<f32>() > v_link_chance) {
                unvisited_lat.pop().unwrap()
            } else if !unvisited_vert.is_empty() {
                unvisited_vert.pop().unwrap()
            } else if !unvisited_lat.is_empty() {
                unvisited_lat.pop().unwrap()
            } else {
                stack.pop();
                continue;
            };

            let a = index_map[&current];
            let b = index_map[&nxt];
            let mut k = [a, b]; k.sort();
            edge_set.insert((k[0], k[1]));
            visited.insert(nxt);
            stack.push(nxt);
        }

        (rooms, edge_set.into_iter().collect())
    }

    pub fn generate_labyrinth(&self, cell_size: i32) -> (Vec<Room>, Vec<(usize, usize)>) {
        let mut rng = thread_rng();
        let size = cell_size.max(8) as f32;
        let mut cols = 4.max((self.width as f32 / size) as i32);
        let mut rows = 4.max((self.depth as f32 / size) as i32);
        
        let room_budget = self.config.max_rooms.max(8).min(256) as i32;
        while cols * rows > room_budget && (cols > 4 || rows > 4) {
            if cols >= rows && cols > 4 {
                cols -= 1;
            } else if rows > 4 {
                rows -= 1;
            } else {
                break;
            }
        }

        let pad = 0.4f32.max((size * 0.16).min(2.4));
        let base_room_w = 3.2f32.max(size - pad * 2.0);
        let base_room_h = 3.2f32.max(size - pad * 2.0);
        let jitter = self.config.room_size_jitter.clamp(0.0, 0.75);

        let mut rooms = Vec::new();
        let mut index_map = HashMap::new();

        for gy in 0..rows {
            for gx in 0..cols {
                let mut size_scale = rng.gen_range((1.0 - jitter * 0.55)..(1.0 + jitter * 0.38));
                let aspect_skew = rng.gen_range((-jitter * 0.48)..(jitter * 0.48));
                if rng.gen_bool((0.14 + jitter * 0.22) as f64) {
                    size_scale *= rng.gen_range(0.78..1.24);
                }

                let room_w = 2.8_f32.max(base_room_w * size_scale * (1.0 + aspect_skew));
                let room_h = 2.8_f32.max(base_room_h * size_scale * (1.0 - aspect_skew));
                
                let idx = rooms.len();
                rooms.push(self.fit_room_to_cell(gx, gy, size, room_w, room_h, pad));
                index_map.insert((gx, gy), idx);
            }
        }

        // DFS based Maze Generation
        let mut visited = HashSet::new();
        let mut stack = vec![(0, 0)];
        visited.insert((0, 0));
        let mut edges = Vec::new();

        while let Some(current) = stack.last().copied() {
            let (cx, cy) = current;
            let mut neighbors = Vec::new();
            for (nx, ny) in [(cx + 1, cy), (cx - 1, cy), (cx, cy + 1), (cx, cy - 1)] {
                if nx >= 0 && nx < cols && ny >= 0 && ny < rows {
                    neighbors.push((nx, ny));
                }
            }
            neighbors.shuffle(&mut rng);
            
            let mut nxt_opt = None;
            for n in neighbors {
                if !visited.contains(&n) {
                    nxt_opt = Some(n);
                    break;
                }
            }

            if let Some(nxt) = nxt_opt {
                let a = index_map[&current];
                let b = index_map[&nxt];
                edges.push((a, b));
                visited.insert(nxt);
                stack.push(nxt);
            } else {
                stack.pop();
            }
        }

        // Add random loop extras based on corridor_density
        let mut existing: HashSet<(usize, usize)> = HashSet::new();
        for &(a, b) in &edges {
            let mut sorted = [a, b];
            sorted.sort();
            existing.insert((sorted[0], sorted[1]));
        }

        let mut all_neighbor_pairs = Vec::new();
        for gy in 0..rows {
            for gx in 0..cols {
                let a = index_map[&(gx, gy)];
                if gx + 1 < cols {
                    all_neighbor_pairs.push((a, index_map[&(gx + 1, gy)]));
                }
                if gy + 1 < rows {
                    all_neighbor_pairs.push((a, index_map[&(gx, gy + 1)]));
                }
            }
        }

        let loop_factor = 0.01_f32.max((self.config.corridor_density * 0.14).min(0.2));
        let mut extras = 1.max((edges.len() as f32 * loop_factor) as usize);
        all_neighbor_pairs.shuffle(&mut rng);
        
        for (a, b) in all_neighbor_pairs {
            let mut k = [a, b];
            k.sort();
            if existing.contains(&(k[0], k[1])) {
                continue;
            }
            edges.push((a, b));
            existing.insert((k[0], k[1]));
            if extras > 0 {
                extras -= 1;
            } else {
                break;
            }
        }

        (rooms, edges)
    }
}
