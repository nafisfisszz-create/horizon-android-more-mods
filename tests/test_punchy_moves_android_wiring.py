from pathlib import Path
import json
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "mods/official/punchy-moves"


def test_punchy_moves_metadata_is_animation_only_and_tracks_108_source():
    meta = tomllib.loads((PACKAGE / "PORT.toml").read_text(encoding="utf-8"))
    assert meta["id"] == "punchy_moves"
    assert meta["name"] == "Punchy Moves"
    assert meta["version"] == "1.0.8"
    assert meta["android_status"] == "external_resource_pack_compatibility"
    assert meta["first_person_owner"] == "punchy"
    assert meta["includes_face_or_pupil_features"] is False
    assert meta["includes_elytra_trails"] is False
    assert meta["includes_feature_settings_page"] is False

    native = json.loads((PACKAGE / "horizon-native.json").read_text(encoding="utf-8"))
    assert native["id"] == "punchy_moves"
    assert native["version"] == "1.0.8"
    assert native["scope"] == "third_person_resource_pack_compatibility"
    assert native["face_pupil_adjustments"] is False
    assert native["first_person_owner"] == "punchy"


def test_punchy_moves_native_paths_and_third_person_animation_hooks_resolve():
    paths = (
        ROOT / "crates/horizon-client/src/game.rs",
        ROOT / "crates/horizon-client/src/mobs.rs",
    )
    for source in paths:
        content = source.read_text(encoding="utf-8")
        for relative in re.findall(r'^\s*#\[path\s*=\s*"([^"]+)"\]', content, re.MULTILINE):
            target = (source.parent / relative).resolve()
            assert target.is_file(), f"{source.relative_to(ROOT)}: missing {relative}"

    game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
    mobs = (ROOT / "crates/horizon-client/src/mobs.rs").read_text(encoding="utf-8")
    anim = (ROOT / "crates/horizon-client/src/mobs/anim.rs").read_text(encoding="utf-8")
    render = (ROOT / "crates/horizon-client/src/mobs/render.rs").read_text(encoding="utf-8")
    bundled = (ROOT / "crates/horizon-client/src/game/android_bundled.rs").read_text(encoding="utf-8")
    runtime = (PACKAGE / "mixins/punchy_moves/runtime.rs").read_text(encoding="utf-8")
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")

    assert "mod punchy_moves;" in game
    assert "punchy_moves_trails" not in game
    assert "pub mod punchy_moves;" in mobs
    assert "super::punchy_moves::pose_start(m, s);" in anim
    assert "super::punchy_moves::pose(p);" in anim
    assert "let mut base = super::punchy_moves::body(m, alpha, base);" in render
    assert "super::punchy_moves::key" in bundled
    assert "super::punchy_moves::listed" in bundled
    assert "super::punchy_moves::frame" in bundled
    assert "super::punchy_moves::settings_page" not in bundled
    assert "super::punchy_moves::settings_click" not in bundled
    assert "punchy_moves::world" not in game

    assert 'pub(crate) const ID: &str = "punchy_moves";' in runtime
    assert 'const NAME: &str = "Punchy Moves";' in runtime
    assert 'const VERSION: &str = "1.0.8";' in runtime
    assert "inspect" in runtime and "twirl_left" in runtime and "twirl_right" in runtime
    assert "settings_page" not in runtime and "settings_click" not in runtime
    assert "KEY_TOGGLE" not in runtime and "struct Config" not in runtime
    assert "punchy_moves::world" not in runtime

    # The built-in native rig is third-person only; the first-person renderer stays with Punchy.
    assert "let state = super::anim::state(m, alpha);" in rig
    assert "p.fall_flying" in runtime
    assert "punchy_moves::hand_pose" not in game
    punchy_runtime = (ROOT / "mods/official/punchy/mixins/punchy/runtime.rs").read_text(encoding="utf-8")
    punchy_engine = (ROOT / "mods/official/punchy/mixins/punchy/engine/engine.rs").read_text(encoding="utf-8")
    assert "hand_motion_input(game)" in punchy_runtime
    assert "motion_delta(&s, false" in punchy_runtime
    assert "motion_delta(&s, true" in punchy_runtime
    assert 'pub mod hand_motion;' in punchy_engine
    assert "first_person" not in runtime
    assert "first_person_amount" not in runtime
    assert "settings" not in runtime.lower()


def test_punchy_moves_has_no_legacy_settings_trails_or_face_component():
    runtime = (PACKAGE / "mixins/punchy_moves/runtime.rs").read_text(encoding="utf-8")
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    game = (ROOT / "crates/horizon-client/src/game.rs").read_text(encoding="utf-8")
    readme = (PACKAGE / "README.md").read_text(encoding="utf-8")

    assert "AMOUNTS" not in runtime
    assert "TrailConfig" not in runtime
    assert "settings_page" not in runtime
    assert "settings_click" not in runtime
    assert "pub fn settings()" not in rig
    assert "pub fn set_settings" not in rig
    assert "pub enabled:" not in rig
    assert "pub others:" not in rig
    assert "struct Settings" not in rig
    assert "Settings::default()" not in rig
    assert "struct Config" not in runtime
    assert "trails.rs" not in game
    assert "punchy_moves_trails" not in game
    assert not (PACKAGE / "mixins/punchy_moves/trails.rs").exists()
    assert not (PACKAGE / "original/livingmotion-1.2.0.hnmod").exists()
    assert "pupil" in readme.lower()
    assert "first-person" in readme
    assert "elytra trails" in readme.lower()


def test_android_body_transform_binding_is_mutable_before_matrix_updates():
    source = (ROOT / "crates/horizon-client/src/mobs/render.rs").read_text(encoding="utf-8")
    lines = source.splitlines()
    needle = "let mut base = super::punchy_moves::body(m, alpha, base);"
    assert any(line.strip() == needle for line in lines)
    index = next(i for i, line in enumerate(lines) if line.strip() == needle)
    following = "\n".join(lines[index + 1 : index + 5])
    assert "base *= glam::Mat4::from_scale" in following
    assert "base *= species_scale(m, alpha);" in following



def test_external_punchy_moves_pack_reads_active_resource_overlay_only():
    runtime = (PACKAGE / "mixins/punchy_moves/runtime.rs").read_text(encoding="utf-8")
    bundled = (ROOT / "crates/horizon-client/src/game/android_bundled.rs").read_text(encoding="utf-8")
    assert 'native_data_tree("assets/punchy_moves/animations")' in runtime
    assert 'native_data_tree("assets/punchy_moves/compat")' in runtime
    assert "punchy_engine::load_clips" in runtime
    assert "super::punchy_moves::resources_reloaded();" in bundled
    assert "bundles_third_party_animation_assets = false" in (PACKAGE / "PORT.toml").read_text(encoding="utf-8")
    assert not list(PACKAGE.rglob("*.animation.json"))


def test_external_pack_adapter_whitelists_only_third_person_player_bones():
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    block = rig.split("fn supported_part(bone: &str)", 1)[1].split("fn apply_resource_clip", 1)[0]
    for bone in ("root", "waist", "body", "head", "right_arm", "left_arm", "right_leg", "left_leg"):
        assert f'"{bone}"' in block
    assert "_ => None" in block
    executable = "\n".join(line.split("//", 1)[0] for line in block.splitlines())
    assert "pupil" not in executable.lower()
    assert "first_person" not in executable.lower()
    assert "right_item" not in block and "left_item" not in block


def test_external_pack_compat_supports_directional_motion_and_reload_fallbacks():
    runtime = (PACKAGE / "mixins/punchy_moves/runtime.rs").read_text(encoding="utf-8")
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    assert '"walking", "directions", "backward", "clip"' in runtime
    assert '"walking", "directions", "left", "clip"' in runtime
    assert '"walking", "directions", "right", "clip"' in runtime
    assert '"jump", "moving", "landing"' in runtime
    assert "clear_resource_pack_animations" in runtime
    assert "pub(crate) fn resources_reloaded()" in runtime
    assert "walk_backward" in rig and "walk_backward_reverse" in rig and "jump_landing" in rig


def test_punchy_engine_is_a_direct_android_dependency_of_horizon_client():
    cargo = (ROOT / "crates/horizon-client/Cargo.toml").read_text(encoding="utf-8")
    assert "[target.'cfg(target_os = \"android\")'.dependencies]" in cargo
    assert 'punchy-engine = { path = "../../mods/official/punchy/mixins/punchy/engine" }' in cargo



def test_resource_pack_body_tracks_have_single_precedence_and_keep_native_body_motion():
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    assert '["body", "waist", "root"]' in rig
    assert 'body_bone != Some(bone.as_str())' in rig
    assert 'fn selected_pack_has_body_track' in rig
    assert 'selected_pack_has_supported_tracks' not in rig
    assert 'selected_pack_has_body_track(set, &state, &o)' in rig


def test_pack_attack_and_mining_clip_fallbacks_cover_both_hands():
    runtime = (PACKAGE / "mixins/punchy_moves/runtime.rs").read_text(encoding="utf-8")
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    assert 'default(&["punch", "punch_right", "sword_attack"])' in runtime
    assert 'default(&["punch_left", "sword_attack"])' in runtime
    assert 'default(&["mine", "mine_right", "generic_mining_right"])' in runtime
    assert 'default(&["mine_left", "generic_mining_left"])' in runtime
    assert 'if o.main_left' in rig


def test_external_pack_adapter_selects_underwater_animation_without_face_tracks():
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    runtime = (PACKAGE / "mixins/punchy_moves/runtime.rs").read_text(encoding="utf-8")
    assert "pub underwater: bool" in rig
    assert "m.fluids.eye_in_water" in rig
    assert 'value_string(animations, &["swimming", "underwater"])' in runtime
    assert "set.bindings.swim_underwater" in rig


def test_removed_settings_derive_does_not_attach_to_held_enum():
    rig = (PACKAGE / "mixins/punchy_moves/rig.rs").read_text(encoding="utf-8")
    expected = "/// What a hand holds, as far as the animations care.\n#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]\npub enum Held"
    assert expected in rig
    assert "Fixed animation profile. Punchy Moves" not in rig
