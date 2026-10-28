# App identity and releases

## Identity

The desktop app installs as **Codesh** (`com.hugocorta.codesh`) or **Codesh
Beta** (`com.hugocorta.codesh-beta`). Local builds use the beta identity.

Each channel has its own profile folder: `Codesh`, `Codesh Beta`, and `Codesh
Dev` for development builds in macOS Application Support, or `codesh`,
`codesh-beta`, and `codesh-dev` under Linux's state directory. `CODESH_DIR`
overrides the location. macOS runtime leases live under `Codesh Runtime`.

The executables are `codesh-app`, `codesh`, and `codesh-server`. Environment
variables use the `CODESH_` prefix, and mobile pairing links use `codesh://pair`.

## Releases and updates

Every commit on `main` that passes CI is published on
[GitHub Releases](https://github.com/hugocxl/codesh/releases) as
`X.Y.<commit count>`, with `X.Y` from `VERSION`
(`.github/workflows/release.yml`). Releases ship Apple silicon only:
`Codesh-{version}-arm64.dmg`, the `codesh-{version}-macos-arm64.zip`
CLI/server pair, and `install-codesh.sh`.

The newest release is marked latest, and installed apps update from its
`releases/latest/download/update.json`. Remote installs fetch
`install-codesh.sh` from the matching release.

## Signing and notarization

Signing supports Developer ID Application and Apple Development certificates.
Automatic updates verify the installed app's team and bundle ID, so ad-hoc
signatures cannot use them. Notarization requires Developer ID and notary
credentials. CI notarizes only when all three Apple notary secrets are present.

`scripts/build-install.py` builds, signs, notarizes, and installs locally. It
accepts a notary keychain profile; without credentials it installs the signed
app without stapler validation. `--build-only` signs without notarizing or
installing.
