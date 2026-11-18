// src/main.rs
mod bots;
mod map;
mod mobile;
mod player;
mod settings;
mod sfx;
mod weapon;

use macroquad::audio::{play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

use map::{ray_aabb, spread_dir, Map};
use mobile::{menu_button_rect, TouchControls, MOBILE_AIM_SCALE};
use player::Player;
use settings::{Menu, MenuAction, Settings};
use sfx::Sfx;
use weapon::Weapon;

struct Tracer {
    a: Vec3,
    b: Vec3,
    life: f32,
}

struct Game {
    map: Map,
    player: Player,
    weapon: Weapon,
    bots: Vec<bots::Bot>,
    tracers: Vec<Tracer>,
    sfx: Sfx,
    hitmarker: f32,
    damage_flash: f32,
    muzzle: f32,
    /// 0 = not scoped, 1 = fully scoped (moves over ~0.3s for a smooth zoom)
    zoom: f32,
    vignette: Texture2D,
    score: u32,
    deaths: u32,
    paused: bool,
    touch: TouchControls,
    settings: Settings,
    menu: Menu,
}

/// Scope zoom length in seconds.
const ZOOM_TIME: f32 = 0.3;
const FOV_NORMAL: f32 = 78.0;
const FOV_SCOPED: f32 = 26.0;
/// The vignette quad is drawn this many screen-heights wide.
const VIGNETTE_SPAN: f32 = 2.6;

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
            weapon: Weapon::new(),
            bots,
            tracers: Vec::new(),
            sfx,
            hitmarker: 0.0,
            damage_flash: 0.0,
            muzzle: 0.0,
            zoom: 0.0,
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
        self.damage_flash = (self.damage_flash - dt).max(0.0);
        self.muzzle = (self.muzzle - dt).max(0.0);
        for t in self.tracers.iter_mut() {
            t.life -= dt;
        }
        self.tracers.retain(|t| t.life > 0.0);

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
        let ez = ease(self.zoom);
        let sens = self.settings.sens_camera
            + (self.settings.sens_scope - self.settings.sens_camera) * ez;
        self.player.look(dx * sens, dy * sens);

        // --- scope
        self.player.scoped = (is_mouse_button_down(MouseButton::Right)
            || self.player.mobile_scope)
            && self.player.alive
            && !self.player.sliding;

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

        if self.player.alive {
            // reload (keyboard R OR mobile reload button)
            if is_key_pressed(KeyCode::R) || tf.reload_pressed {
                let was = self.weapon.reloading;
                self.weapon.start_reload();
                if was <= 0.0 && self.weapon.reloading > 0.0 {
                    self.play(&self.sfx.reload, 0.5);
                }
            }

            // dry-fire auto reload
            if (is_mouse_button_pressed(MouseButton::Left) || tf.fire_just_pressed)
                && self.weapon.mag <= 0
                && self.weapon.reloading <= 0.0
            {
                self.play(&self.sfx.empty, 0.4);
                self.weapon.start_reload();
            }

            // fire
            if (is_mouse_button_down(MouseButton::Left) || self.player.mobile_fire)
                && self.weapon.can_fire()
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
            self.tracers.push(Tracer { a: s.from, b: s.to, life: 0.07 });
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
        self.weapon.fire();
        self.muzzle = 0.06;

        self.player.recoil_pitch += 0.013 + macroquad::rand::gen_range(0.0f32, 0.008);
        self.player.recoil_yaw += macroquad::rand::gen_range(-0.01f32, 0.01);

        self.play(&self.sfx.shoot, 0.5);

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
        self.tracers.push(Tracer { a: eye + d * 0.5, b: end, life: 0.08 });

        if let Some(i) = best_bot {
            self.hitmarker = 0.15;
            self.play(&self.sfx.hit, 0.35);

            let mut killed = false;
            {
                let bot = &mut self.bots[i];
                bot.health -= 34.0;
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

        let fov: f32 = FOV_NORMAL + (FOV_SCOPED - FOV_NORMAL) * ease(self.zoom);
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

        for t in &self.tracers {
            draw_line_3d(t.a, t.b, Color::new(1.0, 0.9, 0.5, 0.9));
        }

        set_default_camera();
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
        if ez > 0.001 {
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
        if self.muzzle > 0.0 && ez < 0.5 {
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
        let ammo = format!("{} / {}", self.weapon.mag, self.weapon.reserve);
        let ts = measure_text(&ammo, None, 34, 1.0);
        draw_text(&ammo, w - 50.0 - ts.width, h - 56.0, 34.0, WHITE);

        // ---------- reload bar
        if self.weapon.reloading > 0.0 {
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
            hints.push("SCOPED");
        }
        for hint in hints {
            draw_text(hint, 40.0, y, 22.0, Color::new(0.5, 1.0, 0.9, 0.9));
            y += 26.0;
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
