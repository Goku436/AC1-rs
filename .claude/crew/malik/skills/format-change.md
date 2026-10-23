# A format change

1. **Baseline**: run the pass-rate tool for the format first and write the number down (e.g. `animcheck`, `actions
   <game>` with no block, `shapes`, `ragdolls`, `bao scan`, `catalog`). NOTES.md's status table has the current ones.
2. **Find the field**: hex (`--bin objects -- ... hex`), then the exe's deserializer for the class (ghidra.md).
   Unknowns stay `?` with their offset.
3. **Parse it** in `crates/forge` (the `rust-recomp-practices` skill): no panics, errors with offsets, bit-packed
   decoders with a unit test on a hand-made sample (not game bytes).
4. **Measure again** over every file: the rate must not drop. Tests that need the install skip cleanly without it.
5. **Record**: NOTES.md's table and the format's notes; who asked gets an inbox line with the answer.
