# Fabric compatibility in Horizon (Rust)

This is a staged compatibility project, not a JVM implementation.

## First milestone: metadata and dependency analysis

The `horizon-port-pack::fabric` module reads `fabric.mod.json` from a Fabric JAR and extracts mod identity, environment, version, dependency declarations, entrypoint class names, Mixin configuration names and access-widener declarations.

Commands:

- `horizon-port-pack fabric-check <mod.jar>`: inspect one JAR and list Java-only blockers.
- `horizon-port-pack fabric-scan <folder>`: scan all `.jar` files directly inside a folder, then report duplicate IDs, missing required dependencies, declared conflicts, dependency cycles and platform requirements.\n- `horizon-port-pack fabric-bridge <mod.jar>`: compare declared Fabric API module dependencies with a small, explicitly documented set of Horizon native hooks.\n- `horizon-port-pack fabric-bytecode <mod.jar>`: scan JVM class constant pools for Fabric API, Fabric Loader, Mixin and Minecraft symbolic references without executing classes.

The resolver produces a dependency-first order. Minecraft, Java and Fabric Loader requirements are reported separately for a later Horizon runtime adapter.

The bytecode scanner can reveal direct class references even when a dependency is not declared in `fabric.mod.json`; it is a constant-pool scan, not a decompiler, and may miss reflection or generated runtime references.\n\nThe bridge assessment currently recognizes lifecycle, command, item, interaction, rendering and networking API modules. Its labels are `partial`, `limited` or `unsupported`; these are not compatibility guarantees. For example, Horizon's health-HUD hook is not a replacement for Fabric's full rendering API. See [fabric-api-bridge.md](fabric-api-bridge.md) for the mapping and gaps.\n\n**Version expressions are preserved but not evaluated yet.** A successful metadata scan does not mean a Java mod can run in Horizon. The folder scan is non-recursive and does not extract or execute JAR contents.

## Java code boundary

Fabric entrypoints, Mixin transformations and access wideners target Java classes and bytecode. Horizon is a Rust implementation and does not provide the same Minecraft Java classpath or JVM contract. This milestone reports these as porting blockers; it never executes classes from an untrusted JAR.

The next compatibility work must map individual Fabric API surfaces to Horizon-native hooks and implement selected mods as native Horizon mods or supported sandboxed components. Arbitrary Fabric JAR execution is not claimed.

## Source and licence note

The uploaded Fabric Loader source is an architectural reference. This implementation is independently written against Fabric's metadata format and does not copy Fabric Loader implementation code. Preserve upstream notices and Apache-2.0 attribution if future work adapts code from that project.


## HMOD porting workflow

The supported content-only path and the Java behavior boundary are documented in [fabric-hmod-workflow.md](fabric-hmod-workflow.md). A successful scan is not an HMOD build; resource/data files must be selected and adapted, and Java behavior must be ported separately.
