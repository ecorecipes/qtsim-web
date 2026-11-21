#!/usr/bin/env bash
# The WebAssembly module (run by tools/run_wasm.mjs under Node, as the browser runs
# it) against the native build of the same source, on every *.txt options file in
# the directories given, in each mode:
#
#     tools/parity_wasm.sh DIR [DIR ...]
#
# Both use the portable libm, so they must give the same bytes. Environment: RUST
# (default target/release/qtsim), WASM (default the release module), NODE (default
# node), JOBS (default 4), MODES (default "exact fast1 fast2").

set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
RUST=${RUST:-$HERE/target/release/qtsim}
WASM=${WASM:-$HERE/target/wasm32-unknown-unknown/release/qtsim.wasm}
NODE=${NODE:-node}
JOBS=${JOBS:-4}
MODES=${MODES:-"exact fast1 fast2"}
[ $# -ge 1 ] || { echo "usage: tools/parity_wasm.sh DIR [DIR ...]"; exit 2; }
[ -x "$RUST" ] && [ -f "$WASM" ] || { echo "build first: cargo build --release; cargo build --release --lib --target wasm32-unknown-unknown"; exit 2; }
WORK=$(mktemp -d "${TMPDIR:-/tmp}/qtsim-parity-wasm.XXXXXX")
find "$@" -name '*.txt' | sort > "$WORK/files"
export RUST WASM NODE WORK MODES HERE

one() {
	f=$1; id=$(echo "$f" | cksum | cut -d' ' -f1)
	for m in $MODES; do
		lvl=0; [ $m = fast1 ] && lvl=1; [ $m = fast2 ] && lvl=2
		QTSIM_FAST=$lvl "$RUST" "$f" > "$WORK/$id.n" 2> /dev/null; echo "exit $?" >> "$WORK/$id.n"
		"$NODE" "$HERE/tools/run_wasm.mjs" --mode $m "$f" "$WASM" > "$WORK/$id.w" 2> /dev/null; echo "exit $?" >> "$WORK/$id.w"
		if cmp -s "$WORK/$id.n" "$WORK/$id.w"; then echo "$m same $f"
		else echo "$m DIFF $f (first difference at line $(cmp "$WORK/$id.n" "$WORK/$id.w" 2>&1 | awk '{print $NF}'))"; fi
		rm -f "$WORK/$id.n" "$WORK/$id.w"
	done
}
export -f one
xargs -P "$JOBS" -I{} bash -c 'one "$@"' _ {} < "$WORK/files" > "$WORK/results"

echo "parity (WebAssembly against native): $(wc -l < "$WORK/files" | tr -d ' ') options files"
for m in $MODES; do
	echo "  $m: $(grep -c "^$m same" "$WORK/results") identical, $(grep -c "^$m DIFF" "$WORK/results") different"
done
grep " DIFF " "$WORK/results" | head -20
bad=$(grep -c " DIFF " "$WORK/results")
rm -rf "$WORK"
[ "$bad" = 0 ]
