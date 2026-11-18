// src/mobile.rs
use macroquad::prelude::*;

use crate::settings::{
    Settings, BTN_COUNT, BTN_FIRE, BTN_JUMP, BTN_LABELS, BTN_RELOAD, BTN_SCOPE, BTN_SLIDE,
};

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
    pub scope_pressed: bool,     // edge (used for toggle scope)
    pub menu_pressed: bool,      // MENU button tapped
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

    pub fn update(&mut self, st: &Settings) -> TouchFrame {
        let w = screen_width();
        let h = screen_height();
        let left_half = w * 0.45;

        let fire_rect = st.button_rect(BTN_FIRE, w, h);
        let scope_rect = st.button_rect(BTN_SCOPE, w, h);
        let jump_rect = st.button_rect(BTN_JUMP, w, h);
        let slide_rect = st.button_rect(BTN_SLIDE, w, h);
        let reload_rect = st.button_rect(BTN_RELOAD, w, h);
        let menu_rect = menu_button_rect(w, h);
        let mut menu_pressed = false;

        let mut jump_pressed = false;
        let mut reload_pressed = false;
        let mut fire_just_pressed = false;
        let mut scope_pressed = false;
        let mut aim_delta = Vec2::ZERO;
        let mut active_ids: Vec<u64> = Vec::new();

        for touch in touches() {
            let id = touch.id;
            let pos = touch.position;
            active_ids.push(id);

            match touch.phase {
                TouchPhase::Started => {
                    if menu_rect.contains(pos) {
                        menu_pressed = true;
                    } else if pos.x < left_half && self.move_touch_id.is_none() {
                        self.move_touch_id = Some(id);
                        self.move_origin = pos;
                        self.move_current = pos;
                    } else if fire_rect.contains(pos) && self.fire_touch_id.is_none() {
                        self.fire_touch_id = Some(id);
                        fire_just_pressed = true;
                    } else if scope_rect.contains(pos) && self.scope_touch_id.is_none() {
                        self.scope_touch_id = Some(id);
                        scope_pressed = true;
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
            scope_pressed,
            menu_pressed,
            aim_delta,
        }
    }

    /// Forget every held control (used when the menu opens/closes).
    pub fn reset(&mut self) {
        self.move_touch_id = None;
        self.aim_touch_id = None;
        self.fire_touch_id = None;
        self.scope_touch_id = None;
        self.jump_touch_id = None;
        self.slide_touch_id = None;
        self.reload_touch_id = None;
    }

    pub fn draw(&self, st: &Settings) {
        let w = screen_width();
        let h = screen_height();
        let op = st.hud_opacity;

        if self.move_touch_id.is_some() {
            draw_circle_lines(
                self.move_origin.x,
                self.move_origin.y,
                JOY_RADIUS,
                3.0,
                Color::new(1.0, 1.0, 1.0, 0.35 * op),
            );
            draw_circle(
                self.move_current.x,
                self.move_current.y,
                26.0,
                Color::new(1.0, 1.0, 1.0, 0.45 * op),
            );
        } else {
            draw_circle_lines(
                w * 0.12,
                h - 150.0,
                JOY_RADIUS,
                2.0,
                Color::new(1.0, 1.0, 1.0, 0.15 * op),
            );
        }

        let active = [
            self.fire_touch_id.is_some(),
            self.scope_touch_id.is_some(),
            self.jump_touch_id.is_some(),
            self.slide_touch_id.is_some(),
            self.reload_touch_id.is_some(),
        ];
        for i in 0..BTN_COUNT {
            Self::draw_button(
                st.button_rect(i, w, h),
                BTN_LABELS[i],
                active[i],
                crate::settings::btn_color(i),
                op,
            );
        }

        Self::draw_button(
            menu_button_rect(w, h),
            "MENU",
            false,
            Color::new(0.5, 0.5, 0.6, 1.0),
            op.max(0.5),
        );
    }

    pub fn draw_button(r: Rect, label: &str, active: bool, col: Color, opacity: f32) {
        let a = (if active { 0.85 } else { 0.45 }) * opacity;
        draw_rectangle(r.x, r.y, r.w, r.h, Color::new(col.r, col.g, col.b, a));
        draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(1.0, 1.0, 1.0, 0.7 * opacity));
        let fs = (r.w * 0.225).clamp(12.0, 30.0);
        let tw = measure_text(label, None, fs as u16, 1.0).width;
        draw_text(
            label,
            r.x + (r.w - tw) * 0.5,
            r.y + r.h * 0.6,
            fs,
            Color::new(1.0, 1.0, 1.0, opacity.max(0.4)),
        );
    }
}

/// Small MENU button under the FPS counter (top-right).
pub fn menu_button_rect(w: f32, _h: f32) -> Rect {
    Rect::new(w - 130.0, 60.0, 100.0, 44.0)
}

fn release_if_gone(slot: &mut Option<u64>, active: &[u64]) {
    if let Some(id) = *slot {
        if !active.contains(&id) {
            *slot = None;
        }
    }
}
