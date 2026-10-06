---
name: bevy-port-architecture
description: How to structure the Bevy runtime in ac1-rs (crates/game). Use for Bevy plugins, ECS design, systems and schedules, skinned meshes and skeletons, animation playback, locomotion and climbing, IK integration, custom asset loading from the user's game install, coordinate conversion from the game's Z-up data, materials and shaders. Use it for any Bevy change in this repo, since Bevy's API changes between releases.
---

# Bevy runtime for ac1-rs

## Step 0: check the Bevy version
Bevy's API changes almost every release. Read the version in `Cargo.toml`, copy the style of nearby code, and check `docs.rs/bevy/<version>` when unsure. Don't upgrade Bevy as a side effect of another task.

## Keep logic out of Bevy
- `forge`: formats. `ik`: pure math. `game`: Bevy glue only.
- If something can be a pure function in `forge` or `ik` with a unit test, put it there. The Bevy side feeds data in, calls it, and applies the result to transforms.

## Coordinates (known facts for this project)
- Game data is Z-up, characters face +X.
- Mesh space = skeleton model space turned 90° about Z.
- Animation clips face +Y; the Reference bone carries the 90° turn. Hash-0 tracks are root motion.
- Bevy is right-handed, Y-up.
- Do all conversion in one `convert` module in `game` with tests (axes, winding order, matrix layout, row vs column major; the inverse bind matrices are row-major). No inline axis swaps in gameplay code. When a character renders rotated or mirrored, fix it in `convert`, not at the call site.

## Characters
- An Entity lists meshes, a main skeleton, and add-on skeletons (head, skirt, hood, sword tag) rooted at main-skeleton bones with the same hash. Build one joint hierarchy with add-ons attached at the matching main bones.
- Skinned vertices use per-submesh bone palettes (palette index → bone). Remap to the joint indices Bevy's skinning expects when building the `Mesh`. Check Bevy's joint limit per mesh for the pinned version and split submeshes if needed.
- Bone ids are CRC32 name hashes. Keep the name→hash map for debugging so logs show names.
- Material overrides come from the Entity's `(placeholder, real)` pairs; resolve them at spawn.

## Animation
- Clips are sampled at 60 fps, with per-track keys. Decode in `forge`, convert to Bevy `AnimationClip` or sample manually; pick one approach and keep it consistent.
- Locomotion picks idle/walk/jog/run half-cycles and rate-matches by root-motion speed. Keep that selection logic pure and tested.
- Climbing: a move is taken only if its end pose puts both hands on real holds; IK then snaps hands and feet. The move graph and rules are in the README.
- Apply IK after animation sampling and before skinning, in an explicit system order.

## Plugins and ordering
- One `Plugin` per subsystem: character spawn, animation, locomotion, climbing, IK, camera, debug.
- Order systems explicitly (`.chain()` or `SystemSet`s): input → locomotion/state → animation sampling → IK → transform propagation → render. Never rely on implicit order.
- Use `FixedUpdate` for gameplay state that must be deterministic; `Update` for input sampling, camera and presentation. Don't mix them in one system.

## Assets
- Custom `AssetLoader`s read from `AC1_GAME_DIR` at runtime through `forge`. Never embed or copy game assets into the repo or the binary.
- Convert to Bevy types (`Mesh`, `Image`) at load, one module per format. Surface loader errors with archive, object name and offset.
- Textures: DXT1/3/5 and BGRA8. Prefer uploading compressed formats if the pinned Bevy and the GPU support them; otherwise decode to RGBA8.

## Debugging
- Add gizmos early: skeleton lines, bone names, holds and foot targets, IK targets.
- Keep test hooks documented in the header of `crates/game/src/main.rs`.
- Use `tracing` spans and Bevy diagnostics before optimizing. Use `opt-level = 1` for the app and `3` for dependencies in the dev profile so debug runs stay usable.
- When something looks wrong, check in this order: coordinate conversion, bone palette remap, system ordering, then the data.
