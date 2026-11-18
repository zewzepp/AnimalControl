// src/main.rs
mod ar_model;
mod bots;
mod map;
mod mobile;
mod player;
mod settings;
mod sfx;
mod weapon;

use macroquad::audio::{play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

use map::{los_clear, ray_aabb, spread_dir, Map};
use mobile::{menu_button_rect, weapon_slot_rects, TouchControls, MOBILE_AIM_SCALE};
use player::Player;
use settings::{Menu, MenuAction, Settings};
use sfx::Sfx;
use weapon::{Kind, Weapon};

struct Tracer {
    a: Vec3,
    b: Vec3,
    life: f32,
    max: f32,
    col: Color,
}

/// Small impact spark / debris particle.
struct Spark {
    pos: Vec3,
    vel: Vec3,
    life: f32,
    max: f32,
    col: Color,
}

/// Screen-space frame of the first-person gun: origin, barrel axis, "down" axis, scale.
struct GunXf {
    p: Vec2,
    ax: Vec2,
    ay: Vec2,
    k: f32,
    alpha: f32,
    mx: f32, // muzzle distance along the barrel (local units)
}

impl GunXf {
    fn pt(&self, lx: f32, ly: f32) -> Vec2 {
        self.p + self.ax * (lx * self.k) + self.ay * (ly * self.k)
    }
}

fn tri_quad(a: Vec2, b: Vec2, c: Vec2, d: Vec2, col: Color) {
    draw_triangle(a, b, c, col);
    draw_triangle(a, c, d, col);
}

fn gun_rect(xf: &GunXf, x0: f32, y0: f32, x1: f32, y1: f32, col: Color) {
    let c = Color::new(col.r, col.g, col.b, col.a * xf.alpha);
    tri_quad(xf.pt(x0, y0), xf.pt(x1, y0), xf.pt(x1, y1), xf.pt(x0, y1), c);
}

fn arm_quad(from: Vec2, dir: Vec2, len: f32, half: f32, col: Color, alpha: f32) {
    let d = dir.normalize_or_zero();
    let perp = vec2(-d.y, d.x) * half;
    let end = from + d * len;
    let c = Color::new(col.r, col.g, col.b, col.a * alpha);
    tri_quad(from + perp, from - perp, end - perp * 1.25, end + perp * 1.25, c);
}

fn gun_quad(xf: &GunXf, p: [(f32, f32); 4], col: Color) {
    let c = Color::new(col.r, col.g, col.b, col.a * xf.alpha);
    tri_quad(xf.pt(p[0].0, p[0].1), xf.pt(p[1].0, p[1].1), xf.pt(p[2].0, p[2].1), xf.pt(p[3].0, p[3].1), c);
}

const SKIN: Color = Color::new(0.93, 0.74, 0.58, 1.0);
const SKIN_SHADE: Color = Color::new(0.80, 0.60, 0.46, 1.0);
const SLEEVE: Color = Color::new(0.22, 0.32, 0.28, 1.0);
const CUFF: Color = Color::new(0.30, 0.42, 0.36, 1.0);
const BODY: Color = Color::new(0.17, 0.18, 0.20, 1.0);
const DARK: Color = Color::new(0.09, 0.09, 0.11, 1.0);
const ACCENT: Color = Color::new(0.32, 0.34, 0.38, 1.0);

/// Grip hand + support hand shared by the two guns. `sx` = where the support hand sits.
fn draw_gun_hands(xf: &GunXf, sx: f32) {
    let k = xf.k;
    // forearms (behind the hands)
    arm_quad(xf.pt(-34.0, 52.0), vec2(0.45, 1.0), 520.0 * k, 40.0 * k, SLEEVE, xf.alpha);
    arm_quad(xf.pt(sx + 30.0, 44.0), vec2(-0.30, 1.0), 520.0 * k, 36.0 * k, SLEEVE, xf.alpha);
    // cuffs
    let c1 = xf.pt(-34.0, 52.0);
    arm_quad(c1, vec2(0.45, 1.0), 40.0 * k, 41.0 * k, CUFF, xf.alpha);
    let c2 = xf.pt(sx + 30.0, 44.0);
    arm_quad(c2, vec2(-0.30, 1.0), 36.0 * k, 37.0 * k, CUFF, xf.alpha);

    // grip hand: palm wrapped round the grip, thumb over the receiver, finger on the trigger
    gun_rect(xf, -60.0, 24.0, -6.0, 66.0, SKIN);
    gun_rect(xf, -60.0, 56.0, -6.0, 66.0, SKIN_SHADE);
    gun_rect(xf, -24.0, 0.0, 30.0, 20.0, SKIN); // thumb
    gun_rect(xf, -6.0, 18.0, 26.0, 28.0, SKIN); // trigger finger

    // support hand: palm under the handguard, fingers curled over the top
    gun_rect(xf, sx, 12.0, sx + 86.0, 46.0, SKIN);
    gun_rect(xf, sx, 38.0, sx + 86.0, 46.0, SKIN_SHADE);
    gun_rect(xf, sx + 6.0, -16.0, sx + 80.0, -4.0, SKIN); // fingers on top
    gun_rect(xf, sx - 8.0, -2.0, sx + 20.0, 14.0, SKIN); // thumb
}

/// The penguin rifle with Roblox-style blocky grey arms (traced from the reference render,
/// see ar_model.rs). Everything is drawn as flat triangles in the gun's local frame, so
/// bob / kick / reload dip / sprint lowering all still work.
fn draw_ar(xf: &GunXf) {
    let inv = 1.0 / ar_model::MODEL_SCALE;
    let mut pal = [Color::new(0.0, 0.0, 0.0, 0.0); ar_model::PALETTE.len()];
    for (i, c) in ar_model::PALETTE.iter().enumerate() {
        pal[i] = Color::new(
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
            xf.alpha,
        );
    }
    for (ci, t) in ar_model::TRIS.iter() {
        let a = xf.pt(t[0] as f32 * inv, t[1] as f32 * inv);
        let b = xf.pt(t[2] as f32 * inv, t[3] as f32 * inv);
        let c = xf.pt(t[4] as f32 * inv, t[5] as f32 * inv);
        draw_triangle(a, b, c, pal[*ci as usize]);
    }

    // penguin badge on the receiver
    let (lx, ly, lr) = ar_model::LOGO;
    let r = lr * xf.k;
    let a = xf.alpha;
    let ctr = xf.pt(lx, ly);
    draw_circle(ctr.x, ctr.y, r, Color::new(0.95, 0.95, 0.97, a));
    draw_circle(ctr.x, ctr.y, r * 0.88, Color::new(0.08, 0.08, 0.10, a));
    let face = xf.pt(lx + lr * 0.05, ly + lr * 0.22);
    draw_circle(face.x, face.y, r * 0.58, Color::new(0.96, 0.96, 0.98, a));
    let e1 = xf.pt(lx - lr * 0.18, ly + lr * 0.08);
    let e2 = xf.pt(lx + lr * 0.26, ly + lr * 0.08);
    draw_circle(e1.x, e1.y, r * 0.09, Color::new(0.05, 0.05, 0.06, a));
    draw_circle(e2.x, e2.y, r * 0.09, Color::new(0.05, 0.05, 0.06, a));
    let bk = xf.pt(lx + lr * 0.04, ly + lr * 0.30);
    draw_circle(bk.x, bk.y, r * 0.12, Color::new(0.93, 0.55, 0.14, a));
}

fn draw_sniper(xf: &GunXf) {
    gun_rect(xf, -190.0, -8.0, -62.0, 26.0, DARK); // stock
    gun_rect(xf, -150.0, -16.0, -90.0, -8.0, BODY); // cheek rest
    gun_rect(xf, -62.0, -14.0, 120.0, 16.0, BODY); // receiver
    gun_rect(xf, 120.0, -10.0, 250.0, 10.0, ACCENT); // handguard
    gun_rect(xf, 250.0, -5.0, 370.0, 5.0, DARK); // barrel
    gun_rect(xf, 350.0, -9.0, 376.0, 9.0, DARK); // muzzle brake
    gun_rect(xf, 0.0, -40.0, 170.0, -24.0, DARK); // scope tube
    gun_rect(xf, -6.0, -45.0, 6.0, -19.0, ACCENT); // eyepiece
    gun_rect(xf, 164.0, -48.0, 178.0, -16.0, ACCENT); // objective bell
    gun_rect(xf, 20.0, -24.0, 32.0, -14.0, ACCENT); // mount
    gun_rect(xf, 130.0, -24.0, 142.0, -14.0, ACCENT); // mount
    gun_rect(xf, -34.0, -10.0, -14.0, -2.0, ACCENT); // bolt handle
    gun_rect(xf, 10.0, 16.0, 46.0, 40.0, DARK); // magazine
    gun_quad(xf, [(-44.0, 16.0), (-14.0, 16.0), (-26.0, 66.0), (-56.0, 64.0)], BODY); // grip
    draw_gun_hands(xf, 135.0);
}

fn draw_knife(xf: &GunXf) {
    let k = xf.k;
    arm_quad(xf.pt(-60.0, 6.0), vec2(0.55, 1.0), 560.0 * k, 44.0 * k, SLEEVE, xf.alpha);
    arm_quad(xf.pt(-60.0, 6.0), vec2(0.55, 1.0), 44.0 * k, 45.0 * k, CUFF, xf.alpha);
    // blade
    gun_quad(xf, [(24.0, -14.0), (180.0, -14.0), (236.0, 2.0), (24.0, 8.0)], Color::new(0.78, 0.80, 0.85, 1.0));
    gun_rect(xf, 24.0, -14.0, 182.0, -8.0, Color::new(0.94, 0.95, 1.0, 1.0)); // spine highlight
    gun_rect(xf, 8.0, -24.0, 24.0, 20.0, DARK); // guard
    gun_rect(xf, -92.0, -10.0, 8.0, 12.0, Color::new(0.27, 0.18, 0.12, 1.0)); // handle
    // fist
    gun_rect(xf, -86.0, -16.0, -6.0, 20.0, SKIN);
    gun_rect(xf, -86.0, 12.0, -6.0, 20.0, SKIN_SHADE);
    gun_rect(xf, -56.0, -24.0, 2.0, -12.0, SKIN); // thumb
}

fn draw_iron_sights(w: f32, h: f32, a: f32) {
    let cx = w * 0.5;
    let cy = h * 0.5;
    let body = Color::new(0.13, 0.14, 0.16, 0.97 * a);
    let rail = Color::new(0.28, 0.30, 0.34, a);
    let dark = Color::new(0.05, 0.05, 0.06, a);
    // rifle seen from behind
    tri_quad(
        vec2(cx - 0.24 * h, h),
        vec2(cx + 0.24 * h, h),
        vec2(cx + 0.085 * h, cy + 0.21 * h),
        vec2(cx - 0.085 * h, cy + 0.21 * h),
        body,
    );
    tri_quad(
        vec2(cx - 0.05 * h, h),
        vec2(cx + 0.05 * h, h),
        vec2(cx + 0.035 * h, cy + 0.21 * h),
        vec2(cx - 0.035 * h, cy + 0.21 * h),
        rail,
    );
    // rear sight posts
    draw_rectangle(cx - 0.085 * h, cy + 0.135 * h, 0.035 * h, 0.08 * h, dark);
    draw_rectangle(cx + 0.05 * h, cy + 0.135 * h, 0.035 * h, 0.08 * h, dark);
    // front sight post with guards; tip sits on the crosshair
    draw_rectangle(cx - 0.0045 * h, cy, 0.009 * h, 0.135 * h, dark);
    draw_rectangle(cx - 0.032 * h, cy + 0.03 * h, 0.007 * h, 0.105 * h, dark);
    draw_rectangle(cx + 0.025 * h, cy + 0.03 * h, 0.007 * h, 0.105 * h, dark);
    draw_circle(cx, cy, 0.006 * h, Color::new(1.0, 0.2, 0.1, a));
}

struct Game {
    map: Map,
    player: Player,
    weapon: Weapon,
    bots: Vec<bots::Bot>,
    tracers: Vec<Tracer>,
    sparks: Vec<Spark>,
    /// true while the knife is held instead of the gun
    held_melee: bool,
    melee_cd: f32,
    swing_t: f32,
    switch_t: f32,
    sfx: Sfx,
    hitmarker: f32,
    damage_flash: f32,
    muzzle: f32,
    /// 0 = not scoped, 1 = fully scoped (moves over ~0.3s for a smooth zoom)
    zoom: f32,
    /// smoothed look delta (screen-normalized), for fluid camera turns
    look_sx: f32,
    look_sy: f32,
    vignette: Texture2D,
    score: u32,
    deaths: u32,
    paused: bool,
    touch: TouchControls,
    settings: Settings,
    menu: Menu,
}

/// Scope zoom length in seconds.
const ZOOM_TIME: f32 = 0.15;
const FOV_NORMAL: f32 = 78.0;
const FOV_SNIPER: f32 = 26.0;
/// Iron sights only zoom a little.
const FOV_AR_ADS: f32 = 62.0;
/// The vignette quad is drawn this many screen-heights wide.
const VIGNETTE_SPAN: f32 = 2.6;

fn ease(t: f32) -> f32 {
    // smootherstep: gentler start and end than smoothstep
    let t = t.clamp(0.0, 1.0);
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Soft radial darkening, built once at startup. Transparent in the middle,
/// fading to dark towards the edges (never fully black).
fn make_vignette() -> Texture2D {
    const N: u16 = 256;
    let mut img = Image::gen_image_color(N, N, Color::new(0.0, 0.0, 0.0, 0.0));
    for y in 0..N as u32 {
        for x in 0..N as u32 {
            let dx = (x as f32 + 0.5) / N as f32 - 0.5;
            let dy = (y as f32 + 0.5) / N as f32 - 0.5;
            // distance from centre measured in screen heights
            let d = (dx * dx + dy * dy).sqrt() * VIGNETTE_SPAN;
            let a = ease((d - 0.25) / 0.50) * 0.85;
            img.set_pixel(x, y, Color::new(0.0, 0.0, 0.0, a));
        }
    }
    let tex = Texture2D::from_image(&img);
    tex.set_filter(FilterMode::Linear);
    tex
}

fn window_conf() -> Conf {
    Conf {
        window_title: "ANIMAL CONTROL".to_owned(),
        window_width: 1280,
        window_height: 720,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Prevent touches from also generating mouse events (would cause double
    // input on Android: a tap on a button would also fire a left-click).
    macroquad::input::simulate_mouse_with_touch(false);

    let sfx = Sfx::load().await;
    let mut game = Game::new(sfx);

    let _ = set_cursor_grab(true);
    let _ = show_mouse(false);

    loop {
        let dt = get_frame_time().min(1.0 / 20.0);
        game.update(dt);
        game.draw();
        next_frame().await;
    }
}

impl Game {
    fn new(sfx: Sfx) -> Self {
        let map = Map::arena();
        let spawns = map.spawns.clone();

        let player = Player::new(spawns[0]);
        let bots = (1..=1)
            .map(|i| bots::Bot::new(spawns[i % spawns.len()]))
            .collect();

        Self {
            map,
            player,
            weapon: Weapon::new(Kind::Ar),
            bots,
            tracers: Vec::new(),
            sparks: Vec::new(),
            held_melee: false,
            melee_cd: 0.0,
            swing_t: 0.0,
            switch_t: 0.0,
            sfx,
            hitmarker: 0.0,
            damage_flash: 0.0,
            muzzle: 0.0,
            zoom: 0.0,
            look_sx: 0.0,
            look_sy: 0.0,
            vignette: make_vignette(),
            score: 0,
            deaths: 0,
            paused: false,
            touch: TouchControls::new(),
            settings: Settings::new(),
            menu: Menu::new(),
        }
    }

    /// Play a sound at `vol`, scaled by the master volume setting.
    fn play(&self, snd: &Sound, vol: f32) {
        play_sound(
            snd,
            PlaySoundParams { looped: false, volume: vol * self.settings.volume },
        );
    }

    fn open_menu(&mut self) {
        self.paused = true;
        self.menu.reset();
        self.touch.reset();
        let _ = set_cursor_grab(false);
        let _ = show_mouse(true);
    }

    fn resume(&mut self) {
        self.apply_loadout();
        self.paused = false;
        self.menu.reset();
        self.touch.reset();
        let _ = set_cursor_grab(true);
        let _ = show_mouse(false);
    }

    fn update(&mut self, dt: f32) {
        if is_key_pressed(KeyCode::Escape) {
            if self.paused {
                self.resume();
            } else {
                self.open_menu();
            }
        }
        // desktop / browser: click the MENU button
        if !self.paused && is_mouse_button_pressed(MouseButton::Left) {
            let (mx, my) = mouse_position();
            if menu_button_rect(screen_width(), screen_height()).contains(vec2(mx, my)) {
                self.open_menu();
            }
        }
        if self.paused {
            return;
        }

        // --- timers
        self.hitmarker = (self.hitmarker - dt).max(0.0);
        self.melee_cd = (self.melee_cd - dt).max(0.0);
        self.swing_t = (self.swing_t - dt).max(0.0);
        self.switch_t = (self.switch_t - dt).max(0.0);
        self.damage_flash = (self.damage_flash - dt).max(0.0);
        self.muzzle = (self.muzzle - dt).max(0.0);
        for t in self.tracers.iter_mut() {
            t.life -= dt;
        }
        self.tracers.retain(|t| t.life > 0.0);
        for s in self.sparks.iter_mut() {
            s.life -= dt;
            s.vel.y -= 9.0 * dt;
            s.pos += s.vel * dt;
        }
        self.sparks.retain(|s| s.life > 0.0);

        // --- touch input (must run before player.update)
        let tf = self.touch.update(&self.settings);
        if tf.menu_pressed {
            self.open_menu();
            return;
        }
        self.player.mobile_move = tf.movement;
        self.player.mobile_jump = tf.jump_pressed;
        self.player.mobile_slide_active = tf.slide_held;
        self.player.mobile_fire = tf.fire_held;
        // SLIDE button: while walking a tap toggles sneak; while running it slides
        let running = tf.movement.length_squared() > 0.5;
        if tf.slide_pressed && self.player.alive {
            if self.player.sneak {
                // stand up, whether walking or running
                self.player.sneak = false;
            } else if !running {
                self.player.sneak = true;
            }
        }

        // weapon slots (tap the gun / knife box, or keys 1 / 2)
        let mut want = tf.switch_to;
        if is_key_pressed(KeyCode::Key1) {
            want = Some(0);
        }
        if is_key_pressed(KeyCode::Key2) {
            want = Some(1);
        }
        if let Some(i) = want {
            self.select_slot(i == 1);
        }

        // per-weapon feel
        let (scope_look, scope_speed) = match self.weapon.kind {
            Kind::Sniper => (0.45, 3.6),
            Kind::Ar => (0.8, 4.8),
        };
        self.player.scope_look = scope_look;
        self.player.scope_speed = scope_speed;
        self.player.speed_mul = if self.held_melee { 1.12 } else { 1.0 };
        self.player.run_cancels_sneak = self.settings.run_cancels_sneak;
        if self.held_melee {
            self.player.mobile_scope = false;
        }
        // scope is a toggle: tap SCOPE to zoom in, tap again to zoom out
        if tf.scope_pressed {
            self.player.mobile_scope = !self.player.mobile_scope;
        }

        // --- look: mobile aim (pixel delta → normalized) OR mouse delta
        let (dx, dy) = if tf.aim_delta.length_squared() > 0.01 {
            let sw = screen_width().max(1.0);
            (
                tf.aim_delta.x / sw * MOBILE_AIM_SCALE,
                tf.aim_delta.y / sw * MOBILE_AIM_SCALE,
            )
        } else {
            let md = mouse_delta_position();
            (md.x.clamp(-0.5, 0.5), md.y.clamp(-0.5, 0.5))
        };
        // exponential smoothing of the look delta (frame-rate independent);
        // the tail keeps decaying after the finger lifts, which feels fluid
        let a = 1.0 - (-48.0 * dt).exp();
        self.look_sx += (dx - self.look_sx) * a;
        self.look_sy += (dy - self.look_sy) * a;
        let (dx, dy) = (self.look_sx, self.look_sy);
        let ez = ease(self.zoom);
        let sens = self.settings.sens_camera
            + (self.settings.sens_scope - self.settings.sens_camera) * ez;
        let engaged = tf.fire_held || self.player.mobile_scope || is_mouse_button_down(MouseButton::Left);
        let assist = self.aim_assist(dt, engaged);
        self.player.look(dx * sens * assist, dy * sens * assist);

        // --- scope
        self.player.scoped = (is_mouse_button_down(MouseButton::Right)
            || self.player.mobile_scope)
            && self.player.alive
            && !self.held_melee;

        // smooth zoom in/out
        let zoom_target = if self.player.scoped { 1.0 } else { 0.0 };
        let zoom_step = dt / ZOOM_TIME;
        if self.zoom < zoom_target {
            self.zoom = (self.zoom + zoom_step).min(zoom_target);
        } else {
            self.zoom = (self.zoom - zoom_step).max(zoom_target);
        }

        // --- player movement
        let was_on_ground = self.player.on_ground;
        let was_sliding = self.player.sliding;
        self.player.update(dt, &self.map);
        if self.player.alive {
            if was_on_ground && self.player.vel.y > 2.0 {
                self.play(&self.sfx.jump, 0.3);
            }
            if !was_sliding && self.player.sliding {
                self.play(&self.sfx.slide, 0.35);
            }
        }

        // --- weapon
        self.weapon.update(dt);

        if self.player.alive && self.held_melee {
            if (is_mouse_button_down(MouseButton::Left) || self.player.mobile_fire)
                && self.melee_cd <= 0.0
                && self.switch_t <= 0.0
            {
                self.melee_attack();
            }
        } else if self.player.alive {
            // reload (keyboard R OR mobile reload button)
            if is_key_pressed(KeyCode::R) || tf.reload_pressed {
                let was = self.weapon.reloading;
                self.weapon.start_reload();
                if was <= 0.0 && self.weapon.reloading > 0.0 {
                    self.play(&self.sfx.reload, 0.5);
                    self.player.mobile_scope = false;
                }
            }

            // dry-fire auto reload
            if (is_mouse_button_pressed(MouseButton::Left) || tf.fire_just_pressed)
                && self.weapon.mag <= 0
                && self.weapon.reloading <= 0.0
            {
                self.play(&self.sfx.empty, 0.4);
                self.weapon.start_reload();
                if self.weapon.reloading > 0.0 {
                    self.player.mobile_scope = false;
                }
            }

            // fire
            if (is_mouse_button_down(MouseButton::Left) || self.player.mobile_fire)
                && self.weapon.can_fire()
                && self.switch_t <= 0.0
            {
                self.fire();
            }
        }

        // --- bots
        let pbounds = self.player.bounds();
        let ppos = self.player.pos;
        let palive = self.player.alive;

        let mut shots = Vec::new();
        for bot in self.bots.iter_mut() {
            if let Some(s) = bot.update(dt, ppos, palive, &pbounds, &self.map) {
                shots.push(s);
            }
        }

        for s in shots {
            self.tracers.push(Tracer {
                a: s.from,
                b: s.to,
                life: 0.12,
                max: 0.12,
                col: Color::new(1.0, 0.35, 0.2, 1.0),
            });
            self.play(&self.sfx.shoot_enemy, 0.25);
            if s.hit_player && self.player.alive {
                self.player.health -= s.damage;
                self.damage_flash = 0.35;
                if self.player.health <= 0.0 {
                    self.player.health = 0.0;
                    self.player.die();
                    self.deaths += 1;
                }
            }
        }
    }

    fn fire(&mut self) {
        let sniper = self.weapon.kind == Kind::Sniper;
        self.weapon.fire();
        self.muzzle = if sniper { 0.09 } else { 0.06 };

        // medium kick, capped so sustained fire can't run away
        let (kick, kick_rand, yaw_r) = if sniper {
            (0.04f32, 0.01f32, 0.012f32)
        } else {
            (0.009, 0.005, 0.006)
        };
        self.player.recoil_pitch = (self.player.recoil_pitch
            + kick
            + macroquad::rand::gen_range(0.0f32, kick_rand))
        .min(0.14);
        self.player.recoil_yaw = (self.player.recoil_yaw
            + macroquad::rand::gen_range(-yaw_r, yaw_r))
        .clamp(-0.05, 0.05);

        self.play(&self.sfx.shoot, if sniper { 0.9 } else { 0.5 });

        let eye = self.player.eye_pos();
        let dir = self.player.aim_dir();
        let spread = self.weapon.spread(self.player.scoped, self.player.sliding);
        let d = spread_dir(dir, spread);

        let mut best_t = f32::INFINITY;
        let mut best_bot: Option<usize> = None;

        for b in &self.map.boxes {
            if let Some(t) = ray_aabb(eye, d, b) {
                if t < best_t {
                    best_t = t;
                    best_bot = None;
                }
            }
        }
        for (i, bot) in self.bots.iter().enumerate() {
            if !bot.alive {
                continue;
            }
            if let Some(t) = ray_aabb(eye, d, &bot.bounds()) {
                if t < best_t {
                    best_t = t;
                    best_bot = Some(i);
                }
            }
        }

        let end = if best_t.is_finite() {
            eye + d * best_t
        } else {
            eye + d * 120.0
        };
        // tracer leaves from the gun muzzle (lower right of the view)
        let up = vec3(0.0, 1.0, 0.0);
        let right = dir.cross(up).normalize_or_zero();
        let upv = right.cross(dir);
        let muzzle = if sniper {
            eye + dir * 0.9 + right * 0.22 - upv * 0.16
        } else {
            // penguin rifle: the beak sits up-left of the crosshair (see ar_model.rs pose)
            eye + dir * 0.9 - right * 0.25 + upv * 0.15
        };
        let (tl, tcol) = if sniper {
            (0.3, Color::new(0.6, 0.85, 1.0, 1.0))
        } else {
            (0.16, Color::new(1.0, 0.8, 0.35, 1.0))
        };
        self.tracers.push(Tracer {
            a: muzzle,
            b: end,
            life: tl,
            max: tl,
            col: tcol,
        });
        if best_t.is_finite() {
            let (col, n) = if best_bot.is_some() {
                (Color::new(1.0, 0.25, 0.15, 1.0), 9)
            } else {
                (Color::new(1.0, 0.9, 0.55, 1.0), 6)
            };
            self.spawn_sparks(end - d * 0.05, d, col, n);
        }

        if let Some(i) = best_bot {
            let dmg = self.weapon.damage;
            self.damage_bot(i, dmg);
        }
    }

    fn spawn_sparks(&mut self, pos: Vec3, d: Vec3, col: Color, n: usize) {
        for _ in 0..n {
            let r = vec3(
                macroquad::rand::gen_range(-1.0f32, 1.0),
                macroquad::rand::gen_range(-0.2f32, 1.0),
                macroquad::rand::gen_range(-1.0f32, 1.0),
            );
            let life = macroquad::rand::gen_range(0.18f32, 0.4);
            self.sparks.push(Spark {
                pos,
                vel: -d * macroquad::rand::gen_range(1.0f32, 3.0) + r * 2.5,
                life,
                max: life,
                col,
            });
        }
    }

    fn damage_bot(&mut self, i: usize, dmg: f32) {
        self.hitmarker = 0.15;
        self.play(&self.sfx.hit, 0.35);

        let mut killed = false;
        {
            let bot = &mut self.bots[i];
            bot.health -= dmg;
            bot.hit_flash = 0.12;
            if bot.health <= 0.0 && bot.alive {
                bot.alive = false;
                bot.respawn_t = 3.0;
                killed = true;
            }
        }
        if killed {
            self.score += 1;
            self.play(&self.sfx.kill, 0.5);
        }
    }

    /// Knife swing: short reach, hits the nearest enemy in front (walls block it).
    fn melee_attack(&mut self) {
        self.melee_cd = 0.45;
        self.swing_t = 0.25;
        self.play(&self.sfx.slide, 0.3);

        let eye = self.player.eye_pos();
        let d = self.player.aim_dir();
        let reach = 2.6;

        let mut wall_t = f32::INFINITY;
        for b in &self.map.boxes {
            if let Some(t) = ray_aabb(eye, d, b) {
                wall_t = wall_t.min(t);
            }
        }
        let mut best: Option<(usize, f32)> = None;
        for (i, bot) in self.bots.iter().enumerate() {
            if !bot.alive {
                continue;
            }
            if let Some(t) = ray_aabb(eye, d, &bot.bounds()) {
                if t <= reach && t < wall_t && best.map_or(true, |(_, bt)| t < bt) {
                    best = Some((i, t));
                }
            }
        }
        if let Some((i, t)) = best {
            self.damage_bot(i, 60.0);
            self.spawn_sparks(eye + d * t, d, Color::new(1.0, 0.25, 0.15, 1.0), 8);
        }
    }

    /// Swap between the gun (false) and the knife (true).
    fn select_slot(&mut self, melee: bool) {
        if melee == self.held_melee || !self.player.alive {
            return;
        }
        self.held_melee = melee;
        self.switch_t = 0.28;
        self.swing_t = 0.0;
        self.weapon.reloading = 0.0;
        self.player.mobile_scope = false;
        self.play(&self.sfx.reload, 0.15);
    }

    /// Apply the loadout chosen in the menu (new gun starts with full ammo).
    fn apply_loadout(&mut self) {
        if self.weapon.kind != self.settings.primary {
            self.weapon = Weapon::new(self.settings.primary);
            self.held_melee = false;
            self.player.mobile_scope = false;
            self.zoom = 0.0;
            self.switch_t = 0.28;
        }
    }

    /// Minimal aim assist: slows the camera slightly while the crosshair is on
    /// an enemy, and while firing/scoped nudges it gently toward their centre.
    /// Returns the sensitivity multiplier for this frame.
    fn aim_assist(&mut self, dt: f32, engaged: bool) -> f32 {
        if !self.player.alive {
            return 1.0;
        }
        let eye = self.player.eye_pos();
        let fwd = self.player.aim_dir();
        let mut best: Option<(f32, Vec3)> = None;
        for bot in &self.bots {
            if !bot.alive {
                continue;
            }
            let c = bot.pos + vec3(0.0, 1.1, 0.0);
            let to = c - eye;
            let dist = to.length();
            if dist < 1.0 || dist > 60.0 {
                continue;
            }
            let dir = to / dist;
            let ang = fwd.dot(dir).clamp(-1.0, 1.0).acos();
            let cone = (0.8 / dist).atan().clamp(0.02, 0.07);
            if ang < cone && los_clear(eye, c, &self.map) {
                if best.map_or(true, |(a, _)| ang < a) {
                    best = Some((ang, dir));
                }
            }
        }
        if let Some((_, dir)) = best {
            if engaged {
                let ty = (-dir.x).atan2(-dir.z);
                let tp = dir.y.clamp(-1.0, 1.0).asin();
                let mut dyaw = ty - (self.player.yaw + self.player.recoil_yaw);
                while dyaw > std::f32::consts::PI {
                    dyaw -= std::f32::consts::TAU;
                }
                while dyaw < -std::f32::consts::PI {
                    dyaw += std::f32::consts::TAU;
                }
                let dp = tp - (self.player.pitch + self.player.recoil_pitch);
                let pull = 1.0 - (-2.5 * dt).exp();
                self.player.yaw += dyaw * pull;
                self.player.pitch = (self.player.pitch + dp * pull).clamp(-1.5, 1.5);
            }
            return 0.8;
        }
        1.0
    }

    /// Where the held weapon sits on screen, with bob / kick / reload / slash / draw-in animation.
    fn gun_xf(&self) -> GunXf {
        let w = screen_width();
        let h = screen_height();
        let k = h / 720.0 * 0.95;
        let p = &self.player;
        let melee = self.held_melee;
        let sniper = self.weapon.kind == Kind::Sniper;
        let ez = if melee { 0.0 } else { ease(self.zoom) };
        let alpha = (1.0 - ez * 2.5).clamp(0.0, 1.0);

        let moving = if p.on_ground && !p.sliding {
            (p.speed_now / 8.4).min(1.0)
        } else {
            0.0
        };
        let bob = vec2(
            p.bob.sin() * 8.0 * moving * k,
            (p.bob * 2.0).sin().abs() * 9.0 * moving * k,
        );
        let sprintf =
            ((p.speed_now - 6.5) / 2.0).clamp(0.0, 1.0) * if p.on_ground { 1.0 } else { 0.0 };

        // gun: higher up the screen, muzzle ends just right/below the crosshair
        let c = vec2(w * 0.5 + 62.0 * k, h * 0.5 + 80.0 * k);
        let (base, mx) = if melee {
            (vec2(w * 0.74, h * 0.90), 0.0)
        } else if sniper {
            (vec2(w * 0.72 + bob.x, h * 0.88 + bob.y), 370.0)
        } else {
            // penguin rifle: sits exactly where the reference render has it
            (
                vec2(w * ar_model::POSE_X + bob.x, h * ar_model::POSE_Y + bob.y),
                ar_model::MUZZLE_X,
            )
        };
        let target = if melee { vec2(w * 0.52, h * 0.30) } else { c };
        let mut ang = (target.y - base.y).atan2(target.x - base.x);
        if !melee && !sniper {
            ang = ar_model::POSE_ANGLE; // fixed reference hold, not aimed at the crosshair
        }
        let mut pos = base;

        // kick (gun muzzle climbs and slides back)
        let win = self.weapon.fire_interval.min(0.12);
        let kick = if melee {
            0.0
        } else {
            ((self.weapon.cooldown - (self.weapon.fire_interval - win)).max(0.0) / win)
                .clamp(0.0, 1.0)
        };
        ang += 0.07 * kick;

        // reload dip
        if !melee && self.weapon.reloading > 0.0 {
            let dip = (self.weapon.reload_progress() * std::f32::consts::PI).sin();
            pos.y += 190.0 * k * dip;
            ang -= 0.35 * dip;
        }
        // sprint / slide lowering
        pos.y += 22.0 * k * sprintf;
        ang -= 0.10 * sprintf;
        if p.sliding {
            pos.y += 30.0 * k;
            ang -= 0.14;
        }
        // knife slash
        if melee && self.swing_t > 0.0 {
            let q = 1.0 - self.swing_t / 0.25;
            let sw = (q * std::f32::consts::PI).sin();
            pos.x -= 200.0 * k * sw;
            pos.y -= 60.0 * k * sw;
            ang -= 0.7 * sw;
        }
        // draw-in when switching weapons
        let sw = (self.switch_t / 0.28).clamp(0.0, 1.0);
        pos.y += sw * sw * 260.0 * k;

        let ax = vec2(ang.cos(), ang.sin());
        pos -= ax * (22.0 * k * kick);

        GunXf {
            p: pos,
            ax,
            ay: vec2(ang.sin(), -ang.cos()),
            k,
            alpha,
            mx,
        }
    }

    /// Blocky first-person arms with the held weapon.
    fn draw_viewmodel(&self) {
        if !self.player.alive || self.paused {
            return;
        }
        let xf = self.gun_xf();
        if xf.alpha <= 0.01 {
            return;
        }
        if self.held_melee {
            draw_knife(&xf);
        } else if self.weapon.kind == Kind::Sniper {
            draw_sniper(&xf);
        } else {
            draw_ar(&xf);
        }
    }

    fn draw(&mut self) {
        // sky colour, so we can tell whether the 3D pass is drawing at all
        clear_background(Color::new(0.45, 0.62, 0.85, 1.0));

        // guard against NaN camera values (would give a black screen)
        if !self.player.yaw.is_finite() {
            self.player.yaw = 0.0;
        }
        if !self.player.pitch.is_finite() {
            self.player.pitch = 0.0;
        }

        let fov_scoped = match self.weapon.kind {
            Kind::Sniper => FOV_SNIPER,
            Kind::Ar => FOV_AR_ADS,
        };
        let fov: f32 = FOV_NORMAL + (fov_scoped - FOV_NORMAL) * ease(self.zoom);
        let camera = self.player.camera(fov.to_radians());
        set_camera(&camera);

        let cam_pos = self.player.eye_pos();
        let cam_fwd = self.player.aim_dir();

        // triangles first, lines last (fewer GPU batch switches)
        self.map.draw_solid(cam_pos, cam_fwd);

        for bot in &self.bots {
            bot.draw();
        }

        self.map.draw_lines(cam_pos, cam_fwd);

        // bullet trails: a bright streak that travels to the target and fades,
        // with a soft glow built from a few offset lines
        for t in &self.tracers {
            let age = (t.max - t.life).max(0.0);
            let head = (age / (0.45 * t.max)).min(1.0);
            let tail = ((age - 0.15 * t.max) / (0.5 * t.max)).clamp(0.0, 1.0);
            let p0 = t.a.lerp(t.b, tail);
            let p1 = t.a.lerp(t.b, head);
            if (p1 - p0).length() < 0.01 {
                continue;
            }
            let fade = (t.life / t.max).clamp(0.0, 1.0);
            let dir = (t.b - t.a).normalize_or_zero();
            let right = dir.cross(vec3(0.0, 1.0, 0.0)).normalize_or_zero();
            let upv = right.cross(dir);
            for (r, al) in [(0.04f32, 0.10f32), (0.02, 0.22)] {
                let c = Color::new(t.col.r, t.col.g, t.col.b, al * fade);
                for off in [right * r, -right * r, upv * r, -upv * r] {
                    draw_line_3d(p0 + off, p1 + off, c);
                }
            }
            draw_line_3d(p0, p1, Color::new(1.0, 1.0, 0.92, fade));
        }
        for s in &self.sparks {
            let a = (s.life / s.max).clamp(0.0, 1.0);
            let sz = 0.03 + 0.04 * a;
            draw_cube(s.pos, vec3(sz, sz, sz), None, Color::new(s.col.r, s.col.g, s.col.b, a));
        }

        set_default_camera();
        self.draw_viewmodel();
        self.draw_hud();

        // --- on-screen touch controls (drawn last so they overlay everything)
        if !self.paused {
            self.touch.draw(&self.settings);
        }

        if self.paused {
            match self.menu.run(&mut self.settings) {
                MenuAction::Resume => self.resume(),
                MenuAction::TestSound => {
                    self.play(&self.sfx.shoot, 0.9);
                    self.play(&self.sfx.kill, 0.9);
                }
                MenuAction::None => {}
            }
        }
    }

    fn draw_hud(&self) {
        let w = screen_width();
        let h = screen_height();
        let cx = w * 0.5;
        let cy = h * 0.5;

        // ---------- scope overlay: soft vignette + thin ring, fading in with the zoom
        let ez = ease(self.zoom);
        if ez > 0.001 && self.weapon.kind == Kind::Sniper {
            let side = h * VIGNETTE_SPAN;
            draw_texture_ex(
                &self.vignette,
                cx - side * 0.5,
                cy - side * 0.5,
                Color::new(1.0, 1.0, 1.0, ez),
                DrawTextureParams {
                    dest_size: Some(vec2(side, side)),
                    ..Default::default()
                },
            );
            // ultra-wide screens: fill anything the vignette square doesn't reach
            if w > side {
                let edge = Color::new(0.0, 0.0, 0.0, 0.85 * ez);
                let x0 = cx - side * 0.5;
                draw_rectangle(0.0, 0.0, x0, h, edge);
                draw_rectangle(cx + side * 0.5, 0.0, w - (cx + side * 0.5), h, edge);
            }

            draw_circle_lines(cx, cy, h * 0.42, 2.0, Color::new(0.0, 0.0, 0.0, 0.7 * ez));
            draw_circle(cx, cy, 1.8, Color::new(0.9, 0.1, 0.1, ez));
        }

        // ---------- AR iron sights (no scope overlay)
        if self.weapon.kind == Kind::Ar && !self.held_melee && ez > 0.01 {
            draw_iron_sights(w, h, ez);
        }

        // ---------- dynamic crosshair (fades out as the scope fades in)
        if ez < 0.999 {
            let gap = 5.0 + self.weapon.bloom * 900.0;
            let len = 9.0;
            let c = Color::new(0.1, 1.0, 0.6, 0.9 * (1.0 - ez));
            draw_line(cx - gap - len, cy, cx - gap, cy, 2.0, c);
            draw_line(cx + gap, cy, cx + gap + len, cy, 2.0, c);
            draw_line(cx, cy - gap - len, cx, cy - gap, 2.0, c);
            draw_line(cx, cy + gap, cx, cy + gap + len, 2.0, c);
            draw_circle(cx, cy, 1.5, c);
        }

        // ---------- muzzle flash (screen centre = exactly on the crosshair)
        if self.muzzle > 0.0 && ez < 0.5 && self.player.alive && !self.held_melee {
            let xf = self.gun_xf();
            let m = xf.pt(xf.mx, 0.0);
            let (cx, cy) = (m.x, m.y);
            let k = (self.muzzle / 0.06).clamp(0.0, 1.0);
            let outer = Color::new(1.0, 0.75, 0.2, 0.9 * k);
            let core = Color::new(1.0, 0.95, 0.7, k);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0 + 0.2;
                let (r0, r1) = if i % 2 == 0 { (6.0, 24.0) } else { (6.0, 15.0) };
                draw_line(
                    cx + a.cos() * r0,
                    cy + a.sin() * r0,
                    cx + a.cos() * r1,
                    cy + a.sin() * r1,
                    3.0,
                    outer,
                );
            }
            draw_circle(cx, cy, 7.0, outer);
            draw_circle(cx, cy, 4.0, core);
        }

        // ---------- hitmarker
        if self.hitmarker > 0.0 {
            let a = (self.hitmarker / 0.15).min(1.0);
            let c = Color::new(1.0, 1.0, 1.0, a);
            let g = 6.0;
            let l = 9.0;
            draw_line(cx - g - l, cy - g - l, cx - g, cy - g, 2.0, c);
            draw_line(cx + g, cy - g, cx + g + l, cy - g - l, 2.0, c);
            draw_line(cx - g - l, cy + g + l, cx - g, cy + g, 2.0, c);
            draw_line(cx + g, cy + g, cx + g + l, cy + g + l, 2.0, c);
        }

        // ---------- damage flash
        if self.damage_flash > 0.0 {
            let a = (self.damage_flash / 0.35) * 0.45;
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.85, 0.0, 0.0, a));
        }

        // ---------- health
        let hp = (self.player.health / 100.0).clamp(0.0, 1.0);
        draw_rectangle(40.0, h - 74.0, 262.0, 24.0, Color::new(0.0, 0.0, 0.0, 0.55));
        draw_rectangle(
            41.0,
            h - 73.0,
            260.0 * hp,
            22.0,
            Color::new(0.2, 0.9, 0.4, 0.9),
        );
        draw_text(
            &format!("{}", self.player.health.ceil() as i32),
            52.0,
            h - 56.0,
            24.0,
            WHITE,
        );

        // ---------- ammo
        let ammo = if self.held_melee {
            "KNIFE".to_string()
        } else {
            format!("{} / {}", self.weapon.mag, self.weapon.reserve)
        };
        let ts = measure_text(&ammo, None, 34, 1.0);
        draw_text(&ammo, w - 50.0 - ts.width, h - 56.0, 34.0, WHITE);

        // ---------- reload bar
        if self.weapon.reloading > 0.0 && !self.held_melee {
            let p = self.weapon.reload_progress();
            draw_text(
                "RELOADING",
                cx - 55.0,
                cy + 44.0,
                20.0,
                Color::new(0.3, 0.9, 1.0, 1.0),
            );
            draw_rectangle(cx - 60.0, cy + 52.0, 120.0, 7.0, Color::new(0.0, 0.0, 0.0, 0.6));
            draw_rectangle(
                cx - 60.0,
                cy + 52.0,
                120.0 * p,
                7.0,
                Color::new(0.3, 0.9, 1.0, 1.0),
            );
        }

        // ---------- score
        draw_text(&format!("KILLS  {}", self.score), 40.0, 52.0, 30.0, WHITE);
        draw_text(
            &format!("DEATHS {}", self.deaths),
            40.0,
            86.0,
            26.0,
            Color::new(1.0, 0.5, 0.5, 1.0),
        );
        draw_text(
            &format!("FPS {}", get_fps()),
            w - 130.0,
            40.0,
            22.0,
            Color::new(1.0, 1.0, 1.0, 0.6),
        );

        // ---------- state hints
        let mut y = 130.0;
        let mut hints: Vec<&str> = Vec::new();
        if self.player.sliding {
            hints.push("SLIDING");
        } else if self.player.crouching {
            hints.push("SNEAKING");
        }
        if self.player.scoped {
            hints.push(if self.weapon.kind == Kind::Sniper { "SCOPED" } else { "ADS" });
        }
        for hint in hints {
            draw_text(hint, 40.0, y, 22.0, Color::new(0.5, 1.0, 0.9, 0.9));
            y += 26.0;
        }

        // ---------- weapon slots (tap to change the weapon you hold)
        let slots = weapon_slot_rects(w, h);
        let labels = [self.weapon.kind.name(), "KNIFE"];
        for i in 0..2 {
            let active = (i == 1) == self.held_melee;
            let col = if active {
                Color::new(0.25, 0.7, 0.4, 1.0)
            } else {
                Color::new(0.35, 0.4, 0.5, 1.0)
            };
            TouchControls::draw_button(slots[i], labels[i], active, col, self.settings.hud_opacity.max(0.6));
        }

        // ---------- dead overlay
        if !self.player.alive {
            draw_rectangle(0.0, 0.0, w, h, Color::new(0.35, 0.0, 0.0, 0.45));
            let t = "YOU DIED";
            let ts = measure_text(t, None, 64, 1.0);
            draw_text(t, cx - ts.width * 0.5, cy + 10.0, 64.0, WHITE);
        }

    }
}
