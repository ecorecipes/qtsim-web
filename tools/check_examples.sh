#!/usr/bin/env bash
# Regression check on tests/inputs: the SHA-256 of the output of every options file
# in each mode, from the native program and from the WebAssembly module (under
# Node), against tests/expected.tsv.
#
#     tools/check_examples.sh            check (exit 1 on any difference)
#     tools/check_examples.sh --record   write tests/expected.tsv; with QTSIM (the C
#                                        QTSim) and QTSIMFAST (the C QTSimFast) set,
#                                        also the C programs' hashes, for reference
#
# tests/expected.tsv: file, mode, sha256 of this program's output, sha256 of the C
# program's (exact: QTSim; fast1, fast2: QTSimFast's QTSIM_FAST=1, 2), or "-".

set -u
HERE=$(cd "$(dirname "$0")/.." && pwd)
cd "$HERE"
RUST=target/release/qtsim
WASM=target/wasm32-unknown-unknown/release/qtsim.wasm
EXP=tests/expected.tsv
[ -x "$RUST" ] && [ -f "$WASM" ] || { echo "build first (make build wasm)"; exit 2; }
sha() { shasum -a 256 | cut -c1-64; }
lvl() { case $1 in exact) echo 0 ;; fast1) echo 1 ;; fast2) echo 2 ;; esac; }

if [ "${1:-}" = "--record" ]; then
	printf 'file\tmode\tsha256_rust\tsha256_c\n' > "$EXP"
	for f in $(find tests/inputs -name '*.txt' | sort); do
		for m in exact fast1 fast2; do
			r=$( (QTSIM_FAST=$(lvl $m) "$RUST" "$f" 2> /dev/null; echo "exit $?") | sha)
			c=-
			ref=${QTSIM:-}; [ $m != exact ] && ref=${QTSIMFAST:-}
			[ -n "$ref" ] && c=$( (QTSIM_FAST=$(lvl $m) "$ref" "$f" 2> /dev/null; echo "exit $?") | sha)
			printf '%s\t%s\t%s\t%s\n' "$f" "$m" "$r" "$c" >> "$EXP"
		done
	done
	echo "wrote $EXP ($(($(wc -l < "$EXP") - 1)) entries; $(awk -F'\t' 'NR>1 && $4!="-" && $3==$4' "$EXP" | wc -l | tr -d ' ') equal to the C programs')"
	exit 0
fi

bad=0; n=0
while IFS=$'\t' read -r f m want c; do
	[ "$f" = file ] && continue
	n=$((n + 1))
	r=$( (QTSIM_FAST=$(lvl $m) "$RUST" "$f" 2> /dev/null; echo "exit $?") | sha)
	w=$( (node tools/run_wasm.mjs --mode $m "$f" "$WASM" 2> /dev/null; echo "exit $?") | sha)
	if [ "$r" != "$want" ] || [ "$w" != "$want" ]; then
		bad=$((bad + 1))
		echo "DIFF $m $f (native $([ "$r" = "$want" ] && echo ok || echo changed), WebAssembly $([ "$w" = "$want" ] && echo ok || echo changed))"
	fi
done < "$EXP"
echo "checked $n outputs (native and WebAssembly): $((n - bad)) as recorded, $bad changed"
echo "recorded agreement with the C programs: $(awk -F'\t' 'NR>1 && $4!="-" && $3==$4' "$EXP" | wc -l | tr -d ' ') of $(awk -F'\t' 'NR>1 && $4!="-"' "$EXP" | wc -l | tr -d ' ')"
[ $bad = 0 ]
