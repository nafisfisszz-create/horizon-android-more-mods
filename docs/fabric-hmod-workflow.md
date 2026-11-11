# Fabric JAR to Horizon HMOD workflow

This document defines the supported conversion boundary. A Fabric JAR is an input to analysis and content extraction; it is not an executable input to Horizon.

## What can become an HMOD

Horizon content mods use API `0.2` and contain a `horizon-mod.toml`, `assets/` and `data/`. They are data-only: no Java entrypoints, Fabric Loader, Mixins, access wideners or arbitrary bytecode are carried into the HMOD.

A Fabric JAR may contain portable resources under:

- `assets/<namespace>/...` — textures, models, language files, sounds and other assets.
- `data/<namespace>/...` — recipes, loot tables, tags, worldgen and other data.

The presence of a file does not prove Horizon supports its schema or behavior. Horizon's content checker is the authority for the resulting HMOD. In particular, Horizon-specific registries under `data/<id>/horizon/` must use Horizon's documented schema; Fabric registry JSON is not automatically equivalent.

## Required workflow

1. Run `fabric-check <mod.jar>` to read `fabric.mod.json` and identify declared Java entrypoints, Mixins and access wideners.
2. Run `fabric-bytecode <mod.jar>` to inventory symbolic references to Fabric, Mixin and Minecraft classes. This is a static constant-pool scan, not execution or full decompilation.
3. Run `fabric-bridge <mod.jar>` to see whether declared Fabric API modules have documented Horizon hook candidates. A partial mapping is not a working compatibility layer.
4. Inspect the JAR's `assets/` and `data/` content. Keep only content that is licensed for redistribution and whose format is supported by Horizon.
5. To create a first-pass data-only package, run `horizon-port-pack fabric-extract <mod.jar> <out.hmod>`. It copies eligible `assets/` and `data/` entries into an API 0.2 content-only HMOD, skips reserved namespaces except vanilla/convention tag additions, and refuses to overwrite an existing output. It does not adapt unsupported JSON schemas.
6. Run `cargo horizon-mod check <out.hmod>` using the Horizon SDK tooling. Fix every validation error, inspect the extracted files, then test the mod in the target Horizon client.
7. If the original mod has behavior implemented in Java, port that behavior separately to Horizon's supported native mod interfaces. A content-only HMOD cannot reproduce Java callbacks, Mixins, networking handlers or arbitrary gameplay logic.

## What the current Fabric tools do not do

- They do not execute Java bytecode or start Fabric Loader.
- They do not translate Java classes into Rust or WebAssembly.
- They do not implement missing Fabric API modules.
- They do not guarantee that arbitrary Fabric `assets/` or `data/` files pass Horizon's content validation.
- `fabric-extract` emits a first-pass content-only HMOD, not a fully ported or guaranteed install-ready mod.

The output of the current commands is an analysis report. HMOD creation happens only after content has been selected/adapted and, where necessary, Java behavior has been ported. Never rename a `.jar` to `.hmod`.
