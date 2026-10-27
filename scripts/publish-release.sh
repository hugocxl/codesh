#!/bin/bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 2 ]]; then
    echo "Usage: $0 <X.Y.Z | X.Y.Z-beta.N> <artifact-directory>" >&2
    echo "A stable release also needs BETA_TAG, the beta it promotes, and DRAFT=true to stay a draft." >&2
    echo "RELEASE_NOTES, when set, replaces the generated release notes." >&2
    exit 1
fi
VERSION="$1"
CHANNEL="$(python3 "$ROOT/scripts/release.py" channel "$VERSION")"
ARTIFACTS="$(cd "$2" && pwd)"
TAG="v$VERSION"
: "${GITHUB_REPOSITORY:?GITHUB_REPOSITORY is required}"
: "${GITHUB_SHA:?GITHUB_SHA is required}"
if [[ "${GITHUB_REF:-}" != refs/heads/main ]]; then
    echo "Error: releases are restricted to main" >&2
    exit 1
fi
cd "$ROOT"
# A beta is built from the commit that triggered it; a stable release from the beta it promotes.
SOURCE="$GITHUB_SHA"
if [[ "$CHANNEL" == stable ]]; then
    : "${BETA_TAG:?BETA_TAG is required for a stable release}"
    SOURCE="$(git rev-parse --verify "refs/tags/$BETA_TAG^{commit}")"
fi
if [[ "$(git rev-parse HEAD)" != "$SOURCE" ]]; then
    echo "Error: checkout does not match the release source" >&2
    exit 1
fi
if [[ "$CHANNEL" == beta && "$(python3 scripts/release.py version | sed -n 's/^version=//p')" != "$VERSION" ]]; then
    echo "Error: version does not match the triggering commit count" >&2
    exit 1
fi

check_source() {
    git fetch origin '+refs/heads/main:refs/remotes/origin/main' --tags
    git merge-base --is-ancestor "$SOURCE" origin/main
    if git show-ref --verify --quiet "refs/tags/$TAG"; then
        if [[ "$(git rev-parse "refs/tags/$TAG^{}")" != "$SOURCE" ]]; then
            echo "Error: $TAG already points to a different commit" >&2
            exit 1
        fi
    fi
}
check_source

cd "$ARTIFACTS"
ASSETS=(install-codesh.sh install-desktop-linux.py codesh-desktop.apparmor)
for ARCH in arm64 x86_64; do
    ASSETS+=("codesh-${VERSION}-linux-${ARCH}.tar.gz")
    ASSETS+=("codesh-desktop-${VERSION}-linux-${ARCH}.tar.gz" "Codesh-${VERSION}-linux-${ARCH}.AppImage")
    DEB_ARCH="$ARCH"
    [[ "$ARCH" != x86_64 ]] || DEB_ARCH=amd64
    DEB_PACKAGE=codesh-desktop
    [[ "$CHANNEL" != beta ]] || DEB_PACKAGE=codesh-desktop-beta
    ASSETS+=("${DEB_PACKAGE}_${VERSION}_${DEB_ARCH}.deb")
    # A beta may skip Intel; a stable release supports both Mac architectures.
    if [[ "$CHANNEL" == beta && "$ARCH" == x86_64 && ! -e "Codesh-${VERSION}-${ARCH}.dmg" && ! -e "codesh-${VERSION}-macos-${ARCH}.zip" ]]; then
        continue
    fi
    ASSETS+=("Codesh-${VERSION}-${ARCH}.dmg" "codesh-${VERSION}-macos-${ARCH}.zip")
done
for ASSET in "${ASSETS[@]}"; do
    if [[ ! -f "$ASSET" || ! -s "$ASSET" || -L "$ASSET" ]]; then
        echo "Error: missing regular release asset: $ASSET" >&2
        exit 1
    fi
done
python3 "$ROOT/scripts/release.py" update "$VERSION" "$GITHUB_REPOSITORY" "$ARTIFACTS"
ASSETS+=(update.json)
shasum -a 256 "${ASSETS[@]}" > SHA256SUMS
ASSETS+=(SHA256SUMS)

PRERELEASE=false
if [[ "$CHANNEL" == beta ]]; then
    PRERELEASE=true
fi
if gh release view "$TAG" --repo "$GITHUB_REPOSITORY" \
    --json isDraft,isPrerelease,targetCommitish > release.json; then
    python3 -c 'import json, sys; r = json.load(sys.stdin); sys.exit(0 if r["isPrerelease"] == (sys.argv[2] == "true") and r["targetCommitish"] == sys.argv[1] else 1)' \
        "$SOURCE" "$PRERELEASE" < release.json
    if [[ "$(python3 -c 'import json, sys; print(json.load(sys.stdin)["isDraft"])' < release.json)" == False ]]; then
        echo "==> $TAG is already published; leaving its assets unchanged"
        if [[ "$CHANNEL" == beta ]]; then
            python3 "$ROOT/scripts/publish-update.py" "$VERSION"
        fi
        exit 0
    fi
else
    # RELEASE_NOTES, when given, replaces the generated notes. Otherwise use GitHub's
    # generated changes, cleaned. A beta lists the changes since the previous beta; a stable
    # release lets GitHub choose the previous release.
    NOTES_ARGS=(-f "tag_name=$TAG" -f "target_commitish=$SOURCE")
    : > release-notes.md
    if [[ "$CHANNEL" == beta ]]; then
        PREVIOUS="$(git -C "$ROOT" describe --tags --match "v${VERSION%%.*}.*-beta*" --abbrev=0 "$SOURCE^" 2>/dev/null || true)"
        if [[ -n "$PREVIOUS" ]]; then
            NOTES_ARGS+=(-f "previous_tag_name=$PREVIOUS")
        fi
    fi
    if [[ "${RELEASE_NOTES:-}" =~ [^[:space:]] ]]; then
        printf '%s\n' "$RELEASE_NOTES" >> release-notes.md
    else
        gh api --method POST "repos/$GITHUB_REPOSITORY/releases/generate-notes" "${NOTES_ARGS[@]}" --jq .body \
            | bash "$ROOT/scripts/clean-changelog.sh" >> release-notes.md
    fi
    DRAFT_FLAGS=(--draft --latest=false)
    if [[ "$CHANNEL" == beta ]]; then
        DRAFT_FLAGS+=(--prerelease)
    fi
    gh release create "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --title "Codesh $VERSION" "${DRAFT_FLAGS[@]}" --notes-file release-notes.md
fi

# A failed upload leaves a resumable draft, never a half-populated public release.
gh release upload "$TAG" --repo "$GITHUB_REPOSITORY" --clobber "${ASSETS[@]}"
gh release view "$TAG" --repo "$GITHUB_REPOSITORY" --json assets > uploaded-assets.json
python3 - uploaded-assets.json "${ASSETS[@]}" <<'PY'
import json, pathlib, sys
remote = json.loads(pathlib.Path(sys.argv[1]).read_text())["assets"]
for name in sys.argv[2:]:
    matches = [asset for asset in remote if asset["name"] == name]
    if len(matches) != 1 or matches[0]["size"] != pathlib.Path(name).stat().st_size:
        sys.exit(f"Error: missing or incomplete uploaded asset: {name}")
PY
cd "$ROOT"
check_source
if [[ "$CHANNEL" == beta ]]; then
    gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --draft=false --prerelease --latest=false
    python3 "$ROOT/scripts/publish-update.py" "$VERSION"
elif [[ "${DRAFT:-false}" == true ]]; then
    echo "==> $TAG is a complete draft. Publishing it as the latest release updates Codesh stable builds."
else
    # The latest release is the Codesh stable feed.
    gh release edit "$TAG" --repo "$GITHUB_REPOSITORY" --target "$SOURCE" \
        --draft=false --prerelease=false --latest
fi
