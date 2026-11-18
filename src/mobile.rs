// src/mobile.rs
use macroquad::prelude::*;

use crate::settings::{
    Settings, BTN_COUNT, BTN_FIRE, BTN_JUMP, BTN_LABELS, BTN_RELOAD, BTN_SCOPE, BTN_SLIDE,
};

const JOY_DEADZONE: f32 = 0.15;

/// Extra pixels around every button that still count as a hit.
const HIT_SLOP: f32 = 14.0;

/// Multiplier applied to touch aim delta before it reaches `Player::look`.
/// Bump this if mobile aim feels too slow; lower it if it feels twitchy.
pub const MOBILE_AIM_SCALE: f32 = 1.0;

pub struct TouchFrame {
    pub movement: Vec2,          // -1..1 on each axis (x = strafe, +y = down on screen)
    pub jump_pressed: bool,      // edge
    pub slide_held: bool,        // slide/crouch button currently down
    pub slide_pressed: bool,     // edge (SLIDE button went down)
    pub reload_pressed: bool,    // edge
    pub fire_held: bool,
    pub fire_just_pressed: bool, // edge
    pub scope_held: bool,
    pub scope_pressed: bool,     // edge (used for toggle scope)
    pub menu_pressed: bool,      // MENU button tapped
    pub aim_delta: Vec2,         // pixel delta on the aim side this frame
}

/// What a new finger landed on.
#[derive(Clone, Copy)]
enum Target {
    Menu,
    Button(usize),
    Joy,
    Aim,
}

/// One-frame edges collected while walking this frame's touches.
#[derive(Default)]
struct Pressed {
    menu: bool,
    jump: bool,
    reload: bool,
    scope: bool,
    fire: bool,
    fire_tap: bool,  // fire tapped and released inside one frame
    slide_tap: bool, // slide tapped and released inside one frame
    slide: bool,     // slide button went down this frame
}

pub struct TouchControls {
    move_touch_id: Option<u64>,
    move_origin: Vec2,
    move_current: Vec2,

    aim_touch_id: Option<u64>,
    aim_last: Vec2,

    /// Finger currently holding each button.
    btn_touch: [Option<u64>; BTN_COUNT],

    /// Finger ids we are already tracking (assigned to a control or ignored).
    known: Vec<u64>,
    /// Finger ids that already ended (the OS can keep reporting them).
    dead: Vec<u64>,
}

fn grow(r: Rect, m: f32) -> Rect {
    Rect::new(r.x - m, r.y - m, r.w + 2.0 * m, r.h + 2.0 * m)
}

fn hit_target(st: &Settings, w: f32, h: f32, pos: Vec2) -> Target {
    if grow(menu_button_rect(w, h), HIT_SLOP).contains(pos) {
        return Target::Menu;
    }

    // Buttons get a little slop; if the slop zones of two buttons overlap, the
    // button whose centre is closest wins.
    let mut best: Option<(usize, f32)> = None;
    for i in 0..BTN_COUNT {
        let r = st.button_rect(i, w, h);
        if grow(r, HIT_SLOP).contains(pos) {
            let c = vec2(r.x + r.w * 0.5, r.y + r.h * 0.5);
            let d = (pos - c).length_squared();
            if best.map_or(true, |(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
    }
    if let Some((i, _)) = best {
        return Target::Button(i);
    }

    if st.joy_zone(w, h).contains(pos) {
        return Target::Joy;
    }
    Target::Aim
}

impl TouchControls {
    pub fn new() -> Self {
        Self {
            move_touch_id: None,
            move_origin: Vec2::ZERO,
            move_current: Vec2::ZERO,
            aim_touch_id: None,
            aim_last: Vec2::ZERO,
            btn_touch: [None; BTN_COUNT],
            known: Vec::new(),
            dead: Vec::new(),
        }
    }

    /// A finger just landed (or, with `tap`, landed and lifted within a frame).
    fn begin(
        &mut self,
        id: u64,
        pos: Vec2,
        tap: bool,
        st: &Settings,
        w: f32,
        h: f32,
        pr: &mut Pressed,
    ) {
        match hit_target(st, w, h, pos) {
            Target::Menu => pr.menu = true,
            Target::Button(i) => {
                // a second finger on a button that is already held does nothing
                if !tap && self.btn_touch[i].is_some() {
                    return;
                }
                match i {
                    BTN_FIRE => {
                        pr.fire = true;
                        pr.fire_tap |= tap;
                    }
                    BTN_SCOPE => pr.scope = true,
                    BTN_JUMP => pr.jump = true,
                    BTN_RELOAD => pr.reload = true,
                    BTN_SLIDE => {
                        pr.slide = true;
                        pr.slide_tap |= tap;
                    }
                    _ => {}
                }
                if !tap {
                    self.btn_touch[i] = Some(id);
                }
            }
            Target::Joy => {
                if !tap && self.move_touch_id.is_none() {
                    self.move_touch_id = Some(id);
                    self.move_origin = pos;
                    self.move_current = pos;
                }
            }
            Target::Aim => {
                if !tap && self.aim_touch_id.is_none() {
                    self.aim_touch_id = Some(id);
                    self.aim_last = pos;
                }
            }
        }
    }

    /// A finger lifted: free whatever it was holding.
    fn release(&mut self, id: u64) {
        if self.move_touch_id == Some(id) {
            self.move_touch_id = None;
        }
        if self.aim_touch_id == Some(id) {
            self.aim_touch_id = None;
        }
        for slot in self.btn_touch.iter_mut() {
            if *slot == Some(id) {
                *slot = None;
            }
        }
        self.known.retain(|k| *k != id);
    }

    pub fn update(&mut self, st: &Settings) -> TouchFrame {
        let w = screen_width();
        let h = screen_height();

        let mut pr = Pressed::default();
        let mut aim_delta = Vec2::ZERO;
        let mut seen: Vec<u64> = Vec::new();

        // Touch events arrive faster than frames, so a finger can go
        // Started -> Moved (or even -> Ended) before we ever look at it. So we
        // never rely on seeing the `Started` phase: any finger id we are not
        // tracking yet is a new finger, whatever phase it reports.
        for touch in touches() {
            let id = touch.id;
            let pos = touch.position;
            seen.push(id);

            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                if self.dead.contains(&id) {
                    continue; // stale repeat of an ended touch
                }
                if !self.known.contains(&id) {
                    // tapped and released inside a single frame: still a tap
                    self.begin(id, pos, true, st, w, h, &mut pr);
                }
                self.release(id);
                self.dead.push(id);
                continue;
            }

            // live finger (the id may have been reused by a new touch)
            self.dead.retain(|d| *d != id);
            if !self.known.contains(&id) {
                self.known.push(id);
                self.begin(id, pos, false, st, w, h, &mut pr);
            } else if self.move_touch_id == Some(id) {
                self.move_current = pos;
            } else if self.aim_touch_id == Some(id) {
                aim_delta += pos - self.aim_last;
                self.aim_last = pos;
            }
        }

        // Fingers that vanished without an Ended event.
        let gone: Vec<u64> = self.known.iter().copied().filter(|k| !seen.contains(k)).collect();
        for id in gone {
            self.release(id);
        }
        self.dead.retain(|d| seen.contains(d));

        // Movement vector from joystick.
        let joy_radius = st.joy_radius();
        let mut movement = Vec2::ZERO;
        if self.move_touch_id.is_some() {
            let offset = self.move_current - self.move_origin;
            let len = offset.length();
            if len > JOY_DEADZONE * joy_radius {
                let clamped = offset.clamp_length_max(joy_radius);
                movement = clamped / joy_radius;
            }
        }

        TouchFrame {
            movement,
            jump_pressed: pr.jump,
            slide_held: self.btn_touch[BTN_SLIDE].is_some() || pr.slide_tap,
            slide_pressed: pr.slide,
            reload_pressed: pr.reload,
            fire_held: self.btn_touch[BTN_FIRE].is_some() || pr.fire_tap,
            fire_just_pressed: pr.fire,
            scope_held: self.btn_touch[BTN_SCOPE].is_some(),
            scope_pressed: pr.scope,
            menu_pressed: pr.menu,
            aim_delta,
        }
    }

    /// Forget every held control (used when the menu opens/closes). Fingers
    /// that are still down at that moment are ignored until they lift, so the
    /// finger that tapped MENU/RESUME doesn't also start aiming.
    pub fn reset(&mut self) {
        self.move_touch_id = None;
        self.aim_touch_id = None;
        self.btn_touch = [None; BTN_COUNT];
        self.known.clear();
        self.dead.clear();
        for t in touches() {
            if matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.dead.push(t.id);
            } else {
                self.known.push(t.id);
            }
        }
    }

    pub fn draw(&self, st: &Settings) {
        let w = screen_width();
        let h = screen_height();
        let op = st.hud_opacity;

        let jr = st.joy_radius();
        if self.move_touch_id.is_some() {
            draw_circle_lines(
                self.move_origin.x,
                self.move_origin.y,
                jr,
                3.0,
                Color::new(1.0, 1.0, 1.0, 0.35 * op),
            );
            // keep the thumb dot inside the ring
            let knob = self.move_origin + (self.move_current - self.move_origin).clamp_length_max(jr);
            draw_circle(
                knob.x,
                knob.y,
                26.0 * st.joy_scale,
                Color::new(1.0, 1.0, 1.0, 0.45 * op),
            );
        } else {
            let c = st.joy_center(w, h);
            draw_circle_lines(c.x, c.y, jr, 2.0, Color::new(1.0, 1.0, 1.0, 0.2 * op));
            draw_circle(c.x, c.y, 26.0 * st.joy_scale, Color::new(1.0, 1.0, 1.0, 0.12 * op));
        }

        for i in 0..BTN_COUNT {
            Self::draw_button(
                st.button_rect(i, w, h),
                BTN_LABELS[i],
                self.btn_touch[i].is_some(),
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

/// MENU button under the FPS counter (top-right). Bigger than before so it is
/// easy to hit.
pub fn menu_button_rect(w: f32, _h: f32) -> Rect {
    Rect::new(w - 150.0, 56.0, 120.0, 52.0)
}
