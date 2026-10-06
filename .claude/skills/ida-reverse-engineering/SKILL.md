---
name: ida-reverse-engineering
description: Reverse engineering the AC1 (Scimitar engine) formats and behavior in ac1-rs, using hex dumps, pass-rate measurement, and IDA Pro / Hex-Rays. Use whenever the task involves an unknown field in a .forge format, a class hash, an unparsed animation, mesh or entity layout, a binary address or sub_XXXX function, decompiler output, IDAPython, or idat headless runs. Use it even when the user only says "this file fails to parse" or pastes a hex dump.
---

# Reverse engineering for ac1-rs

This project reads the game's data formats and recreates behavior. It does not port original code. IDA is for answering specific questions (a field's meaning, a bit layout, a constant), not for translating functions.

## Hard rules
1. Never commit the game binary, IDBs, extracted data, or decompiler dumps. Don't paste decompiled code into the repo. Write the Rust from the understanding, not from the pseudocode.
2. Every claim needs evidence: a hex offset, a pass rate, a code location, or a visual check. If it's a guess, mark it `?` or low confidence in the notes.
3. Hex-Rays output is a hint. Bit-twiddling, x87/SSE code and packed formats are where it's most often wrong. Read the disassembly for those.

## The format loop (use this first, IDA second)
1. **Get samples.** Dump a failing or unknown file with the `forge` tool. Compare a working file and a failing one side by side.
2. **Guess a layout** from sizes: does `header + n * stride` equal the payload size? Do counts match offsets? Do values look like floats, small ints, or hashes?
3. **Implement the guess** in the parser, with errors that include the offset.
4. **Measure over everything.** Run on all files and compare the pass rate to the README. Rising count means the guess is right. Never judge from one file.
5. **Verify visually** when possible: textures to PNG, meshes and animations in the Bevy viewer. A clip can parse and still be wrong.
6. **Write it down** in the README format notes or the module header: offsets, types, units, and what is still `?`.

Go to IDA when step 2 stalls: bit-packed fields, unknown flags, a stride you can't explain, or an enum with no pattern.

## Finding the loader in IDA
The README lists class hashes. Use them as anchors:
- Search for each hash as a 32-bit value, little-endian (bytes appear reversed). It shows up as an immediate in code and as a dword in factory/registration tables. Class hashes already known: Mesh `415d9568`, compiled mesh `fc9e1595`, Entity `0984415e`, EntityGroup `3f742d26`, Skeleton `24aecb7c`, Animation `0fa3067f`.
- Search the stream magic `33 AA FB 57 99 FA 04 10` to find the decompression/stream reader.
- Search `0xEDB88320` (CRC32 table) to find the name-hash function used for bone names.
- Search strings for class and field names. Assert strings often name the original source file.
- Once you find a class's deserializer, read it field by field. Its read order is the file layout.
- Parse MSVC RTTI (`.?AV...@@`) if present to recover class names and vtables.
- For x86 MSVC methods, set `__thiscall` (`this` in ECX) before reading the pseudocode.

`scripts/find_constants.py` automates the hash search.

## Naming and notes
- Rename functions as `Subsystem::Class::Method`. Add a comment `[conf: high|med|low] <evidence>` to each rename.
- Define structs and enums in IDA as you learn them; use `field_0xNN` names until the role is certain.
- Keep findings in the README format notes or `re/notes/<subsystem>.md` (git-ignored if it quotes the binary closely).

## Headless exports
IDA 9 has one `idat` for 32 and 64 bit. Older versions have `idat` and `idat64`.

```bash
idat -A -S"scripts/find_constants.py re/hits.json 0x0fa3067f 0x415d9568" path/to/game.exe
idat -A -S"scripts/export_functions.py re/functions.json" path/to/game.exe
idat -A -S"scripts/export_decompiled.py re/decomp 0x5A3F20" path/to/game.exe
```

- `find_constants.py`: finds 32-bit values across all segments, reports the containing function or xrefs.
- `export_functions.py`: all functions with names, sizes, xref counts.
- `export_decompiled.py`: Hex-Rays output for chosen addresses, for reading outside IDA. Keep out of git.

## Before leaving IDA
Record: the exact layout (offset, type, units), bit widths and ordering for packed fields, enum values, edge cases (flags that change the layout), and the test you'll use to confirm it (which files, what pass rate to expect).
