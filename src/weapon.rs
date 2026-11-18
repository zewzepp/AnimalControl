// src/weapon.rs
pub struct Weapon {
    pub mag: i32,
    pub reserve: i32,
    pub mag_size: i32,
    pub cooldown: f32,
    pub fire_interval: f32,
    pub reloading: f32,
    pub reload_time: f32,
    pub bloom: f32,
}

impl Weapon {
    pub fn new() -> Self {
        Self {
            mag: 30,
            reserve: 150,
            mag_size: 30,
            cooldown: 0.0,
            fire_interval: 0.095,
            reloading: 0.0,
            reload_time: 1.9,
            bloom: 0.0,
        }
    }

    pub fn can_fire(&self) -> bool {
        self.mag > 0 && self.cooldown <= 0.0 && self.reloading <= 0.0
    }

    pub fn fire(&mut self) {
        self.mag -= 1;
        self.cooldown = self.fire_interval;
        self.bloom = (self.bloom + 0.004).min(0.025);
    }

    pub fn start_reload(&mut self) {
        if self.reloading > 0.0 || self.mag == self.mag_size || self.reserve <= 0 {
            return;
        }
        self.reloading = self.reload_time;
    }

    pub fn update(&mut self, dt: f32) {
        self.cooldown -= dt;
        self.bloom = (self.bloom - dt * 0.12).max(0.0);

        if self.reloading > 0.0 {
            self.reloading -= dt;
            if self.reloading <= 0.0 {
                self.reloading = 0.0;
                let need = self.mag_size - self.mag;
                let take = need.min(self.reserve);
                self.mag += take;
                self.reserve -= take;
            }
        }
    }

    pub fn spread(&self, scoped: bool, sliding: bool) -> f32 {
        if scoped {
            return 0.0;
        }
        let mut s = 0.0015 + self.bloom;
        if sliding {
            s += 0.008;
        }
        s
    }

    pub fn reload_progress(&self) -> f32 {
        if self.reloading <= 0.0 {
            1.0
        } else {
            1.0 - self.reloading / self.reload_time
        }
    }
}
