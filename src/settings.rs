// src/settings.rs
// Player settings (sensitivity, volume, HUD) plus the in-game settings menu.

use macroquad::prelude::*;

pub const BTN_FIRE: usize = 0;
pub const BTN_SCOPE: usize = 1;
pub const BTN_JUMP: usize = 2;
pub const BTN_SLIDE: usize = 3;
pub const BTN_RELOAD: usize = 4;
pub const BTN_COUNT: usize = 5;

/// The movement stick is the 6th editable HUD element.
pub const EL_JOY: usize = BTN_COUNT;

pub const BTN_LABELS: [&str; BTN_COUNT] = ["FIRE", "SCOPE", "JUMP", "SLIDE", "RELOAD"];

/// Base joystick radius in pixels (multiplied by the stick's own size setting).
pub const JOY_RADIUS: f32 = 70.0;

/// Base button size in pixels (multiplied by that button's own size setting).
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

fn element_name(i: usize) -> &'static str {
    if i < BTN_COUNT {
        BTN_LABELS[i]
    } else {
        "MOVE STICK"
    }
}

#[derive(Clone)]
pub struct Settings {
    /// Look sensitivity multiplier while NOT scoped (1.0 = default).
    pub sens_camera: f32,
    /// Look sensitivity multiplier while scoped (1.0 = default; the game
    /// already slows aim down when zoomed, this scales on top of that).
    pub sens_scope: f32,
    /// Master volume 0..1.
    pub volume: f32,
    /// Touch button opacity 0..1.
    pub hud_opacity: f32,
    /// Custom button centres as a fraction of the screen (None = default spot).
    pub btn_frac: [Option<Vec2>; BTN_COUNT],
    /// Individual size multiplier for every button.
    pub btn_scale: [f32; BTN_COUNT],
    /// Custom stick centre as a fraction of the screen (None = default spot).
    pub joy_frac: Option<Vec2>,
    /// Size multiplier for the movement stick.
    pub joy_scale: f32,
}

impl Settings {
    pub fn new() -> Self {
        Self {
            sens_camera: 1.0,
            sens_scope: 1.0,
            volume: 1.0,
            hud_opacity: 1.0,
            btn_frac: [None; BTN_COUNT],
            btn_scale: [1.0; BTN_COUNT],
            joy_frac: None,
            joy_scale: 1.0,
        }
    }

    /// Positions and sizes back to default (opacity untouched).
    pub fn reset_layout(&mut self) {
        self.btn_frac = [None; BTN_COUNT];
        self.btn_scale = [1.0; BTN_COUNT];
        self.joy_frac = None;
        self.joy_scale = 1.0;
    }

    pub fn reset_hud(&mut self) {
        self.reset_layout();
        self.hud_opacity = 1.0;
    }

    pub fn button_rect(&self, i: usize, w: f32, h: f32) -> Rect {
        let size = BTN_SIZE[i] * self.btn_scale[i];
        let c = match self.btn_frac[i] {
            Some(f) => vec2(f.x * w, f.y * h),
            None => vec2(w - BTN_DEFAULT[i].0, h - BTN_DEFAULT[i].1),
        };
        Rect::new(c.x - size * 0.5, c.y - size * 0.5, size, size)
    }

    pub fn joy_center(&self, w: f32, h: f32) -> Vec2 {
        match self.joy_frac {
            Some(f) => vec2(f.x * w, f.y * h),
            None => vec2(w * 0.12, h - 150.0),
        }
    }

    pub fn joy_radius(&self) -> f32 {
        JOY_RADIUS * self.joy_scale
    }

    /// The only area where a touch starts the movement stick: a box around the
    /// stick (bottom-left by default). Everything else is buttons or aiming.
    pub fn joy_zone(&self, w: f32, h: f32) -> Rect {
        let c = self.joy_center(w, h);
        let r = self.joy_radius();
        let x0 = (c.x - r * 2.3).max(0.0);
        let y0 = (c.y - r * 2.1).max(0.0);
        let x1 = (c.x + r * 2.3).min(w);
        let y1 = (c.y + r * 2.1).min(h);
        Rect::new(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
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
    TestSound,
}

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Main,
    Sens,
    Edit,
}

pub struct Menu {
    page: Page,
    slider: Option<usize>,
    drag: Option<usize>,
    drag_off: Vec2,
    selected: usize,
}

const SL_CAMERA: usize = 0;
const SL_SCOPE: usize = 1;
const SL_VOLUME: usize = 2;
const SL_OPACITY: usize = 3;
const SL_SIZE: usize = 4;
const SLIDER_RANGES: [(f32, f32); 5] =
    [(0.3, 3.0), (0.3, 3.0), (0.0, 1.0), (0.2, 1.0), (0.5, 2.0)];

fn get_val(s: &Settings, id: usize, sel: usize) -> f32 {
    match id {
        SL_CAMERA => s.sens_camera,
        SL_SCOPE => s.sens_scope,
        SL_VOLUME => s.volume,
        SL_OPACITY => s.hud_opacity,
        _ => {
            if sel < BTN_COUNT {
                s.btn_scale[sel]
            } else {
                s.joy_scale
            }
        }
    }
}

fn set_val(s: &mut Settings, id: usize, sel: usize, v: f32) {
    match id {
        SL_CAMERA => s.sens_camera = v,
        SL_SCOPE => s.sens_scope = v,
        SL_VOLUME => s.volume = v,
        SL_OPACITY => s.hud_opacity = v,
        _ => {
            if sel < BTN_COUNT {
                s.btn_scale[sel] = v;
            } else {
                s.joy_scale = v;
            }
        }
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

fn hit(p: &Pointer, r: Rect) -> bool {
    p.pressed && p.pos.map_or(false, |pos| r.contains(pos))
}

impl Menu {
    pub fn new() -> Self {
        Self {
            page: Page::Main,
            slider: None,
            drag: None,
            drag_off: Vec2::ZERO,
            selected: 0,
        }
    }

    /// Back to the first page (called whenever the menu opens/closes).
    pub fn reset(&mut self) {
        self.page = Page::Main;
        self.slider = None;
        self.drag = None;
    }

    /// One labelled slider row. `lx` = label x, `tx`/`tw` = track, `y` = centre.
    fn slider(
        &mut self,
        s: &mut Settings,
        p: &Pointer,
        id: usize,
        label: &str,
        lx: f32,
        tx: f32,
        tw: f32,
        y: f32,
        u: f32,
    ) {
        draw_text(label, lx, y + 9.0 * u, 26.0 * u, WHITE);

        let (lo, hi) = SLIDER_RANGES[id];
        let area = Rect::new(tx - 20.0 * u, y - 35.0 * u, tw + 40.0 * u, 70.0 * u);

        if let Some(pos) = p.pos {
            if p.pressed && area.contains(pos) {
                self.slider = Some(id);
            }
            if self.slider == Some(id) {
                let t = ((pos.x - tx) / tw).clamp(0.0, 1.0);
                set_val(s, id, self.selected, lo + t * (hi - lo));
            }
        }

        let v = get_val(s, id, self.selected);
        let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
        draw_rectangle(tx, y - 5.0 * u, tw, 10.0 * u, Color::new(0.3, 0.33, 0.4, 1.0));
        draw_rectangle(tx, y - 5.0 * u, tw * t, 10.0 * u, Color::new(0.2, 0.8, 0.5, 1.0));
        draw_circle(tx + tw * t, y, 15.0 * u, WHITE);

        let text = if id == SL_VOLUME || id == SL_OPACITY {
            format!("{}%", (v * 100.0).round() as i32)
        } else {
            format!("{:.2}x", v)
        };
        draw_text(&text, tx + tw + 30.0 * u, y + 9.0 * u, 26.0 * u, WHITE);
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

        if self.page == Page::Edit {
            self.run_editor(s, &p, w, h, u);
            return MenuAction::None;
        }

        // ------------------------------------------------------ panel
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.7));

        let pw = 860.0 * u;
        let ph = 480.0 * u;
        let px = (w - pw) * 0.5;
        let py = (h - ph) * 0.5;
        draw_rectangle(px, py, pw, ph, Color::new(0.08, 0.09, 0.12, 0.97));
        draw_rectangle_lines(px, py, pw, ph, 2.0, Color::new(1.0, 1.0, 1.0, 0.4));

        let lx = px + 40.0 * u;
        let tx = px + 330.0 * u;
        let tw = pw - 330.0 * u - 150.0 * u;
        let y0 = py + 120.0 * u;
        let row = 72.0 * u;
        let by = y0 + 3.0 * row - 10.0 * u;
        let bh = 64.0 * u;

        // ------------------------------------------------ sensitivity page
        if self.page == Page::Sens {
            draw_text("SENSITIVITY", px + 30.0 * u, py + 56.0 * u, 44.0 * u, WHITE);
            self.slider(s, &p, SL_CAMERA, "CAMERA", lx, tx, tw, y0, u);
            self.slider(s, &p, SL_SCOPE, "SCOPE", lx, tx, tw, y0 + row, u);
            draw_text(
                "Scope sensitivity is used while zoomed in.",
                lx,
                y0 + 2.0 * row + 10.0 * u,
                22.0 * u,
                Color::new(1.0, 1.0, 1.0, 0.6),
            );

            let bw = 240.0 * u;
            let back = Rect::new(px + (pw - bw) * 0.5, by, bw, bh);
            ui_button(back, "BACK", 26.0 * u);
            if hit(&p, back) {
                self.page = Page::Main;
                self.slider = None;
            }
            return MenuAction::None;
        }

        // ---------------------------------------------------- main page
        draw_text("SETTINGS", px + 30.0 * u, py + 56.0 * u, 44.0 * u, WHITE);

        draw_text("SENSITIVITY", lx, y0 + 9.0 * u, 26.0 * u, WHITE);
        let sens_btn = Rect::new(tx, y0 - 28.0 * u, tw, 56.0 * u);
        ui_button(sens_btn, "CAMERA / SCOPE   >", 24.0 * u);

        self.slider(s, &p, SL_VOLUME, "VOLUME", lx, tx, tw, y0 + row, u);
        self.slider(s, &p, SL_OPACITY, "HUD OPACITY", lx, tx, tw, y0 + 2.0 * row, u);

        let bw = 190.0 * u;
        let gap = 20.0 * u;
        let bx = px + (pw - (4.0 * bw + 3.0 * gap)) * 0.5;
        let edit = Rect::new(bx, by, bw, bh);
        let reset = Rect::new(bx + (bw + gap), by, bw, bh);
        let test = Rect::new(bx + 2.0 * (bw + gap), by, bw, bh);
        let resume = Rect::new(bx + 3.0 * (bw + gap), by, bw, bh);
        ui_button(edit, "EDIT HUD", 24.0 * u);
        ui_button(reset, "RESET HUD", 24.0 * u);
        ui_button(test, "TEST SOUND", 24.0 * u);
        ui_button(resume, "RESUME", 24.0 * u);

        if hit(&p, sens_btn) {
            self.page = Page::Sens;
            self.slider = None;
        } else if hit(&p, edit) {
            self.page = Page::Edit;
            self.slider = None;
            self.drag = None;
        } else if hit(&p, reset) {
            s.reset_hud();
        } else if hit(&p, test) {
            return MenuAction::TestSound;
        } else if hit(&p, resume) {
            return MenuAction::Resume;
        }
        MenuAction::None
    }

    // ---------------------------------------------------------- HUD editor
    fn run_editor(&mut self, s: &mut Settings, p: &Pointer, w: f32, h: f32, u: f32) {
        draw_rectangle(0.0, 0.0, w, h, Color::new(0.0, 0.0, 0.0, 0.6));

        // the movement stick + the area that activates it
        let jc = s.joy_center(w, h);
        let jr = s.joy_radius();
        let zone = s.joy_zone(w, h);
        draw_rectangle_lines(zone.x, zone.y, zone.w, zone.h, 2.0, Color::new(1.0, 1.0, 1.0, 0.25));
        draw_circle(jc.x, jc.y, jr, Color::new(1.0, 1.0, 1.0, 0.12));
        draw_circle_lines(jc.x, jc.y, jr, 3.0, Color::new(1.0, 1.0, 1.0, 0.6));
        centered_text(
            "MOVE",
            Rect::new(jc.x - jr, jc.y - jr, jr * 2.0, jr * 2.0),
            22.0 * u,
            WHITE,
        );

        // the buttons
        for i in 0..BTN_COUNT {
            let r = s.button_rect(i, w, h);
            crate::mobile::TouchControls::draw_button(
                r,
                BTN_LABELS[i],
                self.drag == Some(i),
                btn_color(i),
                1.0,
            );
        }

        // highlight whatever is selected
        let hl = Color::new(1.0, 0.95, 0.3, 1.0);
        if self.selected < BTN_COUNT {
            let r = s.button_rect(self.selected, w, h);
            draw_rectangle_lines(r.x - 5.0, r.y - 5.0, r.w + 10.0, r.h + 10.0, 3.0, hl);
        } else {
            draw_circle_lines(jc.x, jc.y, jr + 6.0, 3.0, hl);
        }

        // title + buttons
        let title = "TAP A CONTROL, DRAG TO MOVE, USE THE SLIDER TO RESIZE IT";
        let tsz = 24.0 * u;
        let tw = measure_text(title, None, tsz as u16, 1.0).width;
        draw_text(title, (w - tw) * 0.5, 36.0 * u, tsz, WHITE);

        let done = Rect::new(w * 0.5 - 215.0 * u, 54.0 * u, 200.0 * u, 52.0 * u);
        let reset = Rect::new(w * 0.5 + 15.0 * u, 54.0 * u, 200.0 * u, 52.0 * u);
        ui_button(done, "DONE", 26.0 * u);
        ui_button(reset, "RESET", 26.0 * u);

        // size slider for the selected element
        let sy = h - 50.0 * u;
        let stx = w * 0.5 - 100.0 * u;
        let stw = 340.0 * u;
        let slider_area = Rect::new(stx - 20.0 * u, sy - 35.0 * u, stw + 40.0 * u, 70.0 * u);
        draw_rectangle(0.0, h - 100.0 * u, w, 100.0 * u, Color::new(0.0, 0.0, 0.0, 0.55));

        // ---- input
        if let Some(pos) = p.pos {
            if p.pressed {
                if done.contains(pos) {
                    self.page = Page::Main;
                    self.slider = None;
                    self.drag = None;
                    return;
                } else if reset.contains(pos) {
                    s.reset_layout();
                } else if !slider_area.contains(pos) {
                    let mut picked: Option<usize> = None;
                    if (pos - jc).length() <= jr {
                        picked = Some(EL_JOY);
                    }
                    for i in 0..BTN_COUNT {
                        if s.button_rect(i, w, h).contains(pos) {
                            picked = Some(i);
                        }
                    }
                    if let Some(i) = picked {
                        self.selected = i;
                        self.drag = Some(i);
                        let centre = if i < BTN_COUNT {
                            let r = s.button_rect(i, w, h);
                            vec2(r.x + r.w * 0.5, r.y + r.h * 0.5)
                        } else {
                            jc
                        };
                        self.drag_off = centre - pos;
                    }
                }
            }
            if let Some(i) = self.drag {
                let c = pos + self.drag_off;
                let f = vec2((c.x / w).clamp(0.04, 0.96), (c.y / h).clamp(0.06, 0.94));
                if i < BTN_COUNT {
                    s.btn_frac[i] = Some(f);
                } else {
                    s.joy_frac = Some(f);
                }
            }
        }

        let label = format!("{} SIZE", element_name(self.selected));
        self.slider(s, p, SL_SIZE, &label, w * 0.5 - 420.0 * u, stx, stw, sy, u);
    }
}
