//! The few Windows functions the hook needs (no crates: a 32-bit DLL kept small).

use std::ffi::c_void;

pub type Hmodule = *mut c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn LoadLibraryA(name: *const u8) -> Hmodule;
    pub fn GetProcAddress(module: Hmodule, name: *const u8) -> *mut c_void;
    pub fn GetSystemDirectoryA(buf: *mut u8, len: u32) -> u32;
    pub fn GetModuleFileNameA(module: Hmodule, buf: *mut u8, len: u32) -> u32;
    #[cfg(target_pointer_width = "32")]
    pub fn GetModuleHandleA(name: *const u8) -> Hmodule;
    #[cfg(target_pointer_width = "32")]
    pub fn VirtualProtect(addr: *mut c_void, size: usize, prot: u32, old: *mut u32) -> i32;
    #[cfg(target_pointer_width = "32")]
    pub fn VirtualAlloc(addr: *mut c_void, size: usize, kind: u32, prot: u32) -> *mut c_void;
    #[cfg(target_pointer_width = "32")]
    pub fn VirtualQuery(addr: *const c_void, info: *mut MemoryInfo, len: usize) -> usize;
    #[cfg(target_pointer_width = "32")]
    pub fn FlushInstructionCache(process: *mut c_void, addr: *const c_void, size: usize) -> i32;
    #[cfg(target_pointer_width = "32")]
    pub fn GetCurrentProcess() -> *mut c_void;
}

/// `MEMORY_BASIC_INFORMATION` (32-bit).
#[cfg(target_pointer_width = "32")]
#[repr(C)]
#[derive(Default)]
pub struct MemoryInfo {
    pub base: usize,
    pub alloc_base: usize,
    pub alloc_protect: u32,
    pub size: usize,
    pub state: u32,
    pub protect: u32,
    pub kind: u32,
}

/// Can `len` bytes at `addr` be read (committed, not guarded or no-access)? For the game's pointers, checked before
/// following them: a stale one must not crash the game.
#[cfg(target_pointer_width = "32")]
pub fn readable(addr: usize, len: usize) -> bool {
    if addr < 0x10000 {
        return false;
    }
    let mut info = MemoryInfo::default();
    // SAFETY: VirtualQuery only fills the struct.
    let n = unsafe { VirtualQuery(addr as *const c_void, &mut info, std::mem::size_of::<MemoryInfo>()) };
    const MEM_COMMIT: u32 = 0x1000;
    const PAGE_NOACCESS: u32 = 0x01;
    const PAGE_GUARD: u32 = 0x100;
    n != 0 && info.state == MEM_COMMIT && info.protect & (PAGE_NOACCESS | PAGE_GUARD) == 0 && addr + len <= info.base + info.size
}

/// A u32 of the game's memory at `addr`, if readable.
#[cfg(target_pointer_width = "32")]
pub fn peek(addr: usize) -> Option<u32> {
    // SAFETY: checked readable.
    readable(addr, 4).then(|| unsafe { (addr as *const u32).read_unaligned() })
}

/// The class name (MSVC RTTI) of the object at `obj`, as `.?AVName@scimitar@@` gives it: `Name`.
#[cfg(target_pointer_width = "32")]
pub fn class_of(obj: usize) -> Option<String> {
    let vtable = peek(obj)? as usize;
    let locator = peek(vtable.checked_sub(4)?)? as usize;
    let td = peek(locator + 12)? as usize;
    if !readable(td + 8, 64) {
        return None;
    }
    // SAFETY: checked readable; a NUL-terminated mangled name.
    let raw = unsafe { std::ffi::CStr::from_ptr((td + 8) as *const std::ffi::c_char) }.to_string_lossy().into_owned();
    let name = raw.strip_prefix(".?AV").unwrap_or(&raw);
    Some(name.split('@').next().unwrap_or(name).to_string())
}

#[cfg(target_pointer_width = "32")]
pub const PAGE_READWRITE: u32 = 0x04;

/// A file beside the game's exe.
pub fn game_file(name: &str) -> std::path::PathBuf {
    let mut buf = [0u8; 520];
    // SAFETY: the buffer is as long as we say; a null module is the exe.
    let n = unsafe { GetModuleFileNameA(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32) } as usize;
    std::path::Path::new(&String::from_utf8_lossy(&buf[..n]).into_owned()).with_file_name(name)
}

/// A line in `ac1-hook.log` beside the exe.
pub fn log(msg: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(game_file("ac1-hook.log")) {
        let _ = writeln!(f, "{msg}");
    }
}

/// `name!func` (both NUL-terminated) from a module already loaded, or loaded now.
#[cfg(target_pointer_width = "32")]
pub fn proc(module: &[u8], func: &[u8]) -> *mut c_void {
    // SAFETY: NUL-terminated names.
    unsafe {
        let mut m = GetModuleHandleA(module.as_ptr());
        if m.is_null() {
            m = LoadLibraryA(module.as_ptr());
        }
        if m.is_null() { std::ptr::null_mut() } else { GetProcAddress(m, func.as_ptr()) }
    }
}
