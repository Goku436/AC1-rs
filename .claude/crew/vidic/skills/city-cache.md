# Changing what a city holds

The city cache (`crates/game/src/level_cache.rs`, `ac1-cache/` beside the exe) keys on the game files, the source of
the files in its `SOURCES` list and `CACHE_VERSION`.

- Changed a file in `SOURCES`: the key changes by itself.
- Changed code **outside** `SOURCES` that alters collision, holds, guidance, beams, objects or navigation, or added a
  field to `Level` (and to `write_level`): bump `CACHE_VERSION`.
- Check: `AC1_CACHE_CHECK=1` (a fresh build compared with the cache), and for both cities
  `AC1_SCENARIOS=1 cargo test --release -p ac1 --test level_cache`.
- Bypass while iterating: `AC1_NO_CACHE=1`.
- Level loading is shared core code: the whole scenario suite before calling it done.
