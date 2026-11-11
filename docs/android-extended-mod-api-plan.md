# Horizon Android Extended

This document tracks the implementation plan for expanding Horizon's Android mod API.
It is a development plan, not a claim that the listed capabilities already exist.

## Target

Extend Horizon's existing sandboxed WebAssembly mod system on Android. Keep
`.hmod` as the installable format and preserve the manifest capability model:
a mod must declare every host capability it imports, and the player must be
able to review those permissions before enabling it.

This project does **not** promise binary compatibility with Fabric, Forge, or
NeoForge Java mods. Their Java runtime and Mixin environment are not provided
by Horizon.

## Confirmed starting point

- Android 64-bit builds include the Wasmtime `.hmod` runtime.
- The mod SDK is generated from `crates/horizon-mod-sdk/wit/horizon.wit`.
- Existing client capabilities include game/player and clock reads, chat,
  commands, HUD, storage, UI, input, rendering, recipes, and browser access.
- The current client WIT `game` interface does not expose biome, weather,
  nearby entity queries, or sound playback.
- The renderer already contains an audio engine and sound-instance machinery.
  That is not, by itself, a mod-facing audio API.
- `.hnmod` is a source/native-mod path and is not currently supported by the
  Android build. It must not be advertised as working on Android.

## Compatibility rules

1. Add capabilities additively. Existing `.hmod` files must continue to load.
2. Gate each new capability through the existing manifest permission flow.
3. Keep host calls bounded and cheap; do not expose per-frame polling for
   information that can be delivered as events.
4. Validate all identifiers, paths, asset sizes, event queue sizes, and audio
   limits on the host side. A mod is untrusted input.
5. Audio must respect the game's master/category volume, pause, focus loss,
   world unload, and device lifecycle.
6. Do not execute arbitrary Java bytecode or silently claim Fabric/Forge
   compatibility.

## Implementation phases

### Phase 0 — baseline and build validation

- Keep the Android ARM64 development workflow usable from the fork.
- Record the exact branch, API version, and Android build constraints before
  changing generated WIT bindings.
- Run the relevant SDK tests and Android build after each host/API change.

### Phase 1 — world context API

Introduce a separately permissioned world-context capability. Prefer one
bounded snapshot per tick containing only data the engine can safely provide:
dimension, biome identifier, world time, weather state, and whether the local
player is in water or under open sky. Missing world/player data must be
represented explicitly, not guessed.

Do not expose unrestricted world scans or arbitrary block/entity traversal.

### Phase 2 — sound API

Introduce a separately permissioned sound capability with a small surface:
- play a registered or mod-owned sound;
- stop a sound owned by that mod;
- set bounded volume, pitch, loop, and optional world position;
- optionally receive a completion/stopped event.

Sound assets must be loaded through a controlled mod asset path and validated
for supported format, duration, decoded size, and per-mod voice limits. The
host owns playback and cleanup. A mod must not be able to stop vanilla sounds
or another mod's sounds.

### Phase 3 — event-driven world hooks

Add bounded events for world load/unload, dimension change, weather change,
and relevant player-state transitions. Avoid requiring every mod to poll the
game every frame.

### Phase 4 — mod management and diagnostics

Improve dependency declarations, load-order diagnostics, conflict reporting,
and per-mod logs without weakening sandboxing or existing opt-in permissions.

### Phase 5 — Android verification

Test on an actual ARM64 Android device, including:
- install, enable, disable, and remove;
- world exit/re-entry and app pause/resume;
- audio focus loss and output-device changes;
- malformed manifests/assets and oversized event requests;
- multiple mods competing for audio and storage quotas.

## First implementation target

Implement the world-context and sound capabilities as additive WIT interfaces,
then wire them through the Wasmtime host and SDK. Do not add WIT declarations
alone and call the feature complete: a capability is complete only when the
host implements it, permission checks are enforced, tests pass, and an Android
ARM64 build succeeds.
