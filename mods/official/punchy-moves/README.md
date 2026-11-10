# Punchy Moves

- Native mod ID: `punchy_moves`
- Version: `1.0.8` compatibility adapter
- Scope: third-person resource-pack animation compatibility
- First-person owner: **Punchy**

Punchy Moves reads an external animation pack from the active Minecraft resource-pack overlay. Install the compatible pack in the normal `resourcepacks/` folder and enable it in **Options > Resource Packs**. Horizon reads the standard resource-pack namespace:

```text
assets/punchy_moves/animations/**/*.animation.json
assets/punchy_moves/compat/*.json
```

The compatibility adapter parses animation clips with the same native Punchy animation engine used by Punchy. It reads the pack's `defaults.animations` profile and resolves clip names only when the referenced clip exists. It supports idle, directional walking, sprint overlays, jump/landing, falling, surface/underwater swimming, elytra, climbing, crouching, attack, and mining where the profile supplies matching clips. Invalid or unsupported clips are skipped independently; if no compatible clip is available, the existing native third-person rig remains active.

Only these third-person player bones are applied: `root`, `waist`, `body`, `head`, `right_arm`, `left_arm`, `right_leg`, and `left_leg`. Face/eye/pupil tracks, item attachment tracks, camera features, and first-person transforms are ignored. First-person hand/item animation remains exclusively owned by **Punchy**. Punchy Moves does not add eye adjustments, Elytra trails, or the legacy feature settings page.

No third-party animation assets are bundled, copied, rewritten, converted, or installed by this module. The external resource pack remains a separate user-installed pack and must be obtained and licensed independently. Disabling or removing it simply returns Punchy Moves to the native third-person fallback rig.

This is a third-person animation compatibility adapter, not full Fabric parity. Features outside the supported player-model bones and mapped profile states are not claimed as implemented.
