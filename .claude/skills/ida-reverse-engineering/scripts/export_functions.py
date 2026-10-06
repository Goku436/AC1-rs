"""Export all functions from the open IDB to JSON.

Headless:
    idat -A -S"export_functions.py out.json" game.exe
"""
import json

import idaapi
import idautils
import idc
import ida_funcs
import ida_name


def _is_batch():
    try:
        import ida_kernwin
        return bool(ida_kernwin.cvar.batch)
    except Exception:
        return False


def main():
    idaapi.auto_wait()  # let auto-analysis finish before reading anything
    out_path = idc.ARGV[1] if len(idc.ARGV) > 1 else "functions.json"

    funcs = []
    for ea in idautils.Functions():
        f = ida_funcs.get_func(ea)
        if f is None:
            continue
        name = ida_name.get_name(ea)
        funcs.append({
            "addr": "0x%X" % ea,
            "size": f.end_ea - f.start_ea,
            "name": name,
            "default_name": name.startswith(("sub_", "nullsub_", "j_sub_")),
            "lib": bool(f.flags & ida_funcs.FUNC_LIB),
            "thunk": bool(f.flags & ida_funcs.FUNC_THUNK),
            "xrefs_to": sum(1 for _ in idautils.CodeRefsTo(ea, 0)),
        })

    with open(out_path, "w") as fh:
        json.dump({"count": len(funcs), "functions": funcs}, fh, indent=1)
    print("[export_functions] wrote %d functions to %s" % (len(funcs), out_path))

    if _is_batch():
        idc.qexit(0)


main()
