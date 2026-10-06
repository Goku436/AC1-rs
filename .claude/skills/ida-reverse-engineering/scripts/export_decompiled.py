"""Dump Hex-Rays pseudocode for specific functions.

Headless:
    idat -A -S"export_decompiled.py out_dir 0x5A3F20 0x5A4100" game.exe

Args: output directory, then one or more addresses (hex) or function names.
"""
import os

import idaapi
import idc
import ida_funcs
import ida_name
import ida_hexrays


def _is_batch():
    try:
        import ida_kernwin
        return bool(ida_kernwin.cvar.batch)
    except Exception:
        return False


def _resolve(token):
    try:
        return int(token, 16) if token.lower().startswith("0x") else int(token)
    except ValueError:
        ea = ida_name.get_name_ea(idaapi.BADADDR, token)
        return ea


def main():
    idaapi.auto_wait()
    if len(idc.ARGV) < 3:
        print("usage: export_decompiled.py <out_dir> <addr|name> [...]")
        if _is_batch():
            idc.qexit(1)
        return
    if not ida_hexrays.init_hexrays_plugin():
        print("[export_decompiled] Hex-Rays decompiler not available")
        if _is_batch():
            idc.qexit(1)
        return

    out_dir = idc.ARGV[1]
    os.makedirs(out_dir, exist_ok=True)

    for token in idc.ARGV[2:]:
        ea = _resolve(token)
        f = ida_funcs.get_func(ea) if ea != idaapi.BADADDR else None
        if f is None:
            print("[export_decompiled] no function at %s" % token)
            continue
        try:
            text = str(ida_hexrays.decompile(f.start_ea))
        except ida_hexrays.DecompilationFailure as e:
            print("[export_decompiled] failed 0x%X: %s" % (f.start_ea, e))
            continue
        header = "// 0x%X  %s\n// pseudocode only; verify against disassembly\n\n" % (
            f.start_ea, ida_name.get_name(f.start_ea))
        path = os.path.join(out_dir, "0x%X.c" % f.start_ea)
        with open(path, "w") as fh:
            fh.write(header + text)
        print("[export_decompiled] wrote %s" % path)

    if _is_batch():
        idc.qexit(0)


main()
