//! Pointing the game's calls at ours: an import of the exe, or a slot of a COM object's vtable (shared by every object
//! of that class). 32-bit only.

use std::ffi::c_void;

use crate::sys::{FlushInstructionCache, GetCurrentProcess, GetModuleHandleA, PAGE_READWRITE, VirtualAlloc, VirtualProtect};

/// Write `to` into the pointer-sized slot at `slot`; returns what was there.
unsafe fn swap(slot: *mut usize, to: usize) -> usize {
    // SAFETY: a slot in the image's import table or a vtable, made writable for the write.
    unsafe {
        let mut old = 0;
        VirtualProtect(slot as *mut c_void, 4, PAGE_READWRITE, &mut old);
        let was = *slot;
        *slot = to;
        VirtualProtect(slot as *mut c_void, 4, old, &mut old);
        was
    }
}

/// Point the exe's import `dll!func` at `to`; returns the function it pointed at, or None when not imported.
pub unsafe fn import(dll: &str, func: &str, to: usize) -> Option<usize> {
    // SAFETY: the exe's own image, its headers as the PE format lays them out.
    unsafe {
        let base = GetModuleHandleA(std::ptr::null()) as usize;
        let u32_at = |at: usize| *((base + at) as *const u32) as usize;
        let nt = u32_at(0x3c);
        // The import directory: the optional header's data directory 1 (32-bit layout).
        let imports = u32_at(nt + 24 + 96 + 8);
        if imports == 0 {
            return None;
        }
        let cstr = |at: usize| std::ffi::CStr::from_ptr((base + at) as *const std::ffi::c_char).to_string_lossy().into_owned();
        let mut desc = imports;
        loop {
            let (names, name, thunks) = (u32_at(desc), u32_at(desc + 12), u32_at(desc + 16));
            if name == 0 {
                return None;
            }
            if cstr(name).eq_ignore_ascii_case(dll) {
                let mut k = 0;
                loop {
                    let by_name = u32_at(if names != 0 { names } else { thunks } + 4 * k);
                    if by_name == 0 {
                        break;
                    }
                    // (Not by ordinal: the high bit clear, then a hint and the name.)
                    if by_name & 0x8000_0000 == 0 && cstr(by_name + 2) == func {
                        return Some(swap((base + thunks + 4 * k) as *mut usize, to));
                    }
                    k += 1;
                }
            }
            desc += 20;
        }
    }
}

/// Point slot `index` of `object`'s vtable at `to`; returns the method it pointed at.
pub unsafe fn vtable(object: *mut c_void, index: usize, to: usize) -> usize {
    // SAFETY: a live COM object: its first word is its vtable.
    unsafe {
        let table = *(object as *const *mut usize);
        swap(table.add(index), to)
    }
}

/// Detour the game's function at `target`: its first instructions must be exactly `prologue` (5 bytes or more, no
/// jumps or calls among them: they are copied as they are); they go into a stub that goes on with the rest of the
/// function, and `target` then jumps to `hook`. Returns the stub (call it to run the original), or None if the bytes
/// differ (another build of the exe: nothing patched).
pub unsafe fn detour(target: usize, prologue: &[u8], hook: usize) -> Option<usize> {
    const MEM_COMMIT_RESERVE: u32 = 0x3000;
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;
    let n = prologue.len();
    if n < 5 || !crate::sys::readable(target, n) {
        return None;
    }
    // SAFETY: checked readable above; the patch is made writable for the write.
    unsafe {
        if std::slice::from_raw_parts(target as *const u8, n) != prologue {
            return None;
        }
        let stub = VirtualAlloc(std::ptr::null_mut(), 32, MEM_COMMIT_RESERVE, PAGE_EXECUTE_READWRITE) as *mut u8;
        if stub.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(prologue.as_ptr(), stub, n);
        *stub.add(n) = 0xE9;
        ((stub.add(n + 1)) as *mut i32).write_unaligned((target + n) as i32 - (stub as usize + n + 5) as i32);
        let mut old = 0;
        VirtualProtect(target as *mut c_void, n, PAGE_EXECUTE_READWRITE, &mut old);
        let at = target as *mut u8;
        *at = 0xE9;
        (at.add(1) as *mut i32).write_unaligned(hook as i32 - (target + 5) as i32);
        for k in 5..n {
            *at.add(k) = 0x90;
        }
        VirtualProtect(target as *mut c_void, n, old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), target as *const c_void, n);
        Some(stub as usize)
    }
}
