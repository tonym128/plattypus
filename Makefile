ROOT     := $(CURDIR)
GAME_DIR := $(ROOT)/game
TARGET   := mipsel-sony-psx
DIST     := $(ROOT)/dist
GAME_EXE := $(GAME_DIR)/target/$(TARGET)/release/plattypus.exe
MKISOPSX := $(ROOT)/psoxide/tools/mkisopsx

# --- emulator discovery for `make run` ---------------------------------------
# Directories a libretro PS1 core is commonly installed in, searched in order.
# Note RetroArch's own `libretro_directory` setting is not consulted: this
# build of RetroArch is for dynamic cores and refuses to start without an
# explicit -L, so we have to hand it a path ourselves.
RETROARCH_CORE_DIRS ?= $(HOME)/.config/retroarch/cores /usr/lib/libretro \
                       /usr/lib/$(shell uname -m)-linux-gnu/libretro \
                       /usr/local/lib/libretro
# PS1 libretro cores, in preference order.
RETROARCH_PS1_CORES ?= pcsx_rearmed_libretro.so mednafen_psx_libretro.so \
                       swanstation_libretro.so beetle_psx_hw_libretro.so
# DuckStation AppImages, searched in order. Set DUCKSTATION_APPIMAGE to
# override, or point EMULATOR at a native build.
DUCKSTATION_APPIMAGES ?= $(DUCKSTATION_APPIMAGE) \
                         $(HOME)/Downloads/DuckStation-x64.AppImage \
                         $(HOME)/Applications/DuckStation.AppImage

.PHONY: all exe disc iso clean run test help check-media video-bench video-bench-build web \
        ci ci-host ci-game ci-disc clippy fmt-check fmt lockfile-check

all: disc iso

help:
	@echo "Plattypus (PSX / PSoXide) Build Targets:"
	@echo "  make ci          - Run full GitHub CI test suite locally (host + game + disc)"
	@echo "  make ci-host     - Run CI Job 1: host checks (fmt-check, clippy, test, lockfiles)"
	@echo "  make ci-game     - Run CI Job 2: PSX cross-build & RAM budget gate check"
	@echo "  make ci-disc     - Run CI Job 3: disc & ISO mastering smoke tests"
	@echo "  make clippy      - Run clippy lint check (-D warnings)"
	@echo "  make fmt-check   - Check code formatting on all project crates"
	@echo "  make fmt         - Auto-format all project crates"
	@echo "  make test        - Run automated host-side game logic test suite"
	@echo "  make exe         - Compile PSX-EXE (MIPS R3000 bare-metal binary)"
	@echo "  make disc        - Master bootable PS1 disc image (dist/plattypus.{bin,cue})"
	@echo "  make iso         - Master cooked PS1 ISO image (dist/plattypus.iso)"
	@echo "  make web         - Serve Plattypus Web Arcade on http://localhost:8080"
	@echo "  make run         - Run disc in DuckStation, RetroArch, or EMULATOR=... emulator"
	@echo ""
	@echo "Emulator selection for 'make run' (first match wins):"
	@echo "  EMULATOR=/path/to/emulator        - use this binary directly"
	@echo "  DUCKSTATION_APPIMAGE=/path/...    - use this DuckStation AppImage"
	@echo "  RETROARCH_CORE=/path/to/core.so   - use this libretro core for RetroArch"
	@echo "  make check-media - Verify the disc image is well formed"
	@echo "  make video-bench - Measure intro-video playback performance (needs an emulator)"
	@echo "  make clean       - Clean build artifacts"
	@echo ""
	@echo "Video performance options:"
	@echo "  RUNS=N     - repeat the measurement N times and report the median (default 1)"
	@echo "  SECONDS=N  - wall-clock budget per run (default 150; a full run takes ~60s)"

# --- GitHub CI Equivalent Targets ---------------------------------------------
ci: ci-host ci-game ci-disc
	@echo ""
	@echo "=========================================================="
	@echo "  ALL GITHUB CI EQUIVALENT CHECKS PASSED SUCCESSFULLY!"
	@echo "=========================================================="

ci-host: fmt-check clippy test lockfile-check
	@echo "--- CI Job 1 (Host tests, lint, format) Passed ---"

ci-game: exe
	@echo "--- CI Job 2 (PSX build & RAM budget gate) ---"
	@set -euo pipefail; \
	exe="$(DIST)/plattypus.exe"; \
	test -f "$$exe"; \
	size=$$(stat -c%s "$$exe"); \
	pct=$$(( size * 100 / 2097152 )); \
	echo "exe = $$size bytes ($$pct% of 2 MB PS1 RAM)"; \
	if [ "$$size" -ge 2097152 ]; then \
		echo "ERROR: executable exceeds the 2 MB main RAM of a PlayStation"; \
		exit 1; \
	fi
	@echo "--- CI Job 2 (PSX build) Passed ---"

ci-disc: disc iso
	@echo "--- CI Job 3 (Disc mastering smoke test) ---"
	@set -euo pipefail; \
	grep -q '^FILE' $(DIST)/plattypus.cue; \
	named=$$(awk '/^FILE/{gsub(/"/,"",$$2); print $$2}' $(DIST)/plattypus.cue | head -1); \
	echo "cue references: $$named"; \
	test -f "$$named" || test -f "$(DIST)/$$(basename "$$named")"; \
	ls -lh $(DIST)/plattypus.bin $(DIST)/plattypus.cue $(DIST)/plattypus.iso
	@echo "--- CI Job 3 (Disc mastering) Passed ---"

fmt-check:
	@echo "Checking formatting across project crates..."
	@for m in game crates/plattypus-core tools/test_game_logic video_bench; do \
		echo "Checking formatting: $$m"; \
		cargo fmt --manifest-path "$$m/Cargo.toml" --all -- --check || exit 1; \
	done

fmt:
	@echo "Applying formatting across project crates..."
	@for m in game crates/plattypus-core tools/test_game_logic video_bench; do \
		echo "Formatting: $$m"; \
		cargo fmt --manifest-path "$$m/Cargo.toml" --all; \
	done

clippy:
	@echo "Running clippy on host crates..."
	cargo clippy --locked --offline --manifest-path crates/plattypus-core/Cargo.toml --all-targets -- -D warnings
	cargo clippy --locked --offline --manifest-path tools/test_game_logic/Cargo.toml --all-targets -- -D warnings

test:
	@echo "Running host-side game logic test suite..."
	cargo run --locked --offline --manifest-path $(ROOT)/tools/test_game_logic/Cargo.toml

lockfile-check:
	@echo "Verifying lockfiles agree on registry dependencies..."
	@set -euo pipefail; \
	game_lockfile="/tmp/game.lock.reg"; \
	video_lockfile="/tmp/video_bench.lock.reg"; \
	extract_reg() { \
		awk ' \
			/^\[\[package\]\]/ { if (reg && name && ver) print name, ver; name=""; ver=""; reg=0 } \
			/^name = / { name=$$3 } \
			/^version = / { ver=$$3 } \
			/^source = "registry/ { reg=1 } \
			END { if (reg && name && ver) print name, ver } \
		' "$$1" | sort; \
	}; \
	extract_reg game/Cargo.lock > "$$game_lockfile"; \
	extract_reg video_bench/Cargo.lock > "$$video_lockfile"; \
	diff -u "$$game_lockfile" "$$video_lockfile"; \
	echo "Lockfiles agree on registry dependencies."

web:
	@mkdir -p $(ROOT)/web/roms
	@cp $(DIST)/plattypus.exe $(DIST)/plattypus.cue $(DIST)/plattypus.bin $(DIST)/plattypus.iso $(ROOT)/web/roms/ 2>/dev/null || true
	@python3 $(ROOT)/tools/serve_web.py 8080 $(ROOT)/web

# ---------------------------------------------------------------------------
# Intro-video performance measurement.
#
# Builds video_bench (a PSX-EXE that plays the intro through the game's own
# video.rs with a stopwatch attached), masters a disc holding it plus
# INTRO.VID, runs it headless, and writes dist/video_bench_report.json.
# See tools/video_bench/README.md for the metric definitions.
# ---------------------------------------------------------------------------
RUNS    ?= 1
SECONDS ?= 150

video-bench:
	python3 $(ROOT)/tools/video_bench/host_bench.py --runs $(RUNS) --seconds $(SECONDS)

video-bench-build:
	cd $(ROOT)/video_bench && cargo build --release

exe:
	@mkdir -p $(DIST)
	cd $(GAME_DIR) && cargo build --release
	@cp $(GAME_EXE) $(DIST)/plattypus.exe
	@echo "BUILT PSX-EXE -> $(DIST)/plattypus.exe"

iso: exe
	@mkdir -p $(DIST)
	cargo run --release --manifest-path $(MKISOPSX)/Cargo.toml -- \
		--exe $(DIST)/plattypus.exe \
		--out $(DIST)/plattypus.iso \
		--volume PLATTYPUS \
		--iso
	@echo "SUCCESS! Cooked PS1 ISO Mastered: $(DIST)/plattypus.iso"

disc: exe check-media
	@mkdir -p $(DIST)
	cargo run --release --manifest-path $(MKISOPSX)/Cargo.toml -- \
		--exe $(DIST)/plattypus.exe \
		--out $(DIST)/plattypus.bin \
		--volume PLATTYPUS \
		--cdda-track $(ROOT)/Music/title_music.cdda \
		--cdda-track $(ROOT)/Music/credits.cdda \
		--xa-file $(ROOT)/Videos/INTRO.STR \
		--xa-file $(ROOT)/Videos/OUTRO.STR
	@echo "SUCCESS! Bootable PS1 Disc Mastered:"
	@echo "  CUE: $(DIST)/plattypus.cue"
	@echo "  BIN: $(DIST)/plattypus.bin"

# The media inputs are large binaries that are not generated by any target, so
# a missing one used to surface as an opaque failure inside mkisopsx.
check-media: Music/credits.cdda
	@missing=""; \
	for f in Music/title_music.cdda Music/credits.cdda Videos/INTRO.STR Videos/OUTRO.STR; do \
		[ -f "$(ROOT)/$$f" ] || missing="$$missing $$f"; \
	done; \
	if [ -n "$$missing" ]; then \
		echo "ERROR: missing disc media:$$missing"; \
		echo "These are not build outputs. Restore them from your asset store."; \
		exit 1; \
	fi

Music/credits.cdda: Music/Credits-Below_the_Reeds.mp3
	@echo "Generating Music/credits.cdda from Music/Credits-Below_the_Reeds.mp3..."
	@ffmpeg -y -i $< -f s16le -ar 44100 -ac 2 $@.raw 2>/dev/null
	@python3 -c 'with open("$@.raw", "rb") as f: d = f.read()\nrem = len(d) % 2352\nif rem: d += b"\x00" * (2352 - rem)\nwith open("$@", "wb") as f: f.write(d)'
	@rm -f $@.raw
	@echo "Created $@"


run: disc
	@cue="$(DIST)/plattypus.cue"; \
	if [ -n "$(EMULATOR)" ]; then \
		echo "Launching $$cue with EMULATOR=$(EMULATOR)..."; \
		"$(EMULATOR)" "$$cue"; \
		exit $$?; \
	fi; \
	appimage=""; \
	for a in $(DUCKSTATION_APPIMAGES); do \
		if [ -f "$$a" ]; then appimage="$$a"; break; fi; \
	done; \
	if [ -n "$$appimage" ]; then \
		echo "Launching $$cue in DuckStation ($$appimage)..."; \
		APPIMAGE_EXTRACT_AND_RUN=1 "$$appimage" "$$cue"; \
		exit $$?; \
	fi; \
	if command -v duckstation-qt >/dev/null 2>&1; then \
		echo "Launching $$cue in DuckStation (Qt)..."; \
		duckstation-qt "$$cue"; exit $$?; \
	fi; \
	if command -v duckstation >/dev/null 2>&1; then \
		echo "Launching $$cue in DuckStation..."; \
		duckstation "$$cue"; exit $$?; \
	fi; \
	if command -v retroarch >/dev/null 2>&1; then \
		core="$(RETROARCH_CORE)"; \
		if [ -z "$$core" ]; then \
			for d in $(RETROARCH_CORE_DIRS); do \
				for c in $(RETROARCH_PS1_CORES); do \
					if [ -f "$$d/$$c" ]; then core="$$d/$$c"; break; fi; \
				done; \
				[ -n "$$core" ] && break; \
			done; \
		fi; \
		if [ -z "$$core" ]; then \
			echo "ERROR: retroarch is installed but no PS1 libretro core was found."; \
			echo "  RetroArch here is built for dynamic cores and will not start"; \
			echo "  without an explicit -L, so one has to be located by hand."; \
			echo "  Searched: $(RETROARCH_CORE_DIRS)"; \
			echo "  Install a core (e.g. pcsx_rearmed) or set RETROARCH_CORE=/path/to/core.so,"; \
			echo "  or set EMULATOR=/path/to/emulator to use something else."; \
			exit 1; \
		fi; \
		echo "Launching $$cue in RetroArch (core: $$core)..."; \
		retroarch -L "$$core" "$$cue"; \
		exit $$?; \
	fi; \
	echo "No PS1 emulator found. Open $$cue in DuckStation, RetroArch, or similar."; \
	echo "Set EMULATOR=/path/to/emulator, DUCKSTATION_APPIMAGE=/path/to/AppImage,"; \
	echo "or RETROARCH_CORE=/path/to/core.so to point this at your install."

clean:
	cd $(GAME_DIR) && cargo clean
	rm -rf $(DIST)
