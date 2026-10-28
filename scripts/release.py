#!/usr/bin/env python3
"""Versions and packaging metadata of releases (X.Y.<commit count>) and local beta builds."""
import argparse
import hashlib
import json
import plistlib
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BETA_PATTERN = re.compile(r"([0-9]+\.[0-9]+\.[0-9]+)-beta\.([1-9][0-9]*)")
STABLE_PATTERN = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+")
# Each channel installs as its own app.
APPS = {"beta": ("Codesh Beta", "com.hugocorta.codesh-beta"), "stable": ("Codesh", "com.hugocorta.codesh")}


def build_metadata(root=ROOT):
    """The --build-info every executable of this checkout reports."""
    build = (root / "crates/codesh-protocol/src/build.rs").read_text()
    version = (root / "crates/codesh-protocol/src/version.rs").read_text()
    current = re.search(r"pub const CURRENT: Version = (V\d+);", version)[1]
    number = int(re.search(rf"pub const {current}: Version = Version\((\d+)\);", version)[1])
    return {
        "compatibility": int(re.search(r"pub const COMPATIBILITY: u64 = (\d+);", build)[1]),
        "protocol": [number],
    }


def channel(version):
    if BETA_PATTERN.fullmatch(version):
        return "beta"
    if STABLE_PATTERN.fullmatch(version):
        return "stable"
    raise ValueError("release version must be X.Y.Z or X.Y.Z-beta.<positive commit count>")


def build_number(version):
    """A beta's commit count, which orders betas and becomes the bundle version."""
    match = BETA_PATTERN.fullmatch(version)
    if not match:
        raise ValueError("beta version must be X.Y.Z-beta.<positive commit count>")
    return match[2]


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def checkout_version(root):
    """Every commit on main releases as <VERSION>.<commit count>."""
    if git(root, "rev-parse", "--is-shallow-repository") != "false":
        raise ValueError("release numbering requires a full checkout (fetch-depth: 0)")
    base = (root / "VERSION").read_text().strip()
    if not re.fullmatch(r"[0-9]+\.[0-9]+", base):
        raise ValueError("VERSION must be X.Y")
    return f"{base}.{git(root, 'rev-list', '--count', 'HEAD')}"


def replace_once(pattern, replacement, text, label):
    updated, count = re.subn(pattern, replacement, text, flags=re.MULTILINE)
    if count != 1:
        raise ValueError(f"expected exactly one {label}, found {count}")
    return updated


def stamp_version(root, version):
    channel(version)
    manifest_path = root / "Cargo.toml"
    lock_path = root / "Cargo.lock"
    manifest = manifest_path.read_text()
    pattern = r'(\[workspace\.package\]\nversion = ")([^"\n]+)(")'
    current = re.search(pattern, manifest)
    if not current:
        raise ValueError("workspace package version not found")
    manifest = replace_once(
        pattern, lambda match: match[1] + version + match[3], manifest, "workspace version"
    )
    lock = lock_path.read_text()
    manifests = sorted(root.glob("crates/*/Cargo.toml"))
    if not manifests:
        raise ValueError("workspace crates not found")
    for crate in manifests:
        name = re.search(r'^name = "([^"]+)"$', crate.read_text(), re.MULTILINE)
        if not name:
            raise ValueError(f"package name not found in {crate}")
        pattern = (
            r'(\[\[package\]\]\nname = "' + re.escape(name[1])
            + r'"\nversion = ")' + re.escape(current[2]) + r'(")'
        )
        lock = replace_once(
            pattern, lambda match: match[1] + version + match[2], lock, name[1]
        )
    # Only local package versions change. All third-party resolutions stay locked.
    manifest_path.write_text(manifest)
    lock_path.write_text(lock)


def bundle_info(version, build):
    """Info.plist of the app. Its bundle version is the commit count it was built from."""
    name, identifier = APPS[channel(version)]
    if not re.fullmatch(r"[1-9][0-9]*", build):
        raise ValueError("build number must be a positive commit count")
    if channel(version) == "beta" and build != build_number(version):
        raise ValueError("a beta's build number is its commit count")
    return {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": name,
        "CFBundleName": name,
        "CFBundleExecutable": "codesh-app",
        "CFBundleIdentifier": identifier,
        "CFBundleIconFile": "AppIcon",
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version.split("-")[0],
        "CFBundleVersion": build,
        "CFBundleGetInfoString": f"{name} {version}",
        "CodeshVersion": version,
        "LSMinimumSystemVersion": "14.0",
        "LSApplicationCategoryType": "public.app-category.developer-tools",
        "NSHighResolutionCapable": True,
        "NSMicrophoneUsageDescription": "Codesh uses your microphone to dictate text into the active terminal.",
        "NSSpeechRecognitionUsageDescription": "Codesh transcribes your dictation on this device and inserts it into the active terminal.",
        "NSSupportsAutomaticGraphicsSwitching": True,
        "NSPrincipalClass": "NSApplication",
        "NSLocalNetworkUsageDescription": "Commands you run in Codesh terminals can connect to devices on your local network.",
        "NSAppTransportSecurity": {"NSAllowsArbitraryLoadsInWebContent": True},
    }


def update_metadata(version, repository, directory):
    channel(version)
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("invalid GitHub repository")
    platforms = {}
    for platform, arch in (("macos-aarch64", "arm64"), ("macos-x86_64", "x86_64")):
        filename = f"Codesh-{version}-{arch}.dmg"
        if arch == "x86_64" and not (directory / filename).exists():
            continue
        size = (directory / filename).stat().st_size
        if not 0 < size <= 2 * 1024**3:
            raise ValueError(f"invalid update size for {arch}")
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{filename}",
            "size": size,
        }
    for platform, arch in (("linux-aarch64", "arm64"), ("linux-x86_64", "x86_64")):
        filename = f"codesh-desktop-{version}-linux-{arch}.tar.gz"
        path = directory / filename
        if not path.exists():
            continue
        size = path.stat().st_size
        if not 0 < size <= 2 * 1024**3 or path.is_symlink():
            raise ValueError(f"invalid desktop update for {arch}")
        checksum = hashlib.sha256()
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                checksum.update(block)
        platforms[platform] = {
            "url": f"https://github.com/{repository}/releases/download/v{version}/{filename}",
            "size": size,
            "sha256": checksum.hexdigest(),
        }
    return {"schema": 1, "version": version, "platforms": platforms}


def check_build(version, executable):
    channel(version)
    # The first run of an x86_64 build on Apple silicon waits for Rosetta to translate it.
    metadata = json.loads(subprocess.check_output([str(executable), "--build-info"], timeout=60))
    if metadata != {"version": version, **build_metadata()}:
        raise ValueError("Packaged executable build metadata does not match this release")


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("version")
    check = commands.add_parser("check-build")
    check.add_argument("version")
    check.add_argument("executable", type=Path)
    metadata = commands.add_parser("update")
    metadata.add_argument("version")
    metadata.add_argument("repository")
    metadata.add_argument("directory", type=Path)
    for command in ("stamp", "check-version", "channel", "app-name", "plist"):
        subparser = commands.add_parser(command)
        subparser.add_argument("version")
        if command == "plist":
            subparser.add_argument("output", type=Path)
            subparser.add_argument("--build", help="commit count; a beta's is in its version")
    args = parser.parse_args()
    try:
        if args.command == "version":
            version = checkout_version(ROOT)
            print(f"version={version}")
            print(f"tag=v{version}")
            print(f"build_number={version.rsplit('.', 1)[1]}")
        elif args.command == "check-build":
            check_build(args.version, args.executable)
        elif args.command == "stamp":
            stamp_version(ROOT, args.version)
        elif args.command == "check-version":
            channel(args.version)
        elif args.command == "channel":
            print(channel(args.version))
        elif args.command == "app-name":
            print(APPS[channel(args.version)][0])
        elif args.command == "update":
            metadata = update_metadata(args.version, args.repository, args.directory)
            (args.directory / "update.json").write_text(json.dumps(metadata, indent=2) + "\n")
        else:
            if args.build is None and channel(args.version) == "stable":
                raise ValueError("a stable release needs --build <commit count>")
            build = args.build or build_number(args.version)
            args.output.write_bytes(plistlib.dumps(bundle_info(args.version, build)))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
