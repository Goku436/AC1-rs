//! ac1-hook: our DLL inside the real Assassin's Creed, for comparing it with ac1-rs (and fixing it where it breaks).
//! Dropped into the game's folder as `dinput8.dll` (the exe imports DirectInput 8, and Windows looks in the exe's
//! folder first), it loads before the game's own code runs; deleting the file undoes everything. Build it 32-bit:
//! `cargo build --release -p ac1-hook --target i686-pc-windows-msvc`.
//!
//! - DirectInput: `DirectInput8Create` and the COM entry points go on to the real `dinput8.dll` in the system folder,
//!   loaded on first use (not from `DllMain`, under the loader lock).
//! - Startup fix: with a network adapter up, AC1 starts its online thread (`gconnect.ubi.com`, long shut down) as a
//!   seventh engine thread, and the engine has slots for six: it stops itself at a breakpoint 6 s in. The game's
//!   import of `iphlpapi!GetAdaptersInfo` is pointed at ours, which reports no adapters, as with the network off. Only
//!   this game sees it.
//!
//! What it does is written to `ac1-hook.log` beside the exe. Nothing here runs outside a 32-bit build (the 64-bit
//! build is only for the workspace's checks).

#![allow(clippy::missing_safety_doc)]

use std::ffi::c_void;
use std::io::Write;

type Hmodule = *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryA(name: *const u8) -> Hmodule;
    fn GetProcAddress(module: Hmodule, name: *const u8) -> *mut c_void;
    fn GetSystemDirectoryA(buf: *mut u8, len: u32) -> u32;
    #[cfg(target_pointer_width = "32")]
    fn GetModuleHandleA(name: *const u8) -> Hmodule;
    fn GetModuleFileNameA(module: Hmodule, buf: *mut u8, len: u32) -> u32;
    #[cfg(target_pointer_width = "32")]
    fn VirtualProtect(addr: *mut c_void, size: usize, prot: u32, old: *mut u32) -> i32;
}

#[cfg(target_pointer_width = "32")]
const PAGE_READWRITE: u32 = 0x04;
/// `ERROR_NO_DATA`: what `GetAdaptersInfo` returns with no adapters.
const ERROR_NO_DATA: u32 = 232;
const E_FAIL: i32 = 0x8000_4005_u32 as i32;

/// A line in `ac1-hook.log` beside the exe.
fn log(msg: &str) {
    let mut buf = [0u8; 520];
    // SAFETY: the buffer is as long as we say; a null module is the exe.
    let n = unsafe { GetModuleFileNameA(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    let exe = String::from_utf8_lossy(&buf[..n]).into_owned();
    let path = std::path::Path::new(&exe).with_file_name("ac1-hook.log");
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{msg}");
    }
}

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
    unsafe { std::mem::transmute::<*mut c_void, Create>(f)(inst, version, riid, out, outer) }
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
unsafe extern "system" fn no_adapters(_info: *mut c_void, _len: *mut u32) -> u32 {
    ERROR_NO_DATA
}

/// Point the exe's import `dll!func` at `to`. Returns whether it was found.
#[cfg(target_pointer_width = "32")]
unsafe fn patch_import(dll: &str, func: &str, to: usize) -> bool {
    // SAFETY: the exe's own image, its headers as the PE format lays them out.
    unsafe {
        let base = GetModuleHandleA(std::ptr::null()) as usize;
        let rd = |off: usize| (base + off) as *const u32;
        let nt = base + *rd(0x3c) as usize;
        // The import directory: the optional header's data directory 1 (32-bit layout).
        let imports = *((nt + 24 + 96 + 8) as *const u32) as usize;
        if imports == 0 {
            return false;
        }
        let cstr = |at: usize| std::ffi::CStr::from_ptr((base + at) as *const std::ffi::c_char).to_string_lossy().into_owned();
        let mut desc = base + imports;
        loop {
            let names = *(desc as *const u32) as usize;
            let name = *((desc + 12) as *const u32) as usize;
            let thunks = *((desc + 16) as *const u32) as usize;
            if name == 0 {
                return false;
            }
            if cstr(name).eq_ignore_ascii_case(dll) {
                let mut k = 0;
                loop {
                    let by_name = *((base + if names != 0 { names } else { thunks }) as *const u32).add(k) as usize;
                    if by_name == 0 {
                        break;
                    }
                    // (Not by ordinal: the high bit clear, then a hint and the name.)
                    if by_name & 0x8000_0000 == 0 && cstr(by_name + 2) == func {
                        let slot = ((base + thunks) as *mut u32).add(k);
                        let mut old = 0;
                        VirtualProtect(slot as *mut c_void, 4, PAGE_READWRITE, &mut old);
                        *slot = to as u32;
                        VirtualProtect(slot as *mut c_void, 4, old, &mut old);
                        return true;
                    }
                    k += 1;
                }
            }
            desc += 20;
        }
    }
}

#[cfg(not(target_pointer_width = "32"))]
unsafe fn patch_import(_dll: &str, _func: &str, _to: usize) -> bool {
    false
}

/// Windows calls this when the DLL loads: the startup fix goes in before the game's code runs.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(_module: Hmodule, reason: u32, _reserved: *mut c_void) -> i32 {
    const DLL_PROCESS_ATTACH: u32 = 1;
    if reason == DLL_PROCESS_ATTACH {
        // SAFETY: patching our own process's import table, before its code runs.
        let patched = unsafe { patch_import("iphlpapi.dll", "GetAdaptersInfo", no_adapters as *const () as usize) };
        log(&format!("ac1-hook loaded; GetAdaptersInfo {}", if patched { "now reports no adapters (startup fix)" } else { "not found in the imports" }));
    }
    1
}
