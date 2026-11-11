# Fabric compatibility in Horizon (Rust)

This is a staged compatibility project, not a JVM implementation.

## First milestone: metadata and dependency analysis

The `horizon-port-pack::fabric` module reads `fabric.mod.json` from a Fabric JAR and extracts mod identity, environment, version, dependency declarations, entrypoint class names, Mixin configuration names and access-widener declarations.

Commands:

- `horizon-port-pack fabric-check <mod.jar>`: inspect one JAR and list Java-only blockers.
- `horizon-port-pack fabric-scan <folder>`: scan all `.jar` files directly inside a folder, then report duplicate IDs, missing required dependencies, declared conflicts, dependency cycles and platform requirements.

The resolver produces a dependency-first order. Minecraft, Java and Fabric Loader requirements are reported separately for a later Horizon runtime adapter.

**Version expressions are preserved but not evaluated yet.** A successful metadata scan does not mean a Java mod can run in Horizon. The folder scan is non-recursive and does not extract or execute JAR contents.

## Java code boundary

Fabric entrypoints, Mixin transformations and access wideners target Java classes and bytecode. Horizon is a Rust implementation and does not provide the same Minecraft Java classpath or JVM contract. This milestone reports these as porting blockers; it never executes classes from an untrusted JAR.

The next compatibility work must map individual Fabric API surfaces to Horizon-native hooks and implement selected mods as native Horizon mods or supported sandboxed components. Arbitrary Fabric JAR execution is not claimed.

## Source and licence note

The uploaded Fabric Loader source is an architectural reference. This implementation is independently written against Fabric's metadata format and does not copy Fabric Loader implementation code. Preserve upstream notices and Apache-2.0 attribution if future work adapts code from that project.
