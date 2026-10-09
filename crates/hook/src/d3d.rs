//! Direct3D 9: the game's `Direct3DCreate9` → the device's `CreateDevice` → its `Present`, where the hook does its
//! work once a frame (`cmd::frame`), and screenshots of the frame the game just drew (its back buffer, through the
//! game's own D3DX).
//!
//! With `ac1-hook.window` beside the exe (`x y w h`, screen pixels), the game draws at `w`×`h` in a window there: its
//! `/windowed` mode otherwise makes a borderless window the size of the desktop, and resizing that window afterwards
//! stops its rendering. The size is set where the device is made and on every `Reset`.
//!
//! With `ac1-hook.background` beside the exe, its window is told it was put behind another (`WM_ACTIVATEAPP`,
//! `WM_ACTIVATE`, `WM_KILLFOCUS`) no more: the game stopped taking any keys out of focus, the scripted ones too
//! (`wndproc`). Off without the file: with it on, the user could not click in the game.

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{patch, sys};

static CREATE9: AtomicUsize = AtomicUsize::new(0);
static CREATE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static PRESENT: AtomicUsize = AtomicUsize::new(0);
static RESET: AtomicUsize = AtomicUsize::new(0);
/// The game's own window procedure, called by ours (`wndproc`).
static WNDPROC: AtomicUsize = AtomicUsize::new(0);

/// IDirect3D9::CreateDevice, IDirect3DDevice9::Present / GetBackBuffer, IUnknown::Release (vtable slots).
const SLOT_CREATE_DEVICE: usize = 16;
const SLOT_PRESENT: usize = 17;
/// IDirect3DDevice9::Reset.
const SLOT_RESET: usize = 16;
const SLOT_GET_BACK_BUFFER: usize = 18;
const SLOT_RELEASE: usize = 2;
const D3DXIFF_PNG: u32 = 3;

type Create9 = unsafe extern "system" fn(u32) -> *mut c_void;
type CreateDevice = unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void, u32, *mut c_void, *mut *mut c_void) -> i32;
type Reset = unsafe extern "system" fn(*mut c_void, *mut c_void) -> i32;
type Present = unsafe extern "system" fn(*mut c_void, *const c_void, *const c_void, *mut c_void, *const c_void) -> i32;
type GetBackBuffer = unsafe extern "system" fn(*mut c_void, u32, u32, u32, *mut *mut c_void) -> i32;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type SaveSurface = unsafe extern "system" fn(*const u8, u32, *mut c_void, *const c_void, *const c_void) -> i32;

const WM_ACTIVATE: u32 = 0x0006;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_ACTIVATEAPP: u32 = 0x001c;
const GWL_WNDPROC: i32 = -4;

/// The game's window procedure, but for being put behind another window: that is not passed on, so the game goes on
/// as the app in front (and takes the keys `cmd` gives it). Coming back to the front is passed on as usual.
unsafe extern "system" fn wndproc(window: *mut c_void, msg: u32, wparam: usize, lparam: isize) -> isize {
    let deactivated = match msg {
        WM_ACTIVATEAPP => wparam == 0,
        WM_ACTIVATE => wparam & 0xffff == 0,
        WM_KILLFOCUS => true,
        _ => false,
    };
    if deactivated {
        return 0;
    }
    // SAFETY: the game's own window procedure, with the message it was sent.
    unsafe { sys::CallWindowProcA(WNDPROC.load(Ordering::SeqCst), window, msg, wparam, lparam) }
}

/// Hook Direct3D's creation (from `DllMain`: the exe's imports are bound, none of its code has run).
pub fn install() {
    // SAFETY: patching our own process's import table before its code runs.
    match unsafe { patch::import("d3d9.dll", "Direct3DCreate9", direct3d_create9 as *const () as usize) } {
        Some(real) => {
            CREATE9.store(real, Ordering::SeqCst);
            sys::log("d3d9: Direct3DCreate9 hooked");
        }
        None => sys::log("d3d9: Direct3DCreate9 not imported"),
    }
}

unsafe extern "system" fn direct3d_create9(sdk: u32) -> *mut c_void {
    // SAFETY: the real function, with its signature.
    let d3d = unsafe { std::mem::transmute::<usize, Create9>(CREATE9.load(Ordering::SeqCst))(sdk) };
    if !d3d.is_null() && CREATE_DEVICE.load(Ordering::SeqCst) == 0 {
        // SAFETY: a live IDirect3D9.
        let real = unsafe { patch::vtable(d3d, SLOT_CREATE_DEVICE, create_device as *const () as usize) };
        CREATE_DEVICE.store(real, Ordering::SeqCst);
    }
    d3d
}

unsafe extern "system" fn create_device(
    this: *mut c_void,
    adapter: u32,
    kind: u32,
    window: *mut c_void,
    flags: u32,
    params: *mut c_void,
    out: *mut *mut c_void,
) -> i32 {
    // SAFETY: the game's D3DPRESENT_PARAMETERS.
    unsafe { place(params, window) };
    // Its window keeps believing it is in front (`wndproc`), once, when asked for (`ac1-hook.background` beside the exe):
    // off otherwise, as the user could not click in the game with it on.
    if !window.is_null() && WNDPROC.load(Ordering::SeqCst) == 0 && sys::game_file("ac1-hook.background").exists() {
        // SAFETY: the game's live window; its procedure swapped for ours, which calls it.
        let old = unsafe { sys::SetWindowLongA(window, GWL_WNDPROC, wndproc as *const () as usize as i32) };
        if old != 0 {
            WNDPROC.store(old as u32 as usize, Ordering::SeqCst);
            sys::log("window: its deactivation messages are held back (it takes keys behind other windows)");
        }
    }
    // SAFETY: the real method, with its signature.
    let r = unsafe { std::mem::transmute::<usize, CreateDevice>(CREATE_DEVICE.load(Ordering::SeqCst))(this, adapter, kind, window, flags, params, out) };
    // SAFETY: on success `out` holds a live device.
    if r >= 0 && !out.is_null() && unsafe { !(*out).is_null() } && PRESENT.load(Ordering::SeqCst) == 0 {
        let real = unsafe { patch::vtable(*out, SLOT_PRESENT, present as *const () as usize) };
        PRESENT.store(real, Ordering::SeqCst);
        let real = unsafe { patch::vtable(*out, SLOT_RESET, reset as *const () as usize) };
        RESET.store(real, Ordering::SeqCst);
        sys::log("d3d9: device made, Present and Reset hooked");
    }
    r
}

unsafe extern "system" fn reset(this: *mut c_void, params: *mut c_void) -> i32 {
    // SAFETY: the game's D3DPRESENT_PARAMETERS (its window at +28).
    unsafe { place(params, std::ptr::null_mut()) };
    // SAFETY: the real method, with its signature.
    unsafe { std::mem::transmute::<usize, Reset>(RESET.load(Ordering::SeqCst))(this, params) }
}

/// The window asked for in `ac1-hook.window` (`x y w h`), if any.
fn wanted() -> Option<[i32; 4]> {
    let text = std::fs::read_to_string(sys::game_file("ac1-hook.window")).ok()?;
    let v: Vec<i32> = text.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    (v.len() >= 4 && v[2] > 0 && v[3] > 0).then(|| [v[0], v[1], v[2], v[3]])
}

/// Draw at the wanted size, windowed, in a window put there (D3DPRESENT_PARAMETERS: back buffer width +0, height +4,
/// its window +28, windowed +32, the full-screen refresh rate +48).
///
/// # Safety
/// `params` is null or a D3DPRESENT_PARAMETERS.
unsafe fn place(params: *mut c_void, window: *mut c_void) {
    let Some([x, y, w, h]) = wanted() else { return };
    if params.is_null() {
        return;
    }
    let p = params as *mut u32;
    // SAFETY: the struct's fields, as above.
    unsafe {
        *p = w as u32;
        *p.add(1) = h as u32;
        *p.add(8) = 1;
        *p.add(12) = 0;
        let own = *p.add(7) as *mut c_void;
        let win = if own.is_null() { window } else { own };
        if !win.is_null() {
            // SWP_NOZORDER 0x4 | SWP_NOACTIVATE 0x10
            sys::SetWindowPos(win, std::ptr::null_mut(), x, y, w, h, 0x14);
        }
    }
    sys::log(&format!("d3d9: drawing at {w}x{h}, the window at ({x}, {y})"));
}

unsafe extern "system" fn present(this: *mut c_void, src: *const c_void, dst: *const c_void, window: *mut c_void, dirty: *const c_void) -> i32 {
    crate::cmd::frame(this);
    // SAFETY: the real method, with its signature.
    unsafe { std::mem::transmute::<usize, Present>(PRESENT.load(Ordering::SeqCst))(this, src, dst, window, dirty) }
}

/// Save the frame just drawn to `path` (PNG). Called from `Present`, before the frame is shown.
pub fn screenshot(device: *mut c_void, path: &str) -> Result<(), String> {
    let save = sys::proc(b"d3dx9_36.dll\0", b"D3DXSaveSurfaceToFileA\0");
    if save.is_null() {
        return Err("no D3DXSaveSurfaceToFileA".into());
    }
    let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
    // SAFETY: a live device; its back buffer released after.
    unsafe {
        let table = *(device as *const *const usize);
        let mut surface: *mut c_void = std::ptr::null_mut();
        let r = std::mem::transmute::<usize, GetBackBuffer>(*table.add(SLOT_GET_BACK_BUFFER))(device, 0, 0, 0, &mut surface);
        if r < 0 || surface.is_null() {
            return Err(format!("GetBackBuffer {r:#x}"));
        }
        let r = std::mem::transmute::<*mut c_void, SaveSurface>(save)(cpath.as_ptr() as *const u8, D3DXIFF_PNG, surface, std::ptr::null(), std::ptr::null());
        let st = *(surface as *const *const usize);
        std::mem::transmute::<usize, Release>(*st.add(SLOT_RELEASE))(surface);
        if r < 0 { Err(format!("D3DXSaveSurfaceToFile {r:#x}")) } else { Ok(()) }
    }
}
