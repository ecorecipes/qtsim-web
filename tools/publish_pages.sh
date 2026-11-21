#!/usr/bin/env bash
# Publish the explorer to the gh-pages branch of the remote (GitHub Pages):
#
#     tools/publish_pages.sh [remote]     (default: origin)
#
# Builds dist/qtsim.html (make web) and replaces the gh-pages branch with a single
# commit holding qtsim.html, the same page as index.html, and .nojekyll. The branch
# holds only the built page; its commit message names the source commit.

set -eu
HERE=$(cd "$(dirname "$0")/.." && pwd)
cd "$HERE"
REMOTE=${1:-origin}
SRC=$(git rev-parse --short HEAD)
git diff --quiet HEAD || echo "note: the working tree has uncommitted changes; the page is built from them"

make web > /dev/null

T=$(mktemp -d "${TMPDIR:-/tmp}/qtsim-pages.XXXXXX")
trap 'rm -rf "$T"' EXIT
cp dist/qtsim.html "$T/qtsim.html"
cp dist/qtsim.html "$T/index.html"
touch "$T/.nojekyll"
git -C "$T" init -q -b gh-pages
git -C "$T" config user.name "$(git config user.name)"
git -C "$T" config user.email "$(git config user.email)"
git -C "$T" add -A
git -C "$T" commit -q -m "QTSim Explorer, built from $SRC"
# pushed from this repository, with its remote's credentials (no local gh-pages branch)
git fetch -q "$T" gh-pages
git push -q -f "$REMOTE" FETCH_HEAD:refs/heads/gh-pages
echo "published dist/qtsim.html (from $SRC) to $REMOTE gh-pages"
