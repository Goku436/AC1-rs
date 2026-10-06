//! Character-level IK built on the solvers: feet on uneven ground, hands on ledges.
//!
//! Model space is Z-up. Ground and surface queries are supplied by the caller in model space.

use crate::{Pose, Rig, two_bone_ik};
use glam::{Quat, Vec3};

/// Ground hit in model space.
#[derive(Debug, Clone, Copy)]
pub struct GroundHit {
    pub point: Vec3,
    pub normal: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub struct Leg {
    pub upper: usize,
    pub lower: usize,
    pub foot: usize,
    /// Height of the ankle above the sole in the rest pose.
    pub ankle_height: f32,
}

impl Leg {
    pub fn from_rig(rig: &Rig, side: &str) -> Option<Leg> {
        let upper = rig.find(&format!("{side}UpLeg"))?;
        let lower = rig.find(&format!("{side}Leg"))?;
        let foot = rig.find(&format!("{side}Foot"))?;
        // Lowest point of the rest skeleton is the sole: the foot's lowest descendant (toe) or the ankle.
        let model = rig.rest_pose().model(rig);
        let sole = (0..rig.len()).filter(|&b| is_under(rig, b, foot)).map(|b| model[b].pos.z).fold(f32::INFINITY, f32::min);
        Some(Leg { upper, lower, foot, ankle_height: (model[foot].pos.z - sole.min(model[foot].pos.z)).max(0.06) })
    }
}

fn is_under(rig: &Rig, mut b: usize, ancestor: usize) -> bool {
    loop {
        if b == ancestor {
            return true;
        }
        match rig.parents[b] {
            Some(p) => b = p,
            None => return false,
        }
    }
}

/// Foot placement: drops the pelvis so the lower foot can reach the ground, then puts each foot on
/// the ground under it with two-bone IK and tilts it to the slope. Smoothed over time.
#[derive(Debug, Clone)]
pub struct FootPlacement {
    pub hips: usize,
    pub legs: [Leg; 2],
    /// Knees bend toward +X (forward) in model space.
    pub knee_forward: Vec3,
    /// Max slope tilt applied to a foot, radians.
    pub max_tilt: f32,
    /// How fast offsets settle (1/s).
    pub sharpness: f32,
    /// Max probe distance above/below the animated foot.
    pub probe: f32,
    pelvis: f32,
    foot_offset: [f32; 2],
    foot_normal: [Vec3; 2],
}

impl FootPlacement {
    pub fn new(rig: &Rig) -> Option<Self> {
        Some(Self {
            hips: rig.find("Hips")?,
            legs: [Leg::from_rig(rig, "Left")?, Leg::from_rig(rig, "Right")?],
            knee_forward: Vec3::X,
            max_tilt: 0.6,
            sharpness: 14.0,
            probe: 0.9,
            pelvis: 0.0,
            foot_offset: [0.0; 2],
            foot_normal: [Vec3::Z; 2],
        })
    }

    /// `ground(p)` returns the ground under model-space point `p` (vertical probe).
    /// `floor` is the model-space height the animation treats as ground (0 for AC1 rigs).
    pub fn solve(&mut self, pose: &mut Pose, rig: &Rig, floor: f32, dt: f32, weight: f32, ground: impl Fn(Vec3) -> Option<GroundHit>) {
        let k = 1.0 - (-self.sharpness * dt).exp();
        let mut want = [0.0f32; 2];
        let mut normal = [Vec3::Z; 2];
        for (i, leg) in self.legs.iter().enumerate() {
            let ankle = pose.model_of(rig, leg.foot).pos;
            // How high the animation lifts this foot above its floor (swing phase).
            let lift = (ankle.z - floor - leg.ankle_height).max(0.0);
            let probe_from = Vec3::new(ankle.x, ankle.y, floor + self.probe);
            if let Some(hit) = ground(probe_from).filter(|h| (h.point.z - floor).abs() <= self.probe) {
                want[i] = hit.point.z - floor;
                // Fade tilt out while the foot is in the air.
                let air = (lift / 0.15).clamp(0.0, 1.0);
                normal[i] = hit.normal.normalize_or(Vec3::Z).lerp(Vec3::Z, air).normalize();
            }
        }
        // Lower the pelvis for the lower foot; never raise it (the leg would overextend).
        let pelvis_want = want[0].min(want[1]).min(0.0);
        self.pelvis += (pelvis_want - self.pelvis) * k;
        for i in 0..2 {
            self.foot_offset[i] += (want[i] - self.foot_offset[i]) * k;
            self.foot_normal[i] = self.foot_normal[i].lerp(normal[i], k).normalize_or(Vec3::Z);
        }

        pose.translate_model(rig, self.hips, Vec3::Z * self.pelvis * weight);
        for (i, leg) in self.legs.iter().enumerate() {
            let foot = pose.model_of(rig, leg.foot);
            // Animated ankle, shifted by this foot's ground offset (pelvis drop already moved it down).
            let target = foot.pos + Vec3::Z * (self.foot_offset[i] - self.pelvis) * weight;
            let tilt = crate::limited_arc(Vec3::Z, self.foot_normal[i], self.max_tilt);
            let end_rot = Quat::IDENTITY.slerp(tilt, weight) * foot.rot;
            let knee = pose.model_of(rig, leg.lower).pos;
            let pole = knee + self.knee_forward * 0.5;
            two_bone_ik(pose, rig, leg.upper, leg.lower, leg.foot, target, Some(pole), Some(end_rot), 1.0);
        }
    }

    pub fn pelvis_offset(&self) -> f32 {
        self.pelvis
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Arm {
    pub upper: usize,
    pub lower: usize,
    pub hand: usize,
}

impl Arm {
    pub fn from_rig(rig: &Rig, side: &str) -> Option<Arm> {
        Some(Arm { upper: rig.find(&format!("{side}Arm"))?, lower: rig.find(&format!("{side}ForeArm"))?, hand: rig.find(&format!("{side}Hand"))? })
    }
}

/// Grab a surface point: the palm goes to `point`, facing into the surface (`-normal`), with the
/// fingers pointing along `up` (e.g. over a ledge). Elbow points toward `elbow_hint`.
/// `palm_offset` is the hand-local offset from the wrist to the palm centre.
#[allow(clippy::too_many_arguments)]
pub fn place_hand(
    pose: &mut Pose,
    rig: &Rig,
    arm: Arm,
    point: Vec3,
    normal: Vec3,
    up: Vec3,
    elbow_hint: Vec3,
    rest_hand_basis: Quat,
    palm_offset: Vec3,
    weight: f32,
) -> f32 {
    // Desired model rotation: map the rest hand basis (fingers=X, palm=-Z in the hand's own basis
    // frame given by `rest_hand_basis`) onto the surface frame.
    let fingers = up.normalize_or(Vec3::Z);
    let palm = (-normal).normalize_or(Vec3::X);
    let side = fingers.cross(palm).normalize_or(Vec3::Y);
    let fingers = palm.cross(side);
    let surface = Quat::from_mat3(&glam::Mat3::from_cols(fingers, side, -palm));
    let hand_rot = surface * rest_hand_basis.inverse();
    let wrist = point - hand_rot * palm_offset;
    two_bone_ik(pose, rig, arm.upper, arm.lower, arm.hand, wrist, Some(elbow_hint), Some(hand_rot), weight)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::test_rig;

    fn biped() -> Rig {
        // Reuse the test leg twice: hips=0, left leg 1..4, right leg mirrored.
        let mut rig = test_rig();
        rig.names[0] = Some("Hips".into());
        rig.names[1] = Some("LeftUpLeg".into());
        rig.names[2] = Some("LeftLeg".into());
        rig.names[3] = Some("LeftFoot".into());
        let base = rig.len();
        for (k, name) in ["RightUpLeg", "RightLeg", "RightFoot", "RightToe"].iter().enumerate() {
            let mut x = rig.rest[1 + k];
            x.pos.y = -x.pos.y;
            rig.parents.push(Some(if k == 0 { 0 } else { base + k - 1 }));
            rig.names.push(Some(name.to_string()));
            rig.rest.push(x);
        }
        rig
    }

    #[test]
    fn feet_follow_step() {
        let rig = biped();
        let mut fp = FootPlacement::new(&rig).unwrap();
        // Left side ground is 0.2 higher; right side is 0.1 lower than the animation floor.
        let floor = rig.rest_pose().model(&rig)[3].pos.z - fp.legs[0].ankle_height;
        let ground = |p: Vec3| Some(GroundHit { point: Vec3::new(p.x, p.y, floor + if p.y > 0.0 { 0.2 } else { -0.1 }), normal: Vec3::Z });
        let mut pose = rig.rest_pose();
        for _ in 0..200 {
            pose = rig.rest_pose();
            fp.solve(&mut pose, &rig, floor, 1.0 / 60.0, 1.0, ground);
        }
        let m = pose.model(&rig);
        let l = m[fp.legs[0].foot].pos.z - fp.legs[0].ankle_height - floor;
        let r = m[fp.legs[1].foot].pos.z - fp.legs[1].ankle_height - floor;
        assert!((l - 0.2).abs() < 0.01, "left foot at {l}");
        assert!((r + 0.1).abs() < 0.01, "right foot at {r}");
        assert!((fp.pelvis_offset() + 0.1).abs() < 0.01);
    }
}
