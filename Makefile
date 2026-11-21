# QTSim in Rust and WebAssembly, and its browser explorer.
#
#   make              the command-line program (target/release/qtsim)
#   make web          the WebAssembly module and the app: web/ (serve it) and
#                     dist/qtsim.html (one self-contained file, opens from disk)
#   make serve        serve web/ at http://localhost:8000
#   make test         unit tests, and the examples against their recorded outputs
#   make parity DIRS="dir ..."        against the C programs (QTSIM=, QTSIMFAST=)
#   make parity-wasm DIRS="dir ..."   the WebAssembly module against the native build
#
# Needs Rust with the wasm32-unknown-unknown target (rustup target add
# wasm32-unknown-unknown) and, for the app's bundle and the WebAssembly checks, Node.

CARGO ?= cargo
WASM = target/wasm32-unknown-unknown/release/qtsim.wasm
DIRS ?= tests/inputs

.PHONY: build wasm web serve test parity parity-wasm clean

build:
	$(CARGO) build --release

wasm:
	$(CARGO) build --release --lib --target wasm32-unknown-unknown

web: wasm
	cp $(WASM) web/qtsim.wasm
	node tools/bundle.mjs

serve: web
	cd web && python3 -m http.server 8000

test: build wasm
	$(CARGO) test --release
	tools/check_examples.sh

parity: build
	tools/parity.sh $(DIRS)

parity-wasm: build wasm
	tools/parity_wasm.sh $(DIRS)

clean:
	$(CARGO) clean
	rm -rf dist web/qtsim.wasm target-syslibm
