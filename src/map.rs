// src/map.rs
use macroquad::prelude::*;

#[derive(Clone)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

pub struct Map {
    pub boxes: Vec<Aabb>,
    pub spawns: Vec<Vec3>,
}

fn push_box(boxes: &mut Vec<Aabb>, cx: f32, cz: f32, sx: f32, sz: f32, h: f32) {
    boxes.push(Aabb {
        min: vec3(cx - sx * 0.5, 0.0, cz - sz * 0.5),
        max: vec3(cx + sx * 0.5, h, cz + sz * 0.5),
    });
}

impl Map {
    pub fn arena() -> Self {
        let mut boxes = Vec::new();

        // arena boundary
        push_box(&mut boxes, 0.0, -22.0, 45.0, 1.0, 4.0);
        push_box(&mut boxes, 0.0, 22.0, 45.0, 1.0, 4.0);
        push_box(&mut boxes, -22.0, 0.0, 1.0, 45.0, 4.0);
        push_box(&mut boxes, 22.0, 0.0, 1.0, 45.0, 4.0);

        // cover
        push_box(&mut boxes, 0.0, -6.0, 5.0, 1.2, 1.6);
        push_box(&mut boxes, -9.0, 3.0, 1.2, 12.0, 2.6);
        push_box(&mut boxes, 9.0, -3.0, 1.2, 12.0, 2.6);
        push_box(&mut boxes, -13.0, -13.0, 4.0, 4.0, 2.0);
        push_box(&mut boxes, 13.0, 12.0, 4.0, 4.0, 2.0);
        push_box(&mut boxes, 0.0, 15.0, 3.0, 3.0, 3.0);
        push_box(&mut boxes, -16.0, 14.0, 6.0, 1.2, 1.4);
        push_box(&mut boxes, 16.0, -14.0, 6.0, 1.2, 1.4);
        push_box(&mut boxes, 6.0, 8.0, 3.0, 1.0, 1.2);
        push_box(&mut boxes, -6.0, -10.0, 3.0, 1.0, 1.2);
        push_box(&mut boxes, -17.0, -3.0, 1.2, 5.0, 1.8);
        push_box(&mut boxes, 17.0, 4.0, 1.2, 5.0, 1.8);

        let spawns = vec![
            vec3(0.0, 0.0, 18.0),
            vec3(-18.0, 0.0, -18.0),
            vec3(18.0, 0.0, -18.0),
            vec3(-18.0, 0.0, 18.0),
            vec3(18.0, 0.0, 18.0),
            vec3(0.0, 0.0, -18.0),
            vec3(-18.0, 0.0, 0.0),
            vec3(18.0, 0.0, 0.0),
        ];

        Self { boxes, spawns }
    }

    pub fn draw(&self) {
        // floor slab
        draw_cube(
            vec3(0.0, -0.5, 0.0),
            vec3(45.0, 1.0, 45.0),
            None,
            Color::new(0.13, 0.14, 0.17, 1.0),
        );

        // grid
        let grid = Color::new(0.20, 0.22, 0.27, 1.0);
        for i in -22..=22 {
            let f = i as f32;
            draw_line_3d(vec3(f, 0.01, -22.0), vec3(f, 0.01, 22.0), grid);
            draw_line_3d(vec3(-22.0, 0.01, f), vec3(22.0, 0.01, f), grid);
        }

        for b in &self.boxes {
            let center = (b.min + b.max) * 0.5;
            let size = b.max - b.min;
            let col = if b.max.y > 2.5 {
                Color::new(0.28, 0.30, 0.36, 1.0)
            } else {
                Color::new(0.37, 0.39, 0.46, 1.0)
            };
            draw_cube(center, size, None, col);
            draw_cube_wires(center, size * 1.002, Color::new(0.05, 0.06, 0.08, 1.0));
        }
    }
}

/// Slab-method ray vs AABB. Returns entry distance.
pub fn ray_aabb(origin: Vec3, dir: Vec3, b: &Aabb) -> Option<f32> {
    let mut tmin = 0.0f32;
    let mut tmax = f32::INFINITY;

    for i in 0..3 {
        let o = origin[i];
        let d = dir[i];
        let mn = b.min[i];
        let mx = b.max[i];

        if d.abs() < 1e-8 {
            if o < mn || o > mx {
                return None;
            }
        } else {
            let inv = 1.0 / d;
            let mut t1 = (mn - o) * inv;
            let mut t2 = (mx - o) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            if t1 > tmin {
                tmin = t1;
            }
            if t2 < tmax {
                tmax = t2;
            }
            if tmin > tmax {
                return None;
            }
        }
    }
    Some(tmin)
}

pub fn los_clear(a: Vec3, b: Vec3, map: &Map) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 1e-4 {
        return true;
    }
    let dir = d / len;
    for bb in &map.boxes {
        if let Some(t) = ray_aabb(a, dir, bb) {
            if t > 0.0 && t < len - 0.05 {
                return false;
            }
        }
    }
    true
}

/// Random cone spread around a direction.
pub fn spread_dir(dir: Vec3, amount: f32) -> Vec3 {
    if amount <= 0.0 {
        return dir;
    }
    let up = if dir.y.abs() > 0.98 {
        vec3(1.0, 0.0, 0.0)
    } else {
        vec3(0.0, 1.0, 0.0)
    };
    let right = dir.cross(up).normalize();
    let up2 = right.cross(dir).normalize();

    let a = macroquad::rand::gen_range(0.0f32, std::f32::consts::TAU);
    let r = macroquad::rand::gen_range(0.0f32, 1.0).sqrt() * amount;
    (dir + right * (a.cos() * r) + up2 * (a.sin() * r)).normalize()
}

/// Moves a vertical capsule (approximated by a circle + height) and resolves
/// collisions against the map. Returns whether the entity is grounded.
pub fn move_circle(
    pos: &mut Vec3,
    vel: &mut Vec3,
    dt: f32,
    map: &Map,
    radius: f32,
    height: f32,
) -> bool {
    // ---- horizontal sweep
    pos.x += vel.x * dt;
    pos.z += vel.z * dt;

    for b in &map.boxes {
        // skip boxes entirely below or entirely above the body
        if pos.y + height <= b.min.y + 0.05 {
            continue;
        }
        if pos.y >= b.max.y - 0.05 {
            continue;
        }

        let cx = pos.x.clamp(b.min.x, b.max.x);
        let cz = pos.z.clamp(b.min.z, b.max.z);
        let dx = pos.x - cx;
        let dz = pos.z - cz;
        let d2 = dx * dx + dz * dz;

        if d2 >= radius * radius {
            continue;
        }

        if d2 > 1e-8 {
            let d = d2.sqrt();
            let nx = dx / d;
            let nz = dz / d;
            let push = radius - d;
            pos.x += nx * push;
            pos.z += nz * push;

            let vn = vel.x * nx + vel.z * nz;
            if vn < 0.0 {
                vel.x -= vn * nx;
                vel.z -= vn * nz;
            }
        } else {
            // center is inside the box: eject along the shallowest axis
            let left = pos.x - b.min.x;
            let right = b.max.x - pos.x;
            let back = pos.z - b.min.z;
            let front = b.max.z - pos.z;
            let m = left.min(right).min(back).min(front);

            if m == left {
                pos.x = b.min.x - radius;
            } else if m == right {
                pos.x = b.max.x + radius;
            } else if m == back {
                pos.z = b.min.z - radius;
            } else {
                pos.z = b.max.z + radius;
            }
        }
    }

    // ---- vertical sweep
    pos.y += vel.y * dt;

    let mut ground = 0.0f32;
    for b in &map.boxes {
        let cx = pos.x.clamp(b.min.x, b.max.x);
        let cz = pos.z.clamp(b.min.z, b.max.z);
        let dx = pos.x - cx;
        let dz = pos.z - cz;
        if dx * dx + dz * dz < radius * radius {
            if b.max.y <= pos.y + 0.35 && b.max.y > ground {
                ground = b.max.y;
            }
        }
    }

    let mut grounded = false;
    if pos.y <= ground + 1e-4 {
        pos.y = ground;
        if vel.y < 0.0 {
            vel.y = 0.0;
        }
        grounded = true;
    }

    grounded
}
