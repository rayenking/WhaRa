"""Regression tests for the GitHub static signed-updater manifest."""

import importlib.util
import pathlib
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("make-updater-manifest.py")
spec = importlib.util.spec_from_file_location("updater_manifest", SCRIPT)
manifest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(manifest)


class ManifestTests(unittest.TestCase):
    def populate(self, directory):
        assets = pathlib.Path(directory)
        for arch in ("apple-silicon", "intel"):
            archive = assets / f"WhaRa_v0.2.0_{arch}.app.tar.gz"
            archive.write_bytes(b"example updater bundle")
            (assets / (archive.name + ".sig")).write_text("example-minisign-signature")
        return assets

    def test_both_macos_architectures_and_urls(self):
        with tempfile.TemporaryDirectory() as tmp:
            data = manifest.create_manifest(
                self.populate(tmp), "v0.2.0", "rayenking/WhaRa"
            )
            self.assertEqual(data["version"], "0.2.0")
            self.assertEqual(data["platforms"]["darwin-aarch64"]["signature"], "example-minisign-signature")
            self.assertIn("WhaRa_v0.2.0_intel.app.tar.gz", data["platforms"]["darwin-x86_64"]["url"])
            self.assertEqual(data["platforms"]["darwin-aarch64-app"], data["platforms"]["darwin-aarch64"])

    def test_missing_signature_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            assets = self.populate(tmp)
            (assets / "WhaRa_v0.2.0_intel.app.tar.gz.sig").unlink()
            with self.assertRaises(FileNotFoundError):
                manifest.create_manifest(assets, "v0.2.0", "rayenking/WhaRa")

    def test_invalid_release_tags_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                manifest.create_manifest(pathlib.Path(tmp), "not-a-version", "rayenking/WhaRa")


if __name__ == "__main__":
    unittest.main()
