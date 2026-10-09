//! ac1-hook: our DLL inside the real Assassin's Creed, for comparing it with ac1-rs (and fixing it where it breaks).
//! Dropped into the game's folder as `dinput8.dll` (the exe imports DirectInput 8, and Windows looks in the exe's
//! folder first), it loads before the game's own code runs; deleting the file undoes everything. Build it 32-bit:
//! `cargo build --release -p ac1-hook --target i686-pc-windows-msvc`.
//!
//! - DirectInput: `DirectInput8Create` and the COM entry points go on to the real `dinput8.dll` in the system folder,
//!   loaded on first use (not from `DllMain`, under the loader lock). The keyboard the game makes takes keys of ours
//!   too (`input`).
//! - Startup fix: with a network adapter up, AC1 starts its online thread (`gconnect.ubi.com`, long shut down) as a
//!   seventh engine thread, and the engine has slots for six: it stops itself at a breakpoint 6 s in. The game's
//!   import of `iphlpapi!GetAdaptersInfo` is pointed at ours, which reports no adapters, as with the network off. Only
//!   this game sees it.
//! - Direct3D 9: its `Present` runs the hook once a frame (`d3d`), reading commands from a file (`cmd`: screenshots,
//!   key presses), so the game can be driven and seen from outside whatever window has the focus.
//!
//! What it does is written to `ac1-hook.log` beside the exe. Nothing here runs outside a 32-bit build (the 64-bit
//! build is only for the workspace's checks).

#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;

#[cfg(target_pointer_width = "32")]
mod cmd;
#[cfg(target_pointer_width = "32")]
mod d3d;
#[cfg(target_pointer_width = "32")]
mod input;
#[cfg(target_pointer_width = "32")]
mod patch;
mod sys;
#[cfg(target_pointer_width = "32")]
mod trace;

use sys::{GetProcAddress, GetSystemDirectoryA, Hmodule, LoadLibraryA, log};

/// `ERROR_NO_DATA`: what `GetAdaptersInfo` returns with no adapters.
#[cfg(target_pointer_width = "32")]
const ERROR_NO_DATA: u32 = 232;
const E_FAIL: i32 = 0x8000_4005_u32 as i32;

/// The real `dinput8.dll`'s export `name` (NUL-terminated), the DLL loaded from the system folder on first use.
fn real(name: &[u8]) -> *mut c_void {
    static REAL: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let module = *REAL.get_or_init(|| {
        let mut buf = [0u8; 300];
        // SAFETY: the buffer is as long as we say.
        let n = unsafe { GetSystemDirectoryA(buf.as_mut_ptr(), buf.len() as u32) } as usize;
        let mut path = buf[..n].to_vec();
        path.extend_from_slice(b"\\dinput8.dll\0");
        // SAFETY: a NUL-terminated path.
        let m = unsafe { LoadLibraryA(path.as_ptr()) };
        log(&format!("real dinput8.dll from {}: {}", String::from_utf8_lossy(&path[..path.len() - 1]), if m.is_null() { "not loaded" } else { "loaded" }));
        m as usize
    });
    if module == 0 {
        return std::ptr::null_mut();
    }
    // SAFETY: a loaded module and a NUL-terminated name.
    unsafe { GetProcAddress(module as Hmodule, name.as_ptr()) }
}

type Create = unsafe extern "system" fn(*mut c_void, u32, *const c_void, *mut *mut c_void, *mut c_void) -> i32;
type NoArgs = unsafe extern "system" fn() -> i32;
type GetClass = unsafe extern "system" fn(*const c_void, *const c_void, *mut *mut c_void) -> i32;

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DirectInput8Create(inst: *mut c_void, version: u32, riid: *const c_void, out: *mut *mut c_void, outer: *mut c_void) -> i32 {
    let f = real(b"DirectInput8Create\0");
    if f.is_null() {
        return E_FAIL;
    }
    // SAFETY: the real export, with the real signature.
    let r = unsafe { std::mem::transmute::<*mut c_void, Create>(f)(inst, version, riid, out, outer) };
    #[cfg(target_pointer_width = "32")]
    if r >= 0 && !out.is_null() {
        // SAFETY: on success `out` holds the new IDirectInput8.
        input::hook_direct_input(unsafe { *out });
    }
    r
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllCanUnloadNow() -> i32 {
    let f = real(b"DllCanUnloadNow\0");
    // SAFETY: the real export, with the real signature.
    if f.is_null() { 1 } else { unsafe { std::mem::transmute::<*mut c_void, NoArgs>(f)() } }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(clsid: *const c_void, riid: *const c_void, out: *mut *mut c_void) -> i32 {
    let f = real(b"DllGetClassObject\0");
    // SAFETY: the real export, with the real signature.
    if f.is_null() { E_FAIL } else { unsafe { std::mem::transmute::<*mut c_void, GetClass>(f)(clsid, riid, out) } }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllRegisterServer() -> i32 {
    let f = real(b"DllRegisterServer\0");
    // SAFETY: the real export, with the real signature.
    if f.is_null() { E_FAIL } else { unsafe { std::mem::transmute::<*mut c_void, NoArgs>(f)() } }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllUnregisterServer() -> i32 {
    let f = real(b"DllUnregisterServer\0");
    // SAFETY: the real export, with the real signature.
    if f.is_null() { E_FAIL } else { unsafe { std::mem::transmute::<*mut c_void, NoArgs>(f)() } }
}

/// `GetAdaptersInfo` as the game sees it: no adapters (as with the network off).
#[cfg(target_pointer_width = "32")]
unsafe extern "system" fn no_adapters(_info: *mut c_void, _len: *mut u32) -> u32 {
    ERROR_NO_DATA
}

/// Windows calls this when the DLL loads: the startup fix and the hooks go in before the game's code runs.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(_module: Hmodule, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        #[cfg(target_pointer_width = "32")]
        {
            // SAFETY: patching our own process's import table, before its code runs.
            let patched = unsafe { patch::import("iphlpapi.dll", "GetAdaptersInfo", no_adapters as *const () as usize) }.is_some();
            log(&format!("ac1-hook loaded; GetAdaptersInfo {}", if patched { "now reports no adapters (startup fix)" } else { "not found in the imports" }));
            d3d::install();
            trace::install();
        }
        #[cfg(not(target_pointer_width = "32"))]
        log("ac1-hook loaded (64-bit build: does nothing)");
    }
    1
}
