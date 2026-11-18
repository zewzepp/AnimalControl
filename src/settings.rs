// src/settings.rs
// Player settings (sensitivity, volume, HUD) plus the in-game settings menu.

use macroquad::prelude::*;

pub const BTN_FIRE: usize = 0;
pub const BTN_SCOPE: usize = 1;
pub const BTN_JUMP: usize = 2;
pub const BTN_SLIDE: usize = 3;
pub const BTN_RELOAD: usize = 4;
pub const BTN_COUNT: usize = 5;

pub const BTN_LABELS: [&str; BTN_COUNT] = ["FIRE", "SCOPE", "JUMP", "SLIDE", "RELOAD"];

/// Base button size in pixels (multiplied by the HUD size setting).
const BTN_SIZE: [f32; BTN_COUNT] = [110.0, 90.0, 80.0, 80.0, 80.0];

/// Default button centre, as (distance from right edge, distance from bottom edge).
const BTN_DEFAULT: [(f32, f32); BTN_COUNT] = [
    (105.0, 145.0),
    (245.0, 105.0),
    (70.0, 300.0),
    (180.0, 280.0),
    (60.0, 410.0),
];

pub fn btn_color(i: usize) -> Color {
    match i {
        BTN_FIRE => Color::new(0.9, 0.2, 0.2, 1.0),
        BTN_SCOPE => Color::new(0.2, 0.8, 1.0, 1.0),
        BTN_JUMP => Color::new(0.2, 0.9, 0.3, 1.0),
        BTN_SLIDE => Color::new(0.9, 0.8, 0.2, 1.0),
        _ => Color::new(0.9, 0.6, 0.2, 1.0),
    }
}

#[derive(Clone)]
pub struct Settings {
    /// Look sensitivity multiplier (1.0 = default).
    pub sensitivity: f32,
    /// Master volume 0..1.
    pub volume: f32,
    /// Touch button size multiplier.
    pub hud_scale: f32,
    /// Touch button opacity 0..1.
    pub hud_opacity: f32,
    /// Custom button centres as a fraction of the screen (None = default spot).
    pub btn_frac: [Option<Vec2>; BTN_COUNT],
}

impl Settings {
    pub fn new() -> Self {
        Self {
            sensitivity: 1.0,
            volume: 1.0,
            hud_scale: 1.0,
            hud_opacity: 1.0,
            btn_frac: [None; BTN_COUNT],
        }
    }

    pub fn button_rect(&self, i: usize, w: f32, h: f32) -> Rect {
        let size = BTN_SIZE[i] * self.hud_scale;
        let c = match self.btn_frac[i] {
            Some(f) => vec2(f.x * w, f.y * h),
            None => vec2(w - BTN_DEFAULT[i].0, h - BTN_DEFAULT[i].1),
        };
        Rect::new(c.x - size * 0.5, c.y - size * 0.5, size, size)
    }
}

// ------------------------------------------------------------------ pointer

pub struct Pointer {
    pub pos: Option<Vec2>,
    /// true only on the frame the finger/mouse went down
    pub pressed: bool,
}

/// First active touch, or the left mouse button (desktop / browser testing).
pub fn pointer() -> Pointer {
    for t in touches() {
        if matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
            continue;
        }
        return Pointer {
            pos: Some(t.position),
            pressed: matches!(t.phase, TouchPhase::Started),
        };
    }
    if is_mouse_button_down(MouseButton::Left) {
        let (x, y) = mouse_position();
        return Pointer {
            pos: Some(vec2(x, y)),
            pressed: is_mouse_button_pressed(MouseButton::Left),
        };
    }
    Pointer { pos: None, pressed: false }
}

// ------------------------------------------------------------------- menu

pub enum MenuAction {
    None,
    Resume,
}

pub struct Menu {
    pub editing: bool,
    slider: Option<usize>,
    drag: Option<(usize, Vec2)>,
}

const SLIDER_LABELS: [&str; 4] = ["SENSITIVITY", "VOLUME", "HUD SIZE", "HUD OPACITY"];
const SLIDER_RANGES: [(f32, f32); 4] = [(0.3, 3.0), (0.0, 1.0), (0.6, 1.6), (0.2, 1.0)];

fn get_val(s: &Settings, i: usize) -> f32 {
    match i {
        0 => s.sensitivity,
        1 => s.volume,
        2 => s.hud_scale,
        _ => s.hud_opacity,
    }
}

fn set_val(s: &mut Settings, i: usize, v: f32) {
    match i {
        0 => s.sensitivity = v,
        1 => s.volume = v,
        2 => s.hud_scale = v,
        _ => s.hud_opacity = v,
    }
}

fn centered_text(text: &str, r: Rect, size: f32, col: Color) {
    let tw = measure_text(text, None, size as u16, 1.0).width;
    draw_text(text, r.x + (r.w - tw) * 0.5, r.y + r.h * 0.5 + size * 0.3, size, col);
}

fn ui_button(r: Rect, label: &str, size: f32) {
    draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.2, 0.25, 0.35, 0.95));
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(1.0, 1.0, 1.0, 0.6));
    centered_text(label, r, size, WHITE);
}

impl Menu {
    pub fn new() -> Self {
        Self { editing: false, slider: None, drag: None }
    }

    /// Draws the menu and handles its input. Call once per frame while paused.
    pub fn run(&mut self, s: &mut Settings) -> MenuAction {
        let w = screen_width();
        let h = screen_height();
        let u = (h / 720.0).min(w / 1000.0).clamp(0.5, 1.5);
        let p = pointer();

        if p.pos.is_none() {
            self.slider = None;
            self.drag = None;
        }

        // ---------------------------------------------------- layout editor
        if self.editing {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));

            for i in 0..BTN_COUNT {
                let r = s.button_rect(i, w, h);
                crate::mobile::TouchControls::draw_button(
                    r,
                    BTN_LABELS[i],
                    self.drag.map(|d| d.0) == Some(i),
                    btn_color(i),
                    1.0,
                );
            }

            let title = "DRAG THE BUTTONS WHERE YOU WANT THEM";
            let tw = measure_text(title, None, (26.0 * u) as u16, 1.0).width;
            draw_text(title, (w - tw) * 0.5, 40.0 * u, 26.0 * u, WHITE);

            let done = Rect::new(w * 0.5 - 215.0 * u, 60.0 * u, 200.0 * u, 56.0 * u);
            let reset = Rect::new(w * 0.5 + 15.0 * u, 60.0 * u, 200.0 * u, 56.0 * u);
            ui_button(done, "DONE", 26.0 * u);
            ui_button(reset, "RESET", 26.0 * u);

            if let Some(pos) = p.pos {
                if p.pressed {
                    if done.contains(pos) {
                        self.editing = false;
                    } else if reset.contains(pos) {
                        s.btn_frac = [None; BTN_COUNT];
                    } else {
                        for i in 0..BTN_COUNT {
                            let r = s.button_rect(i, w, h);
                            if r.contains(pos) {
                                let centre = vec2(r.x + r.w * 0.5, r.y + r.h * 0.5);
                                self.drag = Some((i, centre - pos));
                            }
                        }
                    }
                }
                if let Some((i, off)) = self.drag {
                    let c = pos + off;
                    s.btn_frac[i] = Some(vec2(
                        (c.x / w).clamp(0.04, 0.96),
                        (c.y / h).clamp(0.06, 0.94),
                    ));
                }
            }
            return MenuAction::None;
        }

        // ------------------------------------------------------ main panel
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));

        let pw = 860.0 * u;
        let ph = 520.0 * u;
        let px = (w - pw) * 0.5;
        let py = (h - ph) * 0.5;
        draw_rectangle(px, py, pw, ph, Color::new(0.08, 0.09, 0.12, 0.97));
        draw_rectangle_lines(px, py, pw, ph, 2.0, Color::new(1.0, 1.0, 1.0, 0.4));
        draw_text("SETTINGS", px + 30.0 * u, py + 56.0 * u, 44.0 * u, WHITE);

        let tx = px + 330.0 * u;
        let tw = pw - 330.0 * u - 150.0 * u;

        for i in 0..4 {
            let y = py + 120.0 * u + i as f32 * 72.0 * u;
            draw_text(SLIDER_LABELS[i], px + 40.0 * u, y + 9.0 * u, 26.0 * u, WHITE);

            let (lo, hi) = SLIDER_RANGES[i];
            let hit = Rect::new(tx - 20.0 * u, y - 35.0 * u, tw + 40.0 * u, 70.0 * u);

            if let Some(pos) = p.pos {
                if p.pressed && hit.contains(pos) {
                    self.slider = Some(i);
                }
                if self.slider == Some(i) {
                    let t = ((pos.x - tx) / tw).clamp(0.0, 1.0);
                    set_val(s, i, lo + t * (hi - lo));
                }
            }

            let v = get_val(s, i);
            let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
            draw_rectangle(tx, y - 5.0 * u, tw, 10.0 * u, Color::new(0.3, 0.33, 0.4, 1.0));
            draw_rectangle(tx, y - 5.0 * u, tw * t, 10.0 * u, Color::new(0.2, 0.8, 0.5, 1.0));
            draw_circle(tx + tw * t, y, 15.0 * u, WHITE);

            let label = if i == 0 || i == 2 {
                format!("{:.2}x", v)
            } else {
                format!("{}%", (v * 100.0).round() as i32)
            };
            draw_text(&label, tx + tw + 30.0 * u, y + 9.0 * u, 26.0 * u, WHITE);
        }

        let by = py + 120.0 * u + 4.0 * 72.0 * u - 10.0 * u;
        let bw = 240.0 * u;
        let bh = 64.0 * u;
        let gap = 25.0 * u;
        let bx = px + (pw - (3.0 * bw + 2.0 * gap)) * 0.5;
        let edit = Rect::new(bx, by, bw, bh);
        let reset = Rect::new(bx + bw + gap, by, bw, bh);
        let resume = Rect::new(bx + 2.0 * (bw + gap), by, bw, bh);
        ui_button(edit, "EDIT HUD", 26.0 * u);
        ui_button(reset, "RESET HUD", 26.0 * u);
        ui_button(resume, "RESUME", 26.0 * u);

        if let Some(pos) = p.pos {
            if p.pressed {
                if edit.contains(pos) {
                    self.editing = true;
                } else if reset.contains(pos) {
                    s.hud_scale = 1.0;
                    s.hud_opacity = 1.0;
                    s.btn_frac = [None; BTN_COUNT];
                } else if resume.contains(pos) {
                    return MenuAction::Resume;
                }
            }
        }
        MenuAction::None
    }
}
