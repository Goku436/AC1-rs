//! AC1's camera shake on a heavy landing: `FX_Camera_Shake_Assassins_Fall` (`HumanFXPack_AlTair` +88).
//!
//! `HumanInAir__SetupToGround_Landing` (0xE05940) starts the FX on a landing from over 3 m, its intensity parameter
//! (0xBE93B29C) `min((drop - 3) / 7, 1)`. The FX's `FXCameraOperator` drives the camera's six channels; in the data five
//! have no amplitude and the pitch is `intensity * envelope(t) * 30 * noise(t, 20 * (200 + 200 t))` degrees over one
//! second (its tracks are 120 ticks of 1/120 s): the envelope 1.5 falling to 0.1 at 0.4 s and to 0 at 1 s, the noise a
//! one-octave fractional Brownian motion of 2D Perlin noise. The second coordinate runs 4000 to 8000 a second, so the
//! noise is new each frame: a jolt that dies away. Read from the FX object's tracks, as Banned445's AC1-Movement-Rewritten
//! (MIT) first did; the noise here is our own gradient noise, not AC1's tables.
//!
//! The other camera shakes in Altaïr's FX pack are combat ones (a punch given or taken, sword hits), not played here.

/// The landing drop (m) over which the camera shakes.
pub const SHAKE_DROP: f32 = 3.0;
/// The drop past `SHAKE_DROP` (m) that shakes it fully.
const SHAKE_RANGE: f32 = 7.0;
/// The FX's length (s).
const SHAKE_TIME: f32 = 1.0;
/// Its amplitude track (degrees).
const SHAKE_DEGREES: f32 = 30.0;

/// A landing's shake, as it plays out.
#[derive(Clone, Copy, Debug, Default)]
pub struct FallShake {
    intensity: f32,
    t: f32,
}

impl FallShake {
    /// Start the shake for a landing from `drop` m (none from `SHAKE_DROP` or less).
    pub fn land(&mut self, drop: f32) {
        if drop > SHAKE_DROP {
            self.intensity = ((drop - SHAKE_DROP) / SHAKE_RANGE).clamp(0.0, 1.0);
            self.t = 0.0;
        }
    }

    /// Advance by `dt` and give the camera's pitch now (degrees; 0 once it has played out).
    pub fn step(&mut self, dt: f32) -> f32 {
        if self.intensity <= 0.0 {
            return 0.0;
        }
        self.t += dt;
        if self.t > SHAKE_TIME {
            self.intensity = 0.0;
            return 0.0;
        }
        self.intensity * envelope(self.t) * SHAKE_DEGREES * noise2(self.t, 20.0 * (200.0 + 200.0 * self.t))
    }
}

/// The FX's envelope track: 1.5 at the start, 0.1 at 0.4 s, 0 at 1 s, straight between.
fn envelope(t: f32) -> f32 {
    if t < 0.4 { 1.5 + (0.1 - 1.5) * t / 0.4 } else { (0.1 * (1.0 - (t - 0.4) / 0.6)).max(0.0) }
}

/// 2D gradient (Perlin) noise, about -0.7..0.7, 0 on the lattice: gradients from a hash of the cell.
fn noise2(x: f32, y: f32) -> f32 {
    let (ix, iy) = (x.floor(), y.floor());
    let (fx, fy) = (x - ix, y - iy);
    let grad = |cx: f32, cy: f32, dx: f32, dy: f32| {
        let mut h = (cx as i32 as u32).wrapping_mul(0x9E37_79B1) ^ (cy as i32 as u32).wrapping_mul(0x85EB_CA77);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2C1B_3C6D);
        h ^= h >> 12;
        let a = (h & 0xFFFF) as f32 / 65536.0 * std::f32::consts::TAU;
        a.cos() * dx + a.sin() * dy
    };
    let s = |f: f32| f * f * (3.0 - 2.0 * f);
    let (sx, sy) = (s(fx), s(fy));
    let lo = grad(ix, iy, fx, fy) + (grad(ix + 1.0, iy, fx - 1.0, fy) - grad(ix, iy, fx, fy)) * sx;
    let hi = grad(ix, iy + 1.0, fx, fy - 1.0) + (grad(ix + 1.0, iy + 1.0, fx - 1.0, fy - 1.0) - grad(ix, iy + 1.0, fx, fy - 1.0)) * sx;
    lo + (hi - lo) * sy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_landings_over_3_m_shake_by_the_drop() {
        let mut s = FallShake::default();
        s.land(3.0);
        assert_eq!(s.step(1.0 / 60.0), 0.0);
        s.land(6.5);
        assert!((s.intensity - 0.5).abs() < 1e-6);
        s.land(25.0);
        assert_eq!(s.intensity, 1.0);
    }

    #[test]
    fn the_shake_dies_away_within_a_second() {
        let mut s = FallShake::default();
        s.land(10.0);
        let early: f32 = (0..12).map(|_| s.step(1.0 / 60.0).abs()).fold(0.0, f32::max);
        for _ in 12..54 {
            s.step(1.0 / 60.0);
        }
        let late: f32 = (54..59).map(|_| s.step(1.0 / 60.0).abs()).fold(0.0, f32::max);
        assert!(early > 2.0 && early <= 45.0, "a jolt at first: {early}");
        assert!(late < 1.0, "almost still at the end: {late}");
        for _ in 0..10 {
            s.step(1.0 / 60.0);
        }
        assert_eq!(s.step(1.0 / 60.0), 0.0);
    }

    #[test]
    fn envelope_and_noise_follow_the_fx() {
        assert!((envelope(0.0) - 1.5).abs() < 1e-6);
        assert!((envelope(0.4) - 0.1).abs() < 1e-6);
        assert!(envelope(1.0).abs() < 1e-6);
        assert_eq!(noise2(3.0, 7.0), 0.0);
        assert!((0..1000).map(|i| noise2(i as f32 * 0.37, i as f32 * 0.91).abs()).all(|v| v <= 1.0));
    }
}
