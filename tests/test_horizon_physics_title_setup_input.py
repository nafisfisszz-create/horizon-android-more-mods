from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def test_horizon_physics_title_setup_is_opt_in():
    runtime = (ROOT / "mods/official/horizon-physics/mixins/horizon_physics/runtime.rs").read_text(encoding="utf-8")
    visible = runtime.split("pub fn setup_visible() -> bool {", 1)[1].split("\n}", 1)[0]
    assert "SETUP_REQUESTED.load(Ordering::Relaxed)" in visible
    assert "return false;" in visible
    assert "Some((s, Some(_))) if !s.setup" in visible


def test_explicit_quick_setup_request_resets_the_wizard_page():
    runtime = (ROOT / "mods/official/horizon-physics/mixins/horizon_physics/runtime.rs").read_text(encoding="utf-8")
    click = runtime.split("pub fn setting_click(i: usize) {", 1)[1].split("\n}", 1)[0]
    assert "if i == 19" in click
    assert "SETUP_REQUESTED.store(true, Ordering::Relaxed);" in click
    assert "with_setup(|s| s.page = 0);" in click


def test_finishing_quick_setup_releases_title_input_gate():
    runtime = (ROOT / "mods/official/horizon-physics/mixins/horizon_physics/runtime.rs").read_text(encoding="utf-8")
    finish = runtime.split("pub fn setup_finish() {", 1)[1].split("\n}", 1)[0]
    assert "SETUP_REQUESTED.store(false, Ordering::Relaxed);" in finish
    assert "update_settings(|st| st.setup = true);" in finish


def test_title_draw_passes_through_when_quick_setup_was_not_requested():
    hook = (ROOT / "mods/official/horizon-physics/mixins/horizon_physics/setup_draw.rs").read_text(encoding="utf-8")
    assert "if !crate::native_horizon_physics::setup_visible()" in hook
    assert "return ::core::ops::ControlFlow::Continue(());" in hook


def test_title_click_passes_through_when_quick_setup_was_not_requested():
    hook = (ROOT / "mods/official/horizon-physics/mixins/horizon_physics/setup_click.rs").read_text(encoding="utf-8")
    assert "if !hp::setup_visible()" in hook
    assert "return ::core::ops::ControlFlow::Continue(());" in hook
