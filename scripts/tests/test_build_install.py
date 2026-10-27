import importlib.util
from pathlib import Path
import plistlib
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
SPEC = importlib.util.spec_from_file_location("build_install", ROOT / "scripts/build-install.py")
BUILD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILD)


class BuildInstallTests(unittest.TestCase):
    def test_development_and_distribution_certificates_can_be_selected_without_ambiguity(self):
        development, distribution = "a" * 40, "b" * 40
        identities = (f'1) {development} "Apple Development: Developer (TEAM)"\n'
                      f'2) {distribution} "Developer ID Application: Developer (TEAM)"')
        with patch.object(BUILD, "output", return_value=identities):
            self.assertEqual(BUILD.signing_identity(development), development)
            self.assertEqual(BUILD.signing_identity("Developer ID Application: Developer (TEAM)"), distribution)
            with self.assertRaises(ValueError):
                BUILD.signing_identity(None)

    def test_packaging_keeps_channel_identity_icon_and_local_build_metadata_consistent(self):
        for version, app_name, icon in (
                ("0.1.0-beta-0", "Codesh Beta", "AppIconBeta.png"),
                ("2.3.0-beta.12", "Codesh Beta", "AppIconBeta.png"),
                ("2.3.0", "Codesh", "AppIcon.png")):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                binaries = directory / "binaries"
                binaries.mkdir()
                for name in (*BUILD.BINARIES, "codesh-app"):
                    (binaries / name).write_bytes(b"executable")
                with patch.object(BUILD, "run") as run, patch.object(BUILD, "output", return_value="12"):
                    app = BUILD.package_app(directory, binaries, version, "signing-identity")
                self.assertEqual(app.name, app_name + ".app")
                info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
                self.assertEqual(info["CFBundleGetInfoString"], f"{app_name} {version}")
                self.assertEqual(info["CodeshVersion"], version)
                self.assertTrue(all(call.args[4].name == icon for call in run.call_args_list
                                    if call.args[0] == "sips"))

    def test_installation_validates_tickets_only_for_notarized_artifacts_and_preserves_other_channel(self):
        for notarized in (False, True):
            with self.subTest(notarized=notarized), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                artifact = directory / "build/Codesh.app"
                executables = artifact / "Contents/MacOS"
                executables.mkdir(parents=True)
                for name in BUILD.BINARIES:
                    (executables / name).write_bytes(b"new")
                apps, commands = directory / "Applications", directory / "bin"
                beta = apps / "Codesh Beta.app"
                beta.mkdir(parents=True)
                def run(*args):
                    if args[0] == "ditto":
                        shutil.copytree(args[1], args[2])
                with patch.object(BUILD, "run", side_effect=run) as calls:
                    BUILD.install(artifact, commands, apps, notarized=notarized)
                ticket_checks = [call for call in calls.call_args_list if call.args[0] == "xcrun"]
                self.assertEqual(len(ticket_checks), int(notarized))
                self.assertTrue(beta.is_dir())
                for name in BUILD.BINARIES:
                    self.assertEqual((commands / name).resolve(), (apps / "Codesh.app/Contents/MacOS" / name).resolve())
                    self.assertEqual((commands / name).read_bytes(), b"new")
