# Fabric API bridge assessment

This table maps selected Fabric API modules to **documented** Horizon-native hook points. It is a porting guide, not an adapter and not a claim that a Fabric mod can run unchanged.

| Fabric API module | Horizon hook API | Assessment | Important gap |
|---|---:|---|---|
| `fabric-lifecycle-events-v1` | 3, 4, 23 | Partial | World lifecycle and server/level tick observations exist, but callback payloads and timing need individual translation. |
| `fabric-command-api-v2` | 8 | Partial | Server command registration/execution exists; Brigadier trees and client-side command behavior are not equivalent. |
| `fabric-item-api-v1` | 9, 13 | Partial | Player item access and selected item-use hooks exist; this does not provide all item registration and callback APIs. |
| `fabric-events-interaction-v0` | 13, 14, 21, 22 | Partial | Selected item, block, player interaction and world-mutation events have hooks; semantics must be checked event by event. |
| `fabric-rendering-v1` | 27 (health HUD only) | Limited | A health-HUD replacement hook exists, not a general renderer for world, entity, block or item content. |
| `fabric-networking-api-v1` | None documented | Unsupported | The inspected hook API does not document a general custom-packet channel. |

Hook API numbers refer to `docs/native-mods.md` in this repository. The relevant hooks are implemented as Rust native mods, not as Fabric Java interfaces.

## Analyzer command

```sh
horizon-port-pack fabric-bridge path/to/mod.jar
```

The command reads only `fabric.mod.json` and reports recognized modules listed in that mod's direct `depends` object. It does not inspect bytecode, infer transitive dependencies, or discover API calls hidden behind reflection. A module absent from the report may still be used indirectly.

## Porting rule

For each required Fabric callback:

1. Find the exact callback and its expected side (client or server).
2. Identify a Horizon hook with equivalent timing, data and cancellation semantics.
3. If no equivalent exists, add a deliberate Horizon hook/API proposal rather than silently approximating behavior.
4. Reimplement the feature in Rust as an `.hnmod` or in the supported sandboxed mod format when its capabilities are sufficient.
5. Test against the exact Horizon revision and Android target.

A Fabric dependency match is only a lead for manual porting. Java entrypoints, Mixins, access wideners, bytecode, and Java-only libraries still need replacement. No arbitrary Fabric JAR execution is provided.
