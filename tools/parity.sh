#!/usr/bin/env bash
# Parity of the Rust port with the C programs, on every *.txt options file in the
# directories given:
#
#     tools/parity.sh DIR [DIR ...]
#
# Compares, per file, standard output and exit status of
#   exact: QTSim (C, the reference)          and  qtsim (Rust)
#   fast1: QTSimFast with QTSIM_FAST=1 (C)   and  qtsim with QTSIM_FAST=1
#   fast2: QTSimFast with QTSIM_FAST=2 (C)   and  qtsim with QTSIM_FAST=2
# Any difference is reported with the first differing line.
#
# Environment: QTSIM (the C QTSim), QTSIMFAST (the C QTSimFast; fast modes skipped
# if absent), RUST (default target/release/qtsim), JOBS (default 4), MODES (default
# "exact fast1 fast2").

set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
QTSIM=${QTSIM:?set QTSIM to the C QTSim binary}
QTSIMFAST=${QTSIMFAST:-}
RUST=${RUST:-$HERE/target/release/qtsim}
JOBS=${JOBS:-4}
MODES=${MODES:-"exact fast1 fast2"}
[ $# -ge 1 ] || { echo "usage: tools/parity.sh DIR [DIR ...]"; exit 2; }
[ -x "$RUST" ] || { echo "no $RUST (cargo build --release)"; exit 2; }
WORK=$(mktemp -d "${TMPDIR:-/tmp}/qtsim-parity.XXXXXX")

find "$@" -name '*.txt' | sort > "$WORK/files"
export QTSIM QTSIMFAST RUST WORK MODES

one() {
	f=$1; id=$(echo "$f" | cksum | cut -d' ' -f1)
	for m in $MODES; do
		case $m in
			exact) ref=$QTSIM; lvl=0 ;;
			fast1) ref=$QTSIMFAST; lvl=1 ;;
			fast2) ref=$QTSIMFAST; lvl=2 ;;
		esac
		[ -n "$ref" ] && [ -x "$ref" ] || continue
		QTSIM_FAST=$lvl "$ref" "$f" > "$WORK/$id.$m.c" 2> /dev/null; echo "exit $?" >> "$WORK/$id.$m.c"
		QTSIM_FAST=$lvl "$RUST" "$f" > "$WORK/$id.$m.r" 2> /dev/null; echo "exit $?" >> "$WORK/$id.$m.r"
		if cmp -s "$WORK/$id.$m.c" "$WORK/$id.$m.r"; then
			echo "$m same $f"
		else
			line=$(cmp "$WORK/$id.$m.c" "$WORK/$id.$m.r" 2>&1 | awk '{print $NF}')
			echo "$m DIFF $f (first difference at line $line)"
		fi
		rm -f "$WORK/$id.$m.c" "$WORK/$id.$m.r"
	done
}
export -f one
xargs -P "$JOBS" -I{} bash -c 'one "$@"' _ {} < "$WORK/files" > "$WORK/results"

echo "parity: $(wc -l < "$WORK/files" | tr -d ' ') options files"
for m in $MODES; do
	s=$(grep -c "^$m same" "$WORK/results"); d=$(grep -c "^$m DIFF" "$WORK/results")
	[ $((s + d)) -gt 0 ] && echo "  $m: $s identical, $d different"
done
grep " DIFF " "$WORK/results" | head -20
bad=$(grep -c " DIFF " "$WORK/results")
rm -rf "$WORK"
[ "$bad" = 0 ]
