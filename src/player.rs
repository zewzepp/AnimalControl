// src/player.rs
use macroquad::prelude::*;

use crate::map::{move_circle, Aabb, Map};

pub const P_RADIUS: f32 = 0.35;
pub const STAND_H: f32 = 1.75;
pub const CROUCH_H: f32 = 1.05;
const GRAVITY: f32 = 24.0;

pub struct Player {
    pub pos: Vec3, // feet position
    pub vel: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub crouching: bool,
    pub sliding: bool,
    pub slide_t: f32,
    pub scoped: bool,
    pub health: f32,
    pub alive: bool,
    pub respawn_t: f32,
    pub eye: f32,
    pub bob: f32,
    pub bob_y: f32,
    pub land: f32,
    pub recoil_pitch: f32,
    pub recoil_yaw: f32,
    pub speed_now: f32,

    // --- mobile touch state (set each frame from main)
    pub mobile_move: Vec2,
    pub mobile_aim: Vec2,
    pub mobile_fire: bool,
    pub mobile_scope: bool,
    pub mobile_jump: bool,
    pub mobile_slide_active: bool,
}

impl Player {
    pub fn new(spawn: Vec3) -> Self {
        Self {
            pos: spawn,
            vel: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            crouching: false,
            sliding: false,
            slide_t: 0.0,
            scoped: false,
            health: 100.0,
            alive: true,
            respawn_t: 0.0,
            eye: STAND_H - 0.12,
            bob: 0.0,
            bob_y: 0.0,
            land: 0.0,
            recoil_pitch: 0.0,
            recoil_yaw: 0.0,
            speed_now: 0.0,
            mobile_move: Vec2::ZERO,
            mobile_aim: Vec2::ZERO,
            mobile_fire: false,
            mobile_scope: false,
            mobile_jump: false,
            mobile_slide_active: false,
        }
    }

    pub fn height(&self) -> f32 {
        if self.crouching || self.sliding {
            CROUCH_H
        } else {
            STAND_H
        }
    }

    pub fn bounds(&self) -> Aabb {
        let h = self.height();
        Aabb {
            min: self.pos + vec3(-P_RADIUS, 0.0, -P_RADIUS),
            max: self.pos + vec3(P_RADIUS, h, P_RADIUS),
        }
    }

    pub fn look(&mut self, dx: f32, dy: f32) {
        const SENS: f32 = 2.2;
        let s = if self.scoped { SENS * 0.45 } else { SENS };
        self.yaw -= dx * s;
        self.pitch = (self.pitch - dy * s).clamp(-1.5, 1.5);
    }

    pub fn eye_pos(&self) -> Vec3 {
        self.pos + vec3(0.0, self.eye + self.bob_y - self.land, 0.0)
    }

    pub fn aim_dir(&self) -> Vec3 {
        let pitch = (self.pitch + self.recoil_pitch).clamp(-1.55, 1.55);
        let yaw = self.yaw + self.recoil_yaw;
        vec3(
            -yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        )
    }

    pub fn camera(&self, fovy: f32) -> Camera3D {
        let eye = self.eye_pos();
        let dir = self.aim_dir();
        Camera3D {
            position: eye,
            target: eye + dir,
            up: vec3(0.0, 1.0, 0.0),
            fovy,
            projection: Projection::Perspective,
            ..Default::default()
        }
    }

    pub fn die(&mut self) {
        self.alive = false;
        self.health = 0.0;
        self.respawn_t = 2.5;
        self.sliding = false;
        self.crouching = false;
        self.scoped = false;
    }

    fn respawn(&mut self, map: &Map) {
        let i = macroquad::rand::gen_range(0usize, map.spawns.len());
        self.pos = map.spawns[i];
        self.vel = Vec3::ZERO;
        self.health = 100.0;
        self.alive = true;
        self.sliding = false;
        self.crouching = false;
        self.pitch = 0.0;
        self.eye = STAND_H - 0.12;
        self.recoil_pitch = 0.0;
        self.recoil_yaw = 0.0;
    }

    pub fn update(&mut self, dt: f32, map: &Map) {
        // recoil recovery
        let k = (1.0 - 9.0 * dt).max(0.0);
        self.recoil_pitch *= k;
        self.recoil_yaw *= k;
        self.land = (self.land - dt * 0.9).max(0.0);

        if !self.alive {
            self.respawn_t -= dt;
            if self.respawn_t <= 0.0 {
                self.respawn(map);
            }
            return;
        }

        // ---------------- input (keyboard or touch)
        let mut wish = Vec2::ZERO;
        if is_key_down(KeyCode::W) {
            wish.y -= 1.0;
        }
        if is_key_down(KeyCode::S) {
            wish.y += 1.0;
        }
        if is_key_down(KeyCode::A) {
            wish.x -= 1.0;
        }
        if is_key_down(KeyCode::D) {
            wish.x += 1.0;
        }

        // mobile joystick override: same convention as the keyboard above
        // (wish.y < 0 = forward), and screen -y is "up" on the stick
        if self.mobile_move.length_squared() > 0.01 {
            wish = vec2(self.mobile_move.x, self.mobile_move.y);
        }
        let wish = wish.normalize_or_zero();

        let fwd = vec2(-self.yaw.sin(), -self.yaw.cos());
        let right = vec2(self.yaw.cos(), -self.yaw.sin());
        let world_wish = (fwd * (-wish.y) + right * wish.x).normalize_or_zero();

        // On mobile, pushing the stick hard in any direction counts as sprint.
        let sprint =
            is_key_down(KeyCode::LeftShift) || self.mobile_move.length_squared() > 0.5;
        let crouch_held = is_key_down(KeyCode::LeftControl)
            || is_key_down(KeyCode::C)
            || self.mobile_slide_active;

        // ---------------- slide start
        if sprint
            && crouch_held
            && !self.sliding
            && !self.crouching
            && self.on_ground
        {
            let hs = vec2(self.vel.x, self.vel.z).length();
            if hs > 7.0 {
                self.sliding = true;
                self.slide_t = 0.85;
                let dir = vec2(self.vel.x, self.vel.z).normalize_or_zero();
                let boost = (hs * 1.25).clamp(9.5, 14.0);
                self.vel.x = dir.x * boost;
                self.vel.z = dir.y * boost;
            }
        }

        self.crouching = crouch_held && !self.sliding;

        // ---------------- slide update
        if self.sliding {
            self.slide_t -= dt;
            let mut hs = vec2(self.vel.x, self.vel.z).length();
            hs -= 3.5 * dt;
            if self.slide_t <= 0.0 || hs < 4.5 || !self.on_ground {
                self.sliding = false;
            } else {
                let dir = vec2(self.vel.x, self.vel.z).normalize_or_zero();
                self.vel.x = dir.x * hs;
                self.vel.z = dir.y * hs;
            }
        }

        // ---------------- horizontal acceleration (omnimovement)
        if !self.sliding {
            let mut speed: f32 = 6.2;
            if sprint {
                speed = 8.4;
            }
            if self.crouching {
                speed = 3.0;
            }
            if self.scoped {
                speed = speed.min(3.6);
            }

            let target = world_wish * speed;
            let dvx = target.x - self.vel.x;
            let dvz = target.y - self.vel.z;
            let max_dv = if self.on_ground { 55.0 * dt } else { 20.0 * dt };
            let len = (dvx * dvx + dvz * dvz).sqrt();
            if len > 1e-5 {
                let f = (max_dv / len).min(1.0);
                self.vel.x += dvx * f;
                self.vel.z += dvz * f;
            }
        }

        // ---------------- jump (keyboard OR mobile button)
        if (is_key_pressed(KeyCode::Space) || self.mobile_jump) && self.on_ground {
            self.vel.y = 8.0;
            self.on_ground = false;
            self.sliding = false;
            self.land = 0.0;
        }

        // ---------------- gravity + integrate
        self.vel.y -= GRAVITY * dt;
        if self.vel.y < -45.0 {
            self.vel.y = -45.0;
        }

        let impact = -self.vel.y;
        let h = self.height();
        let was_ground = self.on_ground;

        let grounded = move_circle(&mut self.pos, &mut self.vel, dt, map, P_RADIUS, h);
        self.on_ground = grounded;

        if grounded && !was_ground {
            self.land = (impact * 0.012).clamp(0.03, 0.16);
        }

        // ---------------- eye height smoothing
        let target_eye = self.height() - 0.12;
        self.eye += (target_eye - self.eye) * (1.0 - (-14.0 * dt).exp());

        // ---------------- head bob
        let hs = vec2(self.vel.x, self.vel.z).length();
        self.speed_now = hs;

        if self.on_ground && hs > 0.5 && !self.sliding {
            self.bob += dt * hs * 1.6;
        } else {
            self.bob += dt * 2.0;
        }

        let bob_amt = if self.on_ground && !self.sliding {
            (hs / 8.0).min(1.0) * 0.035
        } else {
            0.0
        };
        self.bob_y = (self.bob * 2.0).sin() * bob_amt;
    }
}
