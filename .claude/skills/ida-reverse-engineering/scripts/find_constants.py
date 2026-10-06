"""Find 32-bit constants (little-endian) in every segment of the open IDB.

Headless:
    idat -A -S"find_constants.py out.json 0x0fa3067f 0x415d9568" game.exe

Immediates in code and dwords in data tables both match, since both are
stored as 4 little-endian bytes. Reports the containing function for code
hits and the xrefs for data hits.
"""
import json
import struct

import idaapi
import idautils
import idc
import ida_bytes
import ida_funcs
import ida_name
import ida_segment

MAX_HITS_PER_VALUE = 200


def _is_batch():
    try:
        import ida_kernwin
        return bool(ida_kernwin.cvar.batch)
    except Exception:
        return False


def main():
    idaapi.auto_wait()
    if len(idc.ARGV) < 3:
        print("usage: find_constants.py <out.json> <hex_value> [...]")
        if _is_batch():
            idc.qexit(1)
        return

    out_path = idc.ARGV[1]
    values = [int(v, 16) for v in idc.ARGV[2:]]
    results = {"0x%08X" % v: {"hits": [], "truncated": False} for v in values}

    for i in range(ida_segment.get_segm_qty()):
        seg = ida_segment.getnseg(i)
        if seg is None:
            continue
        size = seg.end_ea - seg.start_ea
        data = ida_bytes.get_bytes(seg.start_ea, size)
        if not data:
            continue
        seg_name = ida_segment.get_segm_name(seg)

        for v in values:
            key = "0x%08X" % v
            entry = results[key]
            pat = struct.pack("<I", v)
            pos = data.find(pat)
            while pos != -1:
                if len(entry["hits"]) >= MAX_HITS_PER_VALUE:
                    entry["truncated"] = True
                    break
                ea = seg.start_ea + pos
                f = ida_funcs.get_func(ea)
                hit = {"addr": "0x%X" % ea, "segment": seg_name}
                if f is not None:
                    hit["function"] = "0x%X" % f.start_ea
                    hit["function_name"] = ida_name.get_name(f.start_ea)
                else:
                    hit["xrefs_from"] = ["0x%X" % x.frm for x in idautils.XrefsTo(ea, 0)][:20]
                entry["hits"].append(hit)
                pos = data.find(pat, pos + 1)

    with open(out_path, "w") as fh:
        json.dump(results, fh, indent=1)
    for key, entry in results.items():
        print("[find_constants] %s: %d hits%s" % (
            key, len(entry["hits"]), " (truncated)" if entry["truncated"] else ""))

    if _is_batch():
        idc.qexit(0)


main()
