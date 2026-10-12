//! AC1's wind (`Wind`, one per city, read by `forge::cloth::wind`): a force along the wind's direction, its
//! direction and strength wandering with 1D gradient noise over time and place. It moves the robe and the bones
//! the skeleton's hinges and springs swing.
//!
//! The noise is AC1's: a 256-entry permutation and gradient table filled at start-up from Knuth's subtractive
//! random generator (the Numerical Recipes `ran3`, seeds 84 and 42), smoothstep-blended gradients at whole numbers.
//! How the wind samples it is as Banned445's AC1-Movement-Rewritten (MIT, `src/wind.rs`) recreated it (checked
//! there against a live sample of the running game); written fresh here.

use glam::Vec3;

/// Knuth's subtractive generator (modulus 10⁹).
struct Subtractive {
    table: [i32; 56],
    next: usize,
    nextp: usize,
}

impl Subtractive {
    const BIG: i32 = 1_000_000_000;

    fn new(seed: i32) -> Self {
        let mut table = [0; 56];
        let mut j = (161_803_398 - seed.abs()).abs() % Self::BIG;
        table[55] = j;
        let mut k = 1;
        for i in 1..55 {
            let at = 21 * i % 55;
            table[at] = k;
            k = j - k;
            if k < 0 {
                k += Self::BIG;
            }
            j = table[at];
        }
        for _ in 0..4 {
            for i in 1..56 {
                table[i] -= table[1 + (i + 30) % 55];
                if table[i] < 0 {
                    table[i] += Self::BIG;
                }
            }
        }
        Subtractive { table, next: 0, nextp: 31 }
    }

    /// A draw in [0, 1), rounded as AC1 does (through f32).
    fn draw(&mut self) -> f32 {
        self.next = if self.next == 55 { 1 } else { self.next + 1 };
        self.nextp = if self.nextp == 55 { 1 } else { self.nextp + 1 };
        let mut j = self.table[self.next] - self.table[self.nextp];
        if j < 0 {
            j += Self::BIG;
        }
        self.table[self.next] = j;
        (j as f32 as f64 * 1e-9f32 as f64) as f32
    }
}

/// The noise tables.
pub struct Noise {
    perm: [u8; 256],
    grad: [f32; 256],
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

impl Noise {
    pub fn new() -> Self {
        let mut perm: [u8; 256] = std::array::from_fn(|i| i as u8);
        let mut r = Subtractive::new(84);
        for v in 0..256 {
            let k = ((r.draw() as f64 * 256.0) as f32) as i32 as u8 as usize;
            perm[v] = perm[k];
            perm[k] = v as u8;
        }
        let mut r = Subtractive::new(42);
        let grad = std::array::from_fn(|_| (r.draw() as f64 * 2.0 - 1.0) as f32);
        Noise { perm, grad }
    }

    /// 1D gradient noise: 0 at whole numbers, smooth between.
    pub fn at(&self, x: f32) -> f32 {
        let x = (x as f64 + 4096.0) as f32;
        let i = x as i32;
        let f = (x as f64 - i as f32 as f64) as f32;
        let g = |i: i32| self.grad[self.perm[(i & 255) as usize] as usize];
        let (a, b) = (g(i) * f, g(i + 1) * (f - 1.0));
        a + (b - a) * ((3.0 - 2.0 * f) * f * f)
    }
}

/// One wind (see `forge::cloth::Wind`), in the game's world frame (Z up).
#[derive(Debug, Clone, Copy)]
pub struct Wind {
    pub strength: f32,
    pub noise: bool,
    /// Noise percentages: direction, direction speed, magnitude floor, magnitude speed.
    pub percent: [f32; 4],
    pub direction: Vec3,
}

impl Wind {
    /// Its force at `p` (game world) `t` seconds after it was made.
    pub fn force(&self, noise: &Noise, p: Vec3, t: f32) -> Vec3 {
        let mut d = self.direction;
        if d.abs().max_element() <= 0.001 {
            return Vec3::ZERO;
        }
        if self.noise {
            let spread = 3.0 * self.percent[0] / 100.0;
            let s = 2.0 * self.percent[1] / 100.0 * t;
            d = (d + Vec3::new(noise.at(s + p.x), noise.at(s + p.y), noise.at(s + p.z)) * spread).normalize_or_zero();
            d *= noise.at(6.0 * self.percent[3] / 100.0 * t + (p.x + p.y + p.z)).abs() + self.percent[2] / 100.0;
        }
        d * self.strength
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_zero_at_whole_numbers_and_smooth() {
        let n = Noise::new();
        for i in -5..5 {
            assert!(n.at(i as f32).abs() < 1e-6);
        }
        assert!((n.at(3.4999) - n.at(3.5001)).abs() < 1e-3);
        assert!(n.grad.iter().all(|g| (-1.0..1.0).contains(g)));
    }

    #[test]
    fn a_wind_without_noise_blows_along_its_direction() {
        let w = Wind { strength: 10.0, noise: false, percent: [0.0; 4], direction: Vec3::new(0.6, -0.8, 0.0) };
        assert!(w.force(&Noise::new(), Vec3::ZERO, 3.0).distance(Vec3::new(6.0, -8.0, 0.0)) < 1e-5);
    }

    /// Damascus's wind at the robe's entity, t = 217.3512 s: the force the running game had stored there (Banned445's
    /// measurement).
    #[test]
    fn damascus_wind_matches_the_running_game() {
        let w = Wind {
            strength: 10.0,
            noise: true,
            percent: [61.37931, 100.0, 33.10345, 11.724138],
            direction: Vec3::new(0.852_048_7, -0.523_448_3, 0.0).normalize(),
        };
        let f = w.force(&Noise::new(), Vec3::new(54.472, 7.846, 11.452), 217.3512);
        assert!(f.distance(Vec3::new(3.2603, -4.2908, -0.4926)) < 5e-3, "{f}");
    }
}
