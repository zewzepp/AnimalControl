// src/sfx.rs
// All sounds are synthesized at startup into WAV byte buffers, so the project
// needs zero binary assets.

use macroquad::audio::{load_sound_from_bytes, Sound};

const SR: f32 = 44100.0;
const TAU: f32 = std::f32::consts::TAU;

pub struct Sfx {
    pub shoot: Sound,
    pub shoot_enemy: Sound,
    pub reload: Sound,
    pub hit: Sound,
    pub kill: Sound,
    pub jump: Sound,
    pub slide: Sound,
    pub empty: Sound,
}

impl Sfx {
    pub async fn load() -> Self {
        let shoot = load_sound_from_bytes(&wav(&gen_shot(1.0, 30.0)))
            .await
            .expect("shot wav");
        let shoot_enemy = load_sound_from_bytes(&wav(&gen_shot(0.62, 42.0)))
            .await
            .expect("enemy shot wav");
        let reload = load_sound_from_bytes(&wav(&gen_reload())).await.expect("reload wav");
        let hit = load_sound_from_bytes(&wav(&gen_blip(1500.0, 0.06, 60.0)))
            .await
            .expect("hit wav");
        let kill = load_sound_from_bytes(&wav(&gen_kill())).await.expect("kill wav");
        let jump = load_sound_from_bytes(&wav(&gen_sweep(280.0, 620.0, 0.14)))
            .await
            .expect("jump wav");
        let slide = load_sound_from_bytes(&wav(&gen_slide())).await.expect("slide wav");
        let empty = load_sound_from_bytes(&wav(&gen_blip(900.0, 0.035, 110.0)))
            .await
            .expect("empty wav");

        Self {
            shoot,
            shoot_enemy,
            reload,
            hit,
            kill,
            jump,
            slide,
            empty,
        }
    }
}

// ---------------------------------------------------------------- wav encoder

fn wav(samples: &[f32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        data.extend_from_slice(&v.to_le_bytes());
    }

    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&(SR as u32).to_le_bytes());
    out.extend_from_slice(&((SR as u32) * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

// ---------------------------------------------------------------- tiny rng

struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Rng(seed | 1)
    }
    fn noise(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32) / (u32::MAX as f32) * 2.0 - 1.0
    }
}

// ---------------------------------------------------------------- generators

fn gen_shot(pitch: f32, decay: f32) -> Vec<f32> {
    let n = (SR * 0.24) as usize;
    let mut out = Vec::with_capacity(n);
    let mut rng = Rng::new(0x1234_5678);
    let mut lp = 0.0f32;

    for i in 0..n {
        let t = i as f32 / SR;

        let noise = rng.noise();
        lp += (noise - lp) * 0.35;

        let body_env = (-t * decay).exp();
        let crack = noise * (-t * 95.0).exp() * 0.45;
        let thump = (t * 62.0 * pitch * TAU).sin() * (-t * 22.0).exp() * 0.7;

        let s = (lp * body_env * 1.4 + crack + thump) * 0.72;
        out.push(s.clamp(-1.0, 1.0));
    }
    out
}

fn gen_reload() -> Vec<f32> {
    let n = (SR * 0.36) as usize;
    let mut out = vec![0.0f32; n];
    let mut rng = Rng::new(0x0BAD_F00D);

    let clicks = [(0.0f32, 0.06f32), (0.15, 0.05), (0.27, 0.08)];
    for (start, dur) in clicks {
        let s0 = (start * SR) as usize;
        let len = (dur * SR) as usize;
        for i in 0..len {
            if s0 + i >= n {
                break;
            }
            let t = i as f32 / SR;
            let env = (-t * 60.0).exp();
            let noise = rng.noise();
            let tone = (t * 1800.0 * TAU).sin() * 0.35;
            out[s0 + i] += (noise * 0.6 + tone) * env * 0.55;
        }
    }
    out
}

fn gen_blip(freq: f32, dur: f32, decay: f32) -> Vec<f32> {
    let n = (SR * dur) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SR;
        let env = (-t * decay).exp();
        out.push((t * freq * TAU).sin() * env * 0.5);
    }
    out
}

fn gen_kill() -> Vec<f32> {
    let n = (SR * 0.22) as usize;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / SR;
        let f = 700.0 + t * 2600.0;
        let env = (-t * 14.0).exp();
        let a = (t * f * TAU).sin();
        let b = (t * f * 1.5 * TAU).sin() * 0.4;
        out.push((a + b) * env * 0.42);
    }
    out
}

fn gen_sweep(f0: f32, f1: f32, dur: f32) -> Vec<f32> {
    let n = (SR * dur) as usize;
    let mut out = Vec::with_capacity(n);
    let mut phase = 0.0f32;
    for i in 0..n {
        let t = i as f32 / SR;
        let k = t / dur;
        let f = f0 + (f1 - f0) * k;
        phase += f * TAU / SR;
        let env = (k * std::f32::consts::PI).sin();
        out.push(phase.sin() * env * 0.35);
    }
    out
}

fn gen_slide() -> Vec<f32> {
    let n = (SR * 0.3) as usize;
    let mut out = Vec::with_capacity(n);
    let mut rng = Rng::new(0xDEAD_BEEF);
    let mut lp = 0.0f32;

    for i in 0..n {
        let t = i as f32 / SR;
        let k = t / 0.3;
        let noise = rng.noise();
        lp += (noise - lp) * 0.08;
        let env = (k * std::f32::consts::PI).sin();
        out.push(lp * env * 1.6);
    }
    out
}
