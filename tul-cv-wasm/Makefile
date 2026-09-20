# tul-cv-wasm — browser-side tools for /tul_cv
#
# These modules are workspace members of the repo root, so their build logic
# (and the wasm-bindgen-cli version, derived from the shared Cargo.lock) lives
# in the root Makefile. This file just forwards for convenience.

ROOT := $(CURDIR)/..
MAKE_ROOT := $(MAKE) -C $(ROOT)

.PHONY: all
all:
	$(MAKE_ROOT) cv-wasm

.PHONY: test
test:
	$(MAKE_ROOT) cv-test

.PHONY: clean
clean:
	$(MAKE_ROOT) cv-clean
