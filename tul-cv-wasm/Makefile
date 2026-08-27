# tul-cv-wasm — browser-side tools for /tul_cv
#
# Produces two WASM modules:
#   pkg/tul_cv.js           + pkg/tul_cv_bg.wasm       (core: image/text/pdf/unit)
#   office/pkg/tul_cv_office.js + office/pkg/tul_cv_office_bg.wasm (lazy: xlsx/docx)
#
# Requires: wasm-bindgen-cli 0.2.127 (match wasm-bindgen in Cargo.toml)

CARGO_HOME ?= $(CURDIR)/../target/cargo-home
BINDGEN := wasm-bindgen

.PHONY: all
all: core office

.PHONY: core
core:
	cargo build --release --target wasm32-unknown-unknown
	$(BINDGEN) target/wasm32-unknown-unknown/release/tul_cv_wasm.wasm \
		--out-dir pkg --target web --no-typescript

.PHONY: office
office:
	cd office && cargo build --release --target wasm32-unknown-unknown
	cd office && $(BINDGEN) target/wasm32-unknown-unknown/release/tul_cv_office_wasm.wasm \
		--out-dir pkg --target web --no-typescript

.PHONY: test
test:
	cargo test

.PHONY: clean
clean:
	rm -rf pkg office/pkg target office/target

# keep wasm-bindgen-cli on PATH for cargo build (installed into CARGO_HOME/bin)
export PATH := $(CARGO_HOME)/bin:$(PATH)
export CARGO_HOME
