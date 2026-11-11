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

## Verified Fresh Animations player-pack requirements

Research note (2026-11-11):

- The base Fresh Animations pack does not animate the player. Player support is a separate Player Extension/add-on.
- The Player Extension is documented as requiring the EMF + ETF route; OptiFine does not provide custom player model support.
- The EMF-style model format uses .jem / .jpm model files and expression-driven animation rules. This is not equivalent to a simple keyframe JSON format.
- Consequently, implementing only a generic keyframe player animation reader would not provide Fresh Animations Player Extension compatibility.
- Horizon needs a player-specific model loading and animation evaluation path compatible with the actual EMF player extension format, plus the required player state variables/functions used by its expressions.
- Do not redistribute Fresh Animations or Player Extension assets in this repository. Users should supply their own legally obtained resource-pack files for compatibility testing.

References:
- Fresh Animations official overview: https://fresh-animations.com/
- Player animation explanation: https://fresh-animations.com/player/
- EMF/ETF and CEM format explanation: https://fresh-animations.com/optifine/

### Revised implementation gate

Before coding the parser, inspect representative Player Extension files supplied for local compatibility testing, including player .jem, .jpm, and animation expressions. Without representative real files, parser compatibility cannot be verified and should not be advertised. The implementation must be built against Horizon's actual player renderer path, not guessed from the mod API.


## Supplied Player Extension v1.1 archive audit

Inspected locally supplied archive `FA+Player-v1.1.zip` (2026-11-11). This is a format audit only; the archive has not been copied into the repository.

- The pack contains separate `player.jem` and `player_slim.jem` entry points, plus cape and elytra model files.
- The player entry points compose five `.jpm` animation modules: variables, idle, equipment, movement, and first-person. The archive also includes version-specific `a_player_variables.jpm` variants under `21-2/` and `21-5/` directories.
- The main player model references `player_face.jpm`, but that file is not present in this supplied archive. It appears to be an external dependency or omitted companion asset; this must be resolved from the user's complete pack set before claiming the model is self-contained.
- Across the supplied JEM/JPM files, the audit counted 725 expression assignments and about 96 KB of expression text. The expressions use at least these functions: `abs`, `between`, `clamp`, `cos`, `exp`, `if`, `in`, `max`, `min`, `nbt`, `pow`, `random`, `round`, `sin`, `sqrt`, `todeg`, `torad`, and `wraprad`.
- The expressions depend on a broad runtime state surface: player movement and pose, limb/head/body transforms, held-item and equipment checks, first-person state, swimming/underwater/gliding/climbing, timing variables, and item NBT predicates. A parser alone is insufficient: Horizon must provide compatible variable semantics, expression evaluation, model hierarchy transforms, and the required player render integration.
- The regular and slim player entry files currently have the same file size and animation structure in this archive; both still need independent geometry/skin-layout validation.

### Consequence for implementation

Do not start by translating these expressions into a hand-written list of hardcoded poses. The correct compatibility path is a bounded CEM/EMF subset: parse JEM/JPM hierarchy and animation maps, implement a tested expression parser/evaluator for the functions and operators actually used, then map each referenced runtime variable to authoritative Horizon player state. Unknown functions, missing dependencies, malformed expressions, or unsupported model features must trigger a safe vanilla fallback. First resolve the missing `player_face.jpm` dependency and inspect the actual Horizon player renderer before selecting implementation files.


## Implementation progress — CEM expression stage

The renderer crate now contains `player_animation.rs` with:
- bounded `.jem/.jpm` JSON loading and preservation of unrecognized fields;
- extraction of expression strings and referenced `.jpm` module names;
- a bounded numeric expression evaluator with arithmetic, comparisons, boolean operators, ternary selection, and a limited function set (`sin`, `cos`, `sqrt`, `clamp`, `if`, `lerp`, `atan2`, and others).

The evaluator is an isolated stage. It is **not yet a complete OptiFine/EMF expression implementation**: pack-specific functions such as `nbt(...)` require a real entity/NBT context and are deliberately rejected rather than guessed. The evaluator is not yet called by the player renderer.

## Verified existing player-render path

The game already represents player/mannequin avatars through the mob model pipeline:
- `crates/horizon-client/src/mobs/render.rs` selects the player or slim player body model and skin.
- `crates/horizon-client/src/mobs.rs` stores model parts as a `Part` tree and exposes `render_model`.
- `crates/horizon-client/src/mobs/anim.rs` builds per-frame `Poses` from the existing model tree.
- `crates/horizon-client/src/game.rs` has a separate local-player model geometry path and a guest-player geometry path.

The remaining integration is to translate the CEM cuboid hierarchy into the existing `Part` representation, attach the correct wide/slim model to each player avatar, construct the expression context from that avatar's render state, and apply evaluated values to `Poses` before geometry submission. This is not wired up yet. No APK or installable mod is produced by these commits.


## CEM pose evaluation stage

The renderer-side module now also exposes a pose-binding stage. Given a base pose for each named part, a map of expression bindings such as `head.rx` or `body.ty`, and a per-frame variable context, it evaluates the numeric values and returns updated Horizon-style 9-component poses. Unassigned components retain their base values. Missing part names, unknown channels, invalid expressions, and extreme transform values are rejected.

This is still a bridge API, not a live player integration. The actual Fresh Animations files must first be mapped from their OptiFine CEM animation schema into these bindings; the renderer must then consume the returned poses. The exact pack schema and all required expression functions still need validation against the supplied Player Extension archive. Face-model resources remain out of scope.
