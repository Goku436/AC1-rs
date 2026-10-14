//! The pad, at the level the game reads it (pure Rust, tested on any target): the `PadProxyPC` layout, what our
//! injection writes into it, and the pad files (`ac1lab pad v1`, as Banned445's ac1lab writes them: a record of what
//! the player's pad held each frame, played back later).
//!
//! `PadProxyPC` (v1.02 DX9; the player's is the input interpreter's +0xD0; its `Update`, vtable slot 9, is 0x909C90):
//! each frame its `Update` has the active device (one of the slots from +0x50C, 0x218 bytes apart, the index at
//! +0x504) read its input, then copies the device's state into itself and works out the rest:
//! - +0x08: the 16 buttons of the frame before (bytes, nonzero = down), copied from +0x18 first;
//! - +0x18: the 16 buttons now (bytes), from the device;
//! - +0x28 / +0x30: the clock (u64) at the previous update and now; +0x38: for each button, the clock when it last
//!   went down (u64 each, 16 of them);
//! - +0xB8: the buttons as floats (1.0 down, 0.0 up), 16 of them;
//! - +0x100: the sticks, from the device: left x, left y, right x, right y (floats, -1..1; +y is forward), then
//!   4 floats more (`?`, left as the device gave them).
//!
//! Buttons seen in the live logs (the bits of `btn=`): 0 the legs (jump/climb), 3 the empty hand, 12 high profile;
//! the rest `?` (1, 8, 9 and 15 are pressed in play).
//!
//! Pad file: a first line `ac1lab pad v1`; optional `start <x> <y> <z> <yaw>` (game space, Z up; yaw in degrees from
//! +X toward +Y), where the player stood when the recording began; then a line a frame: `<dt> <buttons> <lx> <ly>
//! <rx> <ry>` (dt the frame's game seconds, buttons hex). `#` starts a comment.

pub const PREV_BUTTONS: usize = 0x08;
pub const BUTTONS: usize = 0x18;
pub const CLOCK_NOW: usize = 0x30;
pub const PRESSED_AT: usize = 0x38;
pub const ANALOG: usize = 0xB8;
pub const STICKS: usize = 0x100;
/// How much of the proxy the injection reads and writes.
pub const SIZE: usize = 0x110;
pub const HEADER: &str = "ac1lab pad v1";

/// Named buttons (bit numbers), as found in the live logs.
pub const NAMES: [(&str, u8); 3] = [("legs", 0), ("hand", 3), ("high", 12)];

/// One frame of pad: the buttons (bit k = button k), the sticks (lx, ly, rx, ry).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Frame {
    pub dt: f32,
    pub buttons: u16,
    pub sticks: [f32; 4],
}

/// A button by name or bit number (decimal).
pub fn button(word: &str) -> Option<u8> {
    NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(word)).map(|&(_, b)| b).or_else(|| word.parse().ok().filter(|&b: &u8| b < 16))
}

/// The pad as the proxy holds it now (from its first `SIZE` bytes).
pub fn read(image: &[u8]) -> Option<Frame> {
    if image.len() < SIZE {
        return None;
    }
    let buttons = (0..16).fold(0u16, |acc, k| if image[BUTTONS + k] != 0 { acc | 1 << k } else { acc });
    let f = |k: usize| f32::from_le_bytes(image[STICKS + 4 * k..STICKS + 4 * k + 4].try_into().unwrap());
    Some(Frame { dt: 0.0, buttons, sticks: [f(0), f(1), f(2), f(3)] })
}

/// Put `frame` into the proxy's image as its `Update` would have from a device holding it: the buttons, their
/// floats, the press times of those that went down this frame (against the frame before, +0x08), the sticks.
pub fn write(image: &mut [u8], frame: &Frame) -> bool {
    if image.len() < SIZE {
        return false;
    }
    let now: [u8; 8] = image[CLOCK_NOW..CLOCK_NOW + 8].try_into().unwrap();
    for k in 0..16 {
        let down = frame.buttons & (1 << k) != 0;
        image[BUTTONS + k] = down as u8;
        let analog: f32 = if down { 1.0 } else { 0.0 };
        image[ANALOG + 4 * k..ANALOG + 4 * k + 4].copy_from_slice(&analog.to_le_bytes());
        if down && image[PREV_BUTTONS + k] == 0 {
            image[PRESSED_AT + 8 * k..PRESSED_AT + 8 * k + 8].copy_from_slice(&now);
        }
    }
    for (k, v) in frame.sticks.iter().enumerate() {
        let v = if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
        image[STICKS + 4 * k..STICKS + 4 * k + 4].copy_from_slice(&v.to_le_bytes());
    }
    true
}

/// A frame's line in a pad file.
pub fn line(f: &Frame) -> String {
    format!("{:.6} {:04x} {:.4} {:.4} {:.4} {:.4}", f.dt, f.buttons, f.sticks[0], f.sticks[1], f.sticks[2], f.sticks[3])
}

/// The start line (game space).
pub fn start_line(pos: [f32; 3], yaw: f32) -> String {
    format!("start {:.4} {:.4} {:.4} {:.2}", pos[0], pos[1], pos[2], yaw)
}

/// A pad file read: where it started (x, y, z, yaw), its frames.
#[derive(Debug, Default, PartialEq)]
pub struct Recording {
    pub start: Option<[f32; 4]>,
    pub frames: Vec<Frame>,
}

/// Read a pad file; an error names the line.
pub fn parse(text: &str) -> Result<Recording, String> {
    let mut rec = Recording::default();
    let mut header = false;
    for (n, raw) in text.lines().enumerate() {
        let l = raw.split('#').next().unwrap_or("").trim();
        if l.is_empty() {
            continue;
        }
        if !header {
            if l != HEADER {
                return Err(format!("line {}: not a pad file (want `{HEADER}` first)", n + 1));
            }
            header = true;
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        if w[0] == "start" {
            let v: Vec<f32> = w[1..].iter().filter_map(|x| x.parse().ok()).collect();
            if v.len() < 3 {
                return Err(format!("line {}: start wants x y z [yaw]", n + 1));
            }
            rec.start = Some([v[0], v[1], v[2], v.get(3).copied().unwrap_or(f32::NAN)]);
            continue;
        }
        if w.len() < 4 {
            return Err(format!("line {}: want dt buttons lx ly [rx ry]", n + 1));
        }
        let num = |k: usize| w.get(k).map_or(Ok(0.0), |x| x.parse::<f32>().map_err(|_| format!("line {}: bad number {x}", n + 1)));
        let buttons = u16::from_str_radix(w[1].trim_start_matches("0x"), 16).map_err(|_| format!("line {}: bad buttons {}", n + 1, w[1]))?;
        let dt = num(0)?;
        if !(dt.is_finite() && dt >= 0.0) {
            return Err(format!("line {}: bad dt", n + 1));
        }
        rec.frames.push(Frame { dt, buttons, sticks: [num(2)?, num(3)?, num(4)?, num(5)?] });
    }
    if !header {
        return Err("empty pad file".into());
    }
    Ok(rec)
}

/// Playing a recording back: by the game's time (each update is given its frame's dt, the record whose time span
/// holds the update's start is played), or one record an update (`by_frame`).
#[derive(Debug)]
pub struct Playback {
    pub frames: Vec<Frame>,
    pub by_frame: bool,
    /// Game seconds played; the record now, and when it begins.
    t: f64,
    at: usize,
    at_t: f64,
}

impl Playback {
    pub fn new(frames: Vec<Frame>, by_frame: bool) -> Self {
        Self { frames, by_frame, t: 0.0, at: 0, at_t: 0.0 }
    }

    /// The record for this update (whose frame lasts `dt`), or None when the recording is over.
    pub fn next(&mut self, dt: f32) -> Option<Frame> {
        if self.by_frame {
            let f = self.frames.get(self.at).copied();
            self.at += 1;
            return f;
        }
        // (A small slack: a record's dt summed in f64 against the same dts given back must not skip one.)
        while self.at < self.frames.len() && self.at_t + (self.frames[self.at].dt as f64) <= self.t + 1e-6 {
            self.at_t += self.frames[self.at].dt as f64;
            self.at += 1;
        }
        let f = self.frames.get(self.at).copied();
        self.t += dt.max(0.0) as f64;
        f
    }

    /// The record playing now (its index) and how many there are.
    pub fn progress(&self) -> (usize, usize) {
        (self.at.min(self.frames.len()), self.frames.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_by_name_or_number() {
        assert_eq!(button("legs"), Some(0));
        assert_eq!(button("HIGH"), Some(12));
        assert_eq!(button("9"), Some(9));
        assert_eq!(button("16"), None);
        assert_eq!(button("jump"), None);
    }

    #[test]
    fn writing_the_proxy() {
        let mut img = vec![0u8; SIZE];
        img[CLOCK_NOW..CLOCK_NOW + 8].copy_from_slice(&1234u64.to_le_bytes());
        // Button 3 was already down the frame before: its press time stays.
        img[PREV_BUTTONS + 3] = 1;
        img[PRESSED_AT + 8 * 3..PRESSED_AT + 8 * 3 + 8].copy_from_slice(&99u64.to_le_bytes());
        img[BUTTONS + 5] = 1;
        let f = Frame { dt: 0.0, buttons: 0x1009, sticks: [0.0, 1.0, -2.0, f32::NAN] };
        assert!(write(&mut img, &f));
        assert_eq!(&img[BUTTONS..BUTTONS + 16], &[1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]);
        let at = |k: usize| u64::from_le_bytes(img[PRESSED_AT + 8 * k..PRESSED_AT + 8 * k + 8].try_into().unwrap());
        assert_eq!((at(0), at(3), at(12), at(5)), (1234, 99, 1234, 0));
        let fl = |o: usize| f32::from_le_bytes(img[o..o + 4].try_into().unwrap());
        assert_eq!((fl(ANALOG), fl(ANALOG + 4), fl(ANALOG + 48)), (1.0, 0.0, 1.0));
        let back = read(&img).unwrap();
        assert_eq!(back.buttons, 0x1009);
        assert_eq!(back.sticks, [0.0, 1.0, -1.0, 0.0]);
        assert!(!write(&mut [0u8; 8], &f));
    }

    #[test]
    fn pad_files() {
        let f = Frame { dt: 1.0 / 60.0, buttons: 0x1001, sticks: [0.0, 1.0, 0.25, -0.5] };
        let text = format!("{HEADER}\n# a comment\n{}\n{}\n0.02 0 0 0\n", start_line([1.0, 2.0, 3.0], 45.0), line(&f));
        let r = parse(&text).unwrap();
        assert_eq!(r.start, Some([1.0, 2.0, 3.0, 45.0]));
        assert_eq!(r.frames.len(), 2);
        assert_eq!(r.frames[0].buttons, 0x1001);
        assert!((r.frames[0].dt - f.dt).abs() < 1e-6);
        assert_eq!(r.frames[0].sticks, f.sticks);
        assert_eq!(r.frames[1].sticks, [0.0; 4]);
        assert!(parse("hello\n").is_err());
        assert!(parse(&format!("{HEADER}\n0.1 zz 0 0\n")).is_err());
        assert!(parse(&format!("{HEADER}\n-1 0 0 0\n")).is_err());
        assert!(parse("").is_err());
    }

    #[test]
    fn playing_back_by_time() {
        let fr = |b: u16, dt: f32| Frame { dt, buttons: b, sticks: [0.0; 4] };
        // Recorded at 60 fps, played at 60: one record an update.
        let mut p = Playback::new(vec![fr(1, 1.0 / 60.0), fr(2, 1.0 / 60.0), fr(3, 1.0 / 60.0)], false);
        let got: Vec<_> = (0..4).map(|_| p.next(1.0 / 60.0).map(|f| f.buttons)).collect();
        assert_eq!(got, vec![Some(1), Some(2), Some(3), None]);
        // Played at 30: every other record.
        let mut p = Playback::new((0..6).map(|k| fr(k, 1.0 / 60.0)).collect(), false);
        let got: Vec<_> = (0..4).map(|_| p.next(1.0 / 30.0).map(|f| f.buttons)).collect();
        assert_eq!(got, vec![Some(0), Some(2), Some(4), None]);
        // Played at 120: each record twice.
        let mut p = Playback::new(vec![fr(7, 1.0 / 60.0), fr(8, 1.0 / 60.0)], false);
        let got: Vec<_> = (0..5).map(|_| p.next(1.0 / 120.0).map(|f| f.buttons)).collect();
        assert_eq!(got, vec![Some(7), Some(7), Some(8), Some(8), None]);
        // By frame, whatever the dt.
        let mut p = Playback::new(vec![fr(1, 0.5), fr(2, 0.5)], true);
        assert_eq!(p.next(0.001).map(|f| f.buttons), Some(1));
        assert_eq!(p.next(0.001).map(|f| f.buttons), Some(2));
        assert_eq!(p.next(0.001), None);
        assert_eq!(p.progress(), (2, 2));
    }
}
