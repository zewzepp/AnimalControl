// src/bots.rs
use macroquad::prelude::*;

use crate::map::{los_clear, move_circle, ray_aabb, spread_dir, Aabb, Map};

/// Bot aim error (radians of random cone). Raise these to make bots miss more.
const BOT_SPREAD_BASE: f32 = 0.08;
const BOT_SPREAD_PER_METER: f32 = 0.006;

pub struct Shot {
    pub from: Vec3,
    pub to: Vec3,
    pub hit_player: bool,
    pub damage: f32,
}

pub struct Bot {
    pub pos: Vec3,
    pub vel: Vec3,
    pub health: f32,
    pub alive: bool,
    pub respawn_t: f32,
    pub fire_t: f32,
    pub hit_flash: f32,
    pub strafe: f32,
    pub strafe_t: f32,
}

impl Bot {
    pub fn new(pos: Vec3) -> Self {
        Self {
            pos,
            vel: Vec3::ZERO,
            health: 100.0,
            alive: true,
            respawn_t: 0.0,
            fire_t: macroquad::rand::gen_range(0.5f32, 2.5),
            hit_flash: 0.0,
            strafe: 1.0,
            strafe_t: 0.0,
        }
    }

    pub fn bounds(&self) -> Aabb {
        Aabb {
            // generous hit volume: wider than the body and tall enough to
            // include the head cube (top at ~2.2), so close shots register
            min: self.pos + vec3(-0.6, 0.0, -0.6),
            max: self.pos + vec3(0.6, 2.3, 0.6),
        }
    }

    pub fn draw(&self) {
        if !self.alive {
            return;
        }

        let body = if self.hit_flash > 0.0 {
            Color::new(1.0, 1.0, 1.0, 1.0)
        } else {
            Color::new(0.88, 0.24, 0.24, 1.0)
        };

        draw_cube(
            self.pos + vec3(0.0, 0.9, 0.0),
            vec3(0.8, 1.8, 0.8),
            None,
            body,
        );
        draw_cube(
            self.pos + vec3(0.0, 1.95, 0.0),
            vec3(0.5, 0.5, 0.5),
            None,
            Color::new(0.18, 0.18, 0.22, 1.0),
        );
        draw_cube_wires(
            self.pos + vec3(0.0, 0.9, 0.0),
            vec3(0.82, 1.82, 0.82),
            Color::new(0.0, 0.0, 0.0, 0.6),
        );
    }

    pub fn update(
        &mut self,
        dt: f32,
        player_pos: Vec3,
        player_alive: bool,
        player_bounds: &Aabb,
        map: &Map,
    ) -> Option<Shot> {
        self.hit_flash = (self.hit_flash - dt).max(0.0);

        // ---- respawn
        if !self.alive {
            self.respawn_t -= dt;
            if self.respawn_t <= 0.0 {
                let i = macroquad::rand::gen_range(0usize, map.spawns.len());
                self.pos = map.spawns[i];
                self.vel = Vec3::ZERO;
                self.health = 100.0;
                self.alive = true;
            }
            return None;
        }

        if !player_alive {
            self.vel.y -= 24.0 * dt;
            move_circle(&mut self.pos, &mut self.vel, dt, map, 0.4, 1.75);
            return None;
        }

        // ---- perception
        let eye = self.pos + vec3(0.0, 1.55, 0.0);
        let target = player_pos + vec3(0.0, 1.1, 0.0);
        let to = target - eye;
        let dist = to.length().max(0.001);
        let dir = to / dist;

        let los = los_clear(eye, target, map);

        // ---- strafe pattern
        self.strafe_t -= dt;
        if self.strafe_t <= 0.0 {
            self.strafe_t = macroquad::rand::gen_range(0.5f32, 1.6);
            self.strafe = macroquad::rand::gen_range(-1.0f32, 1.0);
        }

        // ---- steering
        let fwd = vec2(-dir.x, -dir.z).normalize_or_zero();
        let right = vec2(fwd.y, -fwd.x);

        let mut wish = Vec2::ZERO;
        if los {
            if dist > 11.0 {
                wish += fwd;
            } else if dist < 6.0 {
                wish -= fwd;
            }
            wish += right * self.strafe * 0.9;
        } else {
            wish += fwd * 1.2;
        }
        let wish = wish.normalize_or_zero();

        let target_v = wish * 4.2;
        let dv = target_v - vec2(self.vel.x, self.vel.z);
        let max_dv = 30.0 * dt;
        let l = dv.length();
        if l > 1e-5 {
            let f = (max_dv / l).min(1.0);
            self.vel.x += dv.x * f;
            self.vel.z += dv.y * f;
        }

        self.vel.y -= 24.0 * dt;
        move_circle(&mut self.pos, &mut self.vel, dt, map, 0.4, 1.75);

        // ---- shooting
        self.fire_t -= dt;
        if los && dist < 40.0 && self.fire_t <= 0.0 {
            self.fire_t = macroquad::rand::gen_range(0.35f32, 0.9);

            let spread = BOT_SPREAD_BASE + dist * BOT_SPREAD_PER_METER;
            let d = spread_dir(dir, spread);

            let mut best_t = f32::INFINITY;
            let mut hit_player = false;

            for b in &map.boxes {
                if let Some(t) = ray_aabb(eye, d, b) {
                    if t < best_t {
                        best_t = t;
                        hit_player = false;
                    }
                }
            }
            if let Some(t) = ray_aabb(eye, d, player_bounds) {
                if t < best_t {
                    best_t = t;
                    hit_player = true;
                }
            }

            let end = if best_t.is_finite() {
                eye + d * best_t
            } else {
                eye + d * 60.0
            };

            return Some(Shot {
                from: eye,
                to: end,
                hit_player,
                damage: macroquad::rand::gen_range(7.0f32, 14.0),
            });
        }

        None
    }
}
