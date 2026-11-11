//! Static mapping from commonly used Fabric API modules to documented Horizon hooks.
//! This is an assessment aid only: it does not translate or execute Java code.

use crate::fabric::FabricMetadata;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BridgeAssessment {
    pub fabric_module: &'static str,
    pub horizon_hooks: &'static str,
    pub status: &'static str,
    pub notes: &'static str,
}

const KNOWN: &[BridgeAssessment] = &[
    BridgeAssessment {
        fabric_module: "fabric-lifecycle-events-v1",
        horizon_hooks: "3, 4, 23",
        status: "partial",
        notes: "World lifecycle and server/level tick observations exist; callback timing and payloads still need an explicit port.",
    },
    BridgeAssessment {
        fabric_module: "fabric-command-api-v2",
        horizon_hooks: "8",
        status: "partial",
        notes: "Server command registration and command execution exist; Brigadier argument trees and client command behavior are not equivalent.",
    },
    BridgeAssessment {
        fabric_module: "fabric-item-api-v1",
        horizon_hooks: "9, 13",
        status: "partial",
        notes: "Server-side player item access and item-use hooks exist; item registration and all Fabric item callbacks are not covered by these hooks.",
    },
    BridgeAssessment {
        fabric_module: "fabric-events-interaction-v0",
        horizon_hooks: "13, 14, 21, 22",
        status: "partial",
        notes: "Selected item, block, player-interaction and world-mutation events have native hooks; event semantics must be checked individually.",
    },
    BridgeAssessment {
        fabric_module: "fabric-rendering-v1",
        horizon_hooks: "27 (HUD only)",
        status: "limited",
        notes: "A health-HUD replacement hook exists, but it is not a general world, entity, block or item rendering API.",
    },
    BridgeAssessment {
        fabric_module: "fabric-networking-api-v1",
        horizon_hooks: "none documented",
        status: "unsupported",
        notes: "No equivalent general custom-packet channel is documented in the inspected Horizon hook API.",
    },
];

/// Return bridge notes for Fabric API modules explicitly declared by this mod.
pub fn assess(metadata: &FabricMetadata) -> Vec<BridgeAssessment> {
    let declared: std::collections::BTreeSet<&str> =
        metadata.depends.iter().map(|dep| dep.mod_id.as_str()).collect();
    KNOWN.iter()
        .copied()
        .filter(|entry| declared.contains(entry.fabric_module))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_declared_fabric_api_modules() {
        let metadata = FabricMetadata::parse(
            r#"{"id":"sample","version":"1","depends":{"fabric-lifecycle-events-v1":"*","fabric-networking-api-v1":"*","minecraft":"*"}} "#,
        ).unwrap();
        let result = assess(&metadata);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].fabric_module, "fabric-lifecycle-events-v1");
        assert_eq!(result[1].status, "unsupported");
    }
}
