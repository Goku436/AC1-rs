//! DirectInput: the keyboard device the game makes, its reads (`GetDeviceState`, a snapshot of every key, and
//! `GetDeviceData`, buffered presses and releases) given keys of ours too: `cmd`'s `key` holds a key down for some
//! frames, whatever has the focus.
//!
//! Out of focus the game's devices cannot be acquired and their reads fail. With `ac1-hook.background` beside the exe,
//! `Acquire` is answered as done and a failed read as an empty one (plus ours), so the game goes on taking our keys
//! while another window is in front. Off without the file: answered as done at startup, before its window was in
//! front, the game never acquired the mouse again and the player could not click in its menus. Without it, a failed
//! read is ours alone only while we hold something.

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::{patch, sys};

static CREATE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static GET_STATE: AtomicUsize = AtomicUsize::new(0);
static GET_DATA: AtomicUsize = AtomicUsize::new(0);
static ACQUIRE: AtomicUsize = AtomicUsize::new(0);
static SAID_UNACQUIRED: AtomicBool = AtomicBool::new(false);
/// The keyboard devices the game made (it makes more than one), and its mice.
static KEYBOARDS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static MICE: Mutex<Vec<usize>> = Mutex::new(Vec::new());
/// Mouse buttons held (button, frames left), and motion to give (dx, dy per frame, frames left).
static BUTTONS: Mutex<Vec<(u8, u32)>> = Mutex::new(Vec::new());
static MOTION: Mutex<Vec<(i32, i32, u32)>> = Mutex::new(Vec::new());
static SAID_STATE: AtomicBool = AtomicBool::new(false);
static SAID_DATA: AtomicBool = AtomicBool::new(false);
static SAID_MOUSE: AtomicBool = AtomicBool::new(false);

/// Keys held now (DIK code, frames left), and the presses and releases not yet handed to a buffered read.
static HELD: Mutex<Vec<(u8, u32)>> = Mutex::new(Vec::new());
static EVENTS: Mutex<Vec<(u8, bool)>> = Mutex::new(Vec::new());
static SEQUENCE: AtomicUsize = AtomicUsize::new(0x7000_0000);

/// IDirectInput8::CreateDevice, IDirectInputDevice8::GetDeviceState / GetDeviceData (vtable slots).
const SLOT_CREATE_DEVICE: usize = 3;
/// IDirectInputDevice8::Acquire.
const SLOT_ACQUIRE: usize = 7;
const SLOT_GET_STATE: usize = 9;
const SLOT_GET_DATA: usize = 10;
/// GUID_SysKeyboard {6F1D2B61-D5A0-11CF-BFC7-444553540000}.
const GUID_KEYBOARD: [u8; 16] = [0x61, 0x2b, 0x1d, 0x6f, 0xa0, 0xd5, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00];
/// GUID_SysMouse {6F1D2B60-D5A0-11CF-BFC7-444553540000}.
const GUID_MOUSE: [u8; 16] = [0x60, 0x2b, 0x1d, 0x6f, 0xa0, 0xd5, 0xcf, 0x11, 0xbf, 0xc7, 0x44, 0x45, 0x53, 0x54, 0x00, 0x00];

type CreateDevice = unsafe extern "system" fn(*mut c_void, *const [u8; 16], *mut *mut c_void, *mut c_void) -> i32;
type GetState = unsafe extern "system" fn(*mut c_void, u32, *mut u8) -> i32;
type GetData = unsafe extern "system" fn(*mut c_void, u32, *mut u8, *mut u32, u32) -> i32;

/// The IDirectInput8 the game just made: watch the devices it makes.
pub fn hook_direct_input(di: *mut c_void) {
    if di.is_null() || CREATE_DEVICE.load(Ordering::SeqCst) != 0 {
        return;
    }
    // SAFETY: a live IDirectInput8.
    let real = unsafe { patch::vtable(di, SLOT_CREATE_DEVICE, create_device as *const () as usize) };
    CREATE_DEVICE.store(real, Ordering::SeqCst);
}

unsafe extern "system" fn create_device(this: *mut c_void, guid: *const [u8; 16], out: *mut *mut c_void, outer: *mut c_void) -> i32 {
    // SAFETY: the real method, with its signature.
    let r = unsafe { std::mem::transmute::<usize, CreateDevice>(CREATE_DEVICE.load(Ordering::SeqCst))(this, guid, out, outer) };
    // SAFETY: on success `out` holds a live device; the GUID is 16 bytes.
    unsafe {
        if r >= 0 && !guid.is_null() && *guid == GUID_MOUSE && !(*out).is_null() {
            MICE.lock().unwrap().push(*out as usize);
            if GET_STATE.load(Ordering::SeqCst) == 0 {
                GET_STATE.store(patch::vtable(*out, SLOT_GET_STATE, get_state as *const () as usize), Ordering::SeqCst);
                GET_DATA.store(patch::vtable(*out, SLOT_GET_DATA, get_data as *const () as usize), Ordering::SeqCst);
                ACQUIRE.store(patch::vtable(*out, SLOT_ACQUIRE, acquire as *const () as usize), Ordering::SeqCst);
            }
            sys::log("dinput: mouse made, its reads hooked");
        }
        if r >= 0 && !guid.is_null() && *guid == GUID_KEYBOARD && !(*out).is_null() {
            KEYBOARDS.lock().unwrap().push(*out as usize);
            if GET_STATE.load(Ordering::SeqCst) == 0 {
                GET_STATE.store(patch::vtable(*out, SLOT_GET_STATE, get_state as *const () as usize), Ordering::SeqCst);
                GET_DATA.store(patch::vtable(*out, SLOT_GET_DATA, get_data as *const () as usize), Ordering::SeqCst);
                ACQUIRE.store(patch::vtable(*out, SLOT_ACQUIRE, acquire as *const () as usize), Ordering::SeqCst);
            }
            sys::log("dinput: keyboard made, its reads hooked");
        }
    }
    r
}

unsafe extern "system" fn get_state(this: *mut c_void, size: u32, data: *mut u8) -> i32 {
    // SAFETY: the real method, with its signature.
    let r = unsafe { std::mem::transmute::<usize, GetState>(GET_STATE.load(Ordering::SeqCst))(this, size, data) };
    // The mouse (DIMOUSESTATE or DIMOUSESTATE2: x, y, wheel, then the buttons): our motion and buttons added.
    if is_mouse(this) && size >= 16 && !data.is_null() {
        if !SAID_MOUSE.swap(true, Ordering::SeqCst) {
            sys::log(&format!("dinput: the game reads the mouse by GetDeviceState ({size} bytes)"));
        }
        let buttons = BUTTONS.lock().unwrap();
        let motion = MOTION.lock().unwrap();
        if r < 0 && buttons.is_empty() && motion.is_empty() && !background() {
            return r;
        }
        // SAFETY: a DIMOUSESTATE of `size` bytes.
        unsafe {
            if r < 0 {
                std::ptr::write_bytes(data, 0, size as usize);
            }
            let xy = data as *mut i32;
            for &(dx, dy, _) in motion.iter() {
                *xy += dx;
                *xy.add(1) += dy;
            }
            for &(b, _) in buttons.iter() {
                if 12 + (b as u32) < size {
                    *data.add(12 + b as usize) = 0x80;
                }
            }
        }
        return 0;
    }
    if is_keyboard(this) && size >= 256 && !data.is_null() {
        if !SAID_STATE.swap(true, Ordering::SeqCst) {
            sys::log("dinput: the game reads the keyboard by GetDeviceState");
        }
        let held = HELD.lock().unwrap();
        // (Not acquired, out of focus: the state is ours alone, in background mode or while we hold a key.)
        if r < 0 && held.is_empty() && !background() {
            return r;
        }
        // SAFETY: a 256-byte key array.
        unsafe {
            if r < 0 {
                std::ptr::write_bytes(data, 0, 256);
            }
            for &(k, _) in held.iter() {
                *data.add(k as usize) |= 0x80;
            }
        }
        return 0;
    }
    r
}

unsafe extern "system" fn get_data(this: *mut c_void, item: u32, data: *mut u8, count: *mut u32, flags: u32) -> i32 {
    // SAFETY: the count is the caller's in-out word.
    let room = if count.is_null() { 0 } else { unsafe { *count } };
    // SAFETY: the real method, with its signature.
    let r = unsafe { std::mem::transmute::<usize, GetData>(GET_DATA.load(Ordering::SeqCst))(this, item, data, count, flags) };
    if is_keyboard(this) && !data.is_null() && !count.is_null() && item >= 16 {
        if !SAID_DATA.swap(true, Ordering::SeqCst) {
            sys::log("dinput: the game reads the keyboard by GetDeviceData");
        }
        // Our presses and releases after the real ones, as many as there is room for (DIDEVICEOBJECTDATA: offset, data,
        // time, sequence[, app data]). (Peeking, flag 1, leaves them queued.)
        let mut events = EVENTS.lock().unwrap();
        // SAFETY: in-out count; `room` records of `item` bytes.
        unsafe {
            let mut n = if r < 0 { 0 } else { *count };
            let mut taken = 0;
            for &(k, down) in events.iter() {
                if n >= room {
                    break;
                }
                let rec = data.add((n * item) as usize) as *mut u32;
                *rec = k as u32;
                *rec.add(1) = if down { 0x80 } else { 0 };
                *rec.add(2) = 0;
                *rec.add(3) = SEQUENCE.fetch_add(1, Ordering::SeqCst) as u32;
                n += 1;
                taken += 1;
            }
            if flags & 1 == 0 {
                events.drain(..taken);
            }
            *count = n;
            if taken > 0 && r < 0 {
                return 0;
            }
        }
    }
    r
}

type Acquire = unsafe extern "system" fn(*mut c_void) -> i32;

/// `Acquire`: tried, and answered as done (out of focus it fails, and the game would stop reading the device).
unsafe extern "system" fn acquire(this: *mut c_void) -> i32 {
    // SAFETY: the real method, with its signature.
    let r = unsafe { std::mem::transmute::<usize, Acquire>(ACQUIRE.load(Ordering::SeqCst))(this) };
    if r >= 0 || !background() {
        return r;
    }
    if !SAID_UNACQUIRED.swap(true, Ordering::SeqCst) {
        sys::log(&format!("dinput: Acquire failed ({r:#x}, out of focus): answered as done (background mode)"));
    }
    0
}

/// Background mode (`ac1-hook.background` beside the exe), looked at once a second at most.
fn background() -> bool {
    static AT: Mutex<Option<(std::time::Instant, bool)>> = Mutex::new(None);
    let mut at = AT.lock().unwrap();
    match *at {
        Some((t, on)) if t.elapsed().as_secs_f32() < 1.0 => on,
        _ => {
            let on = sys::game_file("ac1-hook.background").exists();
            *at = Some((std::time::Instant::now(), on));
            on
        }
    }
}

fn is_mouse(device: *mut c_void) -> bool {
    MICE.lock().unwrap().contains(&(device as usize))
}

/// Hold mouse button `b` (0 left, 1 right, 2 middle) for `frames` frames.
pub fn button(b: u8, frames: u32) {
    BUTTONS.lock().unwrap().push((b, frames.max(1)));
}

/// Move the mouse (dx, dy) each frame for `frames` frames (the camera).
pub fn motion(dx: i32, dy: i32, frames: u32) {
    MOTION.lock().unwrap().push((dx, dy, frames.max(1)));
}

fn is_keyboard(device: *mut c_void) -> bool {
    KEYBOARDS.lock().unwrap().contains(&(device as usize))
}

/// Hold key `dik` (a DirectInput key code) for `frames` frames.
pub fn press(dik: u8, frames: u32) {
    HELD.lock().unwrap().push((dik, frames.max(1)));
    EVENTS.lock().unwrap().push((dik, true));
}

/// A frame went by: let go of keys held long enough.
pub fn tick() {
    let mut held = HELD.lock().unwrap();
    let mut events = EVENTS.lock().unwrap();
    held.retain_mut(|(k, left)| {
        *left -= 1;
        if *left == 0 {
            events.push((*k, false));
        }
        *left > 0
    });
    BUTTONS.lock().unwrap().retain_mut(|(_, left)| {
        *left -= 1;
        *left > 0
    });
    MOTION.lock().unwrap().retain_mut(|(_, _, left)| {
        *left -= 1;
        *left > 0
    });
}
