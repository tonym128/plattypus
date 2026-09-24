ROOT     := $(CURDIR)
GAME_DIR := $(ROOT)/game
TARGET   := mipsel-sony-psx
DIST     := $(ROOT)/dist
GAME_EXE := $(GAME_DIR)/target/$(TARGET)/release/plattypus.exe
MKISOPSX := $(ROOT)/psoxide/tools/mkisopsx

.PHONY: all exe disc clean run help

all: disc

help:
	@echo "Plattypus (PSX / PSoXide) Build Targets:"
	@echo "  make exe   - Compile PSX-EXE (MIPS R3000 bare-metal binary)"
	@echo "  make disc  - Master bootable PS1 disc image (dist/plattypus.{bin,cue})"
	@echo "  make run   - Run disc in DuckStation, RetroArch, or configured emulator"
	@echo "  make clean - Clean build artifacts"

exe:
	@mkdir -p $(DIST)
	cd $(GAME_DIR) && cargo build --release
	@cp $(GAME_EXE) $(DIST)/plattypus.exe
	@echo "BUILT PSX-EXE -> $(DIST)/plattypus.exe"

disc: exe
	@mkdir -p $(DIST)
	cargo run --release --manifest-path $(MKISOPSX)/Cargo.toml -- \
		--exe $(DIST)/plattypus.exe \
		--out $(DIST)/plattypus.bin \
		--volume PLATTYPUS \
		--cdda-track $(ROOT)/Music/title_music.cdda \
		--str-file $(ROOT)/Videos/INTRO.STR \
		--str-file $(ROOT)/Videos/INTRO.VID
	@echo "SUCCESS! Bootable PS1 Disc Mastered:"
	@echo "  CUE: $(DIST)/plattypus.cue"
	@echo "  BIN: $(DIST)/plattypus.bin"

run: disc
	@if [ -x "/home/tonym/Downloads/DuckStation-x64.AppImage" ]; then \
		echo "Launching $(DIST)/plattypus.cue in DuckStation AppImage..."; \
		APPIMAGE_EXTRACT_AND_RUN=1 "/home/tonym/Downloads/DuckStation-x64.AppImage" "$(DIST)/plattypus.cue"; \
	elif command -v duckstation >/dev/null 2>&1; then \
		echo "Launching $(DIST)/plattypus.cue in DuckStation..."; \
		duckstation "$(DIST)/plattypus.cue"; \
	elif command -v duckstation-qt >/dev/null 2>&1; then \
		echo "Launching $(DIST)/plattypus.cue in DuckStation (Qt)..."; \
		duckstation-qt "$(DIST)/plattypus.cue"; \
	elif command -v retroarch >/dev/null 2>&1; then \
		echo "Launching $(DIST)/plattypus.cue in RetroArch..."; \
		retroarch "$(DIST)/plattypus.cue"; \
	elif [ -n "$$EMULATOR" ]; then \
		"$$EMULATOR" "$(DIST)/plattypus.cue"; \
	else \
		echo "Please open $(DIST)/plattypus.cue in your PS1 emulator (DuckStation, RetroArch, etc.)"; \
	fi

clean:
	cd $(GAME_DIR) && cargo clean
	rm -rf $(DIST)
