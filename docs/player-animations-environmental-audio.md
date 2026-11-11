# Player Animations + Environmental Audio (Android)

Status: implementation specification. This document does not claim that either feature is implemented or that an installable `.hmod` exists.

Target: Horizon Android 0.7.8, ARM64, existing sandboxed WebAssembly `.hmod` system.

## Scope

The first release targets **the local player model only**. Mob animation support is explicitly out of scope.

The goal is to let a compatible resource pack provide Fresh Animations-style player geometry and animation data, while an independent environmental-audio mod can play ambience based on the current client environment.

## Compatibility findings

- The current `render` mod capability draws HUD/UI-oriented shapes. It is not a hook for replacing or animating the game's player model.
- Therefore a normal `.hmod` cannot implement player-model animation using the current public mod API alone.
- Player animation needs a renderer-side extension: a controlled player-model/pose hook that the renderer invokes for the local player. This is a core Horizon change, not something a standalone WebAssembly mod can safely emulate.
- The audio engine and sound-instance machinery already exist in the renderer. The current mod WIT/runtime does not expose a sound host capability. A separate permissioned audio API is required.
- Existing `game` and `world` capabilities provide some player/world context, but weather and environment snapshots/events need a verified client-side source before being exposed.

## Part A — Player-only animation compatibility

### Required renderer work

1. Add a player-model animation interface at the renderer boundary; do not expose arbitrary renderer internals to mods.
2. Resolve the local player's model parts/bones by stable identifiers.
3. Read a bounded animation definition from a resource-pack-owned path.
4. Evaluate keyframes with interpolation and a deterministic animation clock.
5. Apply the resulting transforms after the vanilla pose is prepared and before the player model is drawn.
6. Restore the vanilla pose every frame before applying the next animation, preventing transform accumulation.
7. Fall back to the vanilla player model on missing, malformed, unsupported, or over-budget assets.

### Initial compatibility target

- Local player only.
- Player geometry and animation files only.
- Idle, walk, run, sneak, swim, jump/fall, and use-item states only when the game exposes reliable state signals.
- No mob models, entity-wide animation system, custom shaders, or arbitrary native code.
- Do not claim full Fresh Animations compatibility until its exact player asset format and required bone names are validated against the supplied pack.

### Validation

- Verify the actual Fresh Animations player files and license/redistribution conditions.
- Build a minimal fixture pack with one idle and one locomotion animation.
- Test first-person and third-person views, armor, held items, skin changes, death/respawn, world transitions, and resource-pack reload.
- Confirm no pose drift after repeated state changes.

## Part B — Environmental Audio API

### Proposed mod-facing operations

Add a separately permissioned `sound` capability to the WIT contract and Wasmtime host. Keep the API small:

- Play a mod-owned sound asset.
- Stop only a sound instance created by that same mod.
- Set bounded volume, pitch, looping, and optional world position.
- Receive a completion event only if the existing mod event lifecycle can deliver it safely.

### Environment inputs

Expose a bounded client snapshot/event feed rather than unrestricted world scanning:

- Current dimension and biome identifier.
- Weather state (clear/rain/thunder) from the authoritative client world state.
- Day/night phase from the existing world clock.
- Player underwater state, when reliably available.
- World load/unload and weather transitions.

Represent unavailable values explicitly. Do not infer weather or biome from visual effects.

### Audio safety and lifecycle

- Mod-owned asset namespace; reject traversal and arbitrary filesystem paths.
- Validate format, duration, decoded size, volume/pitch ranges, and per-mod concurrent voice count.
- Enforce per-mod playback ownership; a mod cannot stop vanilla or another mod's audio.
- Respect master/category volume, pause, audio focus loss, world unload, and app lifecycle.
- Stop and release every mod-owned voice when the mod is disabled or its world context ends.
- Use the existing ambient category where appropriate; do not bypass the game's volume controls.

## Implementation order

1. **Source audit and test baseline:** locate the actual player renderer/model pose path and trace how the client audio engine is owned and called.
2. **Environmental audio host:** add WIT interface, manifest capability/permission, Wasmtime linker implementation, asset validation, cleanup, and host tests.
3. **Environment snapshot/events:** connect weather and player state from authoritative client state; add tests for title screen and world transitions.
4. **Player renderer hook:** add a narrow, renderer-owned local-player pose extension with vanilla fallback.
5. **Animation data reader/evaluator:** implement only the verified player animation format and tested state mapping.
6. **Android ARM64 build and device testing:** run the existing Android development workflow and validate on-device before publishing any compatibility claim.

## Non-goals

- Fabric, Forge, or NeoForge compatibility.
- Running arbitrary Java/Mixin code.
- Mob animations in the first release.
- A claim of complete Fresh Animations support before format and renderer tests pass.
- Shipping third-party Fresh Animations assets inside this repository without checking their license.

## Definition of done

A feature is complete only when the WIT/API and host implementation are wired, permissions and limits are tested, Android ARM64 builds, and the feature is exercised on a real device. A design document or successful desktop compile alone is not a release.
