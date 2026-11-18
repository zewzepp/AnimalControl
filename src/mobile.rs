// src/mobile.rs
use macroquad::prelude::*;

const JOY_RADIUS: f32 = 70.0;
const JOY_DEADZONE: f32 = 0.15;

/// Multiplier applied to touch aim delta before it reaches `Player::look`.
/// Bump this if mobile aim feels too slow; lower it if it feels twitchy.
pub const MOBILE_AIM_SCALE: f32 = 1.0;

pub struct TouchFrame {
    pub movement: Vec2,          // -1..1 on each axis (x = strafe, +y = down on screen)
    pub jump_pressed: bool,      // edge
    pub slide_held: bool,        // slide/crouch button currently down
    pub reload_pressed: bool,    // edge
    pub fire_held: bool,
    pub fire_just_pressed: bool, // edge
    pub scope_held: bool,
    pub aim_delta: Vec2,         // pixel delta on the aim side this frame
}

pub struct TouchControls {
    move_touch_id: Option<u64>,
    move_origin: Vec2,
    move_current: Vec2,

    aim_touch_id: Option<u64>,
    aim_last: Vec2,

    fire_touch_id: Option<u64>,
    scope_touch_id: Option<u64>,
    jump_touch_id: Option<u64>,
    slide_touch_id: Option<u64>,
    reload_touch_id: Option<u64>,
}

impl TouchControls {
    pub fn new() -> Self {
        Self {
            move_touch_id: None,
            move_origin: Vec2::ZERO,
            move_current: Vec2::ZERO,
            aim_touch_id: None,
            aim_last: Vec2::ZERO,
            fire_touch_id: None,
            scope_touch_id: None,
            jump_touch_id: None,
            slide_touch_id: None,
            reload_touch_id: None,
        }
    }

    pub fn update(&mut self) -> TouchFrame {
        let w = screen_width();
        let h = screen_height();
        let left_half = w * 0.45;

        let fire_rect = Self::fire_rect(w, h);
        let scope_rect = Self::scope_rect(w, h);
        let jump_rect = Self::jump_rect(w, h);
        let slide_rect = Self::slide_rect(w, h);
        let reload_rect = Self::reload_rect(w, h);

        let mut jump_pressed = false;
        let mut reload_pressed = false;
        let mut fire_just_pressed = false;
        let mut aim_delta = Vec2::ZERO;
        let mut active_ids: Vec<u64> = Vec::new();

        for touch in touches() {
            let id = touch.id;
            let pos = touch.position;
            active_ids.push(id);

            match touch.phase {
                TouchPhase::Started => {
                    if pos.x < left_half && self.move_touch_id.is_none() {
                        self.move_touch_id = Some(id);
                        self.move_origin = pos;
                        self.move_current = pos;
                    } else if fire_rect.contains(pos) && self.fire_touch_id.is_none() {
                        self.fire_touch_id = Some(id);
                        fire_just_pressed = true;
                    } else if scope_rect.contains(pos) && self.scope_touch_id.is_none() {
                        self.scope_touch_id = Some(id);
                    } else if jump_rect.contains(pos) && self.jump_touch_id.is_none() {
                        self.jump_touch_id = Some(id);
                        jump_pressed = true;
                    } else if slide_rect.contains(pos) && self.slide_touch_id.is_none() {
                        self.slide_touch_id = Some(id);
                    } else if reload_rect.contains(pos) && self.reload_touch_id.is_none() {
                        self.reload_touch_id = Some(id);
                        reload_pressed = true;
                    } else if pos.x > left_half && self.aim_touch_id.is_none() {
                        self.aim_touch_id = Some(id);
                        self.aim_last = pos;
                    }
                }
                TouchPhase::Moved | TouchPhase::Stationary => {
                    if Some(id) == self.move_touch_id {
                        self.move_current = pos;
                    } else if Some(id) == self.aim_touch_id {
                        aim_delta += pos - self.aim_last;
                        self.aim_last = pos;
                    }
                }
                TouchPhase::Ended | TouchPhase::Cancelled => {
                    // Release immediately, and don't count this touch as active
                    // (macroquad can keep reporting ended touches for a while).
                    for slot in [
                        &mut self.move_touch_id,
                        &mut self.aim_touch_id,
                        &mut self.fire_touch_id,
                        &mut self.scope_touch_id,
                        &mut self.jump_touch_id,
                        &mut self.slide_touch_id,
                        &mut self.reload_touch_id,
                    ] {
                        if *slot == Some(id) {
                            *slot = None;
                        }
                    }
                    active_ids.pop();
                }
            }
        }

        // Release controls whose touch ID is no longer active.
        release_if_gone(&mut self.move_touch_id, &active_ids);
        release_if_gone(&mut self.aim_touch_id, &active_ids);
        release_if_gone(&mut self.fire_touch_id, &active_ids);
        release_if_gone(&mut self.scope_touch_id, &active_ids);
        release_if_gone(&mut self.jump_touch_id, &active_ids);
        release_if_gone(&mut self.slide_touch_id, &active_ids);
        release_if_gone(&mut self.reload_touch_id, &active_ids);

        // Movement vector from joystick.
        let mut movement = Vec2::ZERO;
        if self.move_touch_id.is_some() {
            let offset = self.move_current - self.move_origin;
            let len = offset.length();
            if len > JOY_DEADZONE * JOY_RADIUS {
                let clamped = offset.clamp_length_max(JOY_RADIUS);
                movement = clamped / JOY_RADIUS;
            }
        }

        TouchFrame {
            movement,
            jump_pressed,
            slide_held: self.slide_touch_id.is_some(),
            reload_pressed,
            fire_held: self.fire_touch_id.is_some(),
            fire_just_pressed,
            scope_held: self.scope_touch_id.is_some(),
            aim_delta,
        }
    }

    fn fire_rect(w: f32, h: f32) -> Rect {
        Rect::new(w - 160.0, h - 200.0, 110.0, 110.0)
    }
    fn scope_rect(w: f32, h: f32) -> Rect {
        Rect::new(w - 290.0, h - 150.0, 90.0, 90.0)
    }
    fn jump_rect(w: f32, h: f32) -> Rect {
        Rect::new(w - 110.0, h - 340.0, 80.0, 80.0)
    }
    fn slide_rect(w: f32, h: f32) -> Rect {
        Rect::new(w - 220.0, h - 320.0, 80.0, 80.0)
    }
    fn reload_rect(w: f32, h: f32) -> Rect {
        Rect::new(w - 100.0, h - 450.0, 80.0, 80.0)
    }

    pub fn draw(&self) {
        let w = screen_width();
        let h = screen_height();

        if self.move_touch_id.is_some() {
            draw_circle_lines(
                self.move_origin.x,
                self.move_origin.y,
                JOY_RADIUS,
                3.0,
                Color::new(1.0, 1.0, 1.0, 0.35),
            );
            draw_circle(
                self.move_current.x,
                self.move_current.y,
                26.0,
                Color::new(1.0, 1.0, 1.0, 0.45),
            );
        } else {
            draw_circle_lines(
                w * 0.12,
                h - 150.0,
                JOY_RADIUS,
                2.0,
                Color::new(1.0, 1.0, 1.0, 0.15),
            );
        }

        Self::draw_button(Self::fire_rect(w, h), "FIRE", self.fire_touch_id.is_some(), Color::new(0.9, 0.2, 0.2, 1.0));
        Self::draw_button(Self::scope_rect(w, h), "SCOPE", self.scope_touch_id.is_some(), Color::new(0.2, 0.8, 1.0, 1.0));
        Self::draw_button(Self::jump_rect(w, h), "JUMP", self.jump_touch_id.is_some(), Color::new(0.2, 0.9, 0.3, 1.0));
        Self::draw_button(Self::slide_rect(w, h), "SLIDE", self.slide_touch_id.is_some(), Color::new(0.9, 0.8, 0.2, 1.0));
        Self::draw_button(Self::reload_rect(w, h), "RELOAD", self.reload_touch_id.is_some(), Color::new(0.9, 0.6, 0.2, 1.0));
    }

    fn draw_button(r: Rect, label: &str, active: bool, col: Color) {
        let a = if active { 0.85 } else { 0.45 };
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(col.r, col.g, col.b, a));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(1.0, 1.0, 1.0, 0.7));
        let tw = measure_text(label, None, 18, 1.0).width;
        draw_text(label, r.x + (r.w - tw) * 0.5, r.y + r.h * 0.6, 18.0, WHITE);
    }
}

fn release_if_gone(slot: &mut Option<u64>, active: &[u64]) {
    if let Some(id) = *slot {
        if !active.contains(&id) {
            *slot = None;
        }
    }
}
