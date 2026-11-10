"""Regression tests for deterministic Minecraft metadata in Android preparation."""
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "tools"
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

spec = importlib.util.spec_from_file_location("android_prepare_under_test", TOOLS / "android_prepare.py")
android_prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(android_prepare)


class AndroidPrepareManifestPinTests(unittest.TestCase):
    def test_download_game_uses_exact_research_lock_manifest(self):
        pins = json.loads((ROOT / "research-lock.json").read_text(encoding="utf-8"))["minecraft"]
        downloaded = []
        manifest = {
            "id": pins["version"],
            "javaVersion": {"majorVersion": pins["java_major"]},
            "downloads": {
                "client": {"sha1": pins["client_sha1"], "url": "https://example.invalid/client.jar"},
                "server": {"sha1": pins["server_sha1"], "url": "https://example.invalid/server.jar"},
            },
            "assetIndex": {"id": "test-index", "url": "https://example.invalid/index.json", "sha1": "0" * 40},
        }

        def fake_download(url, destination, checksum):
            downloaded.append((url, Path(destination), checksum))
            destination = Path(destination)
            destination.parent.mkdir(parents=True, exist_ok=True)
            if destination.name == f"{pins['version']}.json":
                destination.write_text(json.dumps(manifest), encoding="utf-8")
            elif destination.name == "test-index.json":
                destination.write_text(json.dumps({"objects": {}}), encoding="utf-8")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with mock.patch.object(android_prepare.lab, "download", side_effect=fake_download), \
                 mock.patch.object(android_prepare.lab, "client_library_artifacts", return_value=[]), \
                 mock.patch.object(android_prepare, "parallel_download", side_effect=lambda files: [
                     fake_download(*item) for item in files
                 ]):
                android_prepare.download_game(pins["version"], root)

        expected_url = (
            f"https://piston-meta.mojang.com/v1/packages/"
            f"{pins['manifest_sha1']}/{pins['version']}.json"
        )
        self.assertEqual(downloaded[0][0], expected_url)
        self.assertEqual(downloaded[0][2], pins["manifest_sha1"])
        self.assertFalse(any("version_manifest_v2" in url for url, _, _ in downloaded))


if __name__ == "__main__":
    unittest.main()
