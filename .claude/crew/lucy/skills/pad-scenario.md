# From a pad file to a scenario

1. **Record** in the real game (the `live-ac1` skill): `padrec <file>` while doing the route, with a `start x y z yaw`
   line (game space, Z up, yaw in degrees from +X toward +Y). Pad files are not game data: they go in
   `crates/game/tests/pads/<city-area>_<move>.pad`.
2. **Compare**: `tools/live-compare.sh crates/game/tests/pads/<file>.pad` and read `compare.txt`. The real game's
   numbers (where it ends, the context sequence, timings) are what the scenario checks.
3. **Scenario**: copy the shape of the existing pad scenarios in `crates/game/tests/scenarios.rs` (search
   `AC1_PADPLAY`): `env` with `AC1_LEVEL`, `AC1_NO_CROWD=1`, `AC1_PADPLAY` (`concat!(env!("CARGO_MANIFEST_DIR"),
   "/tests/pads/<file>.pad")`), and `AC1_PADPLAY_CAM=<yaw>` when the comparison handed over the real camera's yaw.
   Tags from the moves it goes through. A comment names the comparison run it came from and what the real game did.
4. **Run it alone** (`AC1_SCENARIO_FILTER=<name>`). If ours fails where the real game didn't, that's the point:
   leave the owner an inbox line with the failing check and the compare output path, and keep the scenario.
5. Update the pipeline page's scenario count.
