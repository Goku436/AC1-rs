---
name: rust-recomp-practices
description: Rust practices for ac1-rs, the from-scratch Rust runtime that reads Assassin's Creed data. Use for any Rust work on the forge crate (archives, LZO, textures, meshes, skeletons, animation parsers), the ik crate (pure math), or code shared with the game crate. Covers parser design, bit-packed decoding, numeric fidelity, testing by pass rate, and repo hygiene. Use it for any Rust change in this repo, even small ones.
---

# Rust practices for ac1-rs

Read `CLAUDE.md` and `README.md` first. They win over this skill if they conflict.

## What kind of project this is
A runtime that reads the user's own game install and recreates behavior. It is not a recompilation. Code is one of two kinds, and each file should say which:
- **Format code** (`forge`): must match the original byte for byte. Wrong is wrong.
- **Recreation** (`ik`, locomotion, climbing): must look and feel right. Doc it as "recreation of behavior seen in-game, not a port."

## Parser design (forge)
- Read through a small cursor type with bounds checks. No indexing raw slices with offsets from the file.
- Parse explicitly: `u32::from_le_bytes`, not transmute or pointer casts.
- Every error carries the file offset and what was expected.
- Never panic on bad input. No `unwrap`, `expect`, or unchecked arithmetic on file-derived values. Use `checked_*` on sizes and offsets before allocating or slicing; a corrupt length must not cause a huge allocation.
- Keep unknown fields as named raw values (`unk_0x18: u32`) so layouts stay honest and diffs show up.
- Document the layout in the module header with offsets, types and units, in the style of `anim.rs`. Mark unknowns `?`.
- For structs that mirror file layout, assert sizes: `const _: () = assert!(core::mem::size_of::<Header>() == 0x20);`

## Bit-packed data (quats, translations)
- Smallest-three quaternions at 16/24/32/48 bits, packed translations (11/11/10 bits, i16x3): write the decoder and a test encoder, then round-trip test.
- Check normalization after decode. A decoded quat with length far from 1 usually means wrong bit order or scale.
- State units in the type or name (`_mm`, `_m`, `_rad`). Original translations are millimetres; convert once, at a defined boundary.
- Use `wrapping_*` or `checked_*` explicitly. Don't depend on debug vs release overflow behavior.
- No silent `as` casts between ints and floats. Use a named helper that documents rounding and truncation.

## Math (ik and shared)
- `f32` unless there's a reason. Compare with tolerances in tests, never `==`.
- Pure functions, no Bevy types, no global state. That keeps them unit-testable.
- Handle degenerate input (zero-length bones, unreachable targets, parallel pole vectors) with defined fallbacks and tests for each.

## Testing
1. **Pass rate is the main metric** for format work. A command should report N/M for each format over the whole install. Record the numbers in the README status table. A change must not lower any number.
2. **Known failures are explicit.** Keep a list of the files that fail today (for example the 16 data files, the 16 clips) so a regression stands out from a known gap.
3. **Unit tests use synthetic data** built in the test. Never commit game bytes as fixtures.
4. Tests that need the install check for it and skip with a clear message when it isn't there. They must not fail on a machine without the game.
5. `proptest` for round-trips and math invariants; `cargo fuzz` targets for parsers.
6. Visual checks (PNG export, viewer) count as verification. Say what you looked at.

## Repo hygiene
- No game files, extracted data, IDBs or decompiled code in git. `out/` stays ignored.
- CI/local: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- One format feature per commit, for example `forge(anim): decode 64-bit quats`.
- Update the README status table and format notes in the same commit as the code.

## When unsure
Say what's unknown. Add a `?` in the notes and an `#[ignore]`d test or a clear error. Don't invent a layout to make a file pass.
