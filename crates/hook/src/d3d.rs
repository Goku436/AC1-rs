//! Direct3D 9: the game's `Direct3DCreate9` → the device's `CreateDevice` → its `Present`, where the hook does its
//! work once a frame (`cmd::frame`), and screenshots of the frame the game just drew (its back buffer, through the
//! game's own D3DX).

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{patch, sys};

static CREATE9: AtomicUsize = AtomicUsize::new(0);
static CREATE_DEVICE: AtomicUsize = AtomicUsize::new(0);
static PRESENT: AtomicUsize = AtomicUsize::new(0);

/// IDirect3D9::CreateDevice, IDirect3DDevice9::Present / GetBackBuffer, IUnknown::Release (vtable slots).
const SLOT_CREATE_DEVICE: usize = 16;
const SLOT_PRESENT: usize = 17;
const SLOT_GET_BACK_BUFFER: usize = 18;
const SLOT_RELEASE: usize = 2;
const D3DXIFF_PNG: u32 = 3;

type Create9 = unsafe extern "system" fn(u32) -> *mut c_void;
type CreateDevice = unsafe extern "system" fn(*mut c_void, u32, u32, *mut c_void, u32, *mut c_void, *mut *mut c_void) -> i32;
type Present = unsafe extern "system" fn(*mut c_void, *const c_void, *const c_void, *mut c_void, *const c_void) -> i32;
type GetBackBuffer = unsafe extern "system" fn(*mut c_void, u32, u32, u32, *mut *mut c_void) -> i32;
type Release = unsafe extern "system" fn(*mut c_void) -> u32;
type SaveSurface = unsafe extern "system" fn(*const u8, u32, *mut c_void, *const c_void, *const c_void) -> i32;

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
    // SAFETY: the real method, with its signature.
    let r = unsafe { std::mem::transmute::<usize, CreateDevice>(CREATE_DEVICE.load(Ordering::SeqCst))(this, adapter, kind, window, flags, params, out) };
    // SAFETY: on success `out` holds a live device.
    if r >= 0 && !out.is_null() && unsafe { !(*out).is_null() } && PRESENT.load(Ordering::SeqCst) == 0 {
        let real = unsafe { patch::vtable(*out, SLOT_PRESENT, present as *const () as usize) };
        PRESENT.store(real, Ordering::SeqCst);
        sys::log("d3d9: device made, Present hooked");
    }
    r
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
