"""Regression tests for the Android workflows that are active in this repository."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class AndroidWorkflowTests(unittest.TestCase):
    def test_active_workflows_match_packaging_templates(self):
        pairs = {
            ".github/workflows/android-source.yml": "packaging/android/source-checks.yml",
            ".github/workflows/android-dev.yml": "packaging/android/dev-builds.yml",
        }
        for active, template in pairs.items():
            with self.subTest(workflow=active):
                self.assertEqual((ROOT / active).read_bytes(), (ROOT / template).read_bytes())

    def test_source_workflow_runs_repository_checks(self):
        text = (ROOT / ".github/workflows/android-source.yml").read_text(encoding="utf-8")
        self.assertIn("branches: [android]", text)
        self.assertIn("python3 tools/check_public_tree.py --history", text)
        self.assertIn("python3 -m unittest discover -s tests -p 'test_android*.py'", text)
        self.assertIn("cmp .github/workflows/android-source.yml packaging/android/source-checks.yml", text)
        self.assertIn("cmp .github/workflows/android-dev.yml packaging/android/dev-builds.yml", text)

    def test_android_dev_workflow_supports_arm64_dispatch_build(self):
        text = (ROOT / ".github/workflows/android-dev.yml").read_text(encoding="utf-8")
        for value in (
            "name: Build Android APK",
            "workflow_dispatch:",
            "build_type:",
            "Dev Builds",
            "Public Builds",
            "abi:",
            "arm64-v8a",
            "x86_64",
            "armeabi-v7a",
            "x86",
            "Install pinned Rust toolchain and Android targets",
            "Prepare pinned Minecraft engine data",
            "Build development APK",
            "--dev-build",
            "Upload Android APK",
            "actions/upload-artifact@",
        ):
            self.assertIn(value, text)

    def test_android_dev_workflow_builds_on_player_animation_changes(self):
        text = (ROOT / ".github/workflows/android-dev.yml").read_text(encoding="utf-8")
        for path in (
            "crates/horizon-client-render/src/player_animation.rs",
            "crates/horizon-client/src/mobs/anim.rs",
            "crates/horizon-client/src/mobs/data.rs",
            "crates/horizon-client/src/mobs/render.rs",
        ):
            self.assertIn(path, text)

    def test_player_cem_workflow_runs_parser_and_pose_tests(self):
        text = (ROOT / ".github/workflows/player-cem-check.yml").read_text(encoding="utf-8")
        self.assertIn("cargo test -p horizon-client-render --lib player_animation", text)
        self.assertIn("cargo test -p horizon-client --lib --no-default-features cem_pose_integration_tests", text)


if __name__ == "__main__":
    unittest.main()
