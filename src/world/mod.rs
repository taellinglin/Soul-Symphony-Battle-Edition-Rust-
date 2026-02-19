pub mod arena_fx;
pub mod camera;
pub mod level;
pub mod player;
pub mod weapons;

use arena_fx::ArenaFx;
use camera::FollowCamera;
use level::{Level, LevelKind};
use macroquad::models::{draw_mesh, Mesh, Vertex};
use macroquad::prelude::*;
use player::Player;
use ::rand::{thread_rng, Rng};
use weapons::WeaponsSystem;
use std::path::Path;

pub struct World {
    level: Level,
    player: Player,
    camera: FollowCamera,
    weapons: WeaponsSystem,
    arena_fx: ArenaFx,
    time: f32,
    compression_factor_smoothed: f32,
    compression_smooth_speed: f32,
    compression_pocket_radius_scale: f32,
    compression_pocket_influence_power: f32,
    compression_pocket_dilation_gain: f32,
    compression_pocket_speed_bias: f32,
    enable_dimensional_compression: bool,
    room_dimension_fields: Vec<RoomDimensionField>,
    room_compression_pockets: Vec<CompressionPocket>,
    player_hp: f32,
    player_hp_max: f32,
    player_xp: f32,
    player_xp_next: f32,
    player_level: i32,
    monsters_total: i32,
    monsters_slain: i32,
    last_events: WorldEvents,
    last_level_kind: LevelKind,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WorldEvents {
    pub audio: crate::game::audio::AudioEvents,
}

#[derive(Clone, Copy, Debug)]
struct RoomDimensionField {
    base: f32,
    amp: f32,
    freq: f32,
    phase: f32,
    center_bias: f32,
    edge_bias: f32,
    persp_u: f32,
    persp_v: f32,
    aspect: f32,
}

#[derive(Clone, Copy, Debug)]
struct CompressionPocket {
    #[allow(dead_code)]
    room_idx: usize,
    center: Vec3,
    radius: f32,
    compression: f32,
    dilation: f32,
    scale: Vec3,
    phase: f32,
}

impl World {
    pub async fn new() -> Self {
        let level = Level::main_arena();
        let level_kind = level.kind;
        let mut player = Player::new(level.spawn_point());
        let water_tex = load_texture("assets/graphics/water/water_base.png")
            .await
            .expect("Failed to load water texture");
        let ball_tex = load_random_ball_texture("assets/graphics/ball").await;
        if let Some(tex) = ball_tex.clone() {
            tex.set_filter(FilterMode::Linear);
            player.set_ball_texture(Some(tex));
        }

        let mut world = Self {
            level,
            player,
            camera: FollowCamera::new(),
            weapons: WeaponsSystem::new(),
            arena_fx: ArenaFx::new(water_tex),
            time: 0.0,
            compression_factor_smoothed: 1.0,
            compression_smooth_speed: 8.0,
            compression_pocket_radius_scale: 2.4,
            compression_pocket_influence_power: 0.78,
            compression_pocket_dilation_gain: 1.55,
            compression_pocket_speed_bias: 0.24,
            enable_dimensional_compression: true,
            room_dimension_fields: Vec::new(),
            room_compression_pockets: Vec::new(),
            player_hp: 120.0,
            player_hp_max: 120.0,
            player_xp: 0.0,
            player_xp_next: 100.0,
            player_level: 1,
            monsters_total: 0,
            monsters_slain: 0,
            last_events: WorldEvents::default(),
            last_level_kind: level_kind,
        };
        world.rebuild_dimension_fields();
        world.rebuild_compression_pockets();
        world
    }

    pub fn update(&mut self, dt: f32) {
        self.time += dt;
        if is_key_pressed(KeyCode::Key1) {
            self.set_level(LevelKind::MainArena);
        }
        if is_key_pressed(KeyCode::Key2) || is_key_pressed(KeyCode::B) {
            self.set_level(LevelKind::BossArena);
        }
        self.camera.update_input(dt);
        let camera_forward = self.camera.forward_for_target(self.player.position());
        let weapon_forward = -camera_forward;
        let water_height = self.level.floor_z() + 0.12 + 0.28;
        let jumped = self.player.update(
            dt,
            self.level.floor_z(),
            self.level.bounds(),
            -camera_forward,
            Some(water_height),
            true,
        );
        let (wrapped, delta) = self.wrap_xy_position(self.player.position(), 0.35);
        if delta.length_squared() > 1.0e-8 {
            let velocity = self.player.velocity();
            self.player.set_position_and_velocity(wrapped, velocity);
            self.camera.nudge(delta);
        }
        self.camera.update_follow(
            self.player.position(),
            dt,
            self.level.bounds(),
            self.level.floor_z(),
            water_height,
        );
        self.weapons.update(
            dt,
            self.player.position(),
            weapon_forward,
            self.level.floor_z(),
            water_height,
            self.level.bounds(),
        );
        let weapon_events = self.weapons.take_events();
        let boss_enter = self.level.kind == LevelKind::BossArena && self.last_level_kind != LevelKind::BossArena;
        self.last_level_kind = self.level.kind;
        self.last_events.audio = crate::game::audio::AudioEvents {
            jump: jumped,
            rocket: weapon_events.rocket,
            bomb: weapon_events.bomb,
            spin: weapon_events.spin,
            throw_attack: weapon_events.throw_attack,
            boss_enter,
        };
        let raw_compression = if self.enable_dimensional_compression {
            self.compression_factor_at(self.player.position(), self.time)
        } else {
            1.0
        };
        let smooth_alpha = (dt * self.compression_smooth_speed).min(1.0);
        self.compression_factor_smoothed +=
            (raw_compression - self.compression_factor_smoothed) * smooth_alpha;
        let compression = self.compression_factor_smoothed;
        self.player.apply_compression_response(compression, dt);
        let thermal = 1.0;
        self.arena_fx.set_compression_factor(compression);
        self.arena_fx.set_thermal_strength(thermal);
        let pseudo_w = (compression - 1.0).clamp(-0.9, 0.9) * 2.25;
        self.arena_fx.set_player_w(pseudo_w);
        self.arena_fx
            .set_corridor_w(self.level.map_w.max(self.level.map_d).max(1.0));
        self.arena_fx.set_level_z_step(6.0);
        let floor_z = self.level.floor_z();
        let world_min = vec3(0.0, 0.0, floor_z);
        let world_max = vec3(self.level.map_w, self.level.map_d, floor_z + 6.5);
        self.arena_fx.set_world_bounds(world_min, world_max);
        self.arena_fx
            .set_fog(vec3(0.02, 0.05, 0.08), 0.8, 18.0);
        self.arena_fx.update(dt);
    }

    pub fn draw(&self, render_target: Option<RenderTarget>) {
        self.camera.apply_with_target(render_target);
        clear_background(BLACK);
        self.level.draw();
        let floor_z = self.level.floor_z();
        let center = vec3(self.level.map_w * 0.5, self.level.map_d * 0.5, floor_z + 0.12);
        let water_uv_scale = 0.06;
        let room_uv_scale = (1.0 / self.level.map_w.max(self.level.map_d).max(1.0)) * 1000.0;
        let room_center = vec3(self.level.map_w * 0.5, self.level.map_d * 0.5, floor_z + 0.02);
        self.arena_fx
            .draw_room_thermal(room_center, vec2(self.level.map_w, self.level.map_d), room_uv_scale);
        let inverted_z = floor_z - 6.5;
        let inverted_center = vec3(self.level.map_w * 0.5, self.level.map_d * 0.5, inverted_z + 0.02);
        self.arena_fx.draw_room_thermal(
            inverted_center,
            vec2(self.level.map_w * 0.96, self.level.map_d * 0.96),
            room_uv_scale,
        );
        self.draw_compression_slice(floor_z);
        self.arena_fx
            .draw_water(center, vec2(self.level.map_w, self.level.map_d), water_uv_scale);
        self.player.draw(Color::from_rgba(230, 200, 90, 255));
        let weapon_forward = -self.camera.forward();
        let water_height = self.level.floor_z() + 0.12 + 0.28;
        self.weapons
            .draw(self.player.position(), weapon_forward, water_height);
        set_default_camera();
    }

    pub fn hud_stats(&self) -> HudStats {
        HudStats {
            hp: self.player_hp,
            hp_max: self.player_hp_max,
            xp: self.player_xp,
            xp_next: self.player_xp_next,
            level: self.player_level,
            monsters_total: self.monsters_total,
            monsters_slain: self.monsters_slain,
            time: self.time,
        }
    }

    pub fn compression_factor(&self) -> f32 {
        self.compression_factor_smoothed
    }

    pub fn take_events(&mut self) -> WorldEvents {
        let events = self.last_events;
        self.last_events = WorldEvents::default();
        events
    }

    fn set_level(&mut self, kind: LevelKind) {
        if self.level.kind == kind {
            return;
        }

        self.level = match kind {
            LevelKind::MainArena => Level::main_arena(),
            LevelKind::BossArena => Level::boss_arena(),
        };
        self.player.set_position(self.level.spawn_point());
        self.compression_factor_smoothed = 1.0;
        self.rebuild_dimension_fields();
        self.rebuild_compression_pockets();
    }

    fn wrap_xy_position(&self, pos: Vec3, margin: f32) -> (Vec3, Vec3) {
        let mut wrapped = pos;
        let mut delta = Vec3::ZERO;
        let span_x = self.level.map_w;
        let span_y = self.level.map_d;
        let low_x = -margin;
        let high_x = span_x + margin;
        let low_y = -margin;
        let high_y = span_y + margin;

        if wrapped.x < low_x {
            wrapped.x += span_x;
            delta.x += span_x;
        } else if wrapped.x > high_x {
            wrapped.x -= span_x;
            delta.x -= span_x;
        }

        if wrapped.y < low_y {
            wrapped.y += span_y;
            delta.y += span_y;
        } else if wrapped.y > high_y {
            wrapped.y -= span_y;
            delta.y -= span_y;
        }

        (wrapped, delta)
    }

    fn rebuild_dimension_fields(&mut self) {
        self.room_dimension_fields.clear();
        if self.level.rooms.is_empty() {
            return;
        }

        let mut rng = thread_rng();
        for room in &self.level.rooms {
            let area = (room.w * room.h).max(1.0);
            let area_norm = (area / 360.0).min(1.0);
            let aspect = room.w / room.h.max(0.001);
            let mut baseline = rng.gen_range(0.94..1.07);
            if rng.gen::<f32>() < 0.22 {
                let swing = rng.gen_range(0.06..0.12);
                baseline += if rng.gen::<f32>() < 0.5 { -swing } else { swing };
            }

            let amplitude = rng.gen_range(0.06..0.18) * (0.72 + area_norm * 0.52);
            let frequency = rng.gen_range(0.55..1.35);
            let center_bias = rng.gen_range(0.3..1.0);
            let edge_bias = rng.gen_range(0.3..1.0);
            let (persp_u, persp_v) = if aspect >= 1.22 {
                (rng.gen_range(0.3..0.78), rng.gen_range(-0.52..-0.12))
            } else if aspect <= 0.82 {
                (rng.gen_range(-0.52..-0.12), rng.gen_range(0.3..0.78))
            } else {
                (rng.gen_range(-0.38..0.38), rng.gen_range(-0.38..0.38))
            };

            self.room_dimension_fields.push(RoomDimensionField {
                base: baseline,
                amp: amplitude,
                freq: frequency,
                phase: rng.gen_range(0.0..std::f32::consts::TAU),
                center_bias,
                edge_bias,
                persp_u,
                persp_v,
                aspect,
            });
        }
    }

    fn rebuild_compression_pockets(&mut self) {
        self.room_compression_pockets.clear();
        if self.level.rooms.is_empty() {
            return;
        }

        let mut rng = thread_rng();
        let arena_mode = matches!(self.level.kind, LevelKind::MainArena);
        let floor_z = self.level.floor_z();
        let wall_h = 2.6;

        for (room_idx, room) in self.level.rooms.iter().enumerate() {
            let pocket_count_choices = if arena_mode { [3, 4, 4] } else { [2, 2, 3] };
            let pocket_count = pocket_count_choices[rng.gen_range(0..pocket_count_choices.len())];
            for _ in 0..pocket_count {
                let cx = rng.gen_range(room.x + room.w * 0.18..room.x + room.w * 0.82);
                let cy = rng.gen_range(room.y + room.h * 0.18..room.y + room.h * 0.82);

                let (cz, radius, compression, dilation, scale_x, scale_y, scale_z) = if arena_mode {
                    let z_low = floor_z + 2.0;
                    let z_high = floor_z + 15.5;
                    let cz = rng.gen_range(z_low..z_high);
                    let radius = rng.gen_range(
                        (6.2_f32).max(room.w.min(room.h) * 0.24)
                            ..(11.5_f32).max(room.w.min(room.h) * 0.52),
                    );
                    let compression = rng.gen_range(0.36..0.82);
                    let dilation = rng.gen_range(0.18..0.58);
                    let scale_x = rng.gen_range(0.74..1.95);
                    let scale_y = rng.gen_range(0.74..1.95);
                    let scale_z = rng.gen_range(0.48..1.22);
                    (cz, radius, compression, dilation, scale_x, scale_y, scale_z)
                } else {
                    let cz = floor_z + rng.gen_range(wall_h * 0.35..wall_h * 0.72);
                    let radius = rng.gen_range(
                        (2.4_f32).max(room.w.min(room.h) * 0.16)
                            ..(4.8_f32).max(room.w.min(room.h) * 0.38),
                    );
                    let compression = rng.gen_range(0.52..0.92);
                    let dilation = rng.gen_range(0.08..0.44);
                    let scale_x = rng.gen_range(0.58..1.7);
                    let scale_y = rng.gen_range(0.58..1.7);
                    let scale_z = rng.gen_range(0.58..1.7);
                    (cz, radius, compression, dilation, scale_x, scale_y, scale_z)
                };

                let mut radius_scale = self.compression_pocket_radius_scale.max(0.5);
                if arena_mode {
                    radius_scale *= 1.18;
                }
                let radius = radius * radius_scale;

                self.room_compression_pockets.push(CompressionPocket {
                    room_idx,
                    center: vec3(cx, cy, cz),
                    radius,
                    compression,
                    dilation,
                    scale: vec3(scale_x, scale_y, scale_z),
                    phase: rng.gen_range(0.0..std::f32::consts::TAU),
                });
            }
        }
    }

    fn compression_factor_at(&self, pos: Vec3, t: f32) -> f32 {
        if self.room_compression_pockets.is_empty() && self.room_dimension_fields.is_empty() {
            return 1.0;
        }

        let mut factor = 1.0;
        if let Some(room_idx) = self.current_room_idx_for_pos(pos) {
            if let Some(field) = self.room_dimension_fields.get(room_idx) {
                let room = &self.level.rooms[room_idx];
                let nx = (pos.x - (room.x + room.w * 0.5)) / (room.w * 0.5).max(0.001);
                let ny = (pos.y - (room.y + room.h * 0.5)) / (room.h * 0.5).max(0.001);
                let edge = nx.abs().max(ny.abs());
                let center = (1.0 - edge).max(0.0);
                let wave = (t * field.freq + field.phase).sin();
                let room_base = field.base + field.amp * wave;
                let room_delta = room_base - 1.0;
                let spatial_blend = center * field.center_bias + edge * field.edge_bias;
                let spatial_blend = spatial_blend.clamp(0.0, 1.35);
                factor *= 1.0 + room_delta * spatial_blend;
            }
        }

        for pocket in &self.room_compression_pockets {
            let pulse = 0.75 + 0.35 * (t * 1.1 + pocket.phase).sin();
            let inv = 1.0 / (pocket.radius * pulse).max(0.001);
            let dx = (pos.x - pocket.center.x) * pocket.scale.x * inv;
            let dy = (pos.y - pocket.center.y) * pocket.scale.y * inv;
            let dz = (pos.z - pocket.center.z) * pocket.scale.z * inv;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist >= 1.0 {
                continue;
            }

            let mut influence = 1.0 - dist;
            let influence_power = self.compression_pocket_influence_power.max(0.25);
            influence = influence.powf(influence_power);
            let mut local_factor = 1.0 - (1.0 - pocket.compression) * influence;
            let dilation_gain = self.compression_pocket_dilation_gain.max(0.0);
            let speed_bias = self.compression_pocket_speed_bias.clamp(0.0, 0.95);
            let sin_norm = 0.5 + 0.5 * (t * 1.35 + pocket.phase * 1.7).sin();
            local_factor += pocket.dilation
                * dilation_gain
                * influence
                * (speed_bias + (1.0 - speed_bias) * sin_norm);
            factor *= local_factor.clamp(0.55, 1.85);
        }

        let speed_norm = self.player.speed_norm();
        let travel_dilation = 1.0 + speed_norm * 0.12;
        factor *= travel_dilation;

        factor.clamp(0.42, 1.9)
    }

    fn current_room_idx_for_pos(&self, pos: Vec3) -> Option<usize> {
        self.level.rooms.iter().enumerate().find_map(|(idx, room)| {
            let in_x = pos.x >= room.x && pos.x <= room.x + room.w;
            let in_y = pos.y >= room.y && pos.y <= room.y + room.h;
            if in_x && in_y {
                Some(idx)
            } else {
                None
            }
        })
    }

    fn draw_compression_slice(&self, floor_z: f32) {
        let x_min = 0.0;
        let x_max = self.level.map_w;
        let y_min = 0.0;
        let y_max = self.level.map_d;
        let z_plane = floor_z + 0.06;
        let steps_x = 24;
        let steps_y = 24;
        let dx = (x_max - x_min) / steps_x as f32;
        let dy = (y_max - y_min) / steps_y as f32;

        let mut samples: Vec<f32> = Vec::with_capacity(steps_x * steps_y * 4);
        let mut min_c = f32::MAX;
        let mut max_c = f32::MIN;
        for yi in 0..steps_y {
            for xi in 0..steps_x {
                let x0 = x_min + xi as f32 * dx;
                let x1 = x0 + dx;
                let y0 = y_min + yi as f32 * dy;
                let y1 = y0 + dy;

                let c00 = self.compression_factor_at(vec3(x0, y0, z_plane), self.time);
                let c10 = self.compression_factor_at(vec3(x1, y0, z_plane), self.time);
                let c11 = self.compression_factor_at(vec3(x1, y1, z_plane), self.time);
                let c01 = self.compression_factor_at(vec3(x0, y1, z_plane), self.time);
                for val in [c00, c10, c11, c01] {
                    min_c = min_c.min(val);
                    max_c = max_c.max(val);
                    samples.push(val);
                }
            }
        }

        let mut vertices: Vec<Vertex> = Vec::with_capacity(steps_x * steps_y * 4);
        let mut indices: Vec<u16> = Vec::with_capacity(steps_x * steps_y * 6);
        let mut sample_idx = 0;
        for yi in 0..steps_y {
            for xi in 0..steps_x {
                let x0 = x_min + xi as f32 * dx;
                let x1 = x0 + dx;
                let y0 = y_min + yi as f32 * dy;
                let y1 = y0 + dy;

                let c00 = compression_to_color(samples[sample_idx], min_c, max_c);
                let c10 = compression_to_color(samples[sample_idx + 1], min_c, max_c);
                let c11 = compression_to_color(samples[sample_idx + 2], min_c, max_c);
                let c01 = compression_to_color(samples[sample_idx + 3], min_c, max_c);
                sample_idx += 4;

                let base = vertices.len() as u16;
                vertices.push(Vertex::new2(vec3(x0, y0, z_plane), vec2(0.0, 0.0), c00));
                vertices.push(Vertex::new2(vec3(x1, y0, z_plane), vec2(1.0, 0.0), c10));
                vertices.push(Vertex::new2(vec3(x1, y1, z_plane), vec2(1.0, 1.0), c11));
                vertices.push(Vertex::new2(vec3(x0, y1, z_plane), vec2(0.0, 1.0), c01));

                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }

        let mesh = Mesh {
            vertices,
            indices,
            texture: None,
        };
        draw_mesh(&mesh);
    }
}

fn compression_to_color(compression: f32, min_c: f32, max_c: f32) -> Color {
    let denom = (max_c - min_c).max(1.0e-4);
    let t = ((compression - min_c) / denom).clamp(0.0, 1.0);
    let (c0, c1, local) = if t < 0.5 {
        (vec3(0.12, 0.38, 0.95), vec3(0.2, 0.9, 0.55), t / 0.5)
    } else {
        (vec3(0.2, 0.9, 0.55), vec3(0.95, 0.2, 0.1), (t - 0.5) / 0.5)
    };
    let col = c0.lerp(c1, local.clamp(0.0, 1.0));
    Color::new(col.x, col.y, col.z, 0.75)
}

async fn load_random_ball_texture(dir: &str) -> Option<Texture2D> {
    let mut files: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
                let ext_l = ext.to_lowercase();
                if !(ext_l == "png" || ext_l == "jpg" || ext_l == "jpeg" || ext_l == "bmp" || ext_l == "tga") {
                    continue;
                }
            } else {
                continue;
            }
            if let Some(path_str) = path.to_str() {
                files.push(path_str.to_string());
            }
        }
    }
    if files.is_empty() {
        return None;
    }
    let idx = macroquad::rand::gen_range(0, files.len());
    let pick = files.swap_remove(idx);
    if Path::new(&pick).exists() {
        if let Ok(tex) = load_texture(&pick).await {
            return Some(tex);
        }
    }
    None
}

#[derive(Clone, Copy, Debug)]
pub struct HudStats {
    pub hp: f32,
    pub hp_max: f32,
    pub xp: f32,
    pub xp_next: f32,
    pub level: i32,
    pub monsters_total: i32,
    pub monsters_slain: i32,
    pub time: f32,
}
